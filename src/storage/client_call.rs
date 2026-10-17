//! The outcome and saved state of client tool calls (`specs/019-client-tools` research R8).
//!
//! A call's history entry is written once, when the call is made; everything that changes
//! after that lives here, keyed by the entry's uid (which is the call id), and is filled
//! into the entry on every history read.

use anyhow::Result;

use crate::{
    schema::{ClientCallOutcome, VizierSession},
    storage::VizierStorage,
};

#[async_trait::async_trait]
pub trait ClientCallStorage {
    /// Record a call as `pending`, when it is made.
    async fn open_client_call(&self, uid: &str, session: &VizierSession) -> Result<()>;

    /// Record how a call ended. Only a `pending` call changes, so a late write never
    /// overrides an outcome already recorded; returns whether this one applied.
    async fn finish_client_call(&self, uid: &str, outcome: &ClientCallOutcome) -> Result<bool>;

    /// Replace the call's saved state, whatever its status.
    async fn save_client_call_state(&self, uid: &str, state: &serde_json::Value) -> Result<()>;

    /// Startup sweep: a call cannot outlive the process awaiting it, so every `pending` one
    /// becomes `interrupted`.
    async fn interrupt_pending_client_calls(&self) -> Result<usize>;
}

#[async_trait::async_trait]
impl ClientCallStorage for VizierStorage {
    async fn open_client_call(&self, uid: &str, session: &VizierSession) -> Result<()> {
        self.0.open_client_call(uid, session).await
    }

    async fn finish_client_call(&self, uid: &str, outcome: &ClientCallOutcome) -> Result<bool> {
        self.0.finish_client_call(uid, outcome).await
    }

    async fn save_client_call_state(&self, uid: &str, state: &serde_json::Value) -> Result<()> {
        self.0.save_client_call_state(uid, state).await
    }

    async fn interrupt_pending_client_calls(&self) -> Result<usize> {
        self.0.interrupt_pending_client_calls().await
    }
}
