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
    schema::{AgentConfig, AgentId},
};

mod dummyplug;
pub mod registry;

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
#[derive(Clone)]
pub struct VizierModel {
    inner: Arc<dyn CompletionModel>,
    info: Arc<ModelInfo>,
}

impl VizierModel {
    pub fn context_window(&self) -> Option<u64> {
        self.info.context_window
    }

    pub fn info(&self) -> &ModelInfo {
        &self.info
    }

    pub async fn complete(&self, req: CompletionRequest) -> Result<CompletionResponse> {
        self.complete_until(req, None).await
    }

    /// A completion, retried on transient failures, that gives up at `deadline`: the turn's
    /// `prompt_timeout`, which otherwise only applies between completions (#41).
    pub async fn complete_until(
        &self,
        req: CompletionRequest,
        deadline: Option<Instant>,
    ) -> Result<CompletionResponse> {
        Ok(self.inner.complete_until(req, deadline).await?)
    }

    pub async fn new(
        _agent_id: AgentId,
        deps: VizierDependencies,
        agent_config: &AgentConfig,
    ) -> Result<Self> {
        Self::build(
            &deps,
            agent_config,
            agent_config.provider.clone(),
            agent_config.model.clone(),
        )
        .await
    }

    pub async fn new_with_override(
        deps: &VizierDependencies,
        agent_config: &AgentConfig,
        model_override: Option<(ProviderId, String)>,
    ) -> Result<Self> {
        let (provider, model) = model_override
            .unwrap_or_else(|| (agent_config.provider.clone(), agent_config.model.clone()));
        Self::build(deps, agent_config, provider, model).await
    }

    async fn build(
        deps: &VizierDependencies,
        agent_config: &AgentConfig,
        provider: ProviderId,
        model: String,
    ) -> Result<Self> {
        let retry = agent_config.completion_retry.clone();

        // dummyplug is the built-in whose model is its own, with no network.
        if provider == ProviderId::DUMMYPLUG {
            return Ok(Self {
                inner: Arc::new(RetryingModel::new(
                    dummyplug::DummyplugModel::new(agent_config),
                    retry,
                )),
                info: Arc::new(ModelInfo {
                    provider,
                    model,
                    context_window: agent_config.context_window,
                    max_output: agent_config.max_tokens,
                    context_source: if agent_config.context_window.is_some() {
                        ContextSource::Override
                    } else {
                        ContextSource::Unknown
                    },
                    caps: Capabilities::default(),
                    cost: None,
                }),
            });
        }

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
        })
    }
}
