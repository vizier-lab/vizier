use axum::{
    Router, middleware,
    routing::{delete, get, post},
};
use reqwest::StatusCode;

use crate::channels::http::{
    api::v1::agents::agents,
    auth::middleware::require_auth,
    models::{self, response::{api_response, APIResponse}},
    state::HTTPState,
};

pub mod agents;
pub mod auth;
pub mod files;

pub fn v1(state: HTTPState) -> Router<HTTPState> {
    Router::new()
        .route("/ping", get(ping))
        // Auth routes (some public, some protected)
        .route("/auth/login", post(auth::login))
        .route(
            "/auth/change-password",
            post(auth::change_password)
                .layer(middleware::from_fn_with_state(state.clone(), require_auth)),
        )
        .route(
            "/auth/api-keys",
            post(auth::create_api_key)
                .layer(middleware::from_fn_with_state(state.clone(), require_auth)),
        )
        .route(
            "/auth/api-keys",
            get(auth::list_api_keys)
                .layer(middleware::from_fn_with_state(state.clone(), require_auth)),
        )
        .route(
            "/auth/api-keys/{key_id}",
            delete(auth::delete_api_key)
                .layer(middleware::from_fn_with_state(state.clone(), require_auth)),
        )
        // Files routes
        .route(
            "/files/upload",
            post(files::upload_file)
                .layer(middleware::from_fn_with_state(state.clone(), require_auth)),
        )
        .route("/files/{file_id}", get(files::download_file))
        // Protected routes
        .nest(
            "/agents",
            agents().layer(middleware::from_fn_with_state(state.clone(), require_auth)),
        )
}

#[utoipa::path(
    get,
    path = "/ping",
    responses(
        (status = 200, description = "pong", body = APIResponse<String>)
    )
)]
async fn ping() -> models::response::Response<String> {
    api_response(StatusCode::OK, "pong".into())
}
