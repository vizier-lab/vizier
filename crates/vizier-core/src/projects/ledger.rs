//! The stale-read ledger (`specs/041-project-coding-workspace` R8).
//!
//! A file may be edited only after the chat has read it and only while its bytes still match
//! what was read. The map is in memory — after a restart the agent reads again — and bounded per
//! session so a long chat cannot grow it without limit.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use parking_lot::Mutex;
use sha2::{Digest, Sha256};

use crate::error::VizierError;

/// The most files one session remembers before the oldest entry is dropped.
pub const MAX_ENTRIES_PER_SESSION: usize = 1_024;

#[derive(Clone)]
struct Entry {
    hash: [u8; 32],
    seq: u64,
}

#[derive(Default)]
struct State {
    entries: HashMap<(String, PathBuf), Entry>,
    next_seq: u64,
}

/// `(session slug, canonical path) → sha256` of the bytes when the chat last read or wrote it.
#[derive(Default)]
pub struct ProjectReadLedger {
    state: Mutex<State>,
}

impl std::fmt::Debug for ProjectReadLedger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectReadLedger")
            .field("entries", &self.state.lock().entries.len())
            .finish()
    }
}

fn hash(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

impl ProjectReadLedger {
    /// Record the file's current bytes, as a read or a successful write does.
    pub fn record(&self, session: &str, path: &Path, bytes: &[u8]) {
        let mut state = self.state.lock();
        state.next_seq += 1;
        let seq = state.next_seq;
        let key = (session.to_string(), path.to_path_buf());

        if !state.entries.contains_key(&key) {
            let in_session = state
                .entries
                .keys()
                .filter(|(s, _)| s == session)
                .count();
            if in_session >= MAX_ENTRIES_PER_SESSION {
                let oldest = state
                    .entries
                    .iter()
                    .filter(|((s, _), _)| s == session)
                    .min_by_key(|(_, entry)| entry.seq)
                    .map(|(key, _)| key.clone());
                if let Some(oldest) = oldest {
                    state.entries.remove(&oldest);
                }
            }
        }
        state
            .entries
            .insert(key, Entry { hash: hash(bytes), seq });
    }

    /// Require a matching remembered hash for the file's current bytes.
    pub fn check(&self, session: &str, path: &Path, current: &[u8]) -> Result<(), VizierError> {
        let state = self.state.lock();
        let Some(entry) = state
            .entries
            .get(&(session.to_string(), path.to_path_buf()))
        else {
            return Err(VizierError(format!(
                "stale_read: read {} again before writing it",
                crate::projects::channel::display_path(path)
            )));
        };
        if entry.hash != hash(current) {
            return Err(VizierError(format!(
                "stale_read: {} changed on disk; read it again before writing it",
                crate::projects::channel::display_path(path)
            )));
        }
        Ok(())
    }

    /// Drop everything remembered for one deleted conversation.
    pub fn forget_session(&self, session: &str) {
        let mut state = self.state.lock();
        state.entries.retain(|(s, _), _| s != session);
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.state.lock().entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path() -> PathBuf {
        PathBuf::from("/tmp/project/src/lib.rs")
    }

    #[test]
    fn a_read_then_an_unchanged_edit_passes() {
        let ledger = ProjectReadLedger::default();
        ledger.record("dp-a__session", &path(), b"alpha");
        assert!(ledger.check("dp-a__session", &path(), b"alpha").is_ok());
    }

    #[test]
    fn an_unread_or_changed_file_is_stale() {
        let ledger = ProjectReadLedger::default();
        let err = ledger.check("s", &path(), b"alpha").unwrap_err();
        assert!(err.0.starts_with("stale_read:"), "{err:?}");
        ledger.record("s", &path(), b"alpha");
        let err = ledger.check("s", &path(), b"beta").unwrap_err();
        assert!(err.0.contains("changed on disk"), "{err:?}");
    }

    #[test]
    fn the_ledger_is_per_session() {
        let ledger = ProjectReadLedger::default();
        ledger.record("one", &path(), b"alpha");
        assert!(ledger.check("two", &path(), b"alpha").is_err());
        ledger.forget_session("one");
        assert!(ledger.check("one", &path(), b"alpha").is_err());
    }

    #[test]
    fn a_session_is_bounded_and_drops_its_oldest_entry() {
        let ledger = ProjectReadLedger::default();
        for i in 0..MAX_ENTRIES_PER_SESSION + 5 {
            ledger.record("s", &PathBuf::from(format!("/tmp/{i}")), b"x");
        }
        assert_eq!(ledger.len(), MAX_ENTRIES_PER_SESSION);
    }
}
