//! The notification inbox (`specs/029-webui-notifications/`). One creation path: every notable
//! event calls [`Notifications`] at the moment it completes, with a deterministic `event_key`
//! so a retried transition stores nothing twice. The one aggregate source — replies in a
//! Discord or Telegram conversation — is projected from `session_history` on read, under a
//! watermark, because a conversation-hour has no completion code path.
//!
//! Writing a notification is always best-effort: a failure is logged and never fails the task
//! run, dream, background job, integration write or turn that caused it.

pub mod cursor;
pub mod text;

use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::{
    schema::{
        EventAction, IntegrationFamily, JobKind, JobState, NotificationCategory, NotificationDraft,
        NotificationTone, Requester, TaskRunState, VizierChannelId, VizierSession,
    },
    storage::{
        VizierStorage, agent::AgentStorage, notification::NotificationStorage, state::StateStorage,
        user::UserStorage,
    },
};

/// The state key holding the last closed UTC hour projected into channel-reply notifications.
pub const REPLIES_THROUGH: &str = "notifications.replies_through";

const HOUR_MS: i64 = 3_600_000;

/// A long downtime catches up only the retained window.
const CATCH_UP_HOURS: i64 = 30 * 24;

#[derive(Clone)]
pub struct Notifications {
    storage: Arc<VizierStorage>,
}

impl Notifications {
    pub fn new(storage: Arc<VizierStorage>) -> Self {
        Self { storage }
    }

    pub fn storage(&self) -> &VizierStorage {
        &self.storage
    }

    /// Record one notification, best-effort. A row for an agent that no longer exists is
    /// skipped: this closes the delete/insert race.
    pub async fn record(&self, draft: NotificationDraft) {
        match self.storage.get_agent(&draft.agent_id).await {
            Ok(Some(_)) => {}
            Ok(None) => return,
            Err(e) => {
                tracing::warn!(
                    event_key = %draft.event_key,
                    "could not confirm the notification's agent still exists: {e}"
                );
                return;
            }
        }
        if let Err(e) = self.storage.insert_notifications(&[draft]).await {
            tracing::warn!("failed to record a notification: {e}");
        }
    }

    /// Record several drafts in one best-effort batch (the reply projection, integration
    /// writes). Storage skips drafts for missing agents.
    pub async fn record_all(&self, drafts: Vec<NotificationDraft>) -> usize {
        match self.storage.insert_notifications(&drafts).await {
            Ok(inserted) => inserted,
            Err(e) => {
                tracing::warn!("failed to record {} notification(s): {e}", drafts.len());
                0
            }
        }
    }

    // -----------------------------------------------------------------------------------------
    // Sources
    // -----------------------------------------------------------------------------------------

    /// A task run that has just closed. `title` falls back to the slug when the task is gone.
    pub async fn task_run_ended(
        &self,
        agent_id: &str,
        task_slug: &str,
        title: &str,
        run_id: i64,
        state: TaskRunState,
        at: DateTime<Utc>,
    ) {
        let Some((summary, tone)) = text::task_run(title, state) else {
            return;
        };
        self.record(draft(
            format!("task_run:{run_id}"),
            agent_id,
            NotificationCategory::TaskRun,
            tone,
            summary,
            text::task_href(agent_id, task_slug, run_id),
            at,
        ))
        .await;
    }

    pub async fn dream_ended(
        &self,
        agent_id: &str,
        cycle_id: &str,
        entries: u64,
        at: DateTime<Utc>,
    ) {
        if entries == 0 {
            return;
        }
        self.record(draft(
            format!("dream:{cycle_id}"),
            agent_id,
            NotificationCategory::DreamCycle,
            NotificationTone::Default,
            text::dream(entries),
            text::dream_href(agent_id, cycle_id),
            at,
        ))
        .await;
    }

    /// One background job at a terminal state; a job still in flight records nothing.
    #[allow(clippy::too_many_arguments)]
    pub async fn job_ended(
        &self,
        job_id: &str,
        agent_id: &str,
        kind: JobKind,
        state: JobState,
        reason: Option<&str>,
        at: DateTime<Utc>,
    ) {
        let Some((summary, tone)) = text::background_job(kind, state, reason) else {
            return;
        };
        self.record(draft(
            format!("job:{job_id}"),
            agent_id,
            NotificationCategory::BackgroundJob,
            tone,
            summary,
            text::chat_href(agent_id),
            at,
        ))
        .await;
    }

    /// One notification per affected agent after an integration write committed. Never
    /// includes a setting value.
    #[allow(clippy::too_many_arguments)]
    pub async fn integration_event(
        &self,
        integration_id: &str,
        name: &str,
        family: IntegrationFamily,
        actor: &Requester,
        action: EventAction,
        ordinal: usize,
        agent_ids: &[String],
        at: DateTime<Utc>,
    ) {
        let who = self.actor_name(actor).await;
        let summary = text::integration_changed(&who, action, name);
        let href = |agent_id: &str| text::integrations_href(agent_id, family);
        let drafts = agent_ids
            .iter()
            .map(|agent_id| {
                draft(
                    format!(
                        "integration:{integration_id}:{}:{}:{ordinal}:{agent_id}",
                        action.as_str(),
                        at.timestamp_millis()
                    ),
                    agent_id,
                    NotificationCategory::Integration,
                    NotificationTone::Default,
                    summary.clone(),
                    href(agent_id),
                    at,
                )
            })
            .collect();
        self.record_all(drafts).await;
    }

    /// A self-management grant switched on or off.
    pub async fn integration_grant(
        &self,
        agent_id: &str,
        family: IntegrationFamily,
        on: bool,
        actor: &Requester,
        at: DateTime<Utc>,
    ) {
        let who = self.actor_name(actor).await;
        self.record(draft(
            format!(
                "grant:{agent_id}:{}:{}:{}",
                family.as_str(),
                if on { "on" } else { "off" },
                at.timestamp_millis()
            ),
            agent_id,
            NotificationCategory::Integration,
            NotificationTone::Default,
            text::integration_grant(&who, family, on),
            text::integrations_href(agent_id, family),
            at,
        ))
        .await;
    }

    /// A WebUI conversation's reply, addressed to the person whose conversation it is: the
    /// session's channel carries their username, which resolves to the id read state uses.
    /// Discord/Telegram keep the hourly `channel_replies` aggregate instead.
    pub async fn chat_replied(
        &self,
        session: &VizierSession,
        history_uid: &str,
        at: DateTime<Utc>,
    ) {
        let VizierChannelId::HTTP(username, _) = &session.1 else {
            return;
        };
        let audience = self
            .storage
            .get_user(username)
            .await
            .ok()
            .flatten()
            .map(|user| user.user_id);
        let Some(audience) = audience else {
            return;
        };
        let href = match &session.2 {
            Some(topic) => format!("/{}/chat/{}", session.0, topic),
            None => text::chat_href(&session.0),
        };
        let mut draft = draft(
            format!("chat:{history_uid}"),
            &session.0,
            NotificationCategory::ChatReply,
            NotificationTone::Default,
            text::chat_replied().to_string(),
            href,
            at,
        );
        draft.audience_user_id = Some(audience);
        self.record(draft).await;
    }

    pub async fn agent_ready(&self, agent_id: &str, build_id: &str, name: &str, at: DateTime<Utc>) {
        self.record(draft(
            format!("setup_ready:{agent_id}:{build_id}"),
            agent_id,
            NotificationCategory::AgentSetupReady,
            NotificationTone::Success,
            text::agent_ready(name),
            text::chat_href(agent_id),
            at,
        ))
        .await;
    }

    pub async fn agent_start_failed(
        &self,
        agent_id: &str,
        episode_id: &str,
        error: &str,
        retry_secs: Option<i64>,
        at: DateTime<Utc>,
    ) {
        self.record(draft(
            format!("start_failed:{agent_id}:{episode_id}"),
            agent_id,
            NotificationCategory::AgentStartFailure,
            NotificationTone::Failed,
            text::agent_start_failed(error, retry_secs),
            text::config_href(agent_id),
            at,
        ))
        .await;
    }

    /// `Requester::User` resolves to the username, `Requester::Agent` to the agent's name, and
    /// either falls back to the id.
    async fn actor_name(&self, actor: &Requester) -> String {
        crate::utils::actor_display_name(&self.storage, actor).await
    }

    // -----------------------------------------------------------------------------------------
    // Channel replies and retention
    // -----------------------------------------------------------------------------------------

    /// Project every closed hour after the watermark into one notification per
    /// `(agent, channel, hour)`. First activation starts at the last closed hour and imports no
    /// history, so the inbox begins empty.
    pub async fn project_replies(&self, now: DateTime<Utc>) -> Result<usize> {
        let now_hour = now.timestamp_millis() / HOUR_MS;
        let through = self
            .storage
            .get_state(REPLIES_THROUGH.to_string())
            .await?
            .and_then(|v| v.as_i64());

        let Some(through) = through else {
            self.storage
                .save_state(REPLIES_THROUGH.to_string(), serde_json::json!(now_hour - 1))
                .await?;
            return Ok(0);
        };

        let to = now_hour - 1;
        let from = (through + 1).max(now_hour - CATCH_UP_HOURS);
        if from > to {
            return Ok(0);
        }

        let batches = self.storage.channel_reply_batches(from, to).await?;
        let drafts = batches
            .iter()
            .map(|batch| {
                draft(
                    format!("reply:{}:{}:{}", batch.agent_id, batch.channel, batch.hour),
                    &batch.agent_id,
                    NotificationCategory::ChannelReplies,
                    NotificationTone::Default,
                    text::channel_replies(&batch.channel, batch.replies),
                    text::channel_href(&batch.agent_id, &batch.channel),
                    batch.at,
                )
            })
            .collect();
        let inserted = self.record_all(drafts).await;
        self.storage
            .save_state(REPLIES_THROUGH.to_string(), serde_json::json!(to))
            .await?;
        Ok(inserted)
    }

    /// Retention: delete rows older than `at`. Called at startup and once a day.
    pub async fn purge_before(&self, at: DateTime<Utc>) -> Result<usize> {
        self.storage.purge_notifications_before(at).await
    }

    /// The startup orphan cleanup, for a delete/insert race that left a row behind.
    pub async fn purge_orphans(&self) -> Result<usize> {
        self.storage.purge_orphaned_notifications().await
    }
}

#[allow(clippy::too_many_arguments)]
fn draft(
    event_key: String,
    agent_id: &str,
    category: NotificationCategory,
    tone: NotificationTone,
    summary: String,
    href: String,
    at: DateTime<Utc>,
) -> NotificationDraft {
    NotificationDraft {
        event_key,
        agent_id: agent_id.to_string(),
        audience_user_id: None,
        category,
        tone,
        summary,
        href,
        created_at: at,
    }
}

#[cfg(test)]
mod tests {
    use parking_lot::Mutex;

    use super::*;
    use crate::{
        schema::NotificationFilter,
        storage::{
            document::LocalDocumentStore,
            notification::NotificationStorage,
            sqlite::{SqliteStorage, init_history_schema, init_notification_schema},
        },
    };

    fn setup() -> (
        Notifications,
        Arc<Mutex<rusqlite::Connection>>,
        tempfile::TempDir,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        init_notification_schema(&conn).unwrap();
        init_history_schema(&conn).unwrap();
        conn.execute_batch(
            "CREATE TABLE agent_config (agent_id TEXT PRIMARY KEY, data TEXT NOT NULL);
             CREATE TABLE state (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE \"user\" (user_id TEXT PRIMARY KEY, username TEXT NOT NULL UNIQUE, data TEXT NOT NULL);",
        )
        .unwrap();
        let conn = Arc::new(Mutex::new(conn));
        let storage = VizierStorage::new(SqliteStorage::new(
            conn.clone(),
            Arc::new(LocalDocumentStore::new(dir.path().to_path_buf())),
        ));
        (Notifications::new(Arc::new(storage)), conn, dir)
    }

    fn agent(conn: &Arc<Mutex<rusqlite::Connection>>, id: &str) {
        let config = crate::schema::AgentConfig {
            name: format!("Agent {id}"),
            owner_id: None,
            shared_to: vec![],
            system_prompt: None,
            description: None,
            provider: crate::config::provider::ProviderId::DUMMYPLUG,
            model: "dummyplug".into(),
            thinking_depth: 1,
            checkpoint_threshold: 0.8,
            tools: Default::default(),
            silent_read_initiative_chance: 0.0,
            max_tokens: None,
            context_window: None,
            include_documents: None,
            prompt_timeout: Default::default(),
            documents: vec![],
            heartbeat_interval: Default::default(),
            core: None,
            dream_enabled: false,
            dream_schedule: None,
            dream_provider: None,
            dream_model: None,
            avatar_url: None,
            embedding: None,
            indexer: None,
            chunking: Default::default(),
            auto_context: Default::default(),
            completion_retry: Default::default(),
        };
        conn.lock()
            .execute(
                "INSERT OR IGNORE INTO agent_config (agent_id, data) VALUES (?1, ?2)",
                rusqlite::params![id, serde_json::to_string(&config).unwrap()],
            )
            .unwrap();
    }

    #[tokio::test]
    async fn a_missing_agent_records_nothing() {
        let (n, conn, _d) = setup();
        agent(&conn, "a");
        n.task_run_ended("gone", "t", "T", 1, TaskRunState::Answered, Utc::now())
            .await;
        assert_eq!(
            n.storage
                .notification_counts("u", &["gone".into()])
                .await
                .unwrap()
                .all,
            0
        );
    }

    #[tokio::test]
    async fn a_duplicate_event_key_is_one_row() {
        let (n, conn, _d) = setup();
        agent(&conn, "a");
        let at = Utc::now();
        n.task_run_ended("a", "brief", "Brief", 7, TaskRunState::Answered, at)
            .await;
        n.task_run_ended("a", "brief", "Brief", 7, TaskRunState::Answered, at)
            .await;
        let items = n
            .storage
            .list_notifications("u", &["a".into()], NotificationFilter::All, None, 10)
            .await
            .unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].text, "Task “Brief” answered.");
        assert_eq!(items[0].href, "/a/tasks?task=brief&run=7");
    }

    #[tokio::test]
    async fn the_reply_projection_starts_at_the_last_closed_hour_and_never_backfills() {
        let (n, _conn, _d) = setup();
        let now = Utc::now();
        assert_eq!(n.project_replies(now).await.unwrap(), 0);
        let through = n
            .storage
            .get_state(REPLIES_THROUGH.to_string())
            .await
            .unwrap()
            .and_then(|v| v.as_i64())
            .unwrap();
        assert_eq!(through, now.timestamp_millis() / HOUR_MS - 1);
    }

    #[tokio::test]
    async fn a_closed_hour_becomes_one_row_per_conversation_and_the_current_hour_waits() {
        let (n, conn, _d) = setup();
        let now = Utc::now();
        let hour = now.timestamp_millis() / HOUR_MS;
        agent(&conn, "a");
        const HOUR_MS_LOCAL: i64 = 3_600_000;
        {
            let c = conn.lock();
            let mut put = |uid: i64, channel: &str, ts: i64| {
                c.execute(
                    "INSERT INTO session_history (uid, agent_id, channel, topic, timestamp, content_type, data, seq) VALUES (?1, 'a', ?2, NULL, ?3, 'Response', '{}', ?4)",
                    rusqlite::params![format!("h{uid}"), channel, ts, uid],
                )
                .unwrap();
            };
            // Two hours ago, now closed.
            put(1, "discord__1", (hour - 2) * HOUR_MS_LOCAL + 5);
            put(2, "discord__1", (hour - 2) * HOUR_MS_LOCAL + 9);
            put(3, "telegram__5", (hour - 2) * HOUR_MS_LOCAL + 2);
            // The current, still-open hour.
            put(4, "discord__1", hour * HOUR_MS_LOCAL + 1);
        }
        // First activation imports nothing, even though replies already exist.
        n.project_replies(now - chrono::Duration::hours(2))
            .await
            .unwrap();
        assert_eq!(
            n.storage
                .notification_counts("u", &["a".into()])
                .await
                .unwrap()
                .all,
            0
        );

        let inserted = n.project_replies(now).await.unwrap();
        assert_eq!(inserted, 2, "one per conversation-hour");
        assert_eq!(n.project_replies(now).await.unwrap(), 0, "idempotent");

        let items = n
            .storage
            .list_notifications("u", &["a".into()], NotificationFilter::All, None, 10)
            .await
            .unwrap();
        let texts: Vec<&str> = items.iter().map(|i| i.text.as_str()).collect();
        assert!(texts.contains(&"Replied 2 times in a Discord conversation."));
        assert!(texts.contains(&"Replied once in a Telegram conversation."));
        assert_eq!(items.len(), 2, "the still-open hour is not projected");
    }

    fn user(conn: &Arc<Mutex<rusqlite::Connection>>, id: &str, username: &str) {
        let user = crate::storage::user::User {
            user_id: id.into(),
            username: username.into(),
            password_hash: String::new(),
            role_id: "r".into(),
            created_at: Utc::now(),
        };
        conn.lock()
            .execute(
                r#"INSERT OR IGNORE INTO "user" (user_id, username, data) VALUES (?1, ?2, ?3)"#,
                rusqlite::params![id, username, serde_json::to_string(&user).unwrap()],
            )
            .unwrap();
    }

    #[tokio::test]
    async fn a_webui_reply_is_addressed_to_its_person_and_a_platform_reply_is_not() {
        let (n, conn, _d) = setup();
        agent(&conn, "a");
        user(&conn, "u-ana", "ana");

        let session = VizierSession(
            "a".into(),
            VizierChannelId::HTTP("ana".into(), "vizier-webui".into()),
            Some("topic-1".into()),
        );
        n.chat_replied(&session, "h-1", Utc::now()).await;

        let mine = n
            .storage
            .list_notifications("u-ana", &["a".into()], NotificationFilter::All, None, 10)
            .await
            .unwrap();
        assert_eq!(mine.len(), 1);
        assert_eq!(mine[0].category, NotificationCategory::ChatReply);
        assert_eq!(mine[0].text, "Replied in your conversation.");
        assert_eq!(mine[0].href, "/a/chat/topic-1");
        assert_eq!(
            n.storage
                .list_notifications("u-bob", &["a".into()], NotificationFilter::All, None, 10)
                .await
                .unwrap()
                .len(),
            0,
            "only the person whose conversation it is sees it"
        );

        // Discord/Telegram keep the hourly aggregate; no per-turn row.
        let discord = VizierSession("a".into(), VizierChannelId::DiscordChanel(1), None);
        n.chat_replied(&discord, "h-2", Utc::now()).await;
        assert_eq!(
            n.storage
                .list_notifications("u-ana", &["a".into()], NotificationFilter::All, None, 10)
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn purge_removes_rows_older_than_the_cutoff() {
        let (n, conn, _d) = setup();
        agent(&conn, "a");
        n.task_run_ended("a", "t", "T", 1, TaskRunState::Answered, Utc::now())
            .await;
        assert_eq!(
            n.purge_before(Utc::now() + chrono::Duration::days(1))
                .await
                .unwrap(),
            1
        );
    }
}
