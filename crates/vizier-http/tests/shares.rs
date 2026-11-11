//! Shared chat snapshots over the real router (`specs/055-shared-chat/contracts/share-api.md`).
//!
//! One runtime, three people: `alice` (owns `agent-a`, holds `owned_agents:share_conversations`),
//! `bob` (signed in, no share permission) and `root` (system). Requests go through
//! `vizier_http::router`, so the body limit, the layers and the headers are the shipped ones.

use axum::{Router, body::Body, http::Request};
use serde_json::{Value, json};
use tower::ServiceExt;
use vizier_core::{
    config::VizierConfig,
    runtime::{RuntimeOptions, VizierRuntime},
    schema::AgentConfig,
    service::shares::MAX_BODY_BYTES,
    storage::{
        agent::AgentStorage,
        share::{Audience, InsertOutcome, ShareContent, ShareRecord, ShareStorage},
        user::UserStorage,
    },
};
use vizier_http::auth::AuthService;

struct Env {
    router: Router,
    runtime: VizierRuntime,
    alice: String,
    bob: String,
    root: String,
    alice_id: String,
    bob_id: String,
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
    config.channels.http.as_mut().unwrap().jwt_secret = "shares-secret".into();
    config.catalogue.refresh = false;
    let runtime = VizierRuntime::start(RuntimeOptions { config, ..Default::default() })
        .await
        .unwrap();
    let storage = runtime.deps().storage.clone();
    let http_cfg = runtime.deps().config.channels.http.clone().unwrap();
    let auth = AuthService::new(&http_cfg);

    let system = storage.create_role("system", vec![], true).await.unwrap();
    let sharer = storage
        .create_role(
            "sharer",
            vec!["owned_agents:view".into(), "owned_agents:share_conversations".into()],
            false,
        )
        .await
        .unwrap();
    let plain = storage.create_role("plain", vec!["owned_agents:view".into()], false).await.unwrap();
    let mut tokens = vec![];
    let mut ids = vec![];
    for (name, role) in [("alice", &sharer), ("bob", &plain), ("root", &system)] {
        let user = storage.create_user(name, "", &role.role_id).await.unwrap();
        tokens.push(auth.generate_token(&user.user_id, name, role.permissions.clone()).unwrap());
        ids.push(user.user_id);
    }
    storage.create_agent("agent-a", &agent("A", &ids[0])).await.unwrap();
    let router = vizier_http::router(&runtime, &http_cfg);
    let mut tokens = tokens.into_iter();
    let mut ids = ids.into_iter();
    Env {
        router,
        runtime,
        alice: tokens.next().unwrap(),
        bob: tokens.next().unwrap(),
        root: tokens.next().unwrap(),
        alice_id: ids.next().unwrap(),
        bob_id: ids.next().unwrap(),
        _dir: dir,
    }
}

struct Reply {
    status: u16,
    headers: axum::http::HeaderMap,
    bytes: Vec<u8>,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.bytes).unwrap_or(Value::Null)
    }
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|v| v.to_str().ok())
    }
}

async fn send(
    router: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Vec<u8>>,
    extra: &[(&str, &str)],
) -> Reply {
    let mut req = Request::builder().method(method).uri(format!("/api/v1{path}"));
    if let Some(token) = token {
        req = req.header("authorization", format!("Bearer {token}"));
    }
    for (k, v) in extra {
        req = req.header(*k, *v);
    }
    let body = match body {
        Some(b) => {
            req = req.header("content-type", "application/json");
            Body::from(b)
        }
        None => Body::empty(),
    };
    let res = router.clone().oneshot(req.body(body).unwrap()).await.unwrap();
    let status = res.status().as_u16();
    let headers = res.headers().clone();
    let bytes = axum::body::to_bytes(res.into_body(), 16 << 20).await.unwrap().to_vec();
    Reply { status, headers, bytes }
}

/// The snapshot as a person's browser would post it: odd spacing and key order that any
/// re-serialisation would change.
const SNAPSHOT: &str = "{ \"turns\" : [ {\"role\":\"person\",\"id\":\"t1\",\"at\":1,\"parts\":[{\"markdown\":\"hello é \\u2028 </script>\",\"kind\":\"text\"}] } ],\n  \"format_version\":1, \"title\":\"Chat\",\"include_trail\":false,\"media\":{} }";

fn post_body(snapshot: &str, extra: Value) -> Vec<u8> {
    let mut fields = json!({
        "agent_id": "agent-a", "topic_id": "t1", "title": "Chat", "audience": "public",
        "include_trail": false, "removals": {"credentials": 0, "paths": 0}
    });
    for (k, v) in extra.as_object().unwrap() {
        fields[k] = v.clone();
    }
    let mut text = serde_json::to_string(&fields).unwrap();
    assert!(text.ends_with('}'));
    text.pop();
    format!("{text},\"snapshot\":{snapshot}}}").into_bytes()
}

async fn create(e: &Env, token: &str, extra: Value) -> Reply {
    send(&e.router, "POST", "/shares", Some(token), Some(post_body(SNAPSHOT, extra)), &[]).await
}

#[tokio::test]
async fn a_public_share_is_created_then_read_by_anyone_with_the_cache_headers() {
    let e = env().await;
    let created = create(&e, &e.alice, json!({})).await;
    assert_eq!(created.status, 201, "{}", String::from_utf8_lossy(&created.bytes));
    let data = created.json()["data"].clone();
    let id = data["share_id"].as_str().unwrap().to_string();
    assert_eq!(id.len(), 21);
    assert_eq!(data["url_path"], format!("/share/{id}"));

    // Anonymous.
    let read = send(&e.router, "GET", &format!("/shares/{id}"), None, None, &[]).await;
    assert_eq!(read.status, 200);
    assert_eq!(read.header("cache-control"), Some("public, max-age=300, s-maxage=300"));
    assert!(!read.header("cache-control").unwrap().contains("immutable"));
    assert_eq!(read.header("etag"), Some(format!("\"{id}\"").as_str()));
    assert_eq!(read.header("x-robots-tag"), Some("noindex, nofollow"));
    assert_eq!(read.header("referrer-policy"), Some("no-referrer"));
    assert_eq!(read.header("x-content-type-options"), Some("nosniff"));
    assert!(read.header("content-type").unwrap().starts_with("application/json"));
    assert!(read.header("vary").is_none());

    // The stored text is inside the envelope exactly as posted.
    let text = String::from_utf8(read.bytes.clone()).unwrap();
    assert!(text.contains(SNAPSHOT), "snapshot text was altered: {text}");

    // No viewer-specific field: the same bytes for a signed-in person.
    let again = send(&e.router, "GET", &format!("/shares/{id}"), Some(&e.bob), None, &[]).await;
    assert_eq!(again.bytes, read.bytes);
    let body = read.json()["data"].clone();
    let mut keys: Vec<_> = body.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(
        keys,
        ["audience", "created_at", "format_version", "include_trail", "share_id", "snapshot", "title"]
    );

    // Conditional read.
    let tag = format!("\"{id}\"");
    let cached = send(&e.router, "GET", &format!("/shares/{id}"), None, None, &[("if-none-match", &tag)]).await;
    assert_eq!(cached.status, 304);
    assert!(cached.bytes.is_empty());
    assert_eq!(cached.header("etag"), Some(tag.as_str()));
    let other = send(&e.router, "GET", &format!("/shares/{id}"), None, None, &[("if-none-match", "\"nope\"")]).await;
    assert_eq!(other.status, 200);
}

#[tokio::test]
async fn creating_needs_a_session_and_the_permission() {
    let e = env().await;
    let anon = send(&e.router, "POST", "/shares", None, Some(post_body(SNAPSHOT, json!({}))), &[]).await;
    assert_eq!(anon.status, 401);
    assert_eq!(create(&e, &e.bob, json!({})).await.status, 403);
    assert_eq!(create(&e, &e.root, json!({})).await.status, 201);
    // Alice holds only the owned tier: another agent is out of reach.
    let storage = e.runtime.deps().storage.clone();
    storage.create_agent("agent-b", &agent("B", &e.bob_id)).await.unwrap();
    let r = create(&e, &e.alice, json!({"agent_id": "agent-b"})).await;
    assert_eq!(r.status, 403);
}

#[tokio::test]
async fn malformed_and_oversized_bodies_are_refused() {
    let e = env().await;
    let post = |body: Vec<u8>| send(&e.router, "POST", "/shares", Some(&e.alice), Some(body), &[]);

    // The route limit beats the router-wide `DefaultBodyLimit::disable()`.
    let filler = "a".repeat(MAX_BODY_BYTES + 1);
    let huge = format!(
        "{{\"agent_id\":\"agent-a\",\"topic_id\":\"t\",\"title\":\"x\",\"audience\":\"public\",\"snapshot\":{{\"format_version\":1,\"turns\":[],\"pad\":\"{filler}\"}}}}"
    );
    assert!(huge.len() > MAX_BODY_BYTES);
    assert_eq!(post(huge.into_bytes()).await.status, 413);

    assert_eq!(post(b"{not json".to_vec()).await.status, 400);
    let bad = |snapshot: &str| post_body(snapshot, json!({}));
    assert_eq!(post(bad("{\"format_version\":2,\"turns\":[{\"parts\":[]}]}")).await.status, 400);
    assert_eq!(post(bad("{\"format_version\":1,\"turns\":[]}")).await.status, 400);
    assert_eq!(post(bad("{\"format_version\":1,\"turns\":[{\"parts\":[]}],\"surprise\":1}")).await.status, 400);
    let long_title = send(
        &e.router,
        "POST",
        "/shares",
        Some(&e.alice),
        Some(post_body(SNAPSHOT, json!({"title": "t".repeat(121)}))),
        &[],
    )
    .await;
    assert_eq!(long_title.status, 400);
    // The trail flag against the document.
    let with_trail = "{\"format_version\":1,\"turns\":[{\"parts\":[],\"trail\":[{\"kind\":\"tool_call\"}]}]}";
    let mismatch = post(bad(with_trail)).await;
    assert_eq!(mismatch.status, 422);
    assert_eq!(mismatch.json()["data"]["error"], "trail_mismatch");
    // Nothing was stored by any of them.
    let storage = e.runtime.deps().storage.clone();
    assert_eq!(storage.count_active_by_creator(&e.alice_id).await.unwrap(), 0);
}

#[tokio::test]
async fn the_eleventh_create_in_an_hour_is_429_with_a_wait() {
    let e = env().await;
    for _ in 0..10 {
        assert_eq!(create(&e, &e.alice, json!({})).await.status, 201);
    }
    let eleventh = create(&e, &e.alice, json!({})).await;
    assert_eq!(eleventh.status, 429);
    let secs = eleventh.json()["data"]["retry_after_secs"].as_u64().unwrap();
    assert!((1..=3600).contains(&secs));
    assert!(eleventh.header("retry-after").is_some());
    // The limit is per person.
    assert_eq!(create(&e, &e.root, json!({})).await.status, 201);
}

#[tokio::test]
async fn the_two_hundred_and_first_active_share_is_429() {
    let e = env().await;
    let storage = e.runtime.deps().storage.clone();
    for i in 0..200 {
        let snapshot_json = SNAPSHOT.to_string();
        let share = ShareContent {
            record: ShareRecord {
                share_id: format!("{i:021}"),
                creator_id: e.alice_id.clone(),
                agent_id: "agent-a".into(),
                topic_id: "t".into(),
                title: "t".into(),
                audience: Audience::Public,
                include_trail: false,
                format_version: 1,
                size_bytes: snapshot_json.len() as i64,
                created_at: i,
                revoked_at: None,
                revoked_by: None,
                removals_json: "{}".into(),
            },
            snapshot_json,
            people: vec![],
        };
        assert_eq!(storage.insert_share(&share, 200).await.unwrap(), InsertOutcome::Inserted);
    }
    assert_eq!(create(&e, &e.alice, json!({})).await.status, 429);
    // A revoked share does not count.
    assert!(storage.revoke_share(&format!("{:021}", 0), &e.alice_id, 5).await.unwrap());
    assert_eq!(create(&e, &e.alice, json!({})).await.status, 201);
}

#[tokio::test]
async fn unknown_malformed_and_restricted_ids_answer_alike() {
    let e = env().await;
    let signed = create(&e, &e.alice, json!({"audience": "signed_in"})).await.json()["data"]["share_id"]
        .as_str()
        .unwrap()
        .to_string();
    let named = create(&e, &e.alice, json!({"audience": "specific", "people": [e.bob_id]})).await.json()["data"]
        ["share_id"]
        .as_str()
        .unwrap()
        .to_string();

    // No session: unknown, malformed and non-public are the same body.
    let paths = [
        "/shares/ZZZZZZZZZZZZZZZZZZZZZ".to_string(),
        "/shares/short".to_string(),
        "/shares/%00".to_string(),
        format!("/shares/{signed}"),
        format!("/shares/{named}"),
        format!("/shares/{signed}x"),
    ];
    let mut bodies = vec![];
    for p in &paths {
        let r = send(&e.router, "GET", p, None, None, &[]).await;
        assert_eq!(r.status, 401, "{p}");
        assert_eq!(r.header("cache-control"), Some("no-store"), "{p}");
        assert_eq!(r.header("x-robots-tag"), Some("noindex, nofollow"));
        bodies.push(r.bytes);
    }
    assert!(bodies.windows(2).all(|w| w[0] == w[1]));
    let first: Value = serde_json::from_slice(&bodies[0]).unwrap();
    assert_eq!((first["message"].as_str(), first["data"]["error"].as_str()), (Some("sign_in_required"), Some("sign_in_required")));

    // Signed in: unknown and not-for-me are one 404 body; allowed readers get private bodies.
    let carol = &e.root;
    let mut refused = vec![];
    for p in ["/shares/ZZZZZZZZZZZZZZZZZZZZZ", "/shares/short"] {
        let r = send(&e.router, "GET", p, Some(&e.alice), None, &[]).await;
        assert_eq!(r.status, 404);
        refused.push(r.bytes);
    }
    // alice is the author of `named`, bob is named, root is neither.
    let not_named = send(&e.router, "GET", &format!("/shares/{named}"), Some(carol), None, &[]).await;
    assert_eq!(not_named.status, 404);
    refused.push(not_named.bytes);
    assert!(refused.windows(2).all(|w| w[0] == w[1]));
    let first: Value = serde_json::from_slice(&refused[0]).unwrap();
    assert_eq!((first["message"].as_str(), first["data"]["error"].as_str()), (Some("share_unavailable"), Some("share_unavailable")));

    for (id, token) in [(&named, &e.bob), (&named, &e.alice), (&signed, &e.root)] {
        let r = send(&e.router, "GET", &format!("/shares/{id}"), Some(token), None, &[]).await;
        assert_eq!(r.status, 200);
        assert_eq!(r.header("cache-control"), Some("private, no-store"));
        assert_eq!(r.header("vary"), Some("Authorization, Cookie"));
        assert!(r.header("etag").is_none());
    }
    // A conditional request never turns a private body into a 304.
    let r = send(&e.router, "GET", &format!("/shares/{signed}"), Some(&e.bob), None, &[("if-none-match", "*")]).await;
    assert_eq!(r.status, 200);
}

#[tokio::test]
async fn a_revoked_share_is_gone_at_the_origin_at_once() {
    let e = env().await;
    let id = create(&e, &e.alice, json!({})).await.json()["data"]["share_id"].as_str().unwrap().to_string();
    let storage = e.runtime.deps().storage.clone();
    assert_eq!(send(&e.router, "GET", &format!("/shares/{id}"), None, None, &[]).await.status, 200);
    assert!(storage.revoke_share(&id, &e.alice_id, 1).await.unwrap());
    assert_eq!(send(&e.router, "GET", &format!("/shares/{id}"), None, None, &[]).await.status, 401);
    assert_eq!(send(&e.router, "GET", &format!("/shares/{id}"), Some(&e.alice), None, &[]).await.status, 404);
}

#[tokio::test]
async fn audience_viewer_matrix_and_people_validation() {
    let e = env().await;
    // Validation on create
    // Specific requires 1-50 people
    assert_eq!(create(&e, &e.alice, json!({"audience": "specific"})).await.status, 400);
    assert_eq!(create(&e, &e.alice, json!({"audience": "specific", "people": []})).await.status, 400);
    let too_many: Vec<String> = (0..51).map(|i| format!("user-{i}")).collect();
    assert_eq!(create(&e, &e.alice, json!({"audience": "specific", "people": too_many})).await.status, 400);
    // Duplicate people rejected
    assert_eq!(create(&e, &e.alice, json!({"audience": "specific", "people": [e.bob_id.clone(), e.bob_id.clone()]})).await.status, 400);
    // Unknown person rejected
    assert_eq!(create(&e, &e.alice, json!({"audience": "specific", "people": ["non-existent-user-id"]})).await.status, 400);
    // Public and SignedIn must not have people
    assert_eq!(create(&e, &e.alice, json!({"audience": "public", "people": [e.bob_id.clone()]})).await.status, 400);
    assert_eq!(create(&e, &e.alice, json!({"audience": "signed_in", "people": [e.bob_id.clone()]})).await.status, 400);

    // 3 x 4 matrix
    let pub_id = create(&e, &e.alice, json!({"audience": "public"})).await.json()["data"]["share_id"].as_str().unwrap().to_string();
    let signed_id = create(&e, &e.alice, json!({"audience": "signed_in"})).await.json()["data"]["share_id"].as_str().unwrap().to_string();
    let specific_id = create(&e, &e.alice, json!({"audience": "specific", "people": [e.bob_id.clone()]})).await.json()["data"]["share_id"].as_str().unwrap().to_string();

    // 1. Public: author, named, other, anonymous all 200
    for token in [Some(&e.alice), Some(&e.bob), Some(&e.root), None] {
        let r = send(&e.router, "GET", &format!("/shares/{pub_id}"), token.copied(), None, &[]).await;
        assert_eq!(r.status, 200);
    }

    // 2. SignedIn: author, named, other 200; anonymous 401
    for token in [Some(&e.alice), Some(&e.bob), Some(&e.root)] {
        let r = send(&e.router, "GET", &format!("/shares/{signed_id}"), token.copied(), None, &[]).await;
        assert_eq!(r.status, 200);
        assert_eq!(r.header("cache-control"), Some("private, no-store"));
        assert_eq!(r.header("vary"), Some("Authorization, Cookie"));
    }
    let anon_signed = send(&e.router, "GET", &format!("/shares/{signed_id}"), None, None, &[]).await;
    assert_eq!(anon_signed.status, 401);

    // 3. Specific: author (alice) 200, named (bob) 200, other (root) 404, anonymous 401
    let r_author = send(&e.router, "GET", &format!("/shares/{specific_id}"), Some(&e.alice), None, &[]).await;
    assert_eq!(r_author.status, 200);
    assert_eq!(r_author.header("cache-control"), Some("private, no-store"));

    let r_named = send(&e.router, "GET", &format!("/shares/{specific_id}"), Some(&e.bob), None, &[]).await;
    assert_eq!(r_named.status, 200);
    assert_eq!(r_named.header("cache-control"), Some("private, no-store"));

    let r_other = send(&e.router, "GET", &format!("/shares/{specific_id}"), Some(&e.root), None, &[]).await;
    assert_eq!(r_other.status, 404);
    assert_eq!(r_other.header("cache-control"), Some("no-store"));

    let r_anon = send(&e.router, "GET", &format!("/shares/{specific_id}"), None, None, &[]).await;
    assert_eq!(r_anon.status, 401);
    assert_eq!(r_anon.header("cache-control"), Some("no-store"));
}

#[tokio::test]
async fn share_miss_limiter_throttles_after_thirty_misses_per_address() {
    let e = env().await;
    let pub_id = create(&e, &e.alice, json!({})).await.json()["data"]["share_id"].as_str().unwrap().to_string();
    let addr = ("x-forwarded-for", "192.168.1.100");

    // 30 misses on non-existent share
    for _ in 0..30 {
        let r = send(&e.router, "GET", "/shares/ZZZZZZZZZZZZZZZZZZZZZ", None, None, &[addr]).await;
        assert_eq!(r.status, 401);
    }

    // 31st miss from same address is 429
    let r31 = send(&e.router, "GET", "/shares/ZZZZZZZZZZZZZZZZZZZZZ", None, None, &[addr]).await;
    assert_eq!(r31.status, 429);
    assert_eq!(r31.json()["data"]["error"], "rate_limited");
    assert!(r31.header("retry-after").is_some());
    assert_eq!(r31.header("cache-control"), Some("no-store"));

    // Hits on real public links are NEVER throttled!
    let real = send(&e.router, "GET", &format!("/shares/{pub_id}"), None, None, &[addr]).await;
    assert_eq!(real.status, 200);

    // Another client address has its own budget
    let other_addr = ("x-forwarded-for", "192.168.1.101");
    let r_other = send(&e.router, "GET", "/shares/ZZZZZZZZZZZZZZZZZZZZZ", None, None, &[other_addr]).await;
    assert_eq!(r_other.status, 401);
}

#[tokio::test]
async fn search_people_endpoint_searches_and_enforces_permissions() {
    let e = env().await;

    // Anonymous -> 401
    let anon = send(&e.router, "GET", "/shares/people?q=bo", None, None, &[]).await;
    assert_eq!(anon.status, 401);

    // Caller without share permission (bob) -> 403
    let forbidden = send(&e.router, "GET", "/shares/people?q=bo", Some(&e.bob), None, &[]).await;
    assert_eq!(forbidden.status, 403);

    // Query < 2 chars -> 400
    let short = send(&e.router, "GET", "/shares/people?q=b", Some(&e.alice), None, &[]).await;
    assert_eq!(short.status, 400);

    // Caller with permission (alice) -> 200 with matching user
    let ok = send(&e.router, "GET", "/shares/people?q=bo", Some(&e.alice), None, &[]).await;
    assert_eq!(ok.status, 200);
    let people = ok.json()["data"].as_array().unwrap().clone();
    assert_eq!(people.len(), 1);
    assert_eq!(people[0]["user_id"], e.bob_id);
    assert_eq!(people[0]["username"], "bob");

    // No sensitive fields leaked
    let obj = people[0].as_object().unwrap();
    let mut keys: Vec<_> = obj.keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, ["user_id", "username"]);

    // Static route precedence: /shares/people is not matched as /shares/{share_id}
    assert_ne!(ok.status, 404);
}
