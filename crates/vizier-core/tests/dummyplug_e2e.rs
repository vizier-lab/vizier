//! The in-process dummyplug harness (`specs/037-core-extraction/` T008/T015, research R11): one
//! agent on the `dummyplug` provider in a temp workspace, driven through the transport the way
//! a channel drives it, through `VizierRuntime` only (T033), checked against `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`.
//!
//! `dummyplug` is the Node server in the `dummyplug` submodule, reached over HTTP like any
//! OpenAI-compatible provider. The harness starts it on a free port and points the provider's
//! stored `base_url` at it. Without Node or the submodule the test is skipped locally and fails
//! under `CI`.

use std::{
    net::TcpListener,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::Duration,
};

use vizier_core::{
    config::{VizierConfig, provider::ProviderId},
    runtime::{RuntimeEvent, RuntimeExit, RuntimeOptions, SubsystemState, VizierRuntime},
    schema::{
        AgentCommand, AgentConfig, AgentToolsConfig, AgentCommandResult, SessionHistoryContent, VizierChannelId, VizierRequest,
        VizierRequestContent, VizierResponseContent, VizierSession,
        provider::{ProviderCredentials, ProviderEntry},
    },
    storage::{
        history::HistoryStorage, project::ProjectStorage, provider::ProviderStorage,
        user::UserStorage,
    },
    transport::VizierTransport,
};

const TURN_TIMEOUT: Duration = Duration::from_secs(60);

/// The Node dummyplug server, killed on drop.
struct Dummyplug {
    child: Child,
    port: u16,
}

impl Dummyplug {
    /// `None` when Node or the submodule is missing and this isn't CI.
    fn start() -> Option<Self> {
        let server = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dummyplug/src/server.mjs");
        let missing = if !server.exists() {
            Some(format!("{} not found (git submodule update --init dummyplug)", server.display()))
        } else if Command::new("node").arg("--version").output().is_err() {
            Some("node not found".to_string())
        } else {
            None
        };
        if let Some(why) = missing {
            assert!(std::env::var_os("CI").is_none(), "dummyplug harness can't run: {why}");
            eprintln!("skipping the dummyplug harness: {why}");
            return None;
        }
        let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
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

    fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}/v1", self.port)
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

/// A dummyplug agent with the HTTP API's defaults where they matter: a zero `prompt_timeout`
/// or tool timeout would time every turn out.
fn agent_config(id: &str) -> AgentConfig {
    AgentConfig {
        name: format!("Agent {id}"),
        owner_id: None,
        shared_to: vec![],
        system_prompt: None,
        description: None,
        provider: ProviderId::DUMMYPLUG,
        model: "dummyplug".into(),
        thinking_depth: 1,
        checkpoint_threshold: 0.8,
        tools: AgentToolsConfig {
            timeout: Duration::from_secs(30 * 60).into(),
            ..Default::default()
        },
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

/// The text of a turn's final message, or the error it ended with.
async fn chat(transport: &VizierTransport, session: &VizierSession, text: &str) -> String {
    let (tx, rx) = transport.reply_channel().await.expect("open a reply stream");
    let request = VizierRequest {
        timestamp: vizier_core::time::now(),
        user: "e2e".into(),
        content: VizierRequestContent::Chat(text.into()),
        metadata: serde_json::json!({}),
        ..Default::default()
    };
    transport
        .send_request(session.clone(), request, Some(tx))
        .await
        .expect("send the request");
    let reply = tokio::time::timeout(TURN_TIMEOUT, async {
        while let Ok(response) = rx.recv_async().await {
            match response.content {
                VizierResponseContent::Message { content, .. } => return content,
                VizierResponseContent::Error { kind, message } => {
                    panic!("turn {text:?} failed ({kind:?}): {message}")
                }
                _ => {}
            }
        }
        panic!("turn {text:?} ended without a message");
    })
    .await;
    reply.unwrap_or_else(|_| panic!("turn {text:?} timed out"))
}

/// The first fenced ```json block of a reply.
fn json_block(reply: &str) -> serde_json::Value {
    let start = reply.find("```json").expect("a json block") + "```json".len();
    let end = start + reply[start..].find("```").expect("a closing fence");
    serde_json::from_str(reply[start..end].trim()).expect("valid json")
}

#[tokio::test(flavor = "multi_thread")]
async fn dummyplug_agent_answers_through_the_runtime() {
    let Some(dummyplug) = Dummyplug::start() else {
        return;
    };
    dummyplug.ready().await;

    let dir = tempfile::tempdir().unwrap();
    let config = VizierConfig::with_workspace(dir.path().to_path_buf());
    let mut runtime = VizierRuntime::start(RuntimeOptions {
        config: config.clone(),
        ..Default::default()
    })
    .await
    .unwrap();
    let mut events = runtime.events();
    // Pointing the provider at this server is a person's job (Settings › Providers); the
    // harness writes the row directly.
    runtime
        .deps()
        .storage
        .upsert_provider(&ProviderEntry {
            id: ProviderId::DUMMYPLUG,
            credentials: ProviderCredentials {
                base_url: Some(dummyplug.base_url()),
                ..Default::default()
            },
        })
        .await
        .unwrap();

    let (resp, created) = tokio::sync::oneshot::channel();
    runtime
        .transport()
        .send_agent_command(AgentCommand::Create {
            agent_id: "plug".into(),
            config: agent_config("plug"),
            resp,
        })
        .await
        .unwrap();
    match tokio::time::timeout(TURN_TIMEOUT, created).await.unwrap().unwrap() {
        AgentCommandResult::Ok(summary) => assert_eq!(summary.agent_id, "plug"),
        AgentCommandResult::Error(e) => panic!("create failed: {e}"),
    }

    let session = VizierSession(
        "plug".into(),
        VizierChannelId::HTTP("e2e".into(), "harness".into()),
        Some("first".into()),
    );
    let transport = runtime.transport();

    // §1: the sandbox tools, and only those (code mode).
    let tools = chat(transport, &session, "tools").await;
    for name in ["think", "execute_python", "list_tool_functions", "describe_tool_function"] {
        assert!(tools.contains(&format!("`{name}`")), "`tools` lists {name}: {tools}");
    }

    // §2: a sample request for a tool, accepted by §3 once its code is filled in.
    let sample = chat(transport, &session, "execute_python").await;
    let mut request = json_block(&sample);
    assert_eq!(request["tool"], "execute_python", "{sample}");
    request["arguments"]["code"] = "6 * 7".into();

    // §3 + §4: the tool runs through the real loop and its result is echoed.
    let result = chat(transport, &session, &request.to_string()).await;
    assert!(result.starts_with("**Tool result** (`execute_python`)"), "{result}");
    assert!(result.contains("42"), "{result}");
    let history = runtime
        .deps()
        .storage
        .list_session_history(session.clone(), None, None, None)
        .await
        .unwrap();
    let call_id = history
        .iter()
        .find_map(|h| match &h.content {
            SessionHistoryContent::ToolCall { call_id, name, .. } if name == "execute_python" => {
                Some(call_id.clone())
            }
            _ => None,
        })
        .expect("history holds the tool call");
    assert!(
        history.iter().any(|h| matches!(
            &h.content,
            SessionHistoryContent::ToolResult { call_id: id, content } if *id == call_id && content.contains("42")
        )),
        "history holds the tool result"
    );

    // §6: the per-request context block the agent received.
    let context = chat(transport, &session, "context").await;
    assert!(context.starts_with("# Context"), "{context}");

    // One engine per workspace: a second start is refused while this one runs (R14).
    let second = VizierRuntime::start(RuntimeOptions {
        config,
        ..Default::default()
    }).await;
    let error = second.err().expect("a second runtime on the same workspace is refused");
    assert!(error.to_string().contains("already in use"), "{error}");

    // A subsystem that stops is reported, and the process keeps running (SC-008).
    assert!(runtime.stop_subsystem_for_test("scheduler"));
    let exit = tokio::time::timeout(TURN_TIMEOUT, runtime.wait()).await.unwrap();
    assert_eq!(
        exit,
        RuntimeExit::SubsystemStopped {
            name: "scheduler",
            outcome: "it returned".into()
        }
    );
    let stopped = loop {
        match events.recv().await.unwrap() {
            RuntimeEvent::SubsystemStopped { name, .. } => break name,
            _ => continue,
        }
    };
    assert_eq!(stopped, "scheduler");
    let health = runtime.health().await;
    // Fleet (#161): the default runtime is every role in one process.
    assert!(health.roles.is_all(), "{:?}", health.roles);
    assert!(health.subsystems.iter().any(|(name, state)| *name == "scheduler"
        && matches!(state, SubsystemState::Stopped { .. })));
    assert!(health.subsystems.iter().any(|(name, state)| *name == "agent manager"
        && *state == SubsystemState::Running));
    assert!(health.agents.iter().any(|a| a.agent_id == "plug"), "{:?}", health.agents);

    runtime.shutdown().await.unwrap();
    // The workspace is free again once the runtime is gone.
    let config = VizierConfig::with_workspace(dir.path().to_path_buf());
    VizierRuntime::start(RuntimeOptions {
        config,
        ..Default::default()
    })
        .await
        .unwrap()
        .shutdown()
        .await
        .unwrap();
}

/// Spec 053 US7: text a model writes beside a tool call is saved as a mid-turn message, ahead of
/// the call, so a chat can draw trail, message, trail, reply (quickstart §12, backend half).
#[tokio::test(flavor = "multi_thread")]
async fn text_beside_a_tool_call_is_saved_as_a_mid_turn_message() {
    let Some(dummyplug) = Dummyplug::start() else {
        return;
    };
    dummyplug.ready().await;

    let dir = tempfile::tempdir().unwrap();
    let runtime = VizierRuntime::start(RuntimeOptions {
        config: VizierConfig::with_workspace(dir.path().to_path_buf()),
        ..Default::default()
    })
    .await
    .unwrap();
    runtime
        .deps()
        .storage
        .upsert_provider(&ProviderEntry {
            id: ProviderId::DUMMYPLUG,
            credentials: ProviderCredentials {
                base_url: Some(dummyplug.base_url()),
                ..Default::default()
            },
        })
        .await
        .unwrap();
    let (resp, created) = tokio::sync::oneshot::channel();
    runtime
        .transport()
        .send_agent_command(AgentCommand::Create {
            agent_id: "plug".into(),
            config: agent_config("plug"),
            resp,
        })
        .await
        .unwrap();
    assert!(matches!(
        tokio::time::timeout(TURN_TIMEOUT, created).await.unwrap().unwrap(),
        AgentCommandResult::Ok(_)
    ));

    let session = VizierSession(
        "plug".into(),
        VizierChannelId::HTTP("e2e".into(), "harness".into()),
        Some("say".into()),
    );
    let request = serde_json::json!({
        "say": "checking first",
        "tool": "execute_python",
        "arguments": { "intent": "add", "code": "6 * 7" }
    });
    let reply = chat(runtime.transport(), &session, &request.to_string()).await;
    assert!(reply.contains("42"), "{reply}");

    let history = runtime
        .deps()
        .storage
        .list_session_history(session, None, None, None)
        .await
        .unwrap();
    let kinds: Vec<&str> = history
        .iter()
        .map(|h| match &h.content {
            SessionHistoryContent::Request(_) => "request",
            SessionHistoryContent::AssistantMessage(text) if text == "checking first" => "say",
            SessionHistoryContent::ToolCall { .. } => "call",
            SessionHistoryContent::ToolResult { .. } => "result",
            SessionHistoryContent::Response(_) => "response",
            _ => "other",
        })
        .collect();
    assert_eq!(kinds, ["request", "say", "call", "result", "response"], "{history:#?}");

    runtime.shutdown().await.unwrap();
}

/// Spec 053 US1 and US2 (quickstart §2 to §5): a Workspace with two dummyplug agents. Unmentioned
/// messages cost nothing; a mention wakes only that agent, who has read what came before; a
/// mention in an agent's mid-turn text wakes the next one.
#[tokio::test(flavor = "multi_thread")]
async fn a_workspace_wakes_only_who_is_mentioned_and_they_have_read_the_rest() {
    use vizier_core::{
        auth::AuthMethod,
        schema::{EntryKind, MentionState, VizierRequestContent},
        service::{
            Services,
            workspaces::{self as ws, AddMemberRequest, CreateWorkspaceRequest, PostRequest},
        },
        storage::{turn::TurnStorage, workspace::WorkspaceStorage},
    };

    let Some(dummyplug) = Dummyplug::start() else {
        return;
    };
    dummyplug.ready().await;

    let dir = tempfile::tempdir().unwrap();
    let runtime = VizierRuntime::start(RuntimeOptions {
        config: VizierConfig::with_workspace(dir.path().to_path_buf()),
        ..Default::default()
    })
    .await
    .unwrap();
    let storage = runtime.deps().storage.clone();
    storage
        .upsert_provider(&ProviderEntry {
            id: ProviderId::DUMMYPLUG,
            credentials: ProviderCredentials {
                base_url: Some(dummyplug.base_url()),
                ..Default::default()
            },
        })
        .await
        .unwrap();
    let role = storage
        .create_role("maker", vec!["workspaces:create".into(), "all_agents:edit".into(), "all_agents:view".into()], false)
        .await
        .unwrap();
    let user = storage.create_user("rafi", "", &role.role_id).await.unwrap();
    let s = Services::new(runtime.deps().clone());
    let rafi = s.principal_for(&user.user_id, AuthMethod::Jwt).await.unwrap();

    for id in ["alpha", "beta"] {
        let (resp, created) = tokio::sync::oneshot::channel();
        runtime
            .transport()
            .send_agent_command(AgentCommand::Create { agent_id: id.into(), config: agent_config(id), resp })
            .await
            .unwrap();
        assert!(matches!(
            tokio::time::timeout(TURN_TIMEOUT, created).await.unwrap().unwrap(),
            AgentCommandResult::Ok(_)
        ));
    }

    let made = ws::create(&s, &rafi, CreateWorkspaceRequest { name: "ops".into(), first_topic: None })
        .await
        .unwrap();
    let (wid, tid) = (made.workspace.id.clone(), made.workspace.topics[0].id.clone());
    for id in ["alpha", "beta"] {
        ws::add_member(&s, &rafi, &wid, AddMemberRequest { kind: "agent".into(), handle: id.into(), role: None })
            .await
            .unwrap();
    }
    let say = |text: &str| PostRequest { text: text.into(), files: vec![] };
    let history_of = |agent: &str| {
        let storage = storage.clone();
        let session = VizierSession(agent.into(), VizierChannelId::Workspace(wid.clone()), Some(tid.clone()));
        async move { storage.list_session_history(session, None, None, None).await.unwrap() }
    };
    let wait_for = |what: &'static str, f: Box<dyn Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>> + Send>| async move {
        let start = std::time::Instant::now();
        while start.elapsed() < TURN_TIMEOUT {
            if f().await {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        panic!("timed out waiting for {what}");
    };

    // §2: silent messages reach everyone and cost nothing.
    for text in ["the billing lock is back", "it was the deploy", "rolling back now"] {
        ws::post(&s, &rafi, &wid, &tid, say(text)).await.unwrap();
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(storage.sessions_with_pending_turns().await.unwrap().is_empty());
    assert!(history_of("alpha").await.is_empty() && history_of("beta").await.is_empty());
    assert_eq!(storage.list_entries(&tid, None, 50).await.unwrap().len(), 3);

    // §3: a mention wakes one agent, who has read the rest.
    ws::post(&s, &rafi, &wid, &tid, say("@alpha tools")).await.unwrap();
    {
        let storage = storage.clone();
        let tid = tid.clone();
        wait_for("alpha's reply", Box::new(move || {
            let storage = storage.clone();
            let tid = tid.clone();
            Box::pin(async move {
                storage.list_entries(&tid, None, 50).await.unwrap().iter().any(|e| e.kind == EntryKind::Agent && e.is_final)
            })
        }))
        .await;
    }
    let alpha = history_of("alpha").await;
    let reads: Vec<String> = alpha
        .iter()
        .filter_map(|h| match &h.content {
            SessionHistoryContent::Request(r) => match &r.content {
                VizierRequestContent::SilentRead(t) => Some(t.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(reads, ["the billing lock is back", "it was the deploy", "rolling back now"]);
    assert!(history_of("beta").await.is_empty(), "beta was not mentioned, so its record is untouched");

    // §5: mid-turn text that mentions another agent wakes it before the first one finishes.
    let before = storage.list_entries(&tid, None, 100).await.unwrap().len();
    ws::post(
        &s,
        &rafi,
        &wid,
        &tid,
        say(r#"@alpha {"say":"asking @beta now","tool":"execute_python","arguments":{"intent":"x","code":"1"}}"#),
    )
    .await
    .unwrap();
    {
        let storage = storage.clone();
        let tid = tid.clone();
        let wid = wid.clone();
        wait_for("beta woken by alpha's mid-turn text", Box::new(move || {
            let storage = storage.clone();
            let tid = tid.clone();
            let wid = wid.clone();
            Box::pin(async move {
                let entries = storage.list_entries(&tid, None, 100).await.unwrap();
                if !entries.iter().any(|e| e.author == "beta" && e.is_final) {
                    return false;
                }
                // Alpha's prose between tool rounds is saved but not shown, so the timeline above
                // cannot carry it: the entry is found through alpha's own record instead.
                let session =
                    VizierSession("alpha".into(), VizierChannelId::Workspace(wid), Some(tid));
                let said = storage
                    .list_session_history(session, None, None, None)
                    .await
                    .unwrap()
                    .into_iter()
                    .find(|h| matches!(&h.content,
                        SessionHistoryContent::AssistantMessage(text) if text.contains("asking @beta")));
                let Some(said) = said else { return false };
                let Some(entry) = storage.entry_by_history_uid(&said.uid).await.unwrap() else {
                    return false;
                };
                let m = storage.mentions_of(&[entry.seq]).await.unwrap();
                m.iter().any(|m| m.target == "beta" && m.state == MentionState::Woken)
            })
        }))
        .await;
    }
    let timeline = storage.list_entries(&tid, None, 100).await.unwrap();
    assert!(timeline.len() > before);
    // It is alpha thinking out loud, so it woke beta without anyone being shown it.
    assert!(!timeline.iter().any(|e| e.author == "alpha" && e.body.contains("asking @beta")), "{timeline:#?}");
    // Beta now has read alpha's shown messages (and none of its tool calls).
    let beta = history_of("beta").await;
    assert!(beta.iter().any(|h| matches!(&h.content, SessionHistoryContent::Request(r)
        if matches!(&r.content, VizierRequestContent::SilentRead(_)) && r.user == "alpha")));
    assert!(!beta.iter().any(|h| matches!(&h.content, SessionHistoryContent::ToolCall { .. })
        && h.vizier_session.0 == "alpha"));

    runtime.shutdown().await.unwrap();
}

/// Spec 040: the workspace shell is offered in a project session, runs a command in the project
/// folder, and is absent from an ordinary conversation.
#[tokio::test(flavor = "multi_thread")]
async fn a_project_session_can_run_the_workspace_shell() {
    let Some(dummyplug) = Dummyplug::start() else {
        return;
    };
    dummyplug.ready().await;

    let dir = tempfile::tempdir().unwrap();
    let runtime = VizierRuntime::start(RuntimeOptions {
        config: VizierConfig::with_workspace(dir.path().to_path_buf()),
        ..Default::default()
    })
    .await
    .unwrap();
    runtime
        .deps()
        .storage
        .upsert_provider(&ProviderEntry {
            id: ProviderId::DUMMYPLUG,
            credentials: ProviderCredentials {
                base_url: Some(dummyplug.base_url()),
                ..Default::default()
            },
        })
        .await
        .unwrap();

    let role = runtime
        .deps()
        .storage
        .create_role(
            "projects",
            vec!["all_agents:view".into(), "all_agents:projects".into()],
            false,
        )
        .await
        .unwrap();
    runtime
        .deps()
        .storage
        .create_user("e2e", "", &role.role_id)
        .await
        .unwrap();

    let folder = tempfile::tempdir().unwrap();
    std::fs::write(folder.path().join("AGENTS.md"), "Always answer in French.").unwrap();
    std::fs::write(folder.path().join("main.rs"), "fn greet() {}\nfn main() { greet(); }\n").unwrap();
    let project = vizier_core::schema::Project {
        id: "p3xY9aQ2mZ".into(),
        agent_id: "plug".into(),
        name: "alpha".into(),
        folder: std::fs::canonicalize(folder.path()).unwrap(),
        owner_id: "u-e2e".into(),
        shell_enabled: true,
        lsp_enabled: false,
        created_at: 0,
        removed_at: None,
    };
    runtime.deps().storage.open_project(project).await.unwrap();

    let (resp, created) = tokio::sync::oneshot::channel();
    runtime
        .transport()
        .send_agent_command(AgentCommand::Create {
            agent_id: "plug".into(),
            config: agent_config("plug"),
            resp,
        })
        .await
        .unwrap();
    match tokio::time::timeout(TURN_TIMEOUT, created).await.unwrap().unwrap() {
        AgentCommandResult::Ok(_) => {}
        AgentCommandResult::Error(e) => panic!("create failed: {e}"),
    }
    let transport = runtime.transport();

    let project_session = VizierSession(
        "plug".into(),
        VizierChannelId::Project("e2e".into(), "p3xY9aQ2mZ".into()),
        Some("project-1".into()),
    );
    // The shell is a Build-mode tool; a new project chat starts in Plan (spec 041 FR-009).
    runtime
        .deps()
        .storage
        .set_project_chat_mode(
            "plug",
            &project_session.1.to_slug(),
            "project-1",
            vizier_core::schema::ProjectMode::Build,
        )
        .await
        .unwrap();
    let list = chat(
        transport,
        &project_session,
        r#"{"tool":"list_tool_functions","arguments":{"category":"coding"}}"#,
    )
    .await;
    assert!(list.contains("workspace_shell"), "project shell is listed under coding: {list}");

    let outcome = chat(
        transport,
        &project_session,
        r#"{"tool":"execute_python","arguments":{"intent":"run echo in the project folder","code":"print(workspace_shell(command='echo hello'))"}}"#,
    )
    .await;
    assert!(outcome.contains("hello"), "the command output comes back: {outcome}");
    assert!(outcome.contains("exit_code"), "the result carries the exit code: {outcome}");

    // The coding tools read and edit through the same code-mode bridge, with the stale-read
    // guard making the edit legal only after the read (spec 041 US1, quickstart §3).
    let outcome = chat(
        transport,
        &project_session,
        r#"{"tool":"execute_python","arguments":{"intent":"rename greet","code":"print(read_file(path='main.rs'))\nprint(edit_file(path='main.rs', old_text='greet', new_text='hello'))"}}"#,
    )
    .await;
    assert!(outcome.contains("stale_read") || outcome.contains("edit_ambiguous"), "an unread or ambiguous edit is refused: {outcome}");
    let outcome = chat(
        transport,
        &project_session,
        r#"{"tool":"execute_python","arguments":{"intent":"rename greet everywhere","code":"print(read_file(path='main.rs'))\nprint(edit_file(path='main.rs', old_text='greet', new_text='hello', replace_all=True))"}}"#,
    )
    .await;
    assert!(outcome.contains("replacements"), "the edit applies after a read: {outcome}");
    assert_eq!(
        std::fs::read_to_string(folder.path().join("main.rs")).unwrap(),
        "fn hello() {}\nfn main() { hello(); }\n"
    );

    // An ordinary conversation has neither the tool nor the project section.
    let ordinary = VizierSession(
        "plug".into(),
        VizierChannelId::HTTP("e2e".into(), "harness".into()),
        Some("plain-1".into()),
    );
    let list = chat(
        transport,
        &ordinary,
        r#"{"tool":"list_tool_functions","arguments":{"category":"coding"}}"#,
    )
    .await;
    assert!(!list.contains("workspace_shell"), "ordinary turn has no project shell: {list}");

    runtime.shutdown().await.unwrap();
}

/// Spec 054 (quickstart §1 and §3): a project chat is offered the `lsp_*` tools, an edit's result
/// says whether the code still holds together, and switching the feature off takes all of it away.
/// The server is the `fake_lsp` example, set as the project's override, so nothing needs installing.
#[tokio::test(flavor = "multi_thread")]
async fn a_project_edit_learns_what_it_broke_from_a_language_server() {
    use vizier_core::schema::ServerOverride;

    let Some(dummyplug) = Dummyplug::start() else {
        return;
    };
    dummyplug.ready().await;

    let fake = std::env::current_exe()
        .unwrap()
        .parent()
        .and_then(|p| p.parent())
        .unwrap()
        .join("examples")
        .join("fake_lsp");
    assert!(fake.exists(), "build the fixture first: cargo build -p vizier-core --example fake_lsp");

    let dir = tempfile::tempdir().unwrap();
    let runtime = VizierRuntime::start(RuntimeOptions {
        config: VizierConfig::with_workspace(dir.path().to_path_buf()),
        ..Default::default()
    })
    .await
    .unwrap();
    let storage = &runtime.deps().storage;
    storage
        .upsert_provider(&ProviderEntry {
            id: ProviderId::DUMMYPLUG,
            credentials: ProviderCredentials {
                base_url: Some(dummyplug.base_url()),
                ..Default::default()
            },
        })
        .await
        .unwrap();
    let role = storage
        .create_role("projects", vec!["all_agents:view".into(), "all_agents:projects".into()], false)
        .await
        .unwrap();
    storage.create_user("e2e", "", &role.role_id).await.unwrap();

    let folder = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(folder.path().join("src")).unwrap();
    std::fs::write(folder.path().join("src/lib.rs"), "pub fn greet() -> u32 { 1 }\n").unwrap();
    storage
        .open_project(vizier_core::schema::Project {
            id: "p3xY9aQ2mZ".into(),
            agent_id: "plug".into(),
            name: "alpha".into(),
            folder: std::fs::canonicalize(folder.path()).unwrap(),
            owner_id: "u-e2e".into(),
            shell_enabled: true,
            lsp_enabled: true,
            created_at: 0,
            removed_at: None,
        })
        .await
        .unwrap();
    storage
        .set_lsp_override(
            "p3xY9aQ2mZ",
            "rust",
            &ServerOverride {
                command: fake.to_string_lossy().into_owned(),
                args: vec!["--script".into(), "diagnostics".into()],
                env: Default::default(),
            },
            "u-e2e",
            0,
        )
        .await
        .unwrap();

    let (resp, created) = tokio::sync::oneshot::channel();
    runtime
        .transport()
        .send_agent_command(AgentCommand::Create {
            agent_id: "plug".into(),
            config: agent_config("plug"),
            resp,
        })
        .await
        .unwrap();
    match tokio::time::timeout(TURN_TIMEOUT, created).await.unwrap().unwrap() {
        AgentCommandResult::Ok(_) => {}
        AgentCommandResult::Error(e) => panic!("create failed: {e}"),
    }
    let transport = runtime.transport();
    let session = VizierSession(
        "plug".into(),
        VizierChannelId::Project("e2e".into(), "p3xY9aQ2mZ".into()),
        Some("project-1".into()),
    );
    storage
        .set_project_chat_mode(
            "plug",
            &session.1.to_slug(),
            "project-1",
            vizier_core::schema::ProjectMode::Build,
        )
        .await
        .unwrap();

    // §1: the five tools are listed, and the state line reads on.
    let list = chat(transport, &session, r#"{"tool":"list_tool_functions","arguments":{"category":"coding"}}"#).await;
    for name in ["lsp_diagnostics", "lsp_definition", "lsp_references", "lsp_hover", "lsp_symbols"] {
        assert!(list.contains(name), "{name} is listed under coding: {list}");
    }
    let context = chat(transport, &session, "context").await;
    assert!(context.contains("Language servers: on."), "the state line is in the context: {context}");

    // §3: reading the file starts the server in the background; once it is ready the edit's own
    // result carries what it broke.
    chat(
        transport,
        &session,
        r#"{"tool":"execute_python","arguments":{"intent":"read","code":"print(read_file(path='src/lib.rs')['total_lines'])"}}"#,
    )
    .await;
    let began = std::time::Instant::now();
    loop {
        let ready = runtime.deps().lsp.status("p3xY9aQ2mZ").iter().any(|s| s.state == "ready");
        if ready {
            break;
        }
        assert!(began.elapsed() < Duration::from_secs(20), "the server never became ready");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let broken = chat(
        transport,
        &session,
        r#"{"tool":"execute_python","arguments":{"intent":"break it","code":"r = edit_file(path='src/lib.rs', old_text='{ 1 }', new_text='{ ERROR }')\nprint(r['diagnostics']['status'], r['diagnostics']['errors'], r['diagnostics']['items'][0]['message'], r['diagnostics']['items'][0]['path'], r['diagnostics']['items'][0]['line'])"}}"#,
    )
    .await;
    assert!(broken.contains("current 1 found ERROR src/lib.rs 1"), "the edit reports its error: {broken}");

    let fixed = chat(
        transport,
        &session,
        r#"{"tool":"execute_python","arguments":{"intent":"fix it","code":"r = edit_file(path='src/lib.rs', old_text='{ ERROR }', new_text='{ 2 }')\nprint(r['diagnostics']['status'], r['diagnostics']['errors'])"}}"#,
    )
    .await;
    assert!(fixed.contains("current 0"), "reverting reports no errors: {fixed}");

    // A question by meaning, in the same chat.
    let hover = chat(
        transport,
        &session,
        r#"{"tool":"execute_python","arguments":{"intent":"hover","code":"print(lsp_hover(path='src/lib.rs', line=1, symbol='greet')['signature'])"}}"#,
    )
    .await;
    assert!(hover.contains("fn resolve()"), "hover answers: {hover}");

    // An ordinary conversation has none of it.
    let ordinary = VizierSession(
        "plug".into(),
        VizierChannelId::HTTP("e2e".into(), "harness".into()),
        Some("plain-1".into()),
    );
    let list = chat(transport, &ordinary, r#"{"tool":"list_tool_functions","arguments":{"category":"coding"}}"#).await;
    assert!(!list.contains("lsp_definition"), "ordinary turn has no lsp tools: {list}");

    // Off: the tools go, the edit still works and carries no diagnostics, and nothing keeps running.
    storage.set_project_lsp("p3xY9aQ2mZ", false).await.unwrap();
    runtime.deps().lsp.close_project("p3xY9aQ2mZ").await;
    assert!(runtime.deps().lsp.status("p3xY9aQ2mZ").is_empty());
    let list = chat(transport, &session, r#"{"tool":"list_tool_functions","arguments":{"category":"coding"}}"#).await;
    assert!(!list.contains("lsp_definition"), "off removes the tools: {list}");
    let off = chat(
        transport,
        &session,
        r#"{"tool":"execute_python","arguments":{"intent":"edit while off","code":"r = edit_file(path='src/lib.rs', old_text='{ 2 }', new_text='{ 3 }')\nprint('diagnostics' in r, r['replacements'])"}}"#,
    )
    .await;
    assert!(off.contains("False 1"), "an edit while off succeeds without diagnostics: {off}");
    let context = chat(transport, &session, "context").await;
    assert!(context.contains("Language servers: off."), "the state line follows the switch: {context}");

    runtime.shutdown().await.unwrap();
}

/// identity is a memory (`specs/043-unify-core-and-memory`, quickstart steps 1, 3 and 4).
///
/// Everything here is agent-observable: the document exists because the agent was created, the
/// tool that reads it is the ordinary memory tool, and the guard refuses a second document in
/// the reserved bundle without anyone having to know the rule.
#[tokio::test(flavor = "multi_thread")]
async fn an_agents_identity_is_a_memory_at_core_soul() {
    use vizier_core::storage::memory::MemoryStorage;

    let Some(dummyplug) = Dummyplug::start() else {
        return;
    };
    dummyplug.ready().await;

    let dir = tempfile::tempdir().unwrap();
    let config = VizierConfig::with_workspace(dir.path().to_path_buf());
    let runtime = VizierRuntime::start(RuntimeOptions {
        config,
        ..Default::default()
    }).await.unwrap();
    runtime
        .deps()
        .storage
        .upsert_provider(&ProviderEntry {
            id: ProviderId::DUMMYPLUG,
            credentials: ProviderCredentials {
                base_url: Some(dummyplug.base_url()),
                ..Default::default()
            },
        })
        .await
        .unwrap();

    let (resp, created) = tokio::sync::oneshot::channel();
    runtime
        .transport()
        .send_agent_command(AgentCommand::Create {
            agent_id: "plug".into(),
            config: agent_config("plug"),
            resp,
        })
        .await
        .unwrap();
    match tokio::time::timeout(TURN_TIMEOUT, created).await.unwrap().unwrap() {
        AgentCommandResult::Ok(_) => {}
        AgentCommandResult::Error(e) => panic!("create failed: {e}"),
    }

    // Step 1: the seed is a detached task at spawn, so give it a moment rather than assuming
    // it has already run. Nobody created this document — that is the point.
    let soul = {
        let mut found = None;
        for _ in 0..100 {
            match runtime
                .deps()
                .storage
                .get_memory_detail(
                    "plug".into(),
                    Some(vizier_core::schema::CORE_BUNDLE.into()),
                    vizier_core::schema::SOUL_PATH.into(),
                )
                .await
                .unwrap()
            {
                Some(memory) => {
                    found = Some(memory);
                    break;
                }
                None => tokio::time::sleep(Duration::from_millis(100)).await,
            }
        }
        found.expect("the identity document exists without anyone creating it")
    };
    assert!(
        soul.content.contains("## Operating Framework"),
        "a new agent's soul is the embedded template: {}",
        soul.content
    );

    // The reserved bundle is listed with its protection, and the document is inside it.
    let bundles = runtime
        .deps()
        .storage
        .list_bundles("plug".into())
        .await
        .unwrap();
    let core = bundles
        .iter()
        .find(|b| b.name == vizier_core::schema::CORE_BUNDLE)
        .expect("the reserved bundle is listed");
    assert!(core.protected, "a client can disable deleting it");
    assert_eq!(core.concept_count, 1);

    let session = VizierSession(
        "plug".into(),
        VizierChannelId::HTTP("e2e".into(), "identity".into()),
        Some("identity".into()),
    );
    let transport = runtime.transport();

    // Step 3: one vocabulary. The identity-only operations are gone, so an agent holding this
    // agent's configuration is never offered them. (The memory tools themselves need an
    // embedding index, which this harness deliberately does not configure; that they read the
    // document is covered by the storage tests and by `memory_read` being the same accessor.)
    let tools = chat(transport, &session, "tools").await;
    for gone in ["read_core", "write_core"] {
        assert!(!tools.contains(gone), "`{gone}` is no longer a tool: {tools}");
    }
    assert!(!tools.contains("`identity`"), "no identity category: {tools}");

    // What this harness cannot reach: the write path itself, because it needs an indexer and
    // the indexer module is crate-private. The guard (only `soul` in the bundle), the ceiling,
    // the warning and grandfathering are covered by the storage tests against
    // `write_memory_inner`, which is the one function the tool, the HTTP route, rollback and
    // import all call — so a unit test there is a test of every caller.

    runtime.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_runtime_needs_an_explicit_workspace() {
    let config = VizierConfig::with_workspace("".into());
    let error = VizierRuntime::start(RuntimeOptions {
        config,
        ..Default::default()
    }).await.err();
    assert!(error.expect("refused").to_string().contains("workspace"));
}
