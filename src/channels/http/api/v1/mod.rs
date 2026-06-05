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
pub mod files;
pub mod providers;
pub mod skills;

pub fn v1(state: HTTPState) -> Router<HTTPState> {
    Router::new()
        .route("/ping", get(ping))
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
        // Protected routes
        .nest(
            "/agents",
            agents().layer(middleware::from_fn_with_state(state.clone(), require_auth)),
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
