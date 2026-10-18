//! Live "setup state" for an agent's build, shown in the WebUI.
//!
//! `BuiltAgent::build` marks an agent as building and every model download it triggers
//! (an Ollama pull, a fastembed model, a Whisper or Kokoro archive) reports into this
//! board as it runs. The WebUI polls `GET /api/v1/utils/setup-state` and renders one
//! spinner plus the current message — there is no progress in the console.
//!
//! Entries are self-cleaning, so nothing has to remember to remove them: a `ready` or
//! `failed` setup lingers for [`TERMINAL_TTL`] and is dropped on the next read, and an
//! entry with no live build and no active download is dropped immediately.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// How long a finished (`ready`/`failed`) setup lingers for the UI to notice it.
const TERMINAL_TTL: Duration = Duration::from_secs(5);

/// Where an agent's setup currently is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SetupPhase {
    /// Building, nothing to report in detail yet.
    #[default]
    Starting,
    /// At least one model download is running.
    Downloading,
    /// The build finished; the agent is up.
    Ready,
    /// The build failed.
    Failed,
}

/// What kind of artifact a download is fetching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DownloadKind {
    /// The agent's chat model (an Ollama pull).
    Model,
    /// A fastembed local embedding model.
    Embedding,
    /// A Whisper speech-to-text model.
    Stt,
    /// A Kokoro text-to-speech model.
    Tts,
}

/// One in-flight download.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DownloadStatus {
    pub kind: DownloadKind,
    pub label: String,
    /// Bytes done. `total == 0` means the size is unknown (indeterminate).
    pub completed: u64,
    pub total: u64,
    /// Human status from the source, e.g. `pulling manifest`.
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// One agent's setup, as the WebUI sees it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SetupState {
    pub agent_id: String,
    pub phase: SetupPhase,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub downloads: Vec<DownloadStatus>,
    /// The owner of the config being built, never sent: it decides who sees the setup of
    /// an agent that is still being created, so not in storage yet.
    #[serde(skip)]
    #[schema(ignore)]
    pub owner_id: Option<String>,
}

/// The answer of `GET /api/v1/utils/setup-state`.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SetupStates {
    pub setups: Vec<SetupState>,
}

#[derive(Clone)]
struct Entry {
    phase: SetupPhase,
    message: String,
    error: Option<String>,
    downloads: Vec<DownloadStatus>,
    owner_id: Option<String>,
    /// How many builds are running for this agent (a create build and a supervisor
    /// retry can overlap). The setup only goes terminal when this reaches zero.
    active_builds: u32,
    /// When the entry went `ready`/`failed`, or a lone download failed. Drives the TTL.
    terminal_at: Option<Instant>,
}

impl Default for Entry {
    fn default() -> Self {
        Self {
            phase: SetupPhase::Starting,
            message: String::new(),
            error: None,
            downloads: Vec::new(),
            owner_id: None,
            active_builds: 0,
            terminal_at: None,
        }
    }
}

/// Shared, in-memory setup state. Cloned onto `VizierDependencies` and read by the API.
#[derive(Clone, Default)]
pub struct SetupBoard(Arc<RwLock<HashMap<String, Entry>>>);

fn upsert(downloads: &mut Vec<DownloadStatus>, kind: DownloadKind) -> &mut DownloadStatus {
    let index = match downloads.iter().position(|d| d.kind == kind) {
        Some(index) => index,
        None => {
            downloads.push(DownloadStatus {
                kind,
                label: String::new(),
                completed: 0,
                total: 0,
                status: String::new(),
                error: None,
            });
            downloads.len() - 1
        }
    };
    &mut downloads[index]
}

/// One running build on the board, from [`SetupBoard::build_started`]. Finish it with
/// [`Self::finished`] or [`Self::failed`]; dropping it unfinished (the build future was
/// cancelled, or it panicked) reports a failure, so the entry can never stay "building".
#[must_use = "dropping the guard reports the build as failed"]
pub struct BuildGuard {
    board: SetupBoard,
    agent_id: String,
    done: bool,
}

impl BuildGuard {
    pub fn finished(mut self) {
        self.done = true;
        self.board.build_finished(&self.agent_id);
    }

    pub fn failed(mut self, error: impl Into<String>) {
        self.done = true;
        self.board.build_failed(&self.agent_id, error);
    }
}

impl Drop for BuildGuard {
    fn drop(&mut self) {
        if !self.done {
            self.board
                .build_failed(&self.agent_id, "the build was stopped before it finished");
        }
    }
}

impl SetupBoard {
    fn with<R>(&self, f: impl FnOnce(&mut HashMap<String, Entry>) -> R) -> R {
        let mut guard = self.0.write().unwrap_or_else(|p| p.into_inner());
        f(&mut guard)
    }

    /// A build for `agent_id` has started. The first one resets its entry; one that overlaps
    /// a build already running only joins it, so it cannot wipe that build's downloads.
    /// `owner_id` is the config's owner (see [`SetupState::owner_id`]).
    pub fn build_started(
        &self,
        agent_id: &str,
        owner_id: Option<String>,
        message: impl Into<String>,
    ) -> BuildGuard {
        self.with(|map| {
            let entry = map.entry(agent_id.to_string()).or_default();
            if owner_id.is_some() {
                entry.owner_id = owner_id;
            }
            if entry.active_builds == 0 {
                entry.phase = SetupPhase::Starting;
                entry.message = message.into();
                entry.error = None;
                entry.downloads.clear();
                entry.terminal_at = None;
            }
            entry.active_builds += 1;
        });
        BuildGuard {
            board: self.clone(),
            agent_id: agent_id.to_string(),
            done: false,
        }
    }

    /// A build finished and the agent is up.
    fn build_finished(&self, agent_id: &str) {
        self.with(|map| {
            if let Some(entry) = map.get_mut(agent_id) {
                entry.active_builds = entry.active_builds.saturating_sub(1);
                if entry.active_builds == 0 {
                    entry.phase = SetupPhase::Ready;
                    entry.message = "Agent ready".to_string();
                    entry.terminal_at = Some(Instant::now());
                }
            }
        });
    }

    /// A build failed to start.
    fn build_failed(&self, agent_id: &str, error: impl Into<String>) {
        self.with(|map| {
            if let Some(entry) = map.get_mut(agent_id) {
                entry.active_builds = entry.active_builds.saturating_sub(1);
                if entry.active_builds == 0 {
                    entry.phase = SetupPhase::Failed;
                    entry.message = "Couldn't start agent".to_string();
                    entry.error = Some(error.into());
                    entry.terminal_at = Some(Instant::now());
                }
            }
        });
    }

    /// A download has begun. `label` is what the UI shows, e.g. the model name.
    pub fn download_started(&self, agent_id: &str, kind: DownloadKind, label: impl Into<String>) {
        let label = label.into();
        self.with(|map| {
            let entry = map.entry(agent_id.to_string()).or_default();
            entry.phase = SetupPhase::Downloading;
            entry.message = label.clone();
            entry.error = None;
            entry.terminal_at = None;
            let download = upsert(&mut entry.downloads, kind);
            download.label = label;
            download.completed = 0;
            download.total = 0;
            download.status = "starting".to_string();
            download.error = None;
        });
    }

    /// Bytes moved. `total == 0` keeps the download indeterminate.
    pub fn download_progress(
        &self,
        agent_id: &str,
        kind: DownloadKind,
        completed: u64,
        total: u64,
        status: impl Into<String>,
    ) {
        let status = status.into();
        self.with(|map| {
            let Some(entry) = map.get_mut(agent_id) else {
                return;
            };
            let label = entry
                .downloads
                .iter()
                .find(|d| d.kind == kind)
                .map(|d| d.label.clone())
                .unwrap_or_default();
            if !label.is_empty() {
                entry.message = label;
            }
            entry.phase = SetupPhase::Downloading;
            let download = upsert(&mut entry.downloads, kind);
            download.completed = completed;
            if total > 0 {
                download.total = total;
            }
            download.status = status;
        });
    }

    /// A download finished successfully.
    pub fn download_finished(&self, agent_id: &str, kind: DownloadKind) {
        self.with(|map| {
            if let Some(entry) = map.get_mut(agent_id) {
                entry.downloads.retain(|d| d.kind != kind);
            }
        });
    }

    /// A download failed. The failure sticks until the TTL, so the UI can show it.
    pub fn download_failed(&self, agent_id: &str, kind: DownloadKind, error: impl Into<String>) {
        let error = error.into();
        self.with(|map| {
            if let Some(entry) = map.get_mut(agent_id) {
                let download = upsert(&mut entry.downloads, kind);
                download.error = Some(error);
                download.status = "failed".to_string();
                entry.terminal_at = Some(Instant::now());
            }
        });
    }

    /// Every setup worth showing right now, dropping the expired ones.
    pub fn snapshot(&self) -> Vec<SetupState> {
        let now = Instant::now();
        self.with(|map| {
            map.retain(|_, entry| {
                let expired = entry
                    .terminal_at
                    .is_some_and(|at| now.duration_since(at) > TERMINAL_TTL);
                // Nothing to show: no build, no download, and not a terminal state still
                // lingering. A `ready`/`failed` entry has `terminal_at` set, so it survives
                // until the TTL instead of being dropped here.
                let idle = entry.downloads.is_empty()
                    && entry.active_builds == 0
                    && entry.terminal_at.is_none();
                !(expired || idle)
            });
            let mut out: Vec<SetupState> = map
                .iter()
                .map(|(agent_id, entry)| SetupState {
                    agent_id: agent_id.clone(),
                    phase: entry.phase,
                    message: entry.message.clone(),
                    error: entry.error.clone(),
                    downloads: entry.downloads.clone(),
                    owner_id: entry.owner_id.clone(),
                })
                .collect();
            out.sort_by(|a, b| a.agent_id.cmp(&b.agent_id));
            out
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dropped_build_reports_failure() {
        let board = SetupBoard::default();
        let guard = board.build_started("a", None, "Building");
        drop(guard);
        let setups = board.snapshot();
        assert_eq!(setups.len(), 1);
        assert_eq!(setups[0].phase, SetupPhase::Failed);
    }

    #[test]
    fn an_overlapping_build_keeps_the_running_downloads() {
        let board = SetupBoard::default();
        let first = board.build_started("a", None, "Building");
        board.download_started("a", DownloadKind::Model, "llama");
        let second = board.build_started("a", None, "Building again");
        assert_eq!(board.snapshot()[0].downloads.len(), 1);

        second.failed("boom");
        assert_ne!(board.snapshot()[0].phase, SetupPhase::Failed);
        board.download_finished("a", DownloadKind::Model);
        first.finished();
        assert_eq!(board.snapshot()[0].phase, SetupPhase::Ready);
    }
}
