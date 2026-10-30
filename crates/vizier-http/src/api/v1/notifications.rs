//! The notification inbox (`specs/029-webui-notifications/contracts/notification-api.md`):
//! list, count, read/unread and mark-all. Every handler resolves the caller's visible agents
//! first, so a revoked agent's rows disappear on the next read and another agent's row is
//! never returned.

use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State, rejection::JsonRejection},
    routing::{get, post, put},
};
use chrono::Utc;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::{
    api::v1::overview::visible_agents,
    auth::Identity,
    models::{
        self,
        response::{APIResponse, api_response, err_response},
    },
    state::HTTPState,
};
use vizier_core::{
    notifications::cursor,
    schema::{NotificationCounts, NotificationFilter, NotificationItem, NotificationPage},
    storage::notification::NotificationStorage,
};

const DEFAULT_LIMIT: usize = 20;
const MAX_LIMIT: usize = 50;

pub fn notifications() -> Router<HTTPState> {
    Router::new()
        .route("/", get(list))
        .route("/count", get(count))
        .route("/read-all", post(read_all))
        .route("/{id}/read", put(set_read))
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

/// The ids the caller may see, or a logged warning when the inbox cannot be projected.
async fn visible(state: &HTTPState, user: &Identity) -> Vec<String> {
    visible_agents(state, user)
        .await
        .map(|agents| agents.into_iter().map(|(id, _)| id).collect())
        .unwrap_or_default()
}

/// Bring the channel-reply projection up to date before a read; a failure is logged and the
/// inbox still answers with what it already has.
async fn project(state: &HTTPState) {
    if let Err(e) = state.deps.notifications.project_replies(Utc::now()).await {
        tracing::warn!("failed to project channel replies into notifications: {e}");
    }
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
    Extension(user): Extension<Identity>,
    Query(query): Query<NotificationQuery>,
) -> models::response::Response<NotificationPage> {
    let filter = match query.filter.as_deref().map(NotificationFilter::parse) {
        Some(Some(filter)) => filter,
        Some(None) => return err_response(StatusCode::BAD_REQUEST, "unknown filter".into()),
        None => NotificationFilter::All,
    };
    let before = match query.before.as_deref().map(cursor::decode) {
        Some(Some(cursor)) => Some(cursor),
        Some(None) => return err_response(StatusCode::BAD_REQUEST, "malformed cursor".into()),
        None => None,
    };
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);

    project(&state).await;
    let agents = visible(&state, &user).await;
    // One more than a page, to know whether there is another.
    let mut items = match state
        .storage
        .list_notifications(&user.user_id, &agents, filter, before, limit + 1)
        .await
    {
        Ok(items) => items,
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let next = (items.len() > limit).then(|| {
        items.truncate(limit);
        cursor::encode(items[limit - 1].at, items[limit - 1].id)
    });
    let counts = match state
        .storage
        .notification_counts(&user.user_id, &agents)
        .await
    {
        Ok(counts) => counts,
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    api_response(StatusCode::OK, NotificationPage { items, counts, next })
}

#[utoipa::path(
    get,
    path = "/notifications/count",
    responses((status = 200, description = "Unread and total counts across the caller's visible agents", body = APIResponse<NotificationCounts>))
)]
pub async fn count(
    State(state): State<HTTPState>,
    Extension(user): Extension<Identity>,
) -> models::response::Response<NotificationCounts> {
    project(&state).await;
    let agents = visible(&state, &user).await;
    match state
        .storage
        .notification_counts(&user.user_id, &agents)
        .await
    {
        Ok(counts) => api_response(StatusCode::OK, counts),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
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
    Extension(user): Extension<Identity>,
    Path(id): Path<i64>,
    body: Result<Json<NotificationReadRequest>, JsonRejection>,
) -> models::response::Response<NotificationCounts> {
    let read = match body {
        Ok(Json(NotificationReadRequest { read: Some(read) })) => read,
        _ => return err_response(StatusCode::BAD_REQUEST, "a `read` boolean is required".into()),
    };
    let agents = visible(&state, &user).await;
    match state
        .storage
        .set_notification_read(&user.user_id, id, read, Utc::now(), &agents)
        .await
    {
        Ok(true) => {}
        Ok(false) => return err_response(StatusCode::NOT_FOUND, "notification not found".into()),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
    match state
        .storage
        .notification_counts(&user.user_id, &agents)
        .await
    {
        Ok(counts) => api_response(StatusCode::OK, counts),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

#[utoipa::path(
    post,
    path = "/notifications/read-all",
    responses((status = 200, description = "Marks every visible unread notification read, with the new counts", body = APIResponse<NotificationCounts>))
)]
pub async fn read_all(
    State(state): State<HTTPState>,
    Extension(user): Extension<Identity>,
) -> models::response::Response<NotificationCounts> {
    project(&state).await;
    let agents = visible(&state, &user).await;
    if let Err(e) = state
        .storage
        .mark_all_notifications_read(&user.user_id, Utc::now(), &agents)
        .await
    {
        return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string());
    }
    match state
        .storage
        .notification_counts(&user.user_id, &agents)
        .await
    {
        Ok(counts) => api_response(StatusCode::OK, counts),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
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
        auth::{AuthMethod, Identity},
        state::HTTPState,
    };
    use vizier_core::{
        config::VizierConfig,
        schema::{AgentConfig, NotificationCategory, NotificationDraft, NotificationTone},
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
            deferred_client_calls: Default::default(),
        };
        (state, dir)
    }

    fn user(id: &str, system: bool) -> Identity {
        Identity {
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

