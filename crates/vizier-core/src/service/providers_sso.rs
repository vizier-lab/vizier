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

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::auth::Principal;
use crate::catalogue::data::Cost;
use crate::config::provider::ProviderId;
use crate::dependencies::VizierDependencies;
use crate::llm::sso::{
    self, ClientRegistration, DeviceFlow, SsoError, SsoClient, SsoPortal, SsoSessions,
};
use crate::schema::{LimitsEdited, ProfileEntry, ProviderCredentials, ProviderEntry};
use crate::service::providers::ProfileEntryResponse;
use crate::service::usage::{self, AffectedAgent};
use crate::service::ServiceError;
use crate::storage::agent::AgentStorage;
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
    pub expires_at: DateTime<Local>,
    pub interval_seconds: u64,
}

/// What a poll of the pending sign-in answered: still waiting (`202`), or signed in (`200`).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum SignInProgress {
    Pending {
        expires_at: DateTime<Local>,
    },
    SignedIn {
        #[serde(skip_serializing_if = "Option::is_none")]
        account_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        role_name: Option<String>,
        expires_at: DateTime<Local>,
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

/// Everything a re-sign-in reuses from the saved connection, so an ended session asks for the
/// confirmation code and nothing else (FR-008, US3 acceptance 5): the start URL, the SSO region,
/// the profile name, and the account and role that sign-in already resolved.
#[derive(Debug)]
struct ReSignIn {
    target: Target,
    start_url: String,
    account_id: Option<String>,
    role_name: Option<String>,
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
    let sign_in = re_sign_in(&body, &saved)?;
    let target = &sign_in.target;

    let client = SsoClient::new(deps.http.clone(), &target.sso_region);
    let registration = stored_registration(deps, &target.profile).await?;
    let flow = sso::begin_sign_in(client, registration, &sign_in.start_url)
        .await
        .map_err(ServiceError::from)?;

    let started = SignInStarted {
        user_code: flow.user_code().to_string(),
        verification_uri: flow.verification_uri().to_string(),
        verification_uri_complete: flow.verification_uri_complete().map(str::to_string),
        expires_at: flow.expires_at(),
        interval_seconds: flow.interval().as_secs(),
    };
    PENDING.lock().await.insert(target.profile.clone(), flow);
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
    let sign_in = re_sign_in(&body, &saved)?;
    let target = &sign_in.target;

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
            // FR-018: the session the person just renewed is what health reports again, in this
            // process and with no Vizier restart.
            deps.sso_failures.clear(&id);
            // FR-019: a completed sign-in is a change to this connection, so exactly the agents
            // using it restart through the existing provider-change path. Another provider's
            // agents are untouched, and Vizier itself keeps running.
            usage::restart_users(deps, &id).await;
            Ok(SignInProgress::SignedIn {
                account_id: sign_in.account_id.clone(),
                role_name: sign_in.role_name.clone(),
                expires_at: persisted.expires_at(),
            })
        }
        Ok(None) => {
            let expires_at = flow.expires_at();
            // A second start while this poll was in flight owns the profile now; keep theirs.
            PENDING
                .lock()
                .await
                .entry(target.profile.clone())
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
        .is_some_and(|expires| expires <= crate::time::now())
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

/// What a sign-in starts from, request over saved row, refusing a connection that cannot sign in
/// at all — and naming the one field that is missing (FR-008, FR-010).
fn re_sign_in(
    body: &SignInRequest,
    saved: &ProviderCredentials,
) -> Result<ReSignIn, ServiceError> {
    let target = target(body, saved)?;
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
    Ok(ReSignIn {
        // The account and the role are the connection's, not the request's: that is what makes a
        // re-sign-in ask for a confirmation code and nothing else (FR-008).
        account_id: non_empty(saved.account_id.as_ref()).map(str::to_string),
        role_name: non_empty(saved.role_name.as_ref()).map(str::to_string),
        start_url: start_url.to_string(),
        target,
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

// ---------------------------------------------------------------- inference profile entries

/// Validate one connection's whole inference-profile list, in one place (data-model "profile
/// entry", FR-021, FR-024, FR-025).
///
/// Every write path runs this before storing anything — `PUT /providers/{id}`'s whole-list save
/// and the add/edit/remove below — so a list one route refuses cannot be stored by another. The
/// passes follow the data model's numbered rules **in its order**: the ARN's shape, then
/// duplicates, then the region, then the display name, the pricing model and the hand-entered
/// price. The order is load-bearing: a list with two faults always reports the same one first —
/// the one a person has to fix before the other becomes visible.
///
/// `connection_region` is the connection's **Bedrock** region, never the SSO region. `None` is a
/// connection whose region is not filled in yet: the ARN's region then has nothing to disagree
/// with, so rule 3 has nothing to compare.
///
/// The pricing pairing (`model`, `price`) is checked here for shape only and takes no part in the
/// ARN rules: it never accepts, rejects, rewrites or region-checks an ARN, and it never fills
/// `context`, `output`, `tool_call` or `image_input` (FR-034a.6, SC-022).
pub fn validate_entries(
    entries: &mut [ProfileEntry],
    connection_region: Option<&str>,
) -> Result<(), ServiceError> {
    // Rule 1: the shape. Every ARN is parsed before anything else is judged, so a bad ARN in the
    // second row is never hidden behind a duplicate or a region problem in the first.
    let mut parsed = Vec::with_capacity(entries.len());
    for entry in entries.iter() {
        let arn = crate::llm::bedrock::parse_inference_profile_arn(&entry.arn).map_err(|error| {
            ServiceError::invalid(format!(
                "{:?} is not an inference profile ARN: {error}",
                entry.arn.trim()
            ))
        })?;
        parsed.push(arn);
    }

    // Rule 2: one ARN, one entry. Compared on the parsed value, so the same ARN with different
    // surrounding whitespace is still the same profile — and the existing entry is left alone
    // (FR-025).
    for (index, arn) in parsed.iter().enumerate() {
        if parsed[..index].iter().any(|seen| seen.raw == arn.raw) {
            return Err(ServiceError::invalid(format!(
                "the ARN {} is already one of this connection's profiles; edit that entry instead \
                 of adding it again",
                arn.raw
            )));
        }
    }

    // Rule 3: the ARN's region is the connection's Bedrock region (FR-024, SC-009).
    if let Some(region) = connection_region.filter(|region| !region.is_empty()) {
        for arn in &parsed {
            if arn.region != region {
                return Err(ServiceError::invalid(format!(
                    "the ARN names region {:?} but this connection uses {:?}",
                    arn.region, region
                )));
            }
        }
    }

    // Rules 4, 4a and 4b: the things a person typed beside the ARN.
    for entry in entries.iter() {
        if let Some(name) = entry.name.as_deref() {
            let length = name.trim().chars().count();
            if length > 100 {
                return Err(ServiceError::invalid(format!(
                    "an entry's name is at most 100 characters; this one is {length}"
                )));
            }
        }
        if let Some(model) = entry
            .model
            .as_deref()
            .map(str::trim)
            .filter(|model| !model.is_empty())
            && !valid_model_id(model)
        {
            return Err(ServiceError::invalid(format!(
                "invalid model id {model:?}: use the catalogue's id, e.g. \
                 anthropic.claude-sonnet-4-5-20250929-v1:0 (lowercase letters, digits, '.', ':' \
                 and '-')"
            )));
        }
        if let Some(price) = &entry.price
            && !valid_price(price)
        {
            return Err(ServiceError::invalid(
                "an entry's price must be a finite, non-negative rate per million tokens: check \
                 the input and output figures (and cache read/write, when set)",
            ));
        }
    }

    // Normalise what a person pasted: the ARN is stored trimmed — still the ARN they saved, with
    // no segment rewritten (FR-027) — and an empty name or model means "not set" (FR-026).
    for (entry, arn) in entries.iter_mut().zip(parsed) {
        entry.arn = arn.raw;
        entry.name = entry
            .name
            .take()
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty());
        entry.model = entry
            .model
            .take()
            .map(|model| model.trim().to_string())
            .filter(|model| !model.is_empty());
    }
    Ok(())
}

/// The limit fill of FR-034a.7: pairing a catalogue model with an ARN is also the act that writes
/// that model's `limit.context` and `limit.output` into the entry's two limits, for the ones the
/// person has not set. One function, run on **writes only**, after [`validate_entries`] — which
/// trims the model id and normalises the ARN this matches on. Never on a read, never in the model
/// list, never in the turn path, so no limit can refuse, delay or disable anything (FR-034a.5).
///
/// `base` is the **stored** list, and it is the whole of the provenance question: a submitted
/// number is the person's when it differs from the stored one, when it arrived on an add, or when
/// it was already claimed as theirs. Whatever `limits_edited` the caller submitted is discarded
/// and recomputed here — that is what makes a client echoing back what it displayed a no-op
/// (contracts §4b rule 1). The capabilities are never touched (FR-034a.6, SC-022).
pub fn fill_limits(
    catalogue: &crate::catalogue::Catalogue,
    base: &[ProfileEntry],
    entries: &mut [ProfileEntry],
) {
    for entry in entries.iter_mut() {
        let stored = base.iter().find(|stored| stored.arn.trim() == entry.arn.trim());

        // Rule 1: which of the two numbers is the person's. An entry saved before this rule carries
        // no provenance, and reads as "any number it holds is the person's", so no old value is
        // adopted as a fill by accident.
        let edited_context = match stored {
            None => entry.context.is_some(),
            Some(stored) => {
                entry.context != stored.context
                    || stored
                        .limits_edited
                        .map_or(stored.context.is_some(), |edited| edited.context)
            }
        };
        let edited_output = match stored {
            None => entry.output.is_some(),
            Some(stored) => {
                entry.output != stored.output
                    || stored
                        .limits_edited
                        .map_or(stored.output.is_some(), |edited| edited.output)
            }
        };

        let pair_changed = match stored {
            None => entry.model.is_some(),
            Some(stored) => entry.model != stored.model,
        };
        // Rule 3: removing a pair fills nothing. A window that was resolved does not stop being
        // true, and removing the pair is not a reason to forget it.
        let pair_removed =
            entry.model.is_none() && stored.is_some_and(|stored| stored.model.is_some());

        // Rules 2 and 4: a pair that is set or changed supplies both limits; an unchanged pair
        // fills only a still-empty, unedited field — the catalogue had not resolved the id when the
        // pair was made, and it does now.
        let model = entry.model.clone();
        let fills = |edited: bool, present: bool| {
            !edited && !pair_removed && (pair_changed || (!present && model.is_some()))
        };
        let context = fills(edited_context, entry.context.is_some());
        let output = fills(edited_output, entry.output.is_some());
        if context || output {
            // The paired row's own numbers, or nothing: an id the catalogue cannot resolve and a
            // row that carries no figure both leave the limit unknown — never "unsupported", never a
            // number from the ARN or from the profile name (rule 4).
            let row = model
                .as_deref()
                .and_then(|id| catalogue.model(&ProviderId::AMAZON_BEDROCK_SSO, id));
            if context {
                entry.context = row.as_ref().and_then(|model| model.limit.context);
            }
            if output {
                entry.output = row.as_ref().and_then(|model| model.limit.output);
            }
        }
        entry.limits_edited = Some(LimitsEdited {
            context: edited_context,
            output: edited_output,
        });
    }
}

/// A catalogue Bedrock id's charset, checked for shape only: an id that has left the catalogue
/// must stay savable and be reported as "pricing not resolved" when used (FR-034a.3, FR-034a.5).
fn valid_model_id(model: &str) -> bool {
    let mut bytes = model.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    (first.is_ascii_lowercase() || first.is_ascii_digit())
        && bytes.all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b':' | b'-')
        })
}

/// A hand-entered cost: finite and non-negative, with no upper bound — a person may know an
/// enterprise rate the catalogue does not carry (FR-034a.4).
fn valid_price(price: &Cost) -> bool {
    let ok = |value: f64| value.is_finite() && value >= 0.0;
    ok(price.input)
        && ok(price.output)
        && price.cache_read.is_none_or(ok)
        && price.cache_write.is_none_or(ok)
}

/// The connection's saved entries, as the settings row shows them (contracts §4).
pub async fn entries(
    deps: &VizierDependencies,
    user: &Principal,
    id: &str,
) -> Result<Vec<ProfileEntryResponse>, ServiceError> {
    crate::access::require(user, PERMISSION)?;
    let id = sso_id(id)?;
    let credentials = saved_credentials(deps, &id).await?;
    Ok(response(deps, &credentials.profiles))
}

/// Add one entry. The whole list is validated, so a duplicate is refused with the existing entry
/// and its name untouched (FR-025).
pub async fn add_entry(
    deps: &VizierDependencies,
    user: &Principal,
    id: &str,
    entry: ProfileEntry,
) -> Result<Vec<ProfileEntryResponse>, ServiceError> {
    crate::access::require(user, PERMISSION)?;
    let id = sso_id(id)?;
    let saved = saved_entry(deps, &id).await?;
    let mut profiles = saved.credentials.profiles.clone();
    profiles.push(entry);
    validate_entries(&mut profiles, saved.credentials.region.as_deref())?;
    // The pairing is the fill (FR-034a.7), matched against the list as it was stored: an added
    // entry has no base row, so every number submitted with it is the person's.
    fill_limits(&deps.catalogue, &saved.credentials.profiles, &mut profiles);
    store_entries(deps, saved, profiles).await
}

/// Replace the entry saved under `arn`. Editing the ARN itself is a removal of the old profile,
/// so an agent still pointing at it is named first, exactly as a removal would (FR-029).
pub async fn update_entry(
    deps: &VizierDependencies,
    user: &Principal,
    id: &str,
    arn: &str,
    entry: ProfileEntry,
    confirm: bool,
) -> Result<Vec<ProfileEntryResponse>, ServiceError> {
    crate::access::require(user, PERMISSION)?;
    let id = sso_id(id)?;
    let saved = saved_entry(deps, &id).await?;
    let wanted = arn.trim();
    let Some(index) = saved
        .credentials
        .profiles
        .iter()
        .position(|existing| existing.arn.trim() == wanted)
    else {
        return Err(missing_entry(wanted));
    };
    let previous = saved.credentials.profiles[index].arn.clone();
    let mut profiles = saved.credentials.profiles.clone();
    profiles[index] = entry;
    validate_entries(&mut profiles, saved.credentials.region.as_deref())?;
    // The stored list is the base, so a pair that changed refreshes the limits the person has not
    // edited and leaves the others alone (FR-034a.7).
    fill_limits(&deps.catalogue, &saved.credentials.profiles, &mut profiles);
    if profiles[index].arn != previous {
        check_agents_use(deps, &previous, confirm).await?;
    }
    store_entries(deps, saved, profiles).await
}

/// Remove one entry. An agent that still chats through it is named in a `409` unless the removal
/// is confirmed — the same shape `usage::check_in_use` gives a provider delete (FR-029).
pub async fn remove_entry(
    deps: &VizierDependencies,
    user: &Principal,
    id: &str,
    arn: &str,
    confirm: bool,
) -> Result<Vec<ProfileEntryResponse>, ServiceError> {
    crate::access::require(user, PERMISSION)?;
    let id = sso_id(id)?;
    let saved = saved_entry(deps, &id).await?;
    let wanted = arn.trim();
    let Some(index) = saved
        .credentials
        .profiles
        .iter()
        .position(|existing| existing.arn.trim() == wanted)
    else {
        return Err(missing_entry(wanted));
    };
    let mut profiles = saved.credentials.profiles.clone();
    let removed = profiles.remove(index);
    check_agents_use(deps, &removed.arn, confirm).await?;
    validate_entries(&mut profiles, saved.credentials.region.as_deref())?;
    // Harmless for a removal, and it completes a fill still pending on a survivor whose pair was
    // made before the catalogue resolved its id (FR-034a.7 rule 4).
    fill_limits(&deps.catalogue, &saved.credentials.profiles, &mut profiles);
    store_entries(deps, saved, profiles).await
}

/// The saved connection row an entry belongs to. Entries are never stored on their own: a row
/// holding only a profile list would be a connection missing its five fields, so the form is
/// saved first and the entries are edited beside it (data-model "SSO connection").
async fn saved_entry(
    deps: &VizierDependencies,
    id: &ProviderId,
) -> Result<ProviderEntry, ServiceError> {
    deps.storage
        .get_provider(id)
        .await
        .map_err(|error| ServiceError::Internal(error.to_string()))?
        .filter(|entry| !entry.credentials.is_empty())
        .ok_or_else(|| {
            ServiceError::invalid(
                "save the Bedrock (SSO) connection before adding inference profiles: the entries \
                 belong to a saved connection",
            )
        })
}

fn missing_entry(arn: &str) -> ServiceError {
    ServiceError::NotFound(format!(
        "this connection has no inference profile {arn:?}; add it first"
    ))
}

async fn store_entries(
    deps: &VizierDependencies,
    mut entry: ProviderEntry,
    profiles: Vec<ProfileEntry>,
) -> Result<Vec<ProfileEntryResponse>, ServiceError> {
    entry.credentials.profiles = profiles;
    deps.storage
        .upsert_provider(&entry)
        .await
        .map_err(|error| ServiceError::Internal(error.to_string()))?;
    // The agents on this connection resolve their model on their next turn, so a new or edited
    // entry takes effect now rather than at the next restart (the provider-change path, FR-019).
    usage::restart_users(deps, &entry.id).await;
    Ok(response(deps, &entry.credentials.profiles))
}

fn response(deps: &VizierDependencies, profiles: &[ProfileEntry]) -> Vec<ProfileEntryResponse> {
    profiles
        .iter()
        .map(|entry| ProfileEntryResponse::from_entry(&deps.catalogue, entry))
        .collect()
}

/// The agents that would lose a profile if `arn` went away: the chat model and the dream model,
/// the two places an agent can name one (media refuses an ARN before the request, FR-033a).
async fn check_agents_use(
    deps: &VizierDependencies,
    arn: &str,
    confirm: bool,
) -> Result<(), ServiceError> {
    if confirm {
        return Ok(());
    }
    let arn = arn.trim();
    let agents: Vec<AffectedAgent> = deps
        .storage
        .list_agents()
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, config)| {
            (config.provider == ProviderId::AMAZON_BEDROCK_SSO && config.model.trim() == arn)
                || (config.dream_provider.as_ref() == Some(&ProviderId::AMAZON_BEDROCK_SSO)
                    && config.dream_model.as_deref().map(str::trim) == Some(arn))
        })
        .map(|(id, config)| AffectedAgent {
            id,
            name: config.name,
            features: vec!["chat".into()],
        })
        .collect();
    if agents.is_empty() {
        return Ok(());
    }
    // The same `409` a provider delete gives, word for word apart from what is being removed
    // (FR-029, FR-040).
    Err(usage::in_use_conflict(
        &format!("inference profile {arn}"),
        agents,
        "remove it",
    ))
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

    /// T055: a re-sign-in starts from the saved connection, so an ended session asks for the
    /// confirmation code and nothing else (FR-008, US3 acceptance 5).
    #[test]
    fn a_re_sign_in_reuses_the_saved_connection_and_asks_for_nothing_else() {
        // An empty body is the whole request: the start URL, the SSO region, the profile, the
        // account and the role all come from the row.
        let body: SignInRequest = serde_json::from_str("{}").unwrap();
        let reuse = re_sign_in(&body, &saved()).unwrap();
        assert_eq!(reuse.start_url, "https://acme.awsapps.com/start");
        assert_eq!(reuse.target.sso_region, "us-east-1");
        assert_eq!(reuse.target.profile, "acme-bedrock");
        assert_eq!(reuse.account_id.as_deref(), Some("123456789012"));
        assert_eq!(reuse.role_name.as_deref(), Some("BedrockOperator"));

        // The account and the role are not even fields of a sign-in, so nothing can ask for
        // them again (FR-008).
        assert!(
            serde_json::from_str::<SignInRequest>(r#"{"account_id":"999999999999"}"#).is_err()
        );
        assert!(serde_json::from_str::<SignInRequest>(r#"{"role_name":"Other"}"#).is_err());

        // A request may still name another profile, and then that one is honoured.
        let other = SignInRequest {
            profile: Some("acme-eu".into()),
            ..Default::default()
        };
        let reuse = re_sign_in(&other, &saved()).unwrap();
        assert_eq!(reuse.target.profile, "acme-eu");
        assert_eq!(
            reuse.account_id.as_deref(),
            Some("123456789012"),
            "the account still comes from the connection"
        );

        // A connection that cannot sign in at all says which field is missing.
        let mut no_url = saved();
        no_url.start_url = None;
        let error = re_sign_in(&SignInRequest::default(), &no_url).unwrap_err();
        assert!(error.message().contains("start URL"), "{error}");
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
        let now = crate::time::now();
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
    // ---- inference profile entries (data-model "profile entry", FR-021, FR-024, FR-025)

    const ARN_A: &str =
        "arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/abc123";
    const ARN_B: &str = "arn:aws:bedrock:us-east-1:123456789012:inference-profile/xyz789";

    fn profile_entry(arn: &str) -> ProfileEntry {
        ProfileEntry {
            arn: arn.into(),
            ..Default::default()
        }
    }

    fn price(input: f64, output: f64) -> Cost {
        Cost {
            input,
            output,
            cache_read: None,
            cache_write: None,
        }
    }

    #[test]
    fn a_valid_list_is_stored_trimmed_and_an_empty_name_means_no_name() {
        let mut entries = vec![
            ProfileEntry {
                arn: format!("  {ARN_A}  "),
                name: Some("  Acme Prod  ".into()),
                context: Some(200_000),
                output: Some(64_000),
                tool_call: Some(true),
                image_input: Some(false),
                model: Some("  anthropic.claude-sonnet-4-5-20250929-v1:0  ".into()),
                price: Some(price(3.0, 15.0)),
                edited: true,
                ..Default::default()
            },
            ProfileEntry {
                arn: ARN_B.into(),
                name: Some("   ".into()),
                ..Default::default()
            },
        ];
        validate_entries(&mut entries, Some("us-east-1")).unwrap();

        assert_eq!(entries[0].arn, ARN_A, "trimmed, and no segment rewritten");
        assert_eq!(entries[0].name.as_deref(), Some("Acme Prod"));
        assert_eq!(
            entries[0].model.as_deref(),
            Some("anthropic.claude-sonnet-4-5-20250929-v1:0")
        );
        // Validation itself writes no limit: the fill is the separate act that follows it
        // (FR-034a.7).
        assert_eq!(entries[0].context, Some(200_000));
        assert_eq!(entries[0].output, Some(64_000));
        assert_eq!(entries[0].tool_call, Some(true));
        assert_eq!(entries[0].image_input, Some(false));
        assert_eq!(entries[1].name, None, "an empty name is no name");
    }

    #[test]
    fn the_shape_is_judged_before_the_duplicate_which_is_judged_before_the_region() {
        // Three faults at once, one per rule: the ARN's shape is the one reported, in the second
        // row and after the duplicate (FR-023, FR-025, FR-024).
        let mut entries = vec![
            profile_entry(ARN_A),
            profile_entry(ARN_A),
            profile_entry("arn:aws:bedrock:eu-west-1:123456789012:inference-profile/x"),
            profile_entry("not-an-arn"),
        ];
        let error = validate_entries(&mut entries, Some("us-east-1")).unwrap_err();
        assert_eq!(error.status(), 400);
        assert!(
            error
                .message()
                .starts_with("\"not-an-arn\" is not an inference profile ARN"),
            "{error}"
        );

        // Every ARN well formed: the duplicate wins over the region mismatch, and the message
        // names the ARN rather than blaming the region.
        let mut entries = vec![
            profile_entry(ARN_A),
            profile_entry(ARN_A),
            profile_entry("arn:aws:bedrock:eu-west-1:123456789012:inference-profile/x"),
        ];
        let error = validate_entries(&mut entries, Some("us-east-1")).unwrap_err();
        assert!(
            error
                .message()
                .contains("already one of this connection's profiles"),
            "{error}"
        );

        // Only the region is left, and it names both regions (FR-024, SC-009).
        let mut entries = vec![profile_entry(ARN_A)];
        let error = validate_entries(&mut entries, Some("eu-west-1")).unwrap_err();
        assert_eq!(
            error.message(),
            "the ARN names region \"us-east-1\" but this connection uses \"eu-west-1\""
        );

        // A connection with no Bedrock region yet has nothing for the ARN's region to disagree
        // with, so the list is accepted.
        let mut entries = vec![profile_entry(ARN_A)];
        validate_entries(&mut entries, None).unwrap();
    }

    #[test]
    fn every_arn_refusal_names_the_value_and_the_expected_shape() {
        // Rule 1 walks every row before anything else is judged, so each refusal reaches the
        // person through one message: the value they typed (trimmed, quoted) and what an
        // inference-profile ARN would look like (FR-023, SC-008).
        for (value, cause) in [
            ("", "an inference profile ARN starts with \"arn:\""),
            (
                "anthropic.claude-sonnet-4-5-20250929-v1:0",
                "an inference profile ARN starts with \"arn:\"",
            ),
            (
                "https://console.aws.amazon.com/bedrock/home?region=us-east-1#/inference-profiles",
                "an inference profile ARN starts with \"arn:\"",
            ),
            (
                "arn:aws:s3:us-east-1:123456789012:inference-profile/x",
                "\"s3\" is not a Bedrock ARN",
            ),
            (
                "arn:aws:bedrock:us-east-1:123456789012:foundation-model/anthropic.claude-3-5-sonnet-20241022-v2:0",
                "names \"inference-profile\" or \"application-inference-profile\"",
            ),
            ("arn:aws:bedrock:us-east:123456789012:inference-profile/x", "is not an AWS region"),
            (
                "arn:aws:bedrock:us-east-1:123:inference-profile/x",
                "must be 12 digits",
            ),
            (
                "arn:aws:bedrock:us-east-1:123456789012:inference-profile",
                "names no profile id",
            ),
            (
                "arn:aws:bedrock:us-east-1:123456789012",
                "an inference profile ARN has the form",
            ),
            (
                "arn:aws-iso-e:bedrock:us-iso-east-1:123456789012:inference-profile/x",
                "unsupported ARN partition \"aws-iso-e\"",
            ),
        ] {
            let mut entries = vec![ProfileEntry {
                arn: value.into(),
                name: Some("  kept  ".into()),
                model: Some("  amazon.nova-lite-v1:0  ".into()),
                ..Default::default()
            }];
            let error = validate_entries(&mut entries, Some("us-east-1")).unwrap_err();
            assert_eq!(error.status(), 400);
            let message = error.message();
            assert!(
                message.starts_with(&format!("{:?} is not an inference profile ARN: ", value.trim())),
                "{value:?} gave {message:?}"
            );
            assert!(message.contains(cause), "{value:?} gave {message:?}");
            // A refused list is not half-normalised: the row is exactly what came in.
            assert_eq!(entries[0].arn, value);
            assert_eq!(entries[0].name.as_deref(), Some("  kept  "));
            assert_eq!(entries[0].model.as_deref(), Some("  amazon.nova-lite-v1:0  "));
        }

        // Two malformed rows: the first is named, not the last, and the second is never hidden
        // behind it.
        let mut entries = vec![
            profile_entry(ARN_A),
            profile_entry("bad-one"),
            profile_entry("bad-two"),
        ];
        let error = validate_entries(&mut entries, Some("us-east-1")).unwrap_err();
        assert!(error.message().contains("\"bad-one\""), "{error}");
        assert!(!error.message().contains("\"bad-two\""), "{error}");
    }

    #[test]
    fn the_duplicate_is_judged_on_the_parsed_arn_and_the_region_only_after_it() {
        // Rule 2 compares parsed ARNs, so padding differs but the profile does not; the message
        // names the trimmed ARN and says how to change it (FR-025).
        let mut entries = vec![profile_entry(ARN_A), profile_entry(&format!("  {ARN_A}  "))];
        let error = validate_entries(&mut entries, Some("eu-west-1")).unwrap_err();
        assert_eq!(
            error.message(),
            format!(
                "the ARN {ARN_A} is already one of this connection's profiles; edit that entry \
                 instead of adding it again"
            )
        );

        // Rule 3 runs on a list with no duplicate at all, and names the first ARN whose region
        // disagrees — both rows here are out of region, so the earlier one is reported.
        let mut entries = vec![
            profile_entry("arn:aws:bedrock:eu-west-1:123456789012:inference-profile/a"),
            profile_entry(ARN_B),
        ];
        let error = validate_entries(&mut entries, Some("us-east-1")).unwrap_err();
        assert_eq!(
            error.message(),
            "the ARN names region \"eu-west-1\" but this connection uses \"us-east-1\""
        );

        // An empty connection region is no region to compare against.
        let mut entries = vec![profile_entry("arn:aws:bedrock:eu-west-1:123456789012:inference-profile/a")];
        validate_entries(&mut entries, Some("")).unwrap();
    }

    #[test]
    fn the_pricing_pairing_is_shape_checked_and_never_touches_the_arn() {
        // A pair from another family and another region, over an ARN that is fine: accepted,
        // because the pairing prices and never validates or region-checks (FR-034a.6, SC-022).
        let mut entries = vec![ProfileEntry {
            arn: ARN_A.into(),
            model: Some("amazon.nova-lite-v1:0".into()),
            price: Some(price(0.0, 0.0)),
            ..Default::default()
        }];
        validate_entries(&mut entries, Some("us-east-1")).unwrap();
        // And validation filled nothing in from the paired model: the limit fill is the
        // separate act that follows it, never a side effect of the shape check (FR-034a.7).
        assert_eq!(entries[0].context, None);
        assert_eq!(entries[0].output, None);
        assert_eq!(entries[0].tool_call, None);
        assert_eq!(entries[0].image_input, None);

        for bad_model in ["Anthropic.Claude", "claude sonnet", "-claude", "claude/sonnet"] {
            let mut entries = vec![ProfileEntry {
                arn: ARN_A.into(),
                model: Some(bad_model.into()),
                ..Default::default()
            }];
            let error = validate_entries(&mut entries, Some("us-east-1")).unwrap_err();
            assert!(error.message().contains("invalid model id"), "{error}");
        }
        // An empty selection is "no model paired", not an invalid id (FR-034a.2).
        let mut entries = vec![ProfileEntry {
            arn: ARN_A.into(),
            model: Some("   ".into()),
            ..Default::default()
        }];
        validate_entries(&mut entries, Some("us-east-1")).unwrap();
        assert_eq!(entries[0].model, None);

        // A price is finite and non-negative, with no upper bound (FR-034a.4).
        let mut nan = price(f64::NAN, 1.0);
        nan.cache_read = Some(-0.5);
        for bad in [price(-1.0, 1.0), nan, price(1.0, f64::INFINITY)] {
            let mut entries = vec![ProfileEntry {
                arn: ARN_A.into(),
                price: Some(bad),
                ..Default::default()
            }];
            let error = validate_entries(&mut entries, Some("us-east-1")).unwrap_err();
            assert!(error.message().contains("finite, non-negative"), "{error}");
        }
        let mut entries = vec![ProfileEntry {
            arn: ARN_A.into(),
            price: Some(price(1_000.0, 2_000.0)),
            ..Default::default()
        }];
        validate_entries(&mut entries, Some("us-east-1")).unwrap();

        // The name is 1-100 characters once trimmed; longer is refused (FR-026).
        let mut entries = vec![ProfileEntry {
            arn: ARN_A.into(),
            name: Some("a".repeat(101)),
            ..Default::default()
        }];
        let error = validate_entries(&mut entries, Some("us-east-1")).unwrap_err();
        assert!(error.message().contains("at most 100 characters"), "{error}");
        let mut entries = vec![ProfileEntry {
            arn: ARN_A.into(),
            name: Some("a".repeat(100)),
            ..Default::default()
        }];
        validate_entries(&mut entries, Some("us-east-1")).unwrap();
    }

    // ---- the agents an entry removal would break (FR-029, US5 acceptance 3)

    async fn deps() -> (VizierDependencies, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let config = crate::config::VizierConfig {
            workspace: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        };
        (VizierDependencies::new(config).await.unwrap(), dir)
    }

    /// The owner as `providers.rs` builds one: the permission check is the same door.
    fn system() -> crate::auth::Principal {
        crate::auth::Principal {
            user_id: "owner".into(),
            username: "owner".into(),
            role: crate::auth::Role {
                role_id: "r".into(),
                name: "r".into(),
                permissions: vec![],
                is_system: true,
                created_at: crate::time::now(),
            },
            permissions: vec![],
            method: crate::auth::AuthMethod::Jwt,
        }
    }

    /// FR-029 / US5 acceptance 3: removing an entry names the agent chatting through it and the
    /// one dreaming through it, the saved row keeps the entry, and confirm is a no-op at the
    /// check — after which the removal really goes through.
    #[tokio::test]
    async fn removing_an_entry_an_agent_uses_names_it_until_confirmed() {
        let (deps, _dir) = deps().await;
        deps.storage
            .upsert_provider(&ProviderEntry {
                id: ProviderId::AMAZON_BEDROCK_SSO.clone(),
                credentials: ProviderCredentials {
                    profiles: vec![profile_entry(ARN_A)],
                    ..saved()
                },
            })
            .await
            .unwrap();

        let mut chat = crate::tests_support::agent_config("acme-chat");
        chat.name = "Acme Chat".into();
        chat.provider = ProviderId::AMAZON_BEDROCK_SSO;
        chat.model = ARN_A.into();
        deps.storage.create_agent("acme-chat", &chat).await.unwrap();

        let mut dream = crate::tests_support::agent_config("acme-dream");
        dream.name = "Acme Dream".into();
        dream.dream_provider = Some(ProviderId::AMAZON_BEDROCK_SSO);
        dream.dream_model = Some(ARN_A.into());
        deps.storage
            .create_agent("acme-dream", &dream)
            .await
            .unwrap();

        let (message, data) =
            match remove_entry(&deps, &system(), "amazon-bedrock-sso", ARN_A, false).await {
                Err(ServiceError::Conflict { message, data, .. }) => (message, data),
                Err(other) => panic!("an entry in use is a conflict, not {other:?}"),
                Ok(_) => panic!("both agents use this entry and neither was named"),
            };

        assert!(
            message.starts_with(&format!("inference profile {ARN_A} is used by ")),
            "{message}"
        );
        assert!(message.contains("Acme Chat"), "{message}");
        assert!(message.contains("Acme Dream"), "{message}");
        assert!(
            message.ends_with("; resend with ?confirm=true to remove it anyway"),
            "{message}"
        );

        let affected = data.expect("the affected agents");
        let agents = affected["agents"].as_array().expect("an array of agents");
        assert_eq!(agents.len(), 2, "one per kind of use: {affected}");
        let mut named: Vec<&str> = agents
            .iter()
            .map(|agent| agent["id"].as_str().expect("an id"))
            .collect();
        named.sort();
        assert_eq!(named, vec!["acme-chat", "acme-dream"]);

        // Nothing was removed: the refusal is the whole of the call so far.
        let row = deps
            .storage
            .get_provider(&ProviderId::AMAZON_BEDROCK_SSO)
            .await
            .unwrap()
            .expect("the connection is still saved");
        assert_eq!(row.credentials.profiles.len(), 1);
        assert_eq!(row.credentials.profiles[0].arn, ARN_A);

        // The confirmation is the whole difference: the check itself is a no-op.
        check_agents_use(&deps, ARN_A, true)
            .await
            .expect("a confirmed removal is not held back");

        // With no agent left the confirmed removal really takes the entry away, and nothing
        // waits on an agent supervisor to restart.
        deps.storage.delete_agent("acme-chat").await.unwrap();
        deps.storage.delete_agent("acme-dream").await.unwrap();
        let left = remove_entry(&deps, &system(), "amazon-bedrock-sso", ARN_A, true)
            .await
            .expect("a confirmed removal goes through");
        assert!(left.is_empty(), "the only entry is gone: {left:?}");
    }

    // ---- the limit fill (FR-034a.7, SC-022)

    const GROK: &str = "us.xai.grok-4.7";
    const GROK_GLOBAL: &str = "global.xai.grok-4.7";
    const NOVA: &str = "amazon.nova-lite-v1:0";

    /// The catalogue shape the fill reads: the `amazon-bedrock` row `catalogue.rs` tests build,
    /// carrying SC-022's reference numbers for the two `xai.grok-4.7` ids and one smaller model for
    /// the re-pairing cases. `tool_call` is deliberately `true` on every row, so a fill that
    /// touched a capability would be visible.
    const FIXTURE: &[u8] = br#"{
        "amazon-bedrock": {
            "id": "amazon-bedrock", "name": "Amazon Bedrock", "npm": "@ai-sdk/amazon-bedrock",
            "models": {
                "us.xai.grok-4.7": {"id": "us.xai.grok-4.7", "tool_call": true, "limit": {"context": 500000, "output": 500000}},
                "global.xai.grok-4.7": {"id": "global.xai.grok-4.7", "tool_call": true, "limit": {"context": 500000, "output": 500000}},
                "amazon.nova-lite-v1:0": {"id": "amazon.nova-lite-v1:0", "tool_call": true, "limit": {"context": 300000, "output": 5000}}
            }
        }
    }"#;

    /// The same rows after a catalogue refresh: the numbers moved, the ids did not.
    const REFRESHED: &[u8] = br#"{
        "amazon-bedrock": {
            "id": "amazon-bedrock", "name": "Amazon Bedrock", "npm": "@ai-sdk/amazon-bedrock",
            "models": {
                "us.xai.grok-4.7": {"id": "us.xai.grok-4.7", "tool_call": true, "limit": {"context": 400000, "output": 400000}},
                "amazon.nova-lite-v1:0": {"id": "amazon.nova-lite-v1:0", "tool_call": true, "limit": {"context": 111000, "output": 2222}}
            }
        }
    }"#;

    fn catalogue_of(json: &[u8]) -> crate::catalogue::Catalogue {
        crate::catalogue::Catalogue::new(crate::catalogue::data::parse(json).unwrap())
    }

    fn limits_catalogue() -> crate::catalogue::Catalogue {
        catalogue_of(FIXTURE)
    }

    fn paired(arn: &str, model: &str) -> ProfileEntry {
        ProfileEntry {
            model: Some(model.into()),
            ..profile_entry(arn)
        }
    }

    /// One write, as every write point runs it: validation first — which trims the model id and
    /// normalises the ARN the fill matches on — then the fill against the stored list.
    fn write(
        catalogue: &crate::catalogue::Catalogue,
        base: &[ProfileEntry],
        mut entries: Vec<ProfileEntry>,
    ) -> Vec<ProfileEntry> {
        validate_entries(&mut entries, Some("us-east-1")).unwrap();
        fill_limits(catalogue, base, &mut entries);
        entries
    }

    #[test]
    fn pairing_the_reference_model_fills_both_limits_with_no_number_typed() {
        // SC-022's reference case: pairing an ARN that carries no numbers with a model whose row
        // holds 500000 for each limit fills both in the same act, and both flags stay false.
        let catalogue = limits_catalogue();
        let entries = write(&catalogue, &[], vec![paired(ARN_A, GROK)]);
        assert_eq!(entries[0].context, Some(500_000));
        assert_eq!(entries[0].output, Some(500_000));
        assert_eq!(
            entries[0].limits_edited,
            Some(LimitsEdited { context: false, output: false }),
            "the fill is not the person's number"
        );

        // The global row fills the same two numbers: only its price differs (SC-020).
        let entries = write(&catalogue, &[], vec![paired(ARN_B, GROK_GLOBAL)]);
        assert_eq!(entries[0].context, Some(500_000));
        assert_eq!(entries[0].output, Some(500_000));
    }

    #[test]
    fn the_embedded_catalogue_carries_the_reference_numbers() {
        // The same figures, read from the seed the product actually ships, so the fixture above is
        // not quietly measuring a catalogue that has drifted.
        let catalogue = crate::catalogue::Catalogue::new(crate::catalogue::seed::load_seed());
        let entries = write(&catalogue, &[], vec![paired(ARN_A, GROK)]);
        assert_eq!(entries[0].context, Some(500_000));
        assert_eq!(entries[0].output, Some(500_000));
        let global = write(&catalogue, &[], vec![paired(ARN_B, GROK_GLOBAL)]);
        assert_eq!(global[0].context, Some(500_000));
        assert_eq!(global[0].output, Some(500_000));
    }

    #[test]
    fn a_hand_typed_limit_survives_a_repair_a_resave_and_a_catalogue_refresh() {
        let catalogue = limits_catalogue();
        // A person typed the context window and left the output empty: only the output is the
        // fill's to write.
        let base = write(
            &catalogue,
            &[],
            vec![ProfileEntry {
                context: Some(20_000),
                ..paired(ARN_A, GROK)
            }],
        );
        assert_eq!(base[0].context, Some(20_000), "the typed number is the person's");
        assert_eq!(base[0].output, Some(500_000), "the empty one is filled");
        assert_eq!(
            base[0].limits_edited,
            Some(LimitsEdited { context: true, output: false })
        );

        // A re-pairing: the typed 20000 stays, the sibling takes the new model's number.
        let repaired = write(
            &catalogue,
            &base,
            vec![ProfileEntry {
                model: Some(NOVA.into()),
                ..base[0].clone()
            }],
        );
        assert_eq!(repaired[0].context, Some(20_000), "0 hand-entered limits overwritten");
        assert_eq!(repaired[0].output, Some(5_000), "the untouched sibling is refreshed");

        // A re-save: the form echoes back exactly what it displayed, and nothing moves.
        let resaved = write(&catalogue, &repaired, repaired.clone());
        assert_eq!(resaved[0].context, Some(20_000));
        assert_eq!(resaved[0].output, Some(5_000));
        assert_eq!(
            resaved[0].limits_edited,
            Some(LimitsEdited { context: true, output: false })
        );

        // A catalogue refresh, with the pair unchanged: the fill is an act, not a live lookup, so
        // a second write against changed numbers moves neither field (rule 3).
        let refreshed = catalogue_of(REFRESHED);
        let after = write(&refreshed, &resaved, resaved.clone());
        assert_eq!(after[0].context, Some(20_000));
        assert_eq!(after[0].output, Some(5_000), "a resolved window does not stop being true");
    }

    #[test]
    fn a_repairing_refreshes_the_unedited_limit_and_keeps_no_previous_model_numbers() {
        // Rule 2: the previous model's 500000 does not survive the pair change.
        let catalogue = limits_catalogue();
        let base = write(&catalogue, &[], vec![paired(ARN_A, GROK)]);
        assert_eq!(base[0].context, Some(500_000));

        let repaired = write(
            &catalogue,
            &base,
            vec![ProfileEntry {
                model: Some(NOVA.into()),
                ..base[0].clone()
            }],
        );
        assert_eq!(repaired[0].context, Some(300_000));
        assert_eq!(repaired[0].output, Some(5_000));
        assert_eq!(
            repaired[0].limits_edited,
            Some(LimitsEdited { context: false, output: false })
        );
    }

    #[test]
    fn no_pair_and_an_unresolvable_pair_leave_the_limits_unknown() {
        let catalogue = limits_catalogue();
        // An id the catalogue cannot resolve: unknown — never "unsupported", and never a number
        // taken from the ARN, which is full of digits (rule 4).
        let unresolved = write(
            &catalogue,
            &[],
            vec![paired(ARN_A, "xai.grok-4.7-not-in-this-catalogue")],
        );
        assert_eq!(unresolved[0].context, None);
        assert_eq!(unresolved[0].output, None);
        assert_eq!(
            unresolved[0].limits_edited,
            Some(LimitsEdited { context: false, output: false })
        );

        // No pair at all: there is nothing to fill from, and the numbers stay empty.
        let unpaired = write(&catalogue, &[], vec![profile_entry(ARN_A)]);
        assert_eq!(unpaired[0].context, None);
        assert_eq!(unpaired[0].output, None);
        assert_eq!(unpaired[0].model, None);

        // Re-pairing to an id that does not resolve empties what the previous pair supplied rather
        // than leaving the old model's numbers behind.
        let base = write(&catalogue, &[], vec![paired(ARN_A, GROK)]);
        let gone = write(
            &catalogue,
            &base,
            vec![ProfileEntry {
                model: Some("amazon.retired-v1:0".into()),
                ..base[0].clone()
            }],
        );
        assert_eq!(gone[0].context, None);
        assert_eq!(gone[0].output, None);
    }

    #[test]
    fn a_cleared_limit_stays_cleared_and_is_not_refilled() {
        let catalogue = limits_catalogue();
        let base = write(&catalogue, &[], vec![paired(ARN_A, GROK)]);
        assert_eq!(base[0].context, Some(500_000));

        // The person empties the context window and leaves the pair alone: clearing is how they say
        // they do not want the catalogue's number (rule 5).
        let cleared = write(
            &catalogue,
            &base,
            vec![ProfileEntry {
                context: None,
                ..base[0].clone()
            }],
        );
        assert_eq!(cleared[0].context, None);
        assert_eq!(cleared[0].output, Some(500_000), "the sibling is untouched");
        assert_eq!(
            cleared[0].limits_edited,
            Some(LimitsEdited { context: true, output: false })
        );

        // And the next save does not quietly put it back.
        let resaved = write(&catalogue, &cleared, cleared.clone());
        assert_eq!(resaved[0].context, None);
        assert_eq!(resaved[0].output, Some(500_000));
        assert_eq!(
            resaved[0].limits_edited,
            Some(LimitsEdited { context: true, output: false })
        );
    }

    #[test]
    fn the_fill_never_touches_the_capabilities() {
        // The fixture rows all say `tool_call: true`, so a fill that copied a capability would
        // show here; SC-022 counts 0 filled (FR-034a.6, FR-028).
        let catalogue = limits_catalogue();
        let entries = write(&catalogue, &[], vec![paired(ARN_A, GROK)]);
        assert_eq!(entries[0].tool_call, None);
        assert_eq!(entries[0].image_input, None);

        let repaired = write(
            &catalogue,
            &entries,
            vec![ProfileEntry {
                model: Some(NOVA.into()),
                ..entries[0].clone()
            }],
        );
        assert_eq!(repaired[0].tool_call, None, "a pair change fills 0 capabilities");
        assert_eq!(repaired[0].image_input, None);
    }

    #[test]
    fn echoing_the_displayed_numbers_back_does_not_make_them_hand_entered() {
        let catalogue = limits_catalogue();
        let filled = write(&catalogue, &[], vec![paired(ARN_A, GROK)]);
        assert_eq!(
            filled[0].limits_edited,
            Some(LimitsEdited { context: false, output: false })
        );

        // The form sends back exactly what it displayed, with the flags a client might guess at:
        // the server derives them and discards the request's own (rule 1).
        let echoed = write(
            &catalogue,
            &filled,
            vec![ProfileEntry {
                limits_edited: Some(LimitsEdited {
                    context: true,
                    output: true,
                }),
                ..filled[0].clone()
            }],
        );
        assert_eq!(
            echoed[0].limits_edited,
            Some(LimitsEdited { context: false, output: false }),
            "a filled number must not become hand-entered by being saved"
        );

        // So a later re-pairing still refreshes both numbers.
        let repaired = write(
            &catalogue,
            &echoed,
            vec![ProfileEntry {
                model: Some(NOVA.into()),
                ..echoed[0].clone()
            }],
        );
        assert_eq!(repaired[0].context, Some(300_000));
        assert_eq!(repaired[0].output, Some(5_000));
    }

    #[test]
    fn a_mismatched_pair_is_accepted_and_the_arn_checks_are_unchanged() {
        let catalogue = limits_catalogue();
        // A `global.` id on a us-east-1 ARN, on purpose: accepted, filled from the row, and the ARN
        // is not rewritten (FR-034a.6).
        let entries = write(&catalogue, &[], vec![paired(ARN_A, GROK_GLOBAL)]);
        assert_eq!(entries[0].arn, ARN_A);
        assert_eq!(entries[0].context, Some(500_000));

        // The region check still reads the ARN alone, and the pair takes no part in it (FR-024).
        let mut entries = vec![paired(ARN_A, NOVA)];
        let error = validate_entries(&mut entries, Some("eu-west-1")).unwrap_err();
        assert_eq!(
            error.message(),
            "the ARN names region \"us-east-1\" but this connection uses \"eu-west-1\""
        );
        // And a well-formed ARN with an unresolvable pair is still a well-formed ARN.
        let mut entries = vec![paired("not-an-arn", GROK)];
        assert!(validate_entries(&mut entries, Some("us-east-1")).is_err());
    }
}

