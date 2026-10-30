//! Provider credentials (contract `http-api.md` § Providers): any selectable provider can be
//! connected with a saved key, and keys found in the environment are shown alongside.

use serde::{Deserialize, Serialize};

use crate::{
    auth::Principal,
    catalogue::{ProviderOrigin, builtin::CredentialField, data::ProviderKind},
    config::provider::ProviderId,
    dependencies::VizierDependencies,
    schema::{ProviderCredentials, ProviderEntry},
    service::{
        ServiceError,
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
}

fn set(v: &Option<String>) -> bool {
    v.as_deref().is_some_and(|s| !s.is_empty())
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
        // A local provider on its default address has no source, but is listed once used.
        let listed = p.id == ProviderId::DUMMYPLUG || p.connection.source.is_some() || (local && used_by.total() > 0);
        if !listed {
            continue;
        }
        let env = crate::llm::resolve::provider_env(&deps.catalogue, &p.id);
        let creds = crate::provider_keys::resolve_credentials(&deps.storage, &p.id, &env)
            .await
            .map(|(c, _)| c)
            .unwrap_or_default();
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
    /// AWS region.
    pub region: Option<String>,
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
    let entry = ProviderEntry {
        id: id.clone(),
        credentials: ProviderCredentials {
            api_key: apply(stored.api_key, body.api_key),
            endpoint: apply(stored.endpoint, body.endpoint),
            base_url: apply(stored.base_url, body.base_url),
            access_token: apply(stored.access_token, body.access_token),
            account_id: apply(stored.account_id, body.account_id),
            access_key_id: apply(stored.access_key_id, body.access_key_id),
            secret_access_key: apply(stored.secret_access_key, body.secret_access_key),
            session_token: apply(stored.session_token, body.session_token),
            region: apply(stored.region, body.region),
        },
    };
    check_bedrock_pair(&id, &entry.credentials).map_err(fail)?;
    let saved = if entry.credentials.is_empty() {
        deps.storage.delete_provider(&id).await
    } else {
        deps.storage.upsert_provider(&entry).await
    };
    if let Err(e) = saved {
        return Err(ServiceError::Internal(e.to_string()));
    }
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
    let credentials = Some(body.into_credentials()).filter(|c| !c.is_empty());
    Ok(match usage::resolve_for_test(deps, &id, credentials, custom.as_ref()).await {
        Ok(resolved) => usage::check(deps, resolved).await,
        Err(e) => TestResult {
            ok: false,
            model_count: None,
            status: None,
            error: Some(e.to_string()),
        },
    })
}
