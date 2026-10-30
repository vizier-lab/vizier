//! The artifact gallery (`specs/035-add-artifacts`, contracts/artifacts-api.md).
//!
//! Reads (list/detail/content/activity) need artifact **view**; delete needs **manage**. Both
//! come from [`crate::artifacts::access`], on top of being able to see the agent.

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use serde::Serialize;

use super::{ServiceError, Services};
use crate::{
    access::{Caller, can_view_agent},
    artifacts::access as artifact_access,
    auth::Principal,
    schema::{
        AgentConfig, Artifact, ArtifactCursor, ArtifactDeleteResult, ArtifactEvent, ArtifactFilters,
        ArtifactKind, ArtifactPage, ArtifactReference, Requester,
    },
    storage::agent::AgentStorage,
};

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ArtifactItemView {
    pub id: String,
    pub agent_id: String,
    pub name: String,
    pub kind: ArtifactKind,
    pub mime_type: String,
    pub size: u64,
    pub version: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub available: bool,
}

impl ArtifactItemView {
    fn new(artifact: &Artifact, available: bool) -> Self {
        Self {
            id: artifact.id.clone(),
            agent_id: artifact.agent_id.clone(),
            name: artifact.name.clone(),
            kind: artifact.kind,
            mime_type: artifact.mime_type.clone(),
            size: artifact.size,
            version: artifact.version,
            created_at: artifact.created_at,
            updated_at: artifact.updated_at,
            available,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ArtifactListView {
    pub items: Vec<ArtifactItemView>,
    pub next: Option<String>,
    pub total: usize,
    pub total_bytes: u64,
    pub agent_total: usize,
    pub agent_total_bytes: u64,
}

fn list_view(page: ArtifactPage, artifacts: &crate::artifacts::Artifacts) -> ArtifactListView {
    let items = page
        .items
        .iter()
        .map(|artifact| ArtifactItemView::new(artifact, artifacts.is_available(&artifact.agent_id, artifact)))
        .collect();
    ArtifactListView {
        items,
        next: page.next,
        total: page.total,
        total_bytes: page.total_bytes,
        agent_total: page.agent_total,
        agent_total_bytes: page.agent_total_bytes,
    }
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ArtifactReferenceView {
    pub kind: String,
    pub label: String,
    pub href: Option<String>,
    pub created_at: DateTime<Utc>,
}

fn reference_view(reference: &ArtifactReference) -> ArtifactReferenceView {
    let href = match reference.kind.as_str() {
        "message" => reference
            .topic
            .as_ref()
            .map(|topic| format!("/{}/chat/{}", reference.agent_id, topic)),
        "memory" => Some(format!(
            "/{}/memory/{}/{}",
            reference.agent_id,
            reference.bundle.as_deref().unwrap_or("default"),
            reference.path.as_deref().unwrap_or("")
        )),
        _ => None,
    };
    ArtifactReferenceView {
        kind: reference.kind.clone(),
        label: reference.label.clone(),
        href,
        created_at: reference.created_at,
    }
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ArtifactEventView {
    pub id: String,
    pub artifact_id: String,
    pub artifact_name: String,
    pub action: crate::schema::ArtifactEventAction,
    pub actor: Requester,
    pub actor_name: String,
    pub source_channel: Option<String>,
    pub source_topic: Option<String>,
    pub source_filename: Option<String>,
    pub created_at: DateTime<Utc>,
}

async fn event_view(s: &Services, event: &ArtifactEvent) -> ArtifactEventView {
    ArtifactEventView {
        id: event.id.clone(),
        artifact_id: event.artifact_id.clone(),
        artifact_name: event.artifact_name.clone(),
        action: event.action,
        actor: event.actor.clone(),
        actor_name: crate::utils::actor_display_name(&s.deps().storage, &event.actor).await,
        source_channel: event.source_channel.clone(),
        source_topic: event.source_topic.clone(),
        source_filename: event.source_filename.clone(),
        created_at: event.created_at,
    }
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ArtifactDetailView {
    pub artifact: ArtifactItemView,
    pub origin: crate::schema::ArtifactOrigin,
    pub available: bool,
    pub references: Vec<ArtifactReferenceView>,
    pub events: Vec<ArtifactEventView>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ArtifactActivityView {
    pub items: Vec<ArtifactEventView>,
    pub next: Option<String>,
}

/// The gallery's filters, as the query string carries them.
#[derive(Debug, Default, Clone)]
pub struct ArtifactQuery {
    pub q: Option<String>,
    pub kind: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<usize>,
}

/// An artifact's current bytes with what a download needs to describe them.
#[derive(Debug, Clone)]
pub struct ArtifactContent {
    pub bytes: Vec<u8>,
    pub mime_type: String,
    pub name: String,
}

/// Resolve the agent and check artifact view permission.
async fn require_view(s: &Services, who: &Principal, agent_id: &str) -> Result<AgentConfig, ServiceError> {
    let config = match s.deps().storage.get_agent(agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return Err(ServiceError::NotFound(format!("agent {agent_id} not found"))),
        Err(e) => return Err(ServiceError::Internal(e.to_string())),
    };
    if !can_view_agent(who, &config) {
        return Err(ServiceError::Forbidden("Access denied".into()));
    }
    if !artifact_access::can_view(&Caller::from(who), &config) {
        return Err(ServiceError::Forbidden(
            "You do not have permission to view this agent's artifacts".into(),
        ));
    }
    Ok(config)
}

async fn require_manage(s: &Services, who: &Principal, agent_id: &str) -> Result<AgentConfig, ServiceError> {
    let config = require_view(s, who, agent_id).await?;
    if !artifact_access::can_manage(&Caller::from(who), &config) {
        return Err(ServiceError::Forbidden(
            "You do not have permission to manage this agent's artifacts".into(),
        ));
    }
    Ok(config)
}

fn parse_day(raw: &str, end: bool) -> Option<DateTime<Utc>> {
    let date = NaiveDate::parse_from_str(raw, "%Y-%m-%d").ok()?;
    let time = if end {
        date.and_hms_milli_opt(23, 59, 59, 999)?
    } else {
        date.and_hms_opt(0, 0, 0)?
    };
    Some(Utc.from_utc_datetime(&time))
}

fn cursor_of(raw: Option<&str>) -> Result<Option<ArtifactCursor>, ServiceError> {
    match raw.filter(|c| !c.is_empty()) {
        Some(raw) => ArtifactCursor::decode(raw)
            .map(Some)
            .ok_or_else(|| ServiceError::invalid("malformed cursor")),
        None => Ok(None),
    }
}

pub async fn list(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    query: ArtifactQuery,
) -> Result<ArtifactListView, ServiceError> {
    require_view(s, who, agent_id).await?;
    let kind = match query.kind.as_deref().filter(|k| !k.is_empty()) {
        Some(kind) => match ArtifactKind::parse(kind) {
            Some(kind) => Some(kind),
            None => return Err(ServiceError::invalid(format!("unknown kind '{kind}'"))),
        },
        None => None,
    };
    let cursor = cursor_of(query.cursor.as_deref())?;
    let from = match query.from.as_deref().filter(|d| !d.is_empty()) {
        Some(raw) => Some(parse_day(raw, false).ok_or_else(|| ServiceError::invalid("malformed from date"))?),
        None => None,
    };
    let to = match query.to.as_deref().filter(|d| !d.is_empty()) {
        Some(raw) => Some(parse_day(raw, true).ok_or_else(|| ServiceError::invalid("malformed to date"))?),
        None => None,
    };
    let filters = ArtifactFilters {
        agent_id: agent_id.to_string(),
        q: query.q,
        kind,
        from,
        to,
        cursor,
        limit: query.limit.unwrap_or(50).clamp(1, 100),
    };
    match s.deps().artifacts.list(filters).await {
        Ok(page) => Ok(list_view(page, &s.deps().artifacts)),
        Err(e) => Err(ServiceError::Internal(e.to_string())),
    }
}

pub async fn activity(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    cursor: Option<&str>,
    limit: Option<usize>,
) -> Result<ArtifactActivityView, ServiceError> {
    require_view(s, who, agent_id).await?;
    let cursor = cursor_of(cursor)?;
    let limit = limit.unwrap_or(50).clamp(1, 100);
    match s.deps().artifacts.activity(agent_id, cursor, limit).await {
        Ok(events) => {
            let next = if events.len() == limit {
                events
                    .last()
                    .map(|event| ArtifactCursor::new(event.created_at, &event.id).encode())
            } else {
                None
            };
            let mut items = Vec::with_capacity(events.len());
            for event in &events {
                items.push(event_view(s, event).await);
            }
            Ok(ArtifactActivityView { items, next })
        }
        Err(e) => Err(ServiceError::Internal(e.to_string())),
    }
}

pub async fn detail(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    artifact_id: &str,
) -> Result<ArtifactDetailView, ServiceError> {
    require_view(s, who, agent_id).await?;
    let detail = match s.deps().artifacts.detail(agent_id, artifact_id).await {
        Ok(Some(detail)) => detail,
        Ok(None) => return Err(ServiceError::NotFound("artifact not found".into())),
        Err(e) => return Err(ServiceError::Internal(e.to_string())),
    };
    let mut events = Vec::with_capacity(detail.events.len());
    for event in &detail.events {
        events.push(event_view(s, event).await);
    }
    Ok(ArtifactDetailView {
        artifact: ArtifactItemView::new(&detail.artifact, detail.available),
        origin: detail.artifact.origin(),
        available: detail.available,
        references: detail.references.iter().map(reference_view).collect(),
        events,
    })
}

pub async fn content(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    artifact_id: &str,
) -> Result<ArtifactContent, ServiceError> {
    require_view(s, who, agent_id).await?;
    let detail = match s.deps().artifacts.detail(agent_id, artifact_id).await {
        Ok(Some(detail)) => detail,
        Ok(None) => return Err(ServiceError::NotFound("artifact not found".into())),
        Err(e) => return Err(ServiceError::Internal(e.to_string())),
    };
    let bytes = s
        .deps()
        .artifacts
        .read_content(agent_id, artifact_id)
        .await
        .map_err(|_| ServiceError::NotFound("artifact content is unavailable".into()))?;
    Ok(ArtifactContent {
        bytes,
        mime_type: detail.artifact.mime_type,
        name: detail.artifact.name,
    })
}

pub async fn delete(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    artifact_id: &str,
) -> Result<ArtifactDeleteResult, ServiceError> {
    require_manage(s, who, agent_id).await?;
    match s
        .deps()
        .artifacts
        .delete(agent_id, artifact_id, Requester::User(who.user_id.clone()))
        .await
    {
        Ok(result) => Ok(result),
        Err(e) => {
            let message = e.to_string();
            if message.contains("not found") {
                Err(ServiceError::NotFound(message))
            } else {
                Err(ServiceError::Internal(message))
            }
        }
    }
}
