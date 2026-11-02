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
use axum::http::{StatusCode, Uri};
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
}

type Shared = Arc<Mutex<Script>>;

async fn handle(State(shared): State<Shared>, uri: Uri, body: Bytes) -> Response {
    let path = uri.path().to_string();
    let body = String::from_utf8_lossy(&body).to_string();
    let reply = {
        let mut script = shared.lock().unwrap();
        script.seen.push((path.clone(), body));
        let route = script.routes.get(&path).cloned();
        match route {
            None => None,
            Some(list) => {
                let cursor = script.cursors.entry(path).or_insert(0);
                let index = (*cursor).min(list.len() - 1);
                *cursor += 1;
                Some(list[index].clone())
            }
        }
    };
    match reply {
        Some((status, body)) => (
            StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            body,
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "{}").into_response(),
    }
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

    pub fn base(&self) -> &str {
        &self.base
    }

    /// Every request the stub saw, as `(path, body)`.
    pub fn seen(&self) -> Vec<(String, String)> {
        self.shared.lock().unwrap().seen.clone()
    }

    /// How many requests reached `path` — the proof that a second confirmation never went out.
    pub fn path_count(&self, path: &str) -> usize {
        self.seen().iter().filter(|(seen, _)| seen == path).count()
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
