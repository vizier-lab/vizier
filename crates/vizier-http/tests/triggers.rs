//! Triggers over the real router (`specs/071-triggers/contracts/http-api.md`): permissions on the
//! management routes, the secret shown once, and the public webhook address.
//!
//! `alice` owns `agent-a` and may manage its triggers and open public webhooks; `bob` may only see
//! the agent; `root` is the system role. Requests go through `vizier_http::router`.

use axum::{Router, body::Body, http::Request};
use serde_json::{Value, json};
use tower::ServiceExt;
use vizier_core::{
    config::VizierConfig,
    runtime::{RuntimeOptions, VizierRuntime},
    schema::AgentConfig,
    storage::{agent::AgentStorage, trigger::TriggerStorage, user::UserStorage},
};
use vizier_http::auth::AuthService;

struct Env {
    router: Router,
    runtime: VizierRuntime,
    alice: String,
    bob: String,
    _dir: tempfile::TempDir,
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

async fn env() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let mut config = VizierConfig::with_workspace(dir.path().to_path_buf());
    config.channels.http.as_mut().unwrap().jwt_secret = "triggers-secret".into();
    config.catalogue.refresh = false;
    let runtime = VizierRuntime::start(RuntimeOptions { config, ..Default::default() }).await.unwrap();
    let storage = runtime.deps().storage.clone();
    let http_cfg = runtime.deps().config.channels.http.clone().unwrap();
    let auth = AuthService::new(&http_cfg);
    let manager = storage
        .create_role(
            "manager",
            vec![
                "owned_agents:view".into(),
                "owned_agents:manage_triggers".into(),
                "owned_agents:public_webhooks".into(),
            ],
            false,
        )
        .await
        .unwrap();
    let viewer = storage.create_role("viewer", vec!["all_agents:view".into()], false).await.unwrap();
    let alice = storage.create_user("alice", "", &manager.role_id).await.unwrap();
    let bob = storage.create_user("bob", "", &viewer.role_id).await.unwrap();
    storage.create_agent("agent-a", &agent(&alice.user_id)).await.unwrap();
    let token = |user: &vizier_core::storage::user::User, perms: Vec<String>| {
        auth.generate_token(&user.user_id, &user.username, perms).unwrap()
    };
    Env {
        router: vizier_http::router(&runtime, &http_cfg),
        alice: token(&alice, manager.permissions.clone()),
        bob: token(&bob, viewer.permissions.clone()),
        runtime,
        _dir: dir,
    }
}

async fn call(router: &Router, method: &str, path: &str, auth: Option<&str>, body: Option<Value>) -> (u16, Value) {
    let mut req = Request::builder().method(method).uri(format!("/api/v1{path}"));
    if let Some(auth) = auth {
        req = req.header("authorization", auth.to_string());
    }
    let body = match body {
        Some(b) => {
            req = req.header("content-type", "application/json");
            Body::from(b.to_string())
        }
        None => Body::empty(),
    };
    let res = router.clone().oneshot(req.body(body).unwrap()).await.unwrap();
    let status = res.status().as_u16();
    let bytes = axum::body::to_bytes(res.into_body(), 4 << 20).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

fn webhook_body() -> Value {
    json!({
        "slug": "ci", "kind": "webhook", "title": "CI", "instruction": "Tell me about failures",
        "settings": { "filter": ["body.ref=refs/heads/master"] },
    })
}

#[tokio::test]
async fn a_webhook_is_created_by_a_permitted_person_and_its_secret_is_shown_once() {
    let env = env().await;
    let bob = format!("Bearer {}", env.bob);
    let alice = format!("Bearer {}", env.alice);

    let (status, _) = call(&env.router, "POST", "/agents/agent-a/triggers", Some(&bob), Some(webhook_body())).await;
    assert_eq!(status, 403, "seeing the agent grants no trigger permission");

    let (status, created) = call(&env.router, "POST", "/agents/agent-a/triggers", Some(&alice), Some(webhook_body())).await;
    assert_eq!(status, 201, "{created}");
    let secret = created["data"]["secret"].as_str().expect("the secret, once").to_string();
    assert_eq!(created["data"]["trigger"]["tool_scope"], json!(["memory_read"]));
    assert_eq!(created["data"]["trigger"]["allows_messaging"], json!(false));
    let address = created["data"]["trigger"]["address_path"].as_str().unwrap().to_string();

    let (status, read) = call(&env.router, "GET", "/agents/agent-a/triggers/ci", Some(&alice), None).await;
    assert_eq!(status, 200);
    assert!(!read.to_string().contains(&secret), "a read never carries the secret");
    let (status, _) = call(&env.router, "GET", "/agents/agent-a/triggers/ci", Some(&bob), None).await;
    assert_eq!(status, 403);

    // The address: the right secret is accepted, anything else is the same 404.
    let id = address.trim_start_matches("/api/v1/hooks/");
    let push = |auth: Option<String>, body: Value| {
        let router = env.router.clone();
        let path = format!("/hooks/{id}");
        async move { call(&router, "POST", &path, auth.as_deref(), Some(body)).await }
    };
    let (status, accepted) = push(Some(format!("Bearer {secret}")), json!({ "ref": "refs/heads/master" })).await;
    assert_eq!(status, 202, "{accepted}");
    assert_eq!(accepted["data"]["outcome"], json!("accepted"));
    let (status, ignored) = push(Some(format!("Bearer {secret}")), json!({ "ref": "refs/heads/x" })).await;
    assert_eq!((status, ignored["data"]["outcome"].clone()), (202, json!("ignored")));
    let (wrong, wrong_body) = push(Some("Bearer nope".into()), json!({})).await;
    let (unknown, unknown_body) = call(&env.router, "POST", "/hooks/trg_nothere", None, Some(json!({}))).await;
    assert_eq!((wrong, &wrong_body), (unknown, &unknown_body), "a wrong secret looks like an unknown trigger");
    assert_eq!(wrong, 404);

    // The secret may also ride in the address.
    let (status, _) = call(&env.router, "POST", &format!("/hooks/{id}/{secret}"), None, Some(json!({ "ref": "refs/heads/master" }))).await;
    assert_eq!(status, 202);

    let trigger = env.runtime.deps().storage.get_trigger("agent-a", "ci").await.unwrap().unwrap();
    let events = env.runtime.deps().storage.list_trigger_events(&trigger.id, None, 10).await.unwrap();
    assert_eq!(events.len(), 3);
}

#[tokio::test]
async fn a_deactivated_trigger_answers_like_an_unknown_one() {
    let env = env().await;
    let alice = format!("Bearer {}", env.alice);
    let (_, created) = call(&env.router, "POST", "/agents/agent-a/triggers", Some(&alice), Some(webhook_body())).await;
    let secret = created["data"]["secret"].as_str().unwrap().to_string();
    let id = created["data"]["trigger"]["id"].as_str().unwrap().to_string();
    let (status, _) = call(&env.router, "PUT", "/agents/agent-a/triggers/ci", Some(&alice), Some(json!({ "is_active": false }))).await;
    assert_eq!(status, 200);
    let (status, _) = call(&env.router, "POST", &format!("/hooks/{id}"), Some(&format!("Bearer {secret}")), Some(json!({}))).await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn the_twenty_first_trigger_is_refused_and_settings_are_validated() {
    let env = env().await;
    let alice = format!("Bearer {}", env.alice);
    let mut reserved = webhook_body();
    reserved["slug"] = json!("new");
    let (status, _) = call(&env.router, "POST", "/agents/agent-a/triggers", Some(&alice), Some(reserved)).await;
    assert_eq!(status, 400, "`new` is the create page's address");
    let mut bad = webhook_body();
    bad["settings"]["filter"] = json!(["nonsense"]);
    let (status, _) = call(&env.router, "POST", "/agents/agent-a/triggers", Some(&alice), Some(bad)).await;
    assert_eq!(status, 400);
    for n in 0..20 {
        let mut body = webhook_body();
        body["slug"] = json!(format!("t{n}"));
        let (status, out) = call(&env.router, "POST", "/agents/agent-a/triggers", Some(&alice), Some(body)).await;
        assert_eq!(status, 201, "{out}");
    }
    let mut body = webhook_body();
    body["slug"] = json!("one-too-many");
    let (status, _) = call(&env.router, "POST", "/agents/agent-a/triggers", Some(&alice), Some(body)).await;
    assert_eq!(status, 409);
}

/// FR-022: a body too large to read says so only to the trigger's sender.
#[tokio::test]
async fn an_oversized_body_is_refused_as_too_large_only_after_the_secret() {
    let env = env().await;
    let alice = format!("Bearer {}", env.alice);
    let (_, created) = call(&env.router, "POST", "/agents/agent-a/triggers", Some(&alice), Some(webhook_body())).await;
    let secret = created["data"]["secret"].as_str().unwrap().to_string();
    let id = created["data"]["trigger"]["id"].as_str().unwrap().to_string();
    for size in [300 * 1024, 600 * 1024] {
        let body = json!({ "text": "x".repeat(size) });
        let path = format!("/hooks/{id}");
        let (wrong, _) = call(&env.router, "POST", &path, Some("Bearer nope"), Some(body.clone())).await;
        assert_eq!(wrong, 404, "{size}");
        let (right, _) = call(&env.router, "POST", &path, Some(&format!("Bearer {secret}")), Some(body)).await;
        assert_eq!(right, 413, "{size}");
    }
}

/// FR-042 and FR-046: triage is saved only with a model, a destination only when it is the
/// person's own conversation, and the events are readable by those who may view.
#[tokio::test]
async fn triage_needs_a_model_and_a_destination_must_be_the_persons_own() {
    use vizier_core::{
        schema::{VizierChannelId, VizierSessionDetail},
        storage::session::SessionStorage,
    };
    let env = env().await;
    let alice = format!("Bearer {}", env.alice);
    let bob = format!("Bearer {}", env.bob);
    let mut body = webhook_body();
    body["triage"] = json!({ "criteria": "only master", "calls_per_hour": 10 });
    let (status, out) = call(&env.router, "POST", "/agents/agent-a/triggers", Some(&alice), Some(body)).await;
    assert_eq!(status, 400, "{out}");

    let storage = env.runtime.deps().storage.clone();
    for (user, topic) in [("alice", "mine"), ("bob", "theirs")] {
        storage
            .save_session_detail(VizierSessionDetail {
                agent_id: "agent-a".into(),
                channel: VizierChannelId::HTTP(user.into(), vizier_core::agents::tools::webui_channel().into()),
                topic: Some(topic.into()),
                title: "T".into(),
                is_thinking: false,
            })
            .await
            .unwrap();
    }
    let mut body = webhook_body();
    body["destination"] = json!("webui:theirs");
    let (status, out) = call(&env.router, "POST", "/agents/agent-a/triggers", Some(&alice), Some(body.clone())).await;
    assert_eq!(status, 400, "{out}");
    body["destination"] = json!("webui:mine");
    let (status, created) = call(&env.router, "POST", "/agents/agent-a/triggers", Some(&alice), Some(body)).await;
    assert_eq!(status, 201, "{created}");

    let update = json!({ "triage": { "criteria": "only master", "calls_per_hour": 10 } });
    let (status, _) = call(&env.router, "PUT", "/agents/agent-a/triggers/ci", Some(&alice), Some(update)).await;
    assert_eq!(status, 400, "nor can an edit add triage without a model");

    let secret = created["data"]["secret"].as_str().unwrap();
    let id = created["data"]["trigger"]["id"].as_str().unwrap();
    call(&env.router, "POST", &format!("/hooks/{id}"), Some(&format!("Bearer {secret}")), Some(json!({ "ref": "x" }))).await;
    let path = "/agents/agent-a/triggers/ci/events";
    let (status, events) = call(&env.router, "GET", path, Some(&alice), None).await;
    assert_eq!(status, 200, "{events}");
    assert!(events.to_string().contains("ignored"), "{events}");
    let (status, _) = call(&env.router, "GET", path, Some(&bob), None).await;
    assert_eq!(status, 403);
}

/// What the Triggers page reads: each trigger's latest run and runs this hour, and the agent's
/// triggers in numbers, for those who may view them only.
#[tokio::test]
async fn the_list_says_what_ran_lately_and_what_waits_for_a_person() {
    use vizier_core::schema::{EventStatus, WhenFull};
    use vizier_core::storage::trigger::{MonitorRecord, NewEvent};
    let env = env().await;
    let alice = format!("Bearer {}", env.alice);
    let bob = format!("Bearer {}", env.bob);
    let summary = "/agents/agent-a/trigger-summary";
    let (status, empty) = call(&env.router, "GET", summary, Some(&alice), None).await;
    assert_eq!(status, 200, "{empty}");
    assert_eq!(empty["data"], json!({ "total": 0, "active": 0, "needs_person": 0 }));
    let (status, _) = call(&env.router, "GET", summary, Some(&bob), None).await;
    assert_eq!(status, 403);

    let (_, created) = call(&env.router, "POST", "/agents/agent-a/triggers", Some(&alice), Some(webhook_body())).await;
    assert_eq!(created["data"]["trigger"]["latest_run"], Value::Null, "{created}");
    assert_eq!(created["data"]["trigger"]["runs_last_hour"], json!(0));
    assert_eq!(created["data"]["trigger"]["needs_person"], json!(false));
    let monitor = json!({
        "slug": "disk", "kind": "monitor", "title": "Disk", "instruction": "Warn me",
        "settings": { "script": "None" },
    });
    let (status, out) = call(&env.router, "POST", "/agents/agent-a/triggers", Some(&alice), Some(monitor)).await;
    assert_eq!(status, 201, "{out}");

    // One run of the webhook, and a monitor whose checks have stopped.
    let storage = env.runtime.deps().storage.clone();
    let ci = storage.get_trigger("agent-a", "ci").await.unwrap().unwrap();
    let now = vizier_core::time::now();
    storage
        .admit_event(NewEvent {
            trigger_id: ci.id.clone(),
            agent_id: ci.agent_id.clone(),
            received_at: now,
            payload: Some(json!({})),
            delivery_id: None,
            status: EventStatus::Waiting,
            reason: None,
            when_full: WhenFull::DropOldest,
            monitor_state: None,
        })
        .await
        .unwrap();
    let run = storage.start_trigger_run(&ci, now, "key".into()).await.unwrap().expect("a run").run;
    let disk = storage.get_trigger("agent-a", "disk").await.unwrap().unwrap();
    let stopped = MonitorRecord { failures: 10, failed_at: Some(now), ..Default::default() };
    storage.save_monitor_record(&disk.id, &stopped).await.unwrap();

    let (_, listed) = call(&env.router, "GET", "/agents/agent-a/triggers", Some(&alice), None).await;
    let items = listed["data"].as_array().expect("a list");
    let by = |slug: &str| items.iter().find(|t| t["slug"] == json!(slug)).unwrap().clone();
    assert_eq!(by("ci")["latest_run"]["id"], json!(run.id), "{listed}");
    assert_eq!(by("ci")["runs_last_hour"], json!(1));
    assert_eq!(by("ci")["needs_person"], json!(false));
    assert_eq!(by("disk")["needs_person"], json!(true));
    let (_, counted) = call(&env.router, "GET", summary, Some(&alice), None).await;
    assert_eq!(counted["data"], json!({ "total": 2, "active": 1, "needs_person": 1 }));

    // Saving the monitor starts its checks again, so it no longer waits for anyone.
    let update = json!({ "title": "Disk space" });
    let (_, updated) = call(&env.router, "PUT", "/agents/agent-a/triggers/disk", Some(&alice), Some(update)).await;
    assert_eq!(updated["data"]["needs_person"], json!(false), "{updated}");
    let (_, counted) = call(&env.router, "GET", summary, Some(&alice), None).await;
    assert_eq!(counted["data"], json!({ "total": 2, "active": 2, "needs_person": 0 }));
}
