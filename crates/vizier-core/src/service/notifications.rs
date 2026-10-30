//! The notification inbox (`specs/029-webui-notifications/contracts/notification-api.md`):
//! list, count, read/unread and mark-all. Every operation resolves the caller's visible agents
//! first, so a revoked agent's rows disappear on the next read and another agent's row is
//! never returned.

use chrono::Utc;

use crate::{
    auth::Principal,
    notifications::cursor,
    schema::{NotificationCounts, NotificationFilter, NotificationPage},
    service::{ServiceError, Services, overview::visible_agents},
    storage::notification::NotificationStorage,
};

const DEFAULT_LIMIT: usize = 20;
const MAX_LIMIT: usize = 50;

/// The ids the caller may see; none when the agents cannot be listed.
async fn visible(services: &Services, user: &Principal) -> Vec<String> {
    visible_agents(services, user)
        .await
        .map(|agents| agents.into_iter().map(|(id, _)| id).collect())
        .unwrap_or_default()
}

/// Bring the channel-reply projection up to date before a read; a failure is logged and the
/// inbox still answers with what it already has.
async fn project(services: &Services) {
    if let Err(e) = services.deps().notifications.project_replies(Utc::now()).await {
        tracing::warn!("failed to project channel replies into notifications: {e}");
    }
}

fn internal(e: impl std::fmt::Display) -> ServiceError {
    ServiceError::Internal(e.to_string())
}

/// `filter` is `all` (default), `unread` or `read`; `before` the `next` of the previous page;
/// `limit` is clamped to 1–50 (default 20).
pub async fn list(
    services: &Services,
    user: &Principal,
    filter: Option<&str>,
    before: Option<&str>,
    limit: Option<usize>,
) -> Result<NotificationPage, ServiceError> {
    let filter = match filter.map(NotificationFilter::parse) {
        Some(Some(filter)) => filter,
        Some(None) => return Err(ServiceError::invalid("unknown filter")),
        None => NotificationFilter::All,
    };
    let before = match before.map(cursor::decode) {
        Some(Some(cursor)) => Some(cursor),
        Some(None) => return Err(ServiceError::invalid("malformed cursor")),
        None => None,
    };
    let limit = limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);

    project(services).await;
    let agents = visible(services, user).await;
    let storage = &services.deps().storage;
    // One more than a page, to know whether there is another.
    let mut items = storage
        .list_notifications(&user.user_id, &agents, filter, before, limit + 1)
        .await
        .map_err(internal)?;
    let next = (items.len() > limit).then(|| {
        items.truncate(limit);
        cursor::encode(items[limit - 1].at, items[limit - 1].id)
    });
    let counts = storage
        .notification_counts(&user.user_id, &agents)
        .await
        .map_err(internal)?;
    Ok(NotificationPage { items, counts, next })
}

pub async fn count(services: &Services, user: &Principal) -> Result<NotificationCounts, ServiceError> {
    project(services).await;
    let agents = visible(services, user).await;
    services
        .deps()
        .storage
        .notification_counts(&user.user_id, &agents)
        .await
        .map_err(internal)
}

/// Marks one notification read or unread for this person; another agent's row is `NotFound`.
pub async fn set_read(
    services: &Services,
    user: &Principal,
    id: i64,
    read: bool,
) -> Result<NotificationCounts, ServiceError> {
    let agents = visible(services, user).await;
    let storage = &services.deps().storage;
    match storage
        .set_notification_read(&user.user_id, id, read, Utc::now(), &agents)
        .await
    {
        Ok(true) => {}
        Ok(false) => return Err(ServiceError::NotFound("notification not found".into())),
        Err(e) => return Err(internal(e)),
    }
    storage
        .notification_counts(&user.user_id, &agents)
        .await
        .map_err(internal)
}

pub async fn read_all(services: &Services, user: &Principal) -> Result<NotificationCounts, ServiceError> {
    project(services).await;
    let agents = visible(services, user).await;
    let storage = &services.deps().storage;
    storage
        .mark_all_notifications_read(&user.user_id, Utc::now(), &agents)
        .await
        .map_err(internal)?;
    storage
        .notification_counts(&user.user_id, &agents)
        .await
        .map_err(internal)
}
