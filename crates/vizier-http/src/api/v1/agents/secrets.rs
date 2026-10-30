//! An agent's secrets over HTTP (`specs/027-agent-secrets/contracts/http-api.md`): list, create
//! or replace, delete, and the change log. Every read is a person managing the agent's store
//! (`vizier_core::service::secrets`); no response carries a value — only the server-computed mask.

use std::collections::BTreeMap;

use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    response::Response,
    routing::{get, put},
};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use vizier_core::service::{ServiceError, secrets as service};
use crate::{
    api::v1::integrations::{failure, ok, plain, svc},
    auth::Principal,
    models::page::{self, ListQuery, Listing, SortKey, SortOrder},
    state::HTTPState,
};

pub use service::{
    SecretDeleted, SecretEventItem, SecretItem, SecretWriteResponse, SetSecretRequest,
};

pub fn secrets() -> Router<HTTPState> {
    Router::new()
        .route("/", get(list))
        .route("/events", get(events))
        .route("/{name}", put(set).delete(remove))
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
pub struct SecretEventList {
    /// Newest first.
    pub items: Vec<SecretEventItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EventsQuery {
    /// Defaults to 20; capped at 100.
    pub limit: Option<usize>,
}

/// A refusal (422 `invalid`, 404 `not_found`, 403 with the permissions that would allow it) or
/// a real failure (500).
fn fail(e: ServiceError) -> Response {
    failure(
        e,
        Some(serde_json::json!([
            service::MANAGE_ALL,
            format!("{} (own agents)", service::MANAGE_OWNED),
        ])),
    )
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
    let items = match service::list(&svc(&state), &user, &agent_id).await {
        Ok(items) => items,
        Err(e) => return fail(e),
    };
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
    match service::events(&svc(&state), &user, &agent_id, query.limit).await {
        Ok(items) => ok(StatusCode::OK, SecretEventList { items }),
        Err(e) => fail(e),
    }
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
    match service::set(&svc(&state), &user, &agent_id, &name, &req.value).await {
        Ok(write) => {
            // A new name is `201 created`, an existing one `200 replaced`.
            let code = if write.status == "created" { StatusCode::CREATED } else { StatusCode::OK };
            ok(code, write)
        }
        Err(e) => fail(e),
    }
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
    match service::delete(&svc(&state), &user, &agent_id, &name).await {
        Ok(deleted) => ok(StatusCode::OK, deleted),
        Err(e) => fail(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use vizier_core::schema::Requester;

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
        let item = |name: &str, by: &str, at: chrono::DateTime<Utc>| SecretItem {
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

    /// Validation is `422`, a missing name `404`, a refused manager `403` with what would allow
    /// it, and a storage failure `500`.
    #[test]
    fn refusals_map_to_422_404_403_and_500() {
        let status = |e| fail(e).status();
        assert_eq!(
            status(ServiceError::Invalid { status: 422, message: "no".into(), code: Some("invalid".into()) }),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert_eq!(status(ServiceError::NotFound("secret not found".into())), StatusCode::NOT_FOUND);
        assert_eq!(status(ServiceError::Forbidden("no".into())), StatusCode::FORBIDDEN);
        assert_eq!(status(ServiceError::Internal("no".into())), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
