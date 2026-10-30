//! An agent's workspace documents (`SOUL.md`, `IDENTITY.md`, `HEARTBEAT.md`) (spec 038, US2).
//!
//! The HTTP routes for these are not mounted today; the operations are here so any adapter can
//! offer them.

use serde::{Deserialize, Serialize};

use crate::{
    access::can_view_agent,
    auth::Principal,
    service::{ServiceError, Services},
    storage::agent::AgentStorage,
    utils::build_path,
};

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UpdateDocumentRequest {
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DocumentContentResponse {
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DocumentUpdateResponse {
    pub message: String,
}

/// The documents an agent keeps in its workspace folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Document {
    Soul,
    Identity,
    Heartbeat,
}

impl Document {
    pub fn file_name(self) -> &'static str {
        match self {
            Self::Soul => "SOUL.md",
            Self::Identity => "IDENTITY.md",
            Self::Heartbeat => "HEARTBEAT.md",
        }
    }
}

async fn authorize(s: &Services, who: &Principal, agent_id: &str) -> Result<(), ServiceError> {
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

fn path_of(s: &Services, agent_id: &str, doc: Document) -> String {
    build_path(&s.deps().config.workspace, &["agents", agent_id, doc.file_name()])
        .to_string_lossy()
        .to_string()
}

pub async fn get(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    doc: Document,
) -> Result<DocumentContentResponse, ServiceError> {
    authorize(s, who, agent_id).await?;
    match std::fs::read_to_string(path_of(s, agent_id, doc)) {
        Ok(content) => Ok(DocumentContentResponse { content }),
        Err(e) => Err(ServiceError::NotFound(format!("failed to read {}: {}", doc.file_name(), e))),
    }
}

pub async fn update(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    doc: Document,
    body: UpdateDocumentRequest,
) -> Result<DocumentUpdateResponse, ServiceError> {
    authorize(s, who, agent_id).await?;
    match std::fs::write(path_of(s, agent_id, doc), &body.content) {
        Ok(_) => Ok(DocumentUpdateResponse {
            message: format!("{} updated successfully", doc.file_name()),
        }),
        Err(e) => Err(ServiceError::Internal(format!(
            "failed to update {}: {}",
            doc.file_name(),
            e
        ))),
    }
}
