//! An agent's CORE document: read, save (with a stale-revision guard), version history and
//! rollback (spec 038, US2).

use serde::{Deserialize, Serialize};

use crate::{
    access::can_view_agent,
    auth::Principal,
    schema::{
        CoreRevision, PaginatedCoreRevisions, RevisionDiff, RevisionOrigin, RollbackResponse,
    },
    service::{ServiceError, Services},
    storage::agent::AgentStorage,
};

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UpdateCoreRequest {
    pub content: String,
    /// The revision this edit started from (`seq` of `GET /core`). When CORE has moved on, the
    /// save is refused with `409 stale_revision` instead of overwriting the newer text.
    #[serde(default)]
    pub base_seq: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CoreContentResponse {
    pub content: String,
    /// The current revision, to send back as `base_seq`. Absent when CORE has no history yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seq: Option<i64>,
}

/// `data` of a `409 stale_revision`.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CoreConflict {
    pub latest_seq: i64,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CoreUpdateResponse {
    pub message: String,
}

/// One `GET /history` dispatches on which query params are present:
/// `seq` ⇒ one version; `to` (± `from`) ⇒ diff; otherwise a paginated list.
#[derive(Debug, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::IntoParams))]
pub struct CoreHistoryQuery {
    pub offset: Option<usize>,
    pub limit: Option<usize>,
    pub seq: Option<i64>,
    pub from: Option<i64>,
    pub to: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct RollbackRequest {
    pub seq: i64,
}

/// The three shapes `GET /history` can answer with, depending on the query.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(untagged)]
pub enum CoreHistoryResponse {
    List(PaginatedCoreRevisions),
    Revision(CoreRevision),
    Diff(RevisionDiff),
}

async fn require_viewable_agent(
    s: &Services,
    who: &Principal,
    agent_id: &str,
) -> Result<(), ServiceError> {
    let config = match s.deps().storage.get_agent(agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return Err(ServiceError::NotFound(format!("agent {agent_id} not found"))),
        Err(e) => return Err(ServiceError::Internal(e.to_string())),
    };
    if !can_view_agent(who, &config) {
        return Err(ServiceError::Forbidden("Access denied".into()));
    }
    Ok(())
}

/// The newest CORE revision's `seq`, if CORE has any history.
async fn latest_seq(s: &Services, agent_id: &str) -> Option<i64> {
    s.deps()
        .storage
        .list_core_revisions(agent_id, 0, 1)
        .await
        .ok()
        .and_then(|page| page.revisions.first().map(|r| r.seq))
}

fn history_error(message: String) -> ServiceError {
    if message.contains("unknown version") {
        ServiceError::NotFound(message)
    } else if message.contains("deletion entry") {
        ServiceError::Invalid { status: 400, message, code: None }
    } else {
        ServiceError::Internal(message)
    }
}

pub async fn get(
    s: &Services,
    who: &Principal,
    agent_id: &str,
) -> Result<CoreContentResponse, ServiceError> {
    require_viewable_agent(s, who, agent_id).await?;

    let seq = latest_seq(s, agent_id).await;
    match s.deps().storage.get_agent_core(agent_id).await {
        Ok(Some(content)) => Ok(CoreContentResponse { content, seq }),
        Ok(None) => Ok(CoreContentResponse { content: String::new(), seq }),
        Err(e) => Err(ServiceError::Internal(format!("failed to read CORE: {}", e))),
    }
}

/// Saves CORE. Returns `{ "message", "seq" }`.
pub async fn update(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    body: UpdateCoreRequest,
) -> Result<serde_json::Value, ServiceError> {
    require_viewable_agent(s, who, agent_id).await?;

    if let Some(base) = body.base_seq
        && let Some(latest) = latest_seq(s, agent_id).await
        && latest != base
    {
        let content = s
            .deps()
            .storage
            .get_agent_core(agent_id)
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
        return Err(ServiceError::Conflict {
            message: "stale_revision".into(),
            code: None,
            data: serde_json::to_value(CoreConflict { latest_seq: latest, content }).ok(),
        });
    }

    match s
        .deps()
        .storage
        .set_agent_core(agent_id, &body.content, &RevisionOrigin::from_user(who))
        .await
    {
        Ok(_) => Ok(serde_json::json!({
            "message": "CORE updated successfully",
            "seq": latest_seq(s, agent_id).await
        })),
        Err(e) => Err(ServiceError::Internal(format!("failed to update CORE: {}", e))),
    }
}

pub async fn history(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    q: CoreHistoryQuery,
) -> Result<CoreHistoryResponse, ServiceError> {
    require_viewable_agent(s, who, agent_id).await?;
    let storage = &s.deps().storage;

    if let Some(seq) = q.seq {
        return match storage.get_core_revision(agent_id, seq).await {
            Ok(Some(rev)) => Ok(CoreHistoryResponse::Revision(rev)),
            Ok(None) => Err(ServiceError::NotFound(format!("unknown version {seq}"))),
            Err(e) => Err(ServiceError::Internal(e.to_string())),
        };
    }

    if let Some(to) = q.to {
        return match storage.diff_core_revisions(agent_id, q.from, to).await {
            Ok(diff) => Ok(CoreHistoryResponse::Diff(diff)),
            Err(e) => Err(history_error(e.to_string())),
        };
    }

    match storage
        .list_core_revisions(agent_id, q.offset.unwrap_or(0), q.limit.unwrap_or(50))
        .await
    {
        Ok(page) if page.total == 0 => Err(ServiceError::NotFound("agent has no CORE".into())),
        Ok(page) => Ok(CoreHistoryResponse::List(page)),
        Err(e) => Err(ServiceError::Internal(e.to_string())),
    }
}

pub async fn rollback(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    body: RollbackRequest,
) -> Result<RollbackResponse, ServiceError> {
    require_viewable_agent(s, who, agent_id).await?;

    s.deps()
        .storage
        .rollback_core(agent_id, body.seq, &RevisionOrigin::from_user(who))
        .await
        .map_err(|e| history_error(e.to_string()))
}
