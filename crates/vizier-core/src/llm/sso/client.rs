//! The wire half of the device flow: one-time client registration, device authorization and
//! token creation, straight from `contracts/bedrock-sso-wire.md` §1–§3, over
//! [`VizierDependencies.http`](crate::VizierDependencies) — no `aws-sdk-*`, no CLI, no file under
//! `~/.aws` (FR-006, research D4).
//!
//! Every endpoint derives from the **SSO region** (the Identity Center instance's), never from
//! the Bedrock region: they are not interchangeable (FR-010). [`SsoEndpoints::at`] is the one
//! override, for the loopback stub the tests use (research D9).
//!
//! The poll loop is a state machine — pending, slow down, approved, denied, expired, already
//! used — bounded by the code's own life minus a five-second guard and by a ten-minute ceiling,
//! so a sign-in can never hang (SC-002). A confirmed flow refuses to be confirmed twice
//! ([`SsoError::AlreadyConfirmed`]): the second call cannot mint a second session from one code.

use crate::time::Timestamp;
use std::fmt;
use std::time::Duration;

use chrono::{Duration as ChronoDuration, Local};
use serde::Deserialize;

use super::error::SsoError;

/// The device-code grant (RFC 8628 §3.4), spelled out because AWS compares it literally.
const DEVICE_CODE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";
/// The refresh grant (§4 of the contract).
const REFRESH_GRANT: &str = "refresh_token";

/// A pending code's last seconds are unusable: stop polling this long before it dies, so a late
/// approval is reported as an expiry rather than as a session that cannot refresh.
const CODE_GUARD: ChronoDuration = ChronoDuration::seconds(5);
/// The overall ceiling on one pending sign-in, whatever `expiresIn` claims (§3).
const FLOW_CEILING: ChronoDuration = ChronoDuration::minutes(10);
/// `slow_down` adds this much to the interval (§3).
const SLOW_DOWN_STEP: Duration = Duration::from_secs(5);
/// A registration is re-made this long before its secret dies, so a sign-in never starts on a
/// client that expires mid-flow (§2).
const REGISTRATION_GUARD: ChronoDuration = ChronoDuration::minutes(5);
/// How long to wait when the endpoint omits `interval` (RFC 8628's default).
const DEFAULT_INTERVAL: Duration = Duration::from_secs(5);

/// The two hosts one SSO region offers, derived once (§1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsoEndpoints {
    /// `https://oidc.{sso_region}.amazonaws.com` — registration, device authorization, tokens.
    pub oidc: String,
    /// `https://portal.sso.{sso_region}.amazonaws.com` — accounts, roles, role credentials.
    pub portal: String,
}

impl SsoEndpoints {
    /// The real endpoints of one SSO region. Both come from `sso_region`, never from the Bedrock
    /// region (FR-010).
    pub fn for_sso_region(sso_region: &str) -> Self {
        Self {
            oidc: format!("https://oidc.{sso_region}.amazonaws.com"),
            portal: format!("https://portal.sso.{sso_region}.amazonaws.com"),
        }
    }

    /// One loopback origin serving both halves — the stub server's form (research D9). Test-only
    /// in practice, but not `cfg(test)`: the flow needs an address it did not derive.
    pub fn at(base: &str) -> Self {
        let base = base.trim_end_matches('/').to_string();
        Self {
            oidc: base.clone(),
            portal: base,
        }
    }
}

/// One registered OIDC client for a profile (§2). Stored in the session envelope, reused on every
/// re-sign-in until its secret is near expiry (FR-009b).
#[derive(Clone, PartialEq, Eq)]
pub struct ClientRegistration {
    pub client_id: String,
    pub client_secret: String,
    /// `None` when the registration never expires (`clientSecretExpiresAt` absent or `0`).
    pub expires_at: Option<Timestamp>,
}

impl ClientRegistration {
    /// Whether this registration may still start a sign-in at `now`: not expiring, or expiring
    /// later than the guard window (§2).
    pub fn is_reusable(&self, now: Timestamp) -> bool {
        match self.expires_at {
            None => true,
            Some(expires_at) => expires_at - REGISTRATION_GUARD > now,
        }
    }
}

impl fmt::Debug for ClientRegistration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientRegistration")
            .field("client_id", &self.client_id)
            .field("client_secret", &"[redacted]")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

/// What the person is shown while the code is pending (§3): the user code, both verification
/// URIs and the expiry — **never** the device code (FR-009b, FR-035).
#[derive(Clone, PartialEq, Eq)]
pub struct DeviceAuthorization {
    /// Stays in the process: printed by nobody, returned by nobody (contracts §7).
    device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: Option<String>,
    /// When the user code dies, on this process's clock.
    pub expires_at: Timestamp,
    /// The endpoint's requested polling interval.
    pub interval: Duration,
}

impl DeviceAuthorization {
    fn device_code(&self) -> &str {
        &self.device_code
    }
}

impl fmt::Debug for DeviceAuthorization {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceAuthorization")
            .field("device_code", &"[redacted]")
            .field("user_code", &self.user_code)
            .field("verification_uri", &self.verification_uri)
            .field("verification_uri_complete", &self.verification_uri_complete)
            .field("expires_at", &self.expires_at)
            .field("interval", &self.interval)
            .finish()
    }
}

/// One token response (§3, §5). `Debug` is redacted: the access and refresh tokens are the
/// session's secret half (FR-009b).
#[derive(Clone, PartialEq, Eq)]
pub struct TokenSet {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Timestamp,
}

impl fmt::Debug for TokenSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TokenSet")
            .field("access_token", &"[redacted]")
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "[redacted]"),
            )
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

/// One answer from `/token`, before the caller decides what it means (`contracts §3` table).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenPoll {
    /// `200`: the person approved; these are the tokens.
    Approved(TokenSet),
    /// `400 authorization_pending`: not yet — poll again after the interval.
    Pending,
    /// `400 slow_down`: polling too fast — the interval grows and the poll repeats.
    SlowDown,
    /// `400 access_denied`: the person refused.
    Denied,
    /// `400 expired_token`: the code died.
    Expired,
    /// `400 invalid_grant`: the code or refresh token was already used. One code, one session.
    Used,
    /// Some other `error` the endpoint named (e.g. `invalid_client`), carried verbatim so the
    /// reason is not lost. Never carries a token: the endpoint's `error` is a short code.
    Refused(String),
}

/// The OIDC half of one SSO region, over the workspace HTTP client.
#[derive(Debug, Clone)]
pub struct SsoClient {
    pub(super) http: reqwest::Client,
    region: String,
    endpoints: SsoEndpoints,
}

/// One response body made safe for a log line and for a person: bounded, and with the value under
/// any sensitive key replaced. AWS answers a malformed request with the one thing that makes a
/// `400` diagnosable (`InvalidRequestException`, `invalid_client`), and the request body — which
/// carries the secret — is never logged. A mis-routed response could still hold a token, so the
/// guard is unconditional (FR-035, contracts §7).
fn redact_error_body(body: &str) -> String {
    // Enough of a body to name the cause, short enough for one log line.
    const LIMIT: usize = 512;
    const SENSITIVE: [&str; 5] = [
        "accessToken",
        "refreshToken",
        "deviceCode",
        "clientSecret",
        "authorizationCode",
    ];

    // `chars()`, not bytes: the cut must never split a UTF-8 sequence.
    let mut text: String = body.chars().take(LIMIT).collect();
    let truncated = body.chars().count() > LIMIT;
    for key in SENSITIVE {
        text = redact_value(&text, key);
    }
    if truncated {
        text.push('…');
    }
    if text.trim().is_empty() {
        return "(the response body was empty)".to_string();
    }
    text
}

/// Replace every `"key": value` pair with `"key": "[redacted]"`, whatever the case of the key and
/// the spacing around the colon; a bare scalar and a quoted string are both handled.
fn redact_value(text: &str, key: &str) -> String {
    let needle = format!("\"{}\"", key.to_ascii_lowercase());
    // ASCII-lowercasing preserves byte offsets, so an index found here is valid in `text`.
    let lower = text.to_ascii_lowercase();
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    while let Some(found) = lower[cursor..].find(&needle) {
        let key_start = cursor + found;
        let mut index = key_start + needle.len();
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() || bytes[index] != b':' {
            // Part of a value or a message, not a pair: copy through and keep looking.
            out.push_str(&text[cursor..key_start + needle.len()]);
            cursor = key_start + needle.len();
            continue;
        }
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let value_start = index;
        let value_end = if index < bytes.len() && bytes[index] == b'"' {
            let mut end = index + 1;
            while end < bytes.len() {
                match bytes[end] {
                    b'\\' => end += 2,
                    b'"' => {
                        end += 1;
                        break;
                    }
                    _ => end += 1,
                }
            }
            end
        } else {
            let mut end = index;
            while end < bytes.len()
                && !matches!(bytes[end], b',' | b'}' | b']')
                && !bytes[end].is_ascii_whitespace()
            {
                end += 1;
            }
            end
        };
        out.push_str(&text[cursor..value_start]);
        out.push_str("\"[redacted]\"");
        cursor = value_end.min(bytes.len());
    }
    out.push_str(&text[cursor..]);
    out
}

impl SsoClient {
    /// The client for one **SSO** region (never the Bedrock region, FR-010).
    pub fn new(http: reqwest::Client, sso_region: &str) -> Self {
        Self {
            http,
            region: sso_region.to_string(),
            endpoints: SsoEndpoints::for_sso_region(sso_region),
        }
    }

    /// The stub-server form: every endpoint at `base` (research D9).
    pub fn at(http: reqwest::Client, base: &str) -> Self {
        Self {
            http,
            region: "test".to_string(),
            endpoints: SsoEndpoints::at(base),
        }
    }

    /// The region every message about this client names.
    pub fn region(&self) -> &str {
        &self.region
    }

    pub(crate) fn endpoints(&self) -> &SsoEndpoints {
        &self.endpoints
    }

    /// One unavailable-endpoint error, with the region named (FR-010).
    pub(crate) fn unavailable(&self, reason: impl Into<String>) -> SsoError {
        SsoError::Unavailable {
            region: self.region.clone(),
            reason: reason.into(),
        }
    }

    /// Nothing is sent with a hole in it. An empty required value is a caller's bug, reported here
    /// with the field named, rather than as a body AWS answers `400` to without saying which field.
    /// Only the field's name appears — never its value (FR-035).
    fn require_non_empty(&self, field: &str, value: &str) -> Result<(), SsoError> {
        if value.trim().is_empty() {
            return Err(self.unavailable(format!(
                "the request was not sent because {field} is empty"
            )));
        }
        Ok(())
    }

    /// Send, retrying exactly once on a transport failure or a 5xx (§3, §2). Returns the status
    /// and body so each caller applies its own table; a repeated failure is returned as the
    /// status it was, or as `Unavailable`.
    pub(crate) async fn send_with_retry<F>(
        &self,
        url: &str,
        build: F,
    ) -> Result<(u16, String), SsoError>
    where
        F: Fn(&str) -> reqwest::RequestBuilder,
    {
        let mut attempt = 0;
        loop {
            attempt += 1;
            let last = attempt > 1;
            let response = match build(url).send().await {
                Ok(response) => response,
                Err(error) if !last => {
                    tracing::debug!(
                        region = %self.region,
                        url = %url,
                        attempt,
                        %error,
                        "the request failed; retrying once"
                    );
                    continue;
                }
                Err(error) => {
                    tracing::warn!(
                        region = %self.region,
                        url = %url,
                        attempt,
                        %error,
                        "the request failed"
                    );
                    return Err(self.unavailable(format!("the request failed: {error}")));
                }
            };
            let status = response.status().as_u16();
            if status >= 500 && !last {
                tracing::debug!(
                    region = %self.region,
                    url = %url,
                    status,
                    attempt,
                    "the endpoint answered 5xx; retrying once"
                );
                continue;
            }
            let body = match response.text().await {
                Ok(body) => body,
                Err(error) if !last => {
                    tracing::debug!(
                        region = %self.region,
                        url = %url,
                        status,
                        %error,
                        "the response body could not be read; retrying once"
                    );
                    continue;
                }
                Err(error) => {
                    tracing::warn!(
                        region = %self.region,
                        url = %url,
                        status,
                        %error,
                        "the response body could not be read"
                    );
                    return Err(self.unavailable(format!("the response could not be read: {error}")));
                }
            };
            if status >= 400 {
                // Only the *response* body is logged: the request body carries the secret.
                tracing::warn!(
                    region = %self.region,
                    url = %url,
                    status,
                    body = %redact_error_body(&body),
                    "the endpoint refused the request"
                );
            }
            return Ok((status, body));
        }
    }

    /// §2: register one public client for this profile. Reuse [`ClientRegistration`] instead of
    /// calling this again while [`ClientRegistration::is_reusable`] holds.
    pub async fn register_client(&self) -> Result<ClientRegistration, SsoError> {
        let url = format!("{}/client/register", self.endpoints.oidc);
        let payload = serde_json::json!({
            "clientName": "vizier",
            "clientType": "public",
            "scopes": ["sso:account:access"],
        });
        tracing::debug!(region = %self.region, "registering a client for this profile");
        let (status, body) = self
            .send_with_retry(&url, |url| self.http.post(url).json(&payload))
            .await?;
        if status != 200 {
            // A wrong region or an unreachable endpoint lands here (§2).
            let detail = redact_error_body(&body);
            return Err(self.unavailable(format!(
                "the client registration answered HTTP {status}: {detail}"
            )));
        }
        let wire: RegistrationWire = serde_json::from_str(&body).map_err(|_| {
            self.unavailable("the client registration answered an unexpected shape")
        })?;
        let registration = ClientRegistration {
            client_id: wire.client_id,
            client_secret: wire.client_secret,
            expires_at: wire
                .client_secret_expires_at
                .filter(|seconds| *seconds > 0)
                .and_then(|seconds| crate::time::from_secs_opt(seconds, 0)),
        };
        tracing::debug!(
            region = %self.region,
            expires_at = ?registration.expires_at,
            "a client registration was made"
        );
        Ok(registration)
    }

    /// §3: ask for a device code. The user code and URIs are what the person sees; the device
    /// code stays here.
    pub async fn start_device_authorization(
        &self,
        registration: &ClientRegistration,
        start_url: &str,
    ) -> Result<DeviceAuthorization, SsoError> {
        self.require_non_empty("client id", &registration.client_id)?;
        self.require_non_empty("client secret", &registration.client_secret)?;
        self.require_non_empty("start url", start_url)?;
        let url = format!("{}/device_authorization", self.endpoints.oidc);
        // rest-json with camelCase keys: the endpoint reads `clientId`, not `client_id` (§3).
        let payload = serde_json::json!({
            "clientId": registration.client_id.as_str(),
            "clientSecret": registration.client_secret.as_str(),
            "startUrl": start_url,
        });
        let (status, body) = self
            .send_with_retry(&url, |url| self.http.post(url).json(&payload))
            .await?;
        if status != 200 {
            let detail = redact_error_body(&body);
            return Err(self.unavailable(format!(
                "the device authorization answered HTTP {status}: {detail}"
            )));
        }
        let wire: DeviceAuthorizationWire = serde_json::from_str(&body).map_err(|_| {
            self.unavailable("the device authorization answered an unexpected shape")
        })?;
        // The device code stays in the process; only the user code is worth a log line (FR-035).
        tracing::debug!(
            region = %self.region,
            user_code = %wire.user_code,
            expires_in = wire.expires_in,
            interval = ?wire.interval,
            "a device code was issued"
        );
        let now = crate::time::now();
        Ok(DeviceAuthorization {
            device_code: wire.device_code,
            user_code: wire.user_code,
            verification_uri: wire.verification_uri,
            verification_uri_complete: wire.verification_uri_complete,
            expires_at: now + ChronoDuration::seconds(wire.expires_in as i64),
            interval: Duration::from_secs(wire.interval.unwrap_or(DEFAULT_INTERVAL.as_secs())),
        })
    }

    /// §3: one poll of `/token` for a device code. The answer's meaning is the caller's.
    pub async fn token_for_device_code(
        &self,
        registration: &ClientRegistration,
        device_code: &str,
    ) -> Result<TokenPoll, SsoError> {
        self.require_non_empty("client id", &registration.client_id)?;
        self.require_non_empty("client secret", &registration.client_secret)?;
        self.require_non_empty("device code", device_code)?;
        let url = format!("{}/token", self.endpoints.oidc);
        // rest-json with camelCase keys: `grantType`/`deviceCode`, not the form spelling (§3).
        let payload = serde_json::json!({
            "clientId": registration.client_id.as_str(),
            "clientSecret": registration.client_secret.as_str(),
            "grantType": DEVICE_CODE_GRANT,
            "deviceCode": device_code,
        });
        let (status, body) = self
            .send_with_retry(&url, |url| self.http.post(url).json(&payload))
            .await?;
        self.read_token(status, &body)
    }

    /// §4/§5: exchange a refresh token for a new access token, through the same `/token`.
    pub async fn refresh_access_token(
        &self,
        registration: &ClientRegistration,
        refresh_token: &str,
    ) -> Result<TokenPoll, SsoError> {
        self.require_non_empty("client id", &registration.client_id)?;
        self.require_non_empty("client secret", &registration.client_secret)?;
        self.require_non_empty("refresh token", refresh_token)?;
        let url = format!("{}/token", self.endpoints.oidc);
        // The same rest-json body as the device grant, with the refresh token named (§5).
        let payload = serde_json::json!({
            "clientId": registration.client_id.as_str(),
            "clientSecret": registration.client_secret.as_str(),
            "grantType": REFRESH_GRANT,
            "refreshToken": refresh_token,
        });
        tracing::debug!(region = %self.region, "refreshing the access token");
        let (status, body) = self
            .send_with_retry(&url, |url| self.http.post(url).json(&payload))
            .await?;
        self.read_token(status, &body)
    }

    /// The `/token` table of `contracts §3`, in one place.
    fn read_token(&self, status: u16, body: &str) -> Result<TokenPoll, SsoError> {
        if status == 200 {
            let wire: TokenWire = serde_json::from_str(body).map_err(|_| {
                self.unavailable("the token endpoint answered an unexpected shape")
            })?;
            let tokens = TokenSet {
                access_token: wire.access_token,
                refresh_token: wire.refresh_token,
                expires_at: crate::time::now() + ChronoDuration::seconds(wire.expires_in as i64),
            };
            tracing::debug!(
                region = %self.region,
                expires_at = %tokens.expires_at,
                "the token endpoint issued an access token"
            );
            return Ok(TokenPoll::Approved(tokens));
        }
        if status == 400 || status == 401 {
            let wire: TokenErrorWire = match serde_json::from_str(body) {
                Ok(wire) => wire,
                Err(_) => {
                    // A refusal AWS did not shape as `{"error": …}`: keep what it did say, minus
                    // anything that looks like a token, so the cause is not lost (FR-035).
                    let detail = redact_error_body(body);
                    return Err(self.unavailable(format!(
                        "the token endpoint answered HTTP {status}: {detail}"
                    )));
                }
            };
            let poll = match wire.error.as_str() {
                "authorization_pending" => TokenPoll::Pending,
                "slow_down" => TokenPoll::SlowDown,
                "access_denied" => TokenPoll::Denied,
                "expired_token" => TokenPoll::Expired,
                "invalid_grant" => TokenPoll::Used,
                other => TokenPoll::Refused(other.to_string()),
            };
            match &poll {
                TokenPoll::Pending => {
                    tracing::debug!(region = %self.region, "the device code is still pending");
                }
                TokenPoll::SlowDown => {
                    tracing::debug!(region = %self.region, "the endpoint asked for a slower poll");
                }
                TokenPoll::Denied => {
                    tracing::warn!(region = %self.region, "the sign-in was denied in the browser");
                }
                TokenPoll::Expired => {
                    tracing::warn!(
                        region = %self.region,
                        "the device code expired before it was confirmed"
                    );
                }
                TokenPoll::Used => {
                    tracing::warn!(region = %self.region, "the device code was already used");
                }
                TokenPoll::Refused(error) => {
                    tracing::warn!(
                        region = %self.region,
                        error = %error,
                        "the token endpoint refused the request"
                    );
                }
                // Never built here; listed so the match stays exhaustive.
                TokenPoll::Approved(_) => {}
            }
            return Ok(poll);
        }
        let detail = redact_error_body(body);
        Err(self.unavailable(format!(
            "the token endpoint answered HTTP {status}: {detail}"
        )))
    }
}

/// A sign-in in progress: one device code, one interval, one deadline (§3).
///
/// Built by [`super::begin_sign_in`] so the client registration is made once and reused. The
/// person's code and URIs are readable; the device code is not.
#[derive(Debug)]
pub struct DeviceFlow {
    client: SsoClient,
    registration: ClientRegistration,
    authorization: DeviceAuthorization,
    interval: Duration,
    deadline: Timestamp,
    confirmed: bool,
}

impl DeviceFlow {
    /// §3: obtain a device code and fix the deadline. Polling is [`DeviceFlow::complete`].
    pub async fn start(
        client: SsoClient,
        registration: ClientRegistration,
        start_url: &str,
    ) -> Result<Self, SsoError> {
        let authorization = client
            .start_device_authorization(&registration, start_url)
            .await?;
        let now = crate::time::now();
        // Both bounds of §3: the code's own life minus the guard, and the ten-minute ceiling.
        let deadline = (authorization.expires_at - CODE_GUARD).min(now + FLOW_CEILING);
        Ok(Self {
            client,
            registration,
            interval: authorization.interval.max(Duration::from_secs(1)),
            deadline,
            confirmed: false,
            authorization,
        })
    }

    /// The code the person types (`FR-012`).
    pub fn user_code(&self) -> &str {
        &self.authorization.user_code
    }

    /// Where the person goes to approve it.
    pub fn verification_uri(&self) -> &str {
        &self.authorization.verification_uri
    }

    /// The same page with the code pre-filled, when AWS returned one.
    pub fn verification_uri_complete(&self) -> Option<&str> {
        self.authorization.verification_uri_complete.as_deref()
    }

    /// When the code dies — the expiry the pending panel shows.
    pub fn expires_at(&self) -> Timestamp {
        self.authorization.expires_at
    }

    /// The registration this flow polls with; the session envelope keeps it (§2).
    pub fn registration(&self) -> &ClientRegistration {
        &self.registration
    }

    /// How long the next poll waits — what the pending panel shows as `interval_seconds`.
    pub fn interval(&self) -> Duration {
        self.interval
    }

    /// Poll until the person answers or the code's life runs out. Never hangs: the loop checks
    /// the deadline before every poll, and each answer is its own error (§3, SC-002).
    ///
    /// A second call after a success returns [`SsoError::AlreadyConfirmed`]: one code mints one
    /// session.
    pub async fn complete(&mut self) -> Result<TokenSet, SsoError> {
        loop {
            match self.poll_once().await? {
                Some(tokens) => return Ok(tokens),
                None => tokio::time::sleep(self.interval).await,
            }
        }
    }

    /// One poll of `/token`, for a caller that answers the person instead of blocking the request
    /// until they reach a browser: `Some(tokens)` means approved, `None` means not yet (the
    /// request answers `202 pending` and the next call polls again).
    ///
    /// The same table as [`DeviceFlow::complete`] (§3): a denial and an expired or already-used
    /// code are terminal, `slow_down` only grows the interval.
    pub async fn poll_once(&mut self) -> Result<Option<TokenSet>, SsoError> {
        if self.confirmed {
            return Err(SsoError::AlreadyConfirmed);
        }
        if crate::time::now() >= self.deadline {
            return Err(SsoError::CodeExpired);
        }
        let poll = self
            .client
            .token_for_device_code(&self.registration, self.authorization.device_code())
            .await?;
        match poll {
            TokenPoll::Approved(tokens) => {
                self.confirmed = true;
                Ok(Some(tokens))
            }
            TokenPoll::Pending => Ok(None),
            TokenPoll::SlowDown => {
                self.interval += SLOW_DOWN_STEP;
                Ok(None)
            }
            TokenPoll::Denied => Err(SsoError::SignInDenied),
            TokenPoll::Expired => Err(SsoError::CodeExpired),
            TokenPoll::Used => Err(SsoError::AlreadyConfirmed),
            TokenPoll::Refused(reason) => Err(self.client.unavailable(reason)),
        }
    }
}

// ---------------------------------------------------------------- wire shapes

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegistrationWire {
    client_id: String,
    client_secret: String,
    /// Epoch seconds; `0` or absent means "never".
    #[serde(default)]
    client_secret_expires_at: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeviceAuthorizationWire {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[serde(default)]
    verification_uri_complete: Option<String>,
    expires_in: u64,
    #[serde(default)]
    interval: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TokenWire {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    expires_in: u64,
}

#[derive(Deserialize)]
struct TokenErrorWire {
    error: String,
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;
    use crate::llm::sso::session::SsoSessions;
    use crate::llm::sso::tests_support::{
        MemSessions, Reply, Stub, approved, denied, pending, used_twice,
    };

    const PROFILE: &str = "acme-bedrock";
    const START_URL: &str = "https://acme.awsapps.com/start";

    fn registration() -> ClientRegistration {
        ClientRegistration {
            client_id: "client-id".into(),
            client_secret: "client-secret-value".into(),
            expires_at: None,
        }
    }

    async fn flow(stub: &Stub) -> DeviceFlow {
        let client = SsoClient::at(reqwest::Client::new(), stub.base());
        DeviceFlow::start(client, registration(), START_URL)
            .await
            .expect("the stub always answers the device authorization")
    }

    /// What a caller does with a confirmed code: persist the session under the profile. Every
    /// failure below returns from `complete` first, so nothing is written (T019).
    async fn sign_in(
        flow: &mut DeviceFlow,
        sessions: &SsoSessions<'_, MemSessions>,
    ) -> Result<(), SsoError> {
        let tokens = flow.complete().await?;
        sessions
            .store_sign_in(PROFILE, flow.registration(), tokens)
            .await?;
        Ok(())
    }

    #[tokio::test]
    async fn a_confirmed_code_stores_exactly_one_session() {
        let stub = Stub::start(vec![approved()]).await;
        let mut flow = flow(&stub).await;
        let storage = MemSessions::default();
        let sessions = SsoSessions::new(&storage);

        sign_in(&mut flow, &sessions).await.unwrap();

        let stored = sessions.load(PROFILE).await.unwrap().expect("a session");
        assert_eq!(stored.access_token, "access-token-value");
        assert_eq!(stored.refresh_token.as_deref(), Some("refresh-token-value"));
        assert_eq!(stored.client_id, "client-id");
        assert_eq!(stored.revision, 1);
        assert_eq!(sessions.storage().len(), 1);

        // §3 request shapes reached the wire: JSON with camelCase keys, the start URL and the
        // device code — never the form spelling AWS answers 400 to (044).
        let seen = stub.seen();
        let sent_to = |path: &str| -> serde_json::Value {
            let (_, body) = seen
                .iter()
                .find(|(seen, _)| seen == path)
                .unwrap_or_else(|| panic!("no request reached {path}"));
            serde_json::from_str(body)
                .unwrap_or_else(|_| panic!("{path} did not receive a JSON body: {body}"))
        };
        let device = sent_to("/device_authorization");
        assert_eq!(device["clientId"], "client-id");
        assert_eq!(device["startUrl"], START_URL);
        assert!(device.get("client_id").is_none(), "{device}");
        assert!(device.get("start_url").is_none(), "{device}");
        let token = sent_to("/token");
        assert_eq!(token["grantType"], DEVICE_CODE_GRANT);
        assert_eq!(token["deviceCode"], "device-code-value");
        assert!(token.get("grant_type").is_none(), "{token}");
        assert!(token.get("device_code").is_none(), "{token}");
        // The code the person reads is the one the stub issued.
        assert_eq!(flow.user_code(), "ABCD-EFGH");
        assert_eq!(flow.verification_uri(), "https://example.test/device");
    }

    #[tokio::test]
    async fn a_denied_sign_in_stores_nothing() {
        let stub = Stub::start(vec![denied()]).await;
        let mut flow = flow(&stub).await;
        let storage = MemSessions::default();
        let sessions = SsoSessions::new(&storage);

        let error = sign_in(&mut flow, &sessions).await.unwrap_err();

        assert_eq!(error, SsoError::SignInDenied);
        assert_eq!(sessions.storage().len(), 0);
        assert!(sessions.load(PROFILE).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn an_unanswered_code_times_out_as_an_expiry_and_stores_nothing() {
        // A seven-second code: minus the five-second guard, the deadline is two seconds away,
        // and the stub never approves.
        let stub = Stub::start_with_code_life(7, vec![pending()]).await;
        let mut flow = flow(&stub).await;
        let storage = MemSessions::default();
        let sessions = SsoSessions::new(&storage);

        let error = sign_in(&mut flow, &sessions).await.unwrap_err();

        assert_eq!(error, SsoError::CodeExpired);
        assert_eq!(sessions.storage().len(), 0);
        // It polled rather than expiring on a clock check alone: at least one token request.
        assert!(stub.path_count("/token") >= 1);
    }

    #[tokio::test]
    async fn a_code_already_used_on_the_wire_is_its_own_error_and_stores_nothing() {
        let stub = Stub::start(vec![used_twice()]).await;
        let mut flow = flow(&stub).await;
        let storage = MemSessions::default();
        let sessions = SsoSessions::new(&storage);

        let error = sign_in(&mut flow, &sessions).await.unwrap_err();

        assert_eq!(error, SsoError::AlreadyConfirmed);
        assert_eq!(sessions.storage().len(), 0);
    }

    #[tokio::test]
    async fn a_code_confirmed_twice_cannot_mint_a_second_session() {
        let stub = Stub::start(vec![approved()]).await;
        let mut flow = flow(&stub).await;
        let storage = MemSessions::default();
        let sessions = SsoSessions::new(&storage);

        sign_in(&mut flow, &sessions).await.unwrap();
        let error = flow.complete().await.unwrap_err();

        assert_eq!(error, SsoError::AlreadyConfirmed);
        assert_eq!(sessions.storage().len(), 1);
        // Only one token request: the second confirmation never reached the wire.
        assert_eq!(stub.path_count("/token"), 1);
    }

    #[tokio::test]
    async fn every_failure_has_its_own_message() {
        let denied_message = SsoError::SignInDenied.to_string();
        let expired_message = SsoError::CodeExpired.to_string();
        let confirmed_message = SsoError::AlreadyConfirmed.to_string();
        for (a, b) in [
            (&denied_message, &expired_message),
            (&denied_message, &confirmed_message),
            (&expired_message, &confirmed_message),
        ] {
            assert_ne!(a, b);
        }
    }

    #[test]
    fn a_registration_is_reused_until_just_before_its_secret_expires() {
        let now = Local.with_ymd_and_hms(2026, 11, 2, 12, 0, 0).unwrap();
        let mut registration = ClientRegistration {
            client_id: "client-id".into(),
            client_secret: "client-secret-value".into(),
            expires_at: None,
        };
        assert!(registration.is_reusable(now));

        registration.expires_at = Some(now + ChronoDuration::hours(1));
        assert!(registration.is_reusable(now));

        // Inside the guard window: register again rather than start a sign-in that dies.
        registration.expires_at = Some(now + ChronoDuration::minutes(2));
        assert!(!registration.is_reusable(now));
    }

    #[test]
    fn the_registration_and_the_flow_never_print_their_secret_half() {
        let printed = format!("{:?}", registration());
        assert!(!printed.contains("client-secret-value"), "{printed}");

        let flow_debug = format!(
            "{:?}",
            DeviceAuthorization {
                device_code: "device-code-value".into(),
                user_code: "ABCD-EFGH".into(),
                verification_uri: "https://example.test/device".into(),
                verification_uri_complete: None,
                expires_at: crate::time::now(),
                interval: Duration::from_secs(5),
            }
        );
        assert!(!flow_debug.contains("device-code-value"), "{flow_debug}");

        let tokens = TokenSet {
            access_token: "access-token-value".into(),
            refresh_token: Some("refresh-token-value".into()),
            expires_at: crate::time::now(),
        };
        let printed = format!("{tokens:?}");
        for poison in ["access-token-value", "refresh-token-value"] {
            assert!(!printed.contains(poison), "{printed}");
        }
    }

    /// The bug of 044, pinned at the wire: the endpoint is rest-json, so a form body must be
    /// refused the way AWS refused it — a `400` that names `InvalidRequestException`.
    #[tokio::test]
    async fn a_form_encoded_device_authorization_is_refused_like_aws() {
        let stub = Stub::start(vec![approved()]).await;
        let url = format!("{}/device_authorization", stub.base());
        let response = reqwest::Client::new()
            .post(url)
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(
                "client_id=client-id&client_secret=client-secret-value\
                 &start_url=https%3A%2F%2Facme.awsapps.com%2Fstart",
            )
            .send()
            .await
            .unwrap();

        assert_eq!(response.status().as_u16(), 400);
        let body = response.text().await.unwrap();
        assert!(body.contains("InvalidRequestException"), "{body}");
    }

    /// §3's own body: JSON, camelCase, nothing the stub can mistake for a form post.
    #[tokio::test]
    async fn the_device_authorization_is_json_with_camel_case_keys() {
        let stub = Stub::start(vec![approved()]).await;
        let client = SsoClient::at(reqwest::Client::new(), stub.base());

        let authorization = client
            .start_device_authorization(&registration(), START_URL)
            .await
            .unwrap();

        assert_eq!(authorization.user_code, "ABCD-EFGH");
        let content_type = stub
            .content_types()
            .into_iter()
            .find(|(path, _)| path == "/device_authorization")
            .map(|(_, content_type)| content_type)
            .expect("the request reached the stub");
        assert!(
            content_type
                .to_ascii_lowercase()
                .starts_with("application/json"),
            "{content_type}"
        );
        let body = stub
            .seen()
            .into_iter()
            .find(|(path, _)| path == "/device_authorization")
            .map(|(_, body)| body)
            .expect("the request reached the stub");
        let sent: serde_json::Value = serde_json::from_str(&body).expect("a JSON body");
        assert_eq!(sent["clientId"], "client-id");
        assert_eq!(sent["clientSecret"], "client-secret-value");
        assert_eq!(sent["startUrl"], START_URL);
        assert!(sent.get("client_id").is_none(), "{body}");
        assert!(sent.get("start_url").is_none(), "{body}");
    }

    /// The same body rule on the refresh grant (§5).
    #[tokio::test]
    async fn the_refresh_grant_is_json_with_camel_case_keys() {
        let stub = Stub::start_replies(vec![(
            "/token",
            vec![Reply::ok(serde_json::json!({
                "accessToken": "renewed-access-token",
                "expiresIn": 3600,
            }))],
        )])
        .await;
        let client = SsoClient::at(reqwest::Client::new(), stub.base());

        let poll = client
            .refresh_access_token(&registration(), "refresh-token-value")
            .await
            .unwrap();

        assert!(matches!(poll, TokenPoll::Approved(_)));
        let body = stub
            .seen()
            .into_iter()
            .find(|(path, _)| path == "/token")
            .map(|(_, body)| body)
            .expect("the request reached the stub");
        let sent: serde_json::Value = serde_json::from_str(&body).expect("a JSON body");
        assert_eq!(sent["grantType"], "refresh_token");
        assert_eq!(sent["refreshToken"], "refresh-token-value");
        assert!(sent.get("grant_type").is_none(), "{body}");
        assert!(sent.get("refresh_token").is_none(), "{body}");
    }
}
