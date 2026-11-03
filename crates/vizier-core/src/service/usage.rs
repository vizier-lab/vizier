//! Which agents use a provider (FR-010b), whether a local or custom provider answers
//! (FR-010c), and the one cheap call behind "Test connection".

use std::collections::{BTreeMap, HashMap};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::dependencies::VizierDependencies;

use crate::{
    config::provider::ProviderId,
    llm::{
        CompletionModel, CompletionRequest, DialectKind, LlmError, Message,
        resolve::{ModelOverrides, ResolveContext, ResolvedModel},
    },
    schema::{
        AgentConfig, CustomProvider, ProfileEntry, ProviderCredentials,
        agent::CompletionRetryConfig,
    },
    storage::agent::AgentStorage,
};

/// Agents per feature using one provider's credentials. Dream and read-image count as chat.
#[derive(Debug, Clone, Default, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UsedBy {
    pub chat: usize,
    pub stt: usize,
    pub tts: usize,
    pub image_gen: usize,
    pub embedding: usize,
}

impl UsedBy {
    pub fn total(&self) -> usize {
        self.chat + self.stt + self.tts + self.image_gen + self.embedding
    }
}

/// An agent that would lose a provider, for the delete warning.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AffectedAgent {
    pub id: String,
    pub name: String,
    pub features: Vec<String>,
}

/// The features of `config` that use `provider`.
fn features(config: &AgentConfig, provider: &ProviderId) -> Vec<&'static str> {
    let tools = &config.tools;
    let chat = config.provider == *provider
        || config.dream_provider.as_ref() == Some(provider)
        || (tools.read_image.enabled
            && tools.read_image.settings.provider.as_ref() == Some(provider));
    let mut out = vec![];
    if chat {
        out.push("chat");
    }
    if tools.stt.enabled && tools.stt.settings.provider.credentials().as_ref() == Some(provider) {
        out.push("stt");
    }
    if tools.tts.enabled && tools.tts.settings.provider.credentials().as_ref() == Some(provider) {
        out.push("tts");
    }
    if tools.image_gen.enabled
        && tools.image_gen.settings.provider.credentials().as_ref() == Some(provider)
    {
        out.push("image_gen");
    }
    if config
        .embedding
        .as_ref()
        .is_some_and(|e| e.provider.credentials().as_ref() == Some(provider))
    {
        out.push("embedding");
    }
    out
}

/// `used_by` for every provider any agent uses, from one scan of the agent configs.
pub async fn used_by_all(deps: &VizierDependencies) -> HashMap<ProviderId, (UsedBy, Vec<AffectedAgent>)> {
    let agents = deps.storage.list_agents().await.unwrap_or_default();
    let mut out: HashMap<ProviderId, (UsedBy, Vec<AffectedAgent>)> = HashMap::new();
    for (agent_id, config) in &agents {
        let mut ids: Vec<ProviderId> = vec![config.provider.clone()];
        ids.extend(config.dream_provider.clone());
        ids.extend(config.tools.read_image.settings.provider.clone());
        ids.extend(config.tools.stt.settings.provider.credentials());
        ids.extend(config.tools.tts.settings.provider.credentials());
        ids.extend(config.tools.image_gen.settings.provider.credentials());
        ids.extend(config.embedding.as_ref().and_then(|e| e.provider.credentials()));
        ids.sort();
        ids.dedup();
        for id in ids {
            let found = features(config, &id);
            if found.is_empty() {
                continue;
            }
            let (used, affected) = out.entry(id).or_default();
            for f in &found {
                match *f {
                    "chat" => used.chat += 1,
                    "stt" => used.stt += 1,
                    "tts" => used.tts += 1,
                    "image_gen" => used.image_gen += 1,
                    _ => used.embedding += 1,
                }
            }
            affected.push(AffectedAgent {
                id: agent_id.clone(),
                name: config.name.clone(),
                features: found.into_iter().map(String::from).collect(),
            });
        }
    }
    out
}

// Reachability -----------------------------------------------------------------------------------

const REACHABLE_TTL: Duration = Duration::from_secs(30);
const REACHABLE_TIMEOUT: Duration = Duration::from_secs(2);

/// `(checked or started at, last answer)`. An entry younger than the TTL is never re-checked,
/// which also keeps one check per provider in flight.
type ReachableCache = parking_lot::Mutex<HashMap<ProviderId, (Instant, Option<bool>)>>;
static REACHABLE: LazyLock<ReachableCache> = LazyLock::new(Default::default);

/// The last known reachability of a local or custom provider. Never waits: a stale or missing
/// answer starts a background check and returns what is known now.
pub fn reachable(deps: &VizierDependencies, id: &ProviderId) -> Option<bool> {
    let mut cache = REACHABLE.lock();
    let now = Instant::now();
    let previous = cache.get(id).copied();
    if let Some((at, value)) = previous
        && now.duration_since(at) < REACHABLE_TTL
    {
        return value;
    }
    let value = previous.and_then(|(_, v)| v);
    cache.insert(id.clone(), (now, value));
    drop(cache);

    let deps = deps.clone();
    let id = id.clone();
    tokio::spawn(async move {
        let ok = match crate::llm::resolve::resolve(&deps, &id, "", &Default::default()).await {
            Ok(resolved) => check(&deps, resolved).await.ok,
            Err(_) => false,
        };
        REACHABLE.lock().insert(id, (Instant::now(), Some(ok)));
    });
    value
}

// Test -------------------------------------------------------------------------------------------

/// The answer to "Test connection".
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TestResult {
    pub ok: bool,
    /// The target the test invoked, when the test is a real call: the SSO provider's test proves
    /// one model or inference profile, so it names it (contract `media-and-test.md` §1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl TestResult {
    fn failed(e: LlmError) -> Self {
        let (status, error) = match e {
            LlmError::Http { status, body, .. } => (
                Some(status),
                Some(if body.trim().is_empty() {
                    reqwest::StatusCode::from_u16(status)
                        .ok()
                        .and_then(|s| s.canonical_reason())
                        .unwrap_or("error")
                        .to_string()
                } else {
                    body.chars().take(300).collect()
                }),
            ),
            other => (None, Some(other.to_string())),
        };
        Self {
            ok: false,
            model: None,
            model_count: None,
            status,
            error,
        }
    }
}

/// One cheap call: the model listing, or a 1-token completion on the Anthropic dialect, which
/// is where a key is checked most reliably.
pub async fn check(deps: &VizierDependencies, mut resolved: ResolvedModel) -> TestResult {
    let no_retry = CompletionRetryConfig {
        max_attempts: 1,
        ..Default::default()
    };
    if resolved.dialect == DialectKind::Anthropic {
        if resolved.model.is_empty() {
            resolved.model = deps
                .catalogue
                .models(&resolved.provider, false)
                .last()
                .map(|m| m.id.clone())
                .unwrap_or_else(|| "claude-haiku-4-5".into());
        }
        let model = crate::llm::build(resolved, deps.http.clone(), no_retry);
        let req = match Message::user_text("ping") {
            Ok(m) => CompletionRequest {
                messages: vec![m],
                tools: vec![],
                tool_choice: None,
                max_tokens: Some(1),
            },
            Err(e) => return TestResult::failed(LlmError::Config(e.to_string())),
        };
        return match tokio::time::timeout(Duration::from_secs(15), model.complete(req)).await {
            Ok(Ok(_)) => TestResult {
                ok: true,
                model: None,
                model_count: None,
                status: None,
                error: None,
            },
            Ok(Err(e)) => TestResult::failed(e),
            Err(_) => TestResult::failed(LlmError::Timeout),
        };
    }
    // The SSO provider's test is a real invoke (contract `media-and-test.md` §1, FR-012a),
    // never the foundation-model listing (FR-012b): only an invoke proves the role may reach
    // *this* model or profile, which is the one thing the person is asking. `amazon-bedrock`'s
    // own behaviour (the listing below) is untouched.
    if resolved.provider == ProviderId::AMAZON_BEDROCK_SSO {
        return check_sso(deps, resolved).await;
    }
    // Bedrock verifies the key pair and region with a signed control-plane listing, which
    // needs no model access and spends no tokens (research R8).
    if resolved.dialect == DialectKind::Bedrock {
        let timeout = Duration::from_secs(15);
        return match tokio::time::timeout(
            timeout,
            crate::llm::bedrock::list_foundation_models(
                &deps.http,
                resolved.credential.as_ref(),
                &resolved.base_url,
                timeout,
            ),
        )
        .await
        {
            Ok(Ok(count)) => TestResult {
                ok: true,
                model: None,
                model_count: Some(count),
                status: None,
                error: None,
            },
            Ok(Err(e)) => TestResult::failed(e),
            Err(_) => TestResult::failed(LlmError::Timeout),
        };
    }
    let model = crate::llm::build(resolved, deps.http.clone(), no_retry);
    match tokio::time::timeout(REACHABLE_TIMEOUT.max(Duration::from_secs(10)), model.list_models())
        .await
    {
        Ok(Ok(models)) => TestResult {
            ok: true,
            model: None,
            model_count: Some(models.len()),
            status: None,
            error: None,
        },
        Ok(Err(e)) => TestResult::failed(e),
        Err(_) => TestResult::failed(LlmError::Timeout),
    }
}

/// The one refusal a test without a target gives, before any AWS request (SC-018).
pub(crate) const NEEDS_MODEL: &str =
    "the test needs a model or inference profile to invoke; choose one first";

/// The SSO connection test: one `Converse` request with one token, the same shape the Anthropic
/// arm sends. A 2xx is the answer, and every other outcome is a message — a failed test is an
/// answer, not an HTTP error (contract `media-and-test.md` §1).
async fn check_sso(deps: &VizierDependencies, resolved: ResolvedModel) -> TestResult {
    let target = resolved.model.clone();
    if target.trim().is_empty() {
        // No target, no request: the answer is the refusal itself (SC-018).
        return TestResult {
            ok: false,
            model: None,
            model_count: None,
            status: None,
            error: Some(NEEDS_MODEL.into()),
        };
    }
    let region = bedrock_region(&resolved.base_url);
    // The dialect and the transport a turn uses, without the retry decorator `llm::build` wraps
    // them in: that decorator rewrites every non-2xx into one "rate limited by the provider after
    // N attempt(s)" string, which would lose both the status this table maps on and AWS's own
    // wording. The test sends exactly one request and reports the answer as it came.
    let model = crate::llm::HttpModel::new(
        crate::llm::dialect::bedrock::Bedrock,
        std::sync::Arc::new(resolved),
        deps.http.clone(),
    );
    let req = match Message::user_text("ping") {
        Ok(m) => CompletionRequest {
            messages: vec![m],
            tools: vec![],
            tool_choice: None,
            max_tokens: Some(1),
        },
        Err(e) => return sso_failure(LlmError::Config(e.to_string()), &target, &region),
    };
    match tokio::time::timeout(Duration::from_secs(15), model.complete(req)).await {
        Ok(Ok(_)) => TestResult {
            ok: true,
            model: Some(target),
            model_count: None,
            status: None,
            error: None,
        },
        Ok(Err(e)) => sso_failure(e, &target, &region),
        Err(_) => sso_failure(LlmError::Timeout, &target, &region),
    }
}

/// The Bedrock region a resolved endpoint names, for the refusal that says where model access is
/// turned on. An endpoint this cannot read is kept as it is, so the message never invents a region.
fn bedrock_region(base_url: &str) -> String {
    base_url
        .trim_start_matches("https://")
        .strip_prefix("bedrock-runtime.")
        .and_then(|rest| rest.strip_suffix(".amazonaws.com"))
        .unwrap_or(base_url)
        .to_string()
}

/// `403` bodies AWS uses when the account has no access to the model. Checked before the action
/// refusals, because AWS's own wording names no action at all.
const MODEL_ACCESS_REFUSALS: [&str; 3] = [
    "don't have access to the model",
    "do not have access to the model",
    "no access to this model",
];

/// `403` bodies that name an IAM action the role may not perform.
const ACTION_REFUSALS: [&str; 3] = [
    "not authorized to perform",
    "is not allowed to perform",
    "accessdenied",
];

/// Contract `media-and-test.md` §1's outcome table, one message per cause. The body of a
/// refusal is passed through **whole** — never shortened the way [`TestResult::failed`] shortens
/// the other providers' — because it is the text a person has to act on.
fn sso_failure(error: LlmError, target: &str, region: &str) -> TestResult {
    let failure = |error: String| TestResult {
        ok: false,
        model: Some(target.to_string()),
        model_count: None,
        status: None,
        error: Some(error),
    };
    match error {
        LlmError::Timeout => failure("request timed out".into()),
        LlmError::Http { status, body, .. } => {
            // FR-016: an ended session is the session's problem, in its own words. A test never
            // reports one as a test failure (contract §1.1).
            if crate::llm::sso::is_session_failure_message(&body) {
                return failure(body);
            }
            let text = body.to_ascii_lowercase();
            match status {
                403 if MODEL_ACCESS_REFUSALS.iter().any(|marker| text.contains(marker)) => {
                    failure(format!(
                        "model {target:?} is not enabled for this account in {region}"
                    ))
                }
                403 if ACTION_REFUSALS.iter().any(|marker| text.contains(marker)) => {
                    failure(
                        crate::llm::sso::SsoError::PermissionMissing {
                            action: "perform bedrock:InvokeModel".into(),
                        }
                        .to_string(),
                    )
                }
                // Only an ARN can be a profile: a foundation-model id that is missing is AWS's
                // own answer, passed through below.
                404 if crate::llm::bedrock::parse_inference_profile_arn(target).is_ok() => {
                    failure(format!(
                        "inference profile {target:?} was not found; it may have been deleted"
                    ))
                }
                429 => failure("AWS throttled the test; try again shortly".into()),
                _ => failure(if body.trim().is_empty() {
                    reqwest::StatusCode::from_u16(status)
                        .ok()
                        .and_then(|s| s.canonical_reason())
                        .unwrap_or("error")
                        .to_string()
                } else {
                    body
                }),
            }
        }
        // A session or credential failure reaches here as `Config`, whose text is already the
        // `SsoError` message verbatim; the rest say what they say.
        other => failure(other.to_string()),
    }
}

/// Resolve `provider` as it would be with `credentials` laid over what is saved, for a test
/// before saving.
pub async fn resolve_for_test(
    deps: &VizierDependencies,
    provider: &ProviderId,
    credentials: Option<ProviderCredentials>,
    custom: Option<&CustomProvider>,
    model: &str,
) -> crate::Result<ResolvedModel> {
    let env = crate::llm::resolve::provider_env(&deps.catalogue, provider);
    let saved = crate::provider_keys::resolve_credentials(&deps.storage, provider, &env)
        .await
        .map(|(c, _)| c);
    let mut credentials = match (saved, credentials) {
        (Some(saved), Some(new)) => Some(saved.merged_with(&new)),
        (saved, new) => new.or(saved),
    };
    let mut sso = None;
    if *provider == ProviderId::AMAZON_BEDROCK_SSO {
        let merged = credentials.clone().unwrap_or_default();
        // The role credential is built from the values on the form laid over the saved row, so
        // the test proves the connection the person is looking at (FR-012a). It reads the session
        // store itself, so this hook needs no second lookup of its own.
        sso = Some(crate::llm::resolve::bedrock_credentials_from(
            deps, provider, &merged, None,
        )?);
        // An inference-profile ARN that is not saved yet is still the target of the test:
        // `resolve_with` refuses an unknown ARN as a profile that is not on the connection
        // (FR-030), which is the right answer for a turn and the wrong one here. The ARN is
        // invoked exactly as given, with the same credentials and with no entry's limits, so AWS
        // gets to answer 200, 403 or 404 — the answer the test exists to fetch.
        if let Ok(arn) = crate::llm::bedrock::parse_inference_profile_arn(model)
            && !merged.profiles.iter().any(|entry| entry.arn.trim() == arn.raw)
        {
            let mut with_target = merged;
            with_target.profiles.push(ProfileEntry {
                arn: arn.raw,
                ..Default::default()
            });
            credentials = Some(with_target);
        }
    }
    crate::llm::resolve::resolve_with(
        &ResolveContext {
            catalogue: &deps.catalogue,
            custom,
            credentials,
            http: deps.http.clone(),
            sso,
        },
        provider,
        model,
        &ModelOverrides::default(),
    )
}

/// `{new id: legacy name}` recorded by `migrate_provider_ids_v1`.
pub async fn migrated_from(deps: &VizierDependencies) -> BTreeMap<String, String> {
    use crate::storage::state::StateStorage;
    deps
        .storage
        .get_state(crate::provider_migration::MAP_KEY.to_string())
        .await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}


/// Restart every agent using `id`, so a changed key or a removed provider takes effect now
/// rather than at the next restart. An agent that can no longer build shows as `Failed`.
pub async fn restart_users(deps: &VizierDependencies, id: &ProviderId) {
    let agents = used_by_all(deps)
        .await
        .remove(id)
        .map(|(_, a)| a)
        .unwrap_or_default();
    for agent in agents {
        let (resp, rx) = tokio::sync::oneshot::channel();
        let cmd = crate::schema::AgentCommand::Restart {
            agent_id: agent.id.clone(),
            resp,
        };
        if deps.transport.send_agent_command(cmd).await.is_err() {
            continue;
        }
        if let Ok(crate::schema::AgentCommandResult::Error(e)) = rx.await {
            tracing::warn!("restarting agent '{}' after a change to '{id}': {e}", agent.id);
        }
    }
}

/// The agents a delete would affect, for the WebUI's warning.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct InUse {
    pub agents: Vec<AffectedAgent>,
}

/// A `409` naming the agents using `id`, unless the delete is confirmed.
pub async fn check_in_use(
    deps: &VizierDependencies,
    id: &ProviderId,
    confirm: bool,
) -> Result<(), crate::service::ServiceError> {
    if confirm {
        return Ok(());
    }
    let agents = used_by_all(deps)
        .await
        .remove(id)
        .map(|(_, a)| a)
        .unwrap_or_default();
    if agents.is_empty() {
        return Ok(());
    }
    let names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
    Err(crate::service::ServiceError::Conflict {
        message: format!(
            "used by {}; resend with ?confirm=true to delete anyway",
            names.join(", ")
        ),
        code: None,
        data: serde_json::to_value(InUse { agents }).ok(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::agent::{EmbeddingConfig, EmbeddingProvider, ImageGenProvider};

    fn agent_config() -> AgentConfig {
        serde_json::from_value(serde_json::json!({
            "name": "a", "owner_id": null, "system_prompt": null, "description": null,
            "provider": "dummyplug", "model": "dummyplug", "thinking_depth": 1,
            "tools": {"timeout": "30s"}, "silent_read_initiative_chance": 0.0,
            "prompt_timeout": "1m", "heartbeat_interval": "1m"
        }))
        .unwrap()
    }

    /// One saved Bedrock connection is the chat provider, the embedding provider and the image
    /// provider at once, so a credential change restarts this agent and no other provider's.
    #[test]
    fn bedrock_counts_for_chat_embeddings_and_images_together() {
        let mut config = agent_config();
        config.provider = ProviderId::AMAZON_BEDROCK;
        config.model = "amazon.nova-lite-v1:0".into();
        config.embedding = Some(EmbeddingConfig {
            provider: EmbeddingProvider::Bedrock,
            model: String::new(),
        });
        config.tools.image_gen.enabled = true;
        config.tools.image_gen.settings.provider = ImageGenProvider::Bedrock;

        let found = features(&config, &ProviderId::AMAZON_BEDROCK);
        assert!(found.contains(&"chat"), "{found:?}");
        assert!(found.contains(&"embedding"), "{found:?}");
        assert!(found.contains(&"image_gen"), "{found:?}");
        assert_eq!(found.iter().filter(|f| **f == "chat").count(), 1, "{found:?}");
        assert!(features(&config, &ProviderId::OPENAI).is_empty());
    }

    // The SSO connection test is a real invoke (contract `media-and-test.md` §1). The stub
    // server of `llm::sso::tests_support` stands in for the portal and the runtime at once, so
    // these tests run the same request path a turn takes.

    use std::sync::Arc;

    use crate::catalogue::data::Capabilities;
    use crate::config::VizierConfig;
    use crate::dependencies::VizierDependencies;
    use crate::llm::auth::{SsoRoleCredential, SsoRoleTarget};
    use crate::llm::resolve::ContextSource;
    use crate::llm::sso::SsoClient;
    use crate::llm::sso::tests_support::{MemSessions, Reply, Stub};
    use crate::schema::SsoSessionSecret;
    use crate::storage::sso_session::SsoSessionStorage;

    const TARGET: &str = "amazon.nova-lite-v1:0";
    const PROFILE_ARN: &str =
        "arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/abc123def456";

    async fn deps() -> (VizierDependencies, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let config = VizierConfig {
            workspace: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        };
        (VizierDependencies::new(config).await.unwrap(), dir)
    }

    /// The SSO connection exactly as the form sends it.
    fn sso_credentials() -> ProviderCredentials {
        ProviderCredentials {
            start_url: Some("https://acme.awsapps.com/start".into()),
            sso_region: Some("us-east-1".into()),
            account_id: Some("123456789012".into()),
            role_name: Some("BedrockOperator".into()),
            sso_profile: Some("acme-bedrock".into()),
            region: Some("us-east-1".into()),
            ..Default::default()
        }
    }

    fn live_session() -> SsoSessionSecret {
        SsoSessionSecret {
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

    /// One `GetRoleCredentials` answer, good for an hour: the invoke needs a credential, and a
    /// renewal of the token itself is a different test's business.
    fn role_credentials() -> Reply {
        Reply::ok(serde_json::json!({
            "roleCredentials": {
                "accessKeyId": "ASIAEXAMPLEKEYID",
                "secretAccessKey": "secret-access-value",
                "sessionToken": "session-token-value",
                "expiration": (chrono::Utc::now() + chrono::TimeDelta::hours(1)).timestamp_millis(),
            }
        }))
    }

    /// The cheapest `Converse` answer a 2xx can be.
    fn converse() -> Reply {
        Reply::ok(serde_json::json!({
            "output": {"message": {"role": "assistant", "content": [{"text": "pong"}]}},
            "stopReason": "end_turn",
            "usage": {"inputTokens": 1, "outputTokens": 1, "totalTokens": 2}
        }))
    }

    /// `target` as a turn would build it, with both the SSO portal and the Bedrock runtime at
    /// `stub`, so `check` sends its real request over a socket.
    fn sso_model(stub: &Stub, target: &str, sessions: Arc<MemSessions>) -> ResolvedModel {
        ResolvedModel {
            provider: ProviderId::AMAZON_BEDROCK_SSO,
            model: target.to_string(),
            dialect: DialectKind::Bedrock,
            base_url: stub.base().to_string(),
            credential: Arc::new(SsoRoleCredential::new(
                SsoClient::at(reqwest::Client::new(), stub.base()),
                sessions,
                SsoRoleTarget {
                    profile: "acme-bedrock".into(),
                    account_id: "123456789012".into(),
                    role_name: "BedrockOperator".into(),
                    region: "us-east-1".into(),
                },
            )),
            headers: vec![],
            query: vec![],
            context_window: None,
            context_source: ContextSource::Unknown,
            max_tokens: None,
            max_output: None,
            caps: Capabilities::default(),
            cost: None,
            stream_required: false,
            default_instructions: None,
        }
    }

    /// The same, signed in: a session row the credential can renew from.
    async fn signed_in(stub: &Stub, target: &str) -> ResolvedModel {
        let sessions = Arc::new(MemSessions::default());
        sessions
            .put_sso_session("acme-bedrock", &live_session(), None)
            .await
            .unwrap();
        sso_model(stub, target, sessions)
    }

    #[tokio::test]
    async fn the_sso_test_invokes_the_target_and_names_it() {
        let path = crate::llm::bedrock::model_path(TARGET, "converse");
        let stub = Stub::start_replies(vec![
            ("/federation/credentials", vec![role_credentials()]),
            (path.as_str(), vec![converse()]),
        ])
        .await;
        let (deps, _dir) = deps().await;

        let result = check(&deps, signed_in(&stub, TARGET).await).await;

        assert!(result.ok, "{:?}", result.error);
        assert_eq!(result.model.as_deref(), Some(TARGET));
        assert_eq!(
            stub.path_count(&path),
            1,
            "one Converse request and nothing else"
        );
        let (_, body) = stub
            .seen()
            .into_iter()
            .find(|(seen, _)| *seen == path)
            .expect("the invoke reached the runtime");
        assert!(body.contains("\"ping\""), "{body}");
        assert!(body.contains("\"maxTokens\":1"), "one token: {body}");
        assert!(!body.contains("toolConfig"), "no tools: {body}");
    }

    #[tokio::test]
    async fn a_long_refusal_comes_back_whole() {
        let path = crate::llm::bedrock::model_path(TARGET, "converse");
        let long = format!("{}END OF THE REFUSAL", "x".repeat(400));
        let stub = Stub::start_replies(vec![
            ("/federation/credentials", vec![role_credentials()]),
            (
                path.as_str(),
                vec![Reply {
                    status: 400,
                    body: serde_json::json!(long),
                }],
            ),
        ])
        .await;
        let (deps, _dir) = deps().await;

        let result = check(&deps, signed_in(&stub, TARGET).await).await;

        let error = result.error.expect("a refusal");
        assert!(!result.ok);
        assert_eq!(result.model.as_deref(), Some(TARGET));
        assert!(error.len() > 300, "not shortened: {} chars", error.len());
        assert!(error.contains("END OF THE REFUSAL"), "{error}");
    }

    #[tokio::test]
    async fn a_throttled_test_says_so() {
        let path = crate::llm::bedrock::model_path(TARGET, "converse");
        let stub = Stub::start_replies(vec![
            ("/federation/credentials", vec![role_credentials()]),
            (
                path.as_str(),
                vec![Reply {
                    status: 429,
                    body: serde_json::json!({"message": "ThrottlingException: too many requests"}),
                }],
            ),
        ])
        .await;
        let (deps, _dir) = deps().await;

        let result = check(&deps, signed_in(&stub, TARGET).await).await;

        assert_eq!(
            result.error.as_deref(),
            Some("AWS throttled the test; try again shortly")
        );
    }

    #[tokio::test]
    async fn a_missing_invoke_permission_names_the_action() {
        let path = crate::llm::bedrock::model_path(TARGET, "converse");
        let stub = Stub::start_replies(vec![
            ("/federation/credentials", vec![role_credentials()]),
            (
                path.as_str(),
                vec![Reply {
                    status: 403,
                    body: serde_json::json!({"message": "User: arn:aws:sts::123456789012:assumed-role/BedrockOperator/session is not authorized to perform: bedrock:InvokeModel on resource: arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/abc123def456 because no identity-based policy allows the bedrock:InvokeModel action"}),
                }],
            ),
        ])
        .await;
        let (deps, _dir) = deps().await;

        let result = check(&deps, signed_in(&stub, TARGET).await).await;

        let error = result.error.expect("the refusal is the answer");
        assert!(error.contains("bedrock:InvokeModel"), "{error}");
    }

    /// The region is what tells a person where to enable the model, so it is named even though
    /// only the endpoint carries it.
    #[test]
    fn a_model_access_refusal_names_the_model_and_the_region() {
        let refusal = LlmError::Http {
            status: 403,
            retry_after: None,
            body: "{\"message\":\"You don't have access to the model with the specified model ID.\"}"
                .into(),
        };
        let result = sso_failure(
            refusal,
            TARGET,
            &bedrock_region("https://bedrock-runtime.eu-west-1.amazonaws.com"),
        );
        assert_eq!(
            result.error.as_deref(),
            Some("model \"amazon.nova-lite-v1:0\" is not enabled for this account in eu-west-1")
        );
    }

    #[tokio::test]
    async fn a_deleted_inference_profile_is_named_in_its_own_words() {
        let path = crate::llm::bedrock::model_path(PROFILE_ARN, "converse");
        let stub = Stub::start_replies(vec![
            ("/federation/credentials", vec![role_credentials()]),
            (
                path.as_str(),
                vec![Reply {
                    status: 404,
                    body: serde_json::json!({"message": "ResourceNotFoundException"}),
                }],
            ),
        ])
        .await;
        let (deps, _dir) = deps().await;

        let result = check(&deps, signed_in(&stub, PROFILE_ARN).await).await;

        let expected = format!(
            "inference profile \"{PROFILE_ARN}\" was not found; it may have been deleted"
        );
        assert_eq!(result.error.as_deref(), Some(expected.as_str()));
    }

    #[test]
    fn a_timeout_says_it_timed_out() {
        assert_eq!(
            sso_failure(LlmError::Timeout, TARGET, "us-east-1")
                .error
                .as_deref(),
            Some("request timed out")
        );
    }

    #[tokio::test]
    async fn a_test_without_a_target_sends_no_request() {
        let stub = Stub::start_replies(vec![]).await;
        let (deps, _dir) = deps().await;

        let result = check(&deps, signed_in(&stub, "").await).await;

        assert!(!result.ok);
        assert!(result.model.is_none(), "nothing was invoked");
        assert_eq!(result.error.as_deref(), Some(NEEDS_MODEL));
        assert!(stub.seen().is_empty(), "no AWS request: {:?}", stub.seen());
    }

    /// Contract §1.1: a test never reports a session problem as a test failure.
    #[tokio::test]
    async fn an_ended_session_is_the_sessions_problem() {
        let stub = Stub::start_replies(vec![]).await;
        let (deps, _dir) = deps().await;
        let signed_out = sso_model(&stub, TARGET, Arc::new(MemSessions::default()));

        let result = check(&deps, signed_out).await;

        let error = result.error.expect("the session is named");
        assert!(crate::llm::sso::is_session_failure_message(&error), "{error}");
        assert!(error.contains("sign in again"), "{error}");
        assert!(stub.seen().is_empty(), "nothing was asked of AWS");
    }

    /// Nothing is saved yet, so this only succeeds because the role credential is built from the
    /// values in the request body: the proof that `sso` is connected before a save (FR-012a).
    #[tokio::test]
    async fn the_pre_save_resolve_uses_the_credentials_in_the_body() {
        let (deps, _dir) = deps().await;

        let resolved = resolve_for_test(
            &deps,
            &ProviderId::AMAZON_BEDROCK_SSO,
            Some(sso_credentials()),
            None,
            TARGET,
        )
        .await
        .expect("resolved from the body alone");

        assert_eq!(resolved.model, TARGET);
        assert_eq!(resolved.dialect, DialectKind::Bedrock);
        assert_eq!(
            resolved.base_url,
            "https://bedrock-runtime.us-east-1.amazonaws.com"
        );
    }

    /// A profile that is not on the connection yet is still the target of a test: AWS should
    /// answer 200, 403 or 404 rather than the test refusing the ARN on its own behalf.
    #[tokio::test]
    async fn an_unsaved_inference_profile_arn_is_the_target_of_the_test() {
        let (deps, _dir) = deps().await;

        let resolved = resolve_for_test(
            &deps,
            &ProviderId::AMAZON_BEDROCK_SSO,
            Some(sso_credentials()),
            None,
            PROFILE_ARN,
        )
        .await
        .expect("an ARN is a target, not a missing entry");

        assert_eq!(resolved.model, PROFILE_ARN);
        assert_eq!(resolved.context_window, None, "no entry means no limits");
        assert_eq!(resolved.context_source, ContextSource::Unknown);
    }
}
