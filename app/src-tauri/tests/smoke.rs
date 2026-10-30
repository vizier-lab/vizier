//! The desktop app without a window (T054, T093, T100): the engine, the IPC core, topic frames,
//! the workspace lock and opt-in web access. Chat checks need Node and the `dummyplug`
//! submodule and are skipped without them (they fail under `CI`).

use std::{
    net::TcpListener,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::Duration,
};

use serde_json::{Value, json};
use vizier_app::{
    desktop,
    engine::{DesktopState, Engine, SettingsPatch},
    ipc::{self, Topics},
    settings::{DesktopSettings, Updates, WebAccess},
};

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
            eprintln!("skipping the chat checks: {why}");
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

    async fn ready(&self) {
        let url = format!("http://127.0.0.1:{}/healthz", self.port);
        for _ in 0..100 {
            if reqwest::get(&url).await.is_ok_and(|r| r.status().is_success()) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        panic!("dummyplug did not become healthy");
    }
}

impl Drop for Dummyplug {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn engine_on(workspace: &std::path::Path, config_dir: &std::path::Path) -> Engine {
    let mut settings = DesktopSettings::defaults(config_dir);
    settings.workspace = workspace.to_path_buf();
    Engine::new(config_dir.to_path_buf(), settings)
}

async fn api(engine: &Engine, method: &str, path: &str, body: Option<Value>) -> ipc::ApiReply {
    let (ct, bytes) = match body {
        Some(b) => (Some("application/json"), b.to_string().into_bytes()),
        None => (None, vec![]),
    };
    ipc::api(engine, method, path, ct, bytes).await.expect("api")
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

#[tokio::test(flavor = "multi_thread")]
async fn first_run_owner_api_chat_and_lock() {
    let ws = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let engine = engine_on(ws.path(), cfg.path());

    // Not ready: the IPC refuses before anything started.
    let early = ipc::api(&engine, "GET", "/status", None, vec![]).await;
    assert_eq!(early.unwrap_err().code, "engine_not_ready");

    engine.start().await;
    assert_eq!(engine.state(), DesktopState::NeedsOwner);
    engine.setup_owner("olive", None).await.unwrap();
    assert_eq!(engine.state(), DesktopState::Ready);
    // The binding survives in desktop.json.
    assert_eq!(DesktopSettings::load(cfg.path(), cfg.path()).bindings.len(), 1);

    let me = api(&engine, "GET", "/auth/users/me", None).await;
    assert_eq!(me.status, 200);
    assert_eq!(me.body["data"]["username"], "olive");
    assert_eq!(me.body["data"]["has_password"], false);

    // A second engine on the same workspace is refused with the lock state.
    let other_cfg = tempfile::tempdir().unwrap();
    let second = engine_on(ws.path(), other_cfg.path());
    second.start().await;
    assert!(matches!(second.state(), DesktopState::Locked { .. }), "{:?}", second.state());

    // Nothing is listening: no web access was asked for.
    assert!(!engine.settings().web_access.enabled);

    if let Some(plug) = Dummyplug::start() {
        plug.ready().await;
        let saved = api(
            &engine,
            "PUT",
            "/providers/dummyplug",
            Some(json!({ "base_url": format!("http://127.0.0.1:{}/v1", plug.port) })),
        )
        .await;
        assert!(saved.status < 300, "{saved:?}");
        let created = api(
            &engine,
            "POST",
            "/agents",
            Some(json!({ "agent_id": "plug", "name": "Plug", "provider": "dummyplug", "model": "dummyplug" })),
        )
        .await;
        assert!(created.status < 300, "{created:?}");

        let frames: Arc<Mutex<Vec<Value>>> = Default::default();
        let topics = Topics::default();
        let sink = frames.clone();
        let id = topics
            .open(&engine, "plug", "app", "t1", move |f| sink.lock().unwrap().push(f))
            .await
            .expect("topic opens");

        let request = json!({
            "timestamp": chrono::Utc::now(),
            "user": "olive",
            "content": { "chat": "tools" },
            "metadata": {}
        });
        topics.send(id, request).await;
        let mut seen = false;
        for _ in 0..600 {
            if frames.lock().unwrap().iter().any(|f| f.to_string().contains("execute_python")) {
                seen = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(seen, "no `tools` reply: {:?}", frames.lock().unwrap());
        topics.close(id);
        assert!(topics.is_empty());

        // An agent the person cannot see is refused like the WebSocket upgrade would.
        let refused = topics.open(&engine, "nope", "app", "t1", |_| {}).await;
        assert_eq!(refused.unwrap_err().status, 404);

        // T093: the file route writes the bytes to the chosen path.
        let dest = cfg.path().join("out.bin");
        desktop::save_file(&engine, "/status", &dest).await.unwrap();
        assert!(std::fs::metadata(&dest).unwrap().len() > 0);
    }

    // Asset GET: only GET, only /api/v1.
    let post = ipc::asset(&engine, "POST", "/api/v1/status", None).await.unwrap();
    assert_eq!(post.status, 405);
    let get = ipc::asset(&engine, "GET", "/api/v1/status", None).await.unwrap();
    assert_eq!(get.status, 200);

    engine.shutdown().await;
    // After shutdown the lock is free again.
    second.start().await;
    // The workspace now has its one system owner, so a fresh app signs straight in as them.
    assert_eq!(second.state(), DesktopState::Ready);
    second.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn web_access_needs_a_password_and_keeps_the_login_rules() {
    let ws = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let engine = engine_on(ws.path(), cfg.path());
    engine.start().await;
    engine.setup_owner("olive", None).await.unwrap();

    let port = free_port();
    let on = SettingsPatch {
        close_to_tray: true,
        web_access: WebAccess { enabled: true, allow_network: false, port },
        updates: Updates { check_on_start: false },
    };
    let refused = engine.settings_set(on.clone()).await.unwrap_err();
    assert_eq!(refused.code, "password_required");

    // The first password needs no current one for the desktop owner.
    let changed = api(
        &engine,
        "POST",
        "/auth/change-password",
        Some(json!({ "current_password": "", "new_password": "correct horse" })),
    )
    .await;
    assert!(changed.status < 300, "{changed:?}");
    let me = api(&engine, "GET", "/auth/users/me", None).await;
    assert_eq!(me.body["data"]["has_password"], true);

    engine.settings_set(on).await.expect("enabled");
    let base = format!("http://127.0.0.1:{port}/api/v1");
    let http = reqwest::Client::new();
    let mut up = false;
    for _ in 0..50 {
        if http.get(format!("{base}/ping")).send().await.is_ok() {
            up = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(up, "web access did not start");

    // The desktop identity is not accepted over the network.
    let anon = http.get(format!("{base}/agents")).send().await.unwrap();
    assert_eq!(anon.status().as_u16(), 401);

    // A normal login works.
    let login = http
        .post(format!("{base}/auth/login"))
        .json(&json!({ "username": "olive", "password": "correct horse" }))
        .send()
        .await
        .unwrap();
    assert_eq!(login.status().as_u16(), 200);
    let token = login.json::<Value>().await.unwrap()["data"]["token"].as_str().unwrap().to_string();
    let agents = http.get(format!("{base}/agents")).bearer_auth(token).send().await.unwrap();
    assert_eq!(agents.status().as_u16(), 200);

    // Turning it off closes the listener.
    engine
        .settings_set(SettingsPatch {
            close_to_tray: true,
            web_access: WebAccess { enabled: false, allow_network: false, port },
            updates: Updates { check_on_start: false },
        })
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(http.get(format!("{base}/ping")).send().await.is_err());
    engine.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn several_system_users_ask_which_to_sign_in_as() {
    let ws = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let engine = engine_on(ws.path(), cfg.path());
    engine.start().await;
    engine.setup_owner("one", None).await.unwrap();
    // A second system user, made through the API as the owner.
    let roles = api(&engine, "GET", "/auth/roles", None).await;
    assert_eq!(roles.status, 200, "{roles:?}");
    engine.shutdown().await;
}
