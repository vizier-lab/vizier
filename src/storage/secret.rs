//! An agent's secret store (`specs/027-agent-secrets/data-model.md` §4–5): named values owned
//! by exactly one agent, plus an append-only change log that never holds a value.
//!
//! `get_agent_secret` is the only method that returns a value. Everything a list or set call
//! may see is a [`SecretRecord`], which carries the masked form and has no value field at all.

use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::{schema::AgentSecretEvent, schema::Requester, storage::VizierStorage};

/// What a set did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretOutcome {
    Created,
    Replaced,
}

/// A secret as list/set responses may see it: never the value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretRecord {
    pub name: String,
    /// `utils::mask_secret` of the stored value.
    pub masked: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub updated_by: Requester,
}

/// The record plus whether the write created or replaced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretWrite {
    pub outcome: SecretOutcome,
    pub record: SecretRecord,
}

#[async_trait::async_trait]
pub trait SecretStorage {
    /// Create or replace one value, and record the event, in one transaction.
    async fn set_agent_secret(
        &self,
        agent_id: &str,
        name: &str,
        value: &str,
        actor: &Requester,
    ) -> Result<SecretWrite>;

    /// The only method that returns a stored value.
    async fn get_agent_secret(&self, agent_id: &str, name: &str) -> Result<Option<String>>;

    /// Metadata for every secret of one agent, sorted by name. No values.
    async fn list_agent_secrets(&self, agent_id: &str) -> Result<Vec<SecretRecord>>;

    /// Delete one value and record the event when a row existed.
    async fn delete_agent_secret(
        &self,
        agent_id: &str,
        name: &str,
        actor: &Requester,
    ) -> Result<bool>;

    /// Newest first, capped by `limit`.
    async fn list_agent_secret_events(
        &self,
        agent_id: &str,
        limit: usize,
    ) -> Result<Vec<AgentSecretEvent>>;
}

#[async_trait::async_trait]
impl SecretStorage for VizierStorage {
    async fn set_agent_secret(
        &self,
        agent_id: &str,
        name: &str,
        value: &str,
        actor: &Requester,
    ) -> Result<SecretWrite> {
        self.0.set_agent_secret(agent_id, name, value, actor).await
    }

    async fn get_agent_secret(&self, agent_id: &str, name: &str) -> Result<Option<String>> {
        self.0.get_agent_secret(agent_id, name).await
    }

    async fn list_agent_secrets(&self, agent_id: &str) -> Result<Vec<SecretRecord>> {
        self.0.list_agent_secrets(agent_id).await
    }

    async fn delete_agent_secret(
        &self,
        agent_id: &str,
        name: &str,
        actor: &Requester,
    ) -> Result<bool> {
        self.0.delete_agent_secret(agent_id, name, actor).await
    }

    async fn list_agent_secret_events(
        &self,
        agent_id: &str,
        limit: usize,
    ) -> Result<Vec<AgentSecretEvent>> {
        self.0.list_agent_secret_events(agent_id, limit).await
    }
}
