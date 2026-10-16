//! What a model call is for, so `VizierModel` can record it (`specs/018-usage-accounting/` R1),
//! and the guard that marks a turn failed when it ends without an answer (R3).

use std::sync::Arc;

use chrono::Utc;

use crate::{
    schema::{AgentId, VizierSession, usage::UsageActivity},
    storage::{VizierStorage, usage::UsageStorage},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsagePurpose {
    /// One round of a chat, task, background or dream turn.
    Turn,
    /// A checkpoint summary, automatic or manual.
    Handover,
    /// `read_image` on a vision model.
    Vision,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TurnId(String);

impl TurnId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for TurnId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TurnId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Required on every `VizierModel` completion: the compiler finds every call site, and none can
/// forget to say what it is for.
#[derive(Debug, Clone)]
pub struct UsageScope {
    pub session: Option<VizierSession>,
    pub purpose: UsagePurpose,
    pub turn: Option<TurnId>,
    pub round: Option<u32>,
}

impl UsageScope {
    pub fn turn(session: Option<VizierSession>, turn: &TurnId, round: u32) -> Self {
        Self {
            session,
            purpose: UsagePurpose::Turn,
            turn: Some(turn.clone()),
            round: Some(round),
        }
    }

    pub fn handover(session: Option<VizierSession>) -> Self {
        Self {
            session,
            purpose: UsagePurpose::Handover,
            turn: None,
            round: None,
        }
    }

    pub fn vision(session: Option<VizierSession>) -> Self {
        Self {
            session,
            purpose: UsagePurpose::Vision,
            turn: None,
            round: None,
        }
    }

    pub fn activity(&self) -> UsageActivity {
        match (self.purpose, &self.session) {
            (UsagePurpose::Handover, _) => UsageActivity::Compaction,
            (UsagePurpose::Vision, _) => UsageActivity::Vision,
            (UsagePurpose::Turn, Some(session)) => session.1.usage_activity(),
            (UsagePurpose::Turn, None) => UsageActivity::Other,
        }
    }
}

/// Held for the length of a turn. Dropped without [`TurnGuard::succeeded`] — an error, the prompt
/// timeout, or the task being aborted, which drops the future — it records the turn as failed,
/// so the rounds already answered count as spent on a failed turn (FR-004).
pub struct TurnGuard {
    turn: TurnId,
    agent_id: AgentId,
    storage: Arc<VizierStorage>,
    done: bool,
}

impl TurnGuard {
    pub fn new(agent_id: AgentId, storage: Arc<VizierStorage>) -> Self {
        Self {
            turn: TurnId::new(),
            agent_id,
            storage,
            done: false,
        }
    }

    pub fn turn(&self) -> &TurnId {
        &self.turn
    }

    pub fn succeeded(mut self) {
        self.done = true;
    }
}

impl Drop for TurnGuard {
    fn drop(&mut self) {
        if self.done {
            return;
        }
        // A drop while the runtime shuts down has nowhere to write; losing that mark is fine.
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let (turn, agent_id, storage) = (self.turn.clone(), self.agent_id.clone(), self.storage.clone());
        handle.spawn(async move {
            if let Err(e) = storage.record_failed_turn(&turn, &agent_id, Utc::now()).await {
                tracing::warn!(agent_id = %agent_id, "failed to record a failed turn's usage: {e}");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use parking_lot::Mutex;
    use rusqlite::Connection;

    use super::*;
    use crate::storage::{document::LocalDocumentStore, sqlite::SqliteStorage};

    fn storage() -> (Arc<VizierStorage>, Arc<Mutex<Connection>>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let conn = Connection::open_in_memory().unwrap();
        crate::storage::sqlite::init_usage_schema(&conn).unwrap();
        let conn = Arc::new(Mutex::new(conn));
        let storage = VizierStorage::new(SqliteStorage::new(
            conn.clone(),
            Arc::new(LocalDocumentStore::new(dir.path().to_path_buf())),
        ));
        (Arc::new(storage), conn, dir)
    }

    fn failed_turns(conn: &Mutex<Connection>) -> i64 {
        conn.lock()
            .query_row("SELECT COUNT(*) FROM usage_failed_turn", [], |r| r.get(0))
            .unwrap()
    }

    /// The spawned write needs a moment on the runtime; yield until it lands or give up.
    async fn settle(conn: &Mutex<Connection>, want: i64) -> i64 {
        for _ in 0..50 {
            if failed_turns(conn) == want {
                break;
            }
            tokio::task::yield_now().await;
        }
        failed_turns(conn)
    }

    #[tokio::test]
    async fn a_guard_dropped_unanswered_marks_its_turn_failed() {
        let (storage, conn, _d) = storage();
        drop(TurnGuard::new("a".into(), storage.clone()));
        assert_eq!(settle(&conn, 1).await, 1);
    }

    #[tokio::test]
    async fn an_answered_turn_records_nothing() {
        let (storage, conn, _d) = storage();
        TurnGuard::new("a".into(), storage).succeeded();
        assert_eq!(settle(&conn, 1).await, 0);
    }

    #[test]
    fn a_scope_takes_its_activity_from_its_purpose_then_its_channel() {
        use crate::schema::VizierChannelId;
        let task = Some(VizierSession(
            "a".into(),
            VizierChannelId::Task("t".into(), Utc::now()),
            None,
        ));
        assert_eq!(UsageScope::turn(task.clone(), &TurnId::new(), 1).activity(), UsageActivity::Scheduled);
        assert_eq!(UsageScope::handover(task.clone()).activity(), UsageActivity::Compaction);
        assert_eq!(UsageScope::vision(task).activity(), UsageActivity::Vision);
        assert_eq!(UsageScope::turn(None, &TurnId::new(), 1).activity(), UsageActivity::Other);
    }
}
