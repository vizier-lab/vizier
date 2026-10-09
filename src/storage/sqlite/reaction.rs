use anyhow::Result;
use chrono::Utc;
use rusqlite::OptionalExtension;

use crate::{
    schema::{ReactionEntry, SessionHistory},
    storage::{
        reaction::{Platform, ReactionStorage, Reactor},
        sqlite::{
            SqliteStorage,
            history::{fill_reactions, parse_history_row},
        },
    },
};

#[async_trait::async_trait]
impl ReactionStorage for SqliteStorage {
    async fn add_reaction(&self, history_uid: &str, reactor: &Reactor, emoji: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR IGNORE INTO message_reaction
                (history_uid, reactor_id, reactor_name, emoji, added_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                history_uid,
                reactor.id,
                reactor.name,
                emoji,
                Utc::now().timestamp_millis()
            ],
        )?;
        Ok(())
    }

    async fn remove_reaction(
        &self,
        history_uid: &str,
        reactor_id: &str,
        emoji: &str,
    ) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM message_reaction
             WHERE history_uid = ?1 AND reactor_id = ?2 AND emoji = ?3",
            rusqlite::params![history_uid, reactor_id, emoji],
        )?;
        Ok(())
    }

    async fn clear_reactions(&self, history_uid: &str, emoji: Option<&str>) -> Result<()> {
        let conn = self.conn.lock();
        match emoji {
            Some(emoji) => conn.execute(
                "DELETE FROM message_reaction WHERE history_uid = ?1 AND emoji = ?2",
                rusqlite::params![history_uid, emoji],
            )?,
            None => conn.execute(
                "DELETE FROM message_reaction WHERE history_uid = ?1",
                rusqlite::params![history_uid],
            )?,
        };
        Ok(())
    }

    async fn list_reactions(&self, history_uid: &str) -> Result<Vec<ReactionEntry>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT reactor_id, reactor_name, emoji FROM message_reaction
             WHERE history_uid = ?1 ORDER BY added_at, rowid",
        )?;
        let reactions = stmt
            .query_map(rusqlite::params![history_uid], |row| {
                Ok(ReactionEntry {
                    user_id: row.get(0)?,
                    user_name: row.get(1)?,
                    emoji: row.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(reactions)
    }

    async fn link_platform_messages(
        &self,
        agent_id: &str,
        platform: Platform,
        chat_id: &str,
        message_ids: &[String],
        history_uid: &str,
    ) -> Result<()> {
        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        for message_id in message_ids {
            tx.execute(
                "INSERT OR REPLACE INTO platform_message_link
                    (agent_id, platform, chat_id, message_id, history_uid)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![agent_id, platform.as_str(), chat_id, message_id, history_uid],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    async fn find_linked_message(
        &self,
        agent_id: &str,
        platform: Platform,
        chat_id: &str,
        message_id: &str,
    ) -> Result<Option<SessionHistory>> {
        let conn = self.conn.lock();
        let row = conn
            .query_row(
                "SELECT h.data, h.seq FROM platform_message_link l
                 JOIN session_history h ON h.uid = l.history_uid
                 WHERE l.agent_id = ?1 AND l.platform = ?2 AND l.chat_id = ?3 AND l.message_id = ?4",
                rusqlite::params![agent_id, platform.as_str(), chat_id, message_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        with_reactions(&conn, row)
    }

    async fn get_history_entry(&self, history_uid: &str) -> Result<Option<SessionHistory>> {
        let conn = self.conn.lock();
        let row = conn
            .query_row(
                "SELECT data, seq FROM session_history WHERE uid = ?1",
                rusqlite::params![history_uid],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        with_reactions(&conn, row)
    }
}

fn with_reactions(
    conn: &rusqlite::Connection,
    row: Option<(String, Option<i64>)>,
) -> Result<Option<SessionHistory>> {
    let Some(mut entry) = row.and_then(parse_history_row) else {
        return Ok(None);
    };
    fill_reactions(conn, std::slice::from_mut(&mut entry))?;
    Ok(Some(entry))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use parking_lot::Mutex;
    use rusqlite::Connection;

    use super::*;
    use crate::schema::{
        SessionHistoryContent, VizierChannelId, VizierResponse, VizierResponseContent,
        VizierSession,
    };
    use crate::storage::{
        document::LocalDocumentStore,
        history::HistoryStorage,
        sqlite::{init_history_schema, init_reaction_schema},
    };

    fn setup() -> (SqliteStorage, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        init_history_schema(&conn).unwrap();
        init_reaction_schema(&conn).unwrap();
        let storage = SqliteStorage::new(
            Arc::new(Mutex::new(conn)),
            Arc::new(LocalDocumentStore::new(dir.path().to_path_buf())),
        );
        (storage, dir)
    }

    fn session() -> VizierSession {
        VizierSession(
            "agent-1".to_string(),
            VizierChannelId::HTTP("someone".to_string(), "webui".to_string()),
            Some("General".to_string()),
        )
    }

    async fn reply(storage: &SqliteStorage) -> String {
        storage
            .save_session_history(
                session(),
                SessionHistoryContent::Response(VizierResponse {
                    timestamp: Utc::now(),
                    content: VizierResponseContent::Message {
                        content: "hello".to_string(),
                        stats: None,
                    },
                    ..Default::default()
                }),
            )
            .await
            .unwrap()
    }

    fn alice() -> Reactor {
        Reactor {
            id: "alice".to_string(),
            name: Some("Alice".to_string()),
        }
    }

    fn emojis(reactions: &[ReactionEntry]) -> Vec<&str> {
        reactions.iter().map(|r| r.emoji.as_str()).collect()
    }

    #[tokio::test]
    async fn adding_twice_keeps_one_row() {
        let (storage, _dir) = setup();
        let uid = reply(&storage).await;
        storage.add_reaction(&uid, &alice(), "👍").await.unwrap();
        storage.add_reaction(&uid, &alice(), "👍").await.unwrap();

        let reactions = storage.list_reactions(&uid).await.unwrap();
        assert_eq!(reactions.len(), 1);
        assert_eq!(reactions[0].user_id, "alice");
        assert_eq!(reactions[0].user_name.as_deref(), Some("Alice"));
    }

    /// FR-002: a remove is never a toggle.
    #[tokio::test]
    async fn removing_an_absent_reaction_never_adds_one() {
        let (storage, _dir) = setup();
        let uid = reply(&storage).await;
        storage.remove_reaction(&uid, "alice", "👍").await.unwrap();
        assert!(storage.list_reactions(&uid).await.unwrap().is_empty());

        storage.add_reaction(&uid, &alice(), "👍").await.unwrap();
        storage.remove_reaction(&uid, "alice", "👍").await.unwrap();
        assert!(storage.list_reactions(&uid).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn clearing_one_emoji_leaves_the_others() {
        let (storage, _dir) = setup();
        let uid = reply(&storage).await;
        let bob = Reactor {
            id: "bob".to_string(),
            name: None,
        };
        storage.add_reaction(&uid, &alice(), "👍").await.unwrap();
        storage.add_reaction(&uid, &bob, "👍").await.unwrap();
        storage.add_reaction(&uid, &alice(), "🎉").await.unwrap();

        storage.clear_reactions(&uid, Some("👍")).await.unwrap();
        assert_eq!(emojis(&storage.list_reactions(&uid).await.unwrap()), vec!["🎉"]);

        storage.clear_reactions(&uid, None).await.unwrap();
        assert!(storage.list_reactions(&uid).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn platform_links_resolve_to_their_entry() {
        let (storage, _dir) = setup();
        let uid = reply(&storage).await;
        storage
            .link_platform_messages(
                "agent-1",
                Platform::Discord,
                "chan",
                &["m1".to_string(), "m2".to_string()],
                &uid,
            )
            .await
            .unwrap();
        storage.add_reaction(&uid, &alice(), "👍").await.unwrap();

        for message_id in ["m1", "m2"] {
            let entry = storage
                .find_linked_message("agent-1", Platform::Discord, "chan", message_id)
                .await
                .unwrap()
                .expect("linked");
            assert_eq!(entry.uid, uid);
            assert_eq!(emojis(&entry.reactions), vec!["👍"]);
        }

        for (platform, chat, message) in [
            (Platform::Discord, "chan", "m3"),
            (Platform::Telegram, "chan", "m1"),
            (Platform::Discord, "other", "m1"),
        ] {
            assert!(
                storage
                    .find_linked_message("agent-1", platform, chat, message)
                    .await
                    .unwrap()
                    .is_none()
            );
        }
        assert!(
            storage
                .find_linked_message("agent-2", Platform::Discord, "chan", "m1")
                .await
                .unwrap()
                .is_none()
        );

        let entry = storage.get_history_entry(&uid).await.unwrap().unwrap();
        assert_eq!(entry.vizier_session, session());
        assert!(storage.get_history_entry("nope").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn blob_reactions_move_into_the_table_once() {
        let (storage, _dir) = setup();
        let uid = reply(&storage).await;
        {
            let conn = storage.conn.lock();
            conn.execute(
                "UPDATE session_history
                    SET data = json_set(data, '$.reactions', json('[{\"user_id\":\"a\",\"emoji\":\"👍\"}]'))
                  WHERE uid = ?1",
                rusqlite::params![uid],
            )
            .unwrap();
        }

        for _ in 0..2 {
            let conn = storage.conn.lock();
            init_reaction_schema(&conn).unwrap();

            let rows: i64 = conn
                .query_row("SELECT COUNT(*) FROM message_reaction", [], |row| row.get(0))
                .unwrap();
            assert_eq!(rows, 1);
            let blob: String = conn
                .query_row(
                    "SELECT json_extract(data, '$.reactions') FROM session_history WHERE uid = ?1",
                    rusqlite::params![uid],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(blob, "[]");
        }

        let list = storage
            .list_session_history(session(), None, None, None)
            .await
            .unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].reactions.len(), 1);
        assert_eq!(list[0].reactions[0].user_id, "a");
        assert_eq!(list[0].reactions[0].emoji, "👍");
        assert_eq!(list[0].reactions[0].user_name, None);
    }
}
