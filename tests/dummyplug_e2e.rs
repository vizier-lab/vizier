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

use vizier::{
    config::{VizierConfig, provider::ProviderId},
    runtime::{RuntimeEvent, RuntimeExit, RuntimeOptions, SubsystemState, VizierRuntime},
    schema::{
        AgentCommand, AgentConfig, AgentToolsConfig, AgentCommandResult, SessionHistoryContent, VizierChannelId, VizierRequest,
        VizierRequestContent, VizierResponse, VizierResponseContent, VizierSession,
        provider::{ProviderCredentials, ProviderEntry},
    },
    storage::{history::HistoryStorage, provider::ProviderStorage},
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
        let server = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("dummyplug/src/server.mjs");
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
        auto_context: Default::default(),
        completion_retry: Default::default(),
    }
}

/// The text of a turn's final message, or the error it ended with.
async fn chat(transport: &VizierTransport, session: &VizierSession, text: &str) -> String {
    let (tx, rx) = flume::unbounded::<VizierResponse>();
    let request = VizierRequest {
        timestamp: chrono::Utc::now(),
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
    let second = VizierRuntime::start(RuntimeOptions { config }).await;
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
    assert!(health.subsystems.iter().any(|(name, state)| *name == "scheduler"
        && matches!(state, SubsystemState::Stopped { .. })));
    assert!(health.subsystems.iter().any(|(name, state)| *name == "agent manager"
        && *state == SubsystemState::Running));
    assert!(health.agents.iter().any(|a| a.agent_id == "plug"), "{:?}", health.agents);

    runtime.shutdown().await.unwrap();
    // The workspace is free again once the runtime is gone.
    let config = VizierConfig::with_workspace(dir.path().to_path_buf());
    VizierRuntime::start(RuntimeOptions { config })
        .await
        .unwrap()
        .shutdown()
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_runtime_needs_an_explicit_workspace() {
    let config = VizierConfig::with_workspace("".into());
    let error = VizierRuntime::start(RuntimeOptions { config }).await.err();
    assert!(error.expect("refused").to_string().contains("workspace"));
}
