//! The signed-in person's connected Discord and Telegram accounts
//! (`specs/069-platform-account-linking/contracts/http-connections.md`). Authorization and storage
//! are in `vizier_core::service::platform_links`; these handlers translate and rate limit.
//!
//! No handler logs a request body: it carries a connect code.

use std::net::SocketAddr;

use axum::{
    Extension, Json,
    extract::{ConnectInfo, Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use vizier_core::{
    auth::Principal,
    schema::ChannelPlatform,
    service::{
        ServiceError,
        platform_links::{self as service, ConnectCodeRequest, ConnectPreview, ConnectResult, Connections},
    },
};

use crate::{
    api::v1::{auth::services, integrations::ok, shares::client_address},
    error::ApiError,
    models::response::APIResponse,
    state::HTTPState,
};

fn refused(e: ServiceError) -> Response {
    ApiError(e).into_response()
}

/// Count one attempt at a code for this person and this address (FR-008). The answer to give
/// instead, `429` with `Retry-After`, when either has used its budget.
fn over_limit(
    state: &HTTPState,
    user: &Principal,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    headers: &HeaderMap,
) -> Option<Response> {
    let address = client_address(
        peer.map(|Extension(ConnectInfo(a))| a.ip()),
        headers,
        &state.config.trusted_proxies,
    );
    for (limiter, key) in [
        (&state.connect_attempts_user, user.user_id.as_str()),
        (&state.connect_attempts_addr, address.as_str()),
    ] {
        if !limiter.check(key) {
            let wait = limiter.retry_after(key).map_or(60, |d| d.as_secs().max(1));
            let mut response = refused(ServiceError::Invalid {
                status: StatusCode::TOO_MANY_REQUESTS.as_u16(),
                message: "Too many attempts. Try again later.".into(),
                code: Some("rate_limited".into()),
            });
            if let Ok(value) = HeaderValue::from_str(&wait.to_string()) {
                response.headers_mut().insert(header::RETRY_AFTER, value);
            }
            return Some(response);
        }
    }
    None
}

/// `discord` or `telegram`.
fn platform(name: &str) -> Option<ChannelPlatform> {
    match name {
        "discord" => Some(ChannelPlatform::Discord),
        "telegram" => Some(ChannelPlatform::Telegram),
        _ => None,
    }
}

/// Any other platform has nothing connected.
fn no_such_platform() -> Response {
    refused(ServiceError::Invalid {
        status: StatusCode::NOT_FOUND.as_u16(),
        message: "No such platform.".into(),
        code: Some("not_linked".into()),
    })
}

#[utoipa::path(
    get,
    path = "/auth/users/me/connections",
    responses((status = 200, description = "Connected accounts, unverified hand-typed ones and recent changes", body = APIResponse<Connections>))
)]
pub async fn list(State(state): State<HTTPState>, Extension(user): Extension<Principal>) -> Response {
    match service::list(&services(&state), &user).await {
        Ok(connections) => ok(StatusCode::OK, connections),
        Err(e) => refused(e),
    }
}

#[utoipa::path(
    post,
    path = "/auth/users/me/connections/preview",
    request_body = ConnectCodeRequest,
    responses(
        (status = 200, description = "What confirming the code would do. Consumes nothing", body = APIResponse<ConnectPreview>),
        (status = 404, description = "`connect_code_unavailable`: unknown, expired or used"),
        (status = 409, description = "`platform_linked_elsewhere`"),
        (status = 429, description = "`rate_limited`, with `Retry-After`")
    )
)]
pub async fn preview(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    headers: HeaderMap,
    Json(request): Json<ConnectCodeRequest>,
) -> Response {
    if let Some(response) = over_limit(&state, &user, peer, &headers) {
        return response;
    }
    match service::preview(&services(&state), &user, &request.code).await {
        Ok(preview) => ok(StatusCode::OK, preview),
        Err(e) => refused(e),
    }
}

#[utoipa::path(
    post,
    path = "/auth/users/me/connections",
    request_body = ConnectCodeRequest,
    responses(
        (status = 200, description = "Connected; `replaced` is the account that was unlinked", body = APIResponse<ConnectResult>),
        (status = 404, description = "`connect_code_unavailable`: unknown, expired or used"),
        (status = 409, description = "`platform_linked_elsewhere`"),
        (status = 429, description = "`rate_limited`, with `Retry-After`")
    )
)]
pub async fn redeem(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    headers: HeaderMap,
    Json(request): Json<ConnectCodeRequest>,
) -> Response {
    if let Some(response) = over_limit(&state, &user, peer, &headers) {
        return response;
    }
    match service::redeem(&services(&state), &user, &request.code).await {
        Ok(result) => ok(StatusCode::OK, result),
        Err(e) => refused(e),
    }
}

#[utoipa::path(
    delete,
    path = "/auth/users/me/connections/{platform}",
    params(("platform" = String, Path, description = "`discord` or `telegram`")),
    responses(
        (status = 204, description = "Unlinked"),
        (status = 404, description = "`not_linked`")
    )
)]
pub async fn unlink(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(name): Path<String>,
) -> Response {
    let Some(platform) = platform(&name) else {
        return no_such_platform();
    };
    match service::unlink(&services(&state), &user, platform).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => refused(e),
    }
}

#[utoipa::path(
    delete,
    path = "/auth/users/me/connections/{platform}/unverified",
    params(("platform" = String, Path, description = "`discord` or `telegram`")),
    responses((status = 204, description = "The hand-typed values for that platform are gone"))
)]
pub async fn clear_unverified(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(name): Path<String>,
) -> Response {
    let Some(platform) = platform(&name) else {
        return no_such_platform();
    };
    match service::clear_unverified(&services(&state), &user, platform).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => refused(e),
    }
}
