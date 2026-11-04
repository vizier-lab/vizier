//! Provider credentials (contract `http-api.md` § Providers): any selectable provider can be
//! connected with a saved key, and keys found in the environment are shown alongside.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    auth::Principal,
    catalogue::{
        ProviderOrigin,
        builtin::CredentialField,
        data::{Cost, ProviderKind},
    },
    config::provider::ProviderId,
    dependencies::VizierDependencies,
    llm::sso::{SsoSessions, expires_at, session_state},
    schema::{LimitsEdited, ProfileEntry, ProviderCredentials, ProviderEntry, SsoSessionState},
    service::{
        ServiceError, providers_sso,
        catalogue::{EntryPricingView, entry_display_name, entry_pricing},
        usage::{self, AffectedAgent, TestResult, UsedBy},
    },
    storage::provider::{CustomProviderStorage, ProviderStorage},
};

/// A connected provider: saved credentials or a key in the environment, never the secrets.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ProviderResponse {
    pub id: ProviderId,
    /// The same id, under the field name pre-015 clients read.
    pub variant: ProviderId,
    pub name: String,
    pub origin: ProviderOrigin,
    pub kind: ProviderKind,
    pub logo: Option<String>,
    pub model_count: usize,
    pub fields: Vec<CredentialField>,
    /// `saved`, `env:NAME`, or `null` for a local provider on its default address.
    pub source: Option<String>,
    pub has_api_key: bool,
    pub endpoint: Option<String>,
    pub base_url: Option<String>,
    pub has_access_token: bool,
    pub account_id: Option<String>,
    /// AWS Bedrock: the access key id is an identifier, not a secret.
    pub access_key_id: Option<String>,
    /// Never the value itself.
    pub has_secret_access_key: bool,
    pub has_session_token: bool,
    /// AWS region, safe to show.
    pub region: Option<String>,
    /// The pre-015 name this provider's saved row was migrated from.
    pub migrated_from: Option<String>,
    pub used_by: UsedBy,
    /// Local and custom providers only; checked in the background.
    pub reachable: Option<bool>,
    // ---- Amazon Bedrock over IAM Identity Center (spec 044, `contracts/provider-credentials-api.md` §4).
    // Non-secret only: the session itself never leaves its encrypted row (FR-009b).
    /// `configured`, `signed-in`, `expired` or `unreadable`; `null` for every other provider.
    pub sso_state: Option<SsoSessionState>,
    /// A session row exists and decrypts with this workspace key.
    pub has_sso_session: bool,
    /// The non-secret handle the session is stored under (FR-009e).
    pub sso_profile: Option<String>,
    /// The Identity Center start URL, shown in clear (FR-009b).
    pub start_url: Option<String>,
    /// The Identity Center's region — not the Bedrock region (FR-010).
    pub sso_region: Option<String>,
    /// The role chosen from the portal.
    pub role_name: Option<String>,
    /// When the stored session's access token expires.
    pub session_expires_at: Option<DateTime<Utc>>,
    /// What the connection last failed with, when that failure was about the session itself —
    /// a session revoked in AWS while its token still looked valid, for instance (FR-016,
    /// FR-018). Cleared by the next successful sign-in, which needs no Vizier restart.
    pub session_failure: Option<String>,
    /// The saved inference-profile entries, as saved: no price is invented here (FR-026, FR-028).
    pub profiles: Vec<ProfileEntryResponse>,
}

/// One saved inference-profile entry as the settings row shows it (contracts §4): the ARN the
/// person pasted, the limits they set, the optional pricing pairing, and a name to show for it.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ProfileEntryResponse {
    /// The ARN exactly as saved (FR-027).
    pub arn: String,
    pub name: Option<String>,
    /// Unknown when unset — never "unsupported", never copied from the paired model (FR-028,
    /// FR-034a.6).
    pub context: Option<u64>,
    pub output: Option<u64>,
    pub tool_call: Option<bool>,
    pub image_input: Option<bool>,
    /// The catalogue model id the entry is priced from, when the person paired one (FR-034a.1).
    pub model: Option<String>,
    /// The hand-entered cost, when there is one (FR-034a.4).
    pub price: Option<Cost>,
    /// Which of the two limits the person set by hand, so a re-pairing refreshes only the others
    /// (FR-034a.7). Server-owned: the client never sets it, and `null` on an entry saved before
    /// this rule reads as "true wherever a number is present".
    pub limits_edited: Option<LimitsEdited>,
    pub edited: bool,
    /// The entry's name, or the profile id from its ARN when it has none (FR-026).
    pub display_name: String,
    /// What its usage will cost right now, computed from the entry's current state — the figure,
    /// or the absence with a cause-specific reason and the way out (contracts §4a, FR-034a.2,
    /// FR-034a.3, FR-034b, SC-021). Never stored, so re-picking a model or entering a price
    /// clears it with no manual edit.
    pub pricing: EntryPricingView,
}

impl ProfileEntryResponse {
    /// The entry as the settings row shows it; shared with the entry CRUD so every surface
    /// renders one shape (spec 044, contracts §4). `pricing` is re-derived from the catalogue as
    /// it stands now on every call, so a refresh relabels and reprices the row and a fixed cause
    /// clears its reason.
    pub(crate) fn from_entry(catalogue: &crate::catalogue::Catalogue, entry: &ProfileEntry) -> Self {
        let display_name = entry_display_name(entry);
        Self {
            arn: entry.arn.clone(),
            name: entry.name.clone(),
            context: entry.context,
            output: entry.output,
            tool_call: entry.tool_call,
            image_input: entry.image_input,
            model: entry.model.clone(),
            price: entry.price.clone(),
            limits_edited: entry.limits_edited,
            edited: entry.edited,
            display_name,
            pricing: EntryPricingView::of(&entry_pricing(catalogue, entry)),
        }
    }
}

/// What the settings row reports about a connection's session, read from the encrypted row on
/// every request: never a cached copy, never a token (contracts §4, FR-009b).
struct SsoView {
    state: SsoSessionState,
    has_session: bool,
    expires_at: Option<DateTime<Utc>>,
}

/// The session half of an SSO connection, or the reason it cannot be shown (FR-009f).
async fn sso_view(deps: &VizierDependencies, credentials: &ProviderCredentials) -> SsoView {
    let configured = SsoView {
        state: SsoSessionState::Configured,
        has_session: false,
        expires_at: None,
    };
    let Some(profile) = credentials.sso_profile.as_deref().filter(|p| !p.is_empty()) else {
        return configured;
    };
    match SsoSessions::new(&*deps.storage).load(profile).await {
        Ok(session) => SsoView {
            state: session_state(session.as_ref(), Utc::now()),
            has_session: session.is_some(),
            expires_at: expires_at(session.as_ref()),
        },
        // A row this key cannot open is reported as such, never as "signed out": the fix is a
        // re-sign-in, and nothing was deleted (FR-009f, SC-019).
        Err(_) => SsoView {
            state: SsoSessionState::Unreadable,
            ..configured
        },
    }
}

fn set(v: &Option<String>) -> bool {
    v.as_deref().is_some_and(|s| !s.is_empty())
}

/// The session failure a provider row reports: only the SSO connection can have one, and only
/// while it is the failure the surfaces still hold (FR-003, FR-018).
fn reported_failure(
    id: &ProviderId,
    failures: &crate::llm::sso::SessionFailures,
) -> Option<String> {
    if *id != ProviderId::AMAZON_BEDROCK_SSO {
        return None;
    }
    failures.get(id)
}

pub async fn connected(deps: &VizierDependencies) -> Vec<ProviderResponse> {
    let summaries = crate::service::catalogue::provider_summaries(deps).await;
    let mut used = usage::used_by_all(deps).await;
    let migrated = usage::migrated_from(deps).await;
    let mut out = vec![];
    for p in summaries {
        if p.origin == ProviderOrigin::Custom {
            continue;
        }
        let used_by = used.remove(&p.id).map(|(u, _)| u).unwrap_or_default();
        let local = p.kind == ProviderKind::Local;
        // Built-in local providers need a saved URL, an environment URL, or an
        // agent using the default address to appear in the connected list.
        let listed = p.connection.source.is_some() || (local && used_by.total() > 0);
        if !listed {
            continue;
        }
        let env = crate::llm::resolve::provider_env(&deps.catalogue, &p.id);
        let creds = crate::provider_keys::resolve_credentials(&deps.storage, &p.id, &env)
            .await
            .map(|(c, _)| c)
            .unwrap_or_default();
        let sso = if p.id == ProviderId::AMAZON_BEDROCK_SSO {
            Some(sso_view(deps, &creds).await)
        } else {
            None
        };
        let profiles = if p.id == ProviderId::AMAZON_BEDROCK_SSO {
            creds
                .profiles
                .iter()
                .map(|entry| ProfileEntryResponse::from_entry(&deps.catalogue, entry))
                .collect()
        } else {
            Vec::new()
        };
        let (sso_profile, start_url, sso_region, role_name) = if p.id == ProviderId::AMAZON_BEDROCK_SSO {
            (
                creds.sso_profile.clone(),
                creds.start_url.clone(),
                creds.sso_region.clone(),
                creds.role_name.clone(),
            )
        } else {
            (None, None, None, None)
        };
        let session_failure = reported_failure(&p.id, &deps.sso_failures);
        out.push(ProviderResponse {
            variant: p.id.clone(),
            migrated_from: migrated.get(p.id.as_str()).cloned(),
            reachable: local.then(|| usage::reachable(deps, &p.id)).flatten(),
            name: p.name,
            origin: p.origin,
            kind: p.kind,
            logo: p.logo,
            model_count: p.model_count,
            fields: p.fields,
            source: p.connection.source,
            has_api_key: set(&creds.api_key),
            endpoint: creds.endpoint,
            base_url: creds.base_url.or(p.endpoint),
            has_access_token: set(&creds.access_token),
            account_id: creds.account_id,
            access_key_id: creds.access_key_id.filter(|v| !v.is_empty()),
            has_secret_access_key: set(&creds.secret_access_key),
            has_session_token: set(&creds.session_token),
            region: creds.region.filter(|v| !v.is_empty()),
            used_by,
            id: p.id,
            sso_state: sso.as_ref().map(|view| view.state),
            has_sso_session: sso.as_ref().is_some_and(|view| view.has_session),
            session_expires_at: sso.as_ref().and_then(|view| view.expires_at),
            session_failure,
            sso_profile,
            start_url,
            sso_region,
            role_name,
            profiles,
        });
    }
    out
}

/// Credentials to save. An omitted field keeps the stored value; an empty string clears it.
#[derive(Debug, Default, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct UpsertProviderRequest {
    pub api_key: Option<String>,
    pub endpoint: Option<String>,
    pub base_url: Option<String>,
    pub access_token: Option<String>,
    pub account_id: Option<String>,
    /// AWS Bedrock access key id.
    pub access_key_id: Option<String>,
    /// AWS Bedrock secret access key (write-only after saving).
    pub secret_access_key: Option<String>,
    /// AWS Bedrock temporary-credential session token (write-only after saving).
    pub session_token: Option<String>,
    /// AWS region; for the SSO provider this is the **Bedrock** region (FR-010).
    pub region: Option<String>,
    /// Amazon Bedrock (SSO): the Identity Center start URL (FR-005).
    pub start_url: Option<String>,
    /// Amazon Bedrock (SSO): the region the Identity Center instance lives in; defaults from
    /// `region` when only that one is given (FR-010).
    pub sso_region: Option<String>,
    /// Amazon Bedrock (SSO): the role chosen from the portal.
    pub role_name: Option<String>,
    /// Amazon Bedrock (SSO): the non-secret handle the session is stored under (FR-009e).
    pub sso_profile: Option<String>,
    /// Amazon Bedrock (SSO): the whole inference-profile list, one operation — omitted keeps the
    /// stored list, `[]` clears it (FR-021).
    pub profiles: Option<Vec<ProfileEntry>>,
    /// The target of a **Test** on the SSO provider: the model id or inference-profile ARN to
    /// invoke (contract `media-and-test.md` §1). Upsert ignores it — it is not a credential, so it
    /// is deliberately absent from [`Self::sent`] and [`Self::into_credentials`], and saving
    /// neither stores nor clears anything for it.
    pub model: Option<String>,
}

impl UpsertProviderRequest {
    /// The fields present in the body.
    pub fn sent(&self) -> Vec<CredentialField> {
        use CredentialField::*;
        [
            (ApiKey, &self.api_key),
            (Endpoint, &self.endpoint),
            (BaseUrl, &self.base_url),
            (AccessToken, &self.access_token),
            (AccountId, &self.account_id),
            (AccessKeyId, &self.access_key_id),
            (SecretAccessKey, &self.secret_access_key),
            (SessionToken, &self.session_token),
            (Region, &self.region),
            (StartUrl, &self.start_url),
            (SsoRegion, &self.sso_region),
            (RoleName, &self.role_name),
            (SsoProfile, &self.sso_profile),
        ]
        .into_iter()
        .filter(|(_, v)| v.is_some())
        .map(|(f, _)| f)
        .collect()
    }

    pub fn into_credentials(self) -> ProviderCredentials {
        ProviderCredentials {
            api_key: self.api_key,
            endpoint: self.endpoint,
            base_url: self.base_url,
            access_token: self.access_token,
            account_id: self.account_id,
            access_key_id: self.access_key_id,
            secret_access_key: self.secret_access_key,
            session_token: self.session_token,
            region: self.region,
            start_url: self.start_url,
            sso_region: self.sso_region,
            role_name: self.role_name,
            sso_profile: self.sso_profile,
            profiles: self.profiles.unwrap_or_default(),
        }
    }
}

/// Apply `update` over `stored`: `None` keeps, `""` clears, a value replaces.
pub fn apply(stored: Option<String>, update: Option<String>) -> Option<String> {
    match update {
        None => stored,
        Some(v) if v.is_empty() => None,
        Some(v) => Some(v),
    }
}

fn field_name(f: CredentialField) -> &'static str {
    match f {
        CredentialField::ApiKey => "api_key",
        CredentialField::Endpoint => "endpoint",
        CredentialField::BaseUrl => "base_url",
        CredentialField::AccessToken => "access_token",
        CredentialField::AccountId => "account_id",
        CredentialField::AccessKeyId => "access_key_id",
        CredentialField::SecretAccessKey => "secret_access_key",
        CredentialField::SessionToken => "session_token",
        CredentialField::Region => "region",
        CredentialField::StartUrl => "start_url",
        CredentialField::SsoRegion => "sso_region",
        CredentialField::RoleName => "role_name",
        CredentialField::SsoProfile => "sso_profile",
    }
}

/// A Bedrock save must not leave exactly one half of the signing pair behind (FR-005). Clearing
/// everything disconnects, and setting the region (or a session token) before the pair is fine.
fn check_bedrock_pair(
    id: &ProviderId,
    creds: &ProviderCredentials,
) -> Result<(), (u16, String)> {
    if *id != ProviderId::AMAZON_BEDROCK {
        return Ok(());
    }
    if let Some(region) = creds.region.as_deref().filter(|r| !r.is_empty())
        && !crate::llm::bedrock::valid_region(region)
    {
        return Err((
            400u16,
            format!("invalid AWS region {region:?}: use a form like us-east-1"),
        ));
    }
    let has = |v: &Option<String>| v.as_deref().is_some_and(|s| !s.is_empty());
    match (has(&creds.access_key_id), has(&creds.secret_access_key)) {
        (true, false) => Err((
            400u16,
            "the secret access key is missing: set it too, or clear the access key ID".into(),
        )),
        (false, true) => Err((
            400u16,
            "the access key ID is missing: set it too, or clear the secret access key".into(),
        )),
        _ => Ok(()),
    }
}

/// The five non-secret fields an SSO connection is made of (data-model "SSO connection").
const SSO_FIELDS: [CredentialField; 5] = [
    CredentialField::StartUrl,
    CredentialField::SsoRegion,
    CredentialField::AccountId,
    CredentialField::RoleName,
    CredentialField::SsoProfile,
];

/// The value of one SSO field, `None` when it is absent or empty.
fn sso_value<'a>(creds: &'a ProviderCredentials, field: CredentialField) -> Option<&'a str> {
    let value = match field {
        CredentialField::StartUrl => &creds.start_url,
        CredentialField::SsoRegion => &creds.sso_region,
        CredentialField::AccountId => &creds.account_id,
        CredentialField::RoleName => &creds.role_name,
        CredentialField::SsoProfile => &creds.sso_profile,
        _ => return None,
    };
    value.as_deref().filter(|v| !v.is_empty())
}

/// The rules an SSO connection is saved under (data-model "SSO connection", FR-009e, FR-010,
/// FR-011), applied to the **merged** credentials so a partial update cannot slip past them:
///
/// - `base_url` and `api_key` are refused: the endpoint follows the regions, and the sign-in
///   replaces the key form entirely;
/// - the two regions default to each other when only one is supplied, and are never equalised
///   afterwards — an Identity Center instance and the Bedrock runtime may sit in different
///   regions — with each message naming which of the two it concerns;
/// - the five fields are all present or all absent, and a refusal names the missing one;
/// - `sso_profile` is unique in the workspace, and a clash names the row that owns it.
///
/// `others` is every stored provider row, for the uniqueness rule. `credentials` is edited in
/// place for the defaulting only.
pub fn check_sso_connection(
    id: &ProviderId,
    credentials: &mut ProviderCredentials,
    others: &[ProviderEntry],
) -> Result<(), (u16, String)> {
    if *id != ProviderId::AMAZON_BEDROCK_SSO {
        if !credentials.profiles.is_empty() {
            return Err((
                400u16,
                format!(
                    "\"{id}\" does not use inference profile entries; they belong to \"{}\"",
                    ProviderId::AMAZON_BEDROCK_SSO
                ),
            ));
        }
        return Ok(());
    }
    if set(&credentials.base_url) {
        return Err((
            400u16,
            format!("\"{id}\" does not take a base_url; its endpoint follows the region"),
        ));
    }
    if set(&credentials.api_key) {
        return Err((
            400u16,
            format!("\"{id}\" does not use api_key; sign in through IAM Identity Center instead"),
        ));
    }

    // FR-010: fill whichever region is missing from the other, never overwrite a present one.
    if !set(&credentials.sso_region) && set(&credentials.region) {
        credentials.sso_region = credentials.region.clone();
    } else if !set(&credentials.region) && set(&credentials.sso_region) {
        credentials.region = credentials.sso_region.clone();
    }
    for (label, region) in [
        ("AWS region", credentials.region.as_deref()),
        ("SSO region", credentials.sso_region.as_deref()),
    ] {
        if let Some(region) = region.filter(|r| !r.is_empty())
            && !crate::llm::bedrock::valid_region(region)
        {
            return Err((
                400u16,
                format!("invalid {label} {region:?}: use a form like us-east-1"),
            ));
        }
    }

    // All five or none: half a connection cannot be signed in with, so it is not saved either.
    let present: Vec<CredentialField> = SSO_FIELDS
        .iter()
        .copied()
        .filter(|field| sso_value(credentials, *field).is_some())
        .collect();
    if !present.is_empty() && present.len() < SSO_FIELDS.len() {
        let missing: Vec<&str> = SSO_FIELDS
            .iter()
            .copied()
            .filter(|field| !present.contains(field))
            .map(field_name)
            .collect();
        return Err((
            400u16,
            format!(
                "the SSO connection is incomplete: {} {} missing — save start_url, sso_region, \
                 account_id, role_name and sso_profile together",
                missing.join(", "),
                if missing.len() == 1 { "is" } else { "are" }
            ),
        ));
    }

    // FR-009e: a profile name addresses exactly one stored session, so it is unique per workspace.
    if let Some(profile) = sso_value(credentials, CredentialField::SsoProfile)
        && let Some(owner) = others
            .iter()
            .find(|entry| entry.id != *id && entry.credentials.sso_profile.as_deref() == Some(profile))
    {
        return Err((
            400u16,
            format!(
                "the profile name {profile:?} is already used by \"{}\"; choose another",
                owner.id
            ),
        ));
    }
    Ok(())
}

/// The provider's connect fields, or why credentials can't be saved for it.
pub fn check_fields(
    deps: &VizierDependencies,
    id: &ProviderId,
    sent: &[CredentialField],
    custom: &[crate::schema::CustomProvider],
) -> Result<(), (u16, String)> {
    let Some(listing) = deps.catalogue.providers(custom).into_iter().find(|p| p.id == *id) else {
        return Err((
            404u16,
            format!("provider \"{id}\" is not available"),
        ));
    };
    if listing.origin == ProviderOrigin::Custom {
        return Err((
            400u16,
            format!("\"{id}\" is a custom provider; edit it under /providers/custom/{id}"),
        ));
    }
    for f in sent {
        if listing.fields.contains(f) {
            continue;
        }
        let message = if *f == CredentialField::BaseUrl {
            // FR-027: a cloud provider's address comes from the catalogue.
            format!(
                "\"{id}\" does not take a base_url; to use another address, add a custom provider"
            )
        } else {
            format!("\"{id}\" does not use {}", field_name(*f))
        };
        return Err((400u16, message));
    }
    Ok(())
}


const PERMISSION: &str = "settings:providers";

/// A `(status, message)` failure as the matching service error.
fn fail((status, message): (u16, String)) -> ServiceError {
    match status {
        404 => ServiceError::NotFound(message),
        500 => ServiceError::Internal(message),
        status => ServiceError::Invalid { status, message, code: None },
    }
}

fn parse_id(id: &str) -> Result<ProviderId, ServiceError> {
    ProviderId::parse(id).map_err(|_| ServiceError::invalid("invalid provider id"))
}

pub async fn list(
    deps: &VizierDependencies,
    user: &Principal,
) -> Result<Vec<ProviderResponse>, ServiceError> {
    crate::access::require(user, PERMISSION)?;
    crate::catalogue::refresh::maybe_refresh(deps);
    Ok(connected(deps).await)
}

pub async fn get(
    deps: &VizierDependencies,
    user: &Principal,
    id: &str,
) -> Result<ProviderResponse, ServiceError> {
    crate::access::require(user, PERMISSION)?;
    let id = parse_id(id)?;
    connected(deps)
        .await
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| ServiceError::NotFound("provider not found".into()))
}

/// Saves the credentials. `None` means nothing remains connected after the save.
pub async fn upsert(
    deps: &VizierDependencies,
    user: &Principal,
    id: &str,
    body: UpsertProviderRequest,
) -> Result<Option<ProviderResponse>, ServiceError> {
    crate::access::require(user, PERMISSION)?;
    let id = parse_id(id)?;
    let custom = deps.storage.list_custom_providers().await.unwrap_or_default();
    check_fields(deps, &id, &body.sent(), &custom).map_err(fail)?;

    let stored = match deps.storage.get_provider(&id).await {
        Ok(entry) => entry.map(|e| e.credentials).unwrap_or_default(),
        Err(e) => return Err(ServiceError::Internal(e.to_string())),
    };
    let update = ProviderCredentials {
        api_key: body.api_key,
        endpoint: body.endpoint,
        base_url: body.base_url,
        access_token: body.access_token,
        account_id: body.account_id,
        access_key_id: body.access_key_id,
        secret_access_key: body.secret_access_key,
        session_token: body.session_token,
        region: body.region,
        start_url: body.start_url,
        sso_region: body.sso_region,
        role_name: body.role_name,
        sso_profile: body.sso_profile,
        // Omitted keeps the stored list; `[]` is an emptied list and clears it (FR-021).
        profiles: body.profiles.clone().unwrap_or_default(),
    };
    let mut credentials = stored.merged_with_profiles(&update, body.profiles);
    // T066 (FR-019, FR-040): the three fields that decide *which* AWS identity an agent talks
    // as. Only they make the stored sign-in stale; any other edit keeps it in use.
    let reconnect = stored.region != credentials.region
        || stored.account_id != credentials.account_id
        || stored.role_name != credentials.role_name;
    check_bedrock_pair(&id, &credentials).map_err(fail)?;
    let rows = deps
        .storage
        .list_providers()
        .await
        .map_err(|e| ServiceError::Internal(e.to_string()))?;
    check_sso_connection(&id, &mut credentials, &rows).map_err(fail)?;
    // The entry rules live in one place, `service::providers_sso::validate_entries`, and this
    // whole-list save is the path the provider form shares with the entry editor: shape, then
    // duplicate, then region (data-model "profile entry", FR-021, FR-024, FR-025).
    providers_sso::validate_entries(&mut credentials.profiles, credentials.region.as_deref())?;
    // Then the limit fill of FR-034a.7, per entry and matched by ARN, with the **stored** list as
    // the base: a whole-list save can neither bypass the fill nor re-open a filled number as if it
    // were the person's (contracts §4b).
    providers_sso::fill_limits(&deps.catalogue, &stored.profiles, &mut credentials.profiles);
    // The one session row this connection owns, taken before `credentials` moves into the
    // entry (a session is keyed by `sso_profile`, not by the connection id).
    let profile = credentials
        .sso_profile
        .as_deref()
        .filter(|p| !p.is_empty())
        .map(String::from);
    let entry = ProviderEntry {
        id: id.clone(),
        credentials,
    };
    let saved = if entry.credentials.is_empty() {
        deps.storage.delete_provider(&id).await
    } else {
        deps.storage.upsert_provider(&entry).await
    };
    if let Err(e) = saved {
        return Err(ServiceError::Internal(e.to_string()));
    }
    // A changed region, account or role leaves tokens minted for an identity that is no longer
    // the connection's (or for another account's role): drop them so the next use asks for a
    // sign-in again. Best effort — the save itself stands either way.
    if reconnect
        && let Some(profile) = profile
    {
        if let Err(e) = SsoSessions::new(&*deps.storage).delete(&profile).await {
            tracing::warn!(
                profile = %profile,
                error = %e,
                "the sso session of a changed connection cannot be deleted"
            );
        }
    }
    // Unconditional, exactly as before: every credential edit restarts the agents on `id`, and
    // `restart_users` targets the agents that use this provider and no other (FR-040).
    usage::restart_users(deps, &id).await;
    Ok(connected(deps).await.into_iter().find(|p| p.id == id))
}

/// Deletes the saved credentials; the answer says whether an environment variable still
/// connects the provider.
pub async fn delete(
    deps: &VizierDependencies,
    user: &Principal,
    id: &str,
    confirm: bool,
) -> Result<String, ServiceError> {
    crate::access::require(user, PERMISSION)?;
    let id = parse_id(id)?;
    usage::check_in_use(deps, &id, confirm).await?;
    if let Err(e) = deps.storage.delete_provider(&id).await {
        return Err(ServiceError::invalid(e.to_string()));
    }
    usage::restart_users(deps, &id).await;
    let env = crate::llm::resolve::provider_env(&deps.catalogue, &id);
    Ok(match crate::provider_keys::credentials_from_env(&id, &env) {
        Some((_, origin)) => format!(
            "deleted the saved credentials; the {} environment variable still connects this provider and can't be deleted here",
            origin.label().trim_start_matches("env:")
        ),
        None => "deleted".into(),
    })
}

/// "Test connection" for `id`, with optional credentials laid over what is saved. `body` is
/// the raw request body: empty or `null` tests what is saved.
pub async fn test(
    deps: &VizierDependencies,
    user: &Principal,
    id: &str,
    body: &[u8],
) -> Result<TestResult, ServiceError> {
    crate::access::require(user, PERMISSION)?;
    let id = parse_id(id)?;
    let body: UpsertProviderRequest = if body.iter().all(u8::is_ascii_whitespace) {
        UpsertProviderRequest::default()
    } else {
        match serde_json::from_slice::<Option<UpsertProviderRequest>>(body) {
            Ok(b) => b.unwrap_or_default(),
            Err(e) => return Err(ServiceError::invalid(e.to_string())),
        }
    };
    let custom = deps.storage.get_custom_provider(&id).await.ok().flatten();
    if custom.is_none() {
        let all = deps.storage.list_custom_providers().await.unwrap_or_default();
        check_fields(deps, &id, &body.sent(), &all).map_err(fail)?;
    }
    let model = body.model.clone().unwrap_or_default();
    // The SSO test invokes a target, so there is nothing to test without one: refuse before the
    // resolution and before any AWS request (SC-018).
    if id == ProviderId::AMAZON_BEDROCK_SSO && model.trim().is_empty() {
        return Err(ServiceError::invalid(usage::NEEDS_MODEL));
    }
    let credentials = Some(body.into_credentials()).filter(|c| !c.is_empty());
    Ok(
        match usage::resolve_for_test(deps, &id, credentials, custom.as_ref(), &model).await {
            Ok(resolved) => usage::check(deps, resolved).await,
            Err(e) => TestResult {
                ok: false,
                model: None,
                billing: None,
                model_count: None,
                status: None,
                error: Some(e.to_string()),
            },
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARN: &str = "arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/abc123";

    fn catalogue() -> crate::catalogue::Catalogue {
        crate::catalogue::Catalogue::new(crate::catalogue::seed::load_seed())
    }

    /// A fully filled SSO connection, with the two legs in two different regions on purpose.
    fn full() -> ProviderCredentials {
        ProviderCredentials {
            start_url: Some("https://acme.awsapps.com/start".into()),
            sso_region: Some("us-east-1".into()),
            account_id: Some("123456789012".into()),
            role_name: Some("BedrockOperator".into()),
            sso_profile: Some("acme-bedrock".into()),
            region: Some("eu-west-1".into()),
            ..Default::default()
        }
    }

    fn check(creds: &mut ProviderCredentials) -> Result<(), (u16, String)> {
        check_sso_connection(&ProviderId::AMAZON_BEDROCK_SSO, creds, &[])
    }

    fn message(result: Result<(), (u16, String)>) -> String {
        let (status, message) = result.expect_err("the save is refused");
        assert_eq!(status, 400);
        message
    }

    #[test]
    fn the_two_regions_default_to_each_other_and_are_never_equalised() {
        let mut bedrock_only = full();
        bedrock_only.sso_region = None;
        check(&mut bedrock_only).unwrap();
        assert_eq!(
            bedrock_only.sso_region.as_deref(),
            Some("eu-west-1"),
            "the missing SSO region defaults from the Bedrock region"
        );

        let mut sso_only = full();
        sso_only.region = None;
        check(&mut sso_only).unwrap();
        assert_eq!(
            sso_only.region.as_deref(),
            Some("us-east-1"),
            "the missing Bedrock region defaults from the SSO region"
        );

        // Both given and different: neither is rewritten (FR-010).
        let mut both = full();
        check(&mut both).unwrap();
        assert_eq!(both.region.as_deref(), Some("eu-west-1"));
        assert_eq!(both.sso_region.as_deref(), Some("us-east-1"));
    }

    #[test]
    fn a_region_is_validated_and_the_message_names_which_one() {
        let mut bad_sso = full();
        bad_sso.sso_region = Some("eu-west".into());
        assert!(message(check(&mut bad_sso)).contains("invalid SSO region"));

        let mut bad_bedrock = full();
        bad_bedrock.region = Some("us east 1".into());
        assert!(message(check(&mut bad_bedrock)).contains("invalid AWS region"));
    }

    #[test]
    fn a_half_filled_connection_is_refused_and_the_missing_field_is_named() {
        let mut half = full();
        half.role_name = None;
        let error = message(check(&mut half));
        // The list of missing fields carries the one that is missing and nothing else.
        assert!(
            error.contains("incomplete: role_name is missing"),
            "only the missing field is named: {error}"
        );

        // Nothing of the five at all is a connection that was never filled in: not an error.
        let mut untouched = ProviderCredentials::default();
        check(&mut untouched).unwrap();
        assert!(untouched.region.is_none() && untouched.sso_region.is_none());
    }

    #[test]
    fn a_profile_name_in_use_by_another_row_is_refused_by_name() {
        let mut creds = full();
        let other = ProviderEntry {
            id: ProviderId::OPENAI,
            credentials: ProviderCredentials {
                sso_profile: Some("acme-bedrock".into()),
                ..Default::default()
            },
        };
        let error = message(check_sso_connection(
            &ProviderId::AMAZON_BEDROCK_SSO,
            &mut creds,
            std::slice::from_ref(&other),
        ));
        assert!(error.contains("acme-bedrock"), "{error}");
        assert!(error.contains("openai"), "the owner is named: {error}");

        // The row's own saved name is not a clash.
        let own = ProviderEntry {
            id: ProviderId::AMAZON_BEDROCK_SSO,
            credentials: full(),
        };
        check_sso_connection(
            &ProviderId::AMAZON_BEDROCK_SSO,
            &mut creds,
            std::slice::from_ref(&own),
        )
        .unwrap();
    }

    /// T054: the provider row names the session cause while it stands, and never on the other
    /// Bedrock provider; a sign-in clears it (FR-003, FR-018).
    #[test]
    fn the_row_reports_a_session_failure_on_the_sso_connection_only() {
        let failures = crate::llm::sso::SessionFailures::default();
        assert_eq!(reported_failure(&ProviderId::AMAZON_BEDROCK_SSO, &failures), None);

        failures.record(
            &ProviderId::AMAZON_BEDROCK_SSO,
            &crate::llm::sso::SsoError::SessionExpired.to_string(),
        );
        let message = reported_failure(&ProviderId::AMAZON_BEDROCK_SSO, &failures)
            .expect("the ended session is reported");
        assert!(message.contains("sign in again"), "{message}");
        // The static-credentials provider shares the wire, not the session (FR-003).
        assert_eq!(reported_failure(&ProviderId::AMAZON_BEDROCK, &failures), None);

        failures.clear(&ProviderId::AMAZON_BEDROCK_SSO);
        assert_eq!(reported_failure(&ProviderId::AMAZON_BEDROCK_SSO, &failures), None);
    }

    #[test]
    fn an_sso_connection_takes_no_base_url_and_no_api_key() {
        let mut with_base = full();
        with_base.base_url = Some("https://elsewhere.example".into());
        assert!(message(check(&mut with_base)).contains("base_url"));

        let mut with_key = full();
        with_key.api_key = Some("bearer".into());
        assert!(message(check(&mut with_key)).contains("api_key"));
    }

    #[test]
    fn another_provider_takes_no_inference_profile_entries() {
        let mut creds = ProviderCredentials {
            access_key_id: Some("AKIAEXAMPLE".into()),
            secret_access_key: Some("secret".into()),
            profiles: vec![ProfileEntry {
                arn: ARN.into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let error = check_sso_connection(&ProviderId::AMAZON_BEDROCK, &mut creds, &[])
            .expect_err("the static-key provider has no entries");
        assert!(error.1.contains("inference profile entries"), "{:?}", error.1);
    }

    #[test]
    fn an_entry_is_shown_by_its_name_or_by_the_profile_id_from_its_arn() {
        let named = ProfileEntry {
            arn: ARN.into(),
            name: Some("Acme Prod — Claude".into()),
            ..Default::default()
        };
        assert_eq!(ProfileEntryResponse::from_entry(&catalogue(), &named).display_name, "Acme Prod — Claude");

        let unnamed = ProfileEntry {
            arn: ARN.into(),
            name: Some("   ".into()),
            ..Default::default()
        };
        assert_eq!(ProfileEntryResponse::from_entry(&catalogue(), &unnamed).display_name, "abc123");

        // A row edited by hand to hold something that is not an ARN still shows what it holds.
        let broken = ProfileEntry {
            arn: "not-an-arn".into(),
            ..Default::default()
        };
        assert_eq!(ProfileEntryResponse::from_entry(&catalogue(), &broken).display_name, "not-an-arn");
    }

    /// T039: the row publishes each outcome in the contract's own shape, values omitted where the
    /// shape has none, and re-derives it per request — so fixing the cause clears the reason with
    /// nothing else touched (contracts §4a, FR-034a.2, FR-034a.3, FR-034b, SC-021).
    #[test]
    fn the_entry_row_publishes_each_pricing_shape_and_clears_it_when_fixed() {
        let c = catalogue();
        let row = |entry: &ProfileEntry| {
            serde_json::to_value(ProfileEntryResponse::from_entry(&c, entry)).unwrap()
        };
        let entry = || ProfileEntry { arn: ARN.into(), ..Default::default() };

        // Nothing paired: unpriced, labelled, and the reason says why and what fixes it.
        let unpaired = row(&entry());
        assert_eq!(unpaired["pricing"]["state"], "not_set");
        assert_eq!(unpaired["pricing"]["flag"], "pricing not set");
        let unpaired_reason = unpaired["pricing"]["reason"].as_str().expect("a reason").to_string();
        assert!(unpaired_reason.contains("no model is paired"));
        assert!(unpaired_reason.contains("enter a price by hand"));
        assert!(unpaired["pricing"].get("cost").is_none());
        assert!(unpaired["pricing"].get("model").is_none());

        // Pairing a model clears it on the next read: the same saved row, no edit.
        let paired = row(&ProfileEntry {
            model: Some("amazon.nova-lite-v1:0".into()),
            ..entry()
        });
        assert_eq!(paired["pricing"]["state"], "from_catalogue");
        assert_eq!(paired["pricing"]["model"], "amazon.nova-lite-v1:0");
        assert!(paired["pricing"]["cost"]["input"].as_f64().is_some_and(|v| v > 0.0));
        assert!(paired["pricing"].get("flag").is_none());
        assert!(paired["pricing"].get("reason").is_none());

        // An explicit price wins and says where the figure came from.
        let entered = row(&ProfileEntry {
            price: Some(Cost { input: 1.0, output: 2.0, cache_read: None, cache_write: None }),
            ..entry()
        });
        assert_eq!(entered["pricing"]["state"], "explicit");
        assert_eq!(entered["pricing"]["source"], "entered price");
        assert_eq!(entered["pricing"]["cost"]["input"], 1.0);
        assert!(entered["pricing"].get("reason").is_none());

        // A gone id is named inside the reason, and no cost is invented.
        let gone = row(&ProfileEntry {
            model: Some("anthropic.retired-model-v1".into()),
            ..entry()
        });
        assert_eq!(gone["pricing"]["state"], "not_resolved");
        assert_eq!(gone["pricing"]["flag"], "pricing not resolved");
        assert!(
            gone["pricing"]["reason"]
                .as_str()
                .unwrap()
                .contains("anthropic.retired-model-v1")
        );
        assert!(gone["pricing"].get("cost").is_none());

        // No Bedrock list: its own state and its own reason, distinct from "nothing paired".
        let empty =
            crate::catalogue::Catalogue::new(crate::catalogue::data::CatalogueData::empty());
        let no_list = serde_json::to_value(ProfileEntryResponse::from_entry(&empty, &entry()))
            .unwrap();
        assert_eq!(no_list["pricing"]["state"], "no_catalogue");
        assert_eq!(no_list["pricing"]["flag"], "pricing not set");
        let no_list_reason = no_list["pricing"]["reason"].as_str().expect("a reason");
        assert!(!no_list_reason.is_empty());
        assert_ne!(no_list_reason, unpaired_reason, "two causes share wording");
        assert_eq!(no_list["cost"], serde_json::Value::Null, "unpriced, not zero");
    }

    async fn deps() -> (crate::dependencies::VizierDependencies, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let config = crate::config::VizierConfig {
            workspace: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        };
        (
            crate::dependencies::VizierDependencies::new(config)
                .await
                .unwrap(),
            dir,
        )
    }

    fn system() -> crate::auth::Principal {
        crate::auth::Principal {
            user_id: "owner".into(),
            username: "owner".into(),
            role: crate::auth::Role {
                role_id: "r".into(),
                name: "r".into(),
                permissions: vec![],
                is_system: true,
                created_at: chrono::Utc::now(),
            },
            permissions: vec![],
            method: crate::auth::AuthMethod::Jwt,
        }
    }

    /// The SSO connection exactly as the form sends it, with one field overridden.
    fn sso_body(field: (&str, &str)) -> UpsertProviderRequest {
        let mut body = serde_json::json!({
            "start_url": "https://acme.awsapps.com/start",
            "sso_region": "us-east-1",
            "account_id": "123456789012",
            "role_name": "BedrockOperator",
            "sso_profile": "acme-bedrock",
            "region": "eu-west-1",
        });
        body[field.0] = serde_json::Value::from(field.1);
        serde_json::from_value(body).unwrap()
    }

    /// A session row worth keeping: nothing reads the tokens here, only whether the row exists.
    fn live_session() -> crate::schema::SsoSessionSecret {
        crate::schema::SsoSessionSecret {
            access_token: "access-token-value".into(),
            refresh_token: Some("refresh-token-value".into()),
            token_expires_at: chrono::Utc::now() + chrono::TimeDelta::hours(1),
            client_id: "client-id".into(),
            client_secret: "client-secret-value".into(),
            client_secret_expires_at: None,
            issued_at: chrono::Utc::now(),
            revision: 0,
        }
    }

    /// `restart_users` sends one `Restart` per affected agent and waits for the answer, so a
    /// test stands in for the agent runtime: each command's `agent_id` is recorded and answered
    /// with an error (which only logs). The idle timeout keeps a forgotten task from hanging.
    async fn restarts(
        deps: &VizierDependencies,
    ) -> (
        std::sync::Arc<std::sync::Mutex<Vec<String>>>,
        tokio::task::JoinHandle<()>,
    ) {
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let recorded = seen.clone();
        // The bus replaced the old per-transport command channel: subscribe once, then take
        // fleet-wide commands off `agents.cmd`. Subscribing here, before the task is spawned,
        // means the subscription is live when the caller sends its first command: the bus
        // delivers at most once, so a command sent earlier would simply be lost.
        let mut commands = deps
            .transport
            .serve_agent_commands()
            .await
            .expect("the test listens for agent commands");
        let task = tokio::spawn(async move {
            loop {
                let Ok(Some(command)) = tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    commands.recv(),
                )
                .await
                else {
                    break;
                };
                if let crate::schema::AgentCommand::Restart { agent_id, resp } = command {
                    recorded.lock().unwrap().push(agent_id);
                    let _ = resp.send(crate::schema::AgentCommandResult::Error("stub".into()));
                }
            }
        });
        (seen, task)
    }

    /// One SSO agent and one dummyplug agent, both saved, so the right restart can be told apart
    /// from a wrong one.
    async fn agents(deps: &VizierDependencies) {
        use crate::storage::agent::AgentStorage;

        let mut sso = crate::tests_support::agent_config("sso");
        sso.provider = ProviderId::AMAZON_BEDROCK_SSO;
        sso.model = "amazon.nova-lite-v1:0".into();
        deps.storage.create_agent("sso", &sso).await.unwrap();
        deps.storage
            .create_agent("plain", &crate::tests_support::agent_config("plain"))
            .await
            .unwrap();
    }

    /// T066 (FR-019, FR-040): a region, account or role change makes the saved sign-in stale, so
    /// its row goes and only the agents on this connection are restarted.
    #[tokio::test]
    async fn a_region_change_drops_the_session_and_restarts_only_the_users_of_the_connection() {
        use crate::storage::sso_session::SsoSessionStorage;

        let (deps, _dir) = deps().await;
        upsert(
            &deps,
            &system(),
            "amazon-bedrock-sso",
            sso_body(("start_url", "https://acme.awsapps.com/start")),
        )
        .await
        .expect("the connection saves");
        deps.storage
            .put_sso_session("acme-bedrock", &live_session(), None)
            .await
            .unwrap();
        assert!(
            deps.storage
                .get_sso_session("acme-bedrock")
                .await
                .unwrap()
                .is_some()
        );
        agents(&deps).await;

        let (seen, runtime) = restarts(&deps).await;
        upsert(
            &deps,
            &system(),
            "amazon-bedrock-sso",
            sso_body(("region", "us-east-1")),
        )
        .await
        .expect("the changed region saves");
        runtime.abort();

        assert!(
            deps.storage
                .get_sso_session("acme-bedrock")
                .await
                .unwrap()
                .is_none(),
            "a new region makes the old sign-in stale, so it is gone"
        );
        assert_eq!(
            *seen.lock().unwrap(),
            vec!["sso".to_string()],
            "only the agent using this connection restarts"
        );
    }

    /// T066: any other edit — `start_url` here — is not a new identity: the sign-in stays, and
    /// the restart every credential edit already did still happens.
    #[tokio::test]
    async fn another_field_change_keeps_the_session_and_still_restarts_its_users() {
        use crate::storage::sso_session::SsoSessionStorage;

        let (deps, _dir) = deps().await;
        upsert(
            &deps,
            &system(),
            "amazon-bedrock-sso",
            sso_body(("start_url", "https://acme.awsapps.com/start")),
        )
        .await
        .expect("the connection saves");
        deps.storage
            .put_sso_session("acme-bedrock", &live_session(), None)
            .await
            .unwrap();
        agents(&deps).await;

        let (seen, runtime) = restarts(&deps).await;
        upsert(
            &deps,
            &system(),
            "amazon-bedrock-sso",
            sso_body(("start_url", "https://other.awsapps.com/start")),
        )
        .await
        .expect("the changed start_url saves");
        runtime.abort();

        assert!(
            deps.storage
                .get_sso_session("acme-bedrock")
                .await
                .unwrap()
                .is_some(),
            "the same identity, so the sign-in is kept"
        );
        assert_eq!(
            *seen.lock().unwrap(),
            vec!["sso".to_string()],
            "the old restart behaviour stands"
        );
    }

    /// A Test on the SSO connection invokes a target, so there is nothing to test without one:
    /// the endpoint answers `400` in those words, before the resolution and before any AWS
    /// request (contract `media-and-test.md` §1, SC-018).
    #[tokio::test]
    async fn a_test_on_the_sso_connection_needs_a_model_and_says_so() {
        let (deps, _dir) = deps().await;
        let body = serde_json::json!({
            "start_url": "https://acme.awsapps.com/start",
            "sso_region": "us-east-1",
            "account_id": "123456789012",
            "role_name": "BedrockOperator",
            "sso_profile": "acme-bedrock",
            "region": "us-east-1",
        })
        .to_string();

        let error = test(&deps, &system(), "amazon-bedrock-sso", body.as_bytes())
            .await
            .expect_err("a test without a target is refused");

        assert_eq!(error.status(), 400);
        assert_eq!(
            error.to_string(),
            "the test needs a model or inference profile to invoke; choose one first"
        );
    }
}
