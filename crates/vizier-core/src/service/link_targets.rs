//! Read-only name index for the WebUI's `[[kind:name]]` entity wiki links (spec 066):
//! what the renderer checks live/broken against and what the composer suggests from.
//! Names only, capped where a kind can grow (contracts/link-targets-endpoint.md), and
//! composed from the existing per-kind services rather than new queries.

use crate::auth::Principal;
use crate::service::{artifacts, skills, ServiceError, Services};
use crate::storage::{
    agent::AgentStorage, artifact::ArtifactStorage, custom_tool::CustomToolStorage, memory::MemoryStorage,
    task::TaskStorage,
};
use crate::schema::MemoryQueryParams;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The newest artifacts the index carries; older ones stay reachable through [`search`].
const ARTIFACT_CAP: usize = 200;
/// The newest memory documents the index carries.
const DOC_CAP: usize = 500;
/// One search page.
const SEARCH_LIMIT: usize = 50;
/// The resolve batch bound.
const RESOLVE_LIMIT: usize = 100;

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct LinkTargets {
    pub artifacts: Vec<String>,
    pub functions: Vec<String>,
    pub tasks: Vec<TaskTarget>,
    pub skills: Vec<SkillTarget>,
    pub memory: MemoryTargets,
    /// Which kinds were cut by a cap — absence there is not evidence of absence.
    pub truncated: BTreeMap<String, bool>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TaskTarget {
    pub slug: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SkillTarget {
    pub slug: String,
    /// `agent` or `global` — the workspace scopes only (research R4).
    pub scope: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct MemoryTargets {
    pub bundles: Vec<String>,
    pub docs: Vec<DocTarget>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DocTarget {
    pub bundle: String,
    pub path: String,
}

/// One link to settle exactly, keyed by its `kind:name` token in the response.
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ResolveLink {
    pub kind: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ResolveRequest {
    pub links: Vec<ResolveLink>,
}

/// The agent must exist and the caller must be permitted to view it — the same gate as
/// every other agent read route (`service::tasks::view_agent` shape), applied once.
async fn require_view(s: &Services, who: &Principal, agent_id: &str) -> Result<(), ServiceError> {
    let config = match s.deps().storage.get_agent(agent_id).await {
        Ok(Some(config)) => config,
        Ok(None) => return Err(ServiceError::NotFound(format!("agent {agent_id} not found"))),
        Err(e) => return Err(ServiceError::Internal(e.to_string())),
    };
    if !crate::access::can_view_agent(who, &config) {
        return Err(ServiceError::Forbidden("Access denied".into()));
    }
    Ok(())
}

type ArtifactNames = (Vec<String>, bool);

/// Artifact names through the artifact service, which carries the artifact-specific view
/// permission: a caller who cannot view artifacts gets an empty list, not a refusal — the
/// other kinds still resolve for them.
async fn artifacts_of(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    q: Option<String>,
    limit: usize,
) -> Result<ArtifactNames, ServiceError> {
    let query = artifacts::ArtifactQuery { q, kind: None, from: None, to: None, cursor: None, limit: Some(limit) };
    match artifacts::list(s, who, agent_id, query).await {
        Ok(view) => {
            let truncated = view.next.is_some();
            Ok((view.items.into_iter().map(|item| item.name).collect(), truncated))
        }
        Err(ServiceError::Forbidden(_)) => Ok((Vec::new(), false)),
        Err(e) => Err(e),
    }
}

fn contains(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

/// The bounded index (no parameters). Every response stays small however many entities
/// the agent accumulates; `truncated` says which kinds were cut.
pub async fn index(s: &Services, who: &Principal, agent_id: &str) -> Result<LinkTargets, ServiceError> {
    require_view(s, who, agent_id).await?;

    let (artifact_names, artifacts_truncated) = artifacts_of(s, who, agent_id, None, ARTIFACT_CAP).await?;
    let functions = s
        .deps()
        .storage
        .list_custom_tools(agent_id, true)
        .await
        .map_err(|e| ServiceError::Internal(e.to_string()))?
        .into_iter()
        .map(|tool| tool.name)
        .collect();
    let tasks = s
        .deps()
        .storage
        .get_task_list(Some(agent_id.to_string()), None)
        .await
        .map_err(|e| ServiceError::Internal(e.to_string()))?
        .into_iter()
        .map(|task| TaskTarget { slug: task.slug, title: task.title })
        .collect();
    let skills = skills::list_agent(s, who, agent_id, true)
        .await?
        .into_iter()
        .map(|skill| SkillTarget { slug: skill.name, scope: skill.scope.to_string() })
        .collect();
    let bundles = s
        .deps()
        .storage
        .list_bundles(agent_id.to_string())
        .await
        .map_err(|e| ServiceError::Internal(e.to_string()))?
        .into_iter()
        .map(|bundle| bundle.name)
        .collect();
    // Names-only reads at the storage layer: they must not depend on a running agent host
    // (the memory-op service travels the bus), and the list's own default order is
    // `updated_at` desc, so the cap keeps the latest documents.
    let docs_page = s
        .deps()
        .storage
        .get_filtered_memories(MemoryQueryParams {
            agent_id: agent_id.to_string(),
            bundle: None,
            tags: None,
            offset: 0,
            limit: DOC_CAP,
            sort_by: None,
            sort_order: None,
        })
        .await
        .map_err(|e| ServiceError::Internal(e.to_string()))?;
    let docs_truncated = docs_page.total > docs_page.memories.len();

    let mut truncated = BTreeMap::new();
    truncated.insert("artifacts".to_string(), artifacts_truncated);
    truncated.insert("memory.docs".to_string(), docs_truncated);

    Ok(LinkTargets {
        artifacts: artifact_names,
        functions,
        tasks,
        skills,
        memory: MemoryTargets {
            bundles,
            docs: docs_page
                .memories
                .into_iter()
                .map(|doc| DocTarget { bundle: doc.bundle, path: doc.slug })
                .collect(),
        },
        truncated,
    })
}

/// One kind, filtered by a case-insensitive substring — the composer's fallback when the
/// index was truncated or came up empty. Returns the same shape with only that kind filled.
pub async fn search(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    kind: &str,
    q: &str,
    limit: usize,
) -> Result<LinkTargets, ServiceError> {
    require_view(s, who, agent_id).await?;
    let limit = limit.clamp(1, SEARCH_LIMIT);
    let empty = LinkTargets {
        artifacts: Vec::new(),
        functions: Vec::new(),
        tasks: Vec::new(),
        skills: Vec::new(),
        memory: MemoryTargets { bundles: Vec::new(), docs: Vec::new() },
        truncated: BTreeMap::new(),
    };

    match kind {
        "artifact" => {
            let (names, _) = artifacts_of(s, who, agent_id, Some(q.to_string()), limit).await?;
            Ok(LinkTargets { artifacts: names, ..empty })
        }
        "function" => {
            let mut functions = s
                .deps()
                .storage
                .list_custom_tools(agent_id, true)
                .await
                .map_err(|e| ServiceError::Internal(e.to_string()))?
                .into_iter()
                .filter(|tool| contains(&tool.name, q))
                .map(|tool| tool.name)
                .collect::<Vec<_>>();
            functions.truncate(limit);
            Ok(LinkTargets { functions, ..empty })
        }
        "task" => {
            let mut tasks = s
                .deps()
                .storage
                .get_task_list(Some(agent_id.to_string()), None)
                .await
                .map_err(|e| ServiceError::Internal(e.to_string()))?
                .into_iter()
                .filter(|task| contains(&task.slug, q) || contains(&task.title, q))
                .map(|task| TaskTarget { slug: task.slug, title: task.title })
                .collect::<Vec<_>>();
            tasks.truncate(limit);
            Ok(LinkTargets { tasks, ..empty })
        }
        "skill" => {
            let mut matches = skills::list_agent(s, who, agent_id, true)
                .await?
                .into_iter()
                .filter(|skill| contains(&skill.name, q))
                .map(|skill| SkillTarget { slug: skill.name, scope: skill.scope.to_string() })
                .collect::<Vec<_>>();
            matches.truncate(limit);
            Ok(LinkTargets { skills: matches, ..empty })
        }
        "memory" => {
            let bundles = s
                .deps()
                .storage
                .list_bundles(agent_id.to_string())
                .await
                .map_err(|e| ServiceError::Internal(e.to_string()))?
                .into_iter()
                .map(|bundle| bundle.name)
                .filter(|name| contains(name, q))
                .collect::<Vec<_>>();
            let docs = s
                .deps()
                .storage
                .get_filtered_memories(MemoryQueryParams {
                    agent_id: agent_id.to_string(),
                    bundle: None,
                    tags: None,
                    offset: 0,
                    limit: DOC_CAP,
                    sort_by: None,
                    sort_order: None,
                })
                .await
                .map_err(|e| ServiceError::Internal(e.to_string()))?
                .memories
                .into_iter()
                .filter(|doc| contains(&doc.bundle, q) || contains(&doc.slug, q) || contains(&doc.title, q))
                .take(limit)
                .map(|doc| DocTarget { bundle: doc.bundle, path: doc.slug })
                .collect();
            Ok(LinkTargets { memory: MemoryTargets { bundles, docs }, ..empty })
        }
        other => Err(ServiceError::invalid(format!("unknown kind '{other}'"))),
    }
}

/// Exact existence per link (`kind:name` → bool). Unlike the index, artifact and memory
/// checks reach past the caps, so a link renders live/broken correctly however old it is.
pub async fn resolve(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    request: ResolveRequest,
) -> Result<BTreeMap<String, bool>, ServiceError> {
    require_view(s, who, agent_id).await?;
    if request.links.len() > RESOLVE_LIMIT {
        return Err(ServiceError::Unprocessable {
            code: "too_many_links".into(),
            message: format!("at most {RESOLVE_LIMIT} links per resolve, got {}", request.links.len()),
            data: serde_json::json!({ "limit": RESOLVE_LIMIT, "got": request.links.len() }),
        });
    }

    let mut out = BTreeMap::new();
    for link in &request.links {
        let key = format!("{}:{}", link.kind, link.name);
        let exists = match link.kind.as_str() {
            "artifact" => match s.deps().storage.get_artifact_by_name(agent_id, &link.name).await {
                Ok(found) => found.is_some(),
                Err(e) => return Err(ServiceError::Internal(e.to_string())),
            },
            "function" => match s.deps().storage.list_custom_tools(agent_id, true).await {
                Ok(tools) => tools.iter().any(|tool| tool.name == link.name),
                Err(e) => return Err(ServiceError::Internal(e.to_string())),
            },
            "task" => match s.deps().storage.get_task_list(Some(agent_id.to_string()), None).await {
                Ok(tasks) => tasks.iter().any(|task| task.slug == link.name),
                Err(e) => return Err(ServiceError::Internal(e.to_string())),
            },
            "skill" => match skills::list_agent(s, who, agent_id, true).await {
                Ok(skills) => skills.iter().any(|skill| skill.name == link.name),
                Err(_) => false,
            },
            "memory" => {
                if link.name.contains('/') {
                    let (bundle, path) = link.name.split_once('/').unwrap_or((link.name.as_str(), ""));
                    match s.deps().storage.get_memory_detail(agent_id.to_string(), Some(bundle.to_string()), path.to_string()).await {
                        Ok(found) => found.is_some(),
                        Err(e) => return Err(ServiceError::Internal(e.to_string())),
                    }
                } else {
                    match s.deps().storage.list_bundles(agent_id.to_string()).await {
                        Ok(bundles) => bundles.iter().any(|bundle| bundle.name == link.name),
                        Err(e) => return Err(ServiceError::Internal(e.to_string())),
                    }
                }
            }
            // An unknown kind is inert (contracts/link-grammar.md), not an error.
            _ => false,
        };
        out.insert(key, exists);
    }
    Ok(out)
}
