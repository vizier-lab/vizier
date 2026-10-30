//! What the global and the per-agent skill routes share: the response (in
//! `vizier_core::service::skills`), its listing rules (paging, `agent`/`global` filters,
//! `name`/`updated` sorts) and the stale-save response.

use axum::Json;
use reqwest::StatusCode;

use crate::models::{
    page::{Listing, SortKey, SortOrder},
    response::{APIResponse, Response, api_response, err_response},
};
use vizier_core::service::{ServiceError, skills::SkillError};

pub use vizier_core::service::skills::{SkillResponse, content_hash, is_stale};

/// `409 stale_content`: the skill changed since the editor loaded it; `data` is its current state.
pub fn stale_content(skill: SkillResponse) -> Response<SkillResponse> {
    (
        StatusCode::CONFLICT,
        Json(APIResponse {
            status: StatusCode::CONFLICT.as_u16(),
            message: Some("stale_content".into()),
            data: Some(skill),
        }),
    )
}

/// The service's refusal in this API's plain error shape.
pub fn refuse<T: serde::Serialize + Clone>(e: ServiceError) -> Response<T> {
    err_response(
        StatusCode::from_u16(e.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        e.message().to_string(),
    )
}

/// A saved skill, or why it was not.
pub fn saved(result: Result<SkillResponse, SkillError>) -> Response<SkillResponse> {
    match result {
        Ok(skill) => api_response(StatusCode::OK, skill),
        Err(SkillError::Stale(skill)) => stale_content(*skill),
        Err(SkillError::Service(e)) => refuse(e),
    }
}

/// Filters `agent` and `global`; sorts `name` and `updated` (newest first).
pub struct SkillListing;

impl Listing<SkillResponse> for SkillListing {
    fn matches(&self, item: &SkillResponse, q: &str) -> bool {
        item.name.to_lowercase().contains(q)
            || item.description.to_lowercase().contains(q)
            || item.keywords.iter().any(|k| k.to_lowercase().contains(q))
    }

    fn filter_keys(&self, _: &[SkillResponse]) -> Vec<String> {
        vec!["agent".into(), "global".into()]
    }

    fn in_filter(&self, filter: &str, item: &SkillResponse) -> bool {
        item.scope == filter
    }

    fn sort_key(&self, item: &SkillResponse, sort: &str) -> SortKey {
        match sort {
            "updated" => SortKey::Num(item.updated.map_or(0, |t| t.timestamp_millis())),
            _ => SortKey::text(&item.name),
        }
    }

    fn sorts(&self) -> &'static [&'static str] {
        &["name", "updated"]
    }

    fn default_order(&self, sort: &str) -> SortOrder {
        if sort == "updated" { SortOrder::Desc } else { SortOrder::Asc }
    }
}
