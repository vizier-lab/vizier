//! The notification inbox (`specs/029-webui-notifications/`). One creation path: every notable
//! event calls [`Notifications`] at the moment it completes, with a deterministic `event_key`
//! so a retried transition stores nothing twice. Discord and Telegram activity creates no
//! notification (`specs/051-realtime-notifications`).
//!
//! Writing a notification is always best-effort: a failure is logged and never fails the task
//! run, dream, background job, integration write or turn that caused it.

pub mod cursor;
pub mod excerpt;
pub mod text;

use std::sync::{Arc, RwLock};

use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::{
    schema::{
        EventAction, InsertedNotification, IntegrationFamily, JobKind, JobState,
        NotificationCategory, NotificationDraft, NotificationTone, Requester, TaskRunState,
        SessionHistory, SessionHistoryContent, VizierChannelId, VizierSession,
    },
    storage::{
        VizierStorage, agent::AgentStorage, history::HistoryStorage,
        notification::NotificationStorage, task_run::TaskRunStorage, user::UserStorage,
    },
    transport::{InboxEvent, VizierTransport},
};

#[derive(Clone)]
pub struct Notifications {
    storage: Arc<VizierStorage>,
    /// Where inserted rows are announced for the live inbox stream, once the transport exists.
    /// A row inserted before then is simply not announced; the table is the replay buffer.
    transport: Arc<RwLock<Option<VizierTransport>>>,
}

impl Notifications {
    pub fn new(storage: Arc<VizierStorage>) -> Self {
        Self { storage, transport: Arc::default() }
    }

    pub fn storage(&self) -> &VizierStorage {
        &self.storage
    }

    /// Announce new rows on the inbox stream from now on. Called once, when the transport
    /// exists, like `SetupBoard::install_publisher`.
    pub fn install_transport(&self, transport: VizierTransport) {
        if let Ok(mut slot) = self.transport.write() {
            *slot = Some(transport);
        }
    }

    /// Tell the inbox stream a row now exists. Publishing never fails the caller.
    fn announce(&self, inserted: Vec<InsertedNotification>) {
        if inserted.is_empty() {
            return;
        }
        let Some(transport) = self.transport.read().ok().and_then(|slot| slot.clone()) else {
            return;
        };
        for InsertedNotification { item, audience } in inserted {
            let source_session = item.source_session.clone();
            transport.publish_inbox_event(InboxEvent::Created { item, audience, source_session });
        }
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
        match self.storage.insert_notifications(&[draft]).await {
            Ok(inserted) => self.announce(inserted),
            Err(e) => tracing::warn!("failed to record a notification: {e}"),
        }
    }

    /// Record several drafts in one best-effort batch (the reply projection, integration
    /// writes). Storage skips drafts for missing agents.
    pub async fn record_all(&self, drafts: Vec<NotificationDraft>) -> usize {
        match self.storage.insert_notifications(&drafts).await {
            Ok(inserted) => {
                let count = inserted.len();
                self.announce(inserted);
                count
            }
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
        let mut draft = draft(
            format!("task_run:{run_id}"),
            agent_id,
            NotificationCategory::TaskRun,
            tone,
            summary,
            text::task_href(agent_id, task_slug, run_id),
            at,
        );
        // A task has no conversation to name, so its title is kept as written now.
        draft.source_title = Some(title.to_string());
        draft.excerpt = self.task_excerpt(agent_id, task_slug, run_id).await;
        self.record(draft).await;
    }

    /// What an answered run said, from the run's own conversation.
    async fn task_excerpt(&self, agent_id: &str, task_slug: &str, run_id: i64) -> Option<String> {
        let runs = self
            .storage
            .list_task_runs(agent_id.to_string(), task_slug.to_string(), None, None, 10)
            .await
            .ok()?;
        let run = runs.into_iter().find(|run| run.id == run_id)?;
        let answer = crate::storage::task_run::task_run_response(&self.storage, &run).await?;
        excerpt::from_markdown(&answer)
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

    /// One background job at a terminal state; a job still in flight records nothing. `origin`
    /// is the conversation the job was launched from: one on Discord or Telegram notifies
    /// nobody (the report still posts in the channel), and any other is kept as the row's
    /// source so its title can be shown to the person it belongs to.
    pub async fn job_ended(
        &self,
        job_id: &str,
        origin: &VizierSession,
        kind: JobKind,
        state: JobState,
        reason: Option<&str>,
        at: DateTime<Utc>,
    ) {
        if origin.1.platform().is_some() {
            return;
        }
        let Some((summary, tone)) = text::background_job(kind, state, reason) else {
            return;
        };
        let mut draft = draft(
            format!("job:{job_id}"),
            &origin.0,
            NotificationCategory::BackgroundJob,
            tone,
            summary,
            text::chat_href(&origin.0),
            at,
        );
        draft.source_session = Some(origin.clone());
        self.record(draft).await;
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
    /// Discord and Telegram conversations never notify: only a WebUI conversation has a
    /// `web_user`.
    pub async fn chat_replied(
        &self,
        session: &VizierSession,
        history_uid: &str,
        at: DateTime<Utc>,
    ) {
        let Some(username) = session.1.web_user() else {
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
        let href = match (&session.2, session.1.project_id()) {
            // A project reply opens its project route, not the ordinary chat route.
            (Some(topic), Some(project_id)) => {
                format!("/{}/projects/{}/chat/{}", session.0, project_id, topic)
            }
            (Some(topic), None) => format!("/{}/chat/{}", session.0, topic),
            (None, _) => text::chat_href(&session.0),
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
        draft.source_session = Some(session.clone());
        // From the saved entry, so only what has been through saving (and redaction) can appear.
        draft.excerpt = match self.storage.get_session_history_entry(session, history_uid).await {
            Ok(Some(SessionHistory { content: SessionHistoryContent::Response(reply), .. })) => {
                excerpt::of_response(&reply)
            }
            _ => None,
        };
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
    // Retention
    // -----------------------------------------------------------------------------------------

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
        source_session: None,
        source_title: None,
        excerpt: None,
    }
}

#[cfg(test)]
mod tests {
    use crate::storage::sql::{SqlStore, params};

    use super::*;
    use crate::{
        schema::{NotificationFilter, SessionHistoryContent},
        storage::{
            document::BlobDocumentStore,
            notification::NotificationStorage,
            sqlite::{SqlStorage, init_history_schema, init_notification_schema},
        },
    };

    fn setup() -> (Notifications, SqlStore, tempfile::TempDir) {
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
        let conn = SqlStore::sqlite(conn);
        let storage = VizierStorage::new(SqlStorage::new(
            conn.clone(),
            Arc::new(BlobDocumentStore::local(dir.path().to_path_buf())),
        ));
        (Notifications::new(Arc::new(storage)), conn, dir)
    }

    fn agent(conn: &SqlStore, id: &str) {
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
            limits: Default::default(),
            auto_context: Default::default(),
            completion_retry: Default::default(),
        };
        conn.lock()
            .unwrap()
            .execute(
                "INSERT OR IGNORE INTO agent_config (agent_id, data) VALUES (?1, ?2)",
                params![id, serde_json::to_string(&config).unwrap()],
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
    async fn a_new_row_is_announced_once_and_a_duplicate_event_key_announces_nothing() {
        let (n, conn, _d) = setup();
        agent(&conn, "a");
        let transport = VizierTransport::new();
        n.install_transport(transport.clone());
        let mut rx = transport.subscribe_inbox_events();
        let at = Utc::now();
        n.task_run_ended("a", "brief", "Brief", 7, TaskRunState::Answered, at).await;
        n.task_run_ended("a", "brief", "Brief", 7, TaskRunState::Answered, at).await;

        let got = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
            .await
            .expect("an announcement in time")
            .unwrap();
        let InboxEvent::Created { item, audience, .. } = got else { panic!("expected created") };
        assert_eq!(item.agent_id, "a");
        assert_eq!(item.text, "Task “Brief” answered.");
        assert!(!item.read);
        assert!(audience.is_none(), "an agent-wide row has no audience");

        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        assert!(rx.try_recv().is_err(), "the duplicate was announced");
    }

    #[tokio::test]
    async fn a_row_is_stored_even_when_nothing_is_listening() {
        let (n, conn, _d) = setup();
        agent(&conn, "a");
        // No transport installed: the table is still the source of truth.
        n.task_run_ended("a", "brief", "Brief", 1, TaskRunState::Answered, Utc::now()).await;
        assert_eq!(n.storage.notification_counts("u", &["a".into()]).await.unwrap().all, 1);
    }

    #[tokio::test]
    async fn a_job_launched_from_discord_or_telegram_notifies_nobody() {
        let (n, conn, _d) = setup();
        agent(&conn, "a");
        for channel in [VizierChannelId::DiscordChanel(1), VizierChannelId::TelegramChannel(2)] {
            let origin = VizierSession("a".into(), channel, None);
            n.job_ended("j-ext", &origin, JobKind::Batch, JobState::Reported, None, Utc::now())
                .await;
        }
        assert_eq!(n.storage.notification_counts("u", &["a".into()]).await.unwrap().all, 0);
    }

    #[tokio::test]
    async fn a_job_launched_from_the_webui_keeps_its_session_as_the_source() {
        let (n, conn, _d) = setup();
        agent(&conn, "a");
        let origin = VizierSession(
            "a".into(),
            VizierChannelId::HTTP("ana".into(), "vizier-webui".into()),
            Some("topic-1".into()),
        );
        n.job_ended("j-web", &origin, JobKind::Delegation, JobState::Reported, None, Utc::now())
            .await;
        let items = n
            .storage
            .list_notifications("u", &["a".into()], NotificationFilter::All, None, 10)
            .await
            .unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].category, NotificationCategory::BackgroundJob);
        assert_eq!(items[0].source_session.as_ref(), Some(&origin));
    }

    fn user(conn: &SqlStore, id: &str, username: &str) {
        let user = crate::storage::user::User {
            user_id: id.into(),
            username: username.into(),
            password_hash: String::new(),
            role_id: "r".into(),
            created_at: Utc::now(),
        };
        conn.lock()
            .unwrap()
            .execute(
                r#"INSERT OR IGNORE INTO "user" (user_id, username, data) VALUES (?1, ?2, ?3)"#,
                params![id, username, serde_json::to_string(&user).unwrap()],
            )
            .unwrap();
    }

    #[tokio::test]
    async fn a_project_reply_links_to_its_project_route() {
        let (n, conn, _d) = setup();
        agent(&conn, "a");
        user(&conn, "u-ana", "ana");
        let session = VizierSession(
            "a".into(),
            VizierChannelId::Project("ana".into(), "p3xY9aQ2mZ".into()),
            Some("topic-1".into()),
        );
        n.chat_replied(&session, "h-project", Utc::now()).await;
        let mine = n
            .storage
            .list_notifications("u-ana", &["a".into()], NotificationFilter::All, None, 10)
            .await
            .unwrap();
        assert_eq!(mine[0].href, "/a/projects/p3xY9aQ2mZ/chat/topic-1");
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

        // Discord and Telegram create no conversation notification at all.
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
    async fn a_chat_row_carries_its_session_and_an_excerpt_from_the_saved_entry() {
        use crate::schema::{VizierResponse, VizierResponseContent};
        let (n, conn, _d) = setup();
        agent(&conn, "a");
        user(&conn, "u-ana", "ana");
        let session = VizierSession(
            "a".into(),
            VizierChannelId::HTTP("ana".into(), "vizier-webui".into()),
            Some("topic-1".into()),
        );
        // What is saved is already redacted; the excerpt can only show what was saved.
        let saved = VizierResponse {
            content: VizierResponseContent::Message {
                content: "The key is [redacted]. **Keep** it safe.".into(),
                stats: None,
            },
            ..Default::default()
        };
        let uid = n
            .storage
            .save_session_history(session.clone(), SessionHistoryContent::Response(saved))
            .await
            .unwrap();
        n.chat_replied(&session, &uid, Utc::now()).await;
        let rows = n
            .storage
            .list_notifications("u-ana", &["a".into()], NotificationFilter::All, None, 10)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].excerpt.as_deref(), Some("The key is [redacted]. Keep it safe."));
        assert_eq!(rows[0].source_session.as_ref(), Some(&session));
        assert_eq!(rows[0].text, "Replied in your conversation.", "the summary stays free of model output");
    }

    #[tokio::test]
    async fn a_task_row_keeps_the_title_it_had_when_the_run_ended() {
        let (n, conn, _d) = setup();
        agent(&conn, "a");
        n.task_run_ended("a", "brief", "Weekly brief", 3, TaskRunState::Answered, Utc::now()).await;
        let rows = n
            .storage
            .list_notifications("u", &["a".into()], NotificationFilter::All, None, 10)
            .await
            .unwrap();
        assert_eq!(rows[0].title.as_deref(), Some("Weekly brief"));
    }

    #[tokio::test]
    async fn rows_are_found_by_their_source_session_so_a_conversation_delete_can_remove_them() {
        let (n, conn, _d) = setup();
        agent(&conn, "a");
        conn.lock()
            .unwrap()
            .execute_batch("CREATE TABLE session_detail (agent_id TEXT, channel TEXT, topic TEXT, data TEXT);")
            .ok();
        let kept = VizierSession("a".into(), VizierChannelId::HTTP("ana".into(), "x".into()), Some("keep".into()));
        let gone = VizierSession("a".into(), VizierChannelId::HTTP("ana".into(), "x".into()), Some("gone".into()));
        for (key, session) in [("k", &kept), ("g", &gone)] {
            let mut d = draft(key.into(), "a", NotificationCategory::ChatReply, NotificationTone::Default, "r".into(), "/a/chat".into(), Utc::now());
            d.source_session = Some(session.clone());
            n.record(d).await;
        }
        let source = serde_json::to_string(&gone).unwrap();
        conn.lock()
            .unwrap()
            .execute("DELETE FROM notification WHERE source_session = ?1", params![source])
            .unwrap();
        let rows = n
            .storage
            .list_notifications("u", &["a".into()], NotificationFilter::All, None, 10)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].source_session.as_ref(), Some(&kept));
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
