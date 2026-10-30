//! The in-process dummyplug harness (`specs/037-core-extraction/` T008, research R11): one
//! agent on the `dummyplug` provider in a temp workspace, driven through the transport the way
//! a channel drives it, checked against `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`.
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

use crate::{
    agents::VizierAgents,
    config::{VizierConfig, provider::ProviderId},
    dependencies::VizierDependencies,
    schema::{
        AgentCommand, AgentCommandResult, SessionHistoryContent, VizierChannelId, VizierRequest,
        VizierRequestContent, VizierResponse, VizierResponseContent, VizierSession,
        provider::{ProviderCredentials, ProviderEntry},
    },
    storage::{history::HistoryStorage, provider::ProviderStorage},
    tests_support::agent_config,
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

/// The text of a turn's final message, or the error it ended with.
async fn chat(deps: &VizierDependencies, session: &VizierSession, text: &str) -> String {
    let (tx, rx) = flume::unbounded::<VizierResponse>();
    let request = VizierRequest {
        timestamp: chrono::Utc::now(),
        user: "e2e".into(),
        content: VizierRequestContent::Chat(text.into()),
        metadata: serde_json::json!({}),
        ..Default::default()
    };
    deps.transport
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
async fn dummyplug_agent_answers_through_the_transport() {
    let Some(dummyplug) = Dummyplug::start() else {
        return;
    };
    dummyplug.ready().await;

    let dir = tempfile::tempdir().unwrap();
    let config = VizierConfig {
        workspace: dir.path().to_string_lossy().to_string(),
        ..Default::default()
    };
    let deps = VizierDependencies::new(config).await.unwrap();
    deps.storage
        .upsert_provider(&ProviderEntry {
            id: ProviderId::DUMMYPLUG,
            credentials: ProviderCredentials {
                base_url: Some(dummyplug.base_url()),
                ..Default::default()
            },
        })
        .await
        .unwrap();

    let mut agents = VizierAgents::new(deps.clone()).await.unwrap();
    let manager = tokio::spawn(async move { agents.run().await });
    let handler_deps = deps.clone();
    let handler = tokio::spawn(async move { handler_deps.run().await });

    // The HTTP API's defaults; `agent_config` leaves both at zero, which times everything out.
    let mut config = agent_config("plug");
    config.prompt_timeout = Duration::from_secs(60 * 60).into();
    config.tools.timeout = Duration::from_secs(30 * 60).into();
    let (resp, created) = tokio::sync::oneshot::channel();
    deps.transport
        .send_agent_command(AgentCommand::Create {
            agent_id: "plug".into(),
            config,
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

    // §1: the sandbox tools, and only those (code mode).
    let tools = chat(&deps, &session, "tools").await;
    for name in ["think", "execute_python", "list_tool_functions", "describe_tool_function"] {
        assert!(tools.contains(&format!("`{name}`")), "`tools` lists {name}: {tools}");
    }

    // §2: a sample request for a tool, accepted by §3 once its code is filled in.
    let sample = chat(&deps, &session, "execute_python").await;
    let mut request = json_block(&sample);
    assert_eq!(request["tool"], "execute_python", "{sample}");
    request["arguments"]["code"] = "6 * 7".into();

    // §3 + §4: the tool runs through the real loop and its result is echoed.
    let result = chat(&deps, &session, &request.to_string()).await;
    assert!(result.starts_with("**Tool result** (`execute_python`)"), "{result}");
    assert!(result.contains("42"), "{result}");
    let history = deps
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
    let context = chat(&deps, &session, "context").await;
    assert!(context.starts_with("# Context"), "{context}");

    manager.abort();
    handler.abort();
}
