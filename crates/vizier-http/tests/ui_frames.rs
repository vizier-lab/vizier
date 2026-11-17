//! Page frames end to end through the real routers (`specs/063-html-generative-ui` research R7a):
//! a person mints a ticket for a page in their own conversation, the ticket opens a frame
//! document with its own sandboxing policy and the kit's runtime inlined, and nothing else opens
//! it. Set `UI_FRAME_DUMP=<file>` to write the served document out, for a browser check.

use axum::{body::Body, http::Request};
use serde_json::json;
use tower::ServiceExt;
use vizier_core::{
    config::VizierConfig,
    runtime::{RuntimeOptions, VizierRuntime},
    schema::{AgentConfig, VizierChannelId, VizierSession},
    storage::{agent::AgentStorage, ui_page::UiPageStorage, user::UserStorage},
};
use vizier_http::embedded::Dispatcher;

struct Reply {
    status: u16,
    headers: axum::http::HeaderMap,
    body: String,
}

async fn reply(res: axum::http::Response<Body>) -> Reply {
    let status = res.status().as_u16();
    let headers = res.headers().clone();
    let bytes = axum::body::to_bytes(res.into_body(), 8 << 20).await.unwrap();
    Reply { status, headers, body: String::from_utf8_lossy(&bytes).into_owned() }
}

fn get(uri: &str) -> Request<Body> {
    Request::builder().uri(uri).body(Body::empty()).unwrap()
}

/// As a person, through the desktop dispatcher.
async fn via(d: &Dispatcher, uri: &str) -> Reply {
    reply(d.dispatch(get(uri)).await).await
}

/// With no credential, through the network router.
async fn public(router: axum::Router, uri: &str) -> Reply {
    reply(router.oneshot(get(uri)).await.unwrap()).await
}

fn agent(owner: &str) -> AgentConfig {
    serde_json::from_value(json!({
        "name": "A", "owner_id": owner, "system_prompt": null, "description": null,
        "provider": "dummyplug", "model": "dummyplug", "thinking_depth": 1,
        "tools": {"timeout": "30s"}, "silent_read_initiative_chance": 0.0,
        "prompt_timeout": "1m", "heartbeat_interval": "1m"
    }))
    .unwrap()
}

#[tokio::test]
async fn a_ticket_opens_a_sandboxed_frame_with_the_runtime() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = VizierConfig::with_workspace(dir.path().to_path_buf());
    config.channels.http.as_mut().unwrap().jwt_secret = "test-secret".into();
    let runtime = VizierRuntime::start(RuntimeOptions { config, ..Default::default() }).await.unwrap();
    let deps = runtime.deps();
    let role = deps.storage.create_role("owner", vec![], true).await.unwrap();
    let owner = deps.storage.create_user("owner", "", &role.role_id).await.unwrap();
    let other = deps.storage.create_user("other", "", &role.role_id).await.unwrap();
    deps.storage.create_agent("agent-a", &agent(&owner.user_id)).await.unwrap();

    let session = VizierSession("agent-a".into(), VizierChannelId::from_web("owner", "vizier-webui"), Some("t1".into()));
    let page = deps
        .ui_pages
        .create(&session, "Hello", "<h1 id=hi>Hi</h1><script>document.title = typeof vizier</script>")
        .await
        .unwrap();
    deps.storage.save_ui_page_state(&page.id, &json!({ "n": 1 }), None).await.unwrap();

    let as_owner = Dispatcher::new(&runtime, owner.user_id.clone());
    let as_other = Dispatcher::new(&runtime, other.user_id.clone());
    let base = format!("/api/v1/agents/agent-a/channel/vizier-webui/topic/t1/pages/{}", page.id);

    // The page and its source, to its owner only.
    let read = via(&as_owner, &base).await;
    assert_eq!(read.status, 200, "{}", read.body);
    let source = via(&as_owner, &format!("{base}/versions/1")).await;
    assert_eq!(source.status, 200);
    assert!(source.headers["content-type"].to_str().unwrap().starts_with("text/plain"), "the source never runs as a WebUI document");
    assert_eq!(via(&as_other, &base).await.status, 404, "another person's conversation");

    // A ticket, then the frame it opens.
    let ticket = via(&as_owner, &format!("{base}/versions/1/frame")).await;
    assert_eq!(ticket.status, 200, "{}", ticket.body);
    let ticket: serde_json::Value = serde_json::from_str(&ticket.body).unwrap();
    let path = ticket["data"]["path"].as_str().unwrap().to_string();
    assert!(path.starts_with("/ui-frames/") && !path.contains("token"), "{path}");
    assert_eq!(via(&as_other, &format!("{base}/versions/1/frame")).await.status, 404);
    assert_eq!(via(&as_owner, &format!("{base}/versions/2/frame")).await.status, 404, "no version 2");

    // The frame route is public: the network router, with no credential at all, serves it.
    let http_cfg = deps.config.channels.http.clone().unwrap();
    let network = vizier_http::router(&runtime, &http_cfg);
    let frame = public(network, &format!("/api/v1{path}")).await;
    assert_eq!(frame.status, 200, "{}", frame.body);
    let csp = frame.headers["content-security-policy"].to_str().unwrap();
    assert!(csp.contains("sandbox allow-scripts") && csp.contains("connect-src 'none'"), "{csp}");
    assert_eq!(frame.headers["cache-control"], "no-store");
    assert!(frame.body.contains(&format!("\"pageId\":\"{}\"", page.id)), "boot data");
    assert!(frame.body.contains("\"state\":{\"n\":1}"), "the saved state is in the boot data");
    // CI's Rust jobs build without the WebUI, so there is no kit to inline; with one (a local
    // build, a release), its stylesheet and runtime are in the frame.
    if frame.body.contains("<style data-vz-kit>") {
        assert!(frame.body.contains("<script data-vz-runtime>"), "the runtime is inlined with the kit");
    }
    assert!(frame.body.ends_with("<h1 id=hi>Hi</h1><script>document.title = typeof vizier</script></body></html>"));

    // A forged ticket opens nothing.
    let network = vizier_http::router(&runtime, &http_cfg);
    let forged = public(network, &format!("/api/v1{path}x")).await;
    assert_eq!(forged.status, 404);

    if let Ok(dump) = std::env::var("UI_FRAME_DUMP") {
        std::fs::write(dump, &frame.body).unwrap();
    }
}
