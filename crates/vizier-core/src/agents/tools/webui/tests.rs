//! The WebUI tools: a message reaches an open tab *and* the inbox, a conversation can be created
//! before its first message, and both refuse what they cannot address.

use std::sync::Arc;
use std::time::Duration;

use super::*;

use crate::agents::tools::{ToolContext, VizierTool};
use crate::schema::{
    NotificationCategory, NotificationFilter, SessionHistoryContent, TOPIC_ID_LEN,
};
use crate::storage::{document::BlobDocumentStore, sqlite::SqlStorage, user::UserStorage};

struct Fixture {
    storage: Arc<VizierStorage>,
    transport: VizierTransport,
    notifications: Notifications,
    runtime: tokio::runtime::Runtime,
    _dir: tempfile::TempDir,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    SqlStorage::init_schema(&conn).unwrap();
    let conn = crate::storage::sql::SqlStore::sqlite(conn);
    // A notification for an agent that is gone is skipped, so the tool's agent has to exist.
    agent(&conn, "dp-a");
    let storage = Arc::new(VizierStorage::new(SqlStorage::new(
        conn,
        Arc::new(BlobDocumentStore::local(dir.path().to_path_buf())),
    )));
    let transport = VizierTransport::new();
    let notifications = Notifications::new(storage.clone());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    Fixture {
        storage,
        transport,
        notifications,
        runtime,
        _dir: dir,
    }
}

fn ctx() -> ToolContext {
    ToolContext {
        session: VizierSession("dp-a".into(), VizierChannelId::System, None),
        pending_attachments: Default::default(),
        hooks: None,
        background_depth: 0,
        subagent_depth: 0,
        consultation_chain: vec![],
        loaded_images: Default::default(),
        deadline: None,
        client: None,
        questions: crate::questions::QuestionsHandle::new(Default::default()),
        interactive: false,
        secret_redactor: Arc::new(crate::secrets::SecretRedactor::default()),
        dream_sources: vec![],
        project: None,
        detached: false,
        parent_detached_id: None,
        custom: Default::default(),
    }
}

fn sender(f: &Fixture) -> SendWebuiMessage {
    SendWebuiMessage {
        storage: f.storage.clone(),
        agent_id: "dp-a".into(),
        transport: f.transport.clone(),
        notifications: f.notifications.clone(),
    }
}

fn creator(f: &Fixture) -> CreateWebuiConversation {
    CreateWebuiConversation {
        storage: f.storage.clone(),
        agent_id: "dp-a".into(),
        transport: f.transport.clone(),
    }
}

/// One configured agent, as the row `Notifications::record` requires.
fn agent(conn: &crate::storage::sql::SqlStore, id: &str) {
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
            crate::storage::sql::params![id, serde_json::to_string(&config).unwrap()],
        )
        .unwrap();
}

fn user(f: &Fixture, username: &str) -> crate::storage::user::User {
    f.runtime
        .block_on(UserStorage::create_user(
            &*f.storage,
            username,
            "x",
            "role-1",
        ))
        .unwrap()
}

/// One message some open tab would receive, waiting at most `secs` for it.
async fn next_event(
    events: &mut tokio::sync::broadcast::Receiver<crate::transport::SessionEvent>,
) -> crate::transport::SessionEvent {
    tokio::time::timeout(Duration::from_secs(5), events.recv())
        .await
        .expect("the event arrives")
        .expect("an open channel")
}

#[test]
fn a_sent_message_is_stored_and_pushed_to_the_open_tab() {
    let f = fixture();
    let user = user(&f, "ana");
    f.runtime.block_on(async {
        let created = creator(&f)
            .call(
                CreateWebuiConversationArgs { username: "ana".into(), title: Some("Deploy".into()) },
                &ctx(),
            )
            .await
            .expect("the conversation is created");
        assert_eq!(created.title, "Deploy");
        assert_eq!(created.topic_id.chars().count(), TOPIC_ID_LEN);

        // A tab watching that conversation, subscribed the way the socket does it.
        let mut events = f.transport.subscribe_session_events();
        let session = VizierSession(
            "dp-a".into(),
            VizierChannelId::HTTP("ana".into(), WEBUI_CHANNEL.into()),
            Some(created.topic_id.clone()),
        );
        let mut inbox = f.transport.subscribe_inbox_events();

        let said = sender(&f)
            .call(
                SendWebuiMessageArgs {
                    username: "ana".into(),
                    topic_id: created.topic_id.clone(),
                    content: "the build is green".into(),
                },
                &ctx(),
            )
            .await
            .expect("the message is sent");
        assert!(said.contains("ana"), "{said}");

        // The tab gets the reply without a reload: the same frame a turn's reply arrives as.
        let event = next_event(&mut events).await;
        assert_eq!(event.session, session);
        let crate::transport::SessionFrame::HistoryEntry(entry) = event.frame else {
            panic!("a sent message is a history entry")
        };
        assert!(matches!(
            &entry.content,
            SessionHistoryContent::Response(response)
                if matches!(&response.content, crate::schema::VizierResponseContent::Message { content, .. } if content == "the build is green")
        ));

        // And the person is badged, with one row per message.
        let stale = tokio::time::timeout(Duration::from_secs(5), inbox.recv())
            .await
            .expect("the inbox event arrives")
            .expect("an open channel");
        assert!(matches!(stale, crate::transport::InboxEvent::Stale { user: Some(id), .. } if id == user.user_id));

        let rows = crate::storage::notification::NotificationStorage::list_notifications(
            &*f.storage,
            &user.user_id,
            &["dp-a".to_string()],
            NotificationFilter::All,
            None,
            20,
        )
        .await
        .unwrap();
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0].category, NotificationCategory::ChatReply);
        assert_eq!(rows[0].href, format!("/dp-a/chat/{}", created.topic_id));
        assert_eq!(rows[0].excerpt.as_deref(), Some("the build is green"));
    });
}

#[test]
fn a_message_to_an_unknown_topic_is_refused_and_nothing_is_stored() {
    let f = fixture();
    let _ = user(&f, "ana");
    f.runtime.block_on(async {
        let mut events = f.transport.subscribe_session_events();
        let err = sender(&f)
            .call(
                SendWebuiMessageArgs {
                    username: "ana".into(),
                    topic_id: "nope".into(),
                    content: "hello".into(),
                },
                &ctx(),
            )
            .await
            .expect_err("a topic that does not exist is refused");
        assert!(err.0.contains("unknown_topic"), "{}", err.0);
        assert!(err.0.contains("webui_create_conversation"), "{}", err.0);
        assert!(
            tokio::time::timeout(Duration::from_millis(200), events.recv())
                .await
                .is_err(),
            "a refused message publishes nothing"
        );

        let stored = crate::storage::history::HistoryStorage::list_session_history(
            &*f.storage,
            VizierSession(
                "dp-a".into(),
                VizierChannelId::HTTP("ana".into(), WEBUI_CHANNEL.into()),
                Some("nope".into()),
            ),
            None,
            None,
            None,
        )
        .await
        .unwrap();
        assert!(stored.is_empty(), "nothing was stored: {stored:?}");
    });
}

#[test]
fn a_conversation_created_for_nobody_is_refused() {
    let f = fixture();
    f.runtime.block_on(async {
        let err = creator(&f)
            .call(
                CreateWebuiConversationArgs {
                    username: "ghost".into(),
                    title: None,
                },
                &ctx(),
            )
            .await
            .expect_err("an unknown person is refused");
        assert!(err.0.contains("unknown_user"), "{}", err.0);
        let topics = ListWebuiTopics {
            storage: f.storage.clone(),
            agent_id: "dp-a".into(),
        }
        .call(
            ListWebuiTopicsArgs {
                username: "ghost".into(),
            },
            &ctx(),
        )
        .await
        .unwrap();
        assert!(topics.is_empty(), "no conversation was made: {topics:?}");
    });
}

#[test]
fn the_title_is_trimmed_capped_and_never_empty() {
    assert_eq!(conversation_title(None), NEW_SESSION_TITLE);
    assert_eq!(conversation_title(Some("   ")), NEW_SESSION_TITLE);
    assert_eq!(conversation_title(Some("  Deploy  ")), "Deploy");
    let capped = conversation_title(Some(&"t".repeat(MAX_TITLE_CHARS + 40)));
    assert_eq!(capped.chars().count(), MAX_TITLE_CHARS);
}

#[test]
fn a_created_conversation_is_listed_before_it_has_a_message() {
    let f = fixture();
    let _ = user(&f, "ana");
    f.runtime.block_on(async {
        let mut inbox = f.transport.subscribe_inbox_events();
        let created = creator(&f)
            .call(
                CreateWebuiConversationArgs {
                    username: "ana".into(),
                    title: None,
                },
                &ctx(),
            )
            .await
            .unwrap();
        let stale = tokio::time::timeout(Duration::from_secs(5), inbox.recv())
            .await
            .expect("the list is resynced")
            .expect("an open channel");
        assert!(matches!(stale, crate::transport::InboxEvent::Stale { .. }));

        let topics = ListWebuiTopics {
            storage: f.storage.clone(),
            agent_id: "dp-a".into(),
        }
        .call(
            ListWebuiTopicsArgs {
                username: "ana".into(),
            },
            &ctx(),
        )
        .await
        .unwrap();
        assert_eq!(topics.len(), 1);
        assert_eq!(topics[0].topic_id, created.topic_id);
        assert_eq!(topics[0].title, NEW_SESSION_TITLE);
        assert!(!topics[0].is_thinking);
    });
}
