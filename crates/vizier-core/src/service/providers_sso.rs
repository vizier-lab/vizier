//! Bedrock over IAM Identity Center: the sign-in, account/role discovery and sign-out a person
//! performs from Settings › Providers (`specs/044-bedrock-sso-inference-profiles`, FR-012,
//! FR-009c, `contracts/provider-credentials-api.md` §3).
//!
//! Every operation here is callable **before** a connection is saved: the form supplies the start
//! URL, SSO region and profile name it is about to save, and the only thing kept between the two
//! calls is the device flow — in this process's memory, keyed by profile. The device code never
//! leaves that memory and never appears in a response (FR-009b, FR-035).
//!
//! The commands that need a session (accounts, roles, sign-out) fall back to the saved row for
//! whatever the caller omits, so a saved connection needs no query parameters at all.

use std::collections::HashMap;
use std::sync::LazyLock;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::auth::Principal;
use crate::config::provider::ProviderId;
use crate::dependencies::VizierDependencies;
use crate::llm::sso::{
    self, ClientRegistration, DeviceFlow, SsoError, SsoClient, SsoPortal, SsoSessions,
};
use crate::schema::ProviderCredentials;
use crate::service::ServiceError;
use crate::storage::provider::ProviderStorage;

const PERMISSION: &str = "settings:providers";

/// The device flows in progress, keyed by profile name.
///
/// Process-wide because the pending panel outlives one request: the person leaves for a browser
/// and comes back to a second call. A restart simply asks for a new code, which is what the
/// pending state means anyway.
static PENDING: LazyLock<Mutex<HashMap<String, DeviceFlow>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// What the form is about to save for one connection. Every field is optional; an omitted value
/// falls back to the saved row, so a saved connection starts a re-sign-in with an empty body
/// (FR-008).
#[derive(Debug, Default, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema, utoipa::IntoParams))]
#[serde(deny_unknown_fields)]
pub struct SignInRequest {
    pub start_url: Option<String>,
    pub sso_region: Option<String>,
    /// The profile name the session is stored under (FR-009e).
    pub profile: Option<String>,
}

/// The pending panel: the code and the URL, and never the device code (contracts §3).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SignInStarted {
    pub user_code: String,
    pub verification_uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_uri_complete: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub interval_seconds: u64,
}

/// What a poll of the pending sign-in answered: still waiting (`202`), or signed in (`200`).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum SignInProgress {
    Pending {
        expires_at: DateTime<Utc>,
    },
    SignedIn {
        #[serde(skip_serializing_if = "Option::is_none")]
        account_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        role_name: Option<String>,
        expires_at: DateTime<Utc>,
    },
}

/// One account the portal listed, for the picker (contracts §3).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AccountView {
    pub account_id: String,
    pub account_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

/// The account list, with the free-text fallback only when the portal listed nothing (FR-007).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AccountList {
    pub accounts: Vec<AccountView>,
    pub fallback: bool,
}

/// One role the identity may assume in an account.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct RoleView {
    pub role_name: String,
    pub account_id: String,
}

/// The role list, with the same fallback rule as [`AccountList`].
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct RoleList {
    pub roles: Vec<RoleView>,
    pub fallback: bool,
}

/// One answer to `POST /{id}/sso/sign-in` (contracts §3): a fresh code to type, the pending state
/// after a poll, or the completed sign-in. Untagged on the wire, exactly the three bodies the
/// contract shows — and none of them carries the device code, a token or a secret half (FR-009b).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(untagged)]
pub enum SignInResponse {
    Started(SignInStarted),
    Progress(SignInProgress),
}

impl SignInResponse {
    /// `200` for a code to show and for a completed sign-in; `202` while the browser step is still
    /// pending, so the form knows to poll again.
    pub fn status(&self) -> u16 {
        match self {
            Self::Started(_) | Self::Progress(SignInProgress::SignedIn { .. }) => 200,
            Self::Progress(SignInProgress::Pending { .. }) => 202,
        }
    }
}

/// After a sign-out the connection is configured and has no session; nothing else changed.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SignOutResponse {
    pub state: String,
}

/// The three values the flow needs, resolved from the request with the saved row underneath.
#[derive(Debug)]
struct Target {
    profile: String,
    sso_region: String,
}

/// Start a sign-in and answer with the code the person types. Callable before the connection is
/// saved (FR-012): nothing is written here except the pending flow in memory.
pub async fn start_sign_in(
    deps: &VizierDependencies,
    user: &Principal,
    id: &str,
    body: SignInRequest,
) -> Result<SignInStarted, ServiceError> {
    crate::access::require(user, PERMISSION)?;
    let id = sso_id(id)?;
    let saved = saved_credentials(deps, &id).await?;
    let target = target(&body, &saved)?;
    let start_url = body
        .start_url
        .as_deref()
        .filter(|url| !url.trim().is_empty())
        .or(saved.start_url.as_deref())
        .ok_or_else(|| {
            ServiceError::invalid(
                "the Identity Center start URL is missing: enter it, e.g. \
                 https://acme.awsapps.com/start",
            )
        })?;
    if !valid_start_url(start_url) {
        return Err(ServiceError::invalid(format!(
            "invalid Identity Center start URL {start_url:?}: use an https URL such as \
             https://acme.awsapps.com/start"
        )));
    }

    let client = SsoClient::new(deps.http.clone(), &target.sso_region);
    let registration = stored_registration(deps, &target.profile).await?;
    let flow = sso::begin_sign_in(client, registration, start_url)
        .await
        .map_err(ServiceError::from)?;

    let started = SignInStarted {
        user_code: flow.user_code().to_string(),
        verification_uri: flow.verification_uri().to_string(),
        verification_uri_complete: flow.verification_uri_complete().map(str::to_string),
        expires_at: flow.expires_at(),
        interval_seconds: flow.interval().as_secs(),
    };
    PENDING.lock().await.insert(target.profile, flow);
    Ok(started)
}

/// Poll the pending sign-in once. `Pending` leaves the flow in place for the next call; a
/// terminal answer consumes it, so one code mints at most one session (§3).
pub async fn complete_sign_in(
    deps: &VizierDependencies,
    user: &Principal,
    id: &str,
    body: SignInRequest,
) -> Result<SignInProgress, ServiceError> {
    crate::access::require(user, PERMISSION)?;
    let id = sso_id(id)?;
    let saved = saved_credentials(deps, &id).await?;
    let target = target(&body, &saved)?;

    let mut flow = PENDING.lock().await.remove(&target.profile).ok_or_else(|| {
        ServiceError::invalid(format!(
            "no sign-in is pending for profile {:?}; start the sign-in again for a new code",
            target.profile
        ))
    })?;

    match flow.poll_once().await {
        Ok(Some(tokens)) => {
            let persisted = SsoSessions::new(&*deps.storage)
                .store_sign_in(&target.profile, flow.registration(), tokens)
                .await
                .map_err(ServiceError::from)?;
            Ok(SignInProgress::SignedIn {
                account_id: saved.account_id.filter(|v| !v.is_empty()),
                role_name: saved.role_name.filter(|v| !v.is_empty()),
                expires_at: persisted.expires_at(),
            })
        }
        Ok(None) => {
            let expires_at = flow.expires_at();
            // A second start while this poll was in flight owns the profile now; keep theirs.
            PENDING
                .lock()
                .await
                .entry(target.profile)
                .or_insert(flow);
            Ok(SignInProgress::Pending { expires_at })
        }
        // Denied, expired and already-used are terminal: the code is spent either way.
        Err(error) => Err(ServiceError::from(error)),
    }
}

/// The one call the form's **Sign in** button makes (contracts §3): it starts a sign-in when none
/// is pending for the profile and continues the pending one otherwise, so a poll is the same
/// request with the same body. The answer says which of the two happened, and the code it shows is
/// the only thing that leaves this process (FR-009b, FR-012).
pub async fn sign_in(
    deps: &VizierDependencies,
    user: &Principal,
    id: &str,
    body: SignInRequest,
) -> Result<SignInResponse, ServiceError> {
    crate::access::require(user, PERMISSION)?;
    let id = sso_id(id)?;
    let saved = saved_credentials(deps, &id).await?;
    let target = target(&body, &saved)?;
    let pending = PENDING.lock().await.contains_key(&target.profile);
    if pending {
        complete_sign_in(deps, user, id.as_str(), body)
            .await
            .map(SignInResponse::Progress)
    } else {
        start_sign_in(deps, user, id.as_str(), body)
            .await
            .map(SignInResponse::Started)
    }
}

/// The accounts the signed-in identity was assigned. Runs on a session, so a connection may still
/// be unsaved; the profile and SSO region may come from the request instead of the row (FR-007,
/// FR-012).
pub async fn discover_accounts(
    deps: &VizierDependencies,
    user: &Principal,
    id: &str,
    body: SignInRequest,
) -> Result<AccountList, ServiceError> {
    crate::access::require(user, PERMISSION)?;
    let id = sso_id(id)?;
    let saved = saved_credentials(deps, &id).await?;
    let target = target(&body, &saved)?;
    let signed_in = signed_in(deps, &target).await?;

    let discovered = SsoPortal::new(signed_in.client)
        .accounts(&signed_in.session.access_token)
        .await
        .map_err(ServiceError::from)?;
    Ok(AccountList {
        accounts: discovered
            .accounts
            .into_iter()
            .map(|account| AccountView {
                account_id: account.account_id,
                account_name: account.account_name,
                email: account.email_address,
            })
            .collect(),
        fallback: discovered.free_text,
    })
}

/// The roles this identity may assume in `account_id`.
pub async fn discover_roles(
    deps: &VizierDependencies,
    user: &Principal,
    id: &str,
    account_id: &str,
    body: SignInRequest,
) -> Result<RoleList, ServiceError> {
    crate::access::require(user, PERMISSION)?;
    let id = sso_id(id)?;
    if !valid_account_id(account_id) {
        return Err(ServiceError::invalid(format!(
            "invalid AWS account id {account_id:?}: it is 12 digits, e.g. 123456789012"
        )));
    }
    let saved = saved_credentials(deps, &id).await?;
    let target = target(&body, &saved)?;
    let signed_in = signed_in(deps, &target).await?;

    let discovered = SsoPortal::new(signed_in.client)
        .roles(&signed_in.session.access_token, account_id)
        .await
        .map_err(ServiceError::from)?;
    Ok(RoleList {
        roles: discovered
            .roles
            .into_iter()
            .map(|role| RoleView {
                role_name: role.role_name,
                account_id: role.account_id,
            })
            .collect(),
        fallback: discovered.free_text,
    })
}

/// Sign out inside Vizier: delete the stored session row for the profile (FR-009c). The
/// connection, its entries and its agents are untouched, and signing out of a session that is
/// already gone is not an error.
pub async fn sign_out(
    deps: &VizierDependencies,
    user: &Principal,
    id: &str,
    body: SignInRequest,
) -> Result<SignOutResponse, ServiceError> {
    crate::access::require(user, PERMISSION)?;
    let id = sso_id(id)?;
    let saved = saved_credentials(deps, &id).await?;
    let Some(profile) = profile(&body, &saved)? else {
        return Ok(SignOutResponse {
            state: "configured".into(),
        });
    };
    PENDING.lock().await.remove(&profile);
    SsoSessions::new(&*deps.storage)
        .delete(&profile)
        .await
        .map_err(ServiceError::from)?;
    Ok(SignOutResponse {
        state: "configured".into(),
    })
}

// ---------------------------------------------------------------- shared plumbing

/// A signed-in session plus the region's client, so the portal calls share one construction.
struct SignedIn {
    session: crate::schema::SsoSessionSecret,
    client: SsoClient,
}

async fn signed_in(
    deps: &VizierDependencies,
    target: &Target,
) -> Result<SignedIn, ServiceError> {
    let client = SsoClient::new(deps.http.clone(), &target.sso_region);
    let sessions = SsoSessions::new(&*deps.storage);
    let session = sessions
        .load(&target.profile)
        .await
        .map_err(ServiceError::from)?
        .ok_or_else(|| ServiceError::from(SsoError::SessionExpired))?;
    let session = sessions
        .access_token(&client, &target.profile, session)
        .await
        .map_err(ServiceError::from)?;
    Ok(SignedIn { session, client })
}

/// The stored registration, when the session has one that has not expired: a re-sign-in then
/// asks only for a code (FR-008). An unreadable row is not a reason to refuse one (FR-009f).
async fn stored_registration(
    deps: &VizierDependencies,
    profile: &str,
) -> Result<Option<ClientRegistration>, ServiceError> {
    let Ok(Some(session)) = SsoSessions::new(&*deps.storage).load(profile).await else {
        return Ok(None);
    };
    if session
        .client_secret_expires_at
        .is_some_and(|expires| expires <= Utc::now())
    {
        return Ok(None);
    }
    Ok(Some(ClientRegistration {
        client_id: session.client_id,
        client_secret: session.client_secret,
        expires_at: session.client_secret_expires_at,
    }))
}

fn sso_id(id: &str) -> Result<ProviderId, ServiceError> {
    let id = ProviderId::parse(id).map_err(|_| ServiceError::invalid("invalid provider id"))?;
    if id != ProviderId::AMAZON_BEDROCK_SSO {
        return Err(ServiceError::invalid(format!(
            "provider \"{id}\" does not sign in through IAM Identity Center"
        )));
    }
    Ok(id)
}

async fn saved_credentials(
    deps: &VizierDependencies,
    id: &ProviderId,
) -> Result<ProviderCredentials, ServiceError> {
    deps.storage
        .get_provider(id)
        .await
        .map(|entry| entry.map(|entry| entry.credentials).unwrap_or_default())
        .map_err(|error| ServiceError::Internal(error.to_string()))
}

fn non_empty(value: Option<&String>) -> Option<&str> {
    value.map(String::as_str).filter(|value| !value.trim().is_empty())
}

/// The profile name: the request's, then the row's.
fn profile(
    body: &SignInRequest,
    saved: &ProviderCredentials,
) -> Result<Option<String>, ServiceError> {
    let Some(profile) = non_empty(body.profile.as_ref()).or_else(|| non_empty(saved.sso_profile.as_ref()))
    else {
        return Ok(None);
    };
    if !valid_profile(profile) {
        return Err(ServiceError::invalid(format!(
            "invalid profile name {profile:?}: use 1-64 letters, digits, '.', '_', '-' or a \
             space, with no leading or trailing space"
        )));
    }
    Ok(Some(profile.to_string()))
}

fn target(body: &SignInRequest, saved: &ProviderCredentials) -> Result<Target, ServiceError> {
    let profile = profile(body, saved)?.ok_or_else(|| {
        ServiceError::invalid(
            "the profile name is missing: enter it, so the sign-in has a non-secret handle",
        )
    })?;
    let sso_region = non_empty(body.sso_region.as_ref())
        .or_else(|| non_empty(saved.sso_region.as_ref()))
        .ok_or_else(|| {
            ServiceError::invalid(
                "the SSO region is missing: enter the region the Identity Center instance lives \
                 in, e.g. us-east-1",
            )
        })?;
    if !crate::llm::bedrock::valid_region(sso_region) {
        return Err(ServiceError::invalid(format!(
            "invalid SSO region {sso_region:?}: use a form like us-east-1"
        )));
    }
    Ok(Target {
        profile,
        sso_region: sso_region.to_string(),
    })
}

fn valid_start_url(url: &str) -> bool {
    // `Url` reads `https:///start` as host `start`, because the URL standard skips extra
    // slashes, so the authority has to be there in the source before the parse is trusted.
    let authority_is_there = url.split_once("://").is_some_and(|(_, rest)| {
        !rest.is_empty() && !rest.starts_with('/') && !rest.starts_with('\\')
    });
    authority_is_there
        && reqwest::Url::parse(url).is_ok_and(|url| {
            url.scheme() == "https"
                && url.host_str().is_some_and(|host| !host.is_empty())
                && url.username().is_empty()
                && url.password().is_none()
        })
}

fn valid_profile(profile: &str) -> bool {
    let length = profile.chars().count();
    (1..=64).contains(&length)
        && profile == profile.trim()
        && profile.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | ' ')
        })
}

fn valid_account_id(account_id: &str) -> bool {
    account_id.len() == 12 && account_id.chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_start_url_is_an_https_url_with_a_host() {
        assert!(valid_start_url("https://acme.awsapps.com/start"));
        assert!(valid_start_url("https://acme.awsapps.com/start#/"));
        assert!(!valid_start_url("http://acme.awsapps.com/start"));
        assert!(!valid_start_url("acme.awsapps.com/start"));
        assert!(!valid_start_url("https:///start"));
        assert!(!valid_start_url("https://user:pass@acme.awsapps.com/start"));
        assert!(!valid_start_url(""));
    }

    #[test]
    fn a_profile_name_is_the_documented_charset_and_length() {
        assert!(valid_profile("acme-bedrock"));
        assert!(valid_profile("Acme Prod_1.io"));
        assert!(!valid_profile(""));
        assert!(!valid_profile(" acme"));
        assert!(!valid_profile("acme "));
        assert!(!valid_profile("acme/bedrock"));
        assert!(!valid_profile(&"a".repeat(65)));
        assert!(valid_profile(&"a".repeat(64)));
    }

    #[test]
    fn an_account_id_is_twelve_digits() {
        assert!(valid_account_id("123456789012"));
        assert!(!valid_account_id("12345678901"));
        assert!(!valid_account_id("12345678901a"));
        assert!(!valid_account_id(""));
    }

    fn saved() -> ProviderCredentials {
        ProviderCredentials {
            start_url: Some("https://acme.awsapps.com/start".into()),
            sso_region: Some("us-east-1".into()),
            sso_profile: Some("acme-bedrock".into()),
            account_id: Some("123456789012".into()),
            role_name: Some("BedrockOperator".into()),
            ..Default::default()
        }
    }

    #[test]
    fn the_request_wins_over_the_saved_row_and_falls_back_to_it() {
        let resolved = target(&SignInRequest::default(), &saved()).unwrap();
        assert_eq!(resolved.profile, "acme-bedrock");
        assert_eq!(resolved.sso_region, "us-east-1");

        let body = SignInRequest {
            start_url: None,
            sso_region: Some("eu-west-1".into()),
            profile: Some("acme-eu".into()),
        };
        let overridden = target(&body, &saved()).unwrap();
        assert_eq!(overridden.profile, "acme-eu");
        assert_eq!(overridden.sso_region, "eu-west-1");
    }

    #[test]
    fn a_missing_profile_or_region_is_named_in_the_refusal() {
        let empty = ProviderCredentials::default();
        let error = target(&SignInRequest::default(), &empty).unwrap_err();
        assert!(error.message().contains("profile name"), "{error}");

        let no_region = ProviderCredentials {
            sso_profile: Some("acme".into()),
            ..Default::default()
        };
        let error = target(&SignInRequest::default(), &no_region).unwrap_err();
        assert!(error.message().contains("SSO region"), "{error}");

        let bad_region = ProviderCredentials {
            sso_profile: Some("acme".into()),
            sso_region: Some("eu-west".into()),
            ..Default::default()
        };
        let error = target(&SignInRequest::default(), &bad_region).unwrap_err();
        assert!(error.message().contains("invalid SSO region"), "{error}");
    }

    #[test]
    fn no_sign_in_answer_carries_a_device_code_token_or_secret_half() {
        let now = Utc::now();
        let answers = [
            SignInResponse::Started(SignInStarted {
                user_code: "ABCD-EFGH".into(),
                verification_uri: "https://device.sso.us-east-1.amazonaws.com/".into(),
                verification_uri_complete: Some(
                    "https://device.sso.us-east-1.amazonaws.com/?user_code=ABCD-EFGH".into(),
                ),
                expires_at: now,
                interval_seconds: 5,
            }),
            SignInResponse::Progress(SignInProgress::Pending { expires_at: now }),
            SignInResponse::Progress(SignInProgress::SignedIn {
                account_id: Some("123456789012".into()),
                role_name: Some("BedrockOperator".into()),
                expires_at: now,
            }),
        ];
        for answer in &answers {
            let value = serde_json::to_value(answer).unwrap();
            let text = value.to_string().to_lowercase();
            for forbidden in [
                "device_code",
                "access_token",
                "refresh_token",
                "client_secret",
                "secret_access_key",
                "session_token",
            ] {
                assert!(!text.contains(forbidden), "{forbidden} in {value}");
            }
        }

        // The statuses the form polls on: the code to show, still pending, signed in.
        assert_eq!(answers[0].status(), 200);
        assert_eq!(answers[1].status(), 202);
        assert_eq!(answers[2].status(), 200);

        // The wire states are the kebab-case names the contract shows.
        assert_eq!(
            serde_json::to_value(&answers[1]).unwrap()["state"],
            serde_json::json!("pending")
        );
        assert_eq!(
            serde_json::to_value(&answers[2]).unwrap()["state"],
            serde_json::json!("signed-in")
        );
        // The user code is not a secret: it is meant to be read out.
        assert_eq!(
            serde_json::to_value(&answers[0]).unwrap()["user_code"],
            serde_json::json!("ABCD-EFGH")
        );
    }

    #[test]
    fn sign_out_needs_no_region_and_no_saved_row() {
        assert_eq!(profile(&SignInRequest::default(), &ProviderCredentials::default()).unwrap(), None);
        let body = SignInRequest {
            profile: Some("acme".into()),
            ..Default::default()
        };
        assert_eq!(
            profile(&body, &ProviderCredentials::default()).unwrap().as_deref(),
            Some("acme")
        );
    }
}
