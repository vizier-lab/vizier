//! T022: a conversation behaves the same over the WebSocket and over the desktop dispatcher's
//! `open_topic`, and a second connection on a session receives the turn too.
//!
//! The turn is the `dummyplug` provider's `tools` request, so it needs Node and the `dummyplug`
//! submodule; without them the test is skipped (and fails under `CI`).

use std::{
    net::TcpListener,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::Duration,
};

use axum::{body::Body, http::Request};
use futures::{SinkExt, StreamExt};
use serde_json::{Value, json};
use vizier_core::{
    config::VizierConfig,
    runtime::{RuntimeOptions, VizierRuntime},
    storage::user::UserStorage,
};
use vizier_http::{auth::AuthService, embedded::Dispatcher};

struct Dummyplug {
    child: Child,
    port: u16,
}

impl Dummyplug {
    fn start() -> Option<Self> {
        let server = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dummyplug/src/server.mjs");
        let missing = if !server.exists() {
            Some("dummyplug submodule missing")
        } else if Command::new("node").arg("--version").output().is_err() {
            Some("node not found")
        } else {
            None
        };
        if let Some(why) = missing {
            assert!(std::env::var_os("CI").is_none(), "cannot run dummyplug: {why}");
            eprintln!("skipping the topic checks: {why}");
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
            .expect("start dummyplug");
        Some(Self { child, port })
    }
}

impl Drop for Dummyplug {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn request(text: &str) -> Value {
    json!({ "timestamp": chrono::Utc::now(), "user": "olive", "content": { "chat": text }, "metadata": {} })
}

/// The frame kinds of a turn, in order: the top-level key of each frame, or its `type`.
fn kinds(frames: &[Value]) -> Vec<String> {
    frames
        .iter()
        .map(|f| match f {
            Value::Object(m) => m
                .get("content")
                .and_then(|c| c.as_object())
                .and_then(|c| c.keys().next().cloned())
                .or_else(|| m.keys().next().cloned())
                .unwrap_or_default(),
            _ => String::new(),
        })
        // The title is made asynchronously, so it may land before or after the reply.
        .filter(|k| k != "session_title")
        .collect()
}

fn finished(frames: &[Value]) -> bool {
    frames.iter().any(|f| f.to_string().contains("execute_python"))
}

#[tokio::test(flavor = "multi_thread")]
async fn a_turn_looks_the_same_over_the_socket_and_the_dispatcher() {
    let Some(plug) = Dummyplug::start() else { return };
    let dir = tempfile::tempdir().unwrap();
    let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let mut config = VizierConfig::with_workspace(dir.path().to_path_buf());
    let http = config.channels.http.as_mut().unwrap();
    http.jwt_secret = "topic-secret".into();
    http.port = port as u32;
    http.host = Some("127.0.0.1".into());
    let runtime = VizierRuntime::start(RuntimeOptions { config }).await.unwrap();
    let storage = runtime.deps().storage.clone();
    let http_cfg = runtime.deps().config.channels.http.clone().unwrap();

    let role = storage.create_role("system", vec![], true).await.unwrap();
    let user = storage.create_user("olive", "", &role.role_id).await.unwrap();
    let jwt = AuthService::new(&http_cfg).generate_token(&user.user_id, "olive", vec![]).unwrap();
    tokio::spawn(vizier_http::serve(&runtime, http_cfg.clone()));
    let dispatcher = Dispatcher::new(&runtime, user.user_id.clone());

    // An agent on the dummyplug provider, created through the dispatcher.
    for (method, path, body) in [
        ("PUT", "/api/v1/providers/dummyplug", json!({ "base_url": format!("http://127.0.0.1:{}/v1", plug.port) })),
        ("POST", "/api/v1/agents", json!({ "agent_id": "plug", "name": "Plug", "provider": "dummyplug", "model": "dummyplug" })),
    ] {
        let res = dispatcher
            .dispatch(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await;
        assert!(res.status().is_success(), "{method} {path}: {}", res.status());
    }

    // Over the dispatcher.
    let (tx, rx) = flume::unbounded();
    let mut topic = dispatcher.open_topic("plug", "app", "t-desktop", tx).await.expect("opens");
    let outbound = topic.outbound().expect("outbound half");
    tokio::spawn(outbound.run());
    // A second connection on the same session receives the turn too.
    let (tx2, rx2) = flume::unbounded();
    let mut second = dispatcher.open_topic("plug", "app", "t-desktop", tx2).await.expect("opens");
    tokio::spawn(second.outbound().expect("outbound half").run());
    topic.handle_inbound(request("tools")).await;

    let mut desktop_frames = vec![];
    let mut other_frames = vec![];
    for _ in 0..600 {
        while let Ok(f) = rx.try_recv() {
            desktop_frames.push(f);
        }
        while let Ok(f) = rx2.try_recv() {
            other_frames.push(f);
        }
        if finished(&desktop_frames) && finished(&other_frames) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(finished(&desktop_frames), "no reply over the dispatcher: {desktop_frames:?}");
    assert!(finished(&other_frames), "the second connection missed the turn");
    assert_eq!(kinds(&desktop_frames), kinds(&other_frames));

    // Over the WebSocket, on a fresh topic.
    let url = format!("ws://127.0.0.1:{port}/api/v1/agents/plug/channel/app/topic/t-socket/chat?token={jwt}");
    let mut socket = None;
    for _ in 0..50 {
        if let Ok((s, _)) = tokio_tungstenite::connect_async(&url).await {
            socket = Some(s);
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let mut socket = socket.expect("websocket connects");
    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(request("tools").to_string().into()))
        .await
        .unwrap();
    let mut socket_frames = vec![];
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while !finished(&socket_frames) && tokio::time::Instant::now() < deadline {
        if let Ok(Some(Ok(tokio_tungstenite::tungstenite::Message::Text(t)))) =
            tokio::time::timeout(Duration::from_millis(200), socket.next()).await
            && let Ok(v) = serde_json::from_str::<Value>(&t)
        {
            socket_frames.push(v);
        }
    }
    assert!(finished(&socket_frames), "no reply over the socket: {socket_frames:?}");

    // The same kinds of frame, in the same order, over both.
    assert_eq!(kinds(&socket_frames), kinds(&desktop_frames));
}
