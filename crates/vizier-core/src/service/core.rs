//! An agent's CORE document: read, save (with a stale-revision guard), version history and
//! rollback.
//!
//! **Deprecated** (specs/043-unify-core-and-memory, FR-034). Identity is the memory document
//! `core/soul` now, and the memory endpoints are where it is read and written. This module is
//! kept working, with its response shapes unchanged, because removing it is a separate
//! decision that has not been taken.
//!
//! It is a **projection**, not a second store: every operation here goes through the memory
//! operations for that one document (FR-035). A projection cannot disagree with its source, so
//! the deprecated surface and the memory surfaces can never hold different identity — which is
//! what SC-013 measures. `seq` is the `memory_revision` ordinal of `core/soul`, so a client
//! that stores it and sends it back as `base_seq` still gets the stale-revision conflict it
//! expects.

use serde::{Deserialize, Serialize};

use crate::{
    access::can_view_agent,
    auth::Principal,
    schema::{
        CORE_BUNDLE, CoreRevision, MemoryOpRequest, MemoryOpResponse, MemoryRevision,
        PaginatedCoreRevisions, RevisionDiff, RevisionOrigin, RollbackResponse, SOUL_PATH,
    },
    service::{ServiceError, Services, memory},
    storage::{agent::AgentStorage, memory::MemoryStorage},
};

/// The title the document is written under when this surface creates it. The memory surface
/// uses whatever the caller supplies; this is only what a client that never learned about
/// titles gets.
const CORE_TITLE: &str = "Core";

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UpdateCoreRequest {
    pub content: String,
    /// The revision this edit started from (`seq` of `GET /core`). When the identity document
    /// has moved on, the save is refused with `409 stale_revision` instead of overwriting the
    /// newer text.
    #[serde(default)]
    pub base_seq: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CoreContentResponse {
    pub content: String,
    /// The current revision, to send back as `base_seq`. Absent when there is no history yet.
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
/// `seq` means one version; `to` (with optional `from`) means a diff; otherwise a paginated
/// list.
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

/// The identity document's address, as a `(bundle, path)` pair.
fn address() -> (Option<String>, String) {
    (Some(CORE_BUNDLE.to_string()), SOUL_PATH.to_string())
}

/// The newest revision's `seq` of the identity document, if it has any history.
async fn latest_seq(s: &Services, agent_id: &str) -> Option<i64> {
    let (bundle, path) = address();
    s.deps()
        .storage
        .list_memory_revisions(agent_id.to_string(), bundle, path, 0, 1)
        .await
        .ok()
        .and_then(|page| page.revisions.first().map(|r| r.seq))
}

/// The document's content, through the memory surface. Empty when it is absent or has been
/// deleted — a real state, because deleting the soul is allowed.
async fn current_content(s: &Services, agent_id: &str) -> String {
    let (bundle, path) = address();
    match memory::op(
        s,
        agent_id,
        MemoryOpRequest::GetById { bundle, path },
    )
    .await
    {
        Ok(MemoryOpResponse::MemoryOption(Some(memory))) => memory.content,
        _ => String::new(),
    }
}

/// A memory revision, in the shape this deprecated surface has always answered with. A deletion
/// entry projects as empty content: this shape has no `deleted` flag, and the content is gone.
fn project_revision(rev: MemoryRevision) -> CoreRevision {
    CoreRevision {
        seq: rev.seq,
        actor: rev.actor,
        trigger: rev.trigger,
        created_at: rev.created_at,
        is_current: rev.is_current,
        size_bytes: rev.size_bytes,
        content: rev.content.unwrap_or_default(),
    }
}

fn history_error(message: String) -> ServiceError {
    if message.contains("unknown version") {
        ServiceError::NotFound(message)
    } else if message.contains("deletion entry") {
        ServiceError::Invalid {
            status: 400,
            message,
            code: None,
        }
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
    Ok(CoreContentResponse {
        content: current_content(s, agent_id).await,
        seq,
    })
}

/// Saves the identity document. Returns `{ "message", "seq" }`.
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
        return Err(ServiceError::Conflict {
            message: "stale_revision".into(),
            code: None,
            data: serde_json::to_value(CoreConflict {
                latest_seq: latest,
                content: current_content(s, agent_id).await,
            })
            .ok(),
        });
    }

    let (bundle, path) = address();
    match memory::op(
        s,
        agent_id,
        MemoryOpRequest::Write {
            bundle,
            path: Some(path),
            create_only: false,
            title: CORE_TITLE.into(),
            content: body.content,
            tags: vec![],
            origin: RevisionOrigin::from_user(who),
        },
    )
    .await
    {
        Ok(MemoryOpResponse::Written { .. }) => Ok(serde_json::json!({
            "message": "CORE updated successfully",
            "seq": latest_seq(s, agent_id).await
        })),
        Ok(_) => Err(ServiceError::Internal("unexpected response".into())),
        // The write path is the same one the memory surface uses, so its refusals (over the
        // identity ceiling, for instance) arrive here with their own status rather than as a
        // 500 that hides the reason.
        Err(e) => Err(memory::op_error(e.to_string())),
    }
}

pub async fn history(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    q: CoreHistoryQuery,
) -> Result<CoreHistoryResponse, ServiceError> {
    require_viewable_agent(s, who, agent_id).await?;
    let (bundle, path) = address();

    if let Some(seq) = q.seq {
        return match memory::op(
            s,
            agent_id,
            MemoryOpRequest::GetRevision { bundle, path, seq },
        )
        .await
        {
            Ok(MemoryOpResponse::Revision(Some(rev))) => {
                Ok(CoreHistoryResponse::Revision(project_revision(rev)))
            }
            Ok(MemoryOpResponse::Revision(None)) => {
                Err(ServiceError::NotFound(format!("unknown version {seq}")))
            }
            Ok(_) => Err(ServiceError::Internal("unexpected response".into())),
            Err(e) => Err(history_error(e.to_string())),
        };
    }

    if let Some(to) = q.to {
        return match memory::op(
            s,
            agent_id,
            MemoryOpRequest::DiffRevisions {
                bundle,
                path,
                from: q.from,
                to,
            },
        )
        .await
        {
            Ok(MemoryOpResponse::Diff(diff)) => Ok(CoreHistoryResponse::Diff(diff)),
            Ok(_) => Err(ServiceError::Internal("unexpected response".into())),
            Err(e) => Err(history_error(e.to_string())),
        };
    }

    match memory::op(
        s,
        agent_id,
        MemoryOpRequest::ListRevisions {
            bundle,
            path,
            offset: q.offset.unwrap_or(0),
            limit: q.limit.unwrap_or(50),
        },
    )
    .await
    {
        Ok(MemoryOpResponse::Revisions(page)) if page.total == 0 => {
            Err(ServiceError::NotFound("agent has no CORE".into()))
        }
        Ok(MemoryOpResponse::Revisions(page)) => Ok(CoreHistoryResponse::List(
            PaginatedCoreRevisions {
                revisions: page
                    .revisions
                    .into_iter()
                    .map(|r| crate::schema::CoreRevisionSummary {
                        seq: r.seq,
                        actor: r.actor,
                        trigger: r.trigger,
                        created_at: r.created_at,
                        is_current: r.is_current,
                        size_bytes: r.size_bytes,
                    })
                    .collect(),
                total: page.total,
                offset: page.offset,
                limit: page.limit,
            },
        )),
        Ok(_) => Err(ServiceError::Internal("unexpected response".into())),
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
    let (bundle, path) = address();

    match memory::op(
        s,
        agent_id,
        MemoryOpRequest::Rollback {
            bundle,
            path,
            seq: body.seq,
            origin: RevisionOrigin::from_user(who),
        },
    )
    .await
    {
        Ok(MemoryOpResponse::Rollback(res)) => Ok(res),
        Ok(_) => Err(ServiceError::Internal("unexpected response".into())),
        Err(e) => Err(history_error(e.to_string())),
    }
}
