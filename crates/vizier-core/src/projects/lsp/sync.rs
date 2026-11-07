//! Keeping a server in step with the files (R7): full-text `didOpen`/`didChange` whenever the
//! content differs from what it was last sent, at most 256 documents open per server.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::json;
use sha2::{Digest, Sha256};

use super::position::path_to_uri;
use super::server::Server;

/// Open documents per server; the oldest is closed beyond this.
pub const MAX_OPEN_DOCUMENTS: usize = 256;
/// A larger file, or one that is not UTF-8, is not sent (`file_unsupported`).
pub const MAX_FILE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug)]
struct Doc {
    version: i64,
    sha: [u8; 32],
    order: u64,
}

/// What a server has been sent. Behind a lock on the server.
#[derive(Default)]
pub struct Documents {
    open: HashMap<String, Doc>,
    counter: u64,
    /// Path to modification time, taken when the server became ready (`notify_changes`).
    pub(super) snapshot: Option<HashMap<PathBuf, SystemTime>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SyncError {
    /// Over 2 MiB, or not UTF-8.
    Unsupported,
}

/// A document as the server now holds it.
#[derive(Debug, Clone, PartialEq)]
pub struct Synced {
    pub uri: String,
    pub version: i64,
}

impl Server {
    /// Make the server hold `bytes` for `path`. Nothing is sent when the content is what it
    /// already has.
    pub async fn sync_document(
        &self,
        path: &Path,
        bytes: &[u8],
        language_id: &str,
    ) -> Result<Synced, SyncError> {
        if bytes.len() > MAX_FILE_BYTES {
            return Err(SyncError::Unsupported);
        }
        let text = std::str::from_utf8(bytes).map_err(|_| SyncError::Unsupported)?;
        let uri = path_to_uri(path).ok_or(SyncError::Unsupported)?;
        let sha: [u8; 32] = Sha256::digest(bytes).into();

        let mut docs = self.docs.lock().await;
        docs.counter += 1;
        let order = docs.counter;
        if let Some(doc) = docs.open.get_mut(&uri) {
            doc.order = order;
            if doc.sha == sha {
                return Ok(Synced { uri, version: doc.version });
            }
            doc.version += 1;
            doc.sha = sha;
            let version = doc.version;
            self.notify(
                "textDocument/didChange",
                json!({
                    "textDocument": { "uri": uri, "version": version },
                    "contentChanges": [{ "text": text }]
                }),
            );
            return Ok(Synced { uri, version });
        }

        while docs.open.len() >= MAX_OPEN_DOCUMENTS {
            let Some(oldest) = docs
                .open
                .iter()
                .min_by_key(|(_, d)| d.order)
                .map(|(u, _)| u.clone())
            else {
                break;
            };
            docs.open.remove(&oldest);
            self.pushed.lock().remove(&oldest);
            self.notify("textDocument/didClose", json!({ "textDocument": { "uri": oldest } }));
        }
        docs.open.insert(uri.clone(), Doc { version: 1, sha, order });
        self.notify(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": uri, "languageId": language_id, "version": 1, "text": text
                }
            }),
        );
        Ok(Synced { uri, version: 1 })
    }

    pub fn did_save(&self, uri: &str) {
        self.notify("textDocument/didSave", json!({ "textDocument": { "uri": uri } }));
    }

    /// The documents currently open, most recently used first.
    pub async fn open_documents(&self) -> Vec<String> {
        let docs = self.docs.lock().await;
        let mut all: Vec<_> = docs.open.iter().map(|(u, d)| (d.order, u.clone())).collect();
        all.sort_by_key(|a| std::cmp::Reverse(a.0));
        all.into_iter().map(|(_, u)| u).collect()
    }

    pub async fn is_open(&self, path: &Path) -> bool {
        match path_to_uri(path) {
            Some(uri) => self.docs.lock().await.open.contains_key(&uri),
            None => false,
        }
    }
}

/// More changes than this and the server restarts instead of being told one by one (R7).
pub const MAX_CHANGES: usize = 5_000;
/// A snapshot stops growing here; a repository this large is watched approximately.
const MAX_SNAPSHOT_FILES: usize = 250_000;

/// What `notify_changes` found.
#[derive(Debug, Clone, PartialEq)]
pub enum Changes {
    /// This many changes were sent to the server.
    Sent(usize),
    /// Nothing differs from the snapshot.
    Unchanged,
    /// So many files changed that the server should start over.
    TooMany(usize),
    /// The server has not been ready long enough to have a snapshot.
    NoSnapshot,
}

/// Path to modification time for every file under `root` the project's ignore rules do not hide.
pub fn snapshot_of(root: &Path) -> HashMap<PathBuf, SystemTime> {
    let mut files = HashMap::new();
    let walker = ignore::WalkBuilder::new(root)
        .hidden(false)
        .follow_links(false)
        .filter_entry(|entry| {
            let name = entry.file_name();
            name != ".git" && name != ".vizier"
        })
        .build();
    for entry in walker.flatten() {
        if files.len() >= MAX_SNAPSHOT_FILES {
            break;
        }
        if entry.file_type().is_some_and(|t| t.is_file())
            && let Ok(modified) = entry.metadata().map_err(std::io::Error::other).and_then(|m| m.modified())
        {
            files.insert(entry.into_path(), modified);
        }
    }
    files
}

impl Server {
    /// Remember what is on disk under `root`, to tell later what a command changed.
    pub async fn take_snapshot(&self, root: &Path) {
        let root = root.to_path_buf();
        if let Ok(taken) = tokio::task::spawn_blocking(move || snapshot_of(&root)).await {
            self.docs.lock().await.snapshot = Some(taken);
        }
    }

    /// Compare disk with the snapshot and send `workspace/didChangeWatchedFiles` for the
    /// difference, so a server learns of changes made outside the tools (a command, a branch
    /// switch). The snapshot is replaced either way.
    pub async fn notify_changes(&self, root: &Path) -> Changes {
        let before = {
            let docs = self.docs.lock().await;
            match &docs.snapshot {
                Some(snapshot) => snapshot.clone(),
                None => return Changes::NoSnapshot,
            }
        };
        let walk_root = root.to_path_buf();
        let Ok(after) = tokio::task::spawn_blocking(move || snapshot_of(&walk_root)).await else {
            return Changes::Unchanged;
        };
        let mut changes = Vec::new();
        for (path, modified) in &after {
            let kind = match before.get(path) {
                None => 1,
                Some(old) if old != modified => 2,
                Some(_) => continue,
            };
            if let Some(uri) = path_to_uri(path) {
                changes.push(json!({ "uri": uri, "type": kind }));
            }
        }
        for path in before.keys() {
            if !after.contains_key(path)
                && let Some(uri) = path_to_uri(path)
            {
                changes.push(json!({ "uri": uri, "type": 3 }));
            }
        }
        self.docs.lock().await.snapshot = Some(after);
        match changes.len() {
            0 => Changes::Unchanged,
            n if n > MAX_CHANGES => Changes::TooMany(n),
            n => {
                self.notify("workspace/didChangeWatchedFiles", json!({ "changes": changes }));
                Changes::Sent(n)
            }
        }
    }
}
