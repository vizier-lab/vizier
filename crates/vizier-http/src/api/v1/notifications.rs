//! The notification inbox (`specs/029-webui-notifications/contracts/notification-api.md`):
//! list, count, read/unread and mark-all. Every handler resolves the caller's visible agents
//! first, so a revoked agent's rows disappear on the next read and another agent's row is
//! never returned.

use axum::{
    Extension, Json, Router,
    extract::{
        Path, Query, State, WebSocketUpgrade,
        rejection::JsonRejection,
        ws::{Message, WebSocket},
    },
    routing::{get, post, put},
};
use futures::{SinkExt, StreamExt};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::{
    api::v1::auth,
    auth::{Principal, middleware::TokenExpiry},
    inbox::InboxConnection,
    models::{
        self,
        response::{APIResponse, api_response, err_response},
    },
    state::HTTPState,
};
use vizier_core::{
    schema::{NotificationCounts, NotificationItem, NotificationPage},
    service::notifications,
};

pub fn notifications() -> Router<HTTPState> {
    Router::new()
        .route("/", get(list))
        .route("/count", get(count))
        .route("/read-all", post(read_all))
        .route("/{id}/read", put(set_read))
}

/// The live stream (`specs/051-realtime-notifications`), mounted at `/inbox`.
pub fn inbox() -> Router<HTTPState> {
    Router::new().route("/stream", get(stream))
}

/// How often the server pings. There is no idle deadline: a push-only socket never sends a
/// frame, so the chat socket's idle timeout would drop every quiet inbox (research R10).
const PING_EVERY: std::time::Duration = std::time::Duration::from_secs(30);

#[utoipa::path(
    get,
    path = "/inbox/stream",
    responses(
        (status = 101, description = "WebSocket established. The server pushes JSON frames (`hello`, `notification`, `read`, `setup`, `setup_cleared`, `resync`) and ignores anything the client sends"),
        (status = 401, description = "Missing, invalid or expired credentials", body = APIResponse<String>)
    )
)]
pub async fn stream(
    ws: WebSocketUpgrade,
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Extension(expiry): Extension<TokenExpiry>,
) -> axum::response::Response {
    let (out_tx, out_rx) = flume::unbounded();
    let connection = match InboxConnection::open(&state, &user, out_tx).await {
        Ok(connection) => connection,
        Err(e) => {
            if matches!(e, vizier_core::service::ServiceError::Internal(_)) {
                tracing::error!("failed to open the inbox stream: {e}");
            }
            return axum::response::Response::builder()
                .status(e.status())
                .body(axum::body::Body::empty())
                .unwrap();
        }
    };
    ws.on_upgrade(move |socket| serve_socket(socket, connection, out_rx, expiry))
}

/// Writes the connection's frames to the socket, pings, and ends at the token's expiry.
async fn serve_socket(
    socket: WebSocket,
    connection: InboxConnection,
    out_rx: flume::Receiver<serde_json::Value>,
    expiry: TokenExpiry,
) {
    let (mut writer, mut reader) = socket.split();
    let pushing = tokio::spawn(connection.run());

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    // A credential that does not expire waits "forever"; the sleep is simply never reached.
    let until_expiry = expiry
        .remaining(now)
        .unwrap_or(std::time::Duration::from_secs(60 * 60 * 24 * 365));
    let expired = tokio::time::sleep(until_expiry);
    tokio::pin!(expired);
    let mut ping = tokio::time::interval(PING_EVERY);
    // The first tick is immediate; the hello is the first thing a client should see.
    ping.tick().await;

    loop {
        tokio::select! {
            _ = ping.tick() => {
                if writer.send(Message::Ping(vec![].into())).await.is_err() {
                    break;
                }
            }
            frame = out_rx.recv_async() => {
                match frame {
                    Ok(frame) => {
                        if writer.send(Message::Text(frame.to_string().into())).await.is_err() {
                            break;
                        }
                    }
                    // The connection ended: the person no longer resolves.
                    Err(_) => {
                        let _ = writer.send(Message::Close(None)).await;
                        break;
                    }
                }
            }
            msg = reader.next() => {
                match msg {
                    Some(Ok(Message::Ping(data))) => {
                        let _ = writer.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                    // The stream only pushes; whatever else a client sends is ignored.
                    Some(Ok(_)) => {}
                }
            }
            _ = &mut expired => {
                // The reconnect then meets a 401, and the client signs the person out.
                let _ = writer.send(Message::Close(None)).await;
                break;
            }
        }
    }
    pushing.abort();
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct NotificationQuery {
    /// `all` (default), `unread` or `read`.
    pub filter: Option<String>,
    /// The `next` of the previous page.
    pub before: Option<String>,
    /// 1–50; default 20.
    pub limit: Option<usize>,
}

#[utoipa::path(
    get,
    path = "/notifications",
    params(NotificationQuery),
    responses(
        (status = 200, description = "The caller's inbox, newest first, with counts for its filters", body = APIResponse<NotificationPage>),
        (status = 400, description = "An unknown filter, or a cursor that isn't one", body = APIResponse<String>)
    )
)]
pub async fn list(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Query(query): Query<NotificationQuery>,
) -> models::response::Response<NotificationPage> {
    match notifications::list(
        &auth::services(&state),
        &user,
        query.filter.as_deref(),
        query.before.as_deref(),
        query.limit,
    )
    .await
    {
        Ok(page) => api_response(StatusCode::OK, page),
        Err(e) => auth::err(e),
    }
}

#[utoipa::path(
    get,
    path = "/notifications/count",
    responses((status = 200, description = "Unread and total counts across the caller's visible agents", body = APIResponse<NotificationCounts>))
)]
pub async fn count(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
) -> models::response::Response<NotificationCounts> {
    match notifications::count(&auth::services(&state), &user).await {
        Ok(counts) => api_response(StatusCode::OK, counts),
        Err(e) => auth::err(e),
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct NotificationReadRequest {
    /// `true` marks it read, `false` marks it unread.
    #[serde(default)]
    pub read: Option<bool>,
}

#[utoipa::path(
    put,
    path = "/notifications/{id}/read",
    request_body = NotificationReadRequest,
    params(("id" = i64, Path, description = "The notification's public id")),
    responses(
        (status = 200, description = "This person's new read state, with the inbox counts", body = APIResponse<NotificationCounts>),
        (status = 400, description = "A missing or non-boolean `read`", body = APIResponse<String>),
        (status = 404, description = "No such notification for this person to see", body = APIResponse<String>)
    )
)]
pub async fn set_read(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(id): Path<i64>,
    body: Result<Json<NotificationReadRequest>, JsonRejection>,
) -> models::response::Response<NotificationCounts> {
    let read = match body {
        Ok(Json(NotificationReadRequest { read: Some(read) })) => read,
        _ => return err_response(StatusCode::BAD_REQUEST, "a `read` boolean is required".into()),
    };
    match notifications::set_read(&auth::services(&state), &user, id, read).await {
        Ok(counts) => api_response(StatusCode::OK, counts),
        Err(e) => auth::err(e),
    }
}

#[utoipa::path(
    post,
    path = "/notifications/read-all",
    responses((status = 200, description = "Marks every visible unread notification read, with the new counts", body = APIResponse<NotificationCounts>))
)]
pub async fn read_all(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
) -> models::response::Response<NotificationCounts> {
    match notifications::read_all(&auth::services(&state), &user).await {
        Ok(counts) => api_response(StatusCode::OK, counts),
        Err(e) => auth::err(e),
    }
}

/// The item type is part of the page schema; named here so both are in `ApiDoc`.
#[allow(dead_code)]
fn _item_in_schema(_: NotificationItem) {}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::Utc;

    use super::*;
    use crate::{
        api::v1::agents::user_can_view_agent,
        auth::{AuthMethod, Principal},
        state::HTTPState,
    };
    use vizier_core::{
        config::VizierConfig,
        schema::{AgentConfig, NotificationCategory, NotificationDraft, NotificationFilter, NotificationTone},
        storage::{agent::AgentStorage, notification::NotificationStorage},
    };
    use crate::test_support::agent_config;

    async fn state() -> (HTTPState, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let config = VizierConfig {
            workspace: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        };
        let deps = vizier_core::dependencies::VizierDependencies::new(config).await.unwrap();
        let state = HTTPState {
            auth: crate::state::AuthMode::Network,
            config: deps.config.clone(),
            transport: deps.transport.clone(),
            storage: deps.storage.clone(),
            background_jobs: deps.background_jobs.clone(),
            catalogue: deps.catalogue.clone(),
            deps,
            client_events: Arc::new(vizier_core::utils::rate_limit::RateLimiter::new(
                10,
                std::time::Duration::from_secs(60),
            )),
            client_state_saves: Arc::new(vizier_core::utils::rate_limit::RateLimiter::new(
                2,
                std::time::Duration::from_secs(1),
            )),
            visit_throttle: Arc::new(vizier_core::utils::rate_limit::RateLimiter::new(
                1,
                std::time::Duration::from_secs(60),
            )),
        };
        (state, dir)
    }

    fn user(id: &str, system: bool) -> Principal {
        Principal {
            user_id: id.into(),
            username: id.into(),
            role: vizier_core::storage::user::Role {
                role_id: "r".into(),
                name: "role".into(),
                permissions: vec![],
                is_system: system,
                created_at: Utc::now(),
            },
            permissions: vec![],
            method: AuthMethod::Jwt,
        }
    }

    async fn agent(state: &HTTPState, id: &str, owner: Option<&str>) {
        let mut config = agent_config(id);
        config.owner_id = owner.map(str::to_string);
        state.storage.create_agent(id, &config).await.unwrap();
    }

    async fn put(state: &HTTPState, key: &str, agent_id: &str) {
        state
            .storage
            .insert_notifications(&[NotificationDraft {
                event_key: key.into(),
                agent_id: agent_id.into(),
                audience_user_id: None,
                category: NotificationCategory::TaskRun,
                tone: NotificationTone::Default,
                summary: format!("Notification {key}"),
                href: "/x".into(),
                created_at: Utc::now(),
                source_session: None,
                source_title: None,
                excerpt: None,
            }])
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn the_inbox_pages_and_counts_only_the_visible_agents() {
        let (state, _d) = state().await;
        agent(&state, "a", Some("u1")).await;
        agent(&state, "b", Some("u2")).await;
        put(&state, "1", "a").await;
        put(&state, "2", "a").await;
        put(&state, "3", "b").await;

        let admin = user("root", true);
        let (status, Json(first)) = list(
            State(state.clone()),
            Extension(admin.clone()),
            Query(NotificationQuery {
                filter: None,
                before: None,
                limit: Some(2),
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let page = first.data.unwrap();
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.counts.all, 3);
        let next = page.next.expect("another page");

        // The next page continues after the cursor without repeating a row.
        let (status, Json(second)) = list(
            State(state.clone()),
            Extension(admin.clone()),
            Query(NotificationQuery {
                filter: None,
                before: Some(next),
                limit: Some(2),
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let page = second.data.unwrap();
        assert_eq!(page.items.len(), 1);
        assert!(page.next.is_none());

        // A stranger sees nothing: ownership is the visibility rule.
        let (status, Json(empty)) = count(State(state.clone()), Extension(user("stranger", false))).await;
        assert_eq!(status, StatusCode::OK);
        let counts = empty.data.unwrap();
        assert_eq!((counts.all, counts.unread), (0, 0));

        let (status, Json(owners)) = count(State(state), Extension(user("u1", false))).await;
        assert_eq!(status, StatusCode::OK);
        let counts = owners.data.unwrap();
        assert_eq!((counts.all, counts.unread), (2, 2));
    }

    #[tokio::test]
    async fn read_mutations_are_per_person_and_invisible_ids_are_not_found() {
        let (state, _d) = state().await;
        agent(&state, "a", Some("u1")).await;
        put(&state, "1", "a").await;
        let id = state
            .storage
            .list_notifications("u1", &["a".into()], NotificationFilter::All, None, 1)
            .await
            .unwrap()[0]
            .id;

        let (status, Json(read)) = set_read(
            State(state.clone()),
            Extension(user("u1", false)),
            Path(id),
            Ok(Json(NotificationReadRequest { read: Some(true) })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(read.data.unwrap().unread, 0);
        assert_eq!(
            state
                .storage
                .notification_counts("u1", &["a".into()])
                .await
                .unwrap(),
            NotificationCounts {
                all: 1,
                unread: 0,
                read: 1
            }
        );

        // Someone who may not see the agent cannot mark its notification.
        let (status, _) = set_read(
            State(state.clone()),
            Extension(user("stranger", false)),
            Path(id),
            Ok(Json(NotificationReadRequest { read: Some(true) })),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        // Mark-all leaves nothing unread for the reader, and another person still sees unread.
        let (status, Json(all)) = read_all(State(state.clone()), Extension(user("u1", false))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(all.data.unwrap().unread, 0);
        assert_eq!(
            state
                .storage
                .notification_counts("u1", &["a".into()])
                .await
                .unwrap()
                .unread,
            0
        );
        let other = state
            .storage
            .notification_counts("u2", &["a".into()])
            .await
            .unwrap();
        assert_eq!(other.unread, 1);
    }

    #[tokio::test]
    async fn an_unknown_filter_or_a_bad_cursor_is_a_400() {
        let (state, _d) = state().await;
        let (status, _) = list(
            State(state.clone()),
            Extension(user("root", true)),
            Query(NotificationQuery {
                filter: Some("everything".into()),
                before: None,
                limit: None,
            }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        let (status, _) = list(
            State(state.clone()),
            Extension(user("root", true)),
            Query(NotificationQuery {
                filter: None,
                before: Some("not a cursor".into()),
                limit: None,
            }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        // The same visibility rule the Home page uses, for the record.
        let mut config = agent_config("a");
        config.owner_id = Some("u1".into());
        assert!(user_can_view_agent(&user("u1", false), &config));
        assert!(!user_can_view_agent(&user("stranger", false), &config));
    }
}

