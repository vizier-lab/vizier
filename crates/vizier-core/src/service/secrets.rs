//! An agent's secrets (`specs/027-agent-secrets/contracts/http-api.md`; spec 038 US2): list, create
//! or replace, delete, and the change log. Every operation is a person managing the agent's
//! store ([`crate::secrets::access`]); no result carries a value — only the server-computed mask.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{
    ServiceError, Services,
    integrations::{agents_by_id, requester_name, usernames},
};
use crate::{
    access::Caller,
    auth::Principal,
    schema::{AgentConfig, AgentId, AgentSecretEvent, Requester, SecretAction},
    secrets::{
        access,
        service::{self as secret_service, SecretError},
    },
    storage::{
        agent::AgentStorage,
        secret::{SecretOutcome, SecretRecord},
    },
};

pub use crate::secrets::access::{MANAGE_ALL, MANAGE_OWNED};

/// One secret as a person sees it: its name, the server's mask, and who changed it when.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SecretItem {
    pub name: String,
    /// `utils::mask_secret` of the stored value. There is no field for the value itself.
    pub masked: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub updated_by: Requester,
    /// The username or the agent's name; the id when it cannot be resolved.
    pub updated_by_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SecretWriteResponse {
    /// `created` or `replaced`.
    pub status: String,
    pub secret: SecretItem,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SetSecretRequest {
    /// The value to store. Never returned by any route.
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SecretDeleted {
    pub name: String,
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SecretEventItem {
    pub id: String,
    pub name: String,
    pub action: SecretAction,
    pub actor: Requester,
    pub actor_name: String,
    pub at: DateTime<Utc>,
}

/// The agent, or the refusal: `404` when it does not exist, `403` when the person is not one of
/// its secret managers. The permission is checked here, before any read or write — hiding the
/// page is not the protection.
async fn manageable_agent(s: &Services, who: &Principal, agent_id: &str) -> Result<AgentConfig, ServiceError> {
    let agent = match s.deps().storage.get_agent(agent_id).await {
        Ok(Some(agent)) => agent,
        Ok(None) => return Err(ServiceError::NotFound("agent not found".into())),
        Err(e) => return Err(ServiceError::Internal(e.to_string())),
    };
    if access::can_manage(Caller::from(who), &agent) {
        Ok(agent)
    } else {
        Err(ServiceError::Forbidden("you may not manage this agent's secrets".into()))
    }
}

fn item(record: SecretRecord, agents: &HashMap<AgentId, AgentConfig>, users: &HashMap<String, String>) -> SecretItem {
    SecretItem {
        updated_by_name: requester_name(&record.updated_by, agents, users),
        name: record.name,
        masked: record.masked,
        created_at: record.created_at,
        updated_at: record.updated_at,
        updated_by: record.updated_by,
    }
}

fn event_item(
    event: AgentSecretEvent,
    agents: &HashMap<AgentId, AgentConfig>,
    users: &HashMap<String, String>,
) -> SecretEventItem {
    SecretEventItem {
        actor_name: requester_name(&event.actor, agents, users),
        id: event.id,
        name: event.name,
        action: event.action,
        actor: event.actor,
        at: event.at,
    }
}

/// A refusal (422 `invalid`, 404 `not_found`) or a real failure (500). The message never
/// quotes a value: validation messages state the rule only.
fn refusal(err: SecretError) -> ServiceError {
    match err {
        SecretError::InvalidName(message) | SecretError::InvalidValue(message) => ServiceError::Invalid {
            status: 422,
            message,
            code: Some("invalid".into()),
        },
        SecretError::NotFound => ServiceError::NotFound("secret not found".into()),
        SecretError::Storage(e) => ServiceError::Internal(e.to_string()),
    }
}

/// The secrets' names, masks and last change, sorted by name. Paging is the adapter's.
pub async fn list(s: &Services, who: &Principal, agent_id: &str) -> Result<Vec<SecretItem>, ServiceError> {
    manageable_agent(s, who, agent_id).await?;
    let records = secret_service::list(&s.deps().storage, agent_id).await.map_err(refusal)?;
    let agents = agents_by_id(s).await;
    let users = usernames(s).await;
    Ok(records.into_iter().map(|record| item(record, &agents, &users)).collect())
}

/// The change log, newest first. A huge or zero limit is clamped to 1–100, not rejected.
pub async fn events(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    limit: Option<usize>,
) -> Result<Vec<SecretEventItem>, ServiceError> {
    manageable_agent(s, who, agent_id).await?;
    let events = secret_service::events(&s.deps().storage, agent_id, limit.unwrap_or(20))
        .await
        .map_err(refusal)?;
    let agents = agents_by_id(s).await;
    let users = usernames(s).await;
    Ok(events.into_iter().map(|event| event_item(event, &agents, &users)).collect())
}

/// The status pair a write reports: a new name is `created`, an existing one `replaced`.
pub fn write_status(outcome: SecretOutcome) -> &'static str {
    match outcome {
        SecretOutcome::Created => "created",
        SecretOutcome::Replaced => "replaced",
    }
}

/// Nothing here restarts, refreshes or signals an agent: a write is a row and an event, and the
/// next retrieval reads it.
pub async fn set(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    name: &str,
    value: &str,
) -> Result<SecretWriteResponse, ServiceError> {
    manageable_agent(s, who, agent_id).await?;
    let actor = Requester::User(who.user_id.clone());
    let write = secret_service::set(&s.deps().storage, agent_id, name, value, &actor)
        .await
        .map_err(refusal)?;
    let agents = agents_by_id(s).await;
    let users = usernames(s).await;
    Ok(SecretWriteResponse {
        status: write_status(write.outcome).to_string(),
        secret: item(write.record, &agents, &users),
    })
}

pub async fn delete(s: &Services, who: &Principal, agent_id: &str, name: &str) -> Result<SecretDeleted, ServiceError> {
    manageable_agent(s, who, agent_id).await?;
    let actor = Requester::User(who.user_id.clone());
    secret_service::delete(&s.deps().storage, agent_id, name, &actor)
        .await
        .map_err(refusal)?;
    Ok(SecretDeleted { name: name.to_string(), deleted: true })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::secret::SecretStorage;

    fn storage() -> (std::sync::Arc<crate::storage::VizierStorage>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::storage::sqlite::init_secret_schema(&conn).unwrap();
        let storage = crate::storage::sqlite::SqliteStorage::new(
            std::sync::Arc::new(parking_lot::Mutex::new(conn)),
            std::sync::Arc::new(crate::storage::document::LocalDocumentStore::new(dir.path().to_path_buf())),
        );
        (std::sync::Arc::new(crate::storage::VizierStorage::new(storage)), dir)
    }

    /// `created` for a new name, `replaced` for an existing one, and the record keeps
    /// `created_at` across the replace.
    #[tokio::test]
    async fn a_write_reports_created_then_replaced() {
        let (storage, _dir) = storage();
        let actor = Requester::User("user-1".into());
        let first = secret_service::set(&storage, "dp-a", "TOKEN", "agent-token-abc123", &actor)
            .await
            .unwrap();
        assert_eq!(first.outcome, SecretOutcome::Created);
        assert_eq!(write_status(first.outcome), "created");
        let second = secret_service::set(&storage, "dp-a", "TOKEN", "rotated-token-xyz789", &actor)
            .await
            .unwrap();
        assert_eq!(second.outcome, SecretOutcome::Replaced);
        assert_eq!(write_status(second.outcome), "replaced");
        assert_eq!(second.record.created_at, first.record.created_at);
        assert_eq!(second.record.masked, "••••z789");
    }

    #[tokio::test]
    async fn a_missing_delete_is_not_found_and_records_no_event() {
        let (storage, _dir) = storage();
        assert!(matches!(
            secret_service::delete(&storage, "dp-a", "NO_SUCH", &Requester::User("u".into())).await,
            Err(SecretError::NotFound)
        ));
        assert!(storage.list_agent_secret_events("dp-a", 20).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn the_event_limit_defaults_and_caps_through_the_service() {
        let (storage, _dir) = storage();
        let actor = Requester::User("user-1".into());
        for name in ["A", "B", "C"] {
            secret_service::set(&storage, "dp-a", name, "value-00000001", &actor).await.unwrap();
        }
        assert_eq!(secret_service::events(&storage, "dp-a", 20).await.unwrap().len(), 3);
        assert_eq!(secret_service::events(&storage, "dp-a", 1000).await.unwrap().len(), 3);
        assert_eq!(secret_service::events(&storage, "dp-a", 1).await.unwrap().len(), 1);
    }

    /// Validation is `422`, a missing name `404`, and a storage failure `500`.
    #[test]
    fn refusals_map_to_422_404_and_500() {
        assert_eq!(refusal(SecretError::InvalidName("no".into())).status(), 422);
        assert_eq!(refusal(SecretError::InvalidValue("no".into())).status(), 422);
        assert_eq!(refusal(SecretError::NotFound).status(), 404);
        assert_eq!(
            refusal(SecretError::Storage(crate::error::VizierError("no".into()).into())).status(),
            500
        );
    }
}
