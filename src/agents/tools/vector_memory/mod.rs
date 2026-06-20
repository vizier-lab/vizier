use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use slugify::slugify;

use crate::agents::tools::{ToolContext, VizierTool};
use crate::error::VizierError;
use crate::indexer::VizierIndexer;
use crate::schema::{AgentId, MemoryVisibility, VizierAttachment, VizierAttachmentContent};
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
)> {
    Ok((
        MemoryRead::new(agent_id.clone(), storage.clone(), indexer.clone()),
        MemoryWrite::new(agent_id.clone(), storage.clone(), indexer.clone()),
        MemoryList::new(agent_id.clone(), storage.clone()),
        MemoryDetail::new(agent_id.clone(), storage.clone()),
        MemoryFollow::new(agent_id.clone(), storage.clone()),
        MemoryGraphTool::new(agent_id.clone(), storage.clone()),
        MemoryDelete::new(agent_id.clone(), storage.clone(), indexer),
    ))
}

pub type MemoryRead = ReadVectorMemory;
pub struct ReadVectorMemory(AgentId, Arc<VizierStorage>, VizierIndexer);

impl MemoryRead {
    fn new(agent_id: AgentId, store: Arc<VizierStorage>, indexer: VizierIndexer) -> Self {
        Self(agent_id, store, indexer)
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct MemoryListArgs {
    #[schemars(description = "Maximum number of memories to return")]
    #[serde(default = "default_limit")]
    pub limit: Option<usize>,

    #[schemars(description = "Number of memories to skip")]
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
pub struct MemorySummary {
    pub slug: String,
    pub title: String,
    pub timestamp: chrono::DateTime<Utc>,
    pub visibility: String,
    pub tags: Vec<String>,
    pub relations: Vec<String>,
    pub attachment_count: usize,
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
    type Output = Vec<MemorySummary>;

    fn name() -> String {
        "memory_list".to_string()
    }

    fn description(&self) -> String {
        "List your memories with pagination. Returns slug, title, tags, and visibility for each memory. Use memory_detail to read full content.".into()
    }

    async fn call(
        &self,
        args: Self::Input,
        _ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let limit = args.limit.unwrap_or(50);
        let offset = args.offset.unwrap_or(0);

        let all_memory = self
            .1
            .get_all_agent_memory(self.0.clone())
            .await
            .map_err(|err| VizierError(err.to_string()))?;

        Ok(all_memory
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|m| MemorySummary {
                slug: m.slug,
                title: m.title,
                timestamp: m.timestamp,
                visibility: m.visibility.to_string(),
                tags: m.tags,
                relations: m.relations,
                attachment_count: m.attachments.len(),
            })
            .collect())
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct MemoryReadArgs {
    #[schemars(description = "Terms, keywords, or prompt to search")]
    pub query: String,
}

#[async_trait::async_trait]
impl VizierTool for MemoryRead {
    type Input = MemoryReadArgs;
    type Output = Vec<String>;

    fn name() -> String {
        "memory_read".to_string()
    }

    fn description(&self) -> String {
        "Semantic search across your memories. Returns content that matches the query. Memory content may contain [[slug]] links to related memories — use memory_detail to explore them.".into()
    }

    async fn call(
        &self,
        args: Self::Input,
        _ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let res = self
            .1
            .query_memory(self.0.clone(), args.query, 10, 0.1, &self.2)
            .await
            .map_err(|err| VizierError(err.to_string()))?;

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
        description = "memory content in markdown. Use [[slug]] to link to other memories, e.g. 'related to [[project-setup]] and [[api-reference]]'. Links are automatically tracked for the knowledge graph."
    )]
    pub content: String,

    #[schemars(
        description = "tags for categorization, e.g. ['rust', 'architecture', 'project-x']"
    )]
    #[serde(default)]
    pub tags: Vec<String>,

    #[schemars(
        description = "visibility: 'private' (default, only you), 'global' (all agents), or 'shared' (specific agents)"
    )]
    #[serde(default = "default_visibility")]
    pub visibility: String,

    #[schemars(description = "list of agent IDs to share with (only when visibility is 'shared')")]
    #[serde(default)]
    pub shared_to: Vec<String>,

    #[schemars(
        description = "filenames of session files to attach (use list_session_files to see available files)"
    )]
    #[serde(default)]
    pub attachments: Option<Vec<String>>,
}

fn default_visibility() -> String {
    "private".to_string()
}

#[async_trait::async_trait]
impl VizierTool for MemoryWrite {
    type Input = MemoryWriteArgs;
    type Output = String;

    fn name() -> String {
        "memory_write".to_string()
    }

    fn description(&self) -> String {
        "Write or update a memory. Use [[slug]] syntax in content to link to other memories (e.g. 'see [[project-architecture]] for details'). Tags can be added for categorization.".into()
    }

    async fn call(
        &self,
        args: Self::Input,
        ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let slug = slugify!(&args.title).to_string();
        let visibility: MemoryVisibility = args
            .visibility
            .parse()
            .map_err(|e: String| VizierError(e))?;

        let content = format!("{}\n timestamp: {}", args.content, Utc::now());

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
                Some(slug.clone()),
                args.title,
                content,
                visibility.clone(),
                args.shared_to.clone(),
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
            "memory {} is written with {} visibility{}",
            slug, visibility, relations_info
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
    #[schemars(description = "Slug of the memory to retrieve")]
    pub slug: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct MemoryDetailOutput {
    pub slug: String,
    pub title: String,
    pub content: String,
    pub timestamp: chrono::DateTime<Utc>,
    pub agent_id: String,
    pub visibility: String,
    pub shared_to: Vec<String>,
    pub tags: Vec<String>,
    pub relations: Vec<String>,
    pub attachments: Vec<String>,
}

#[async_trait::async_trait]
impl VizierTool for MemoryDetail {
    type Input = MemoryDetailArgs;
    type Output = String;

    fn name() -> String {
        "memory_detail".to_string()
    }

    fn description(&self) -> String {
        "Get full memory content by slug. Content may contain [[slug]] links to related memories — call this tool with those slugs to traverse the knowledge graph. Memory attachments are added to your session files.".into()
    }

    async fn call(
        &self,
        args: Self::Input,
        ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let memory = self
            .1
            .get_memory_detail(self.0.clone(), args.slug)
            .await
            .map_err(|err| VizierError(err.to_string()))?;

        match memory {
            Some(m) => {
                let output = serde_json::to_string_pretty(&MemoryDetailOutput {
                    slug: m.slug,
                    title: m.title.clone(),
                    content: m.content,
                    timestamp: m.timestamp,
                    agent_id: m.agent_id,
                    visibility: m.visibility.to_string(),
                    shared_to: m.shared_to,
                    tags: m.tags,
                    relations: m.relations,
                    attachments: m.attachments.iter().map(|a| a.filename.clone()).collect(),
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
    #[schemars(description = "slug of the memory to start from")]
    pub slug: String,

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
        "Follow [[slug]] links from a memory to traverse the knowledge graph. Returns related memories at the specified depth. Use this to explore connections between memories.".into()
    }

    async fn call(
        &self,
        args: Self::Input,
        _ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let depth = args.depth.unwrap_or(1);

        let mut visited = std::collections::HashSet::new();
        let mut result = Vec::new();
        let mut current_slugs = vec![args.slug.clone()];

        for _ in 0..depth {
            let mut next_slugs = Vec::new();

            for slug in &current_slugs {
                if visited.contains(slug) {
                    continue;
                }
                visited.insert(slug.clone());

                let related = self
                    .1
                    .get_related_memories(self.0.clone(), slug.clone())
                    .await
                    .map_err(|err| VizierError(err.to_string()))?;

                for memory in related {
                    if !visited.contains(&memory.slug) {
                        let attachment_names: Vec<String> = memory
                            .attachments
                            .iter()
                            .map(|a| a.filename.clone())
                            .collect();
                        result.push(MemoryDetailOutput {
                            slug: memory.slug.clone(),
                            title: memory.title,
                            content: memory.content,
                            timestamp: memory.timestamp,
                            agent_id: memory.agent_id,
                            visibility: memory.visibility.to_string(),
                            shared_to: memory.shared_to,
                            tags: memory.tags,
                            relations: memory.relations,
                            attachments: attachment_names,
                        });
                        next_slugs.push(memory.slug);
                    }
                }
            }

            current_slugs = next_slugs;
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
    pub title: String,
    pub tags: Vec<String>,
    pub visibility: String,
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
        "Get the knowledge graph structure of your memories. Returns nodes (memories) and edges (links between them). Use this to understand how your memories are connected.".into()
    }

    async fn call(
        &self,
        args: Self::Input,
        _ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let graph = self
            .1
            .get_memory_graph(self.0.clone())
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
                title: n.title,
                tags: n.tags,
                visibility: n.visibility.to_string(),
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
    #[schemars(description = "Slug of the memory to delete")]
    pub slug: String,
}

#[async_trait::async_trait]
impl VizierTool for MemoryDelete {
    type Input = MemoryDeleteArgs;
    type Output = String;

    fn name() -> String {
        "memory_delete".to_string()
    }

    fn description(&self) -> String {
        "Delete a memory by slug. Permanently removes the memory and its embedding. Use memory_detail first to verify the slug if unsure.".into()
    }

    async fn call(
        &self,
        args: Self::Input,
        _ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let slug = args.slug.clone();

        let detail = self
            .1
            .get_memory_detail(self.0.clone(), slug.clone())
            .await
            .map_err(|e| VizierError(e.to_string()))?;

        let outgoing = match &detail {
            Some(m) if !m.relations.is_empty() => m.relations.clone(),
            _ => vec![],
        };

        let has_incoming = self
            .1
            .has_incoming_links(self.0.clone(), slug.clone())
            .await
            .map_err(|e| VizierError(e.to_string()))?;

        if !outgoing.is_empty() || has_incoming {
            let mut msg = format!(
                "Cannot delete memory '{}': it is linked in the knowledge graph.\n\n",
                slug
            );
            if !outgoing.is_empty() {
                msg += "Outgoing links (this memory references):\n";
                for s in &outgoing {
                    msg += &format!("- [[{}]]\n", s);
                }
                msg += "\n";
            }
            if has_incoming {
                let related = self
                    .1
                    .get_related_memories(self.0.clone(), slug.clone())
                    .await
                    .map_err(|e| VizierError(e.to_string()))?;
                let incoming: Vec<_> = related
                    .iter()
                    .filter(|m| m.relations.contains(&slug))
                    .collect();
                msg += "Incoming links (other memories reference this one):\n";
                for m in &incoming {
                    msg += &format!("- \"{}\" ({})\n", m.title, m.slug);
                }
                msg += "\n";
            }
            msg += "Remove the [[slug]] links from those memories first, or use memory_write to update them.";
            return Err(VizierError(msg));
        }

        self.1
            .delete_memory(self.0.clone(), slug.clone(), &self.2)
            .await
            .map_err(|err| VizierError(err.to_string()))?;

        Ok(format!("Memory '{}' deleted", slug))
    }
}
