use anyhow::Result;
use axum::{
    Router,
    routing::{any, delete, get, post},
};
use reqwest::{
    Method,
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
};
use tower_http::cors::{Any, CorsLayer};

use crate::{
    channels::{
        VizierChannel,
        http::state::{ChatTransport, HTTPState},
    },
    config::HTTPChannelConfig,
    dependencies::VizierDependencies,
};

pub mod models;

mod api;
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
        let chat_transport = ChatTransport::new();

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

        let app = Router::new()
            // webui
            .route("/", get(webui::index))
            .route("/{*path}", get(webui::assets))
            // api
            .route("/api/v1/ping", get(api::v1::ping))
            // session api
            .route("/api/v1/session", get(api::v1::session::list_sessions))
            .route("/api/v1/session", post(api::v1::session::create_session))
            .route(
                "/api/v1/session/{session_id}",
                post(api::v1::session::create_custom_session),
            )
            .route(
                "/api/v1/session/{session_id}",
                delete(api::v1::session::delete_sessions),
            )
            .route(
                "/api/v1/session/{session_id}/chat",
                any(api::v1::session::chat),
            )
            // memory api
            .route("/api/v1/memory", get(api::v1::memory::get_all_memories))
            .route(
                "/api/v1/memory/{slug}",
                get(api::v1::memory::get_memory_detail),
            )
            .route(
                "/api/v1/memory/{slug}",
                delete(api::v1::memory::delete_memory),
            )
            .layer(cors)
            .with_state(HTTPState {
                db: self.deps.database.clone(),
                transport: chat_transport.clone(),
            });

        let listener =
            tokio::net::TcpListener::bind(format!("0.0.0.0:{}", self.config.port)).await?;

        let transport = self.deps.transport.clone();
        let transport_handle = tokio::spawn(async move {
            let _ = chat_transport.run(transport).await;
        });

        let server = axum::serve(listener, app);
        log::info!("http listening on port {}", self.config.port);

        server.await?;
        transport_handle.abort();

        Ok(())
    }
}
