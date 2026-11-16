//! Interactive pages of a WebUI topic, for people (`specs/063-html-generative-ui` contracts/http.md).
//! Read-only: pages are made by the agent and go with their conversation. Authorization follows
//! the topic routes: the session is built from the caller's own username, and a page is visible
//! only when it was made in that very session; any other id is a 404, so a caller cannot tell it
//! exists.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{ServiceError, Services, detached::topic_session};
use crate::{
    auth::Principal,
    schema::TopicId,
    storage::ui_page::{UiPageRecord, UiPageStorage, UiPageVersionRecord, VersionOrigin},
};

/// One version, as the page route lists it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UiPageVersionView {
    pub version: u32,
    pub size: u64,
    /// `full` or `edits`.
    pub origin: String,
    pub base_version: Option<u32>,
    /// Epoch milliseconds.
    pub created_at: i64,
    /// `{"state": "pending" | "unverified" | "clean" | "problems", ...}`.
    pub verification: serde_json::Value,
}

/// A page with its versions and saved state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UiPageView {
    pub page_id: String,
    pub title: String,
    pub latest_version: u32,
    pub state: Option<serde_json::Value>,
    pub state_seq: u64,
    pub versions: Vec<UiPageVersionView>,
}

fn version_view(record: UiPageVersionRecord) -> UiPageVersionView {
    UiPageVersionView {
        version: record.version,
        size: record.size,
        origin: match record.origin {
            VersionOrigin::Full => "full".into(),
            VersionOrigin::Edits => "edits".into(),
        },
        base_version: record.base_version,
        created_at: record.created_at,
        verification: record.verification,
    }
}

fn not_found() -> ServiceError {
    ServiceError::NotFound("page not found".into())
}

/// The page, when it was made in the caller's own topic.
async fn own_page(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    channel_id: &str,
    topic_id: &TopicId,
    page_id: &str,
) -> Result<UiPageRecord, ServiceError> {
    let session = topic_session(s, who, agent_id, channel_id, topic_id).await?;
    match s.deps().storage.get_ui_page(page_id).await {
        Ok(Some(page)) if page.session == session => Ok(page),
        Ok(_) => Err(not_found()),
        Err(e) => Err(ServiceError::Internal(e.to_string())),
    }
}

/// A page, its versions oldest first, and its saved state.
pub async fn get_page(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    channel_id: &str,
    topic_id: &TopicId,
    page_id: &str,
) -> Result<UiPageView, ServiceError> {
    let page = own_page(s, who, agent_id, channel_id, topic_id, page_id).await?;
    let versions = s
        .deps()
        .storage
        .list_ui_page_versions(&page.id)
        .await
        .map_err(|e| ServiceError::Internal(e.to_string()))?;
    Ok(UiPageView {
        page_id: page.id,
        title: page.title,
        latest_version: page.latest_version,
        state: page.state,
        state_seq: page.state_seq,
        versions: versions.into_iter().map(version_view).collect(),
    })
}

/// One version's HTML.
pub async fn get_version(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    channel_id: &str,
    topic_id: &TopicId,
    page_id: &str,
    version: u32,
) -> Result<String, ServiceError> {
    let page = own_page(s, who, agent_id, channel_id, topic_id, page_id).await?;
    match s.deps().ui_pages.html(&page, version).await {
        Ok(Some(html)) => Ok(html),
        Ok(None) => Err(ServiceError::NotFound("version not found".into())),
        Err(e) => Err(ServiceError::Internal(e.to_string())),
    }
}
