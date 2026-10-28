use std::sync::Arc;

use anyhow::Result;
use tokio::time::Instant;

use crate::{
    catalogue::data::Capabilities,
    config::provider::ProviderId,
    dependencies::VizierDependencies,
    llm::{
        self, CompletionModel, CompletionRequest, CompletionResponse, ContextSource, RetryingModel,
        resolve::ModelOverrides,
    },
    schema::{AgentConfig, AgentId, metrics::TokenCounts, usage::UsageEvent},
    storage::{VizierStorage, usage::UsageStorage},
};

use usage::{TurnGuard, UsageScope};

pub mod registry;
pub mod usage;

pub use crate::llm::model::{PROMPT_TIMED_OUT, PROVIDER_UNAVAILABLE, RATE_LIMITED};

/// What an agent's model resolved to, for agent health and `vizier agent ps`.
#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub provider: ProviderId,
    pub model: String,
    pub context_window: Option<u64>,
    pub max_output: Option<u64>,
    pub context_source: ContextSource,
    pub caps: Capabilities,
    pub cost: Option<crate::catalogue::data::Cost>,
}

/// An agent's completion model: `llm::resolve`, then `llm::build`, behind one cheap handle.
///
/// Every answered completion is recorded as a usage event for `agent_id` (`specs/018-usage-
/// accounting/` R1): this is the one place chat, dream, handover and vision calls all pass.
#[derive(Clone)]
pub struct VizierModel {
    inner: Arc<dyn CompletionModel>,
    info: Arc<ModelInfo>,
    agent_id: AgentId,
    storage: Arc<VizierStorage>,
}

impl VizierModel {
    pub fn context_window(&self) -> Option<u64> {
        self.info.context_window
    }

    pub fn info(&self) -> &ModelInfo {
        &self.info
    }

    pub async fn complete(
        &self,
        req: CompletionRequest,
        scope: &UsageScope,
    ) -> Result<CompletionResponse> {
        self.complete_until(req, None, scope).await
    }

    /// A completion, retried on transient failures, that gives up at `deadline`: the turn's
    /// `prompt_timeout`, which otherwise only applies between completions (#41).
    ///
    /// An answered call is recorded off the call's path (R2): a failed write is logged and never
    /// delays or fails the call. `RetryingModel` returns only the answered attempt, so a retried
    /// call is recorded once (FR-005); a call that errors is not recorded at all.
    pub async fn complete_until(
        &self,
        req: CompletionRequest,
        deadline: Option<Instant>,
        scope: &UsageScope,
    ) -> Result<CompletionResponse> {
        let started = Instant::now();
        let response = self.inner.complete_until(req, deadline).await?;
        self.record(scope, &response, started.elapsed());
        Ok(response)
    }

    fn record(&self, scope: &UsageScope, response: &CompletionResponse, took: std::time::Duration) {
        let session = scope.session.as_ref();
        let event = UsageEvent {
            agent_id: self.agent_id.clone(),
            at: chrono::Utc::now(),
            activity: scope.activity(),
            channel: session.map(|s| s.1.to_slug()),
            topic: session.and_then(|s| s.2.clone()),
            place: session.map_or_else(|| "system".to_string(), |s| s.usage_place()),
            turn_id: scope.turn.as_ref().map(|t| t.as_str().to_string()),
            round: scope.round,
            provider: self.info.provider.to_string(),
            model: self.info.model.clone(),
            served_model: response.model.clone(),
            tokens: TokenCounts {
                input: response.usage.input_tokens,
                cached: response.usage.cached_input_tokens,
                cache_write: response.usage.cache_creation_input_tokens,
                output: response.usage.output_tokens,
            },
            duration_ms: took.as_millis() as u64,
            legacy: false,
        };
        let storage = self.storage.clone();
        tokio::spawn(async move {
            let agent_id = event.agent_id.clone();
            if let Err(e) = storage.record_usage(event).await {
                tracing::warn!(agent_id = %agent_id, "failed to record model usage: {e}");
            }
        });
    }

    /// A guard for one turn's rounds; see [`TurnGuard`].
    pub fn guard(&self) -> TurnGuard {
        TurnGuard::new(self.agent_id.clone(), self.storage.clone())
    }

    pub async fn new(
        agent_id: AgentId,
        deps: VizierDependencies,
        agent_config: &AgentConfig,
    ) -> Result<Self> {
        Self::build(
            &agent_id,
            &deps,
            agent_config,
            agent_config.provider.clone(),
            agent_config.model.clone(),
        )
        .await
    }

    pub async fn new_with_override(
        agent_id: &AgentId,
        deps: &VizierDependencies,
        agent_config: &AgentConfig,
        model_override: Option<(ProviderId, String)>,
    ) -> Result<Self> {
        let (provider, model) = model_override
            .unwrap_or_else(|| (agent_config.provider.clone(), agent_config.model.clone()));
        Self::build(agent_id, deps, agent_config, provider, model).await
    }

    async fn build(
        agent_id: &AgentId,
        deps: &VizierDependencies,
        agent_config: &AgentConfig,
        provider: ProviderId,
        model: String,
    ) -> Result<Self> {
        let retry = agent_config.completion_retry.clone();

        let overrides = ModelOverrides {
            context_window: agent_config.context_window,
            max_tokens: agent_config.max_tokens,
        };
        let mut resolved = llm::resolve::resolve(deps, &provider, &model, &overrides).await?;
        llm::resolve::fill_context_from_provider(&mut resolved, &deps.http).await;
        if resolved.context_window.is_none()
            && let Some(window) = registry::detect_context_window(&model)
        {
            resolved.context_window = Some(window);
            resolved.context_source = ContextSource::Name;
        }

        let info = ModelInfo {
            provider: resolved.provider.clone(),
            model: resolved.model.clone(),
            context_window: resolved.context_window,
            max_output: resolved.max_tokens.or(resolved.max_output),
            context_source: resolved.context_source,
            caps: resolved.caps.clone(),
            cost: resolved.cost.clone(),
        };
        tracing::debug!(
            provider = %info.provider,
            model = %info.model,
            dialect = ?resolved.dialect,
            context_window = ?info.context_window,
            context_source = ?info.context_source,
            "model resolved"
        );
        Ok(Self {
            inner: llm::build(resolved, deps.http.clone(), retry),
            info: Arc::new(info),
            agent_id: agent_id.clone(),
            storage: deps.storage.clone(),
        })
    }
}
