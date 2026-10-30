//! Parity between the network router and the desktop dispatcher, route by route (FR-009).
//!
//! Every vector is sent through both with the same person; the status and the JSON envelope
//! must match. With `CONFORMANCE_RECORD=1` the responses are written to
//! `target/038-baseline/conformance/` (do this on the handlers before a port); otherwise they
//! are compared with what is recorded there, so a port proves "no behaviour change".

use std::{collections::BTreeSet, path::PathBuf};

use axum::{Router, body::Body, http::Request};
use serde_json::{Value, json};
use tower::ServiceExt;
use vizier_core::{
    config::VizierConfig,
    runtime::{RuntimeOptions, VizierRuntime},
    schema::AgentConfig,
    storage::{agent::AgentStorage, user::UserStorage},
};
use vizier_http::{auth::AuthService, embedded::Dispatcher};

struct Person {
    id: String,
    jwt: String,
    label: &'static str,
}

mod common;
mod groups;
use common::Vector;

fn agent(name: &str, owner: &str) -> AgentConfig {
    serde_json::from_value(json!({
        "name": name, "owner_id": owner, "system_prompt": null, "description": null,
        "provider": "dummyplug", "model": "dummyplug", "thinking_depth": 1,
        "tools": {"timeout": "30s"}, "silent_read_initiative_chance": 0.0,
        "prompt_timeout": "1m", "heartbeat_interval": "1m"
    }))
    .unwrap()
}

/// Generated or time-dependent values, which differ between two otherwise equal responses.
fn scrub(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (k, v) in map.iter_mut() {
                if k.ends_with("_at") || k == "timestamp" || k == "uid" || k == "updated" || k == "seq" || k == "revision_id" || k == "role_id" || k == "id" || k == "accounting_since" || k == "hours" {
                    *v = Value::Null;
                } else {
                    scrub(v);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(scrub),
        _ => {}
    }
}

async fn send(router: Router, vector: &Vector, jwt: Option<&str>) -> (u16, Value) {
    let mut req = Request::builder()
        .method(vector.method)
        .uri(format!("/api/v1{}", vector.path));
    if let Some(jwt) = jwt {
        req = req.header("authorization", format!("Bearer {jwt}"));
    }
    let body = match &vector.body {
        Some(b) => {
            req = req.header("content-type", "application/json");
            Body::from(b.to_string())
        }
        None => Body::empty(),
    };
    let res = router.oneshot(req.body(body).unwrap()).await.unwrap();
    let status = res.status().as_u16();
    let bytes = axum::body::to_bytes(res.into_body(), 4 << 20).await.unwrap();
    let mut json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    scrub(&mut json);
    (status, json)
}

#[tokio::test]
async fn desktop_dispatch_matches_the_network_router() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = VizierConfig::with_workspace(dir.path().to_path_buf());
    config.channels.http.as_mut().unwrap().jwt_secret = "conformance-secret".into();
    // No network from a test: a catalogue read would otherwise refresh from models.dev.
    config.catalogue.refresh = false;
    let runtime = VizierRuntime::start(RuntimeOptions { config }).await.unwrap();
    let storage = runtime.deps().storage.clone();
    let http_cfg = runtime.deps().config.channels.http.clone().unwrap();
    let auth = AuthService::new(&http_cfg);

    let system = storage.create_role("system", vec![], true).await.unwrap();
    let owner_role = storage
        .create_role(
            "owner",
            vec!["owned_agents:view".into(), "owned_agents:edit".into(), "owned_agents:core".into()],
            false,
        )
        .await
        .unwrap();
    let none_role = storage.create_role("none", vec![], false).await.unwrap();

    let mut people = vec![];
    for (name, role, label) in [("root", &system, "system"), ("olive", &owner_role, "owner"), ("nobody", &none_role, "none")] {
        let user = storage.create_user(name, "", &role.role_id).await.unwrap();
        let jwt = auth.generate_token(&user.user_id, name, role.permissions.clone()).unwrap();
        people.push(Person { id: user.user_id, jwt, label });
    }
    storage.create_agent("agent-a", &agent("A", &people[1].id)).await.unwrap();
    storage.create_agent("agent-b", &agent("B", &people[0].id)).await.unwrap();

    let record = std::env::var("CONFORMANCE_RECORD").is_ok();
    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/038-baseline/conformance");
    if record {
        std::fs::create_dir_all(&out_dir).unwrap();
    }

    let mut covered = BTreeSet::new();
    for person in &people {
        let dispatcher = Dispatcher::new(&runtime, person.id.clone());
        for (i, vector) in groups::all().iter().enumerate() {
            covered.insert(vector.path.to_string());
            let net = send(vizier_http::router(&runtime, &http_cfg), vector, Some(&person.jwt)).await;
            let res = dispatcher
                .dispatch(
                    Request::builder()
                        .method(vector.method)
                        .uri(format!("/api/v1{}", vector.path))
                        .header("content-type", "application/json")
                        .body(vector.body.as_ref().map(|b| Body::from(b.to_string())).unwrap_or_else(Body::empty))
                        .unwrap(),
                )
                .await;
            let status = res.status().as_u16();
            let bytes = axum::body::to_bytes(res.into_body(), 4 << 20).await.unwrap();
            let mut body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
            scrub(&mut body);
            let desk = (status, body);

            let label = format!("{} {} {} as {}", i, vector.method, vector.path, person.label);
            assert_eq!(net, desk, "network and desktop differ: {label}");

            // Identities differ between runs, so the baseline keys on the person's role label.
            let legacy = format!("{}_{}_{}.json", person.label, vector.method, vector.path.trim_matches('/').replace('/', "_"));
            // Vectors that differ only in their body would share a name, so a body is hashed in
            // (`DefaultHasher::new` is deterministic). A baseline recorded without the hash is
            // still read when no hashed one exists.
            let named = match &vector.body {
                Some(b) => {
                    use std::hash::{Hash, Hasher};
                    let mut h = std::collections::hash_map::DefaultHasher::new();
                    b.to_string().hash(&mut h);
                    format!("{}_{:08x}.json", legacy.trim_end_matches(".json"), h.finish() as u32)
                }
                None => legacy.clone(),
            };
            let file = if !record && !out_dir.join(&named).exists() { out_dir.join(&legacy) } else { out_dir.join(&named) };
            let mut snapshot = json!({ "status": desk.0, "body": desk.1 });
            strip_ids(&mut snapshot, &people);
            if record {
                std::fs::write(&file, serde_json::to_string_pretty(&snapshot).unwrap()).unwrap();
            } else if let Ok(saved) = std::fs::read_to_string(&file) {
                let mut saved: Value = serde_json::from_str(&saved).unwrap();
                let mut snapshot = snapshot.clone();
                // Fixed when the workspace was created, so it differs between runs.
                nullify(&mut saved, "accounting_since");
                nullify(&mut snapshot, "accounting_since");
                // Relative to today (the dream nights list), so it moves with the clock.
                nullify(&mut saved, "date");
                nullify(&mut snapshot, "date");
                assert_eq!(saved, snapshot, "behaviour changed: {label}");
            }
        }
    }

    // Coverage: every OpenAPI route needs a vector. The ratchet lists what is still pending;
    // it only shrinks, so a new route cannot be added without a vector.
    let res = vizier_http::router(&runtime, &http_cfg)
        .oneshot(Request::builder().uri("/openapi.json").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(res.into_body(), 8 << 20).await.unwrap();
    let spec: Value = serde_json::from_slice(&bytes).unwrap();
    let pending: BTreeSet<String> = spec["paths"]
        .as_object()
        .unwrap()
        .keys()
        .filter(|p| !covered.iter().any(|c| template_matches(p, c)))
        .cloned()
        .collect();
    let allowed: BTreeSet<String> = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/conformance/pending.txt"))
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect();
    if std::env::var("CONFORMANCE_LIST_PENDING").is_ok() {
        for p in &pending { println!("{p}"); }
    }
    let new: Vec<_> = pending.difference(&allowed).collect();
    assert!(new.is_empty(), "routes without a conformance vector (add one in tests/conformance/groups/): {new:?}");
}

/// User ids are generated per run; replace them so two runs compare.
fn strip_ids(value: &mut Value, people: &[Person]) {
    match value {
        Value::String(s) => {
            for p in people {
                if s.contains(&p.id) {
                    *s = s.replace(&p.id, p.label);
                }
            }
        }
        Value::Object(m) => m.values_mut().for_each(|v| strip_ids(v, people)),
        Value::Array(a) => a.iter_mut().for_each(|v| strip_ids(v, people)),
        _ => {}
    }
}

/// Whether a concrete vector path (`/agents/agent-a/core`) is an instance of an OpenAPI
/// template (`/agents/{agent_id}/core`); query strings are ignored.
fn template_matches(template: &str, concrete: &str) -> bool {
    let concrete = concrete.split('?').next().unwrap_or(concrete);
    let (t, c): (Vec<_>, Vec<_>) = (template.split('/').collect(), concrete.split('/').collect());
    t.len() == c.len() && t.iter().zip(&c).all(|(t, c)| (t.starts_with('{') && t.ends_with('}')) || t == c)
}

fn nullify(value: &mut Value, key: &str) {
    match value {
        Value::Object(m) => {
            for (k, v) in m.iter_mut() {
                if k == key {
                    *v = Value::Null;
                } else {
                    nullify(v, key);
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|v| nullify(v, key)),
        _ => {}
    }
}
