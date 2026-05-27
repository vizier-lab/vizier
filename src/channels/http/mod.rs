use anyhow::Result;
use axum::{Router, routing::get};
use tower_http::limit::RequestBodyLimitLayer;
use reqwest::{
    Method,
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
};
use tower_http::cors::{Any, CorsLayer};
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

impl VizierChannel for HTTPChannel {
    async fn run(&mut self) -> Result<()> {
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
        };

        let mut app = Router::new()
            .nest("/api", api::api(state.clone()))
            // webui
            .route("/", get(webui::index))
            .route("/{*path}", get(webui::assets))
            .layer(cors)
            .layer(RequestBodyLimitLayer::new(10 * 1024 * 1024))
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
        api::v1::agents::agent_detail,
        api::v1::agents::agent_usage,
        api::v1::agents::channel::list_topics,
        api::v1::agents::channel::get_topic_history,
        api::v1::agents::channel::delete_topic,
        api::v1::agents::channel::chat,
        api::v1::agents::documents::get_agent_doc,
        api::v1::agents::documents::update_agent_doc,
        api::v1::agents::documents::get_identity_doc,
        api::v1::agents::documents::update_identity_doc,
        api::v1::agents::documents::get_heartbeat_doc,
        api::v1::agents::documents::update_heartbeat_doc,
        api::v1::agents::memory::get_all_memories,
        api::v1::agents::memory::create_memory,
        api::v1::agents::memory::get_memory_detail,
        api::v1::agents::memory::update_memory,
        api::v1::agents::memory::delete_memory,
        api::v1::agents::memory::query_memories,
        api::v1::agents::task::get_tasks,
        api::v1::agents::task::get_task,
        api::v1::agents::task::create_task,
        api::v1::agents::task::update_task,
        api::v1::agents::task::delete_task,
        api::v1::files::upload_file,
        api::v1::files::download_file,
    ),
    components(
        schemas(
            api::v1::auth::LoginRequest,
            api::v1::auth::LoginResponse,
            api::v1::auth::ChangePasswordRequest,
            api::v1::auth::CreateApiKeyRequest,
            api::v1::auth::CreateApiKeyResponse,
            api::v1::auth::ApiKeyResponse,
            api::v1::agents::AgentSummary,
            api::v1::agents::UsageQuery,
            api::v1::agents::channel::HistoryQuery,
            api::v1::agents::channel::TopicEntry,
            api::v1::agents::documents::UpdateDocumentRequest,
            api::v1::agents::documents::DocumentContentResponse,
            api::v1::agents::documents::DocumentUpdateResponse,
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
            crate::channels::http::models::response::APIResponse<String>,
            crate::channels::http::models::response::APIResponse<api::v1::auth::LoginResponse>,
            crate::channels::http::models::response::APIResponse<api::v1::auth::CreateApiKeyResponse>,
            crate::channels::http::models::response::APIResponse<Vec<api::v1::auth::ApiKeyResponse>>,
            crate::channels::http::models::response::APIResponse<Vec<api::v1::agents::AgentSummary>>,
            crate::channels::http::models::response::APIResponse<api::v1::agents::AgentSummary>,
            crate::channels::http::models::response::APIResponse<Vec<api::v1::agents::channel::TopicEntry>>,
            crate::channels::http::models::response::APIResponse<api::v1::agents::documents::DocumentContentResponse>,
            crate::channels::http::models::response::APIResponse<api::v1::agents::documents::DocumentUpdateResponse>,
            crate::channels::http::models::response::APIResponse<Vec<api::v1::agents::memory::MemorySummary>>,
            crate::channels::http::models::response::APIResponse<api::v1::agents::memory::MemoryDetail>,
            crate::channels::http::models::response::APIResponse<Vec<api::v1::agents::memory::MemoryDetail>>,
            crate::channels::http::models::response::APIResponse<api::v1::agents::memory::CreateMemoryResponse>,
            crate::channels::http::models::response::APIResponse<api::v1::agents::memory::UpdateMemoryResponse>,
            crate::channels::http::models::response::APIResponse<api::v1::agents::task::TaskResponse>,
            crate::channels::http::models::response::APIResponse<Vec<api::v1::agents::task::TaskResponse>>,
            crate::channels::http::models::response::APIResponse<api::v1::files::UploadResponse>,
            crate::schema::AgentUsageStats,
            crate::schema::SessionHistory,
        )
    ),
    info(
        title = "Vizier API",
        version = "1.0.0",
        description = "21st Century Digital Steward API"
    )
)]
struct ApiDoc;
