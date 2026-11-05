//! The inbox stream (`specs/051-realtime-notifications`): `hello` first, a `notification` with
//! counts, read state across a person's sockets, nothing for someone who cannot see the agent,
//! and a socket that stays open past the chat socket's idle timeout.
//!
//! Needs no provider: agents are stored directly and notifications come from the same service
//! the engine uses, so the test runs everywhere.

use std::{net::TcpListener, time::Duration};

use futures::StreamExt;
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::Message;
use vizier_core::{
    auth::AuthMethod,
    config::VizierConfig,
    runtime::{RuntimeOptions, VizierRuntime},
    schema::{AgentConfig, TaskRunState},
    service::{Services, notifications},
    storage::{agent::AgentStorage, user::UserStorage},
};
use vizier_http::auth::AuthService;

type Socket = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

struct Fixture {
    runtime: VizierRuntime,
    port: u16,
    http: vizier_core::config::HTTPChannelConfig,
    _dir: tempfile::TempDir,
}

fn agent(owner: Option<&str>) -> AgentConfig {
    serde_json::from_value(json!({
        "name": "Alpha", "owner_id": owner, "system_prompt": null, "description": null,
        "provider": "dummyplug", "model": "dummyplug", "thinking_depth": 1,
        "tools": {"timeout": "30s"}, "silent_read_initiative_chance": 0.0,
        "prompt_timeout": "1m", "heartbeat_interval": "1m"
    }))
    .unwrap()
}

async fn start(idle_timeout_secs: u64) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let mut config = VizierConfig::with_workspace(dir.path().to_path_buf());
    let http = config.channels.http.as_mut().unwrap();
    http.jwt_secret = "inbox-secret".into();
    http.port = port as u32;
    http.host = Some("127.0.0.1".into());
    http.ws_idle_timeout_secs = idle_timeout_secs;
    let runtime = VizierRuntime::start(RuntimeOptions { config, ..Default::default() })
        .await
        .unwrap();
    let http = runtime.deps().config.channels.http.clone().unwrap();
    tokio::spawn(vizier_http::serve(&runtime, http.clone()));
    Fixture { runtime, port, http, _dir: dir }
}

impl Fixture {
    /// A person with a role of their own; returns their id and a token.
    async fn person(&self, name: &str, system: bool) -> (String, String) {
        let storage = &self.runtime.deps().storage;
        let role = storage.create_role(&format!("role-{name}"), vec![], system).await.unwrap();
        let user = storage.create_user(name, "", &role.role_id).await.unwrap();
        let token = AuthService::new(&self.http)
            .generate_token(&user.user_id, name, vec![])
            .unwrap();
        (user.user_id, token)
    }

    async fn connect(&self, token: &str) -> Socket {
        let url = format!("ws://127.0.0.1:{}/api/v1/inbox/stream?token={token}", self.port);
        for _ in 0..50 {
            if let Ok((socket, _)) = tokio_tungstenite::connect_async(&url).await {
                return socket;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        panic!("the inbox stream never accepted a connection");
    }

    async fn run_ended(&self, agent_id: &str, run: i64) {
        self.runtime
            .deps()
            .notifications
            .task_run_ended(agent_id, "brief", "Brief", run, TaskRunState::Answered, chrono::Utc::now())
            .await;
    }

    fn services(&self) -> Services {
        Services::new(self.runtime.deps().clone())
    }
}

/// The next JSON frame within `wait`, skipping pings; `None` on timeout, and `Err` on close.
async fn next(socket: &mut Socket, wait: Duration) -> Result<Option<Value>, ()> {
    let deadline = tokio::time::Instant::now() + wait;
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(left, socket.next()).await {
            Err(_) => return Ok(None),
            Ok(None) | Ok(Some(Err(_))) => return Err(()),
            Ok(Some(Ok(Message::Text(text)))) => return Ok(Some(serde_json::from_str(&text).unwrap())),
            Ok(Some(Ok(Message::Close(_)))) => return Err(()),
            Ok(Some(Ok(_))) => continue,
        }
    }
}

async fn frame(socket: &mut Socket) -> Value {
    next(socket, Duration::from_secs(5))
        .await
        .expect("the socket stayed open")
        .expect("a frame in time")
}

#[tokio::test(flavor = "multi_thread")]
async fn hello_comes_first_and_a_new_row_follows_with_counts() {
    let fx = start(300).await;
    let (_, token) = fx.person("olive", true).await;
    fx.runtime.deps().storage.create_agent("alpha", &agent(None)).await.unwrap();

    let mut socket = fx.connect(&token).await;
    let hello = frame(&mut socket).await;
    assert_eq!(hello["type"], "hello");
    assert_eq!(hello["counts"], json!({ "all": 0, "unread": 0, "read": 0 }));
    assert_eq!(hello["latest_id"], 0);
    assert_eq!(hello["setups"], json!([]));

    fx.run_ended("alpha", 1).await;
    let pushed = frame(&mut socket).await;
    assert_eq!(pushed["type"], "notification");
    assert_eq!(pushed["item"]["agent_id"], "alpha");
    assert_eq!(pushed["item"]["category"], "task_run");
    assert_eq!(pushed["item"]["read"], false);
    assert_eq!(pushed["counts"]["unread"], 1);
    assert!(
        pushed["item"].get("source_session").is_none(),
        "the source session is internal and never sent"
    );

    // A reconnect reports where the inbox stands now.
    let mut again = fx.connect(&token).await;
    let hello = frame(&mut again).await;
    assert_eq!(hello["counts"]["unread"], 1);
    assert_eq!(hello["latest_id"], pushed["item"]["id"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn read_state_reaches_every_socket_of_that_person_and_nobody_else() {
    let fx = start(300).await;
    let (olive_id, olive) = fx.person("olive", true).await;
    let (_, pat) = fx.person("pat", true).await;
    fx.runtime.deps().storage.create_agent("alpha", &agent(None)).await.unwrap();

    let mut first = fx.connect(&olive).await;
    let mut second = fx.connect(&olive).await;
    let mut other = fx.connect(&pat).await;
    for socket in [&mut first, &mut second, &mut other] {
        assert_eq!(frame(socket).await["type"], "hello");
    }
    fx.run_ended("alpha", 1).await;
    let mut id = Value::Null;
    for socket in [&mut first, &mut second, &mut other] {
        let pushed = frame(socket).await;
        assert_eq!(pushed["type"], "notification");
        id = pushed["item"]["id"].clone();
    }

    let services = fx.services();
    let principal = services.principal_for(&olive_id, AuthMethod::Jwt).await.unwrap();
    notifications::set_read(&services, &principal, id.as_i64().unwrap(), true).await.unwrap();

    for socket in [&mut first, &mut second] {
        let read = frame(socket).await;
        assert_eq!(read["type"], "read");
        assert_eq!(read["id"], id);
        assert_eq!(read["read"], true);
        assert_eq!(read["counts"]["unread"], 0);
    }
    assert_eq!(
        next(&mut other, Duration::from_millis(700)).await,
        Ok(None),
        "another person was told about olive's read state"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn someone_who_cannot_view_the_agent_is_told_nothing_about_it() {
    let fx = start(300).await;
    let (olive_id, _) = fx.person("olive", true).await;
    let (sam_id, sam) = fx.person("sam", false).await;
    // Alpha is olive's; sam has no access to it. Beta is sam's own.
    fx.runtime.deps().storage.create_agent("alpha", &agent(Some(&olive_id))).await.unwrap();
    fx.runtime.deps().storage.create_agent("beta", &agent(Some(&sam_id))).await.unwrap();

    let mut socket = fx.connect(&sam).await;
    assert_eq!(frame(&mut socket).await["type"], "hello");

    fx.run_ended("alpha", 1).await;
    assert_eq!(
        next(&mut socket, Duration::from_millis(700)).await,
        Ok(None),
        "a row for an agent sam cannot view reached sam"
    );

    // The stream is still alive for what sam may see.
    fx.run_ended("beta", 2).await;
    let pushed = frame(&mut socket).await;
    assert_eq!(pushed["type"], "notification");
    assert_eq!(pushed["item"]["agent_id"], "beta");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_quiet_socket_stays_open_past_the_chat_idle_timeout() {
    // The chat socket closes after this long without a client frame; the inbox never sends one.
    let fx = start(1).await;
    let (_, token) = fx.person("olive", true).await;
    fx.runtime.deps().storage.create_agent("alpha", &agent(None)).await.unwrap();

    let mut socket = fx.connect(&token).await;
    assert_eq!(frame(&mut socket).await["type"], "hello");
    assert_eq!(
        next(&mut socket, Duration::from_secs(3)).await,
        Ok(None),
        "the socket closed while quiet"
    );

    fx.run_ended("alpha", 1).await;
    assert_eq!(frame(&mut socket).await["type"], "notification");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_person_who_no_longer_exists_loses_the_stream_at_the_next_event() {
    let fx = start(300).await;
    let (_, token) = fx.person("olive", true).await;
    let (pat_id, pat) = fx.person("pat", true).await;
    fx.runtime.deps().storage.create_agent("alpha", &agent(None)).await.unwrap();

    let mut socket = fx.connect(&pat).await;
    assert_eq!(frame(&mut socket).await["type"], "hello");
    fx.runtime.deps().storage.delete_user(&pat_id).await.unwrap();
    fx.run_ended("alpha", 1).await;

    assert_eq!(
        next(&mut socket, Duration::from_secs(5)).await,
        Err(()),
        "the stream stayed open for a deleted person"
    );

    // Everyone else is unaffected.
    let mut alive = fx.connect(&token).await;
    assert_eq!(frame(&mut alive).await["type"], "hello");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_missing_token_is_refused() {
    let fx = start(300).await;
    let (_, token) = fx.person("olive", true).await;
    // Once the server is demonstrably up, a connection without credentials is refused.
    let _ready = fx.connect(&token).await;
    let url = format!("ws://127.0.0.1:{}/api/v1/inbox/stream", fx.port);
    assert!(
        tokio_tungstenite::connect_async(&url).await.is_err(),
        "an unauthenticated socket was accepted"
    );
    let bad = format!("{url}?token=not-a-token");
    assert!(
        tokio_tungstenite::connect_async(&bad).await.is_err(),
        "a bad token was accepted"
    );
}
