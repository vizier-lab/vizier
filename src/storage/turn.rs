//! Accepted work that has not finished (`specs/023-persist-turn-activity`): one `session_turn`
//! row per request from `send_request` until its turn ends, so a restart can pick it up again.

use anyhow::Result;

use crate::schema::{VizierRequest, VizierSession};
use crate::storage::VizierStorage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnState {
    Queued,
    Running,
}

#[derive(Debug, Clone)]
pub struct PendingTurn {
    pub id: i64,
    pub session: VizierSession,
    pub request: VizierRequest,
    pub state: TurnState,
    pub request_uid: Option<String>,
}

#[async_trait::async_trait]
pub trait TurnStorage {
    /// Journal an accepted request as `queued`; the row id is the order of acceptance.
    async fn accept_turn(&self, session: &VizierSession, request: &VizierRequest) -> Result<i64>;

    /// The turn has begun to run.
    async fn start_turn(&self, id: i64) -> Result<()>;

    /// The history uid of the turn's saved `Request` entry.
    async fn set_turn_request_uid(&self, id: i64, uid: &str) -> Result<()>;

    /// A resume of the turn is about to run, with `steps` entries after its request. Returns
    /// the new count of resumes without progress: reset to 1 when `steps` grew since the last.
    async fn note_resume(&self, id: i64, steps: usize) -> Result<u32>;

    /// The turn ended in any way: delete its row.
    async fn finish_turn(&self, id: i64) -> Result<()>;

    /// Delete every row of a session (an abort).
    async fn cancel_session_turns(&self, session: &VizierSession) -> Result<usize>;

    /// An agent's rows, in the order they were accepted.
    async fn pending_turns(&self, agent_id: &str) -> Result<Vec<PendingTurn>>;

    /// Every session that has a row, for the startup clean-up.
    async fn sessions_with_pending_turns(&self) -> Result<Vec<VizierSession>>;
}

#[async_trait::async_trait]
impl TurnStorage for VizierStorage {
    async fn accept_turn(&self, session: &VizierSession, request: &VizierRequest) -> Result<i64> {
        self.0.accept_turn(session, request).await
    }

    async fn start_turn(&self, id: i64) -> Result<()> {
        self.0.start_turn(id).await
    }

    async fn set_turn_request_uid(&self, id: i64, uid: &str) -> Result<()> {
        self.0.set_turn_request_uid(id, uid).await
    }

    async fn note_resume(&self, id: i64, steps: usize) -> Result<u32> {
        self.0.note_resume(id, steps).await
    }

    async fn finish_turn(&self, id: i64) -> Result<()> {
        self.0.finish_turn(id).await
    }

    async fn cancel_session_turns(&self, session: &VizierSession) -> Result<usize> {
        self.0.cancel_session_turns(session).await
    }

    async fn pending_turns(&self, agent_id: &str) -> Result<Vec<PendingTurn>> {
        self.0.pending_turns(agent_id).await
    }

    async fn sessions_with_pending_turns(&self) -> Result<Vec<VizierSession>> {
        self.0.sessions_with_pending_turns().await
    }
}
