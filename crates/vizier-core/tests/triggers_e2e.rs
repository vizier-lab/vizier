//! Triggers end to end (`specs/071-triggers/quickstart.md`): one dummyplug agent, a webhook trigger
//! whose events reach the agent through the real runtime, and the result delivered to the person
//! who set it up and nobody else.
//!
//! Like `dummyplug_e2e.rs`, the test starts the Node dummyplug server and is skipped locally when
//! Node or the submodule is missing.

use std::{
    net::TcpListener,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::Duration,
};

use vizier_core::{
    config::{VizierConfig, provider::ProviderId},
    runtime::{RuntimeOptions, VizierRuntime},
    schema::{
        AgentCommand, AgentCommandResult, AgentConfig, AgentToolsConfig, NotificationCategory,
        OverlapPolicy, Requester, SessionHistoryContent, Trigger, TriggerRunState, VizierChannelId,
        VizierRequest, VizierRequestContent, VizierResponseContent, VizierSession,
        notification::NotificationFilter,
        provider::{ProviderCredentials, ProviderEntry},
    },
    storage::{
        history::HistoryStorage, notification::NotificationStorage, provider::ProviderStorage,
        trigger::TriggerStorage, user::UserStorage,
    },
    triggers::{self, Intake, PushRequest, TriggerKind, webhook::WebhookKind},
};

const TIMEOUT: Duration = Duration::from_secs(60);

struct Dummyplug {
    child: Child,
    port: u16,
}

impl Dummyplug {
    fn start() -> Option<Self> {
        let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        Self::start_on(port)
    }

    fn start_on(port: u16) -> Option<Self> {
        let server = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dummyplug/src/server.mjs");
        let missing = if !server.exists() {
            Some(format!("{} not found", server.display()))
        } else if Command::new("node").arg("--version").output().is_err() {
            Some("node not found".to_string())
        } else {
            None
        };
        if let Some(why) = missing {
            assert!(std::env::var_os("CI").is_none(), "dummyplug harness can't run: {why}");
            eprintln!("skipping the trigger harness: {why}");
            return None;
        }
        let child = Command::new("node")
            .arg(&server)
            .env("DUMMYPLUG_HOST", "127.0.0.1")
            .env("DUMMYPLUG_PORT", port.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("start the dummyplug server");
        Some(Self { child, port })
    }

    async fn ready(&self) {
        let url = format!("http://127.0.0.1:{}/healthz", self.port);
        for _ in 0..100 {
            if reqwest::get(&url).await.is_ok_and(|r| r.status().is_success()) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        panic!("dummyplug did not become healthy at {url}");
    }
}

impl Drop for Dummyplug {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn agent_config(owner: &str) -> AgentConfig {
    AgentConfig {
        name: "Plug".into(),
        owner_id: Some(owner.into()),
        shared_to: vec![],
        system_prompt: None,
        description: None,
        provider: ProviderId::DUMMYPLUG,
        model: "dummyplug".into(),
        thinking_depth: 1,
        checkpoint_threshold: 0.8,
        tools: AgentToolsConfig { timeout: Duration::from_secs(30 * 60).into(), ..Default::default() },
        silent_read_initiative_chance: 0.0,
        max_tokens: None,
        context_window: None,
        include_documents: None,
        prompt_timeout: Duration::from_secs(60 * 60).into(),
        documents: vec![],
        heartbeat_interval: Duration::from_secs(30 * 60).into(),
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
    }
}

/// A runtime with one dummyplug agent owned by `owner`, and a second person who can see it.
async fn harness() -> Option<(Dummyplug, VizierRuntime, tempfile::TempDir, String, String)> {
    let dummyplug = Dummyplug::start()?;
    dummyplug.ready().await;
    let (runtime, dir, owner, other) = start_on(dummyplug.port).await;
    Some((dummyplug, runtime, dir, owner, other))
}

/// A fresh runtime whose dummyplug provider is expected on `port`.
async fn start_on(port: u16) -> (VizierRuntime, tempfile::TempDir, String, String) {
    let dir = tempfile::tempdir().unwrap();
    let runtime = VizierRuntime::start(RuntimeOptions {
        config: VizierConfig::with_workspace(dir.path().to_path_buf()),
        ..Default::default()
    })
    .await
    .unwrap();
    let deps = runtime.deps();
    deps.storage
        .upsert_provider(&ProviderEntry {
            id: ProviderId::DUMMYPLUG,
            credentials: ProviderCredentials {
                base_url: Some(format!("http://127.0.0.1:{port}/v1")),
                ..Default::default()
            },
        })
        .await
        .unwrap();
    let role = deps.storage.create_role("viewers", vec!["all_agents:view".into()], false).await.unwrap();
    let owner = deps.storage.create_user("owner", "x", &role.role_id).await.unwrap();
    let other = deps.storage.create_user("other", "x", &role.role_id).await.unwrap();
    let (resp, created) = tokio::sync::oneshot::channel();
    runtime
        .transport()
        .send_agent_command(AgentCommand::Create { agent_id: "plug".into(), config: agent_config(&owner.user_id), resp })
        .await
        .unwrap();
    match tokio::time::timeout(TIMEOUT, created).await.unwrap().unwrap() {
        AgentCommandResult::Ok(_) => {}
        AgentCommandResult::Error(e) => panic!("create failed: {e}"),
    }
    (runtime, dir, owner.user_id, other.user_id)
}

fn webhook(owner: &str, token: &str) -> Trigger {
    Trigger {
        id: "trg_e2e".into(),
        agent_id: "plug".into(),
        slug: "ci".into(),
        kind: "webhook".into(),
        title: "CI".into(),
        instruction: "Tell me when CI fails on master".into(),
        settings: serde_json::json!({ "token": token, "filter": ["body.ref=refs/heads/master"] }),
        requester: Requester::User("owner".into()),
        delivers_to: Some(owner.into()),
        is_active: true,
        awaiting_approval: false,
        needs_attention: None,
        tool_scope: Some(vec!["memory_read".into()]),
        overlap: OverlapPolicy::Wait,
        hourly_limit: Some(20),
        cooldown_secs: 0,
        triage: None,
        destination: None,
        summary_hours: None,
        last_summary_at: None,
        created_at: vizier_core::time::now(),
        last_fired_at: None,
    }
}

fn push(token: &str, body: &str) -> PushRequest {
    PushRequest {
        headers: vec![
            ("authorization".into(), format!("Bearer {token}")),
            ("content-type".into(), "application/json".into()),
        ],
        body: body.as_bytes().to_vec(),
        path_secret: None,
        received_at: vizier_core::time::now(),
    }
}

async fn eventually<F, Fut>(secs: u64, mut check: F)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let deadline = std::time::Instant::now() + Duration::from_secs(secs);
    while std::time::Instant::now() < deadline {
        if check().await {
            return;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    panic!("the condition did not hold within {secs}s");
}

/// Quickstart steps 1-3: an authenticated event starts one run in its own conversation, framed as
/// untrusted, and its reply reaches the person who set the trigger up and nobody else. A request
/// for another branch is ignored by the filter and starts nothing.
#[tokio::test(flavor = "multi_thread")]
async fn a_webhook_event_starts_one_run_and_the_reply_reaches_only_its_setter() {
    let Some((_dummyplug, runtime, _dir, owner, other)) = harness().await else { return };
    let deps = runtime.deps();
    let trigger = webhook(&owner, "tok");
    deps.storage.create_trigger(&trigger).await.unwrap();

    let ignored = WebhookKind.receive(&trigger, &push("tok", r#"{"ref":"refs/heads/feature"}"#)).unwrap();
    assert!(matches!(ignored, Intake::Ignore { .. }));
    triggers::accept(deps, &trigger, ignored, None).await.unwrap();

    // A value the agent keeps as a secret, leaked into the payload by the sender.
    use vizier_core::storage::secret::SecretStorage;
    deps.storage
        .set_agent_secret("plug", "github_token", "ghp_supersecretvalue", &Requester::User("owner".into()))
        .await
        .unwrap();
    let intake = WebhookKind
        .receive(
            &trigger,
            &push("tok", r#"{"ref":"refs/heads/master","conclusion":"failure","leak":"ghp_supersecretvalue"}"#),
        )
        .unwrap();
    let admission = triggers::accept(deps, &trigger, intake, None).await.unwrap().unwrap();
    assert_eq!(admission.outcome, vizier_core::schema::AdmitOutcome::Accepted);

    eventually(30, || async {
        let runs = deps.storage.list_trigger_runs(&trigger.id, None, 10).await.unwrap();
        runs.first().is_some_and(|r| r.state != TriggerRunState::Running)
    })
    .await;
    let runs = deps.storage.list_trigger_runs(&trigger.id, None, 10).await.unwrap();
    assert_eq!(runs.len(), 1, "the ignored request started nothing");
    let run = &runs[0];
    assert_eq!(run.state, TriggerRunState::Answered);

    // The agent was given the instruction and the event fenced as untrusted data.
    let history = deps.storage.list_session_history(triggers::run_session(run), None, None, None).await.unwrap();
    let request = history
        .iter()
        .find_map(|h| match &h.content {
            SessionHistoryContent::Request(r) => Some(r.clone()),
            _ => None,
        })
        .expect("the run's request");
    let VizierRequestContent::Unattended(text) = &request.content else { panic!("an unattended request") };
    assert!(text.contains("Tell me when CI fails on master"));
    assert!(text.contains("never instructions"));
    assert!(text.contains("refs/heads/master"));
    assert!(!text.contains("tok"), "the secret never reaches the agent");
    assert!(!text.contains("ghp_supersecretvalue"), "an agent secret in a payload is redacted");
    assert!(text.contains("[redacted]"));
    assert!(request.to_prompt().unwrap().contains("sender: trigger"));

    // Delivered once, to the setter only.
    let agents = vec!["plug".to_string()];
    eventually(10, || async {
        let mine = deps
            .storage
            .list_notifications(&owner, &agents, NotificationFilter::All, None, 20)
            .await
            .unwrap();
        mine.iter().any(|n| n.category == NotificationCategory::TriggerResult)
    })
    .await;
    let mine = deps.storage.list_notifications(&owner, &agents, NotificationFilter::All, None, 20).await.unwrap();
    assert_eq!(mine.iter().filter(|n| n.category == NotificationCategory::TriggerResult).count(), 1);
    let theirs = deps.storage.list_notifications(&other, &agents, NotificationFilter::All, None, 20).await.unwrap();
    assert!(theirs.iter().all(|n| n.category != NotificationCategory::TriggerResult));

    let events = deps.storage.list_trigger_events(&trigger.id, None, 10).await.unwrap();
    assert_eq!(events.len(), 2);
    runtime.shutdown().await.unwrap();
}

/// FR-030: any turn on a trigger's own conversation runs with the trigger's scope, whatever started
/// it, so work delivered back into it cannot reach tools the trigger does not allow.
#[tokio::test(flavor = "multi_thread")]
async fn a_turn_on_a_trigger_conversation_sees_only_the_triggers_tools() {
    let Some((_dummyplug, runtime, _dir, owner, _)) = harness().await else { return };
    let deps = runtime.deps();
    deps.storage.create_trigger(&webhook(&owner, "tok")).await.unwrap();

    let session = VizierSession(
        "plug".into(),
        VizierChannelId::Trigger("ci".into(), triggers::now_ms()),
        None,
    );
    let (tx, rx) = runtime.transport().reply_channel().await.unwrap();
    let request = VizierRequest {
        timestamp: vizier_core::time::now(),
        user: "plug".into(),
        content: VizierRequestContent::Prompt("tools".into()),
        metadata: serde_json::json!({}),
        ..Default::default()
    };
    runtime.transport().send_request(session, request, Some(tx)).await.unwrap();
    let reply = tokio::time::timeout(TIMEOUT, async {
        while let Ok(response) = rx.recv_async().await {
            if let VizierResponseContent::Message { content, .. } = response.content {
                return content;
            }
        }
        panic!("no reply");
    })
    .await
    .unwrap();
    assert!(reply.contains("execute_python"), "{reply}");
    // What a script may call: the same agent, scoped on the trigger's conversation and not on an
    // ordinary one. This test agent has no embedding model, so it has no memory tools at all.
    let scoped = describe(&runtime).await;
    let open = describe_in(
        &runtime,
        VizierSession("plug".into(), VizierChannelId::HTTP("owner".into(), "x".into()), Some("t".into())),
    )
    .await;
    assert!(open.contains("schedule_cron_task") && open.contains("set_secret"), "{open}");
    assert!(scoped.contains("nothing_to_report"), "{scoped}");
    for outside in ["schedule_cron_task", "set_secret", "spawn_subagents", "wake_session", "webui_send_message"] {
        assert!(!scoped.contains(outside), "{outside} is outside the trigger's scope: {scoped}");
    }
    runtime.shutdown().await.unwrap();
}

/// FR-030, T010: a conversation a scoped turn wakes keeps its scope.
#[tokio::test(flavor = "multi_thread")]
async fn a_conversation_woken_by_a_trigger_run_keeps_the_triggers_scope() {
    let Some((_dummyplug, runtime, _dir, owner, _)) = harness().await else { return };
    let deps = runtime.deps();
    let mut trigger = webhook(&owner, "tok");
    trigger.tool_scope = Some(vec!["memory_read".into(), "sessions".into()]);
    deps.storage.create_trigger(&trigger).await.unwrap();

    let listing = serde_json::json!({ "tool": "list_tool_functions", "arguments": {} }).to_string();
    let wake = format!(
        "wake_session(target=\"http__owner__x\", topic_id=\"t\", instruction={}, wait=True)",
        serde_json::to_string(&listing).unwrap()
    );
    let call = serde_json::json!({ "tool": "execute_python", "arguments": { "intent": "wake", "code": wake } });
    let session = VizierSession("plug".into(), VizierChannelId::Trigger("ci".into(), triggers::now_ms()), None);
    chat(&runtime, &session, &call.to_string()).await;

    let woken = VizierSession("plug".into(), VizierChannelId::HTTP("owner".into(), "x".into()), Some("t".into()));
    let reply = {
        let mut found = None;
        for _ in 0..150 {
            let history = deps.storage.list_session_history(woken.clone(), None, None, None).await.unwrap();
            found = history.iter().find_map(|h| match &h.content {
                SessionHistoryContent::Response(r) => match &r.content {
                    VizierResponseContent::Message { content, .. } => Some(content.clone()),
                    _ => None,
                },
                _ => None,
            });
            if found.is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        found.expect("the woken turn replied")
    };
    assert!(reply.contains("wake_session"), "{reply}");
    for outside in ["schedule_cron_task", "set_secret", "spawn_subagents"] {
        assert!(!reply.contains(outside), "{outside} is outside the scope the wake carried: {reply}");
    }
    runtime.shutdown().await.unwrap();
}

/// The scripted catalogue a trigger-conversation turn sees.
async fn describe(runtime: &VizierRuntime) -> String {
    let session = VizierSession(
        "plug".into(),
        VizierChannelId::Trigger("ci".into(), triggers::now_ms()),
        None,
    );
    describe_in(runtime, session).await
}

async fn describe_in(runtime: &VizierRuntime, session: VizierSession) -> String {
    let (tx, rx) = runtime.transport().reply_channel().await.unwrap();
    let call = serde_json::json!({ "tool": "list_tool_functions", "arguments": {} }).to_string();
    let request = VizierRequest {
        timestamp: vizier_core::time::now(),
        user: "plug".into(),
        content: VizierRequestContent::Prompt(call),
        metadata: serde_json::json!({}),
        ..Default::default()
    };
    runtime.transport().send_request(session, request, Some(tx)).await.unwrap();
    tokio::time::timeout(TIMEOUT, async {
        while let Ok(response) = rx.recv_async().await {
            if let VizierResponseContent::Message { content, .. } = response.content {
                return content;
            }
        }
        panic!("no reply");
    })
    .await
    .unwrap()
}

/// Quickstart steps 10-12: a monitor's first check saves a baseline without firing, a later one
/// that finds something fires exactly one run, a failing script records its error and fires
/// nothing, and none of it calls a model.
#[tokio::test(flavor = "multi_thread")]
async fn a_monitor_saves_a_baseline_then_fires_once_and_a_failure_fires_nothing() {
    use vizier_core::triggers::monitor;
    let Some((_dummyplug, runtime, _dir, owner, _)) = harness().await else { return };
    let deps = runtime.deps();
    let mut trigger = webhook(&owner, "unused");
    trigger.id = "trg_mon".into();
    trigger.slug = "watch".into();
    trigger.kind = "monitor".into();
    trigger.title = "Watch".into();
    trigger.settings = serde_json::json!({
        "script": "s = input[\"state\"]\nn = 1 if s is None else s + 1\n{\"state\": n, \"fire\": {\"n\": n}} if n >= 2 else {\"state\": n}",
        "interval_secs": 60,
    });
    deps.storage.create_trigger(&trigger).await.unwrap();

    let first = monitor::check_once(deps, &trigger).await;
    assert_eq!(first.error, None, "{first:?}");
    assert_eq!(deps.storage.monitor_record(&trigger.id).await.unwrap().state, Some(serde_json::json!(1)));
    assert!(deps.storage.list_trigger_events(&trigger.id, None, 10).await.unwrap().is_empty(), "a baseline fires nothing");

    let second = monitor::check_once(deps, &trigger).await;
    assert_eq!(second.error, None, "{second:?}");
    eventually(30, || async {
        let runs = deps.storage.list_trigger_runs(&trigger.id, None, 10).await.unwrap();
        runs.first().is_some_and(|r| r.state != TriggerRunState::Running)
    })
    .await;
    let runs = deps.storage.list_trigger_runs(&trigger.id, None, 10).await.unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(deps.storage.monitor_record(&trigger.id).await.unwrap().state, Some(serde_json::json!(2)));

    let mut broken = trigger.clone();
    broken.settings["script"] = serde_json::json!("1 / 0");
    let failed = monitor::check_once(deps, &broken).await;
    assert!(failed.error.is_some());
    let record = deps.storage.monitor_record(&trigger.id).await.unwrap();
    assert_eq!(record.failures, 1);
    assert!(record.last_error.is_some());
    assert_eq!(record.state, Some(serde_json::json!(2)), "a failed check keeps the state");
    assert_eq!(deps.storage.list_trigger_runs(&trigger.id, None, 10).await.unwrap().len(), 1);
    runtime.shutdown().await.unwrap();
}

/// The text of a turn's final message.
async fn chat(runtime: &VizierRuntime, session: &VizierSession, text: &str) -> String {
    let (tx, rx) = runtime.transport().reply_channel().await.unwrap();
    let request = VizierRequest {
        timestamp: vizier_core::time::now(),
        user: "owner".into(),
        content: VizierRequestContent::Chat(text.into()),
        metadata: serde_json::json!({}),
        ..Default::default()
    };
    runtime.transport().send_request(session.clone(), request, Some(tx)).await.unwrap();
    tokio::time::timeout(TIMEOUT, async {
        while let Ok(response) = rx.recv_async().await {
            if let VizierResponseContent::Message { content, .. } = response.content {
                return content;
            }
        }
        panic!("no reply");
    })
    .await
    .unwrap()
}

/// Quickstart step 13 and FR-041: from a person's chat, the agent may create a trigger only as far
/// as that person may; what it creates waits for approval, and the owner is told.
#[tokio::test(flavor = "multi_thread")]
async fn an_agent_creates_a_trigger_only_as_far_as_its_person_may_and_it_waits_for_approval() {
    let Some((_dummyplug, runtime, _dir, owner, _)) = harness().await else { return };
    let deps = runtime.deps();
    let session = VizierSession("plug".into(), VizierChannelId::HTTP("owner".into(), "chat".into()), Some("t".into()));
    let create = serde_json::json!({ "tool": "execute_python", "arguments": {
        "intent": "create a monitor",
        "code": "create_trigger(slug=\"watch\", kind=\"monitor\", title=\"Watch\", instruction=\"Tell me when it changes\", settings={\"script\": \"None\", \"tools\": [\"shell\", \"secrets\", \"memory\"]}, on_own_initiative=False)",
    }})
    .to_string();

    // `owner` can see the agent but holds no trigger permission yet.
    let refused = chat(&runtime, &session, &create).await;
    assert!(refused.contains("forbidden"), "{refused}");
    assert!(deps.storage.get_trigger("plug", "watch").await.unwrap().is_none());

    let user = deps.storage.get_user("owner").await.unwrap().unwrap();
    let role = deps.storage.get_role(&user.role_id).await.unwrap().unwrap();
    let mut permissions = role.permissions.clone();
    permissions.push("owned_agents:manage_triggers".into());
    deps.storage.update_role(&role.role_id, &role.name, permissions).await.unwrap();

    let created = chat(&runtime, &session, &create).await;
    assert!(created.contains("\"status\": \"ok\"") || created.contains("\"status\":\"ok\""), "{created}");
    let trigger = deps.storage.get_trigger("plug", "watch").await.unwrap().expect("the trigger");
    assert!(trigger.awaiting_approval && !trigger.is_active);
    assert_eq!(trigger.tool_scope, Some(vec!["memory_read".to_string()]));
    assert_eq!(trigger.delivers_to.as_deref(), Some(owner.as_str()));
    assert_eq!(trigger.settings["tools"], serde_json::json!(["memory_read"]), "the agent grants itself nothing");

    // Nor can it widen them later, and its unapproved script is not run (FR-031).
    let run = |code: &str| {
        serde_json::json!({ "tool": "execute_python", "arguments": { "intent": "x", "code": code } }).to_string()
    };
    chat(
        &runtime,
        &session,
        &run("update_trigger(slug=\"watch\", settings={\"script\": \"None\", \"tools\": [\"shell\"]})"),
    )
    .await;
    let after = deps.storage.get_trigger("plug", "watch").await.unwrap().unwrap();
    assert_eq!(after.settings["tools"], serde_json::json!(["memory_read"]));
    let tested = chat(&runtime, &session, &run("test_trigger(slug=\"watch\")")).await;
    assert!(tested.contains("awaiting_approval"), "{tested}");

    let agents = vec!["plug".to_string()];
    let notes = deps.storage.list_notifications(&owner, &agents, NotificationFilter::All, None, 20).await.unwrap();
    assert!(notes.iter().any(|n| n.category == NotificationCategory::TriggerNotice), "the owner is told");
    runtime.shutdown().await.unwrap();
}

/// FR-041 and spec 069: in a Telegram direct message the person is the one who linked that account.
/// An id typed into a profile names nobody.
#[tokio::test(flavor = "multi_thread")]
async fn a_telegram_message_acts_for_the_linked_person_never_for_a_typed_id() {
    use vizier_core::{
        platform_links::PlatformPerson, schema::ChannelPlatform,
        storage::platform_link::PlatformLinkStorage,
    };
    let Some((_dummyplug, runtime, _dir, owner, _)) = harness().await else { return };
    let deps = runtime.deps();
    let user = deps.storage.get_user("owner").await.unwrap().unwrap();
    let role = deps.storage.get_role(&user.role_id).await.unwrap().unwrap();
    let mut permissions = role.permissions.clone();
    permissions.push("owned_agents:manage_triggers".into());
    deps.storage.update_role(&role.role_id, &role.name, permissions).await.unwrap();
    let profile = vizier_core::storage::user::UserProfile {
        user_id: owner.clone(),
        discord_id: None,
        discord_username: None,
        telegram_id: Some("4242".into()),
        telegram_username: None,
        alias: vec![],
    };
    deps.storage.upsert_user_profile(&owner, &profile).await.unwrap();

    let session = VizierSession("plug".into(), VizierChannelId::TelegramChannel(4242), None);
    let create = serde_json::json!({ "tool": "execute_python", "arguments": {
        "intent": "create a monitor",
        "code": "create_trigger(slug=\"watch\", kind=\"monitor\", title=\"Watch\", instruction=\"Tell me\", settings={\"script\": \"None\"}, on_own_initiative=False)",
    }})
    .to_string();
    let refused = chat(&runtime, &session, &create).await;
    assert!(refused.contains("no_person"), "{refused}");
    assert!(deps.storage.get_trigger("plug", "watch").await.unwrap().is_none());

    let account = PlatformPerson {
        platform: ChannelPlatform::Telegram,
        id: "4242".into(),
        username: None,
        display_name: None,
    };
    let now = vizier_core::time::now().timestamp_millis();
    deps.storage.issue_connect_code(&account, "hash", now, now + 60_000).await.unwrap();
    deps.storage.redeem_connect_code("hash", &owner, now).await.unwrap();
    chat(&runtime, &session, &create).await;
    let trigger = deps.storage.get_trigger("plug", "watch").await.unwrap().expect("the trigger");
    assert_eq!(trigger.delivers_to.as_deref(), Some(owner.as_str()));
    runtime.shutdown().await.unwrap();
}

/// FR-046: with a destination, the reply also lands in the setter's own WebUI conversation.
#[tokio::test(flavor = "multi_thread")]
async fn a_result_also_lands_in_the_conversation_the_trigger_names() {
    use vizier_core::{schema::VizierSessionDetail, storage::session::SessionStorage};
    let Some((_dummyplug, runtime, _dir, owner, _)) = harness().await else { return };
    let deps = runtime.deps();
    let channel = VizierChannelId::HTTP("owner".into(), vizier_core::agents::tools::webui_channel().into());
    deps.storage
        .save_session_detail(VizierSessionDetail {
            agent_id: "plug".into(),
            channel: channel.clone(),
            topic: Some("inbox1".into()),
            title: "Alerts".into(),
            is_thinking: false,
        })
        .await
        .unwrap();
    let mut trigger = webhook(&owner, "tok");
    trigger.destination = Some("webui:inbox1".into());
    deps.storage.create_trigger(&trigger).await.unwrap();
    let intake = WebhookKind.receive(&trigger, &push("tok", r#"{"ref":"refs/heads/master"}"#)).unwrap();
    triggers::accept(deps, &trigger, intake, None).await.unwrap();

    let conversation = VizierSession("plug".into(), channel, Some("inbox1".into()));
    eventually(30, || async {
        deps.storage
            .list_session_history(conversation.clone(), None, None, None)
            .await
            .unwrap()
            .iter()
            .any(|h| matches!(&h.content, SessionHistoryContent::Response(r)
                if matches!(&r.content, VizierResponseContent::Message { content, .. } if content.contains("Trigger \u{201c}CI\u{201d}"))))
    })
    .await;
    runtime.shutdown().await.unwrap();
}

/// A run whose turn was still pending when vizier stopped is resumed after the restart and closed,
/// so it never stays `running` and blocks its trigger (research R3).
#[test]
fn a_run_resumed_after_a_restart_is_closed_and_the_next_event_runs() {
    use vizier_core::storage::turn::TurnStorage;
    let first = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    // A provider that accepts connections and never answers keeps the first run's turn pending.
    let silent = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = silent.local_addr().unwrap().port();
    let Some((engine, dir)) = first.block_on(async {
        // Node and the submodule are needed for the second start.
        drop(Dummyplug::start()?);
        let (runtime, dir, owner, _) = start_on(port).await;
        let deps = runtime.deps();
        let trigger = webhook(&owner, "tok");
        deps.storage.create_trigger(&trigger).await.unwrap();
        let intake = WebhookKind.receive(&trigger, &push("tok", r#"{"ref":"refs/heads/master"}"#)).unwrap();
        triggers::accept(deps, &trigger, intake, None).await.unwrap();
        eventually(30, || async {
            let runs = deps.storage.list_trigger_runs(&trigger.id, None, 10).await.unwrap();
            match runs.first() {
                Some(run) => deps
                    .storage
                    .sessions_with_pending_turns()
                    .await
                    .unwrap()
                    .contains(&triggers::run_session(run)),
                None => false,
            }
        })
        .await;
        // Let the turn be claimed by the agent, so the restart resumes it.
        tokio::time::sleep(Duration::from_millis(500)).await;
        Some((runtime, dir))
    }) else {
        return;
    };
    // The process "dies": every task goes at once, with no chance to close the run. Only then is
    // the engine dropped, which frees the workspace.
    first.shutdown_timeout(Duration::from_secs(5));
    drop(engine);
    drop(silent);

    let second = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    second.block_on(async {
        let dummyplug = Dummyplug::start_on(port).unwrap();
        dummyplug.ready().await;
        let runtime = VizierRuntime::start(RuntimeOptions {
            config: VizierConfig::with_workspace(dir.path().to_path_buf()),
            ..Default::default()
        })
        .await
        .unwrap();
        let deps = runtime.deps();
        let trigger = deps.storage.get_trigger("plug", "ci").await.unwrap().unwrap();
        eventually(45, || async {
            let runs = deps.storage.list_trigger_runs(&trigger.id, None, 10).await.unwrap();
            runs.iter().all(|r| r.state != TriggerRunState::Running)
        })
        .await;
        let runs = deps.storage.list_trigger_runs(&trigger.id, None, 10).await.unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].state, TriggerRunState::Answered, "the resumed turn's reply closed it");

        let intake = WebhookKind.receive(&trigger, &push("tok", r#"{"ref":"refs/heads/master","n":2}"#)).unwrap();
        triggers::accept(deps, &trigger, intake, None).await.unwrap();
        eventually(45, || async {
            let runs = deps.storage.list_trigger_runs(&trigger.id, None, 10).await.unwrap();
            runs.len() == 2 && runs.iter().all(|r| r.state == TriggerRunState::Answered)
        })
        .await;
        runtime.shutdown().await.unwrap();
    });
}

/// A stand-in triage model speaking the OpenAI chat protocol: an event whose payload names
/// `master` is important, one batch naming `garble` gets an answer that cannot be read.
async fn fake_triage_model() -> u16 {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut data = vec![];
                let mut buffer = [0u8; 4096];
                let body = loop {
                    let Ok(n) = socket.read(&mut buffer).await else { return };
                    if n == 0 {
                        return;
                    }
                    data.extend_from_slice(&buffer[..n]);
                    let text = String::from_utf8_lossy(&data);
                    let Some((head, body)) = text.split_once("\r\n\r\n") else { continue };
                    let length = head
                        .lines()
                        .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap()))
                        .unwrap_or(0);
                    if body.len() >= length {
                        break body.to_string();
                    }
                };
                let asked: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
                assert!(asked["tools"].as_array().is_none_or(Vec::is_empty), "triage is offered no tool: {asked}");
                let prompt = asked["messages"]
                    .as_array()
                    .and_then(|m| m.last())
                    .map(|m| m["content"].to_string())
                    .unwrap_or_default();
                let answer = if prompt.contains("garble") {
                    "I would rather not say.".to_string()
                } else {
                    let verdicts: Vec<serde_json::Value> = prompt
                        .split("Event id ")
                        .skip(1)
                        .filter_map(|part| {
                            let id: i64 = part.split(':').next()?.trim().parse().ok()?;
                            let important = part.contains("master");
                            Some(serde_json::json!({ "id": id, "important": important, "reason": if important { "on master" } else { "a side branch" } }))
                        })
                        .collect();
                    serde_json::to_string(&verdicts).unwrap()
                };
                let reply = serde_json::json!({
                    "id": "x", "object": "chat.completion", "created": 0, "model": "tri",
                    "choices": [{ "index": 0, "message": { "role": "assistant", "content": answer }, "finish_reason": "stop" }],
                    "usage": { "prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2 }
                })
                .to_string();
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
                    reply.len()
                );
                let _ = socket.write_all(response.as_bytes()).await;
            });
        }
    });
    port
}

async fn send_event(runtime: &VizierRuntime, id: &str, body: serde_json::Value) {
    let deps = runtime.deps();
    let trigger = deps.storage.get_trigger_by_id(id).await.unwrap().unwrap();
    let intake = WebhookKind.receive(&trigger, &push("tok", &body.to_string())).unwrap();
    triggers::accept(deps, &trigger, intake, None).await.unwrap();
}

/// Nothing waits for triage and no run is open.
async fn settled(runtime: &VizierRuntime, id: &str) {
    let storage = &runtime.deps().storage;
    eventually(45, || async {
        storage.pending_triage(id, 1).await.unwrap().is_empty()
            && storage.open_trigger_runs().await.unwrap().is_empty()
    })
    .await;
}

/// T048, FR-042 to FR-045 and SC-005: triage splits mixed events, only important ones reach a run
/// and one delivery, an unreadable verdict fails open, a storm stays within the run limit, and a
/// removed model marks the trigger once.
#[tokio::test(flavor = "multi_thread")]
async fn triage_lets_only_important_events_through_within_the_limit() {
    use vizier_core::schema::{EventStatus, TriageConfig};
    let Some((_dummyplug, runtime, _dir, owner, _)) = harness().await else { return };
    let deps = runtime.deps();
    let port = fake_triage_model().await;
    deps.storage
        .upsert_provider(&ProviderEntry {
            id: ProviderId::LLAMA_CPP,
            credentials: ProviderCredentials {
                base_url: Some(format!("http://127.0.0.1:{port}/v1")),
                ..Default::default()
            },
        })
        .await
        .unwrap();
    let mut trigger = webhook(&owner, "tok");
    trigger.settings = serde_json::json!({ "token": "tok" });
    trigger.hourly_limit = Some(3);
    trigger.triage = Some(TriageConfig {
        criteria: "only master".into(),
        provider: Some(ProviderId::LLAMA_CPP.to_string()),
        model: Some("tri".into()),
        calls_per_hour: 200,
    });
    deps.storage.create_trigger(&trigger).await.unwrap();
    let agents = vec!["plug".to_string()];
    let results = || async {
        deps.storage
            .list_notifications(&owner, &agents, NotificationFilter::All, None, 50)
            .await
            .unwrap()
            .into_iter()
            .filter(|n| n.category == NotificationCategory::TriggerResult)
            .count()
    };

    // Mixed events: the side branches never start a run.
    send_event(&runtime, &trigger.id, serde_json::json!({ "ref": "feature-a" })).await;
    send_event(&runtime, &trigger.id, serde_json::json!({ "ref": "feature-b" })).await;
    settled(&runtime, &trigger.id).await;
    assert!(deps.storage.list_trigger_runs(&trigger.id, None, 10).await.unwrap().is_empty());
    send_event(&runtime, &trigger.id, serde_json::json!({ "ref": "master" })).await;
    settled(&runtime, &trigger.id).await;
    eventually(10, || async { results().await == 1 }).await;
    let events = deps.storage.list_trigger_events(&trigger.id, None, 10).await.unwrap();
    let count = |status| events.iter().filter(|e| e.status == status).count();
    assert_eq!((count(EventStatus::Unimportant), count(EventStatus::Handled)), (2, 1), "{events:?}");
    assert!(events.iter().any(|e| e.reason.as_deref() == Some("a side branch")));

    // An answer that cannot be read counts as important, marked as not triaged.
    send_event(&runtime, &trigger.id, serde_json::json!({ "ref": "garble" })).await;
    settled(&runtime, &trigger.id).await;
    let events = deps.storage.list_trigger_events(&trigger.id, None, 10).await.unwrap();
    assert!(events.iter().any(|e| e.untriaged && e.status == EventStatus::Handled), "{events:?}");

    // A storm: the limit bounds runs and deliveries, and the rest keep waiting.
    for n in 0..30 {
        send_event(&runtime, &trigger.id, serde_json::json!({ "ref": "master", "n": n })).await;
    }
    settled(&runtime, &trigger.id).await;
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert_eq!(deps.storage.list_trigger_runs(&trigger.id, None, 20).await.unwrap().len(), 3);
    assert!(results().await <= 3);
    let events = deps.storage.list_trigger_events(&trigger.id, None, 100).await.unwrap();
    assert_eq!(events.len(), 34, "every event is accounted for");
    assert!(events.iter().any(|e| e.status == EventStatus::Waiting));

    // The model goes away: events count as important and the trigger is marked, told once.
    let mut gone = deps.storage.get_trigger_by_id(&trigger.id).await.unwrap().unwrap();
    gone.triage.as_mut().unwrap().provider = Some("nosuchprovider".into());
    deps.storage.save_trigger(&gone).await.unwrap();
    send_event(&runtime, &trigger.id, serde_json::json!({ "ref": "feature-c" })).await;
    send_event(&runtime, &trigger.id, serde_json::json!({ "ref": "feature-d" })).await;
    settled(&runtime, &trigger.id).await;
    let marked = deps.storage.get_trigger_by_id(&trigger.id).await.unwrap().unwrap();
    assert!(marked.needs_attention.is_some() && marked.is_active, "{marked:?}");
    let notes = deps.storage.list_notifications(&owner, &agents, NotificationFilter::All, None, 50).await.unwrap();
    assert_eq!(notes.iter().filter(|n| n.text.contains("needs a triage model")).count(), 1);
    runtime.shutdown().await.unwrap();
}

/// FR-050: a quiet run delivers nothing and the unimportant and quiet work reaches one summary.
#[tokio::test(flavor = "multi_thread")]
async fn a_run_with_nothing_to_report_delivers_nothing_and_reaches_the_summary() {
    let Some((_dummyplug, runtime, _dir, owner, _)) = harness().await else { return };
    let deps = runtime.deps();
    // Past the scheduler's first tick, which would otherwise start the run below itself, or close
    // the summary window while it is still empty.
    tokio::time::sleep(Duration::from_secs(2)).await;
    let mut trigger = webhook(&owner, "tok");
    trigger.summary_hours = Some(1);
    trigger.created_at = vizier_core::time::now() - chrono::Duration::hours(2);
    deps.storage.create_trigger(&trigger).await.unwrap();
    // A run that is open when the agent says there is nothing to report.
    let started = triggers::now_ms();
    let session = VizierSession("plug".into(), VizierChannelId::Trigger("ci".into(), started), None);
    send_waiting_event(deps, &trigger).await;
    let run = deps
        .storage
        .start_trigger_run(&trigger, started, session.to_slug())
        .await
        .unwrap()
        .expect("a run")
        .run;
    let call = serde_json::json!({ "tool": "execute_python", "arguments": {
        "intent": "quiet", "code": "nothing_to_report(note=\"all green\")" } })
    .to_string();
    let (tx, rx) = runtime.transport().reply_channel().await.unwrap();
    let request = VizierRequest {
        timestamp: vizier_core::time::now(),
        user: "plug".into(),
        content: VizierRequestContent::Prompt(call),
        metadata: serde_json::json!({}),
        ..Default::default()
    };
    runtime.transport().send_request(session.clone(), request, Some(tx)).await.unwrap();
    tokio::time::timeout(TIMEOUT, async {
        while let Ok(response) = rx.recv_async().await {
            if response.ends_turn() {
                return;
            }
        }
    })
    .await
    .unwrap();
    // Closes the run as its dispatcher would.
    triggers::resume(deps).await;
    eventually(30, || async {
        deps.storage.get_trigger_run(run.id).await.unwrap().unwrap().state == TriggerRunState::Answered
    })
    .await;
    let closed = deps.storage.get_trigger_run(run.id).await.unwrap().unwrap();
    assert_eq!(closed.quiet_note.as_deref(), Some("all green"));

    triggers::tick(deps).await;
    let agents = vec!["plug".to_string()];
    let notes = deps.storage.list_notifications(&owner, &agents, NotificationFilter::All, None, 50).await.unwrap();
    assert!(notes.iter().all(|n| n.category != NotificationCategory::TriggerResult), "{notes:?}");
    assert_eq!(notes.iter().filter(|n| n.category == NotificationCategory::TriggerNotice).count(), 1);
    triggers::tick(deps).await;
    let again = deps.storage.list_notifications(&owner, &agents, NotificationFilter::All, None, 50).await.unwrap();
    assert_eq!(again.len(), notes.len(), "one summary per window");
    runtime.shutdown().await.unwrap();
}

async fn send_waiting_event(deps: &vizier_core::dependencies::VizierDependencies, trigger: &Trigger) {
    use vizier_core::{schema::{EventStatus, WhenFull}, storage::trigger::NewEvent};
    deps.storage
        .admit_event(NewEvent {
            trigger_id: trigger.id.clone(),
            agent_id: trigger.agent_id.clone(),
            received_at: triggers::now_ms(),
            payload: Some(serde_json::json!({})),
            delivery_id: None,
            status: EventStatus::Waiting,
            reason: None,
            when_full: WhenFull::DropOldest,
            monitor_state: None,
        })
        .await
        .unwrap();
}

/// FR-031 and SC-008: a monitor's script calls the tools its trigger lists, through the agent,
/// and no others.
#[tokio::test(flavor = "multi_thread")]
async fn a_monitor_script_calls_only_the_tools_it_lists() {
    use vizier_core::triggers::monitor;
    let Some((_dummyplug, runtime, _dir, owner, _)) = harness().await else { return };
    let deps = runtime.deps();
    let mut trigger = webhook(&owner, "unused");
    trigger.kind = "monitor".into();
    trigger.settings = serde_json::json!({ "script": "t = current_time()\n{\"state\": 1}", "tools": ["time"] });
    deps.storage.create_trigger(&trigger).await.unwrap();
    let allowed = monitor::check_once(deps, &trigger).await;
    assert_eq!(allowed.error, None, "{allowed:?}");

    for (script, tools) in [
        ("current_time()\n{\"state\": 2}", serde_json::json!([])),
        ("set_secret(name=\"a\", value=\"b\")\n{\"state\": 2}", serde_json::json!(["time"])),
        ("spawn_subagents(tasks=[\"x\"])\n{\"state\": 2}", serde_json::json!(["subagents"])),
    ] {
        trigger.settings = serde_json::json!({ "script": script, "tools": tools });
        let refused = monitor::check_once(deps, &trigger).await;
        assert!(refused.error.is_some(), "{script}: {refused:?}");
    }
    assert_eq!(deps.storage.monitor_record(&trigger.id).await.unwrap().state, Some(serde_json::json!(1)));
    runtime.shutdown().await.unwrap();
}
