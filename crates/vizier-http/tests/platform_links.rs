//! Platform account linking over the real router
//! (`specs/069-platform-account-linking/contracts/http-connections.md`).
//!
//! Codes are issued as the bots issue them, then redeemed by two signed-in people.

use std::net::SocketAddr;

use axum::{
    Router,
    body::Body,
    extract::ConnectInfo,
    http::{HeaderMap, Request},
};
use serde_json::{Value, json};
use tower::ServiceExt;
use vizier_core::{
    config::VizierConfig,
    platform_links::{
        PlatformPerson, UNREGISTERED_USER, code,
        connect::{ConnectReply, issue},
        sender::resolve,
    },
    runtime::{RuntimeOptions, VizierRuntime},
    schema::ChannelPlatform,
    storage::{
        platform_link::PlatformLinkStorage,
        user::{UserProfile, UserStorage},
    },
};
use vizier_http::auth::AuthService;

struct Env {
    router: Router,
    runtime: VizierRuntime,
    rin: String,
    kai: String,
    rin_id: String,
    _dir: tempfile::TempDir,
}

async fn env() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let mut config = VizierConfig::with_workspace(dir.path().to_path_buf());
    config.channels.http.as_mut().unwrap().jwt_secret = "links-secret".into();
    config.catalogue.refresh = false;
    config.base_url = Some("https://vizier.example".into());
    let runtime = VizierRuntime::start(RuntimeOptions { config, ..Default::default() })
        .await
        .unwrap();
    let storage = runtime.deps().storage.clone();
    let http_cfg = runtime.deps().config.channels.http.clone().unwrap();
    let auth = AuthService::new(&http_cfg);
    let role = storage.create_role("plain", vec![], false).await.unwrap();
    let mut people = vec![];
    for name in ["rin", "kai"] {
        let user = storage.create_user(name, "", &role.role_id).await.unwrap();
        let token = auth.generate_token(&user.user_id, name, vec![]).unwrap();
        people.push((user.user_id, token));
    }
    let router = vizier_http::router(&runtime, &http_cfg);
    let kai = people.pop().unwrap();
    let rin = people.pop().unwrap();
    Env {
        router,
        runtime,
        rin: rin.1,
        kai: kai.1,
        rin_id: rin.0,
        _dir: dir,
    }
}

fn discord(id: &str) -> PlatformPerson {
    PlatformPerson {
        platform: ChannelPlatform::Discord,
        id: id.into(),
        username: Some(format!("handle_{id}")),
        display_name: Some(format!("Name {id}")),
    }
}

/// `/connect` from `person` in a private chat; the code as the bot shows it.
async fn connect(env: &Env, person: &PlatformPerson) -> String {
    match issue(env.runtime.deps(), person, true, None).await {
        ConnectReply::Code { code, link, .. } => {
            assert_eq!(
                link.as_deref(),
                Some(format!("https://vizier.example/connect#{code}").as_str())
            );
            code
        }
        other => panic!("expected a code, got {other:?}"),
    }
}

struct Reply {
    status: u16,
    headers: HeaderMap,
    json: Value,
}

async fn send(
    env: &Env,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
    peer: &str,
) -> Reply {
    let mut request = Request::builder().method(method).uri(format!("/api/v1{path}"));
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    let body = match body {
        Some(body) => {
            request = request.header("content-type", "application/json");
            Body::from(body.to_string())
        }
        None => Body::empty(),
    };
    let mut request = request.body(body).unwrap();
    let peer: SocketAddr = format!("{peer}:40000").parse().unwrap();
    request.extensions_mut().insert(ConnectInfo(peer));
    let response = env.router.clone().oneshot(request).await.unwrap();
    let status = response.status().as_u16();
    let headers = response.headers().clone();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20).await.unwrap();
    Reply {
        status,
        headers,
        json: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    }
}

const HOME: &str = "203.0.113.7";

async fn preview(env: &Env, token: &str, code: &str) -> Reply {
    send(env, "POST", "/auth/users/me/connections/preview", Some(token), Some(json!({ "code": code })), HOME).await
}

async fn redeem(env: &Env, token: &str, code: &str) -> Reply {
    send(env, "POST", "/auth/users/me/connections", Some(token), Some(json!({ "code": code })), HOME).await
}

async fn sender_of(env: &Env, person: PlatformPerson) -> String {
    resolve(&*env.runtime.deps().storage, person).await.label
}

#[tokio::test]
async fn a_code_is_previewed_then_redeemed_once() {
    let env = env().await;
    let shown = connect(&env, &discord("1")).await;
    assert_eq!(sender_of(&env, discord("1")).await, UNREGISTERED_USER);

    for _ in 0..2 {
        let reply = preview(&env, &env.rin, &shown.to_lowercase()).await;
        assert_eq!(reply.status, 200, "{}", reply.json);
        let data = &reply.json["data"];
        assert_eq!(data["platform"], "discord");
        assert_eq!(data["platform_user_id"], "1");
        assert_eq!(data["platform_username"], "handle_1");
        assert_eq!(data["vizier_username"], "rin");
        assert_eq!(data["replaces"], Value::Null);
    }

    let reply = redeem(&env, &env.rin, &shown).await;
    assert_eq!(reply.status, 200, "{}", reply.json);
    assert_eq!(reply.json["data"]["link"]["platform_user_id"], "1");
    assert_eq!(sender_of(&env, discord("1")).await, "rin");

    let used = redeem(&env, &env.rin, &shown).await;
    let unknown = redeem(&env, &env.rin, "ZZZZ-ZZZZ").await;
    let malformed = preview(&env, &env.rin, "nonsense").await;
    for reply in [&used, &unknown, &malformed] {
        assert_eq!(reply.status, 404, "{}", reply.json);
        assert_eq!(reply.json["code"], "connect_code_unavailable");
        assert_eq!(reply.json["message"], unknown.json["message"]);
    }

    let listing = send(&env, "GET", "/auth/users/me/connections", Some(&env.rin), None, HOME).await;
    assert_eq!(listing.json["data"]["links"][0]["platform_user_id"], "1");
    assert_eq!(listing.json["data"]["events"][0]["action"], "linked");
}

#[tokio::test]
async fn an_expired_code_reads_like_a_wrong_one() {
    let env = env().await;
    let now = vizier_core::time::now().timestamp_millis();
    env.runtime
        .deps()
        .storage
        .issue_connect_code(&discord("1"), &code::hash("AAAAAAAA"), now - 2000, now - 1000)
        .await
        .unwrap();

    let expired = redeem(&env, &env.rin, "AAAA-AAAA").await;
    let unknown = redeem(&env, &env.rin, "BBBB-BBBB").await;
    assert_eq!(expired.status, 404);
    assert_eq!(expired.json, unknown.json);
}

#[tokio::test]
async fn someone_elses_account_is_refused_without_naming_them() {
    let env = env().await;
    let shown = connect(&env, &discord("1")).await;
    assert_eq!(redeem(&env, &env.rin, &shown).await.status, 200);

    let again = connect(&env, &discord("1")).await;
    for reply in [preview(&env, &env.kai, &again).await, redeem(&env, &env.kai, &again).await] {
        assert_eq!(reply.status, 409, "{}", reply.json);
        assert_eq!(reply.json["code"], "platform_linked_elsewhere");
        assert!(!reply.json.to_string().contains("rin"), "{}", reply.json);
    }
    assert_eq!(sender_of(&env, discord("1")).await, "rin");
}

#[tokio::test]
async fn a_second_account_replaces_the_first() {
    let env = env().await;
    let first = connect(&env, &discord("1")).await;
    redeem(&env, &env.rin, &first).await;
    let second = connect(&env, &discord("2")).await;

    let reply = preview(&env, &env.rin, &second).await;
    assert_eq!(reply.json["data"]["replaces"]["platform_user_id"], "1");
    let reply = redeem(&env, &env.rin, &second).await;
    assert_eq!(reply.json["data"]["replaced"]["platform_user_id"], "1");
    assert_eq!(sender_of(&env, discord("1")).await, UNREGISTERED_USER);
    assert_eq!(sender_of(&env, discord("2")).await, "rin");
}

#[tokio::test]
async fn the_eleventh_attempt_by_one_person_is_refused() {
    let env = env().await;
    // Ten attempts from ten different addresses: only the person's own budget runs out.
    for i in 0..10 {
        let reply = send(&env, "POST", "/auth/users/me/connections", Some(&env.rin), Some(json!({ "code": "ZZZZZZZZ" })), &format!("198.51.100.{i}")).await;
        assert_eq!(reply.status, 404);
    }
    let reply = send(&env, "POST", "/auth/users/me/connections/preview", Some(&env.rin), Some(json!({ "code": "ZZZZZZZZ" })), "198.51.100.99").await;
    assert_eq!(reply.status, 429, "{}", reply.json);
    assert_eq!(reply.json["code"], "rate_limited");
    assert!(reply.headers.contains_key("retry-after"));
}

#[tokio::test]
async fn the_eleventh_attempt_from_one_address_is_refused() {
    let env = env().await;
    for i in 0..10 {
        let token = if i % 2 == 0 { &env.rin } else { &env.kai };
        assert_eq!(redeem(&env, token, "ZZZZZZZZ").await.status, 404);
    }
    assert_eq!(redeem(&env, &env.kai, "ZZZZZZZZ").await.status, 429);
}

#[tokio::test]
async fn unlinking_takes_effect_at_once_and_only_on_the_callers_own() {
    let env = env().await;
    let shown = connect(&env, &discord("1")).await;
    redeem(&env, &env.rin, &shown).await;

    let theirs = send(&env, "DELETE", "/auth/users/me/connections/discord", Some(&env.kai), None, HOME).await;
    assert_eq!((theirs.status, theirs.json["code"].as_str()), (404, Some("not_linked")));
    assert_eq!(sender_of(&env, discord("1")).await, "rin");

    let own = send(&env, "DELETE", "/auth/users/me/connections/discord", Some(&env.rin), None, HOME).await;
    assert_eq!(own.status, 204);
    assert_eq!(sender_of(&env, discord("1")).await, UNREGISTERED_USER);
    let twice = send(&env, "DELETE", "/auth/users/me/connections/discord", Some(&env.rin), None, HOME).await;
    assert_eq!(twice.status, 404);
    let other = send(&env, "DELETE", "/auth/users/me/connections/matrix", Some(&env.rin), None, HOME).await;
    assert_eq!(other.status, 404);
}

#[tokio::test]
async fn hand_typed_ids_are_refused_shown_unverified_and_removable() {
    let env = env().await;
    let storage = env.runtime.deps().storage.clone();
    storage
        .upsert_user_profile(
            &env.rin_id,
            &UserProfile {
                user_id: env.rin_id.clone(),
                discord_id: Some("999".into()),
                discord_username: Some("old".into()),
                telegram_id: None,
                telegram_username: None,
                alias: vec![],
            },
        )
        .await
        .unwrap();
    // A hand-typed id identifies nobody.
    assert_eq!(sender_of(&env, discord("999")).await, UNREGISTERED_USER);

    let refused = send(&env, "PUT", "/auth/users/me/profile", Some(&env.rin), Some(json!({ "discord_id": "555" })), HOME).await;
    assert_eq!((refused.status, refused.json["code"].as_str()), (422, Some("connect_required")));
    // `null` keeps what is stored, and the aliases still save.
    let saved = send(&env, "PUT", "/auth/users/me/profile", Some(&env.rin), Some(json!({ "discord_id": null, "alias": ["Rin"] })), HOME).await;
    assert_eq!(saved.status, 200, "{}", saved.json);
    assert_eq!(saved.json["data"]["discord_id"], "999");
    assert_eq!(saved.json["data"]["alias"][0], "Rin");

    let listing = send(&env, "GET", "/auth/users/me/connections", Some(&env.rin), None, HOME).await;
    assert_eq!(listing.json["data"]["unverified"][0]["platform_user_id"], "999");

    let cleared = send(&env, "DELETE", "/auth/users/me/connections/discord/unverified", Some(&env.rin), None, HOME).await;
    assert_eq!(cleared.status, 204);
    let listing = send(&env, "GET", "/auth/users/me/connections", Some(&env.rin), None, HOME).await;
    assert_eq!(listing.json["data"]["unverified"], json!([]));
}

#[tokio::test]
async fn a_deleted_persons_accounts_read_as_unregistered() {
    let env = env().await;
    let shown = connect(&env, &discord("1")).await;
    redeem(&env, &env.rin, &shown).await;

    env.runtime.deps().storage.delete_user(&env.rin_id).await.unwrap();

    assert_eq!(sender_of(&env, discord("1")).await, UNREGISTERED_USER);
}

#[tokio::test]
async fn a_group_chat_gets_no_code_and_one_account_is_limited() {
    let env = env().await;
    let asked = issue(env.runtime.deps(), &discord("1"), false, Some("https://t.me/bot".into())).await;
    assert_eq!(asked, ConnectReply::AskInPrivate { bot_link: Some("https://t.me/bot".into()) });
    for _ in 0..5 {
        connect(&env, &discord("1")).await;
    }
    assert!(matches!(
        issue(env.runtime.deps(), &discord("1"), true, None).await,
        ConnectReply::TooMany { .. }
    ));
    // Another account has its own budget.
    connect(&env, &discord("2")).await;
}

#[tokio::test]
async fn nobody_signed_out_reaches_the_connections() {
    let env = env().await;
    for (method, path) in [
        ("GET", "/auth/users/me/connections"),
        ("POST", "/auth/users/me/connections"),
        ("POST", "/auth/users/me/connections/preview"),
        ("DELETE", "/auth/users/me/connections/discord"),
        ("DELETE", "/auth/users/me/connections/discord/unverified"),
    ] {
        let reply = send(&env, method, path, None, Some(json!({ "code": "ZZZZZZZZ" })), HOME).await;
        assert_eq!(reply.status, 401, "{method} {path}");
    }
}
