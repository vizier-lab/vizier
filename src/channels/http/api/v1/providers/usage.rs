//! Which agents use a provider (FR-010b), whether a local or custom provider answers
//! (FR-010c), and the one cheap call behind "Test connection".

use std::collections::{BTreeMap, HashMap};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::{
    channels::http::state::HTTPState,
    config::provider::ProviderId,
    llm::{
        CompletionRequest, DialectKind, LlmError, Message,
        resolve::{ModelOverrides, ResolveContext, ResolvedModel},
    },
    schema::{AgentConfig, CustomProvider, ProviderCredentials, agent::CompletionRetryConfig},
    storage::agent::AgentStorage,
};

/// Agents per feature using one provider's credentials. Dream and read-image count as chat.
#[derive(Debug, Clone, Default, Serialize, utoipa::ToSchema)]
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
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
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
pub async fn used_by_all(state: &HTTPState) -> HashMap<ProviderId, (UsedBy, Vec<AffectedAgent>)> {
    let agents = state.storage.list_agents().await.unwrap_or_default();
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
pub fn reachable(state: &HTTPState, id: &ProviderId) -> Option<bool> {
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

    let state = state.clone();
    let id = id.clone();
    tokio::spawn(async move {
        let ok = match crate::llm::resolve::resolve(&state.deps, &id, "", &Default::default()).await {
            Ok(resolved) => check(&state, resolved).await.ok,
            Err(_) => false,
        };
        REACHABLE.lock().insert(id, (Instant::now(), Some(ok)));
    });
    value
}

// Test -------------------------------------------------------------------------------------------

/// The answer to "Test connection".
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct TestResult {
    pub ok: bool,
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
            model_count: None,
            status,
            error,
        }
    }
}

/// One cheap call: the model listing, or a 1-token completion on the Anthropic dialect, which
/// is where a key is checked most reliably.
pub async fn check(state: &HTTPState, mut resolved: ResolvedModel) -> TestResult {
    let no_retry = CompletionRetryConfig {
        max_attempts: 1,
        ..Default::default()
    };
    if resolved.dialect == DialectKind::Anthropic {
        if resolved.model.is_empty() {
            resolved.model = state
                .catalogue
                .models(&resolved.provider, false)
                .last()
                .map(|m| m.id.clone())
                .unwrap_or_else(|| "claude-haiku-4-5".into());
        }
        let model = crate::llm::build(resolved, state.deps.http.clone(), no_retry);
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
                model_count: None,
                status: None,
                error: None,
            },
            Ok(Err(e)) => TestResult::failed(e),
            Err(_) => TestResult::failed(LlmError::Timeout),
        };
    }
    // Bedrock verifies the key pair and region with a signed control-plane listing, which
    // needs no model access and spends no tokens (research R8).
    if resolved.dialect == DialectKind::Bedrock {
        let timeout = Duration::from_secs(15);
        return match tokio::time::timeout(
            timeout,
            crate::llm::bedrock::list_foundation_models(
                &state.deps.http,
                resolved.credential.as_ref(),
                &resolved.base_url,
                timeout,
            ),
        )
        .await
        {
            Ok(Ok(count)) => TestResult {
                ok: true,
                model_count: Some(count),
                status: None,
                error: None,
            },
            Ok(Err(e)) => TestResult::failed(e),
            Err(_) => TestResult::failed(LlmError::Timeout),
        };
    }
    let model = crate::llm::build(resolved, state.deps.http.clone(), no_retry);
    match tokio::time::timeout(REACHABLE_TIMEOUT.max(Duration::from_secs(10)), model.list_models())
        .await
    {
        Ok(Ok(models)) => TestResult {
            ok: true,
            model_count: Some(models.len()),
            status: None,
            error: None,
        },
        Ok(Err(e)) => TestResult::failed(e),
        Err(_) => TestResult::failed(LlmError::Timeout),
    }
}

/// Resolve `provider` as it would be with `credentials` laid over what is saved, for a test
/// before saving.
pub async fn resolve_for_test(
    state: &HTTPState,
    provider: &ProviderId,
    credentials: Option<ProviderCredentials>,
    custom: Option<&CustomProvider>,
) -> crate::Result<ResolvedModel> {
    let env = crate::llm::resolve::provider_env(&state.catalogue, provider);
    let saved = crate::provider_keys::resolve_credentials(&state.storage, provider, &env)
        .await
        .map(|(c, _)| c);
    let credentials = match (saved, credentials) {
        (Some(saved), Some(new)) => Some(saved.merged_with(&new)),
        (saved, new) => new.or(saved),
    };
    crate::llm::resolve::resolve_with(
        &ResolveContext {
            catalogue: &state.catalogue,
            custom,
            credentials,
            http: state.deps.http.clone(),
        },
        provider,
        "",
        &ModelOverrides::default(),
    )
}

/// `{new id: legacy name}` recorded by `migrate_provider_ids_v1`.
pub async fn migrated_from(state: &HTTPState) -> BTreeMap<String, String> {
    use crate::storage::state::StateStorage;
    state
        .storage
        .get_state(crate::provider_migration::MAP_KEY.to_string())
        .await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::agent::{EmbeddingConfig, EmbeddingProvider, ImageGenProvider};

    /// One saved Bedrock connection is the chat provider, the embedding provider and the image
    /// provider at once, so a credential change restarts this agent and no other provider's.
    #[test]
    fn bedrock_counts_for_chat_embeddings_and_images_together() {
        let mut config = crate::tests_support::agent_config("a");
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
}
