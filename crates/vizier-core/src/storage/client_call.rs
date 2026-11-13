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
    ///
    /// Fenced: refused with `OwnershipLost` when a newer owner holds `agent:{agent_id}`
    /// (specs/057 contracts/ownership.md).
    async fn open_client_call(&self, uid: &str, session: &VizierSession) -> Result<()>;

    /// Record how a call ended. Only a `pending` or `deferred` call changes, so a late write
    /// never overrides an outcome already recorded; returns whether this one applied.
    ///
    /// Fenced: refused with `OwnershipLost` when a newer owner holds `agent:{agent_id}`
    /// (specs/057 contracts/ownership.md).
    async fn finish_client_call(&self, uid: &str, outcome: &ClientCallOutcome) -> Result<bool>;

    /// Replace the call's saved state, whatever its status.
    ///
    /// Fenced: refused with `OwnershipLost` when a newer owner holds `agent:{agent_id}`
    /// (specs/057 contracts/ownership.md).
    async fn save_client_call_state(&self, uid: &str, state: &serde_json::Value) -> Result<()>;

    /// Startup sweep: a call cannot outlive the process awaiting it, so every `pending` one
    /// becomes `interrupted`.
    async fn interrupt_pending_client_calls(&self) -> Result<usize>;

    /// Recovery of `agent:{agent_id}`: its `pending` calls whose owner is older than `below` (or
    /// null) become `interrupted` and are re-owned by the held fence, in the same statement.
    /// `deferred` calls are left as they are. Returns how many.
    ///
    /// Fenced: refused with `OwnershipLost` when a newer owner holds `agent:{agent_id}`.
    async fn interrupt_expired_client_calls(&self, agent_id: &str, below: u64) -> Result<usize>;

    /// `pending → deferred`: the call waits for a client to reconnect (research R18).
    ///
    /// Fenced: refused with `OwnershipLost` when a newer owner holds `agent:{agent_id}`
    /// (specs/057 contracts/ownership.md).
    async fn defer_client_call(&self, uid: &str) -> Result<bool>;

    /// Take the claim on a deferred call: exactly one gateway delivers it at a time.
    ///
    /// The claim is a lease on the stored row, **not** a new status: [`ClientCallOutcome`]
    /// stays the final-outcome vocabulary the WebUI reads, and an unknown status reads as
    /// `interrupted` (`crates/vizier-core/src/schema/history.rs`). One conditional update
    /// decides the winner, so exactly one caller's claim changes the row; returns whether
    /// this caller won it.
    ///
    /// A claim older than `timeout` has lapsed - the gateway that held it died - so the
    /// call is free again and the row stays `deferred`.
    async fn claim_client_call(
        &self,
        uid: &str,
        claimant: &str,
        timeout: chrono::Duration,
    ) -> Result<bool>;

    /// Give the claim on a deferred call back, so the next gateway can deliver it.
    async fn release_client_call_claim(&self, uid: &str, claimant: &str) -> Result<bool>;

    /// The session's deferred calls, oldest first.
    async fn deferred_client_calls(&self, session: &VizierSession) -> Result<Vec<String>>;
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

    async fn interrupt_expired_client_calls(&self, agent_id: &str, below: u64) -> Result<usize> {
        self.0.interrupt_expired_client_calls(agent_id, below).await
    }

    async fn defer_client_call(&self, uid: &str) -> Result<bool> {
        self.0.defer_client_call(uid).await
    }

    async fn claim_client_call(
        &self,
        uid: &str,
        claimant: &str,
        timeout: chrono::Duration,
    ) -> Result<bool> {
        self.0.claim_client_call(uid, claimant, timeout).await
    }

    async fn release_client_call_claim(&self, uid: &str, claimant: &str) -> Result<bool> {
        self.0.release_client_call_claim(uid, claimant).await
    }

    async fn deferred_client_calls(&self, session: &VizierSession) -> Result<Vec<String>> {
        self.0.deferred_client_calls(session).await
    }
}
