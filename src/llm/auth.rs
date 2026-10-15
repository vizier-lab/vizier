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
}
