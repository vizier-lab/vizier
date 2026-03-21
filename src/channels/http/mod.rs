use anyhow::Result;
use axum::{Router, routing::get};
use reqwest::{
    Method,
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
};
use tower_http::cors::{Any, CorsLayer};

use crate::{
    channels::{VizierChannel, http::state::HTTPState},
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
            .nest("/api", api::api())
            // webui
            .route("/", get(webui::index))
            .route("/{*path}", get(webui::assets))
            .layer(cors)
            .with_state(HTTPState {
                config: self.deps.config.clone(),
                storage: self.deps.storage.clone(),
                transport: self.deps.transport.clone(),
            });

        let listener =
            tokio::net::TcpListener::bind(format!("0.0.0.0:{}", self.config.port)).await?;

        let server = axum::serve(listener, app);
        log::info!("http listening on port {}", self.config.port);

        server.await?;

        Ok(())
    }
}
