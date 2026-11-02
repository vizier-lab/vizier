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

use std::fmt;
use std::time::Duration;

use chrono::{DateTime, Duration as ChronoDuration, Utc};
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
    pub expires_at: Option<DateTime<Utc>>,
}

impl ClientRegistration {
    /// Whether this registration may still start a sign-in at `now`: not expiring, or expiring
    /// later than the guard window (§2).
    pub fn is_reusable(&self, now: DateTime<Utc>) -> bool {
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
    pub expires_at: DateTime<Utc>,
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
    pub expires_at: DateTime<Utc>,
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

/// reqwest 0.13 has no `RequestBuilder::form`; a form body is encoded here instead.
pub(crate) fn form_request(
    http: &reqwest::Client,
    url: &str,
    params: &[(&str, &str)],
) -> reqwest::RequestBuilder {
    let body = serde_urlencoded::to_string(params).unwrap_or_default();
    http.post(url)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
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
                Err(e) => {
                    if !last {
                        continue;
                    }
                    return Err(self.unavailable(format!("the request failed: {e}")));
                }
            };
            let status = response.status().as_u16();
            if status >= 500 && !last {
                continue;
            }
            let body = match response.text().await {
                Ok(body) => body,
                Err(e) => {
                    if !last {
                        continue;
                    }
                    return Err(self.unavailable(format!("the response could not be read: {e}")));
                }
            };
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
        let (status, body) = self
            .send_with_retry(&url, |url| self.http.post(url).json(&payload))
            .await?;
        if status != 200 {
            // A wrong region or an unreachable endpoint lands here (§2).
            return Err(self.unavailable(format!(
                "the client registration answered HTTP {status}"
            )));
        }
        let wire: RegistrationWire = serde_json::from_str(&body).map_err(|_| {
            self.unavailable("the client registration answered an unexpected shape")
        })?;
        Ok(ClientRegistration {
            client_id: wire.client_id,
            client_secret: wire.client_secret,
            expires_at: wire
                .client_secret_expires_at
                .filter(|seconds| *seconds > 0)
                .and_then(|seconds| DateTime::from_timestamp(seconds, 0)),
        })
    }

    /// §3: ask for a device code. The user code and URIs are what the person sees; the device
    /// code stays here.
    pub async fn start_device_authorization(
        &self,
        registration: &ClientRegistration,
        start_url: &str,
    ) -> Result<DeviceAuthorization, SsoError> {
        let url = format!("{}/device_authorization", self.endpoints.oidc);
        let (status, body) = self
            .send_with_retry(&url, |url| {
                form_request(&self.http, url, &[
                    ("client_id", registration.client_id.as_str()),
                    ("client_secret", registration.client_secret.as_str()),
                    ("start_url", start_url),
                ])
            })
            .await?;
        if status != 200 {
            return Err(self.unavailable(format!(
                "the device authorization answered HTTP {status}"
            )));
        }
        let wire: DeviceAuthorizationWire = serde_json::from_str(&body).map_err(|_| {
            self.unavailable("the device authorization answered an unexpected shape")
        })?;
        let now = Utc::now();
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
        let url = format!("{}/token", self.endpoints.oidc);
        let (status, body) = self
            .send_with_retry(&url, |url| {
                form_request(&self.http, url, &[
                    ("client_id", registration.client_id.as_str()),
                    ("client_secret", registration.client_secret.as_str()),
                    ("grant_type", DEVICE_CODE_GRANT),
                    ("device_code", device_code),
                ])
            })
            .await?;
        self.read_token(status, &body)
    }

    /// §4/§5: exchange a refresh token for a new access token, through the same `/token`.
    pub async fn refresh_access_token(
        &self,
        registration: &ClientRegistration,
        refresh_token: &str,
    ) -> Result<TokenPoll, SsoError> {
        let url = format!("{}/token", self.endpoints.oidc);
        let (status, body) = self
            .send_with_retry(&url, |url| {
                form_request(&self.http, url, &[
                    ("client_id", registration.client_id.as_str()),
                    ("client_secret", registration.client_secret.as_str()),
                    ("grant_type", REFRESH_GRANT),
                    ("refresh_token", refresh_token),
                ])
            })
            .await?;
        self.read_token(status, &body)
    }

    /// The `/token` table of `contracts §3`, in one place.
    fn read_token(&self, status: u16, body: &str) -> Result<TokenPoll, SsoError> {
        if status == 200 {
            let wire: TokenWire = serde_json::from_str(body).map_err(|_| {
                self.unavailable("the token endpoint answered an unexpected shape")
            })?;
            return Ok(TokenPoll::Approved(TokenSet {
                access_token: wire.access_token,
                refresh_token: wire.refresh_token,
                expires_at: Utc::now() + ChronoDuration::seconds(wire.expires_in as i64),
            }));
        }
        if status == 400 || status == 401 {
            let wire: TokenErrorWire = serde_json::from_str(body).map_err(|_| {
                self.unavailable("the token endpoint refused without naming a reason")
            })?;
            return Ok(match wire.error.as_str() {
                "authorization_pending" => TokenPoll::Pending,
                "slow_down" => TokenPoll::SlowDown,
                "access_denied" => TokenPoll::Denied,
                "expired_token" => TokenPoll::Expired,
                "invalid_grant" => TokenPoll::Used,
                other => TokenPoll::Refused(other.to_string()),
            });
        }
        Err(self.unavailable(format!("the token endpoint answered HTTP {status}")))
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
    deadline: DateTime<Utc>,
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
        let now = Utc::now();
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
    pub fn expires_at(&self) -> DateTime<Utc> {
        self.authorization.expires_at
    }

    /// The registration this flow polls with; the session envelope keeps it (§2).
    pub fn registration(&self) -> &ClientRegistration {
        &self.registration
    }

    /// Poll until the person answers or the code's life runs out. Never hangs: the loop checks
    /// the deadline before every poll, and each answer is its own error (§3, SC-002).
    ///
    /// A second call after a success returns [`SsoError::AlreadyConfirmed`]: one code mints one
    /// session.
    pub async fn complete(&mut self) -> Result<TokenSet, SsoError> {
        if self.confirmed {
            return Err(SsoError::AlreadyConfirmed);
        }
        loop {
            if Utc::now() >= self.deadline {
                return Err(SsoError::CodeExpired);
            }
            let poll = self
                .client
                .token_for_device_code(&self.registration, self.authorization.device_code())
                .await?;
            match poll {
                TokenPoll::Approved(tokens) => {
                    self.confirmed = true;
                    return Ok(tokens);
                }
                TokenPoll::Pending => tokio::time::sleep(self.interval).await,
                TokenPoll::SlowDown => {
                    self.interval += SLOW_DOWN_STEP;
                    tokio::time::sleep(self.interval).await;
                }
                TokenPoll::Denied => return Err(SsoError::SignInDenied),
                TokenPoll::Expired => return Err(SsoError::CodeExpired),
                TokenPoll::Used => return Err(SsoError::AlreadyConfirmed),
                TokenPoll::Refused(reason) => return Err(self.client.unavailable(reason)),
            }
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
    use crate::llm::sso::tests_support::{MemSessions, Stub, approved, denied, pending, used_twice};

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
        sessions: &SsoSessions<MemSessions>,
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
        let sessions = SsoSessions::new(MemSessions::default());

        sign_in(&mut flow, &sessions).await.unwrap();

        let stored = sessions.load(PROFILE).await.unwrap().expect("a session");
        assert_eq!(stored.access_token, "access-token-value");
        assert_eq!(stored.refresh_token.as_deref(), Some("refresh-token-value"));
        assert_eq!(stored.client_id, "client-id");
        assert_eq!(stored.revision, 1);
        assert_eq!(sessions.storage().len(), 1);

        // §3 request shapes reached the wire: the start URL, the grant and the device code.
        let seen = stub.seen();
        assert!(seen.iter().any(|(path, body)| path == "/device_authorization"
            && body.contains("start_url=https%3A%2F%2Facme.awsapps.com%2Fstart")));
        assert!(seen.iter().any(|(path, body)| path == "/token"
            && body.contains("grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code")
            && body.contains("device_code=device-code-value")));
        // The code the person reads is the one the stub issued.
        assert_eq!(flow.user_code(), "ABCD-EFGH");
        assert_eq!(flow.verification_uri(), "https://example.test/device");
    }

    #[tokio::test]
    async fn a_denied_sign_in_stores_nothing() {
        let stub = Stub::start(vec![denied()]).await;
        let mut flow = flow(&stub).await;
        let sessions = SsoSessions::new(MemSessions::default());

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
        let sessions = SsoSessions::new(MemSessions::default());

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
        let sessions = SsoSessions::new(MemSessions::default());

        let error = sign_in(&mut flow, &sessions).await.unwrap_err();

        assert_eq!(error, SsoError::AlreadyConfirmed);
        assert_eq!(sessions.storage().len(), 0);
    }

    #[tokio::test]
    async fn a_code_confirmed_twice_cannot_mint_a_second_session() {
        let stub = Stub::start(vec![approved()]).await;
        let mut flow = flow(&stub).await;
        let sessions = SsoSessions::new(MemSessions::default());

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
        let now = Utc.with_ymd_and_hms(2026, 11, 2, 12, 0, 0).unwrap();
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
                expires_at: Utc::now(),
                interval: Duration::from_secs(5),
            }
        );
        assert!(!flow_debug.contains("device-code-value"), "{flow_debug}");

        let tokens = TokenSet {
            access_token: "access-token-value".into(),
            refresh_token: Some("refresh-token-value".into()),
            expires_at: Utc::now(),
        };
        let printed = format!("{tokens:?}");
        for poison in ["access-token-value", "refresh-token-value"] {
            assert!(!printed.contains(poison), "{printed}");
        }
    }

}
