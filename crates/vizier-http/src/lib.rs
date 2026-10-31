#![allow(unused)]

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

use crate::state::HTTPState;
use vizier_core::{config::HTTPChannelConfig, error::VizierError, runtime::VizierRuntime};

pub mod models;

pub mod api;
pub mod auth;
pub mod embedded;
pub mod error;
mod state;
pub mod topic;
#[cfg(test)]
mod test_support;
mod webui;

/// The whole HTTP surface (REST API, WebSocket, OpenAPI and Swagger UI, the embedded WebUI)
/// over a running engine, for a host that serves it itself.
pub fn router(runtime: &VizierRuntime, config: &HTTPChannelConfig) -> Router {
    let state = new_state(runtime, state::AuthMode::Network);
    let cors = cors_layer(config.cors_origins.as_deref());

    let app = Router::new()
        .nest("/api", api::api(state.clone()))
        // webui
        .route("/", get(webui::index))
        .route("/{*path}", get(webui::assets))
        .layer(NormalizePathLayer::trim_trailing_slash())
        .layer(cors)
        .layer(DefaultBodyLimit::disable())
        .with_state(state);

    app.merge(SwaggerUi::new("/swagger").url("/openapi.json", ApiDoc::openapi()))
}

pub(crate) fn new_state(runtime: &VizierRuntime, auth: state::AuthMode) -> HTTPState {
    let deps = runtime.deps().clone();
    HTTPState {
        config: deps.config.clone(),
        storage: deps.storage.clone(),
        transport: deps.transport.clone(),
        auth,
        background_jobs: deps.background_jobs.clone(),
        catalogue: deps.catalogue.clone(),
        deps,
        client_events: std::sync::Arc::new(vizier_core::utils::rate_limit::RateLimiter::new(
            10,
            std::time::Duration::from_secs(60),
        )),
        client_state_saves: std::sync::Arc::new(vizier_core::utils::rate_limit::RateLimiter::new(
            2,
            std::time::Duration::from_secs(1),
        )),
        visit_throttle: std::sync::Arc::new(vizier_core::utils::rate_limit::RateLimiter::new(
            1,
            std::time::Duration::from_secs(60),
        )),
        deferred_client_calls: Default::default(),
    }
}

fn cors_layer(origins: Option<&[String]>) -> CorsLayer {
    let layer = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([AUTHORIZATION, ACCEPT, CONTENT_TYPE]);
    match origins {
        None => layer.allow_origin(Any),
        Some(list) => layer.allow_origin(tower_http::cors::AllowOrigin::list(
            list.iter().filter_map(|o| o.parse().ok()),
        )),
    }
}

/// Binds `0.0.0.0:{config.port}` and serves [`router`]. The future owns what it needs, so it
/// can be spawned; it resolves only if the listener can't be bound or the server stops.
pub fn serve(
    runtime: &VizierRuntime,
    config: HTTPChannelConfig,
) -> impl std::future::Future<Output = vizier_core::Result<()>> + Send + 'static {
    let app = router(runtime, &config);
    async move {
        let listener = tokio::net::TcpListener::bind(format!(
            "{}:{}",
            config.host.as_deref().unwrap_or("0.0.0.0"),
            config.port
        ))
            .await
            .map_err(|e| VizierError(e.to_string()))?;

        let server = axum::serve(listener, app);
        tracing::info!("http listening on port {}", config.port);

        server.await.map_err(|e| VizierError(e.to_string()))
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        api::v1::ping,
        api::v1::status,
        api::v1::overview::overview_counts,
        api::v1::overview::activity,
        api::v1::agents::conversations::list,
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
        api::v1::agents::restart_agent,
        api::v1::agents::dream::dream_nights,
        api::v1::agents::dream::dream_cycles,
        api::v1::agents::memory::list_bundles,
        api::v1::agents::agent_usage,
        api::v1::agents::agent_usage_conversation,
        api::v1::agents::channel::list_topics,
        api::v1::agents::channel::get_topic_history,
        api::v1::agents::channel::list_topic_files,
        api::v1::agents::channel::promote_topic_file,
        api::v1::agents::channel::delete_topic,
        api::v1::agents::channel::rename_topic,
        api::v1::agents::artifacts::list_artifacts,
        api::v1::agents::artifacts::activity,
        api::v1::agents::artifacts::artifact_detail,
        api::v1::agents::artifacts::artifact_content,
        api::v1::agents::artifacts::delete_artifact,
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
        api::v1::agents::agent_btw,
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
        api::v1::agents::memory::attach_artifact,
        api::v1::agents::memory::detach_attachment,
        api::v1::agents::secrets::list,
        api::v1::agents::secrets::events,
        api::v1::agents::secrets::set,
        api::v1::agents::secrets::remove,
        api::v1::agents::projects::list,
        api::v1::agents::projects::open,
        api::v1::agents::projects::detail,
        api::v1::agents::projects::update,
        api::v1::agents::projects::remove,
        api::v1::agents::projects::sessions,
        api::v1::agents::projects::create_session,
        api::v1::agents::projects::set_mode,
        api::v1::agents::projects::files,
        api::v1::agents::projects::git_summary,
        api::v1::agents::projects::git_changes,
        api::v1::agents::projects::git_diff,
        api::v1::agents::projects::git_branches,
        api::v1::agents::projects::git_switch,
        api::v1::agents::projects::list_worktrees,
        api::v1::agents::projects::skills,
        api::v1::agents::projects::create_worktree,
        api::v1::agents::projects::worktree_detail,
        api::v1::agents::projects::delete_worktree,
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
        api::v1::catalogue::media_models,
        api::v1::catalogue::refresh_media,
        api::v1::utils::setup_state,
        api::v1::notifications::list,
        api::v1::notifications::count,
        api::v1::notifications::set_read,
        api::v1::notifications::read_all,
    ),
    components(
        schemas(
            crate::models::page::SortOrder,
            vizier_core::catalogue::media::MediaCapability,
            vizier_core::catalogue::media::MediaModelSummary,
            api::v1::catalogue::MediaModels,
            api::v1::ServerStatus,
            api::v1::overview::Overview,
            api::v1::overview::Rank,
            api::v1::overview::ActivityRow,
            api::v1::overview::TodayByKind,
            api::v1::overview::AgentCounts,
            api::v1::agents::conversations::ConversationKind,
            api::v1::agents::conversations::ConversationItem,
            api::v1::agents::conversations::ConversationCounts,
            api::v1::overview::Activity,
            api::v1::agents::conversations::ConversationPage,
            api::v1::auth::RoleItem,
            api::v1::agents::AgentListItem,
            api::v1::agents::dream::DreamCycles,
            api::v1::auth::LoginRequest,
            api::v1::auth::LoginResponse,
            api::v1::auth::ChangePasswordRequest,
            api::v1::auth::CreateApiKeyRequest,
            api::v1::auth::CreateApiKeyResponse,
            api::v1::auth::ApiKeyResponse,
            vizier_core::schema::AgentSummary,
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
            api::v1::agents::channel::RenameTopicRequest,
            api::v1::agents::ChatRequest,
            api::v1::agents::ChatResponse,
            api::v1::agents::BtwRequest,
            api::v1::agents::BtwResponse,
            api::v1::agents::core::UpdateCoreRequest,
            api::v1::agents::core::CoreContentResponse,
            api::v1::agents::core::CoreUpdateResponse,
            api::v1::agents::core::RollbackRequest,
            api::v1::agents::core::CoreHistoryResponse,
            api::v1::agents::memory::MemoryRollbackRequest,
            api::v1::agents::memory::MemoryHistoryResponse,
            vizier_core::schema::RevisionActor,
            vizier_core::schema::RevisionTrigger,
            vizier_core::schema::RevisionDiff,
            vizier_core::schema::DiffHunk,
            vizier_core::schema::DiffLine,
            vizier_core::schema::RollbackResponse,
            vizier_core::schema::CoreRevisionSummary,
            vizier_core::schema::CoreRevision,
            vizier_core::schema::PaginatedCoreRevisions,
            vizier_core::schema::MemoryRevisionSummary,
            vizier_core::schema::MemoryRevision,
            vizier_core::schema::PaginatedMemoryRevisions,
            api::v1::agents::memory::CreateMemoryRequest,
            api::v1::agents::memory::UpdateMemoryRequest,
            api::v1::agents::memory::AttachArtifactRequest,
            api::v1::agents::memory::QueryMemoryRequest,
            api::v1::agents::memory::MemorySummary,
            api::v1::agents::memory::MemoryDetail,
            vizier_core::schema::MemoryAttachmentView,
            vizier_core::schema::Artifact,
            vizier_core::schema::ArtifactKind,
            vizier_core::schema::ArtifactOrigin,
            vizier_core::schema::ArtifactEventAction,
            vizier_core::schema::ArtifactDeleteResult,
            api::v1::agents::artifacts::ArtifactItemView,
            api::v1::agents::artifacts::ArtifactListView,
            api::v1::agents::artifacts::ArtifactDetailView,
            api::v1::agents::artifacts::ArtifactActivityView,
            api::v1::agents::artifacts::ArtifactEventView,
            api::v1::agents::artifacts::ArtifactReferenceView,
            api::v1::agents::channel::PromoteFileRequest,
            api::v1::agents::memory::CreateMemoryResponse,
            api::v1::agents::memory::UpdateMemoryResponse,
            api::v1::agents::secrets::SecretItem,
            api::v1::agents::secrets::SecretList,
            api::v1::agents::secrets::SecretWriteResponse,
            api::v1::agents::secrets::SetSecretRequest,
            api::v1::agents::secrets::SecretDeleted,
            api::v1::agents::secrets::SecretEventItem,
            api::v1::agents::secrets::SecretEventList,
            api::v1::agents::projects::ProjectSummary,
            api::v1::agents::projects::ProjectDetail,
            api::v1::agents::projects::OpenProjectRequest,
            api::v1::agents::projects::UpdateProjectRequest,
            api::v1::agents::projects::ProjectSession,
            api::v1::agents::projects::CreateProjectSessionResponse,
            api::v1::agents::projects::ProjectRemoved,
            api::v1::agents::projects::SetModeRequest,
            api::v1::agents::projects::SetModeResponse,
            api::v1::agents::projects::FileSuggestion,
            api::v1::agents::projects::ProjectSkillRow,
            api::v1::agents::projects::SwitchBranchRequest,
            api::v1::agents::projects::WorktreeSummary,
            api::v1::agents::projects::WorktreeDetail,
            api::v1::agents::projects::CreateWorktreeRequest,
            api::v1::agents::projects::CreateWorktreeResponse,
            api::v1::agents::projects::WorktreeDeleted,
            vizier_core::projects::git::GitSummary,
            vizier_core::projects::git::GitChange,
            vizier_core::projects::git::GitChangeKind,
            vizier_core::projects::git::GitFileDiff,
            vizier_core::projects::git::GitBranch,
            vizier_core::projects::git::BranchHolder,
            crate::models::response::APIResponse<Vec<api::v1::agents::projects::ProjectSummary>>,
            crate::models::response::APIResponse<api::v1::agents::projects::ProjectSummary>,
            crate::models::response::APIResponse<api::v1::agents::projects::ProjectDetail>,
            crate::models::response::APIResponse<Vec<api::v1::agents::projects::ProjectSession>>,
            crate::models::response::APIResponse<api::v1::agents::projects::CreateProjectSessionResponse>,
            crate::models::response::APIResponse<api::v1::agents::projects::ProjectRemoved>,
            crate::models::response::APIResponse<api::v1::agents::projects::SetModeResponse>,
            crate::models::response::APIResponse<Vec<api::v1::agents::projects::FileSuggestion>>,
            crate::models::response::APIResponse<Vec<api::v1::agents::projects::ProjectSkillRow>>,
            crate::models::response::APIResponse<vizier_core::projects::git::GitSummary>,
            crate::models::response::APIResponse<Vec<vizier_core::projects::git::GitChange>>,
            crate::models::response::APIResponse<vizier_core::projects::git::GitFileDiff>,
            crate::models::response::APIResponse<Vec<vizier_core::projects::git::GitBranch>>,
            crate::models::response::APIResponse<Vec<api::v1::agents::projects::WorktreeSummary>>,
            crate::models::response::APIResponse<api::v1::agents::projects::CreateWorktreeResponse>,
            crate::models::response::APIResponse<api::v1::agents::projects::WorktreeDetail>,
            crate::models::response::APIResponse<api::v1::agents::projects::WorktreeDeleted>,
            api::v1::agents::task::GetTasksQuery,
            api::v1::agents::task::CreateTaskRequest,
            api::v1::agents::task::ScheduleRequest,
            api::v1::agents::task::TaskResponse,
            api::v1::agents::task::RunsQuery,
            api::v1::agents::task::TaskRunResponse,
            api::v1::agents::task::TaskRunsResponse,
            crate::models::response::APIResponse<String>,
            crate::models::response::APIResponse<api::v1::auth::LoginResponse>,
            crate::models::response::APIResponse<api::v1::auth::CreateApiKeyResponse>,
            crate::models::response::APIResponse<Vec<api::v1::auth::ApiKeyResponse>>,
            crate::models::response::APIResponse<Vec<vizier_core::schema::AgentSummary>>,
            crate::models::response::APIResponse<vizier_core::schema::AgentSummary>,
            crate::models::response::APIResponse<api::v1::agents::AgentDetail>,
            crate::models::response::APIResponse<Vec<api::v1::agents::channel::TopicEntry>>,
            crate::models::response::APIResponse<api::v1::agents::ChatResponse>,
            crate::models::response::APIResponse<api::v1::agents::core::CoreContentResponse>,
            crate::models::response::APIResponse<api::v1::agents::core::CoreUpdateResponse>,
            crate::models::response::APIResponse<api::v1::agents::core::CoreHistoryResponse>,
            crate::models::response::APIResponse<api::v1::agents::memory::MemoryHistoryResponse>,
            crate::models::response::APIResponse<vizier_core::schema::RollbackResponse>,
            crate::models::response::APIResponse<Vec<api::v1::agents::memory::MemorySummary>>,
            crate::models::response::APIResponse<api::v1::agents::memory::MemoryDetail>,
            crate::models::response::APIResponse<Vec<api::v1::agents::memory::MemoryDetail>>,
            crate::models::response::APIResponse<api::v1::agents::memory::CreateMemoryResponse>,
            crate::models::response::APIResponse<api::v1::agents::memory::UpdateMemoryResponse>,
            crate::models::response::APIResponse<api::v1::agents::task::TaskResponse>,
            crate::models::response::APIResponse<Vec<api::v1::agents::task::TaskResponse>>,
            crate::models::response::APIResponse<api::v1::agents::task::TaskRunsResponse>,
            crate::models::response::APIResponse<api::v1::files::UploadResponse>,
            vizier_core::schema::usage::UsageReport,
            vizier_core::schema::usage::UsageRange,
            vizier_core::schema::usage::UsageSummary,
            vizier_core::schema::usage::FailedTotals,
            vizier_core::schema::usage::PeriodTotals,
            vizier_core::schema::usage::DailyUsage,
            vizier_core::schema::usage::DayActivity,
            vizier_core::schema::usage::BreakdownRow,
            vizier_core::schema::usage::ModelRow,
            vizier_core::schema::usage::ConversationRow,
            vizier_core::schema::usage::UnpricedNote,
            vizier_core::schema::usage::UsageActivity,
            vizier_core::schema::usage::ConversationUsage,
            vizier_core::schema::usage::CallRow,
            vizier_core::schema::usage::UsageLink,
            vizier_core::schema::usage::UsageLinkKind,
            vizier_core::schema::usage::NoLinkReason,
            vizier_core::schema::SessionHistory,
            vizier_core::setup::SetupStates,
            vizier_core::setup::SetupState,
            vizier_core::setup::SetupPhase,
            vizier_core::setup::DownloadStatus,
            vizier_core::setup::DownloadKind,
            vizier_core::schema::NotificationCategory,
            vizier_core::schema::NotificationTone,
            vizier_core::schema::NotificationFilter,
            vizier_core::schema::NotificationItem,
            vizier_core::schema::NotificationCounts,
            vizier_core::schema::NotificationPage,
            api::v1::notifications::NotificationReadRequest,
            crate::models::response::APIResponse<vizier_core::schema::NotificationPage>,
            crate::models::response::APIResponse<vizier_core::schema::NotificationCounts>,
            crate::models::response::APIResponse<vizier_core::setup::SetupStates>,
        )
    ),
    info(
        title = "Vizier API",
        version = "1.0.0",
        description = "21st Century Digital Steward API"
    )
)]
struct ApiDoc;
