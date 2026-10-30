//! An agent's memory: concept documents, bundles, graph, search, attachments and version
//! history (spec 038, US2). Operations go through the agent's memory-op channel on the
//! transport, as the HTTP handlers did.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    access::{Caller, can_view_agent},
    artifacts::access as artifacts_access,
    auth::Principal,
    schema::{
        AgentConfig, BundleSummary, ImportReport, Memory, MemoryGraph, MemoryOpRequest,
        MemoryOpResponse, MemoryPassageResult, MemoryQueryParams, MemoryRevision,
        PaginatedMemoryRevisions, RevisionDiff, RevisionOrigin, RevisionTrigger, RollbackResponse,
        VizierAttachment, default_bundle,
    },
    service::{ServiceError, Services},
    storage::{agent::AgentStorage, artifact::ArtifactStorage},
};

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateMemoryRequest {
    pub title: String,
    pub content: String,
    #[serde(default)]
    pub bundle: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub attachments: Option<Vec<VizierAttachment>>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UpdateMemoryRequest {
    pub title: String,
    pub content: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub attachments: Option<Vec<VizierAttachment>>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct QueryMemoryRequest {
    pub query: String,
    #[serde(default)]
    pub bundle: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default = "default_threshold")]
    pub threshold: f64,
}

fn default_limit() -> usize {
    10
}

fn default_threshold() -> f64 {
    0.5
}

#[derive(Debug, Deserialize)]
pub struct ListMemoryParams {
    #[serde(default)]
    pub bundle: Option<String>,
    #[serde(default)]
    pub tags: Option<String>,
    #[serde(default = "default_list_offset")]
    pub offset: usize,
    #[serde(default = "default_list_limit")]
    pub limit: usize,
    #[serde(default)]
    pub sort_by: Option<String>,
    #[serde(default)]
    pub sort_order: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GraphQueryParams {
    #[serde(default)]
    pub search: Option<String>,
}

fn default_list_offset() -> usize {
    0
}

fn default_list_limit() -> usize {
    50
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct MemorySummary {
    pub agent_id: String,
    pub bundle: String,
    pub path: String,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub tags: Vec<String>,
    pub relations: Vec<String>,
    pub attachment_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct MemoryDetail {
    pub agent_id: String,
    pub bundle: String,
    pub path: String,
    pub title: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub tags: Vec<String>,
    pub relations: Vec<String>,
    pub attachments: Vec<crate::schema::MemoryAttachmentView>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateMemoryResponse {
    pub agent_id: String,
    pub bundle: String,
    pub title: String,
    pub path: String,
    pub message: String,
    pub tags: Vec<String>,
    pub relations: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UpdateMemoryResponse {
    pub agent_id: String,
    pub bundle: String,
    pub path: String,
    pub message: String,
    pub tags: Vec<String>,
    pub relations: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct PaginatedMemoryResponse {
    pub memories: Vec<MemorySummary>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AttachArtifactRequest {
    pub artifact: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::IntoParams))]
pub struct DetachAttachmentQuery {
    pub name: String,
}

#[derive(Debug, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DeleteBundleParams {
    /// Delete the bundle even if it still contains concept documents (permanently deleting
    /// them too). Reserved for an operator's explicit, confirmed action — the
    /// `memory_delete_bundle` agent tool never sets this.
    #[serde(default)]
    pub force: bool,
}

/// One `GET /history/{bundle}/{*path}` dispatches on which query params are present:
/// `seq` ⇒ one version; `to` (± `from`) ⇒ diff; otherwise a paginated list. The sub-operation
/// lives in the query rather than the path because `*path` is greedy.
#[derive(Debug, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::IntoParams))]
pub struct MemoryHistoryQuery {
    pub offset: Option<usize>,
    pub limit: Option<usize>,
    pub seq: Option<i64>,
    pub from: Option<i64>,
    pub to: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct MemoryRollbackRequest {
    pub seq: i64,
}

/// The three shapes `GET /history/...` can answer with, depending on the query.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(untagged)]
pub enum MemoryHistoryResponse {
    List(PaginatedMemoryRevisions),
    Revision(MemoryRevision),
    Diff(RevisionDiff),
}

/// A bundle export: the `.zip` bytes and the file name to offer them as.
pub struct BundleExport {
    pub bytes: Vec<u8>,
    pub filename: String,
}

fn summarize_memory(memory: &Memory) -> MemorySummary {
    MemorySummary {
        agent_id: memory.agent_id.clone(),
        bundle: memory.bundle.clone(),
        path: memory.slug.clone(),
        title: memory.title.clone(),
        created_at: memory.created_at,
        updated_at: memory.updated_at,
        tags: memory.tags.clone(),
        relations: memory.relations.clone(),
        attachment_count: memory.attachment_count,
    }
}

async fn detail_from_memory(s: &Services, memory: &Memory) -> MemoryDetail {
    use crate::schema::{MemoryAttachment, MemoryAttachmentView, VizierAttachmentContent};
    let deps = s.deps();
    let mut attachments = Vec::with_capacity(memory.attachments.len());
    for attachment in &memory.attachments {
        match attachment {
            MemoryAttachment::Artifact { artifact_id, name } => {
                let artifact = deps
                    .storage
                    .get_artifact(&memory.agent_id, artifact_id)
                    .await
                    .ok()
                    .flatten();
                let available = artifact.is_some()
                    && deps
                        .artifacts
                        .path_for(&memory.agent_id, artifact_id)
                        .ok()
                        .flatten()
                        .is_some();
                attachments.push(MemoryAttachmentView::Artifact {
                    artifact_id: artifact_id.clone(),
                    name: name.clone(),
                    mime_type: artifact.as_ref().map(|a| a.mime_type.clone()),
                    size: artifact.as_ref().map(|a| a.size),
                    available,
                });
            }
            MemoryAttachment::LegacySessionFile(legacy) => {
                let available = match &legacy.content {
                    VizierAttachmentContent::Local(url) => {
                        let file_id = url.trim_start_matches("/api/v1/files/");
                        deps.file_manager.path_of(file_id).await.is_ok()
                    }
                    _ => true,
                };
                attachments.push(MemoryAttachmentView::LegacySessionFile {
                    filename: legacy.filename.clone(),
                    content: legacy.content.clone(),
                    available,
                });
            }
        }
    }
    MemoryDetail {
        agent_id: memory.agent_id.clone(),
        bundle: memory.bundle.clone(),
        path: memory.slug.clone(),
        title: memory.title.clone(),
        content: memory.content.clone(),
        created_at: memory.created_at,
        updated_at: memory.updated_at,
        tags: memory.tags.clone(),
        relations: memory.relations.clone(),
        attachments,
    }
}

/// The status a failed memory operation maps to, from its message.
fn op_error(msg: String) -> ServiceError {
    let lower = msg.to_lowercase();
    if lower.contains("already exists") {
        ServiceError::Conflict { message: msg, code: None, data: None }
    } else if lower.contains("malformed") || lower.contains("archive") {
        ServiceError::Invalid { status: 400, message: msg, code: None }
    } else if lower.contains("does not exist") || lower.contains("not found") {
        ServiceError::NotFound(msg)
    } else if lower.contains("linked in the knowledge graph") || lower.contains("still has") {
        ServiceError::Conflict { message: msg, code: None, data: None }
    } else if lower.contains("unknown version") {
        ServiceError::NotFound(msg)
    } else if lower.contains("deletion entry") {
        ServiceError::Invalid { status: 400, message: msg, code: None }
    } else {
        ServiceError::Internal(msg)
    }
}

fn unexpected() -> ServiceError {
    ServiceError::Internal("unexpected response".into())
}

async fn require_agent(
    s: &Services,
    who: &Principal,
    agent_id: &str,
) -> Result<AgentConfig, ServiceError> {
    let config = match s.deps().storage.get_agent(agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return Err(ServiceError::NotFound(format!("agent {agent_id} not found"))),
        Err(e) => return Err(ServiceError::Internal(e.to_string())),
    };
    if !can_view_agent(who, &config) {
        return Err(ServiceError::Forbidden("Access denied".into()));
    }
    Ok(config)
}

async fn require_memory_and_artifact_view(
    s: &Services,
    who: &Principal,
    agent_id: &str,
) -> Result<AgentConfig, ServiceError> {
    let config = require_agent(s, who, agent_id).await?;
    if !artifacts_access::can_view(&Caller::from(who), &config) {
        return Err(ServiceError::Forbidden(
            "You do not have permission to view this agent's artifacts".into(),
        ));
    }
    Ok(config)
}

async fn op(s: &Services, agent_id: &str, op: MemoryOpRequest) -> anyhow::Result<MemoryOpResponse> {
    s.deps().transport.send_memory_op(&agent_id.to_string(), op).await
}

fn clean_graph_search(params: GraphQueryParams) -> Option<String> {
    params
        .search
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn history_path(raw: &str) -> String {
    raw.trim_end_matches(".md").to_string()
}

// ---- attachments ----

pub async fn attach_artifact(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    bundle: String,
    path: &str,
    body: AttachArtifactRequest,
) -> Result<MemoryDetail, ServiceError> {
    require_memory_and_artifact_view(s, who, agent_id).await?;
    let path = path.trim_end_matches(".md").to_string();
    let artifact = match s.deps().storage.get_artifact_by_name(agent_id, &body.artifact).await {
        Ok(Some(artifact)) => artifact,
        Ok(None) => {
            return Err(ServiceError::NotFound(format!(
                "artifact '{}' not found",
                body.artifact
            )));
        }
        Err(e) => return Err(ServiceError::Internal(e.to_string())),
    };
    let request = MemoryOpRequest::AttachArtifact {
        bundle: Some(bundle),
        path,
        artifact_id: artifact.id,
        artifact_name: artifact.name,
        origin: RevisionOrigin::from_user(who),
    };
    match op(s, agent_id, request).await {
        Ok(MemoryOpResponse::Memory(memory)) => Ok(detail_from_memory(s, &memory).await),
        Ok(_) => Err(unexpected()),
        Err(e) => Err(attachment_error(e.to_string())),
    }
}

pub async fn detach_attachment(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    bundle: String,
    path: &str,
    query: DetachAttachmentQuery,
) -> Result<MemoryDetail, ServiceError> {
    require_memory_and_artifact_view(s, who, agent_id).await?;
    let request = MemoryOpRequest::DetachAttachment {
        bundle: Some(bundle),
        path: path.trim_end_matches(".md").to_string(),
        name: query.name,
        origin: RevisionOrigin::from_user(who),
    };
    match op(s, agent_id, request).await {
        Ok(MemoryOpResponse::Memory(memory)) => Ok(detail_from_memory(s, &memory).await),
        Ok(_) => Err(unexpected()),
        Err(e) => Err(attachment_error(e.to_string())),
    }
}

fn attachment_error(msg: String) -> ServiceError {
    if msg.contains("not found") {
        ServiceError::NotFound(msg)
    } else {
        op_error(msg)
    }
}

// ---- list / search / graph ----

pub async fn list(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    params: ListMemoryParams,
) -> Result<PaginatedMemoryResponse, ServiceError> {
    require_agent(s, who, agent_id).await?;

    let tags = params.tags.map(|t| {
        t.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
    });
    let query_params = MemoryQueryParams {
        agent_id: agent_id.to_string(),
        bundle: params.bundle,
        tags,
        offset: params.offset,
        limit: params.limit,
        sort_by: params.sort_by,
        sort_order: params.sort_order,
    };

    match op(s, agent_id, MemoryOpRequest::List { params: query_params }).await {
        Ok(MemoryOpResponse::Paginated(result)) => Ok(PaginatedMemoryResponse {
            memories: result.memories.iter().map(summarize_memory).collect(),
            total: result.total,
            offset: result.offset,
            limit: result.limit,
        }),
        Ok(_) => Err(unexpected()),
        Err(e) => Err(ServiceError::Internal(e.to_string())),
    }
}

pub async fn create(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    body: CreateMemoryRequest,
) -> Result<CreateMemoryResponse, ServiceError> {
    require_agent(s, who, agent_id).await?;
    if body.attachments.as_ref().is_some_and(|list| !list.is_empty()) {
        return Err(ServiceError::Invalid {
            status: 422,
            message: "memory attachments must be artifacts; use POST /agents/{agent_id}/memory/doc/{bundle}/{path}/attachments".into(),
            code: None,
        });
    }

    let request = MemoryOpRequest::Write {
        bundle: body.bundle.clone(),
        path: body.path,
        create_only: true,
        title: body.title.clone(),
        content: body.content,
        tags: body.tags.clone(),
        origin: RevisionOrigin::from_user(who),
    };
    match op(s, agent_id, request).await {
        Ok(MemoryOpResponse::Memory(memory)) => Ok(CreateMemoryResponse {
            agent_id: agent_id.to_string(),
            bundle: memory.bundle,
            title: body.title,
            path: memory.slug,
            message: "memory created successfully".to_string(),
            tags: memory.tags,
            relations: memory.relations,
        }),
        Ok(_) => Err(unexpected()),
        Err(e) => Err(op_error(e.to_string())),
    }
}

/// Saves a memory; `bundle` `None` means the default bundle.
pub async fn update(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    bundle: Option<String>,
    path: String,
    body: UpdateMemoryRequest,
) -> Result<UpdateMemoryResponse, ServiceError> {
    require_agent(s, who, agent_id).await?;
    if body.attachments.as_ref().is_some_and(|list| !list.is_empty()) {
        return Err(ServiceError::Invalid {
            status: 422,
            message: "memory attachments must be artifacts; use POST /agents/{agent_id}/memory/doc/{bundle}/{path}/attachments".into(),
            code: None,
        });
    }
    let request = MemoryOpRequest::Write {
        bundle,
        path: Some(path),
        create_only: false,
        title: body.title,
        content: body.content,
        tags: body.tags.clone(),
        origin: RevisionOrigin::from_user(who),
    };
    match op(s, agent_id, request).await {
        Ok(MemoryOpResponse::Memory(memory)) => Ok(UpdateMemoryResponse {
            agent_id: agent_id.to_string(),
            bundle: memory.bundle,
            path: memory.slug,
            message: "memory updated successfully".to_string(),
            tags: memory.tags,
            relations: memory.relations,
        }),
        Ok(_) => Err(unexpected()),
        Err(e) => Err(op_error(e.to_string())),
    }
}

pub async fn query(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    params: QueryMemoryRequest,
) -> Result<Vec<MemoryPassageResult>, ServiceError> {
    require_agent(s, who, agent_id).await?;
    let request = MemoryOpRequest::Query {
        bundle: params.bundle,
        query: params.query,
        limit: params.limit,
        threshold: params.threshold,
    };
    match op(s, agent_id, request).await {
        // Passage results, carrying the same addresses as the agent-facing `memory_search`.
        Ok(MemoryOpResponse::PassageList(passages)) => Ok(passages),
        Ok(_) => Err(unexpected()),
        Err(e) => Err(ServiceError::Internal(e.to_string())),
    }
}

/// The graph of one bundle, or across bundles when `bundle` is `None`.
pub async fn graph(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    bundle: Option<String>,
    params: GraphQueryParams,
) -> Result<MemoryGraph, ServiceError> {
    require_agent(s, who, agent_id).await?;
    let search = clean_graph_search(params);
    match op(s, agent_id, MemoryOpRequest::GetGraph { bundle, search }).await {
        Ok(MemoryOpResponse::Graph(graph)) => Ok(graph),
        Ok(_) => Err(unexpected()),
        Err(e) => Err(op_error(e.to_string())),
    }
}

pub async fn detail(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    bundle: Option<String>,
    path: String,
) -> Result<MemoryDetail, ServiceError> {
    require_agent(s, who, agent_id).await?;
    match op(s, agent_id, MemoryOpRequest::GetById { bundle, path }).await {
        Ok(MemoryOpResponse::MemoryOption(Some(memory))) => Ok(detail_from_memory(s, &memory).await),
        Ok(MemoryOpResponse::MemoryOption(None)) => Err(ServiceError::NotFound("Not Found".into())),
        Ok(_) => Err(unexpected()),
        Err(e) => Err(ServiceError::Internal(e.to_string())),
    }
}

pub async fn related(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    bundle: Option<String>,
    path: String,
) -> Result<Vec<MemoryDetail>, ServiceError> {
    require_agent(s, who, agent_id).await?;
    match op(s, agent_id, MemoryOpRequest::GetRelated { bundle, path }).await {
        Ok(MemoryOpResponse::MemoryList(memories)) => {
            let mut response = Vec::with_capacity(memories.len());
            for memory in &memories {
                response.push(detail_from_memory(s, memory).await);
            }
            Ok(response)
        }
        Ok(_) => Err(unexpected()),
        Err(e) => Err(ServiceError::Internal(e.to_string())),
    }
}

/// Deletes a memory. Returns the confirmation message.
pub async fn delete(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    bundle: Option<String>,
    path: String,
) -> Result<String, ServiceError> {
    require_agent(s, who, agent_id).await?;
    let request = MemoryOpRequest::Delete {
        bundle,
        path: path.clone(),
        origin: RevisionOrigin::from_user(who),
    };
    match op(s, agent_id, request).await {
        Ok(MemoryOpResponse::Unit) => Ok(format!("{path} deleted")),
        Ok(_) => Err(unexpected()),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("linked in the knowledge graph") {
                Err(ServiceError::Conflict { message: msg, code: None, data: None })
            } else {
                Err(ServiceError::NotFound(msg))
            }
        }
    }
}

// ---- bundles ----

pub async fn list_bundles(
    s: &Services,
    who: &Principal,
    agent_id: &str,
) -> Result<Vec<BundleSummary>, ServiceError> {
    require_agent(s, who, agent_id).await?;
    match op(s, agent_id, MemoryOpRequest::ListBundles).await {
        Ok(MemoryOpResponse::Bundles(bundles)) => Ok(bundles),
        Ok(_) => Err(unexpected()),
        Err(e) => Err(ServiceError::Internal(e.to_string())),
    }
}

pub async fn delete_bundle(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    bundle: String,
    params: DeleteBundleParams,
) -> Result<String, ServiceError> {
    require_agent(s, who, agent_id).await?;
    let request = MemoryOpRequest::DeleteBundle {
        bundle: bundle.clone(),
        force: params.force,
        origin: RevisionOrigin::from_user(who),
    };
    match op(s, agent_id, request).await {
        Ok(MemoryOpResponse::Unit) => Ok(format!("bundle '{bundle}' deleted")),
        Ok(_) => Err(unexpected()),
        Err(e) => Err(op_error(e.to_string())),
    }
}

pub async fn export_bundle(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    bundle: String,
) -> Result<BundleExport, ServiceError> {
    require_agent(s, who, agent_id).await?;
    match op(s, agent_id, MemoryOpRequest::ExportBundle { bundle: bundle.clone() }).await {
        Ok(MemoryOpResponse::Export(bytes)) => Ok(BundleExport {
            bytes,
            filename: format!("{bundle}.zip"),
        }),
        Ok(_) => Err(unexpected()),
        Err(e) => Err(op_error(e.to_string())),
    }
}

/// Imports a `.zip` into `bundle` (the default bundle when `None`).
pub async fn import_bundle(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    bundle: Option<String>,
    zip_bytes: Vec<u8>,
) -> Result<ImportReport, ServiceError> {
    require_agent(s, who, agent_id).await?;
    let bundle = bundle.unwrap_or_else(default_bundle);
    let request = MemoryOpRequest::ImportBundle {
        bundle,
        zip_bytes,
        origin: RevisionOrigin::from_user(who).with_trigger(RevisionTrigger::Import),
    };
    match op(s, agent_id, request).await {
        Ok(MemoryOpResponse::Import(report)) => Ok(report),
        Ok(_) => Err(unexpected()),
        Err(e) => Err(op_error(e.to_string())),
    }
}

/// The agent check `import_bundle`'s caller runs before it reads the upload, so a refused
/// person's body is never parsed.
pub async fn authorize(s: &Services, who: &Principal, agent_id: &str) -> Result<(), ServiceError> {
    require_agent(s, who, agent_id).await.map(|_| ())
}

// ---- version history ----

pub async fn history(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    bundle: String,
    path: &str,
    q: MemoryHistoryQuery,
) -> Result<MemoryHistoryResponse, ServiceError> {
    require_agent(s, who, agent_id).await?;
    let path = history_path(path);
    let bundle = Some(bundle);

    let request = if let Some(seq) = q.seq {
        MemoryOpRequest::GetRevision { bundle, path, seq }
    } else if let Some(to) = q.to {
        MemoryOpRequest::DiffRevisions { bundle, path, from: q.from, to }
    } else {
        MemoryOpRequest::ListRevisions {
            bundle,
            path,
            offset: q.offset.unwrap_or(0),
            limit: q.limit.unwrap_or(50),
        }
    };

    match op(s, agent_id, request).await {
        Ok(MemoryOpResponse::Revisions(page)) if page.total == 0 => {
            Err(ServiceError::NotFound("memory has no history".into()))
        }
        Ok(MemoryOpResponse::Revisions(page)) => Ok(MemoryHistoryResponse::List(page)),
        Ok(MemoryOpResponse::Revision(Some(rev))) => Ok(MemoryHistoryResponse::Revision(rev)),
        Ok(MemoryOpResponse::Revision(None)) => Err(ServiceError::NotFound("unknown version".into())),
        Ok(MemoryOpResponse::Diff(diff)) => Ok(MemoryHistoryResponse::Diff(diff)),
        Ok(_) => Err(unexpected()),
        Err(e) => Err(op_error(e.to_string())),
    }
}

pub async fn rollback(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    bundle: String,
    path: &str,
    body: MemoryRollbackRequest,
) -> Result<RollbackResponse, ServiceError> {
    require_agent(s, who, agent_id).await?;
    let request = MemoryOpRequest::Rollback {
        bundle: Some(bundle),
        path: history_path(path),
        seq: body.seq,
        origin: RevisionOrigin::from_user(who),
    };
    match op(s, agent_id, request).await {
        Ok(MemoryOpResponse::Rollback(res)) => Ok(res),
        Ok(_) => Err(unexpected()),
        Err(e) => Err(op_error(e.to_string())),
    }
}
