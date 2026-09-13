use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use slugify::slugify;

use crate::agents::tools::{ToolContext, VizierTool};
use crate::error::VizierError;
use crate::indexer::VizierIndexer;
use crate::schema::{AgentId, VizierAttachment, VizierAttachmentContent};
use crate::storage::VizierStorage;
use crate::storage::memory::MemoryStorage;
use crate::storage::session_file::SessionFileStorage;
use crate::utils::get_mime_type;

pub fn init_vector_memory(
    agent_id: String,
    storage: Arc<VizierStorage>,
    indexer: VizierIndexer,
) -> Result<(
    MemoryRead,
    MemoryWrite,
    MemoryList,
    MemoryDetail,
    MemoryFollow,
    MemoryGraphTool,
    MemoryDelete,
    MemoryDeleteBundle,
)> {
    Ok((
        MemoryRead::new(agent_id.clone(), storage.clone(), indexer.clone()),
        MemoryWrite::new(agent_id.clone(), storage.clone(), indexer.clone()),
        MemoryList::new(agent_id.clone(), storage.clone()),
        MemoryDetail::new(agent_id.clone(), storage.clone()),
        MemoryFollow::new(agent_id.clone(), storage.clone()),
        MemoryGraphTool::new(agent_id.clone(), storage.clone()),
        MemoryDelete::new(agent_id.clone(), storage.clone(), indexer.clone()),
        MemoryDeleteBundle::new(agent_id.clone(), storage.clone(), indexer),
    ))
}

const BUNDLE_FIELD_DESC: &str = "Bundle name. Bundles are named containers for related memories (e.g. one per project or person) — omit this to use your default bundle; naming a new bundle creates it automatically.";

pub type MemoryRead = ReadVectorMemory;
pub struct ReadVectorMemory(AgentId, Arc<VizierStorage>, VizierIndexer);

impl MemoryRead {
    fn new(agent_id: AgentId, store: Arc<VizierStorage>, indexer: VizierIndexer) -> Self {
        Self(agent_id, store, indexer)
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct MemoryListArgs {
    #[schemars(
        description = "Bundle to focus on. Omit to see the top-level list of your bundles (name, concept count, last updated); name one to list its concepts instead (flattened across any nesting, paginated by limit/offset)."
    )]
    #[serde(default)]
    pub bundle: Option<String>,

    #[schemars(
        description = "Maximum number of memories to return (only applies when bundle is set)"
    )]
    #[serde(default = "default_limit")]
    pub limit: Option<usize>,

    #[schemars(description = "Number of memories to skip (only applies when bundle is set)")]
    #[serde(default = "default_offset")]
    pub offset: Option<usize>,
}

fn default_limit() -> Option<usize> {
    Some(50)
}

fn default_offset() -> Option<usize> {
    Some(0)
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct BundleSummaryOutput {
    pub name: String,
    pub concept_count: usize,
    pub updated_at: Option<chrono::DateTime<Utc>>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct MemorySummary {
    pub bundle: String,
    pub path: String,
    pub title: String,
    pub updated_at: chrono::DateTime<Utc>,
    pub tags: Vec<String>,
    pub relations: Vec<String>,
    pub attachment_count: usize,
    pub read_count: u64,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum MemoryListOutput {
    /// The top-level view (`bundle` omitted): one summary row per bundle.
    Bundles(Vec<BundleSummaryOutput>),
    /// A focused view (`bundle` named): that bundle's concepts.
    Concepts(Vec<MemorySummary>),
}

pub type MemoryList = ListVectorMemory;
pub struct ListVectorMemory(AgentId, Arc<VizierStorage>);

impl MemoryList {
    fn new(agent_id: AgentId, store: Arc<VizierStorage>) -> Self {
        Self(agent_id, store)
    }
}

#[async_trait::async_trait]
impl VizierTool for MemoryList {
    type Input = MemoryListArgs;
    type Output = MemoryListOutput;

    fn name() -> String {
        "memory_list".to_string()
    }

    fn description(&self) -> String {
        "Browse your memory. Called with no bundle, returns the top-level list of your bundles \
        (name, concept count, last updated) — the same zoom level as memory_graph() with no bundle. \
        Called with a bundle name, lists that bundle's concepts (flattened across any nested \
        subdirectories, paginated). Use memory_detail to read a concept's full content, or \
        memory_read to search across everything at once instead of browsing."
            .into()
    }

    async fn call(
        &self,
        args: Self::Input,
        _ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        match args.bundle {
            None => {
                let bundles = self
                    .1
                    .list_bundles(self.0.clone())
                    .await
                    .map_err(|err| VizierError(err.to_string()))?;
                Ok(MemoryListOutput::Bundles(
                    bundles
                        .into_iter()
                        .map(|b| BundleSummaryOutput {
                            name: b.name,
                            concept_count: b.concept_count,
                            updated_at: b.updated_at,
                        })
                        .collect(),
                ))
            }
            Some(bundle) => {
                let limit = args.limit.unwrap_or(50);
                let offset = args.offset.unwrap_or(0);

                let all_memory = self
                    .1
                    .get_all_agent_memory(self.0.clone(), Some(bundle))
                    .await
                    .map_err(|err| VizierError(err.to_string()))?;

                Ok(MemoryListOutput::Concepts(
                    all_memory
                        .into_iter()
                        .skip(offset)
                        .take(limit)
                        .map(|m| MemorySummary {
                            bundle: m.bundle,
                            path: m.slug,
                            title: m.title,
                            updated_at: m.updated_at,
                            tags: m.tags,
                            relations: m.relations,
                            attachment_count: m.attachment_count,
                            read_count: m.read_count,
                        })
                        .collect(),
                ))
            }
        }
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct MemoryReadArgs {
    #[schemars(description = "Terms, keywords, or prompt to search")]
    pub query: String,

    #[schemars(
        description = "Narrow the search to one bundle. Omit to search across all of your bundles at once, ranked by relevance regardless of which bundle a match lives in — the useful default, since a bundle is just an organizational container."
    )]
    #[serde(default)]
    pub bundle: Option<String>,
}

#[async_trait::async_trait]
impl VizierTool for MemoryRead {
    type Input = MemoryReadArgs;
    type Output = Vec<String>;

    fn name() -> String {
        "memory_read".to_string()
    }

    fn description(&self) -> String {
        "Semantic search across your memories (all bundles by default, or one named bundle). \
        Returns content that matches the query. Memory content may contain [label](path/to/concept.md) \
        same-bundle links or [[bundle/slug]] / [[bundle]] cross-bundle links — use memory_detail or \
        memory_follow to explore them.".into()
    }

    async fn call(
        &self,
        args: Self::Input,
        _ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let res = self
            .1
            .query_memory(
                self.0.clone(),
                args.bundle.clone(),
                args.query,
                10,
                0.1,
                &self.2,
            )
            .await
            .map_err(|err| VizierError(err.to_string()))?;

        for memory in &res {
            let _ = self
                .1
                .increment_read_count(
                    self.0.clone(),
                    Some(memory.bundle.clone()),
                    memory.slug.clone(),
                )
                .await;
        }

        Ok(res.iter().map(|memory| memory.content.clone()).collect())
    }
}

pub type MemoryWrite = WriteVectorMemory;
pub struct WriteVectorMemory(AgentId, Arc<VizierStorage>, VizierIndexer);

impl MemoryWrite {
    fn new(agent_id: AgentId, store: Arc<VizierStorage>, indexer: VizierIndexer) -> Self {
        Self(agent_id, store, indexer)
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema, Clone)]
pub struct MemoryWriteArgs {
    #[schemars(description = "title of the memory")]
    pub title: String,

    #[schemars(
        description = "memory content in markdown. Link to another concept in the SAME bundle with an ordinary markdown link: [label](path/to/concept.md). Link to a concept in a DIFFERENT bundle with [[bundle/slug]], or reference that whole bundle with bare [[bundle]]. Links are automatically tracked for the knowledge graph."
    )]
    pub content: String,

    #[schemars(description = BUNDLE_FIELD_DESC)]
    #[serde(default)]
    pub bundle: Option<String>,

    #[schemars(
        description = "Where to file this concept within the bundle, as a possibly multi-segment path with no extension (e.g. 'friends/bred' to nest it under a 'friends' subdirectory). Omit to derive it from the title. Writing to a path that already exists updates that memory in place."
    )]
    #[serde(default)]
    pub path: Option<String>,

    #[schemars(
        description = "tags for categorization, e.g. ['rust', 'architecture', 'project-x']"
    )]
    #[serde(default)]
    pub tags: Vec<String>,

    #[schemars(
        description = "filenames of session files to attach (use list_session_files to see available files)"
    )]
    #[serde(default)]
    pub attachments: Option<Vec<String>>,
}

#[async_trait::async_trait]
impl VizierTool for MemoryWrite {
    type Input = MemoryWriteArgs;
    type Output = String;

    fn name() -> String {
        "memory_write".to_string()
    }

    fn description(&self) -> String {
        "Write or update a memory. A write with no bundle named goes to your default bundle; \
        naming a new bundle creates it automatically. Use `path` to nest a concept under a \
        subdirectory (e.g. 'friends/bred'). Same-bundle links are ordinary markdown links \
        ([label](path/to/concept.md)); cross-bundle links use [[bundle/slug]] or bare [[bundle]]. \
        Tags can be added for categorization."
            .into()
    }

    async fn call(
        &self,
        args: Self::Input,
        ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let path = args.path.clone().unwrap_or_else(|| slugify!(&args.title));
        let bundle = args.bundle.clone();

        let content = format!("{}", args.content);

        let mut attachments = Vec::new();
        if let Some(filenames) = &args.attachments {
            for filename in filenames {
                match self.1.get_session_file(&ctx.session, filename).await {
                    Ok(Some(record)) => {
                        let url = format!("/api/v1/files/{}", record.file_id);
                        attachments.push(VizierAttachment {
                            filename: record.filename,
                            content: VizierAttachmentContent::Local(url),
                        });
                    }
                    Ok(None) => {
                        return Err(VizierError(format!(
                            "session file '{}' not found",
                            filename
                        )));
                    }
                    Err(e) => {
                        return Err(VizierError(format!(
                            "failed to look up session file '{}': {}",
                            filename, e
                        )));
                    }
                }
            }
        }

        let memory = self
            .1
            .write_memory(
                self.0.clone(),
                bundle,
                Some(path),
                false,
                args.title,
                content,
                args.tags.clone(),
                attachments,
                &self.2,
            )
            .await
            .map_err(|err| VizierError(err.to_string()))?;

        let relations_info = if memory.relations.is_empty() {
            String::new()
        } else {
            format!(" Links: [{}]", memory.relations.join(", "))
        };

        Ok(format!(
            "memory '{}/{}' written{}",
            memory.bundle, memory.slug, relations_info
        ))
    }
}

pub type MemoryDetail = GetVectorMemory;
pub struct GetVectorMemory(AgentId, Arc<VizierStorage>);

impl MemoryDetail {
    fn new(agent_id: AgentId, store: Arc<VizierStorage>) -> Self {
        Self(agent_id, store)
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct MemoryDetailArgs {
    #[schemars(
        description = "Path of the memory to retrieve (multi-segment for a nested concept, e.g. 'friends/bred')"
    )]
    pub path: String,

    #[schemars(description = BUNDLE_FIELD_DESC)]
    #[serde(default)]
    pub bundle: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct MemoryDetailOutput {
    pub bundle: String,
    pub path: String,
    pub title: String,
    pub content: String,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
    pub agent_id: String,
    pub tags: Vec<String>,
    pub relations: Vec<String>,
    pub attachments: Vec<String>,
    pub read_count: u64,
}

#[async_trait::async_trait]
impl VizierTool for MemoryDetail {
    type Input = MemoryDetailArgs;
    type Output = String;

    fn name() -> String {
        "memory_detail".to_string()
    }

    fn description(&self) -> String {
        "Get full memory content by (bundle, path) — bundle defaults to your default bundle. \
        Content may contain same-bundle markdown links or [[bundle/slug]]/[[bundle]] cross-bundle \
        wikilinks — call memory_follow or memory_detail with those to traverse the knowledge graph. \
        Memory attachments are added to your session files.".into()
    }

    async fn call(
        &self,
        args: Self::Input,
        ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let memory = self
            .1
            .get_memory_detail(self.0.clone(), args.bundle.clone(), args.path)
            .await
            .map_err(|err| VizierError(err.to_string()))?;

        match memory {
            Some(m) => {
                let _ = self
                    .1
                    .increment_read_count(self.0.clone(), Some(m.bundle.clone()), m.slug.clone())
                    .await;

                let output = serde_json::to_string_pretty(&MemoryDetailOutput {
                    bundle: m.bundle,
                    path: m.slug,
                    title: m.title.clone(),
                    content: m.content,
                    created_at: m.created_at,
                    updated_at: m.updated_at,
                    agent_id: m.agent_id,
                    tags: m.tags,
                    relations: m.relations,
                    attachments: m.attachments.iter().map(|a| a.filename.clone()).collect(),
                    read_count: m.read_count,
                })
                .unwrap_or_default();

                let mut stored_files = Vec::new();
                for att in &m.attachments {
                    if let VizierAttachmentContent::Local(url) = &att.content {
                        let file_id = url.trim_start_matches("/api/v1/files/");
                        let mime_type = get_mime_type(&att.filename);

                        if self
                            .1
                            .save_session_file(&ctx.session, &att.filename, &mime_type, 0, file_id)
                            .await
                            .is_ok()
                        {
                            stored_files.push(att.filename.clone());
                        }
                    }
                }

                if stored_files.is_empty() {
                    Ok(output)
                } else {
                    let files = stored_files.join(", ");
                    Ok(format!("{}\n\n[+{} to session files]", output, files))
                }
            }
            None => Ok("Memory not found".to_string()),
        }
    }
}

pub type MemoryFollow = FollowVectorMemory;
pub struct FollowVectorMemory(AgentId, Arc<VizierStorage>);

impl MemoryFollow {
    fn new(agent_id: AgentId, store: Arc<VizierStorage>) -> Self {
        Self(agent_id, store)
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct MemoryFollowArgs {
    #[schemars(
        description = "Path of the memory to start from (multi-segment for a nested concept)"
    )]
    pub path: String,

    #[schemars(description = BUNDLE_FIELD_DESC)]
    #[serde(default)]
    pub bundle: Option<String>,

    #[schemars(
        description = "traversal depth (1 = immediate links only, 2 = links of links, etc.). Default is 1."
    )]
    #[serde(default = "default_depth")]
    pub depth: Option<usize>,
}

fn default_depth() -> Option<usize> {
    Some(1)
}

#[async_trait::async_trait]
impl VizierTool for MemoryFollow {
    type Input = MemoryFollowArgs;
    type Output = Vec<MemoryDetailOutput>;

    fn name() -> String {
        "memory_follow".to_string()
    }

    fn description(&self) -> String {
        "Follow same-bundle and cross-bundle links from a memory to traverse the knowledge \
        graph — a bare [[bundle]] reference resolves to every concept in that bundle. Returns \
        related memories at the specified depth."
            .into()
    }

    async fn call(
        &self,
        args: Self::Input,
        _ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let depth = args.depth.unwrap_or(1);
        let default_bundle = args
            .bundle
            .clone()
            .unwrap_or_else(crate::schema::default_bundle);

        let mut visited = std::collections::HashSet::new();
        let mut result = Vec::new();
        let mut current: Vec<(String, String)> = vec![(default_bundle, args.path.clone())];

        for _ in 0..depth {
            let mut next: Vec<(String, String)> = Vec::new();

            for (bundle, path) in &current {
                let key = (bundle.clone(), path.clone());
                if visited.contains(&key) {
                    continue;
                }
                visited.insert(key);

                let related = self
                    .1
                    .get_related_memories(self.0.clone(), Some(bundle.clone()), path.clone())
                    .await
                    .map_err(|err| VizierError(err.to_string()))?;

                for memory in related {
                    let memory_key = (memory.bundle.clone(), memory.slug.clone());
                    if !visited.contains(&memory_key) {
                        let _ = self
                            .1
                            .increment_read_count(
                                self.0.clone(),
                                Some(memory.bundle.clone()),
                                memory.slug.clone(),
                            )
                            .await;

                        let attachment_names: Vec<String> = memory
                            .attachments
                            .iter()
                            .map(|a| a.filename.clone())
                            .collect();
                        result.push(MemoryDetailOutput {
                            bundle: memory.bundle.clone(),
                            path: memory.slug.clone(),
                            title: memory.title,
                            content: memory.content,
                            created_at: memory.created_at,
                            updated_at: memory.updated_at,
                            agent_id: memory.agent_id,
                            tags: memory.tags,
                            relations: memory.relations,
                            attachments: attachment_names,
                            read_count: memory.read_count,
                        });
                        next.push(memory_key);
                    }
                }
            }

            current = next;
        }

        Ok(result)
    }
}

pub type MemoryGraphTool = GetMemoryGraph;
pub struct GetMemoryGraph(AgentId, Arc<VizierStorage>);

impl MemoryGraphTool {
    fn new(agent_id: AgentId, store: Arc<VizierStorage>) -> Self {
        Self(agent_id, store)
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct MemoryGraphArgs {
    #[schemars(description = BUNDLE_FIELD_DESC)]
    #[serde(default)]
    pub bundle: Option<String>,

    #[schemars(description = "filter by tags (optional)")]
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct MemoryGraphOutput {
    pub nodes: Vec<MemoryGraphNodeOutput>,
    pub edges: Vec<MemoryGraphEdgeOutput>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct MemoryGraphNodeOutput {
    pub slug: String,
    pub bundle: String,
    pub title: String,
    pub tags: Vec<String>,
    /// `true` for a synthetic node standing in for a link that crosses out of the bundle being
    /// viewed (only ever set at the concept level, when `bundle` was named).
    pub boundary: bool,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct MemoryGraphEdgeOutput {
    pub source: String,
    pub target: String,
    pub broken: bool,
}

#[async_trait::async_trait]
impl VizierTool for MemoryGraphTool {
    type Input = MemoryGraphArgs;
    type Output = MemoryGraphOutput;

    fn name() -> String {
        "memory_graph".to_string()
    }

    fn description(&self) -> String {
        "Get the knowledge graph structure of your memory. Called with no bundle, returns the \
        top-level graph (bundles as nodes, cross-bundle links as edges) — the same zoom level as \
        memory_list() with no bundle. Called with a bundle name, returns that bundle's concepts \
        as nodes and their links as edges, plus one boundary node per other bundle it links out \
        to."
        .into()
    }

    async fn call(
        &self,
        args: Self::Input,
        _ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let graph = self
            .1
            .get_memory_graph(self.0.clone(), args.bundle, None)
            .await
            .map_err(|err| VizierError(err.to_string()))?;

        let mut nodes: Vec<MemoryGraphNodeOutput> = graph
            .nodes
            .into_iter()
            .filter(|n| {
                if let Some(ref tags) = args.tags {
                    if !tags.is_empty() {
                        return tags.iter().any(|t| n.tags.contains(t));
                    }
                }
                true
            })
            .map(|n| MemoryGraphNodeOutput {
                slug: n.slug,
                bundle: n.bundle,
                title: n.title,
                tags: n.tags,
                boundary: n.boundary,
            })
            .collect();

        let node_slugs: std::collections::HashSet<String> =
            nodes.iter().map(|n| n.slug.clone()).collect();

        let edges: Vec<MemoryGraphEdgeOutput> = graph
            .edges
            .into_iter()
            .filter(|e| node_slugs.contains(&e.source) || node_slugs.contains(&e.target))
            .map(|e| MemoryGraphEdgeOutput {
                source: e.source,
                target: e.target,
                broken: e.broken,
            })
            .collect();

        nodes.sort_by(|a, b| a.slug.cmp(&b.slug));

        Ok(MemoryGraphOutput { nodes, edges })
    }
}

pub type MemoryDelete = DeleteVectorMemory;
pub struct DeleteVectorMemory(AgentId, Arc<VizierStorage>, VizierIndexer);

impl MemoryDelete {
    fn new(agent_id: AgentId, store: Arc<VizierStorage>, indexer: VizierIndexer) -> Self {
        Self(agent_id, store, indexer)
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct MemoryDeleteArgs {
    #[schemars(description = "Path of the memory to delete (multi-segment for a nested concept)")]
    pub path: String,

    #[schemars(description = BUNDLE_FIELD_DESC)]
    #[serde(default)]
    pub bundle: Option<String>,
}

#[async_trait::async_trait]
impl VizierTool for MemoryDelete {
    type Input = MemoryDeleteArgs;
    type Output = String;

    fn name() -> String {
        "memory_delete".to_string()
    }

    fn description(&self) -> String {
        "Delete a memory by (bundle, path) — bundle defaults to your default bundle. Permanently \
        removes the memory and its embedding. Use memory_detail first to verify the path if unsure."
            .into()
    }

    async fn call(
        &self,
        args: Self::Input,
        _ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let path = args.path.clone();
        let bundle = args.bundle.clone();

        let detail = self
            .1
            .get_memory_detail(self.0.clone(), bundle.clone(), path.clone())
            .await
            .map_err(|e| VizierError(e.to_string()))?;

        let outgoing = match &detail {
            Some(m) if !m.relations.is_empty() => m.relations.clone(),
            _ => vec![],
        };

        let has_incoming = self
            .1
            .has_incoming_links(self.0.clone(), bundle.clone(), path.clone())
            .await
            .map_err(|e| VizierError(e.to_string()))?;

        if !outgoing.is_empty() || has_incoming {
            let mut msg = format!(
                "Cannot delete memory '{}': it is linked in the knowledge graph.\n\n",
                path
            );
            if !outgoing.is_empty() {
                msg += "Outgoing links (this memory references):\n";
                for s in &outgoing {
                    msg += &format!("- {}\n", s);
                }
                msg += "\n";
            }
            if has_incoming {
                let related = self
                    .1
                    .get_related_memories(self.0.clone(), bundle.clone(), path.clone())
                    .await
                    .map_err(|e| VizierError(e.to_string()))?;
                let incoming: Vec<_> = related
                    .iter()
                    .filter(|m| m.relations.iter().any(|r| r.contains(&path)))
                    .collect();
                msg += "Incoming links (other memories reference this one):\n";
                for m in &incoming {
                    msg += &format!("- \"{}\" ({}/{})\n", m.title, m.bundle, m.slug);
                }
                msg += "\n";
            }
            msg +=
                "Remove those links from those memories first, or use memory_write to update them.";
            return Err(VizierError(msg));
        }

        self.1
            .delete_memory(self.0.clone(), bundle, path.clone(), &self.2)
            .await
            .map_err(|err| VizierError(err.to_string()))?;

        Ok(format!("Memory '{}' deleted", path))
    }
}

pub type MemoryDeleteBundle = DeleteVectorMemoryBundle;
pub struct DeleteVectorMemoryBundle(AgentId, Arc<VizierStorage>, VizierIndexer);

impl MemoryDeleteBundle {
    fn new(agent_id: AgentId, store: Arc<VizierStorage>, indexer: VizierIndexer) -> Self {
        Self(agent_id, store, indexer)
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct MemoryDeleteBundleArgs {
    #[schemars(
        description = "Name of the bundle to delete. Required — there is no default, so this can't be triggered by accident. The bundle must already be empty of concepts (use memory_delete on each one first, or memory_list(bundle) to see what's left)."
    )]
    pub bundle: String,
}

#[async_trait::async_trait]
impl VizierTool for MemoryDeleteBundle {
    type Input = MemoryDeleteBundleArgs;
    type Output = String;

    fn name() -> String {
        "memory_delete_bundle".to_string()
    }

    fn description(&self) -> String {
        "Permanently delete an empty bundle (its index.md/log.md). Fails if the bundle still \
        contains any concept documents — delete those first with memory_delete, or check with \
        memory_list(bundle)."
            .into()
    }

    async fn call(
        &self,
        args: Self::Input,
        _ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        self.1
            .delete_bundle(self.0.clone(), args.bundle.clone(), false, &self.2)
            .await
            .map_err(|err| VizierError(err.to_string()))?;

        Ok(format!("Bundle '{}' deleted", args.bundle))
    }
}
