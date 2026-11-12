//! Shared chat snapshots over the real router (`specs/056-shared-chat/contracts/share-api.md`).
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
    send_from(router, method, path, token, body, extra, None).await
}

/// `send` as a connection from `peer` (what the server's listener adds to every request).
async fn send_from(
    router: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Vec<u8>>,
    extra: &[(&str, &str)],
    peer: Option<std::net::SocketAddr>,
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
    let mut req = req.body(body).unwrap();
    if let Some(peer) = peer {
        req.extensions_mut().insert(axum::extract::ConnectInfo(peer));
    }
    let res = router.clone().oneshot(req).await.unwrap();
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
        ["created_at", "format_version", "include_trail", "share_id", "snapshot", "title"]
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
        let r = send(&e.router, "GET", &format!("/shares/{pub_id}"), token.map(|t| t.as_str()), None, &[]).await;
        assert_eq!(r.status, 200);
    }

    // 2. SignedIn: author, named, other 200; anonymous 401
    for token in [Some(&e.alice), Some(&e.bob), Some(&e.root)] {
        let r = send(&e.router, "GET", &format!("/shares/{signed_id}"), token.map(|t| t.as_str()), None, &[]).await;
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

const MISS: &str = "/shares/ZZZZZZZZZZZZZZZZZZZZZ";

fn peer(ip: &str) -> Option<std::net::SocketAddr> {
    Some(format!("{ip}:50000").parse().unwrap())
}

/// T052 / FR-049: 60 refused reads a minute per address; real public links are never counted.
#[tokio::test]
async fn share_miss_limiter_throttles_after_sixty_misses_per_address() {
    let e = env().await;
    let pub_id = create(&e, &e.alice, json!({})).await.json()["data"]["share_id"].as_str().unwrap().to_string();
    let a = peer("203.0.113.10");

    // Hits on a real public link are not counted, however many there are.
    for _ in 0..70 {
        let r = send_from(&e.router, "GET", &format!("/shares/{pub_id}"), None, None, &[], a).await;
        assert_eq!(r.status, 200);
    }
    for _ in 0..60 {
        assert_eq!(send_from(&e.router, "GET", MISS, None, None, &[], a).await.status, 401);
    }
    let over = send_from(&e.router, "GET", MISS, None, None, &[], a).await;
    assert_eq!(over.status, 429);
    assert_eq!(over.json()["data"]["error"], "rate_limited");
    assert!(over.header("retry-after").is_some());
    assert_eq!(over.header("cache-control"), Some("no-store"));
    assert_eq!(over.header("x-robots-tag"), Some("noindex, nofollow"));
    // Signed-in misses are counted per address too.
    let signed = send_from(&e.router, "GET", MISS, Some(&e.alice), None, &[], a).await;
    assert_eq!(signed.status, 429);
    // The real link still opens while the address is throttled.
    let real = send_from(&e.router, "GET", &format!("/shares/{pub_id}"), None, None, &[], a).await;
    assert_eq!(real.status, 200);
    // Another address has its own budget.
    assert_eq!(send_from(&e.router, "GET", MISS, None, None, &[], peer("203.0.113.11")).await.status, 401);
}

/// A caller cannot reach a fresh bucket by writing `X-Forwarded-For`, nor put somebody else
/// into theirs; a proxy on this machine is believed.
#[tokio::test]
async fn forwarding_headers_do_not_choose_the_miss_bucket() {
    let e = env().await;
    let a = peer("203.0.113.20");
    for i in 0..60 {
        let spoof = format!("10.0.0.{i}");
        let r = send_from(&e.router, "GET", MISS, None, None, &[("x-forwarded-for", &spoof)], a).await;
        assert_eq!(r.status, 401);
    }
    let r = send_from(&e.router, "GET", MISS, None, None, &[("x-forwarded-for", "10.9.9.9")], a).await;
    assert_eq!(r.status, 429, "a spoofed header did not buy a new budget");
    // The victim named in a header is not throttled by it.
    let victim = send_from(&e.router, "GET", MISS, None, None, &[], peer("198.51.100.99")).await;
    assert_eq!(victim.status, 401);
    // Behind a proxy on this machine the address it appended is the visitor.
    let proxy = peer("127.0.0.1");
    for _ in 0..60 {
        let h = [("x-forwarded-for", "7.7.7.7, 192.0.2.50")];
        assert_eq!(send_from(&e.router, "GET", MISS, None, None, &h, proxy).await.status, 401);
    }
    let h = [("x-forwarded-for", "7.7.7.7, 192.0.2.50")];
    assert_eq!(send_from(&e.router, "GET", MISS, None, None, &h, proxy).await.status, 429);
    let h = [("x-forwarded-for", "7.7.7.7, 192.0.2.51")];
    assert_eq!(send_from(&e.router, "GET", MISS, None, None, &h, proxy).await.status, 401);
}

/// T053: the audience cannot change, there is no update route.
#[tokio::test]
async fn a_share_cannot_be_edited() {
    let e = env().await;
    let id = create(&e, &e.alice, json!({"audience": "specific", "people": [e.bob_id.clone()]}))
        .await
        .json()["data"]["share_id"]
        .as_str()
        .unwrap()
        .to_string();
    for method in ["PUT", "PATCH", "POST"] {
        let body = Some(br#"{"audience":"public"}"#.to_vec());
        let r = send(&e.router, method, &format!("/shares/{id}"), Some(&e.alice), body, &[]).await;
        assert!(matches!(r.status, 404 | 405), "{method} -> {}", r.status);
    }
    let r = send(&e.router, "GET", &format!("/shares/{id}"), Some(&e.root), None, &[]).await;
    assert_eq!(r.status, 404, "still only for the named");
}

/// A bad or expired token on the open route is "no session", not an error.
#[tokio::test]
async fn a_bad_token_on_a_public_link_reads_as_no_session() {
    let e = env().await;
    let id = create(&e, &e.alice, json!({})).await.json()["data"]["share_id"].as_str().unwrap().to_string();
    let signed = create(&e, &e.alice, json!({"audience": "signed_in"})).await.json()["data"]["share_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(send(&e.router, "GET", &format!("/shares/{id}"), Some("garbage"), None, &[]).await.status, 200);
    assert_eq!(send(&e.router, "GET", &format!("/shares/{signed}"), Some("garbage"), None, &[]).await.status, 401);
}

/// FR-012: 30 searches a minute per person.
#[tokio::test]
async fn people_search_is_limited_to_thirty_a_minute_per_person() {
    let e = env().await;
    for _ in 0..30 {
        assert_eq!(send(&e.router, "GET", "/shares/people?q=bo", Some(&e.alice), None, &[]).await.status, 200);
    }
    let over = send(&e.router, "GET", "/shares/people?q=bo", Some(&e.alice), None, &[]).await;
    assert_eq!(over.status, 429);
    assert!(over.header("retry-after").is_some());
    // Somebody else has their own budget.
    assert_eq!(send(&e.router, "GET", "/shares/people?q=al", Some(&e.root), None, &[]).await.status, 200);
}

/// FR-039: revoking deletes the document, and the id then answers like one that never existed.
#[tokio::test]
async fn revoking_leaves_no_snapshot_behind_and_the_answers_stay_identical() {
    let e = env().await;
    let id = create(&e, &e.alice, json!({})).await.json()["data"]["share_id"].as_str().unwrap().to_string();
    assert_eq!(send(&e.router, "DELETE", &format!("/shares/{id}"), Some(&e.alice), None, &[]).await.status, 204);
    let kept = e.runtime.deps().storage.get_share(&id).await.unwrap().unwrap();
    assert_eq!(kept.snapshot_json, "");
    let gone = send(&e.router, "GET", &format!("/shares/{id}"), Some(&e.alice), None, &[]).await;
    let unknown = send(&e.router, "GET", MISS, Some(&e.alice), None, &[]).await;
    assert_eq!((gone.status, &gone.bytes), (unknown.status, &unknown.bytes));
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

#[tokio::test]
async fn list_shares_returns_caller_shares_and_enforces_admin_for_all() {
    let e = env().await;

    // Create 2 shares as Alice
    let pub_id = create(&e, &e.alice, json!({"audience": "public"})).await.json()["data"]["share_id"].as_str().unwrap().to_string();
    let spec_id = create(&e, &e.alice, json!({"audience": "specific", "people": [e.bob_id.clone()]})).await.json()["data"]["share_id"].as_str().unwrap().to_string();

    // Anonymous GET /shares -> 401
    let anon = send(&e.router, "GET", "/shares", None, None, &[]).await;
    assert_eq!(anon.status, 401);

    // Alice GET /shares -> returns 2 items, newest first, with people_count
    let alice_list = send(&e.router, "GET", "/shares", Some(&e.alice), None, &[]).await;
    assert_eq!(alice_list.status, 200);
    let items = alice_list.json()["data"].as_array().unwrap().clone();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["share_id"], spec_id);
    assert_eq!(items[0]["people_count"], 1);
    assert_eq!(items[1]["share_id"], pub_id);
    assert_eq!(items[1]["people_count"], 0);
    // Never contains snapshot content
    assert!(items[0].get("snapshot").is_none());
    assert!(items[0].get("snapshot_json").is_none());
    assert!(items[0].get("creator_name").is_none());

    // Bob GET /shares -> empty list
    let bob_list = send(&e.router, "GET", "/shares", Some(&e.bob), None, &[]).await;
    assert_eq!(bob_list.status, 200);
    assert_eq!(bob_list.json()["data"].as_array().unwrap().len(), 0);

    // Bob GET /shares?all=true -> 403 forbidden
    let bob_all = send(&e.router, "GET", "/shares?all=true", Some(&e.bob), None, &[]).await;
    assert_eq!(bob_all.status, 403);

    // Root (system) GET /shares?all=true -> 200 with creator_name
    let root_all = send(&e.router, "GET", "/shares?all=true", Some(&e.root), None, &[]).await;
    assert_eq!(root_all.status, 200);
    let all_items = root_all.json()["data"].as_array().unwrap().clone();
    assert!(all_items.len() >= 2);
    assert_eq!(all_items.iter().find(|i| i["share_id"] == pub_id).unwrap()["creator_name"], "alice");
    // FR-043: a share whose audience does not include the administrator shows no title.
    let specific = all_items.iter().find(|i| i["share_id"] == spec_id).unwrap();
    assert!(specific.get("title").is_none(), "{specific}");
    assert_eq!(specific["people_count"], 1);
    assert!(!root_all.json().to_string().contains(&e.bob_id), "no names of the people");
    assert_eq!(all_items.iter().find(|i| i["share_id"] == pub_id).unwrap()["title"], "Chat");
}

#[tokio::test]
async fn revoke_share_lifecycle_and_immediate_origin_effect() {
    let e = env().await;
    let pub_id = create(&e, &e.alice, json!({"audience": "public"})).await.json()["data"]["share_id"].as_str().unwrap().to_string();

    // Anonymous DELETE -> 401
    let anon = send(&e.router, "DELETE", &format!("/shares/{pub_id}"), None, None, &[]).await;
    assert_eq!(anon.status, 401);

    // Bob (not author, not admin) DELETE -> 404 (neutral, does not confirm ID)
    let bob_del = send(&e.router, "DELETE", &format!("/shares/{pub_id}"), Some(&e.bob), None, &[]).await;
    assert_eq!(bob_del.status, 404);

    // Non-existent ID DELETE by Alice -> 404
    let unknown_del = send(&e.router, "DELETE", "/shares/ZZZZZZZZZZZZZZZZZZZZZ", Some(&e.alice), None, &[]).await;
    assert_eq!(unknown_del.status, 404);

    // Alice (author) DELETE -> 204
    let del = send(&e.router, "DELETE", &format!("/shares/{pub_id}"), Some(&e.alice), None, &[]).await;
    assert_eq!(del.status, 204);

    // Repeated DELETE -> 204 (idempotent)
    let repeat_del = send(&e.router, "DELETE", &format!("/shares/{pub_id}"), Some(&e.alice), None, &[]).await;
    assert_eq!(repeat_del.status, 204);

    // After revoke: Anonymous GET -> 401
    let anon_get = send(&e.router, "GET", &format!("/shares/{pub_id}"), None, None, &[]).await;
    assert_eq!(anon_get.status, 401);

    // After revoke: Signed-in GET -> 404 immediately at origin
    let signed_get = send(&e.router, "GET", &format!("/shares/{pub_id}"), Some(&e.alice), None, &[]).await;
    assert_eq!(signed_get.status, 404);

    // Admin (root) can revoke someone else's share
    let pub2_id = create(&e, &e.alice, json!({"audience": "public"})).await.json()["data"]["share_id"].as_str().unwrap().to_string();
    let root_del = send(&e.router, "DELETE", &format!("/shares/{pub2_id}"), Some(&e.root), None, &[]).await;
    assert_eq!(root_del.status, 204);
}

#[tokio::test]
async fn cache_window_asserts_contract_max_age_and_no_immutable() {
    let e = env().await;
    let pub_id = create(&e, &e.alice, json!({"audience": "public"})).await.json()["data"]["share_id"].as_str().unwrap().to_string();

    let res = send(&e.router, "GET", &format!("/shares/{pub_id}"), None, None, &[]).await;
    assert_eq!(res.status, 200);
    let cc = res.header("cache-control").unwrap();
    // Must never be immutable (so revocation takes effect within 300s)
    assert!(!cc.contains("immutable"));
    assert_eq!(cc, "public, max-age=300, s-maxage=300");
}
