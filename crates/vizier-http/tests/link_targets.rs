//! The entity wiki-link name index over the real router (`specs/066`,
//! `contracts/link-targets-endpoint.md`).
//!
//! One runtime, three people: `alice` owns `agent-a` and may view its artifacts; `viewer`
//! can view every agent but has no artifact permission (per-kind visibility); `stranger`
//! may not view `agent-a` at all. Fixtures are created through the shipped services and
//! storage, so the test also proves the index composes real data.

use axum::{Router, body::Body, http::Request};
use chrono::{Local, Timelike};
use serde_json::{Value, json};
use tower::ServiceExt;
use vizier_core::{
    config::VizierConfig,
    custom_tools::{CreateInput, Env as ToolEnv},
    runtime::{RuntimeOptions, VizierRuntime},
    schema::{AgentConfig, Artifact, ArtifactEvent, ArtifactEventAction, ArtifactKind, Requester},
    storage::{
        agent::AgentStorage, artifact::ArtifactStorage, custom_tool::CustomToolActor, user::UserStorage,
    },
};
use vizier_http::auth::AuthService;

struct Env {
    router: Router,
    runtime: VizierRuntime,
    alice: String,
    viewer: String,
    stranger: String,
    root: String,
    _dir: tempfile::TempDir,
}

fn agent(name: &str, owner: &str) -> AgentConfig {
    serde_json::from_value(json!({
        "name": name, "owner_id": owner, "system_prompt": null, "description": null,
        "provider": "dummyplug", "model": "dummyplug", "thinking_depth": 1,
        "tools": {"timeout": "30s"}, "silent_read_initiative_chance": 0.0,
        "prompt_timeout": "1m", "heartbeat_interval": "1m"
    }))
    .unwrap()
}

async fn env() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let mut config = VizierConfig::with_workspace(dir.path().to_path_buf());
    config.channels.http.as_mut().unwrap().jwt_secret = "link-targets-secret".into();
    config.catalogue.refresh = false;
    let runtime = VizierRuntime::start(RuntimeOptions { config, ..Default::default() })
        .await
        .unwrap();
    let storage = runtime.deps().storage.clone();
    let http_cfg = runtime.deps().config.channels.http.clone().unwrap();
    let auth = AuthService::new(&http_cfg);

    let system = storage.create_role("system", vec![], true).await.unwrap();
    let owner_role = storage
        .create_role("owner", vec!["owned_agents:view".into(), "owned_agents:view_artifacts".into()], false)
        .await
        .unwrap();
    let viewer_role = storage.create_role("viewer", vec!["all_agents:view".into()], false).await.unwrap();
    let plain = storage.create_role("plain", vec![], false).await.unwrap();

    let mut tokens = vec![];
    let mut users = vec![];
    for (name, role) in [("alice", &owner_role), ("viewer", &viewer_role), ("stranger", &plain), ("root", &system)] {
        let user = storage.create_user(name, "", &role.role_id).await.unwrap();
        tokens.push(auth.generate_token(&user.user_id, name, role.permissions.clone()).unwrap());
        users.push(user.user_id);
    }
    let alice_id = users[0].clone();
    storage.create_agent("agent-a", &agent("A", &alice_id)).await.unwrap();

    // Artifact, through the storage the promote path uses.
    let now = Local::now().with_nanosecond(0).unwrap();
    let artifact = Artifact {
        id: "art-1".into(),
        agent_id: "agent-a".into(),
        name: "report".into(),
        kind: ArtifactKind::Markdown,
        mime_type: "text/markdown".into(),
        size: 12,
        version: 1,
        content_file: "content.v1".into(),
        origin_channel: None,
        origin_topic: None,
        source_filename: None,
        created_by: Requester::User(alice_id.clone()),
        created_at: now,
        updated_at: now,
    };
    let event = ArtifactEvent {
        id: "evt-1".into(),
        agent_id: "agent-a".into(),
        artifact_id: "art-1".into(),
        artifact_name: "report".into(),
        action: ArtifactEventAction::Promoted,
        actor: Requester::User(alice_id.clone()),
        source_channel: None,
        source_topic: None,
        source_filename: None,
        created_at: now,
    };
    storage.create_artifact(&artifact, &event).await.unwrap();

    // Function, through the agent-facing write path (there is no person create route).
    let tool_env = ToolEnv {
        agent_id: "agent-a".into(),
        actor: CustomToolActor::Person { user_id: alice_id.clone(), name: "alice".into() },
        names: runtime.deps().custom_tools.published_names("agent-a").await,
        runner: None,
        reserved: vec![],
    };
    runtime
        .deps()
        .custom_tools
        .create(
            &tool_env,
            CreateInput {
                name: "fetch_weather".into(),
                description: "Look up the weather".into(),
                input_schema: json!({"type": "object", "properties": {}}),
                script: "pass".into(),
                category: None,
                category_description: None,
                output_description: None,
                sample_input: None,
                reason: None,
            },
        )
        .await
        .unwrap();

    // Skill, a workspace file like the agent itself would have.
    let skill_dir = dir.path().join("agents").join("agent-a").join("skills").join("summarize");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: summarize\ndescription: Summarize text\n---\nDo the thing.",
    )
    .unwrap();

    // Memory docs are seeded in quickstart.md's e2e instead: the memory-op service travels
    // the bus and the indexer type is crate-private, so this harness cannot write one.

    let router = vizier_http::router(&runtime, &http_cfg);
    let mut tokens = tokens.into_iter();
    Env {
        router,
        runtime,
        alice: tokens.next().unwrap(),
        viewer: tokens.next().unwrap(),
        stranger: tokens.next().unwrap(),
        root: tokens.next().unwrap(),
        _dir: dir,
    }
}

struct Reply {
    status: u16,
    body: Value,
}

async fn send(env: &Env, method: &str, uri: &str, token: &str, body: Option<Value>) -> Reply {
    let mut req = Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {}", token));
    let req = match body {
        Some(json) => req.header("content-type", "application/json").body(Body::from(json.to_string())),
        None => req.body(Body::empty()),
    }
    .unwrap();
    let res = env.router.clone().oneshot(req).await.unwrap();
    let status = res.status().as_u16();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap();
    let text = String::from_utf8_lossy(&bytes).to_string();
    let body = serde_json::from_slice(&bytes).unwrap_or_else(|_| json!({ "raw": text }));
    Reply { status, body }
}

fn data(reply: &Reply) -> Value {
    reply.body["data"].clone()
}

/// The task fixture goes through the shipped route itself.
async fn seed_task(env: &Env) {
    let reply = send(
        env,
        "POST",
        "/api/v1/agents/agent-a/tasks",
        &env.alice,
        Some(json!({
            "slug": "weekly-report", "title": "Weekly report", "instruction": "write it",
            "schedule": {"type": "Cron", "expression": "0 9 * * 1"}
        })),
    )
    .await;
    assert_eq!(reply.status, 201, "{}", reply.body);
}

#[tokio::test]
async fn index_serves_every_kind_its_names() {
    let env = env().await;
    seed_task(&env).await;

    let reply = send(&env, "GET", "/api/v1/agents/agent-a/link-targets", &env.alice, None).await;
    assert_eq!(reply.status, 200, "{}", reply.body);
    let targets = data(&reply);
    assert_eq!(targets["artifacts"], json!(["report"]));
    assert_eq!(targets["functions"], json!(["fetch_weather"]));
    assert_eq!(targets["tasks"][0]["slug"], "weekly-report");
    assert_eq!(targets["tasks"][0]["title"], "Weekly report");
    assert!(targets["skills"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["slug"] == "summarize" && s["scope"] == "agent"));
    // Every agent carries the seeded `core` bundle (its soul); docs are e2e-seeded.
    assert!(targets["memory"]["bundles"].as_array().unwrap().iter().any(|b| b == "core"));
    assert!(targets["memory"]["docs"].is_array());
    assert_eq!(targets["truncated"]["artifacts"], false);
    assert_eq!(targets["truncated"]["memory.docs"], false);
}

#[tokio::test]
async fn artifact_names_respect_artifact_view_permission() {
    let env = env().await;
    seed_task(&env).await;

    // `viewer` sees the agent but has no artifact permission: empty artifacts, rest intact.
    let reply = send(&env, "GET", "/api/v1/agents/agent-a/link-targets", &env.viewer, None).await;
    assert_eq!(reply.status, 200, "{}", reply.body);
    let targets = data(&reply);
    assert_eq!(targets["artifacts"], json!([]));
    assert_eq!(targets["functions"], json!(["fetch_weather"]));

    // The owner still sees the artifact, and resolve reaches past the index caps exactly.
    let reply = send(
        &env,
        "POST",
        "/api/v1/agents/agent-a/link-targets/resolve",
        &env.alice,
        Some(json!({"links": [
            {"kind": "artifact", "name": "report"},
            {"kind": "artifact", "name": "gone"},
            {"kind": "function", "name": "fetch_weather"},
            {"kind": "task", "name": "weekly-report"},
            {"kind": "skill", "name": "summarize"},
            {"kind": "skill", "name": "no-such-skill"},
            {"kind": "memory", "name": "research"},
            {"kind": "memory", "name": "research/notes"},
            {"kind": "memory", "name": "research/missing"},
            {"kind": "flavour", "name": "vanilla"}
        ]})),
    )
    .await;
    assert_eq!(reply.status, 200, "{}", reply.body);
    let resolved = data(&reply);
    assert_eq!(resolved["artifact:report"], true);
    assert_eq!(resolved["artifact:gone"], false);
    assert_eq!(resolved["function:fetch_weather"], true);
    assert_eq!(resolved["task:weekly-report"], true);
    assert_eq!(resolved["skill:summarize"], true);
    assert_eq!(resolved["skill:no-such-skill"], false);
    // No memory docs are seeded here (the e2e covers the memory leg); both read as absent.
    assert_eq!(resolved["memory:research"], false);
    assert_eq!(resolved["memory:research/notes"], false);
    assert_eq!(resolved["memory:research/missing"], false);
    assert_eq!(resolved["flavour:vanilla"], false);
}

#[tokio::test]
async fn search_filters_one_kind() {
    let env = env().await;
    seed_task(&env).await;

    let reply = send(
        &env,
        "GET",
        "/api/v1/agents/agent-a/link-targets?kind=task&q=weekly&limit=5",
        &env.alice,
        None,
    )
    .await;
    assert_eq!(reply.status, 200, "{}", reply.body);
    let targets = data(&reply);
    assert_eq!(targets["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(targets["tasks"][0]["slug"], "weekly-report");
    assert_eq!(targets["functions"], json!([]));

    let reply = send(&env, "GET", "/api/v1/agents/agent-a/link-targets?kind=flavour&q=x", &env.alice, None).await;
    assert_eq!(reply.status, 400);
}

#[tokio::test]
async fn permission_and_existence_edges() {
    let env = env().await;
    // No view permission.
    let reply = send(&env, "GET", "/api/v1/agents/agent-a/link-targets", &env.stranger, None).await;
    assert_eq!(reply.status, 403, "{}", reply.body);
    // Unknown agent.
    let reply = send(&env, "GET", "/api/v1/agents/nobody/link-targets", &env.root, None).await;
    assert_eq!(reply.status, 404, "{}", reply.body);
    // Oversized resolve batch is 422.
    let links: Vec<Value> = (0..101).map(|i| json!({"kind": "artifact", "name": format!("n{i}")})).collect();
    let reply = send(
        &env,
        "POST",
        "/api/v1/agents/agent-a/link-targets/resolve",
        &env.alice,
        Some(json!({ "links": links })),
    )
    .await;
    assert_eq!(reply.status, 422, "{}", reply.body);
}
