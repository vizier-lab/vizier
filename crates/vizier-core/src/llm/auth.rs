//! Turning stored credentials into request auth (contract `provider-layer.md` § Auth). Provider
//! quirks that are not wire format live here, as `CredentialSource` impls, never as provider
//! checks inside a dialect (research R5).

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine;
use reqwest::RequestBuilder;
use serde::Deserialize;
use tokio::sync::Mutex;

use super::error::LlmError;
use super::http;
use super::sigv4::{self, SigV4Request};
use super::sso::{RoleCredentials, SsoClient, SsoError, SsoPortal, SsoSessions};
use crate::storage::sso_session::SsoSessionStorage;

/// What a credential source may need to know about the request it authorises.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RequestInfo {
    /// The conversation already holds an assistant turn or a tool result.
    pub agent_initiated: bool,
    /// The request carries an image.
    pub has_images: bool,
}

#[async_trait::async_trait]
pub trait CredentialSource: Send + Sync {
    async fn authorize(
        &self,
        req: RequestBuilder,
        info: &RequestInfo,
    ) -> Result<RequestBuilder, LlmError>;

    /// [`CredentialSource::authorize`] for a source that signs the request body as well as its
    /// headers (AWS SigV4). The default keeps every existing source unchanged; the bytes given
    /// here are the exact bytes the caller sends.
    async fn authorize_with_body(
        &self,
        req: RequestBuilder,
        info: &RequestInfo,
        _method: &str,
        _url: &str,
        _body: &[u8],
    ) -> Result<RequestBuilder, LlmError> {
        self.authorize(req, info).await
    }

    /// A base URL that replaces the configured one, when the credential decides it (Copilot
    /// tokens name their own API host).
    async fn base_url(&self) -> Result<Option<String>, LlmError> {
        Ok(None)
    }

    /// Drop a cached token after a 401. Returns whether anything was dropped, i.e. whether a
    /// retry can succeed.
    async fn invalidate(&self) -> bool {
        false
    }
}

/// How a static key is sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyStyle {
    Bearer,
    Header(&'static str),
    Query(&'static str),
    None,
}

/// A fixed key: a bearer token, a named header, a query parameter, or nothing.
pub struct StaticKey {
    pub style: KeyStyle,
    pub key: Option<String>,
}

impl StaticKey {
    pub fn shared(style: KeyStyle, key: Option<String>) -> Arc<dyn CredentialSource> {
        Arc::new(Self {
            style,
            key: key.filter(|k| !k.is_empty()),
        })
    }

    pub fn none() -> Arc<dyn CredentialSource> {
        Self::shared(KeyStyle::None, None)
    }
}

#[async_trait::async_trait]
impl CredentialSource for StaticKey {
    async fn authorize(
        &self,
        req: RequestBuilder,
        _info: &RequestInfo,
    ) -> Result<RequestBuilder, LlmError> {
        let Some(key) = &self.key else {
            return Ok(req);
        };
        Ok(match self.style {
            KeyStyle::Bearer => req.bearer_auth(key),
            KeyStyle::Header(name) => req.header(name, key),
            KeyStyle::Query(name) => req.query(&[(name, key)]),
            KeyStyle::None => req,
        })
    }
}

// AWS Signature Version 4 (Bedrock).

/// Signs each request with an AWS access key pair and a region (contract `provider-layer.md` §2).
/// The body-aware path is the only one that can sign, because the payload hash covers the exact
/// bytes sent; calling [`CredentialSource::authorize`] without a body is a configuration error,
/// never an unsigned request.
pub struct SigV4Credential {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub session_token: Option<String>,
    pub region: String,
    pub service: &'static str,
}

impl SigV4Credential {
    /// A Bedrock credential (`service = "bedrock"`), ready for `ResolvedModel.credential`.
    pub fn bedrock(
        access_key_id: String,
        secret_access_key: String,
        session_token: Option<String>,
        region: String,
    ) -> Arc<dyn CredentialSource> {
        Arc::new(Self {
            access_key_id,
            secret_access_key,
            session_token: session_token.filter(|t| !t.is_empty()),
            region,
            service: super::bedrock::SERVICE,
        })
    }

    /// The host to sign: `host[:port]` with an explicit non-default port.
    fn sign_host(url: &reqwest::Url) -> Result<String, LlmError> {
        let host = url
            .host_str()
            .ok_or_else(|| LlmError::Config(format!("the request URL {url} has no host")))?;
        Ok(match url.port() {
            Some(port) => format!("{host}:{port}"),
            None => host.to_string(),
        })
    }
}

impl std::fmt::Debug for SigV4Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SigV4Credential")
            .field("access_key_id", &self.access_key_id)
            .field("secret_access_key", &"[redacted]")
            .field(
                "session_token",
                &self.session_token.as_ref().map(|_| "[redacted]"),
            )
            .field("region", &self.region)
            .field("service", &self.service)
            .finish()
    }
}

#[async_trait::async_trait]
impl CredentialSource for SigV4Credential {
    async fn authorize(
        &self,
        _req: RequestBuilder,
        _info: &RequestInfo,
    ) -> Result<RequestBuilder, LlmError> {
        Err(LlmError::Config(SIGV4_BODY_REQUIRED.into()))
    }

    async fn authorize_with_body(
        &self,
        req: RequestBuilder,
        _info: &RequestInfo,
        method: &str,
        url: &str,
        body: &[u8],
    ) -> Result<RequestBuilder, LlmError> {
        let parsed = reqwest::Url::parse(url)
            .map_err(|e| LlmError::Config(format!("cannot sign the request URL {url}: {e}")))?;
        let host = Self::sign_host(&parsed)?;
        let signature = sigv4::sign(&SigV4Request {
            method,
            path: parsed.path(),
            query: parsed.query().unwrap_or(""),
            host: &host,
            content_type: Some("application/json"),
            body,
            access_key_id: &self.access_key_id,
            secret_access_key: &self.secret_access_key,
            session_token: self.session_token.as_deref(),
            region: &self.region,
            service: self.service,
            now: chrono::Utc::now(),
        });
        let mut req = req
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header("x-amz-date", signature.amz_date)
            .header(reqwest::header::AUTHORIZATION, signature.authorization);
        if let Some(token) = &self.session_token {
            req = req.header("x-amz-security-token", token);
        }
        Ok(req)
    }
}

// AWS Bedrock over IAM Identity Center (spec 044).

/// Why a SigV4 signature cannot be computed without the body: the payload hash covers the exact
/// bytes sent, so an unsigned-body request would be a configuration error, never a request with a
/// missing header.
const SIGV4_BODY_REQUIRED: &str = "SigV4 signing requires the request body; use the body-aware path";

/// The half of an SSO connection a role credential needs: which stored session, which role, and
/// the Bedrock region the signed requests go to. Every field is non-secret.
#[derive(Debug, Clone)]
pub struct SsoRoleTarget {
    /// The profile name the session row is keyed by (FR-009e).
    pub profile: String,
    pub account_id: String,
    pub role_name: String,
    /// The **Bedrock** region — the SSO region lives with the client (FR-010).
    pub region: String,
}

/// A Bedrock credential whose three halves are derived from an IAM Identity Center session
/// (FR-002, FR-009d, research D5).
///
/// The role credentials live in [`SsoRoleCredential::cache`] — one mutex per connection, so
/// concurrent turns collapse into one renewal (FR-014) — and nowhere else: nothing here can be
/// written to disk. Signing is delegated to the same [`SigV4Credential`] the static-keys provider
/// uses, so the wire behaviour is shared rather than copied.
pub struct SsoRoleCredential {
    client: SsoClient,
    storage: Arc<dyn SsoSessionStorage + Send + Sync>,
    target: SsoRoleTarget,
    cache: Mutex<Option<RoleCredentials>>,
}

impl SsoRoleCredential {
    pub fn new(
        client: SsoClient,
        storage: Arc<dyn SsoSessionStorage + Send + Sync>,
        target: SsoRoleTarget,
    ) -> Self {
        Self {
            client,
            storage,
            target,
            cache: Mutex::new(None),
        }
    }

    /// How long before expiry a cached credential is renewed: one call every five minutes at
    /// most, and zero calls while the cache is good (FR-013).
    const RENEW_BEFORE: chrono::TimeDelta = chrono::TimeDelta::minutes(5);

    /// The role's three halves, from the cache or one portal call.
    ///
    /// The whole path runs under the cache mutex, so two turns near expiry make one renewal and
    /// the second uses its result. An expired access token is refreshed through the session's own
    /// refresh token first (`SsoSessions::access_token`, §5).
    async fn role_credentials(&self) -> Result<RoleCredentials, LlmError> {
        let mut cache = self.cache.lock().await;
        if let Some(cached) = cache.as_ref()
            && cached.expires_at - Self::RENEW_BEFORE > chrono::Utc::now()
        {
            return Ok(cached.clone());
        }

        let sessions = SsoSessions::new(self.storage.as_ref());
        let session = sessions
            .load(&self.target.profile)
            .await?
            .ok_or(SsoError::SessionExpired)?;
        let session = sessions
            .access_token(&self.client, &self.target.profile, session)
            .await?;
        let credentials = SsoPortal::new(self.client.clone())
            .role_credentials(
                &session.access_token,
                &self.target.account_id,
                &self.target.role_name,
            )
            .await?;
        *cache = Some(credentials.clone());
        Ok(credentials)
    }
}

impl std::fmt::Debug for SsoRoleCredential {
    /// Names the connection and the expiry, never a credential half (research D8.3, FR-035).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SsoRoleCredential")
            .field("profile", &self.target.profile)
            .field("account_id", &self.target.account_id)
            .field("role_name", &self.target.role_name)
            .field("region", &self.target.region)
            .field("cached", &self.cache.try_lock().map(|c| c.is_some()).unwrap_or(true))
            .finish()
    }
}

#[async_trait::async_trait]
impl CredentialSource for SsoRoleCredential {
    async fn authorize(
        &self,
        _req: RequestBuilder,
        _info: &RequestInfo,
    ) -> Result<RequestBuilder, LlmError> {
        Err(LlmError::Config(SIGV4_BODY_REQUIRED.into()))
    }

    async fn authorize_with_body(
        &self,
        req: RequestBuilder,
        info: &RequestInfo,
        method: &str,
        url: &str,
        body: &[u8],
    ) -> Result<RequestBuilder, LlmError> {
        let credentials = self.role_credentials().await?;
        let signer = SigV4Credential {
            access_key_id: credentials.access_key_id,
            secret_access_key: credentials.secret_access_key,
            session_token: Some(credentials.session_token),
            region: self.target.region.clone(),
            service: super::bedrock::SERVICE,
        };
        signer
            .authorize_with_body(req, info, method, url, body)
            .await
    }

    /// A 401 clears the cache so the existing `complete_once` retry renews once and retries once
    /// (FR-015). `false` when nothing was cached: a retry could not succeed.
    async fn invalidate(&self) -> bool {
        self.cache.lock().await.take().is_some()
    }
}

// GitHub Copilot. Ported from rig-core 0.39 `providers/copilot`.

pub const COPILOT_API_BASE_URL: &str = "https://api.githubcopilot.com";
const COPILOT_TOKEN_URL: &str = "https://api.github.com/copilot_internal/v2/token";
const COPILOT_EDITOR_VERSION: &str = "vscode/1.107.0";
const COPILOT_EDITOR_PLUGIN_VERSION: &str = "copilot-chat/0.35.0";
const COPILOT_USER_AGENT: &str = "GitHubCopilotChat/0.35.0";
const COPILOT_API_VERSION: &str = "2025-04-01";

#[derive(Clone)]
struct CopilotToken {
    token: String,
    /// Unix seconds.
    expires_at: Option<i64>,
    api_base: Option<String>,
}

/// GitHub Copilot: trades a GitHub token for a short-lived Copilot token, cached until it
/// expires (minus a minute), and re-fetched after a 401. A key that already is a Copilot
/// token (`tid=…;exp=…`, which rig 0.39's `CopilotAuth::ApiKey` sent as-is) is used directly.
pub struct CopilotExchange {
    http: reqwest::Client,
    github_token: String,
    configured_base_url: String,
    token_url: String,
    cached: Mutex<Option<CopilotToken>>,
}

impl CopilotExchange {
    pub fn new(http: reqwest::Client, key: String, configured_base_url: String) -> Arc<Self> {
        Self::with_token_url(http, key, configured_base_url, COPILOT_TOKEN_URL.to_string())
    }

    pub fn with_token_url(
        http: reqwest::Client,
        key: String,
        configured_base_url: String,
        token_url: String,
    ) -> Arc<Self> {
        Arc::new(Self {
            http,
            github_token: key,
            configured_base_url,
            token_url,
            cached: Mutex::new(None),
        })
    }

    fn is_copilot_token(key: &str) -> bool {
        key.starts_with("tid=") || key.contains(";exp=")
    }

    async fn token(&self) -> Result<CopilotToken, LlmError> {
        if Self::is_copilot_token(&self.github_token) {
            return Ok(CopilotToken {
                token: self.github_token.clone(),
                expires_at: None,
                api_base: None,
            });
        }

        let mut cached = self.cached.lock().await;
        let now = unix_now();
        if let Some(t) = cached.as_ref()
            && t.expires_at.is_none_or(|exp| exp - 60 > now)
        {
            return Ok(t.clone());
        }

        #[derive(Deserialize)]
        struct Endpoints {
            api: Option<String>,
        }
        #[derive(Deserialize)]
        struct TokenResponse {
            token: Option<String>,
            expires_at: Option<i64>,
            endpoints: Option<Endpoints>,
        }

        let req = self
            .http
            .get(&self.token_url)
            .header(reqwest::header::ACCEPT, "application/json")
            .header("editor-version", COPILOT_EDITOR_VERSION)
            .header("editor-plugin-version", COPILOT_EDITOR_PLUGIN_VERSION)
            .header(reqwest::header::USER_AGENT, COPILOT_USER_AGENT)
            .header(
                reqwest::header::AUTHORIZATION,
                format!("token {}", self.github_token),
            )
            .timeout(Duration::from_secs(30));
        let body = http::send_json(req).await?;
        let parsed: TokenResponse = serde_json::from_value(body)
            .map_err(|e| LlmError::Decode(format!("copilot token response: {e}")))?;
        let token = parsed.token.ok_or_else(|| {
            LlmError::Decode("the GitHub Copilot token response did not include a token".into())
        })?;
        let fresh = CopilotToken {
            token,
            expires_at: parsed.expires_at,
            api_base: parsed.endpoints.and_then(|e| e.api),
        };
        *cached = Some(fresh.clone());
        Ok(fresh)
    }
}

#[async_trait::async_trait]
impl CredentialSource for CopilotExchange {
    async fn authorize(
        &self,
        req: RequestBuilder,
        info: &RequestInfo,
    ) -> Result<RequestBuilder, LlmError> {
        let token = self.token().await?;
        let mut req = req
            .bearer_auth(&token.token)
            .header("copilot-integration-id", "vscode-chat")
            .header("editor-version", COPILOT_EDITOR_VERSION)
            .header("editor-plugin-version", COPILOT_EDITOR_PLUGIN_VERSION)
            .header(reqwest::header::USER_AGENT, COPILOT_USER_AGENT)
            .header("openai-intent", "conversation-panel")
            .header("x-github-api-version", COPILOT_API_VERSION)
            .header("x-request-id", nanoid::nanoid!())
            .header("x-vscode-user-agent-library-version", "electron-fetch")
            .header(
                "X-Initiator",
                if info.agent_initiated { "agent" } else { "user" },
            );
        if info.has_images {
            req = req.header("copilot-vision-request", "true");
        }
        Ok(req)
    }

    async fn base_url(&self) -> Result<Option<String>, LlmError> {
        if self.configured_base_url.trim_end_matches('/') != COPILOT_API_BASE_URL {
            return Ok(None);
        }
        let token = self.token().await?;
        Ok(token
            .api_base
            .or_else(|| copilot_base_url_from_token(&token.token)))
    }

    async fn invalidate(&self) -> bool {
        self.cached.lock().await.take().is_some()
    }
}

/// `proxy-ep=proxy.<x>.githubcopilot.com` in a Copilot token → `https://api.<x>.githubcopilot.com`,
/// accepted only for https Copilot hosts with no path.
fn copilot_base_url_from_token(token: &str) -> Option<String> {
    let proxy_ep = token
        .split(';')
        .find_map(|part| part.trim().strip_prefix("proxy-ep="))?
        .trim();
    if proxy_ep.is_empty() {
        return None;
    }
    let candidate = if proxy_ep.contains("://") {
        proxy_ep.to_string()
    } else {
        format!("https://{proxy_ep}")
    };
    let url = reqwest::Url::parse(&candidate).ok()?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    let host = url.host_str()?.to_ascii_lowercase();
    if host != "githubcopilot.com" && !host.ends_with(".githubcopilot.com") {
        return None;
    }
    let api_host = host
        .strip_prefix("proxy.")
        .map(|suffix| format!("api.{suffix}"))
        .unwrap_or(host);
    Some(format!("https://{api_host}"))
}

// ChatGPT subscription (Codex backend). Ported from rig-core 0.39 `providers/chatgpt`.

/// A ChatGPT access token plus the account id header. The account id, when not given, is read
/// from the token's claims, as rig did.
pub struct ChatGptAccount {
    pub access_token: String,
    pub account_id: Option<String>,
    originator: String,
    user_agent: String,
}

impl ChatGptAccount {
    pub fn new(access_token: String, account_id: Option<String>) -> Arc<Self> {
        let account_id = account_id
            .filter(|a| !a.is_empty())
            .or_else(|| chatgpt_account_id_from_jwt(&access_token));
        let originator = std::env::var("CHATGPT_ORIGINATOR")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "vizier".to_string());
        let user_agent = std::env::var("CHATGPT_USER_AGENT")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| {
                format!(
                    "vizier/{} ({} {}; {originator})",
                    env!("CARGO_PKG_VERSION"),
                    std::env::consts::OS,
                    std::env::consts::ARCH,
                )
            });
        Arc::new(Self {
            access_token,
            account_id,
            originator,
            user_agent,
        })
    }
}

#[async_trait::async_trait]
impl CredentialSource for ChatGptAccount {
    async fn authorize(
        &self,
        req: RequestBuilder,
        _info: &RequestInfo,
    ) -> Result<RequestBuilder, LlmError> {
        let mut req = req
            .bearer_auth(&self.access_token)
            .header("originator", &self.originator)
            .header(reqwest::header::USER_AGENT, &self.user_agent)
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .header("session_id", nanoid::nanoid!());
        if let Some(account) = &self.account_id {
            req = req.header("ChatGPT-Account-Id", account);
        }
        Ok(req)
    }
}

fn chatgpt_account_id_from_jwt(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .ok()?;
    let claims: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    claims
        .get("https://api.openai.com/auth")?
        .get("chatgpt_account_id")?
        .as_str()
        .map(str::to_owned)
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copilot_base_url_from_token_derives_the_api_host() {
        assert_eq!(
            copilot_base_url_from_token("tid=1;proxy-ep=proxy.individual.githubcopilot.com;exp=2")
                .as_deref(),
            Some("https://api.individual.githubcopilot.com")
        );
        assert_eq!(
            copilot_base_url_from_token(
                "tid=1;proxy-ep=https://proxy.individual.githubcopilot.com;exp=2"
            )
            .as_deref(),
            Some("https://api.individual.githubcopilot.com")
        );
        assert_eq!(copilot_base_url_from_token("tid=1;exp=2"), None);
        assert_eq!(
            copilot_base_url_from_token(
                "tid=1;proxy-ep=http://proxy.individual.githubcopilot.com;exp=2"
            ),
            None
        );
        assert_eq!(
            copilot_base_url_from_token("tid=1;proxy-ep=https://evil.example.com;exp=2"),
            None
        );
        assert_eq!(
            copilot_base_url_from_token(
                "tid=1;proxy-ep=https://proxy.individual.githubcopilot.com/base;exp=2"
            ),
            None
        );
    }

    #[test]
    fn chatgpt_account_id_is_read_from_the_token() {
        let claims = serde_json::json!({"https://api.openai.com/auth": {"chatgpt_account_id": "acct_1"}});
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&claims).unwrap());
        let token = format!("h.{payload}.s");
        assert_eq!(chatgpt_account_id_from_jwt(&token).as_deref(), Some("acct_1"));
        assert_eq!(chatgpt_account_id_from_jwt("opaque"), None);
    }

    fn bedrock_credential() -> SigV4Credential {
        SigV4Credential {
            access_key_id: "AKIDEXAMPLE".into(),
            secret_access_key: "secret-access-key".into(),
            session_token: Some("session-token".into()),
            region: "us-east-1".into(),
            service: crate::llm::bedrock::SERVICE,
        }
    }

    #[tokio::test]
    async fn sigv4_needs_a_body_and_signs_the_headers() {
        let credential = bedrock_credential();
        let url = "https://bedrock-runtime.us-east-1.amazonaws.com/model/amazon.nova-lite-v1%3A0/converse";
        let unsigned = reqwest::Client::new().post(url);
        let err = credential
            .authorize(unsigned, &RequestInfo::default())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("body-aware"));

        let req = reqwest::Client::new().post(url);
        let built = credential
            .authorize_with_body(req, &RequestInfo::default(), "POST", url, br#"{"messages":[]}"#)
            .await
            .unwrap()
            .build()
            .unwrap();
        let authorization = built
            .headers()
            .get(reqwest::header::AUTHORIZATION)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(
            authorization.starts_with("AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/"),
            "{authorization}"
        );
        assert!(authorization.contains("Signature="));
        assert_eq!(
            built
                .headers()
                .get("x-amz-security-token")
                .unwrap()
                .to_str()
                .unwrap(),
            "session-token"
        );
        assert!(built.headers().get("x-amz-date").is_some());
        assert_eq!(
            built.headers().get(reqwest::header::CONTENT_TYPE).unwrap(),
            "application/json"
        );
    }

    fn sso_session(expires_at: chrono::DateTime<chrono::Utc>) -> crate::schema::SsoSessionSecret {
        crate::schema::SsoSessionSecret {
            access_token: "access-token-value".into(),
            refresh_token: Some("refresh-token-value".into()),
            token_expires_at: expires_at,
            client_id: "client-id".into(),
            client_secret: "client-secret-value".into(),
            client_secret_expires_at: None,
            issued_at: chrono::Utc::now(),
            revision: 0,
        }
    }

    #[tokio::test]
    async fn sso_role_credentials_are_fetched_once_and_reused_until_invalidated() {
        use crate::llm::sso::tests_support::{MemSessions, Reply, Stub};
        use crate::storage::sso_session::SsoSessionStorage;

        let stub = Stub::start_portal(
            Reply::ok(serde_json::json!({"accountList": []})),
            Reply::ok(serde_json::json!({"roleList": []})),
            Reply::ok(serde_json::json!({"roleCredentials": {
                "accessKeyId": "ASIAEXAMPLEKEYID",
                "secretAccessKey": "secret-access-value",
                "sessionToken": "session-token-value",
                "expiration": 4102444800000_i64
            }})),
        )
        .await;
        let storage = std::sync::Arc::new(MemSessions::default());
        storage
            .put_sso_session(
                "acme-bedrock",
                &sso_session(chrono::Utc::now() + chrono::TimeDelta::hours(1)),
                None,
            )
            .await
            .unwrap();
        let credential = SsoRoleCredential::new(
            SsoClient::at(reqwest::Client::new(), stub.base()),
            storage,
            SsoRoleTarget {
                profile: "acme-bedrock".into(),
                account_id: "123456789012".into(),
                role_name: "BedrockOperator".into(),
                region: "us-east-1".into(),
            },
        );
        let url = "https://bedrock-runtime.us-east-1.amazonaws.com/model/arn/converse";

        for _ in 0..2 {
            let built = credential
                .authorize_with_body(
                    reqwest::Client::new().post(url),
                    &RequestInfo::default(),
                    "POST",
                    url,
                    br#"{"messages":[]}"#,
                )
                .await
                .unwrap()
                .build()
                .unwrap();
            let authorization = built
                .headers()
                .get(reqwest::header::AUTHORIZATION)
                .unwrap()
                .to_str()
                .unwrap();
            assert!(
                authorization.starts_with("AWS4-HMAC-SHA256 Credential=ASIAEXAMPLEKEYID/"),
                "{authorization}"
            );
            assert_eq!(
                built
                    .headers()
                    .get("x-amz-security-token")
                    .unwrap()
                    .to_str()
                    .unwrap(),
                "session-token-value"
            );
        }
        // The second call was answered from the cache: zero extra portal requests (FR-013).
        assert_eq!(stub.path_count("/federation/credentials"), 1);

        assert!(credential.invalidate().await);
        assert!(!credential.invalidate().await, "nothing left to drop");

        credential
            .authorize_with_body(
                reqwest::Client::new().post(url),
                &RequestInfo::default(),
                "POST",
                url,
                br#"{"messages":[]}"#,
            )
            .await
            .unwrap();
        assert_eq!(stub.path_count("/federation/credentials"), 2);
    }

    #[tokio::test]
    async fn sso_authorize_without_a_body_is_a_configuration_error() {
        use crate::llm::sso::tests_support::MemSessions;

        let credential = SsoRoleCredential::new(
            SsoClient::at(reqwest::Client::new(), "http://127.0.0.1:1"),
            std::sync::Arc::new(MemSessions::default()),
            SsoRoleTarget {
                profile: "acme-bedrock".into(),
                account_id: "123456789012".into(),
                role_name: "BedrockOperator".into(),
                region: "us-east-1".into(),
            },
        );
        let error = credential
            .authorize(reqwest::Client::new().post("https://example.test"), &RequestInfo::default())
            .await
            .unwrap_err();
        assert!(error.to_string().contains("body-aware"));
    }

    #[tokio::test]
    async fn sso_debug_never_prints_a_credential_half() {
        use crate::llm::sso::tests_support::MemSessions;

        let credential = SsoRoleCredential::new(
            SsoClient::at(reqwest::Client::new(), "http://127.0.0.1:1"),
            std::sync::Arc::new(MemSessions::default()),
            SsoRoleTarget {
                profile: "acme-bedrock".into(),
                account_id: "123456789012".into(),
                role_name: "BedrockOperator".into(),
                region: "us-east-1".into(),
            },
        );
        let printed = format!("{credential:?}");
        assert!(printed.contains("acme-bedrock"));
        assert!(printed.contains("passwords") == false);
        assert!(printed.contains("[redacted]") || printed.contains("cached"));
    }

    #[test]
    fn sigv4_debug_never_prints_the_secrets() {
        let printed = format!("{:?}", bedrock_credential());
        assert!(!printed.contains("secret-access-key"), "{printed}");
        assert!(!printed.contains("session-token"), "{printed}");
        assert!(printed.contains("[redacted]"));
        assert!(printed.contains("AKIDEXAMPLE"));
    }
}
