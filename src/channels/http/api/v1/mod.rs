use axum::{
    Router, middleware,
    routing::{delete, get, post, put},
};
use reqwest::StatusCode;

use crate::channels::http::{
    api::v1::agents::agents,
    auth::middleware::{PermissionState, require_auth, require_permission},
    models::{self, response::{api_response, APIResponse}},
    state::HTTPState,
};

pub mod agents;
pub mod auth;
pub mod catalogue;
pub mod embedding;
pub mod files;
pub mod integrations;
pub mod overview;
pub mod providers;
pub mod skill_support;
pub mod skills;
pub mod utils;

pub fn v1(state: HTTPState) -> Router<HTTPState> {
    Router::new()
        .route("/ping", get(ping))
        .route("/status", get(status))
        // Auth routes (some public, some protected)
        .route("/auth/login", post(auth::login))
        .route(
            "/auth/users/me",
            get(auth::get_current_user)
                .layer(middleware::from_fn_with_state(state.clone(), require_auth)),
        )
        .route(
            "/auth/users/me/profile",
            get(auth::get_my_profile)
                .layer(middleware::from_fn_with_state(state.clone(), require_auth)),
        )
        .route(
            "/auth/users/me/profile",
            put(auth::update_my_profile)
                .layer(middleware::from_fn_with_state(state.clone(), require_auth)),
        )
        // Setup routes (public - only works when no users exist)
        .route("/auth/setup-status", get(auth::setup_status))
        .route("/auth/setup", post(auth::setup_first_user))
        .route(
            "/auth/change-password",
            post(auth::change_password)
                .layer(middleware::from_fn_with_state(PermissionState { permission: "settings:password".to_string() }, require_permission))
                .layer(middleware::from_fn_with_state(state.clone(), require_auth)),
        )
        .route(
            "/auth/api-keys",
            post(auth::create_api_key)
                .layer(middleware::from_fn_with_state(PermissionState { permission: "settings:api_keys".to_string() }, require_permission))
                .layer(middleware::from_fn_with_state(state.clone(), require_auth)),
        )
        .route(
            "/auth/api-keys",
            get(auth::list_api_keys)
                .layer(middleware::from_fn_with_state(PermissionState { permission: "settings:api_keys".to_string() }, require_permission))
                .layer(middleware::from_fn_with_state(state.clone(), require_auth)),
        )
        .route(
            "/auth/api-keys/{key_id}",
            delete(auth::delete_api_key)
                .layer(middleware::from_fn_with_state(PermissionState { permission: "settings:api_keys".to_string() }, require_permission))
                .layer(middleware::from_fn_with_state(state.clone(), require_auth)),
        )
        // Role management routes (require roles:manage permission)
        .route(
            "/auth/roles",
            get(auth::list_roles)
                .layer(middleware::from_fn_with_state(PermissionState { permission: "roles:manage".to_string() }, require_permission))
                .layer(middleware::from_fn_with_state(state.clone(), require_auth)),
        )
        .route(
            "/auth/roles",
            post(auth::create_role)
                .layer(middleware::from_fn_with_state(PermissionState { permission: "roles:manage".to_string() }, require_permission))
                .layer(middleware::from_fn_with_state(state.clone(), require_auth)),
        )
        .route(
            "/auth/roles/available-permissions",
            get(auth::available_permissions)
                .layer(middleware::from_fn_with_state(PermissionState { permission: "roles:manage".to_string() }, require_permission))
                .layer(middleware::from_fn_with_state(state.clone(), require_auth)),
        )
        .route(
            "/auth/roles/{role_id}",
            put(auth::update_role)
                .layer(middleware::from_fn_with_state(PermissionState { permission: "roles:manage".to_string() }, require_permission))
                .layer(middleware::from_fn_with_state(state.clone(), require_auth)),
        )
        .route(
            "/auth/roles/{role_id}",
            delete(auth::delete_role)
                .layer(middleware::from_fn_with_state(PermissionState { permission: "roles:manage".to_string() }, require_permission))
                .layer(middleware::from_fn_with_state(state.clone(), require_auth)),
        )
        // User management routes (require users:manage permission)
        .route(
            "/auth/users",
            get(auth::list_users)
                .layer(middleware::from_fn_with_state(PermissionState { permission: "users:manage".to_string() }, require_permission))
                .layer(middleware::from_fn_with_state(state.clone(), require_auth)),
        )
        .route(
            "/auth/users",
            post(auth::create_user)
                .layer(middleware::from_fn_with_state(PermissionState { permission: "users:manage".to_string() }, require_permission))
                .layer(middleware::from_fn_with_state(state.clone(), require_auth)),
        )
        .route(
            "/auth/users/{user_id}",
            put(auth::update_user)
                .layer(middleware::from_fn_with_state(PermissionState { permission: "users:manage".to_string() }, require_permission))
                .layer(middleware::from_fn_with_state(state.clone(), require_auth)),
        )
        .route(
            "/auth/users/{user_id}",
            delete(auth::delete_user)
                .layer(middleware::from_fn_with_state(PermissionState { permission: "users:manage".to_string() }, require_permission))
                .layer(middleware::from_fn_with_state(state.clone(), require_auth)),
        )
        // Files routes
        .route(
            "/files/upload",
            post(files::upload_file)
                .layer(middleware::from_fn_with_state(state.clone(), require_auth)),
        )
        .route("/files/{file_id}", get(files::download_file))
        // The catalogue: readable by any signed-in user; logos are public.
        .nest("/catalogue", catalogue::catalogue(state.clone()))
        // Protected routes
        .nest(
            "/agents",
            agents().layer(middleware::from_fn_with_state(state.clone(), require_auth)),
        )
        .nest("/integrations", integrations::integrations(state.clone()))
        .nest(
            "/overview",
            overview::overview().layer(middleware::from_fn_with_state(state.clone(), require_auth)),
        )
        .nest(
            "/providers",
            providers::providers()
                .layer(middleware::from_fn_with_state(PermissionState { permission: "settings:providers".to_string() }, require_permission))
                .layer(middleware::from_fn_with_state(state.clone(), require_auth)),
        )
        .nest(
            "/skills",
            skills::skills().layer(middleware::from_fn_with_state(state.clone(), require_auth)),
        )
        .nest(
            "/embedding-models",
            embedding::embedding().layer(middleware::from_fn_with_state(state.clone(), require_auth)),
        )
        .nest(
            "/utils",
            utils::utils().layer(middleware::from_fn_with_state(state.clone(), require_auth)),
        )
}

/// What the sign-in page needs before anyone has signed in.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct ServerStatus {
    pub online: bool,
    pub version: &'static str,
    /// No one has been set up yet: show Onboarding.
    pub setup_required: bool,
}

#[utoipa::path(
    get,
    path = "/status",
    responses(
        (status = 200, description = "Public. Whether vizier is up, its version, and whether first-run setup is still needed", body = APIResponse<ServerStatus>)
    )
)]
async fn status(
    axum::extract::State(state): axum::extract::State<HTTPState>,
) -> models::response::Response<ServerStatus> {
    use crate::storage::user::UserStorage;
    // A storage that can't answer is not "set up": say so rather than sending someone to Onboarding.
    match state.storage.user_exists().await {
        Ok(exists) => api_response(
            StatusCode::OK,
            ServerStatus { online: true, version: env!("CARGO_PKG_VERSION"), setup_required: !exists },
        ),
        Err(e) => models::response::err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
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
