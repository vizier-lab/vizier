//! The notification inbox (`specs/029-webui-notifications/contracts/notification-api.md`):
//! list, count, read/unread and mark-all. Every operation resolves the caller's visible agents
//! first, so a revoked agent's rows disappear on the next read and another agent's row is
//! never returned.

use std::collections::HashMap;



use crate::{
    storage::workspace::WorkspaceStorage,
    auth::Principal,
    notifications::cursor,
    schema::{NotificationCounts, NotificationFilter, NotificationItem, NotificationPage, VizierSession},
    service::{ServiceError, Services, overview::visible_agents},
    storage::{notification::NotificationStorage, session::SessionStorage},
    transport::InboxEvent,
};

const DEFAULT_LIMIT: usize = 20;
const MAX_LIMIT: usize = 50;

/// The ids the caller may see; none when the agents cannot be listed.
async fn visible(services: &Services, user: &Principal) -> Vec<String> {
    let mut ids: Vec<String> = visible_agents(services, user)
        .await
        .map(|agents| agents.into_iter().map(|(id, _)| id).collect())
        .unwrap_or_default();
    ids.extend(workspace_scopes(services, user).await);
    ids
}

/// One scope id for each Workspace the person is in now. Their Workspace rows are visible through
/// it, and revoking membership removes it, so a removal takes effect at once with nothing to
/// delete (`specs/053-workspace-channel` research R20).
pub(crate) async fn workspace_scopes(services: &Services, user: &Principal) -> Vec<String> {
    services
        .deps()
        .storage
        .workspaces_of_person(&user.username)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(workspace, _)| crate::schema::workspace_scope(&workspace.id))
        .collect()
}

/// The title a row shows, once per distinct conversation on the page: the conversation's
/// current title (so a rename shows at once), "New conversation" before one is made, or the
/// task's own title already on the row. A conversation that belongs to another person yields
/// neither title nor excerpt, even to someone who can view the agent
/// (`specs/051-realtime-notifications` FR-023).
pub(crate) async fn fill_titles(services: &Services, user: &Principal, items: &mut [NotificationItem]) {
    let mut seen: HashMap<VizierSession, Option<String>> = HashMap::new();
    for item in items.iter_mut() {
        let Some(session) = item.source_session.clone() else {
            continue;
        };
        let owner = session.1.web_user().map(str::to_string);
        if owner.as_deref().is_some_and(|name| name != user.username) {
            item.title = None;
            item.excerpt = None;
            continue;
        }
        let found = match seen.get(&session) {
            Some(found) => found.clone(),
            None => {
                let found = services
                    .deps()
                    .storage
                    .get_session_detail_by_topic(session.0.clone(), session.1.clone(), session.2.clone())
                    .await
                    .ok()
                    .flatten()
                    .map(|detail| detail.title)
                    .filter(|title| !title.trim().is_empty());
                seen.insert(session, found.clone());
                found
            }
        };
        match (found, owner) {
            (Some(title), _) => item.title = Some(title),
            (None, Some(_)) => item.title = Some("New conversation".to_string()),
            (None, None) => {}
        }
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

    let agents = visible(services, user).await;
    let storage = &services.deps().storage;
    // One more than a page, to know whether there is another.
    let mut items = storage
        .list_notifications(&user.user_id, &agents, filter, before, limit + 1)
        .await
        .map_err(internal)?;
    fill_titles(services, user, &mut items).await;
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
        .set_notification_read(&user.user_id, id, read, crate::time::now(), &agents)
        .await
    {
        Ok(true) => {}
        Ok(false) => return Err(ServiceError::NotFound("notification not found".into())),
        Err(e) => return Err(internal(e)),
    }
    // This person's other clients learn of it at once; nobody else is told.
    services.deps().transport.publish_inbox_event(InboxEvent::Read {
        user_id: user.user_id.clone(),
        id: Some(id),
        read,
    });
    storage
        .notification_counts(&user.user_id, &agents)
        .await
        .map_err(internal)
}

pub async fn read_all(services: &Services, user: &Principal) -> Result<NotificationCounts, ServiceError> {
    let agents = visible(services, user).await;
    let storage = &services.deps().storage;
    storage
        .mark_all_notifications_read(&user.user_id, crate::time::now(), &agents)
        .await
        .map_err(internal)?;
    services.deps().transport.publish_inbox_event(InboxEvent::Read {
        user_id: user.user_id.clone(),
        id: None,
        read: true,
    });
    storage
        .notification_counts(&user.user_id, &agents)
        .await
        .map_err(internal)
}
