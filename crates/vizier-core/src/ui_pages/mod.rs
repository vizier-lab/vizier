//! Interactive pages an agent draws in a WebUI reply (`specs/063-html-generative-ui`).
//!
//! A page is a complete HTML document an agent made in one conversation, kept with every version
//! it went through. [`UiPages`] is the one write path: it stores each version's HTML in the blob
//! store under `ui-pages/{agent}/{page}/{sha256}.html` and its metadata through [`UiPageStorage`], and
//! (in `verify`) checks a new version in an open tab when there is one.

pub mod edits;
pub mod limits;
pub mod verify;

use std::sync::Arc;

use anyhow::{Result, anyhow};
use bytes::Bytes;
use rand::RngExt;
use sha2::{Digest, Sha256};

use crate::{
    schema::VizierSession,
    storage::{
        VizierStorage,
        blob::{BlobKey, BlobStore},
        ui_page::{
            AddVersionOutcome, NewUiPageVersion, UiPageRecord, UiPageStorage, VersionOrigin,
        },
    },
};

/// The blob folder of every page.
const ROOT: &str = "ui-pages";

/// A new page id: `pg_` and 22 base62 characters.
pub fn new_page_id() -> String {
    const ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    let mut rng = rand::rng();
    let tail: String = (0..22)
        .map(|_| ALPHABET[rng.random_range(0..ALPHABET.len())] as char)
        .collect();
    format!("pg_{tail}")
}

/// The blob folder of one page.
fn page_prefix(agent_id: &str, page_id: &str) -> Result<BlobKey> {
    BlobKey::new(ROOT)?.join(agent_id)?.join(page_id)
}

/// The blob of one version's HTML, named by its content: two writers racing for the same
/// version number (a stale edit set, a retry) can never overwrite each other's bytes, and the
/// loser's file is removed with the page.
fn content_key(agent_id: &str, page_id: &str, sha256: &str) -> Result<BlobKey> {
    page_prefix(agent_id, page_id)?.join(&format!("{sha256}.html"))
}

fn digest(html: &str) -> String {
    let hash = Sha256::digest(html.as_bytes());
    hash.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// What adding a version did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewVersion {
    Added(u32),
    /// The page has moved on; its latest version.
    Stale { latest: u32 },
    NotFound,
}

/// How a new version was made, for its metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Made {
    Full,
    Edits { count: u32 },
}

#[derive(Clone)]
pub struct UiPages {
    storage: Arc<VizierStorage>,
    blobs: Arc<dyn BlobStore>,
}

impl UiPages {
    pub fn new(storage: Arc<VizierStorage>, blobs: Arc<dyn BlobStore>) -> Self {
        Self { storage, blobs }
    }

    pub fn storage(&self) -> &Arc<VizierStorage> {
        &self.storage
    }

    /// Create a page in `session` with `html` as its version 1. The blob is written first, under
    /// its final key, so a row never names a missing file.
    pub async fn create(&self, session: &VizierSession, title: &str, html: &str) -> Result<UiPageRecord> {
        let agent_id = session.0.clone();
        let id = new_page_id();
        let sha256 = self.write_blob(&agent_id, &id, html).await?;
        let now = crate::time::now().timestamp_millis();
        let page = UiPageRecord {
            id: id.clone(),
            agent_id: agent_id.clone(),
            session: session.clone(),
            title: title.to_string(),
            latest_version: 1,
            state: None,
            state_seq: 0,
            created_at: now,
            updated_at: now,
        };
        let first = NewUiPageVersion {
            size: html.len() as u64,
            sha256,
            origin: VersionOrigin::Full,
            base_version: None,
            edit_count: None,
        };
        if let Err(e) = self.storage.create_ui_page(&page, &first).await {
            self.remove_page_blobs(&agent_id, &id).await;
            return Err(e);
        }
        Ok(page)
    }

    /// Save `html` as the version after `base` of `page`. The blob is written before the row,
    /// under its content key; a stale base leaves an unreferenced blob that the page's deletion
    /// removes with the rest.
    pub async fn add_version(
        &self,
        page: &UiPageRecord,
        base: u32,
        html: &str,
        made: Made,
        title: Option<&str>,
    ) -> Result<NewVersion> {
        let sha256 = self.write_blob(&page.agent_id, &page.id, html).await?;
        let (origin, edit_count) = match made {
            Made::Full => (VersionOrigin::Full, None),
            Made::Edits { count } => (VersionOrigin::Edits, Some(count)),
        };
        let version = NewUiPageVersion {
            size: html.len() as u64,
            sha256,
            origin,
            base_version: Some(base),
            edit_count,
        };
        Ok(
            match self.storage.add_ui_page_version(&page.id, base, &version, title).await? {
                AddVersionOutcome::Added(number) => NewVersion::Added(number),
                AddVersionOutcome::Stale { latest } => NewVersion::Stale { latest },
                AddVersionOutcome::NotFound => NewVersion::NotFound,
            },
        )
    }

    /// One version's HTML, or `None` when the version (or its file) is gone.
    pub async fn html(&self, page: &UiPageRecord, version: u32) -> Result<Option<String>> {
        let Some(record) = self.storage.get_ui_page_version(&page.id, version).await? else {
            return Ok(None);
        };
        let key = content_key(&page.agent_id, &page.id, &record.sha256)?;
        match self.blobs.get(&key).await? {
            Some(bytes) => Ok(Some(
                String::from_utf8(bytes.to_vec()).map_err(|_| anyhow!("page {} v{version} is not UTF-8", page.id))?,
            )),
            None => Ok(None),
        }
    }

    /// Delete a conversation's pages: rows, then blobs. Best effort on the blobs.
    pub async fn delete_session(&self, session: &VizierSession) {
        match self.storage.delete_ui_pages_for_session(session).await {
            Ok(ids) => {
                for id in ids {
                    self.remove_page_blobs(&session.0, &id).await;
                }
            }
            Err(e) => tracing::warn!(?session, "could not delete the conversation's pages: {e}"),
        }
    }

    /// Remove every page blob of an agent, after its rows went with the agent. Best effort.
    pub async fn delete_agent_blobs(&self, agent_id: &str) {
        let prefix = match BlobKey::new(ROOT).and_then(|root| root.join(agent_id)) {
            Ok(prefix) => prefix,
            Err(e) => return tracing::warn!(agent_id, "no page folder for this agent id: {e}"),
        };
        if let Err(e) = self.blobs.delete_prefix(&prefix).await {
            tracing::warn!(agent_id, "could not remove the agent's page blobs: {e}");
        }
    }

    /// Write `html` under its content key and return its sha256.
    async fn write_blob(&self, agent_id: &str, page_id: &str, html: &str) -> Result<String> {
        let sha256 = digest(html);
        let key = content_key(agent_id, page_id, &sha256)?;
        self.blobs.put(&key, Bytes::copy_from_slice(html.as_bytes())).await?;
        Ok(sha256)
    }

    async fn remove_page_blobs(&self, agent_id: &str, page_id: &str) {
        match page_prefix(agent_id, page_id) {
            Ok(prefix) => {
                if let Err(e) = self.blobs.delete_prefix(&prefix).await {
                    tracing::warn!(agent_id, page_id, "could not remove a page's blobs: {e}");
                }
            }
            Err(e) => tracing::warn!(agent_id, page_id, "no blob folder for this page: {e}"),
        }
    }
}

#[cfg(test)]
mod tests;
