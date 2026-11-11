//! Shared chat snapshots (`specs/055-shared-chat/contracts/share-api.md`). Authorization,
//! validation and storage are in `vizier_core::service::shares`; these handlers translate, set
//! the response headers of the contract, and hand the stored text to the client without
//! parsing it into a value.

use axum::{
    Extension,
    body::Bytes,
    extract::{Path, State, rejection::BytesRejection},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use vizier_core::{
    auth::Principal,
    service::{
        ServiceError,
        shares::{self as service, CreatedShare, ShareRead, ShareView},
    },
};

pub use vizier_core::service::shares::{CreateShareRequest, PersonMatch};

use crate::{
    api::v1::integrations::{fail, ok, svc},
    models::response::{APIResponse, api_response},
    state::HTTPState,
};

/// A service refusal in the existing envelope, with its stable code in `data.error`.
fn refusal(e: ServiceError) -> Response {
    let status = StatusCode::from_u16(e.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let code = match e.code() {
        Some(code) => code.to_string(),
        None => match status {
            // A read's two refusals are their own codes (`sign_in_required`, `share_unavailable`).
            StatusCode::NOT_FOUND | StatusCode::UNAUTHORIZED => e.message().to_string(),
            StatusCode::FORBIDDEN => "forbidden".into(),
            StatusCode::TOO_MANY_REQUESTS => "rate_limited".into(),
            StatusCode::PAYLOAD_TOO_LARGE => "too_large".into(),
            _ => "error".into(),
        },
    };
    fail(status, e.message().to_string(), serde_json::json!({ "error": code }))
}

/// Headers every read carries, whatever the outcome (the contract's "Always").
fn guard_headers(headers: &mut HeaderMap) {
    headers.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(
        HeaderName::from_static("x-robots-tag"),
        HeaderValue::from_static("noindex, nofollow"),
    );
}

/// Client address for miss rate limiting (X-Forwarded-For, X-Real-IP, or local fallback).
fn client_address(headers: &HeaderMap) -> String {
    if let Some(xff) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(client) = xff.split(',').next().map(str::trim).filter(|s| !s.is_empty()) {
            return client.to_string();
        }
    }
    if let Some(rip) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
        let client = rip.trim();
        if !client.is_empty() {
            return client.to_string();
        }
    }
    "127.0.0.1".to_string()
}

#[derive(Debug, serde::Deserialize)]
pub struct PeopleQuery {
    #[serde(default)]
    pub q: String,
}

#[utoipa::path(
    get,
    path = "/shares/people",
    params(("q" = String, Query, description = "Prefix match on username (≥ 2 characters)")),
    responses(
        (status = 200, description = "Matching people", body = APIResponse<Vec<PersonMatch>>),
        (status = 400, description = "Query too short"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Requires share permission")
    )
)]
pub async fn search_people(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    axum::extract::Query(query): axum::extract::Query<PeopleQuery>,
) -> Response {
    match service::search_people(&svc(&state), &user, &query.q).await {
        Ok(people) => ok(StatusCode::OK, people),
        Err(e) => refusal(e),
    }
}

#[utoipa::path(
    post,
    path = "/shares",
    request_body = CreateShareRequest,
    responses(
        (status = 201, description = "Created. The link is `url_path` on this instance", body = APIResponse<CreatedShare>),
        (status = 400, description = "Invalid shape, format version, title or people"),
        (status = 401, description = "Missing or invalid credentials"),
        (status = 403, description = "Needs `owned_agents:share_conversations` for an agent the caller owns, or `all_agents:share_conversations`"),
        (status = 413, description = "The body is over 5 MiB"),
        (status = 422, description = "`trail_mismatch`: `include_trail` is false but the document holds trail parts"),
        (status = 429, description = "10 creates an hour (with `retry_after_secs`), or 200 active shares"),
        (status = 503, description = "Storage is unavailable")
    )
)]
pub async fn create(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    // Before the body is looked at: a person over the rate costs the server nothing more.
    if !state.share_creates.check(&user.user_id) {
        let wait = state
            .share_creates
            .retry_after(&user.user_id)
            .map_or(60, |d| d.as_secs().max(1));
        let mut response = fail(
            StatusCode::TOO_MANY_REQUESTS,
            "You are sharing too fast; try again later",
            serde_json::json!({ "error": "rate_limited", "retry_after_secs": wait }),
        );
        if let Ok(value) = HeaderValue::from_str(&wait.to_string()) {
            response.headers_mut().insert(header::RETRY_AFTER, value);
        }
        return response;
    }
    let Ok(body) = body else {
        return refusal(ServiceError::TooLarge("The snapshot is too large to share".into()));
    };
    match service::create(&svc(&state), &user, &body).await {
        Ok(created) => ok(StatusCode::CREATED, created),
        Err(e) => refusal(e),
    }
}

/// Whether `If-None-Match` names this share's tag.
fn matches_etag(headers: &HeaderMap, share_id: &str) -> bool {
    let Some(value) = headers.get(header::IF_NONE_MATCH).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    let tag = format!("\"{share_id}\"");
    value
        .split(',')
        .map(str::trim)
        .any(|t| t == "*" || t == tag || t.strip_prefix("W/") == Some(tag.as_str()))
}

#[utoipa::path(
    get,
    path = "/shares/{share_id}",
    params(("share_id" = String, Path, description = "21 URL-safe characters")),
    responses(
        (status = 200, description = "The snapshot. Public shares: `Cache-Control: public, max-age=300, s-maxage=300` and `ETag`; others `private, no-store`", body = APIResponse<ShareView>),
        (status = 304, description = "`If-None-Match` matches a public share"),
        (status = 401, description = "`sign_in_required`: no session, and the share is not an active public one (or does not exist; the same body)"),
        (status = 404, description = "`share_unavailable`: signed in, and the share is unknown, revoked, or not for this person (one body)")
    )
)]
pub async fn read(
    State(state): State<HTTPState>,
    Extension(viewer): Extension<Option<Principal>>,
    Path(share_id): Path<String>,
    request_headers: HeaderMap,
) -> Response {
    let mut response = match service::read(&svc(&state), viewer.as_ref(), &share_id).await {
        Ok(ShareRead { view, cache }) => {
            let not_modified = cache == vizier_core::storage::share::CachePolicy::Shared
                && matches_etag(&request_headers, &view.share_id);
            let etag = format!("\"{}\"", view.share_id);
            let mut response = match not_modified {
                true => StatusCode::NOT_MODIFIED.into_response(),
                false => api_response(StatusCode::OK, view).into_response(),
            };
            let headers = response.headers_mut();
            headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache.cache_control()));
            if cache.varies_by_credentials() {
                headers.insert(header::VARY, HeaderValue::from_static("Authorization, Cookie"));
            } else if let Ok(tag) = HeaderValue::from_str(&etag) {
                headers.insert(header::ETAG, tag);
            }
            response
        }
        Err(e) => {
            let status = StatusCode::from_u16(e.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            if status == StatusCode::NOT_FOUND || status == StatusCode::UNAUTHORIZED {
                let client = client_address(&request_headers);
                if !state.share_misses.check(&client) {
                    let wait = state
                        .share_misses
                        .retry_after(&client)
                        .map_or(60, |d| d.as_secs().max(1));
                    let mut response = fail(
                        StatusCode::TOO_MANY_REQUESTS,
                        "Too many requests; try again later",
                        serde_json::json!({ "error": "rate_limited", "retry_after_secs": wait }),
                    );
                    if let Ok(value) = HeaderValue::from_str(&wait.to_string()) {
                        response.headers_mut().insert(header::RETRY_AFTER, value);
                    }
                    response
                        .headers_mut()
                        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
                    guard_headers(response.headers_mut());
                    return response;
                }
            }
            let mut response = refusal(e);
            response
                .headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            response
        }
    };
    guard_headers(response.headers_mut());
    response
}
