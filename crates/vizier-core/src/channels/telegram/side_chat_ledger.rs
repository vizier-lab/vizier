//! The Telegram deletion ledger (`specs/033-btw-side-chat/`, research R13).
//!
//! A side chat's messages are deleted after 2 minutes of inactivity. The exchange itself lives
//! in memory, so without this a restart would leave its messages in the chat for good. The
//! ledger is the one persisted trace: chat ids, message ids and a deadline, kept in the
//! existing `state` table under one key per agent. It holds no text, user or exchange id.

use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::storage::VizierStorage;
use crate::storage::state::StateStorage;

/// How long a side chat's messages stay after its last activity.
pub const KEEP: Duration = Duration::minutes(2);

/// Telegram refuses to delete a bot's message after 48 hours, so an older item is dropped.
const GIVE_UP: Duration = Duration::hours(48);

/// One chat's messages waiting for deletion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub chat_id: i64,
    pub message_ids: Vec<i32>,
    pub delete_after: DateTime<Utc>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ledger {
    #[serde(default)]
    pub items: Vec<Item>,
}

impl Ledger {
    /// Adds `message_ids` to the chat's item. Activity only ever moves the deadline forward.
    pub fn merge(&mut self, chat_id: i64, message_ids: &[i32], delete_after: DateTime<Utc>) {
        match self.items.iter_mut().find(|item| item.chat_id == chat_id) {
            Some(item) => {
                for id in message_ids {
                    if !item.message_ids.contains(id) {
                        item.message_ids.push(*id);
                    }
                }
                item.delete_after = item.delete_after.max(delete_after);
            }
            None => self.items.push(Item {
                chat_id,
                message_ids: message_ids.to_vec(),
                delete_after,
            }),
        }
    }

    /// Forgets `message_ids` of the chat once their deletion was attempted; an empty item goes.
    pub fn clear(&mut self, chat_id: i64, message_ids: &[i32]) {
        for item in self.items.iter_mut().filter(|item| item.chat_id == chat_id) {
            item.message_ids.retain(|id| !message_ids.contains(id));
        }
        self.items.retain(|item| !item.message_ids.is_empty());
    }

    /// Items whose deadline has passed.
    pub fn due(&self, now: DateTime<Utc>) -> Vec<Item> {
        self.items
            .iter()
            .filter(|item| item.delete_after <= now)
            .cloned()
            .collect()
    }

    /// Whether `message_id` is one the ledger is holding for deletion.
    pub fn holds(&self, chat_id: i64, message_id: i32) -> bool {
        self.items
            .iter()
            .any(|item| item.chat_id == chat_id && item.message_ids.contains(&message_id))
    }

    /// The chat's deadline, if it has an item.
    pub fn deadline(&self, chat_id: i64) -> Option<DateTime<Utc>> {
        self.items
            .iter()
            .find(|item| item.chat_id == chat_id)
            .map(|item| item.delete_after)
    }

    /// Drops items too old for Telegram to delete.
    pub fn prune(&mut self, now: DateTime<Utc>) {
        self.items.retain(|item| now - item.delete_after < GIVE_UP);
    }
}

/// The ledger in the `state` table. Every change is a read-modify-write, serialised here, and
/// the key is deleted once the ledger is empty.
#[derive(Clone)]
pub struct LedgerStore {
    storage: Arc<VizierStorage>,
    key: String,
    lock: Arc<tokio::sync::Mutex<()>>,
}

impl LedgerStore {
    pub fn new(storage: Arc<VizierStorage>, agent_id: &str) -> Self {
        Self {
            storage,
            key: format!("side_chat_pending__{agent_id}"),
            lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    async fn load(&self) -> Ledger {
        match self.storage.get_state(self.key.clone()).await {
            Ok(Some(value)) => serde_json::from_value(value).unwrap_or_default(),
            Ok(None) => Ledger::default(),
            Err(err) => {
                tracing::warn!("failed to read the side chat ledger: {err}");
                Ledger::default()
            }
        }
    }

    /// Applies `change` to the stored ledger and returns what it returns.
    pub async fn update<R>(&self, change: impl FnOnce(&mut Ledger) -> R) -> R {
        let _guard = self.lock.lock().await;
        let mut ledger = self.load().await;
        let result = change(&mut ledger);
        let saved = if ledger.items.is_empty() {
            self.storage.delete_state(self.key.clone()).await
        } else {
            match serde_json::to_value(&ledger) {
                Ok(value) => self.storage.save_state(self.key.clone(), value).await,
                Err(err) => Err(err.into()),
            }
        };
        if let Err(err) = saved {
            tracing::warn!("failed to save the side chat ledger: {err}");
        }
        result
    }

    /// Records messages to delete 2 minutes after now.
    pub async fn merge(&self, chat_id: i64, message_ids: &[i32]) {
        let delete_after = Utc::now() + KEEP;
        self.update(|ledger| ledger.merge(chat_id, message_ids, delete_after))
            .await
    }

    pub async fn clear(&self, chat_id: i64, message_ids: &[i32]) {
        self.update(|ledger| ledger.clear(chat_id, message_ids)).await
    }

    pub async fn holds(&self, chat_id: i64, message_id: i32) -> bool {
        let _guard = self.lock.lock().await;
        self.load().await.holds(chat_id, message_id)
    }

    /// The chat's deadline, or 2 minutes from now when it has none or it has passed.
    pub async fn deadline_or_soon(&self, chat_id: i64) -> DateTime<Utc> {
        let _guard = self.lock.lock().await;
        let soon = Utc::now() + KEEP;
        match self.load().await.deadline(chat_id) {
            Some(deadline) if deadline > Utc::now() => deadline,
            _ => soon,
        }
    }

    /// Records messages with an explicit deadline (never moving an existing one back).
    pub async fn merge_until(&self, chat_id: i64, message_ids: &[i32], delete_after: DateTime<Utc>) {
        self.update(|ledger| ledger.merge(chat_id, message_ids, delete_after))
            .await
    }

    /// Drops what is too old and returns what is due now.
    pub async fn take_due(&self) -> Vec<Item> {
        let now = Utc::now();
        self.update(|ledger| {
            ledger.prune(now);
            ledger.due(now)
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::VizierConfig;
    use crate::dependencies::VizierDependencies;

    fn at(minutes: i64) -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(1_800_000_000, 0).unwrap() + Duration::minutes(minutes)
    }

    #[test]
    fn merge_moves_the_deadline_forward_and_never_back() {
        let mut ledger = Ledger::default();
        ledger.merge(7, &[1, 2], at(2));
        ledger.merge(7, &[2, 3], at(5));
        assert_eq!(ledger.items.len(), 1);
        assert_eq!(ledger.items[0].message_ids, vec![1, 2, 3]);
        assert_eq!(ledger.items[0].delete_after, at(5));
        ledger.merge(7, &[4], at(3));
        assert_eq!(ledger.items[0].delete_after, at(5));
        ledger.merge(8, &[9], at(1));
        assert_eq!(ledger.items.len(), 2);
    }

    #[test]
    fn clear_drops_ids_and_then_the_empty_item() {
        let mut ledger = Ledger::default();
        ledger.merge(7, &[1, 2], at(2));
        ledger.clear(7, &[1]);
        assert_eq!(ledger.items[0].message_ids, vec![2]);
        ledger.clear(7, &[2]);
        assert!(ledger.items.is_empty());
    }

    #[test]
    fn holds_and_due_read_the_ledger() {
        let mut ledger = Ledger::default();
        ledger.merge(7, &[1], at(2));
        assert!(ledger.holds(7, 1));
        assert!(!ledger.holds(7, 2));
        assert!(!ledger.holds(8, 1));
        assert!(ledger.due(at(1)).is_empty());
        assert_eq!(ledger.due(at(2)).len(), 1);
        assert_eq!(ledger.deadline(7), Some(at(2)));
        assert_eq!(ledger.deadline(8), None);
    }

    #[test]
    fn prune_drops_what_telegram_can_no_longer_delete() {
        let mut ledger = Ledger::default();
        ledger.merge(7, &[1], at(0));
        ledger.merge(8, &[2], at(60 * 47));
        ledger.prune(at(60 * 49));
        assert_eq!(ledger.items.len(), 1);
        assert_eq!(ledger.items[0].chat_id, 8);
    }

    #[tokio::test]
    async fn the_key_goes_when_the_ledger_empties_and_holds_no_text() {
        let dir = tempfile::tempdir().unwrap();
        let deps = VizierDependencies::new(VizierConfig {
            workspace: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
        let store = LedgerStore::new(deps.storage.clone(), "agent");
        let key = "side_chat_pending__agent".to_string();

        store.merge(5, &[10, 11]).await;
        assert!(store.holds(5, 10).await);
        let stored = deps.storage.get_state(key.clone()).await.unwrap().unwrap();
        // Only chat id, message ids and a time.
        let item = &stored["items"][0];
        let mut fields: Vec<_> = item.as_object().unwrap().keys().cloned().collect();
        fields.sort();
        assert_eq!(fields, vec!["chat_id", "delete_after", "message_ids"]);

        store.clear(5, &[10]).await;
        assert!(deps.storage.get_state(key.clone()).await.unwrap().is_some());
        store.clear(5, &[11]).await;
        assert!(deps.storage.get_state(key).await.unwrap().is_none());
        assert!(!store.holds(5, 11).await);
    }

    #[tokio::test]
    async fn take_due_returns_only_past_deadlines() {
        let dir = tempfile::tempdir().unwrap();
        let deps = VizierDependencies::new(VizierConfig {
            workspace: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
        let store = LedgerStore::new(deps.storage.clone(), "agent");
        store.merge_until(5, &[1], Utc::now() - Duration::seconds(1)).await;
        store.merge_until(6, &[2], Utc::now() + Duration::minutes(5)).await;
        let due = store.take_due().await;
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].chat_id, 5);
    }
}
