//! Spec 058 T045: the two read routes of a topic's detached scripts. The list shows the
//! caller's own conversation only; the detail carries code, calls and the report; anybody
//! else's script, or an agent the caller cannot see, is a 404.

use axum::{body::Body, http::Request};
use serde_json::json;
use vizier_core::{
    config::VizierConfig,
    runtime::{RuntimeOptions, VizierRuntime},
    schema::{AgentConfig, DetachedScript, DetachedScriptState, VizierChannelId, VizierSession},
    storage::{agent::AgentStorage, detached_script::DetachedScriptStorage, user::UserStorage},
};
use vizier_http::embedded::Dispatcher;

async fn get(dispatcher: &Dispatcher, uri: &str) -> (u16, serde_json::Value) {
    let res = dispatcher
        .dispatch(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await;
    let status = res.status().as_u16();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 22).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or_default())
}

fn script(id: &str, session: VizierSession) -> DetachedScript {
    DetachedScript {
        id: id.into(),
        agent_id: session.0.clone(),
        session,
        parent_id: None,
        depth: 0,
        intent: "check the pages".into(),
        state: DetachedScriptState::Running,
        timeout_secs: 600,
        code: "web_fetch(url=u)".into(),
        tool_calls: json!([]),
        created_at: vizier_core::time::now(),
        finished_at: None,
        outcome: None,
        delivered_at: None,
        cancelled_by: None,
    }
}

#[tokio::test]
async fn a_topics_scripts_are_listed_and_read_only_by_their_owner() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = VizierRuntime::start(RuntimeOptions {
        config: VizierConfig::with_workspace(dir.path().to_path_buf()),
        ..Default::default()
    })
    .await
    .unwrap();
    let storage = runtime.deps().storage.clone();
    let admin = storage.create_role("admin", vec![], true).await.unwrap();
    let none = storage.create_role("none", vec![], false).await.unwrap();
    let ana = storage.create_user("ana", "", &admin.role_id).await.unwrap();
    let bo = storage.create_user("bo", "", &admin.role_id).await.unwrap();
    let nobody = storage.create_user("nobody", "", &none.role_id).await.unwrap();
    let config: AgentConfig = serde_json::from_value(json!({
        "name": "A", "owner_id": ana.user_id, "system_prompt": null, "description": null,
        "provider": "dummyplug", "model": "dummyplug", "thinking_depth": 1,
        "tools": {"timeout": "30s"}, "silent_read_initiative_chance": 0.0,
        "prompt_timeout": "1m", "heartbeat_interval": "1m"
    }))
    .unwrap();
    storage.create_agent("agent-a", &config).await.unwrap();

    let anas = VizierSession(
        "agent-a".into(),
        VizierChannelId::from_web("ana", "webui"),
        Some("t1".into()),
    );
    storage.open_detached_script(script("d_running", anas.clone())).await.unwrap();
    storage.open_detached_script(script("d_finished", anas.clone())).await.unwrap();
    storage
        .close_detached_script(
            "d_finished",
            DetachedScriptState::Succeeded,
            json!({"ok": true, "result": 42, "stdout": "", "error": null}),
            json!([{"seq": 1, "name": "web_fetch", "arguments": {"url": "https://x"}, "ok": true, "duration_ms": 9}]),
            vizier_core::time::now(),
        )
        .await
        .unwrap();

    let list = "/api/v1/agents/agent-a/channel/webui/topic/t1/detached-scripts";
    let as_ana = Dispatcher::new(&runtime, ana.user_id.clone());
    let (status, body) = get(&as_ana, list).await;
    assert_eq!(status, 200, "{body}");
    let ids: Vec<&str> = body["data"].as_array().unwrap().iter().map(|s| s["id"].as_str().unwrap()).collect();
    assert_eq!(ids.len(), 2, "{body}");
    assert!(ids.contains(&"d_running") && ids.contains(&"d_finished"));

    let (status, body) = get(&as_ana, &format!("{list}/d_finished")).await;
    assert_eq!(status, 200, "{body}");
    let detail = &body["data"];
    assert_eq!(detail["state"], "succeeded");
    assert_eq!(detail["code"], "web_fetch(url=u)");
    assert_eq!(detail["tool_calls"][0]["name"], "web_fetch");
    assert_eq!(detail["report"]["result"], 42);
    assert_eq!(detail["tool_call_count"], 1);

    // Bo can see the agent, but the conversation is Ana's: his own topic t1 has nothing, and
    // her script's id does not exist for him.
    let as_bo = Dispatcher::new(&runtime, bo.user_id.clone());
    let (status, body) = get(&as_bo, list).await;
    assert_eq!(status, 200);
    assert!(body["data"].as_array().unwrap().is_empty(), "{body}");
    assert_eq!(get(&as_bo, &format!("{list}/d_finished")).await.0, 404);

    // Somebody who cannot see the agent gets a 404 for both routes.
    let as_nobody = Dispatcher::new(&runtime, nobody.user_id.clone());
    assert_eq!(get(&as_nobody, list).await.0, 404);
    assert_eq!(get(&as_nobody, &format!("{list}/d_running")).await.0, 404);
}
