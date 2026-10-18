use anyhow::Result;
use axum::{Router, extract::DefaultBodyLimit, routing::get};
use reqwest::{
    Method,
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::normalize_path::NormalizePathLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    channels::{VizierChannel, http::state::HTTPState},
    config::HTTPChannelConfig,
    dependencies::VizierDependencies,
};

pub mod models;

pub mod api;
pub mod auth;
pub mod rate_limit;
mod state;
mod webui;

pub struct HTTPChannel {
    config: HTTPChannelConfig,
    deps: VizierDependencies,
}

impl HTTPChannel {
    pub fn new(config: HTTPChannelConfig, deps: VizierDependencies) -> Result<Self> {
        Ok(Self { config, deps })
    }
}

#[async_trait::async_trait]
impl VizierChannel for HTTPChannel {
    async fn run(&self) -> Result<()> {
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::PATCH,
                Method::DELETE,
                Method::OPTIONS,
            ])
            .allow_headers([AUTHORIZATION, ACCEPT, CONTENT_TYPE]);

        let state = HTTPState {
            config: self.deps.config.clone(),
            storage: self.deps.storage.clone(),
            transport: self.deps.transport.clone(),
            background_jobs: self.deps.background_jobs.clone(),
            catalogue: self.deps.catalogue.clone(),
            deps: self.deps.clone(),
            client_events: std::sync::Arc::new(rate_limit::RateLimiter::new(
                10,
                std::time::Duration::from_secs(60),
            )),
            client_state_saves: std::sync::Arc::new(rate_limit::RateLimiter::new(
                2,
                std::time::Duration::from_secs(1),
            )),
            deferred_client_calls: Default::default(),
        };

        let mut app = Router::new()
            .nest("/api", api::api(state.clone()))
            // webui
            .route("/", get(webui::index))
            .route("/{*path}", get(webui::assets))
            .layer(NormalizePathLayer::trim_trailing_slash())
            .layer(cors)
            .layer(DefaultBodyLimit::disable())
            .with_state(state);

        app = app.merge(SwaggerUi::new("/swagger").url("/openapi.json", ApiDoc::openapi()));

        let listener =
            tokio::net::TcpListener::bind(format!("0.0.0.0:{}", self.config.port)).await?;

        let server = axum::serve(listener, app);
        tracing::info!("http listening on port {}", self.config.port);

        server.await?;

        Ok(())
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        api::v1::ping,
        api::v1::auth::login,
        api::v1::auth::change_password,
        api::v1::auth::create_api_key,
        api::v1::auth::list_api_keys,
        api::v1::auth::delete_api_key,
        api::v1::agents::list_agents,
        api::v1::agents::create_agent,
        api::v1::agents::agent_detail,
        api::v1::agents::update_agent,
        api::v1::agents::delete_agent,
        api::v1::agents::agent_usage,
        api::v1::agents::agent_usage_conversation,
        api::v1::agents::channel::list_topics,
        api::v1::agents::channel::get_topic_history,
        api::v1::agents::channel::delete_topic,
        api::v1::agents::channel::chat,
        api::v1::agents::jobs::list_jobs,
        api::v1::agents::jobs::get_job,
        api::v1::agents::jobs::cancel_job,
        api::v1::agents::jobs::get_piece_history,
        api::v1::agents::integrations::list,
        api::v1::agents::integrations::create,
        api::v1::agents::integrations::set_attachment,
        api::v1::agents::integrations::remove,
        api::v1::agents::integrations::get_grants,
        api::v1::agents::integrations::set_grants,
        api::v1::agents::integrations::list_events,
        api::v1::integrations::get_integration,
        api::v1::integrations::update_integration,
        api::v1::integrations::delete_integration,
        api::v1::integrations::list_kinds,
        api::v1::integrations::kind_icon,
        api::v1::agents::agent_chat,
        api::v1::agents::core::get_core,
        api::v1::agents::core::update_core,
        api::v1::agents::core::get_core_history,
        api::v1::agents::core::rollback_core,
        api::v1::agents::memory::get_all_memories,
        api::v1::agents::memory::create_memory,
        api::v1::agents::memory::get_memory_detail,
        api::v1::agents::memory::update_memory,
        api::v1::agents::memory::delete_memory,
        api::v1::agents::memory::query_memories,
        api::v1::agents::memory::get_memory_history,
        api::v1::agents::memory::rollback_memory,
        api::v1::agents::task::get_tasks,
        api::v1::agents::task::get_task,
        api::v1::agents::task::create_task,
        api::v1::agents::task::update_task,
        api::v1::agents::task::delete_task,
        api::v1::agents::task::list_runs,
        api::v1::agents::task::get_run_history,
        api::v1::files::upload_file,
        api::v1::files::download_file,
        api::v1::providers::list_providers,
        api::v1::providers::get_provider,
        api::v1::providers::upsert_provider,
        api::v1::providers::delete_provider,
        api::v1::providers::test_provider,
        api::v1::providers::custom::list,
        api::v1::providers::custom::create,
        api::v1::providers::custom::update,
        api::v1::providers::custom::remove,
        api::v1::providers::custom::fetch_models,
        api::v1::providers::custom::test,
        api::v1::catalogue::catalogue_status,
        api::v1::catalogue::list_providers,
        api::v1::catalogue::list_models,
        api::v1::catalogue::get_model,
        api::v1::catalogue::logo,
        api::v1::catalogue::refresh,
        api::v1::utils::setup_state,
    ),
    components(
        schemas(
            api::v1::auth::LoginRequest,
            api::v1::auth::LoginResponse,
            api::v1::auth::ChangePasswordRequest,
            api::v1::auth::CreateApiKeyRequest,
            api::v1::auth::CreateApiKeyResponse,
            api::v1::auth::ApiKeyResponse,
            crate::schema::AgentSummary,
            api::v1::agents::AgentDetail,
            api::v1::agents::CreateAgentRequest,
            api::v1::agents::CreateAgentTools,
            api::v1::agents::DeleteAgentQuery,
            api::v1::providers::ProviderResponse,
            api::v1::providers::UpsertProviderRequest,
            api::v1::providers::InUse,
            api::v1::providers::usage::UsedBy,
            api::v1::providers::usage::AffectedAgent,
            api::v1::providers::usage::TestResult,
            api::v1::providers::custom::CustomProviderResponse,
            api::v1::providers::custom::CustomProviderBody,
            api::v1::agents::channel::HistoryQuery,
            api::v1::agents::channel::TopicEntry,
            api::v1::agents::ChatRequest,
            api::v1::agents::ChatResponse,
            api::v1::agents::core::UpdateCoreRequest,
            api::v1::agents::core::CoreContentResponse,
            api::v1::agents::core::CoreUpdateResponse,
            api::v1::agents::core::RollbackRequest,
            api::v1::agents::core::CoreHistoryResponse,
            api::v1::agents::memory::MemoryRollbackRequest,
            api::v1::agents::memory::MemoryHistoryResponse,
            crate::schema::RevisionActor,
            crate::schema::RevisionTrigger,
            crate::schema::RevisionDiff,
            crate::schema::DiffHunk,
            crate::schema::DiffLine,
            crate::schema::RollbackResponse,
            crate::schema::CoreRevisionSummary,
            crate::schema::CoreRevision,
            crate::schema::PaginatedCoreRevisions,
            crate::schema::MemoryRevisionSummary,
            crate::schema::MemoryRevision,
            crate::schema::PaginatedMemoryRevisions,
            api::v1::agents::memory::CreateMemoryRequest,
            api::v1::agents::memory::UpdateMemoryRequest,
            api::v1::agents::memory::QueryMemoryRequest,
            api::v1::agents::memory::MemorySummary,
            api::v1::agents::memory::MemoryDetail,
            api::v1::agents::memory::CreateMemoryResponse,
            api::v1::agents::memory::UpdateMemoryResponse,
            api::v1::agents::task::GetTasksQuery,
            api::v1::agents::task::CreateTaskRequest,
            api::v1::agents::task::ScheduleRequest,
            api::v1::agents::task::TaskResponse,
            api::v1::agents::task::RunsQuery,
            api::v1::agents::task::TaskRunResponse,
            api::v1::agents::task::TaskRunsResponse,
            crate::channels::http::models::response::APIResponse<String>,
            crate::channels::http::models::response::APIResponse<api::v1::auth::LoginResponse>,
            crate::channels::http::models::response::APIResponse<api::v1::auth::CreateApiKeyResponse>,
            crate::channels::http::models::response::APIResponse<Vec<api::v1::auth::ApiKeyResponse>>,
            crate::channels::http::models::response::APIResponse<Vec<crate::schema::AgentSummary>>,
            crate::channels::http::models::response::APIResponse<crate::schema::AgentSummary>,
            crate::channels::http::models::response::APIResponse<api::v1::agents::AgentDetail>,
            crate::channels::http::models::response::APIResponse<Vec<api::v1::agents::channel::TopicEntry>>,
            crate::channels::http::models::response::APIResponse<api::v1::agents::ChatResponse>,
            crate::channels::http::models::response::APIResponse<api::v1::agents::core::CoreContentResponse>,
            crate::channels::http::models::response::APIResponse<api::v1::agents::core::CoreUpdateResponse>,
            crate::channels::http::models::response::APIResponse<api::v1::agents::core::CoreHistoryResponse>,
            crate::channels::http::models::response::APIResponse<api::v1::agents::memory::MemoryHistoryResponse>,
            crate::channels::http::models::response::APIResponse<crate::schema::RollbackResponse>,
            crate::channels::http::models::response::APIResponse<Vec<api::v1::agents::memory::MemorySummary>>,
            crate::channels::http::models::response::APIResponse<api::v1::agents::memory::MemoryDetail>,
            crate::channels::http::models::response::APIResponse<Vec<api::v1::agents::memory::MemoryDetail>>,
            crate::channels::http::models::response::APIResponse<api::v1::agents::memory::CreateMemoryResponse>,
            crate::channels::http::models::response::APIResponse<api::v1::agents::memory::UpdateMemoryResponse>,
            crate::channels::http::models::response::APIResponse<api::v1::agents::task::TaskResponse>,
            crate::channels::http::models::response::APIResponse<Vec<api::v1::agents::task::TaskResponse>>,
            crate::channels::http::models::response::APIResponse<api::v1::agents::task::TaskRunsResponse>,
            crate::channels::http::models::response::APIResponse<api::v1::files::UploadResponse>,
            crate::schema::usage::UsageReport,
            crate::schema::usage::UsageRange,
            crate::schema::usage::UsageSummary,
            crate::schema::usage::FailedTotals,
            crate::schema::usage::PeriodTotals,
            crate::schema::usage::DailyUsage,
            crate::schema::usage::DayActivity,
            crate::schema::usage::BreakdownRow,
            crate::schema::usage::ModelRow,
            crate::schema::usage::ConversationRow,
            crate::schema::usage::UnpricedNote,
            crate::schema::usage::UsageActivity,
            crate::schema::usage::ConversationUsage,
            crate::schema::usage::CallRow,
            crate::schema::usage::UsageLink,
            crate::schema::usage::UsageLinkKind,
            crate::schema::usage::NoLinkReason,
            crate::schema::SessionHistory,
            crate::setup::SetupStates,
            crate::setup::SetupState,
            crate::setup::SetupPhase,
            crate::setup::DownloadStatus,
            crate::setup::DownloadKind,
            crate::channels::http::models::response::APIResponse<crate::setup::SetupStates>,
        )
    ),
    info(
        title = "Vizier API",
        version = "1.0.0",
        description = "21st Century Digital Steward API"
    )
)]
struct ApiDoc;
