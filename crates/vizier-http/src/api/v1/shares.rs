//! Shared chat snapshots (`specs/056-shared-chat/contracts/share-api.md`). Authorization,
//! validation and storage are in `vizier_core::service::shares`; these handlers translate, set
//! the response headers of the contract, and hand the stored text to the client without
//! parsing it into a value.

use std::net::{IpAddr, SocketAddr};

use axum::{
    Extension,
    body::Bytes,
    extract::{ConnectInfo, Path, State, rejection::BytesRejection},
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

pub use vizier_core::service::shares::{CreateShareRequest, PersonMatch, ShareListItem};

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

/// The key the miss limiter counts under.
///
/// The connection's own address is used. `X-Forwarded-For` / `X-Real-IP` are believed only when
/// the connection comes from this machine (a reverse proxy in front of the server): anyone else
/// could send those headers with any value and either dodge the limit or throw another person
/// into their bucket. Behind the proxy the entry the proxy appended (the last one) is taken, not
/// the first, which the caller wrote. IPv6 addresses count per /64. Without a peer address
/// (in-process calls) every caller shares one bucket named `unknown`.
pub(crate) fn client_address(peer: Option<IpAddr>, headers: &HeaderMap) -> String {
    let Some(peer) = peer else {
        return "unknown".to_string();
    };
    let forwarded = peer.is_loopback().then(|| {
        let from = |name: &str| {
            headers.get(name).and_then(|v| v.to_str().ok()).and_then(|v| {
                v.split(',').next_back().and_then(|last| last.trim().parse::<IpAddr>().ok())
            })
        };
        from("x-forwarded-for").or_else(|| from("x-real-ip"))
    });
    match forwarded.flatten().unwrap_or(peer) {
        IpAddr::V4(v4) => v4.to_string(),
        IpAddr::V6(v6) => {
            let s = v6.segments();
            format!("{:x}:{:x}:{:x}:{:x}::/64", s[0], s[1], s[2], s[3])
        }
    }
}

/// `429` with `Retry-After`, the same body wherever a limit is hit.
fn too_many(message: &str, wait: u64) -> Response {
    let mut response = fail(
        StatusCode::TOO_MANY_REQUESTS,
        message.to_string(),
        serde_json::json!({ "error": "rate_limited", "retry_after_secs": wait }),
    );
    if let Ok(value) = HeaderValue::from_str(&wait.to_string()) {
        response.headers_mut().insert(header::RETRY_AFTER, value);
    }
    response
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
    // 30 searches a minute per person (FR-012).
    if !state.share_searches.check(&user.user_id) {
        let wait = state
            .share_searches
            .retry_after(&user.user_id)
            .map_or(60, |d| d.as_secs().max(1));
        return too_many("You are searching too fast; try again later", wait);
    }
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
        (status = 422, description = "`trail_mismatch`: `include_trail` is false but the document holds trail parts; `secret_detected`: the text holds a PEM private key or a vendor-prefixed token (last-resort backstop, not redaction)"),
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
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
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
                let client = client_address(peer.map(|Extension(ConnectInfo(a))| a.ip()), &request_headers);
                if !state.share_misses.check(&client) {
                    let wait = state
                        .share_misses
                        .retry_after(&client)
                        .map_or(60, |d| d.as_secs().max(1));
                    let mut response = too_many("Too many requests; try again later", wait);
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

#[derive(Debug, serde::Deserialize)]
pub struct ListSharesQuery {
    pub before: Option<i64>,
    pub limit: Option<usize>,
    pub all: Option<bool>,
}

#[utoipa::path(
    get,
    path = "/shares",
    params(
        ("before" = Option<i64>, Query, description = "Keyset pagination: created_at timestamp"),
        ("limit" = Option<usize>, Query, description = "Max items to return (≤ 100, default 50)"),
        ("all" = Option<bool>, Query, description = "List all shares across instance (requires admin)")
    ),
    responses(
        (status = 200, description = "List of shares", body = APIResponse<Vec<ShareListItem>>),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "all=true requires admin permission")
    )
)]
pub async fn list(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    axum::extract::Query(query): axum::extract::Query<ListSharesQuery>,
) -> Response {
    let limit = query.limit.unwrap_or(50);
    let all = query.all.unwrap_or(false);
    let result = if all {
        service::list_all(&svc(&state), &user, query.before, limit).await
    } else {
        service::list_mine(&svc(&state), &user, query.before, limit).await
    };
    match result {
        Ok(items) => ok(StatusCode::OK, items),
        Err(e) => refusal(e),
    }
}

#[utoipa::path(
    delete,
    path = "/shares/{share_id}",
    params(("share_id" = String, Path, description = "21 URL-safe characters")),
    responses(
        (status = 204, description = "Revoked (or already revoked)"),
        (status = 401, description = "Authentication required"),
        (status = 404, description = "Unknown share or not authorized to revoke")
    )
)]
pub async fn revoke(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(share_id): Path<String>,
) -> Response {
    match service::revoke(&svc(&state), &user, &share_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => refusal(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.insert(*k, HeaderValue::from_str(v).unwrap());
        }
        h
    }

    fn ip(s: &str) -> Option<IpAddr> {
        Some(s.parse().unwrap())
    }

    /// A caller cannot pick their own bucket with a header.
    #[test]
    fn forwarding_headers_are_ignored_unless_the_peer_is_this_machine() {
        let spoof = headers(&[("x-forwarded-for", "9.9.9.9"), ("x-real-ip", "8.8.8.8")]);
        assert_eq!(client_address(ip("203.0.113.7"), &spoof), "203.0.113.7");
        assert_eq!(client_address(ip("203.0.113.7"), &HeaderMap::new()), "203.0.113.7");
        // Different peers are different buckets, and no peer is one shared, named bucket.
        assert_ne!(client_address(ip("203.0.113.7"), &spoof), client_address(ip("203.0.113.8"), &spoof));
        assert_eq!(client_address(None, &spoof), "unknown");
    }

    #[test]
    fn behind_a_local_proxy_the_entry_the_proxy_added_is_used() {
        // The first entry is whatever the caller wrote; the last is the proxy's own.
        let h = headers(&[("x-forwarded-for", "1.1.1.1, 198.51.100.4")]);
        assert_eq!(client_address(ip("127.0.0.1"), &h), "198.51.100.4");
        assert_eq!(client_address(ip("::1"), &h), "198.51.100.4");
        let real = headers(&[("x-real-ip", "198.51.100.5")]);
        assert_eq!(client_address(ip("127.0.0.1"), &real), "198.51.100.5");
        // Garbage falls back to the peer.
        let junk = headers(&[("x-forwarded-for", "not an ip")]);
        assert_eq!(client_address(ip("127.0.0.1"), &junk), "127.0.0.1");
    }

    #[test]
    fn ipv6_counts_per_slash_64() {
        let a = client_address(ip("2001:db8:1:2:aaaa::1"), &HeaderMap::new());
        let b = client_address(ip("2001:db8:1:2:bbbb::9"), &HeaderMap::new());
        let c = client_address(ip("2001:db8:1:3::1"), &HeaderMap::new());
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
