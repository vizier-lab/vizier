//! Test-only support for the SSO flow: the loopback stub server of research D9 and an in-memory
//! [`SsoSessionStorage`] so a test can assert what was — and was not — written.
//!
//! The stub is an `axum` router on `127.0.0.1:0`, the same shape the provider transport tests
//! already use (`llm::transport_tests`), so the flow's HTTP client speaks to a real socket and no
//! new dependency is introduced. Each path is scripted with one or more replies, consumed in
//! order; the last one repeats, so "pending forever" is one reply rather than a count.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::{Result, bail};
use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::schema::SsoSessionSecret;
use crate::storage::sso_session::{SsoSessionStorage, SsoWrite};

/// What a device-flow test wants `/token` to answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenReply {
    Approved,
    Pending,
    Denied,
    /// `400 invalid_grant`: this device code was already used.
    Used,
}

pub fn approved() -> TokenReply {
    TokenReply::Approved
}

pub fn pending() -> TokenReply {
    TokenReply::Pending
}

pub fn denied() -> TokenReply {
    TokenReply::Denied
}

pub fn used_twice() -> TokenReply {
    TokenReply::Used
}

/// One scripted reply.
#[derive(Debug, Clone)]
pub struct Reply {
    pub status: u16,
    pub body: serde_json::Value,
}

impl Reply {
    pub fn ok(body: serde_json::Value) -> Self {
        Self { status: 200, body }
    }

    /// A refusal carrying only a message — the shape a portal error takes.
    pub fn refusing(status: u16) -> Self {
        Self {
            status,
            body: json!({"message": "refused"}),
        }
    }
}

#[derive(Default)]
struct Script {
    routes: HashMap<String, Vec<(u16, String)>>,
    cursors: HashMap<String, usize>,
    seen: Vec<(String, String)>,
    /// `(path, Content-Type)` per request; empty when the request carried none. Lets a test prove
    /// the wire is JSON rather than form-encoded (044).
    content_types: Vec<(String, String)>,
    /// `(path, Authorization)` per request that carried one — which credential signed it.
    authorizations: Vec<(String, String)>,
}

type Shared = Arc<Mutex<Script>>;

async fn handle(
    State(shared): State<Shared>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let path = uri.path().to_string();
    let body = String::from_utf8_lossy(&body).to_string();
    let content_type = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let authorization = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let reply = {
        let mut script = shared.lock().unwrap();
        script.seen.push((path.clone(), body.clone()));
        script
            .content_types
            .push((path.clone(), content_type.clone()));
        if let Some(authorization) = authorization {
            script.authorizations.push((path.clone(), authorization));
        }
        let route = script.routes.get(&path).cloned();
        match route {
            None => None,
            Some(list) => {
                let cursor = script.cursors.entry(path.clone()).or_insert(0);
                let index = (*cursor).min(list.len() - 1);
                *cursor += 1;
                Some(list[index].clone())
            }
        }
    };
    // A 2xx the script prepared must only be earned: the OIDC endpoints are rest-json with
    // camelCase keys, which is exactly the mistake the old stub let through (044). A scripted
    // refusal is answered as written — the retry and `authorization_pending` tests rely on it.
    if method == Method::POST {
        if let Some((status, _)) = &reply {
            if (200..300).contains(status) {
                if let Some(problem) = wire_problem(&path, &content_type, &body) {
                    return (
                        StatusCode::BAD_REQUEST,
                        [(axum::http::header::CONTENT_TYPE, "application/json")],
                        json!({
                            "error": "InvalidRequestException",
                            "message": problem,
                        })
                        .to_string(),
                    )
                        .into_response();
                }
            }
        }
    }
    match reply {
        Some((status, body)) => (
            StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            body,
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "{}").into_response(),
    }
}

/// What is wrong with a request body AWS would answer `400 InvalidRequestException` to, if
/// anything: the `Content-Type`, then the camelCase keys the endpoint reads, all non-empty.
/// `/client/register` registers a client and has no credentials yet, so its required keys are its
/// own; the other two carry `clientId` and `clientSecret`. A GET (§4 portal) is never checked.
fn wire_problem(path: &str, content_type: &str, body: &str) -> Option<String> {
    let is_json = content_type
        .split(';')
        .next()
        .map(|mime| mime.trim().eq_ignore_ascii_case("application/json"))
        .unwrap_or(false);
    if !is_json {
        return Some(format!(
            "Content-Type {content_type:?} is not application/json"
        ));
    }
    let value: serde_json::Value = match serde_json::from_str(body) {
        Ok(value) => value,
        Err(_) => return Some("the request body is not JSON".to_string()),
    };
    let Some(object) = value.as_object() else {
        return Some("the request body is not a JSON object".to_string());
    };
    let text = |key: &str| -> Option<String> {
        object
            .get(key)
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .filter(|value| !value.trim().is_empty())
    };
    let required: Vec<&str> = match path {
        "/client/register" => vec!["clientName", "clientType"],
        "/device_authorization" => vec!["clientId", "clientSecret", "startUrl"],
        "/token" => {
            let mut required = vec!["clientId", "clientSecret", "grantType"];
            match object.get("grantType").and_then(|value| value.as_str()) {
                Some("urn:ietf:params:oauth:grant-type:device_code") => {
                    required.push("deviceCode");
                }
                Some("refresh_token") => required.push("refreshToken"),
                Some(other) => return Some(format!("unsupported grantType {other:?}")),
                None => {}
            }
            required
        }
        _ => return None,
    };
    for key in required {
        if text(key).is_none() {
            return Some(format!("{key} is missing or empty"));
        }
    }
    None
}

/// A stub server standing in for one SSO region.
pub struct Stub {
    base: String,
    shared: Shared,
}

impl Stub {
    async fn with_routes(routes: Vec<(&str, Vec<(u16, String)>)>) -> Self {
        let mut script = Script::default();
        for (path, replies) in routes {
            script.routes.insert(path.to_string(), replies);
        }
        let shared = Arc::new(Mutex::new(script));
        let app = Router::new().fallback(handle).with_state(shared.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self { base, shared }
    }

    /// The device-flow stub: `/device_authorization` always answers a ten-minute code, and
    /// `/token` walks `replies` (the last one repeats).
    pub async fn start(replies: Vec<TokenReply>) -> Self {
        Self::start_with_code_life(600, replies).await
    }

    /// The same with the code's own life shortened, for the expiry case.
    pub async fn start_with_code_life(code_life_seconds: u64, replies: Vec<TokenReply>) -> Self {
        let device_authorization = json!({
            "deviceCode": "device-code-value",
            "userCode": "ABCD-EFGH",
            "verificationUri": "https://example.test/device",
            "verificationUriComplete": "https://example.test/device?user_code=ABCD-EFGH",
            "expiresIn": code_life_seconds,
            "interval": 0,
        });
        let tokens = replies
            .into_iter()
            .map(|reply| match reply {
                TokenReply::Approved => (
                    200,
                    json!({
                        "accessToken": "access-token-value",
                        "refreshToken": "refresh-token-value",
                        "expiresIn": 3600,
                    })
                    .to_string(),
                ),
                TokenReply::Pending => (
                    400,
                    json!({"error": "authorization_pending"}).to_string(),
                ),
                TokenReply::Denied => (400, json!({"error": "access_denied"}).to_string()),
                TokenReply::Used => (400, json!({"error": "invalid_grant"}).to_string()),
            })
            .collect();
        let registration = json!({
            "clientId": "registered-client-id",
            "clientSecret": "registered-client-secret-value",
            "clientSecretExpiresAt": 0,
        });
        Self::with_routes(vec![
            ("/client/register", vec![(200, registration.to_string())]),
            (
                "/device_authorization",
                vec![(200, device_authorization.to_string())],
            ),
            ("/token", tokens),
        ])
        .await
    }

    /// The portal stub: one reply each for the three §4 endpoints.
    pub async fn start_portal(accounts: Reply, roles: Reply, credential: Reply) -> Self {
        Self::with_routes(vec![
            (
                "/assignment/accounts",
                vec![(accounts.status, accounts.body.to_string())],
            ),
            (
                "/assignment/accounts/123456789012/roles",
                vec![(roles.status, roles.body.to_string())],
            ),
            (
                "/federation/credentials",
                vec![(credential.status, credential.body.to_string())],
            ),
        ])
        .await
    }

    /// The same script written with [`Reply`]s, for a combination this harness does not model:
    /// a renewal's `/token` next to `/federation/credentials`, or a retried invoke.
    pub async fn start_replies(routes: Vec<(&str, Vec<Reply>)>) -> Self {
        Self::with_routes(
            routes
                .into_iter()
                .map(|(path, replies)| {
                    (
                        path,
                        replies
                            .into_iter()
                            .map(|reply| (reply.status, reply.body.to_string()))
                            .collect(),
                    )
                })
                .collect(),
        )
        .await
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    /// Every request the stub saw, as `(path, body)`.
    pub fn seen(&self) -> Vec<(String, String)> {
        self.shared.lock().unwrap().seen.clone()
    }

    /// The `Content-Type` of every request the stub saw, as `(path, value)` — empty when the
    /// request carried none. The proof that the wire is JSON, not form-encoded (044).
    pub fn content_types(&self) -> Vec<(String, String)> {
        self.shared.lock().unwrap().content_types.clone()
    }

    /// How many requests reached `path` — the proof that a second confirmation never went out.
    pub fn path_count(&self, path: &str) -> usize {
        self.seen().iter().filter(|(seen, _)| seen == path).count()
    }

    /// The `Authorization` header of every request that carried one, in order — which credential
    /// signed a retried request, not just that it was sent.
    pub fn authorizations(&self) -> Vec<String> {
        self.shared
            .lock()
            .unwrap()
            .authorizations
            .iter()
            .map(|(_, authorization)| authorization.clone())
            .collect()
    }
}

/// An in-memory [`SsoSessionStorage`] with the trait's revision semantics, so a test can assert
/// either the row a flow wrote or the absence of one.
#[derive(Default)]
pub struct MemSessions {
    rows: Mutex<HashMap<String, (SsoSessionSecret, u64)>>,
    unreadable: Mutex<Vec<String>>,
}

impl MemSessions {
    pub fn len(&self) -> usize {
        self.rows.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Make reads of `profile` fail as a wrong workspace key would (FR-009f).
    pub fn mark_unreadable(&self, profile: &str) {
        self.unreadable.lock().unwrap().push(profile.to_string());
    }
}

#[async_trait::async_trait]
impl SsoSessionStorage for MemSessions {
    async fn get_sso_session(&self, profile: &str) -> Result<Option<SsoSessionSecret>> {
        if self
            .unreadable
            .lock()
            .unwrap()
            .iter()
            .any(|marked| marked == profile)
        {
            bail!("the envelope for {profile} cannot be opened with this key");
        }
        Ok(self
            .rows
            .lock()
            .unwrap()
            .get(profile)
            .map(|(secret, _)| secret.clone()))
    }

    async fn put_sso_session(
        &self,
        profile: &str,
        secret: &SsoSessionSecret,
        expected_revision: Option<u64>,
    ) -> Result<SsoWrite> {
        let mut rows = self.rows.lock().unwrap();
        let current = rows.get(profile).map(|(_, revision)| *revision);
        if current != expected_revision {
            return Ok(SsoWrite::Conflict);
        }
        let next = expected_revision.unwrap_or(0) + 1;
        let mut stored = secret.clone();
        stored.revision = next;
        rows.insert(profile.to_string(), (stored, next));
        Ok(SsoWrite::Written(next))
    }

    async fn delete_sso_session(&self, profile: &str) -> Result<()> {
        self.rows.lock().unwrap().remove(profile);
        Ok(())
    }
}
