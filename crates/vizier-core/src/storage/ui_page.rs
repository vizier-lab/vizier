//! Interactive pages (`specs/063-html-generative-ui/data-model.md`): one `ui_page` row per page an
//! agent drew in a conversation, and one `ui_page_version` row per version. The HTML itself is in
//! the blob store (`ui-pages/{agent}/{page}/{sha256}.html`); rows hold only its metadata.
//!
//! Every write that advances a page is fenced on its agent's scope (spec 057), and a new version
//! is one guarded increment, so of two edit sets based on the same version exactly one wins. The
//! page's saved state is the person's data, written by a gateway, and is not fenced.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{schema::VizierSession, storage::VizierStorage};

/// How a version was produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionOrigin {
    /// A complete HTML document (a new page, or a full replacement).
    Full,
    /// An edit set applied to `base_version`.
    Edits,
}

impl VersionOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Edits => "edits",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "full" => Some(Self::Full),
            "edits" => Some(Self::Edits),
            _ => None,
        }
    }
}

/// A page, without its versions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiPageRecord {
    pub id: String,
    pub agent_id: String,
    /// The conversation the page was made in; only that conversation may change it.
    pub session: VizierSession,
    pub title: String,
    pub latest_version: u32,
    /// The page's saved state, shared by its versions (`null` until a page saves one).
    pub state: Option<Value>,
    /// Raised by one on every state save.
    pub state_seq: u64,
    /// Epoch milliseconds.
    pub created_at: i64,
    pub updated_at: i64,
}

/// One version's metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiPageVersionRecord {
    pub page_id: String,
    pub version: u32,
    /// Bytes of the HTML.
    pub size: u64,
    /// Hex sha256 of the HTML.
    pub sha256: String,
    pub origin: VersionOrigin,
    pub base_version: Option<u32>,
    pub edit_count: Option<u32>,
    /// `{"state": "pending" | "unverified" | "clean" | "problems", ...}` (data-model.md).
    pub verification: Value,
    /// The `UiPage` history entry that draws this version.
    pub history_uid: Option<String>,
    pub created_at: i64,
}

/// What a new version carries; the number is assigned by the store.
#[derive(Debug, Clone, PartialEq)]
pub struct NewUiPageVersion {
    pub size: u64,
    pub sha256: String,
    pub origin: VersionOrigin,
    pub base_version: Option<u32>,
    pub edit_count: Option<u32>,
}

/// What `add_ui_page_version` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddVersionOutcome {
    /// Saved as this version.
    Added(u32),
    /// The page has moved on: its latest version is `latest`, not the base the caller named.
    Stale { latest: u32 },
    NotFound,
}

/// What `save_ui_page_state` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveStateOutcome {
    /// Saved; the page's new `state_seq`.
    Saved(u64),
    /// `expected_seq` was given and the stored one differs.
    Stale(u64),
    NotFound,
}

/// The verification a version starts with.
pub fn pending_verification() -> Value {
    serde_json::json!({ "state": "pending" })
}

#[async_trait::async_trait]
pub trait UiPageStorage {
    /// Record a new page and its version 1. Fenced.
    async fn create_ui_page(&self, page: &UiPageRecord, first: &NewUiPageVersion) -> Result<()>;

    /// Add the version after `base`, when `base` is still the latest. A `title` renames the page
    /// in the same transaction. Fenced.
    async fn add_ui_page_version(
        &self,
        page_id: &str,
        base: u32,
        version: &NewUiPageVersion,
        title: Option<&str>,
    ) -> Result<AddVersionOutcome>;

    async fn get_ui_page(&self, page_id: &str) -> Result<Option<UiPageRecord>>;

    async fn get_ui_page_version(
        &self,
        page_id: &str,
        version: u32,
    ) -> Result<Option<UiPageVersionRecord>>;

    /// Every version of a page, oldest first.
    async fn list_ui_page_versions(&self, page_id: &str) -> Result<Vec<UiPageVersionRecord>>;

    /// A version's verification outcome. Fenced.
    async fn set_ui_page_verification(
        &self,
        page_id: &str,
        version: u32,
        verification: &Value,
    ) -> Result<()>;

    /// The history entry that draws a version. Fenced.
    async fn set_ui_page_history_uid(&self, page_id: &str, version: u32, uid: &str)
    -> Result<()>;

    /// Save the page's state. With `expected_seq`, only when it still matches. Not fenced: the
    /// state is the person's, written by whichever gateway their tab is on.
    async fn save_ui_page_state(
        &self,
        page_id: &str,
        state: &Value,
        expected_seq: Option<u64>,
    ) -> Result<SaveStateOutcome>;

    /// Delete a conversation's pages and versions, returning their ids so the caller can remove
    /// the blobs.
    async fn delete_ui_pages_for_session(&self, session: &VizierSession) -> Result<Vec<String>>;

    /// Delete every page of an agent, returning their ids.
    async fn delete_ui_pages_for_agent(&self, agent_id: &str) -> Result<Vec<String>>;

    /// Mark the agent's `pending` verifications written under a fence older than `below` as
    /// `unverified(closed)`: the instance that was waiting for them is gone. Fenced. Returns how
    /// many rows changed.
    async fn sweep_pending_ui_verifications(&self, agent_id: &str, below: u64) -> Result<u64>;
}

#[async_trait::async_trait]
impl UiPageStorage for VizierStorage {
    async fn create_ui_page(&self, page: &UiPageRecord, first: &NewUiPageVersion) -> Result<()> {
        self.0.create_ui_page(page, first).await
    }

    async fn add_ui_page_version(
        &self,
        page_id: &str,
        base: u32,
        version: &NewUiPageVersion,
        title: Option<&str>,
    ) -> Result<AddVersionOutcome> {
        self.0.add_ui_page_version(page_id, base, version, title).await
    }

    async fn get_ui_page(&self, page_id: &str) -> Result<Option<UiPageRecord>> {
        self.0.get_ui_page(page_id).await
    }

    async fn get_ui_page_version(
        &self,
        page_id: &str,
        version: u32,
    ) -> Result<Option<UiPageVersionRecord>> {
        self.0.get_ui_page_version(page_id, version).await
    }

    async fn list_ui_page_versions(&self, page_id: &str) -> Result<Vec<UiPageVersionRecord>> {
        self.0.list_ui_page_versions(page_id).await
    }

    async fn set_ui_page_verification(
        &self,
        page_id: &str,
        version: u32,
        verification: &Value,
    ) -> Result<()> {
        self.0.set_ui_page_verification(page_id, version, verification).await
    }

    async fn set_ui_page_history_uid(
        &self,
        page_id: &str,
        version: u32,
        uid: &str,
    ) -> Result<()> {
        self.0.set_ui_page_history_uid(page_id, version, uid).await
    }

    async fn save_ui_page_state(
        &self,
        page_id: &str,
        state: &Value,
        expected_seq: Option<u64>,
    ) -> Result<SaveStateOutcome> {
        self.0.save_ui_page_state(page_id, state, expected_seq).await
    }

    async fn delete_ui_pages_for_session(&self, session: &VizierSession) -> Result<Vec<String>> {
        self.0.delete_ui_pages_for_session(session).await
    }

    async fn delete_ui_pages_for_agent(&self, agent_id: &str) -> Result<Vec<String>> {
        self.0.delete_ui_pages_for_agent(agent_id).await
    }

    async fn sweep_pending_ui_verifications(&self, agent_id: &str, below: u64) -> Result<u64> {
        self.0.sweep_pending_ui_verifications(agent_id, below).await
    }
}
