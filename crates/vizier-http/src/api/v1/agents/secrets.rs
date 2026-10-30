//! An agent's secrets over HTTP (`specs/027-agent-secrets/contracts/http-api.md`): list, create
//! or replace, delete, and the change log. Every read is a person managing the agent's store
//! (`secrets::access`); no response carries a value — only the server-computed mask.

use std::collections::{BTreeMap, HashMap};

use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    response::Response,
    routing::{get, put},
};
use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use vizier_core::{
    access::Caller,
    schema::{AgentConfig, AgentId, AgentSecretEvent, Requester, SecretAction},
    secrets::{
        access,
        service::{self, SecretError},
    },
    storage::{
        agent::AgentStorage,
        secret::{SecretOutcome, SecretRecord},
    },
};
use crate::{
    api::v1::integrations::{agents_by_id, fail, ok, plain, requester_name, usernames},
    auth::Principal,
    models::page::{self, ListQuery, Listing, SortKey, SortOrder},
    state::HTTPState,
};

pub fn secrets() -> Router<HTTPState> {
    Router::new()
        .route("/", get(list))
        .route("/events", get(events))
        .route("/{name}", put(set).delete(remove))
}

/// One secret as a person sees it: its name, the server's mask, and who changed it when.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct SecretItem {
    pub name: String,
    /// `utils::mask_secret` of the stored value. There is no field for the value itself.
    pub masked: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub updated_by: Requester,
    /// The username or the agent's name; the id when it cannot be resolved.
    pub updated_by_name: String,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct SecretList {
    /// By default sorted by name.
    pub items: Vec<SecretItem>,
    /// With `limit`: matches after `q`, before `offset`/`limit`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<usize>,
    /// With `limit`: matches per filter key (`all`). Always carries `all`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub counts: Option<BTreeMap<String, usize>>,
}

/// The secrets table's search and sort. Names and the person or agent who last changed them are
/// searchable; there is no filter beyond `all`.
pub struct SecretListing;

impl Listing<SecretItem> for SecretListing {
    fn matches(&self, item: &SecretItem, q: &str) -> bool {
        item.name.to_lowercase().contains(q) || item.updated_by_name.to_lowercase().contains(q)
    }

    fn filter_keys(&self, _: &[SecretItem]) -> Vec<String> {
        Vec::new()
    }

    fn in_filter(&self, _: &str, _: &SecretItem) -> bool {
        false
    }

    fn sort_key(&self, item: &SecretItem, sort: &str) -> SortKey {
        match sort {
            "updated" => SortKey::Num(item.updated_at.timestamp_millis()),
            _ => SortKey::text(&item.name),
        }
    }

    fn sorts(&self) -> &'static [&'static str] {
        &["name", "updated"]
    }

    fn default_order(&self, sort: &str) -> SortOrder {
        if sort == "updated" {
            SortOrder::Desc
        } else {
            SortOrder::Asc
        }
    }
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct SecretWriteResponse {
    /// `created` or `replaced`.
    pub status: String,
    pub secret: SecretItem,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize, utoipa::ToSchema)]
pub struct SetSecretRequest {
    /// The value to store. Never returned by any route.
    pub value: String,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct SecretDeleted {
    pub name: String,
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct SecretEventItem {
    pub id: String,
    pub name: String,
    pub action: SecretAction,
    pub actor: Requester,
    pub actor_name: String,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct SecretEventList {
    /// Newest first.
    pub items: Vec<SecretEventItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EventsQuery {
    /// Defaults to 20; capped at 100.
    pub limit: Option<usize>,
}

/// The agent, or the response to return: `404` when it does not exist, `403` when the caller is
/// not one of its secret managers. The permission is checked here, before any read or write —
/// hiding the page is not the protection.
async fn manageable_agent(
    state: &HTTPState,
    user: &Principal,
    agent_id: &str,
) -> Result<AgentConfig, Box<Response>> {
    let agent = match state.storage.get_agent(agent_id).await {
        Ok(Some(agent)) => agent,
        Ok(None) => return Err(Box::new(plain(StatusCode::NOT_FOUND, "agent not found"))),
        Err(e) => return Err(Box::new(plain(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))),
    };
    if access::can_manage(Caller::from(user), &agent) {
        Ok(agent)
    } else {
        Err(Box::new(fail(
            StatusCode::FORBIDDEN,
            "you may not manage this agent's secrets",
            serde_json::json!({
                "error": "forbidden",
                "required": [
                    access::MANAGE_ALL,
                    format!("{} (own agents)", access::MANAGE_OWNED),
                ],
            }),
        )))
    }
}

fn item(
    record: SecretRecord,
    agents: &HashMap<AgentId, AgentConfig>,
    users: &HashMap<String, String>,
) -> SecretItem {
    SecretItem {
        updated_by_name: requester_name(&record.updated_by, agents, users),
        name: record.name,
        masked: record.masked,
        created_at: record.created_at,
        updated_at: record.updated_at,
        updated_by: record.updated_by,
    }
}

fn event_item(
    event: AgentSecretEvent,
    agents: &HashMap<AgentId, AgentConfig>,
    users: &HashMap<String, String>,
) -> SecretEventItem {
    SecretEventItem {
        actor_name: requester_name(&event.actor, agents, users),
        id: event.id,
        name: event.name,
        action: event.action,
        actor: event.actor,
        at: event.at,
    }
}

/// The status pair a write reports: a new name is `201 created`, an existing one `200 replaced`.
fn write_status(outcome: SecretOutcome) -> (&'static str, StatusCode) {
    match outcome {
        SecretOutcome::Created => ("created", StatusCode::CREATED),
        SecretOutcome::Replaced => ("replaced", StatusCode::OK),
    }
}

/// The status a service refusal maps to. Nothing here restarts, refreshes or signals an agent:
/// a write is a row and an event, and the next retrieval reads it.
fn error_status(err: &SecretError) -> StatusCode {
    match err {
        SecretError::InvalidName(_) | SecretError::InvalidValue(_) => StatusCode::UNPROCESSABLE_ENTITY,
        SecretError::NotFound => StatusCode::NOT_FOUND,
        SecretError::Storage(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// A refusal (422 `invalid`, 404 `not_found`) or a real failure (500). The message never
/// quotes a value: validation messages state the rule only.
fn secret_error(err: SecretError) -> Response {
    let status = error_status(&err);
    match err {
        SecretError::InvalidName(message) | SecretError::InvalidValue(message) => fail(
            status,
            message,
            serde_json::json!({ "error": "invalid" }),
        ),
        SecretError::NotFound => plain(status, "secret not found"),
        SecretError::Storage(e) => plain(status, e.to_string()),
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/secrets",
    params(("agent_id" = String, Path, description = "Agent ID"), ListQuery),
    responses(
        (status = 200, description = "Names, masks and last change, sorted by name. With `limit`, one page with `total` and `counts`. No values", body = crate::models::response::APIResponse<SecretList>),
        (status = 403, description = "The caller is not a secret manager for this agent"),
        (status = 404, description = "Agent not found")
    )
)]
pub async fn list(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(agent_id): Path<String>,
    Query(paging): Query<ListQuery>,
) -> Response {
    if let Err(response) = manageable_agent(&state, &user, &agent_id).await {
        return *response;
    }
    let records = match service::list(&state.storage, &agent_id).await {
        Ok(records) => records,
        Err(e) => return secret_error(e),
    };
    let agents = agents_by_id(&state).await;
    let users = usernames(&state).await;
    let items = records.into_iter().map(|record| item(record, &agents, &users)).collect();
    // Without `limit` the answer stays the unpaged list clients have always got; paging is opt-in.
    if !paging.is_paged() {
        return ok(
            StatusCode::OK,
            SecretList { items, total: None, counts: None },
        );
    }
    match page::paginate(items, &SecretListing, &paging) {
        Ok(page) => ok(
            StatusCode::OK,
            SecretList {
                items: page.items,
                total: Some(page.total),
                counts: Some(page.counts),
            },
        ),
        Err(e) => plain(StatusCode::BAD_REQUEST, e.code),
    }
}

#[utoipa::path(
    get,
    path = "/agents/{agent_id}/secrets/events",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("limit" = Option<usize>, Query, description = "How many changes, newest first. Default 20, max 100")
    ),
    responses(
        (status = 200, description = "The change log. No value, mask or hash", body = crate::models::response::APIResponse<SecretEventList>),
        (status = 403, description = "The caller is not a secret manager for this agent"),
        (status = 404, description = "Agent not found")
    )
)]
pub async fn events(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(agent_id): Path<String>,
    Query(query): Query<EventsQuery>,
) -> Response {
    if let Err(response) = manageable_agent(&state, &user, &agent_id).await {
        return *response;
    }
    // `service::events` clamps to 1–100, so a huge or zero limit is answered, not rejected.
    let events = match service::events(&state.storage, &agent_id, query.limit.unwrap_or(20)).await {
        Ok(events) => events,
        Err(e) => return secret_error(e),
    };
    let agents = agents_by_id(&state).await;
    let users = usernames(&state).await;
    let items = events.into_iter().map(|event| event_item(event, &agents, &users)).collect();
    ok(StatusCode::OK, SecretEventList { items })
}

#[utoipa::path(
    put,
    path = "/agents/{agent_id}/secrets/{name}",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("name" = String, Path, description = "Secret name")
    ),
    request_body = SetSecretRequest,
    responses(
        (status = 201, description = "Created", body = crate::models::response::APIResponse<SecretWriteResponse>),
        (status = 200, description = "Replaced; created_at is preserved", body = crate::models::response::APIResponse<SecretWriteResponse>),
        (status = 403, description = "The caller is not a secret manager for this agent"),
        (status = 404, description = "Agent not found"),
        (status = 422, description = "The name or value fails validation")
    )
)]
pub async fn set(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, name)): Path<(String, String)>,
    Json(req): Json<SetSecretRequest>,
) -> Response {
    if let Err(response) = manageable_agent(&state, &user, &agent_id).await {
        return *response;
    }
    let actor = Requester::User(user.user_id.clone());
    let write = match service::set(&state.storage, &agent_id, &name, &req.value, &actor).await {
        Ok(write) => write,
        Err(e) => return secret_error(e),
    };
    let agents = agents_by_id(&state).await;
    let users = usernames(&state).await;
    let (status, code) = write_status(write.outcome);
    ok(
        code,
        SecretWriteResponse { status: status.to_string(), secret: item(write.record, &agents, &users) },
    )
}

#[utoipa::path(
    delete,
    path = "/agents/{agent_id}/secrets/{name}",
    params(
        ("agent_id" = String, Path, description = "Agent ID"),
        ("name" = String, Path, description = "Secret name")
    ),
    responses(
        (status = 200, description = "Deleted, and a `deleted` change recorded", body = crate::models::response::APIResponse<SecretDeleted>),
        (status = 403, description = "The caller is not a secret manager for this agent"),
        (status = 404, description = "Agent not found, or no secret with that name")
    )
)]
pub async fn remove(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((agent_id, name)): Path<(String, String)>,
) -> Response {
    if let Err(response) = manageable_agent(&state, &user, &agent_id).await {
        return *response;
    }
    let actor = Requester::User(user.user_id.clone());
    match service::delete(&state.storage, &agent_id, &name, &actor).await {
        Ok(()) => ok(StatusCode::OK, SecretDeleted { name, deleted: true }),
        Err(e) => secret_error(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vizier_core::storage::secret::SecretStorage;

    fn storage() -> (std::sync::Arc<vizier_core::storage::VizierStorage>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        vizier_core::storage::sqlite::init_secret_schema(&conn).unwrap();
        let storage = vizier_core::storage::sqlite::SqliteStorage::new(
            std::sync::Arc::new(parking_lot::Mutex::new(conn)),
            std::sync::Arc::new(vizier_core::storage::document::LocalDocumentStore::new(
                dir.path().to_path_buf(),
            )),
        );
        (std::sync::Arc::new(vizier_core::storage::VizierStorage::new(storage)), dir)
    }

    /// The status mapping the handlers rely on: `created` for a new name, `replaced` for an
    /// existing one, and the record keeps `created_at` across the replace.
    #[tokio::test]
    async fn a_write_reports_created_then_replaced() {
        let (storage, _dir) = storage();
        let actor = Requester::User("user-1".into());
        let first = service::set(&storage, "dp-a", "TOKEN", "agent-token-abc123", &actor)
            .await
            .unwrap();
        assert_eq!(first.outcome, SecretOutcome::Created);
        let second = service::set(&storage, "dp-a", "TOKEN", "rotated-token-xyz789", &actor)
            .await
            .unwrap();
        assert_eq!(second.outcome, SecretOutcome::Replaced);
        assert_eq!(second.record.created_at, first.record.created_at);
        assert_eq!(second.record.masked, "••••z789");
    }

    #[tokio::test]
    async fn a_missing_delete_is_not_found_and_records_no_event() {
        let (storage, _dir) = storage();
        assert!(matches!(
            service::delete(&storage, "dp-a", "NO_SUCH", &Requester::User("u".into())).await,
            Err(SecretError::NotFound)
        ));
        assert!(storage.list_agent_secret_events("dp-a", 20).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn the_event_limit_defaults_and_caps_through_the_service() {
        let (storage, _dir) = storage();
        let actor = Requester::User("user-1".into());
        for name in ["A", "B", "C"] {
            service::set(&storage, "dp-a", name, "value-00000001", &actor).await.unwrap();
        }
        assert_eq!(service::events(&storage, "dp-a", 20).await.unwrap().len(), 3);
        assert_eq!(service::events(&storage, "dp-a", 1000).await.unwrap().len(), 3);
        assert_eq!(service::events(&storage, "dp-a", 1).await.unwrap().len(), 1);
    }

    /// The DTOs have no value field at all: a mask is the most a response can carry.
    #[test]
    fn the_dtos_serialise_without_a_value() {
        let item = SecretItem {
            name: "TOKEN".into(),
            masked: "••••c123".into(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            updated_by: Requester::Agent("dp-a".into()),
            updated_by_name: "dv-a".into(),
        };
        let text = serde_json::to_string(&SecretList { items: vec![item], total: None, counts: None }).unwrap();
        assert!(text.contains("masked"));
        assert!(!text.contains("\"value\""));
    }

    /// Paging is opt-in and searches names and whoever last changed them.
    #[test]
    fn the_listing_searches_names_and_sorts_by_name_or_update() {
        let now = Utc::now();
        let item = |name: &str, by: &str, at: DateTime<Utc>| SecretItem {
            name: name.into(),
            masked: "••••c123".into(),
            created_at: now,
            updated_at: at,
            updated_by: Requester::User("u".into()),
            updated_by_name: by.into(),
        };
        let items = vec![
            item("TOKEN", "alice", now - chrono::Duration::hours(1)),
            item("API_KEY", "bob", now),
        ];

        let page = page::paginate(
            items.clone(),
            &SecretListing,
            &ListQuery { q: Some("ali".into()), limit: Some(25), ..Default::default() },
        )
        .ok()
        .unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].name, "TOKEN");
        assert_eq!(page.counts["all"], 1);

        let page = page::paginate(
            items,
            &SecretListing,
            &ListQuery { sort: Some("updated".into()), limit: Some(25), ..Default::default() },
        )
        .ok()
        .unwrap();
        assert_eq!(page.items[0].name, "API_KEY");
    }

    /// `201` for a new name and `200` for a replace, as the routes document.
    #[test]
    fn a_write_maps_to_201_then_200() {
        assert_eq!(write_status(SecretOutcome::Created), ("created", StatusCode::CREATED));
        assert_eq!(write_status(SecretOutcome::Replaced), ("replaced", StatusCode::OK));
    }

    /// Validation is `422`, a missing name `404`, and a storage failure `500`.
    #[test]
    fn refusals_map_to_422_404_and_500() {
        assert_eq!(error_status(&SecretError::InvalidName("no".into())), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(error_status(&SecretError::InvalidValue("no".into())), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(error_status(&SecretError::NotFound), StatusCode::NOT_FOUND);
        assert_eq!(
            error_status(&SecretError::Storage(vizier_core::error::VizierError("no".into()).into())),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
