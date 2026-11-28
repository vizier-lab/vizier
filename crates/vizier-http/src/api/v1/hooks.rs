//! The public webhook address (`specs/071-triggers/contracts/http-api.md`): outside
//! `require_auth`, authenticated by the trigger's own secret.
//!
//! An unknown trigger, an inactive one, a pull kind, a wrong secret and a bad signature all get the
//! same `404`, and misses are rate limited per address (FR-021). The sender is answered at once; the
//! run happens later (FR-023). Neither the address nor its secret is logged.

use std::net::SocketAddr;

use axum::{
    Extension,
    body::Bytes,
    extract::{ConnectInfo, Path, State, rejection::BytesRejection},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::Response,
};
use vizier_core::{
    storage::trigger::TriggerStorage,
    triggers::{self, Intake, PushRefusal, PushRequest},
};

use crate::{
    api::v1::{
        integrations::{fail, ok},
        shares::client_address,
    },
    state::HTTPState,
};

/// What axum reads before the kind looks at it. Larger than the kind's own cap, so the size is
/// refused only after the sender is authenticated, but bounded so nothing reads without end.
pub const READ_LIMIT: usize = 512 * 1024;

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct Accepted {
    pub event_id: i64,
    /// `accepted`, `ignored`, `dropped`, `refused` or `duplicate`.
    pub outcome: vizier_core::schema::AdmitOutcome,
}

#[utoipa::path(
    post,
    path = "/hooks/{trigger_id}",
    params(("trigger_id" = String, Path, description = "The trigger's address")),
    responses(
        (status = 202, description = "Recorded; the run happens later", body = Accepted),
        (status = 404, description = "Unknown, inactive, or not authenticated (one body)"),
        (status = 413, description = "The body is over 256 KiB"),
        (status = 415, description = "Not JSON, form-encoded or text"),
        (status = 429, description = "Too many misses from this address"),
        (status = 503, description = "Storage is unavailable")
    ),
    security(("bearer" = []))
)]
pub async fn receive(
    State(state): State<HTTPState>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    Path(trigger_id): Path<String>,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    handle(state, peer, trigger_id, None, headers, body).await
}

/// The same, with the secret in the address for senders that cannot set a header.
pub async fn receive_with_secret(
    State(state): State<HTTPState>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    Path((trigger_id, secret)): Path<(String, String)>,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    handle(state, peer, trigger_id, Some(secret), headers, body).await
}

async fn handle(
    state: HTTPState,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    trigger_id: String,
    path_secret: Option<String>,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    let miss = || {
        let client = client_address(
            peer.as_ref().map(|Extension(ConnectInfo(a))| a.ip()),
            &headers,
            &state.config.trusted_proxies,
        );
        // Shares and webhooks count misses in one bucket per address.
        if !state.share_misses.check(&client) {
            let wait = state.share_misses.retry_after(&client).map_or(60, |d| d.as_secs().max(1));
            let mut response = fail(
                StatusCode::TOO_MANY_REQUESTS,
                "Too many requests; try again later",
                serde_json::json!({ "error": "rate_limited", "retry_after_secs": wait }),
            );
            if let Ok(value) = HeaderValue::from_str(&wait.to_string()) {
                response.headers_mut().insert(header::RETRY_AFTER, value);
            }
            return response;
        }
        fail(StatusCode::NOT_FOUND, "Not found", serde_json::json!({ "error": "not_found" }))
    };

    let deps = &state.deps;
    let trigger = match deps.storage.get_trigger_by_id(&trigger_id).await {
        Ok(Some(trigger)) if trigger.accepts_events() => trigger,
        Ok(_) => return miss(),
        Err(e) => {
            tracing::error!("could not read a trigger for a webhook: {e}");
            return fail(StatusCode::SERVICE_UNAVAILABLE, "Unavailable", serde_json::json!({ "error": "unavailable" }));
        }
    };
    let Some(kind) = deps.triggers.kinds.get(&trigger.kind).filter(|k| k.is_public()) else {
        return miss();
    };
    let (body, unread) = match body {
        Ok(body) => (body.to_vec(), false),
        Err(_) => (vec![], true),
    };
    let request = PushRequest {
        headers: headers
            .iter()
            .filter_map(|(name, value)| Some((name.as_str().to_ascii_lowercase(), value.to_str().ok()?.to_string())))
            .collect(),
        body,
        path_secret,
        received_at: vizier_core::time::now(),
    };
    // Over what axum reads at all: only a sender known without the body is told so (FR-022).
    if unread {
        if !kind.authenticates_without_body(&trigger, &request) {
            return miss();
        }
        return fail(StatusCode::PAYLOAD_TOO_LARGE, "Too large", serde_json::json!({ "error": "too_large" }));
    }
    let intake = match kind.receive(&trigger, &request) {
        Ok(Intake::Reject) => return miss(),
        Ok(intake) => intake,
        Err(PushRefusal::TooLarge) => {
            return fail(StatusCode::PAYLOAD_TOO_LARGE, "Too large", serde_json::json!({ "error": "too_large" }));
        }
        Err(PushRefusal::UnsupportedType) => {
            return fail(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "Send JSON, form-encoded or text",
                serde_json::json!({ "error": "unsupported_type" }),
            );
        }
    };
    match triggers::accept(deps, &trigger, intake, None).await {
        Ok(Some(admission)) => ok(
            StatusCode::ACCEPTED,
            Accepted { event_id: admission.event_id, outcome: admission.outcome },
        ),
        Ok(None) => miss(),
        Err(e) => {
            tracing::error!(trigger = %trigger.slug, "could not record a webhook event: {e:#}");
            fail(StatusCode::SERVICE_UNAVAILABLE, "Unavailable", serde_json::json!({ "error": "unavailable" }))
        }
    }
}
