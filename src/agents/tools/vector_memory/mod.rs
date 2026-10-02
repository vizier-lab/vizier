use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use slugify::slugify;

use crate::agents::tools::{ToolContext, VizierTool};
use crate::error::VizierError;
use crate::indexer::VizierIndexer;
use crate::schema::{AgentId, RevisionOrigin, VizierAttachment, VizierAttachmentContent};
use crate::storage::VizierStorage;
use crate::storage::memory::MemoryStorage;
use crate::storage::session_file::SessionFileStorage;
use crate::utils::get_mime_type;

/// The eight memory tools, in the order `VizierTools::new` registers them.
pub type MemoryToolset = (
    MemorySearch,
    MemoryWrite,
    MemoryList,
    MemoryRead,
    MemoryFollow,
    MemoryGraphTool,
    MemoryDelete,
    MemoryDeleteBundle,
);

pub fn init_vector_memory(
    agent_id: String,
    storage: Arc<VizierStorage>,
    indexer: VizierIndexer,
    recall: RecallSettings,
) -> Result<MemoryToolset> {
    Ok((
        MemorySearch::new(agent_id.clone(), storage.clone(), indexer.clone(), recall.clone()),
        MemoryWrite::new(agent_id.clone(), storage.clone(), indexer.clone(), recall),
        MemoryList::new(agent_id.clone(), storage.clone()),
        MemoryRead::new(agent_id.clone(), storage.clone()),
        MemoryFollow::new(agent_id.clone(), storage.clone()),
        MemoryGraphTool::new(agent_id.clone(), storage.clone()),
        MemoryDelete::new(agent_id.clone(), storage.clone(), indexer.clone()),
        MemoryDeleteBundle::new(agent_id.clone(), storage.clone(), indexer),
    ))
}

const BUNDLE_FIELD_DESC: &str = "Bundle name. Bundles are named containers for related memories (e.g. one per project or person) — omit this to use your default bundle; naming a new bundle creates it automatically.";

/// Relevance floor for the agent-facing `memory_search` tool.
///
/// Was **0.1**, against the automatic-context path's 0.5 — a 5x disagreement between two callers
/// of one index, which is itself evidence neither was set deliberately (research Decision 10). At
/// 0.1 the filter is a no-op: nearly any passage clears a cosine similarity of 0.1 against nearly
/// any query, so the only thing bounding a search was its result limit, and the "empty result when
/// nothing qualifies" contract (FR-014) could essentially never fire.
///
/// **0.20, measured** against a real index rather than guessed (quickstart Step 3 / task T052).
/// Searching a 14-passage document with fastembed `all-MiniLM-L6-v2`, scores separated cleanly:
///
/// | Query kind | Observed score |
/// |---|---|
/// | Direct heading + topic match ("when do we deploy") | 0.30 – 0.49 |
/// | Weaker topical match ("vendor contracts") | 0.20 – 0.32 |
/// | Entirely unrelated ("banana bread recipe") | 0.04 – 0.08 |
///
/// So relevance and noise are separated by a gap between roughly 0.08 and 0.20, and 0.20 sits at
/// the bottom of it: every genuine topical match observed clears it, and every unrelated query is
/// rejected with 2.5x of margin. An earlier pass at this set 0.35, which *looked* conservative and
/// turned out to reject a direct heading match ("incident review", 0.32) — a false negative is the
/// worse failure here, because the agent has no way to tell "nothing matched" from "the filter was
/// too tight".
///
/// Still looser than the automatic-context default, which FR-023 requires to be the stricter of the
/// two: a search the agent *chose* to run should surface weaker matches than an injection it did
/// not ask for. Both numbers are keyed to one embedding model on one corpus, so the history-replay
/// harness (task T051) remains the thing that settles them for a given deployment.
pub const SEARCH_THRESHOLD: f64 = 0.20;

/// The agent's chunking and search settings, as the memory tools need them.
#[derive(Clone)]
pub struct RecallSettings {
    pub chunking: crate::config::ChunkLimits,
    /// Results returned by one `memory_search` call (FR-013).
    pub search_limit: usize,
    /// Relevance floor for `memory_search` (FR-013). Deliberately looser than the
    /// automatic-context threshold: a deliberate search should surface weaker matches than an
    /// unasked-for injection does.
    pub search_threshold: f64,
    /// How many passages one document may contribute to one search (FR-012).
    pub per_document: usize,
}

pub type MemorySearch = ReadVectorMemory;
pub struct ReadVectorMemory(AgentId, Arc<VizierStorage>, VizierIndexer, RecallSettings);

impl MemorySearch {
    fn new(
        agent_id: AgentId,
        store: Arc<VizierStorage>,
        indexer: VizierIndexer,
        recall: RecallSettings,
    ) -> Self {
        Self(agent_id, store, indexer, recall)
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
        subdirectories, paginated). Use memory_read to read a concept's full content, or \
        memory_search to search across everything at once instead of browsing."
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
pub struct MemorySearchArgs {
    #[schemars(description = "Terms, keywords, or prompt to search")]
    pub query: String,

    #[schemars(
        description = "Narrow the search to one bundle. Omit to search across all of your bundles at once, ranked by relevance regardless of which bundle a match lives in — the useful default, since a bundle is just an organizational container."
    )]
    #[serde(default)]
    pub bundle: Option<String>,
}

/// One search hit as the agent sees it. Mirrors `MemoryPassageResult` (the shape the HTTP
/// endpoint returns too) minus `truncated`, which only automatic context ever sets.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct MemorySearchHit {
    pub bundle: String,
    pub path: String,
    pub title: String,
    pub ordinal: usize,
    pub ordinal_end: usize,
    pub line_start: usize,
    pub line_end: usize,
    pub score: f64,
    pub text: String,
}

#[async_trait::async_trait]
impl VizierTool for MemorySearch {
    type Input = MemorySearchArgs;
    type Output = Vec<MemorySearchHit>;

    fn name() -> String {
        "memory_search".to_string()
    }

    fn description(&self) -> String {
        "Semantic search across your memories (all bundles by default, or one named bundle). \
        Returns short **passages** — the matching excerpts of your memories, not whole documents \
        — each with the bundle, path, title, passage ordinal and line span it came from. When a \
        passage is not enough, call memory_read with that bundle and path to get the whole \
        document. Memory content may contain [label](path/to/concept.md) same-bundle links or \
        [[bundle/slug]] / [[bundle]] cross-bundle links — use memory_read or memory_follow to \
        explore them. An empty result means nothing matched well enough; it does not mean your \
        memory is empty, so try different terms or memory_list to browse."
            .into()
    }

    async fn call(
        &self,
        args: Self::Input,
        _ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let recall = &self.3;
        let res = self
            .1
            .query_memory(
                self.0.clone(),
                args.bundle.clone(),
                args.query,
                recall.search_limit,
                recall.search_threshold,
                recall.per_document,
                &self.2,
                &recall.chunking,
            )
            .await
            .map_err(|err| VizierError(err.to_string()))?;

        // A read is recorded once per source document, not once per passage — the count measures
        // how often a memory gets used, and one search returning three of its passages is one use
        // (FR-016).
        let mut counted = std::collections::HashSet::new();
        for hit in &res {
            if counted.insert((hit.bundle.clone(), hit.path.clone())) {
                let _ = self
                    .1
                    .increment_read_count(
                        self.0.clone(),
                        Some(hit.bundle.clone()),
                        hit.path.clone(),
                    )
                    .await;
            }
        }

        Ok(res
            .into_iter()
            .map(|p| MemorySearchHit {
                bundle: p.bundle,
                path: p.path,
                title: p.title,
                ordinal: p.ordinal,
                ordinal_end: p.ordinal_end,
                line_start: p.line_start,
                line_end: p.line_end,
                score: p.score,
                text: p.text,
            })
            .collect())
    }
}

pub type MemoryWrite = WriteVectorMemory;
pub struct WriteVectorMemory(AgentId, Arc<VizierStorage>, VizierIndexer, RecallSettings);

impl MemoryWrite {
    fn new(
        agent_id: AgentId,
        store: Arc<VizierStorage>,
        indexer: VizierIndexer,
        recall: RecallSettings,
    ) -> Self {
        Self(agent_id, store, indexer, recall)
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

        let content = args.content.clone();

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
                &RevisionOrigin::from_session(&ctx.session),
                &self.2,
                &self.3.chunking,
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

pub type MemoryRead = GetVectorMemory;
pub struct GetVectorMemory(AgentId, Arc<VizierStorage>);

impl MemoryRead {
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
impl VizierTool for MemoryRead {
    type Input = MemoryDetailArgs;
    type Output = String;

    fn name() -> String {
        "memory_read".to_string()
    }

    fn description(&self) -> String {
        "Read one memory in full by (bundle, path) — bundle defaults to your default bundle. This \
        is what you call after memory_search returns a passage that is not enough on its own: the \
        search result's bundle and path are exactly this tool's arguments, so no second search is \
        needed. Content may contain same-bundle markdown links or [[bundle/slug]]/[[bundle]] \
        cross-bundle wikilinks — call memory_follow or memory_read with those to traverse the \
        knowledge graph. Memory attachments are added to your session files.".into()
    }

    async fn call(
        &self,
        args: Self::Input,
        ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let path_for_error = args.path.clone();
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
            // FR-019: say plainly that it is gone, and say what to do instead. An empty result or
            // an opaque error both read to an agent as "the tool failed", and it retries.
            None => Err(VizierError(format!(
                "no memory exists at bundle '{}' path '{}' — it no longer exists, or it was never \
                 there. It was not moved: this address is how memories are named. Use memory_list \
                 to see what the bundle holds, or memory_search to find it by content.",
                args.bundle.as_deref().unwrap_or("default"),
                path_for_error,
            ))),
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
        removes the memory and its passages. Use memory_read first to verify the path if unsure."
            .into()
    }

    async fn call(
        &self,
        args: Self::Input,
        ctx: &ToolContext,
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
            .delete_memory(
                self.0.clone(),
                bundle,
                path.clone(),
                &RevisionOrigin::from_session(&ctx.session),
                &self.2,
            )
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
        ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        self.1
            .delete_bundle(
                self.0.clone(),
                args.bundle.clone(),
                false,
                &RevisionOrigin::from_session(&ctx.session),
                &self.2,
            )
            .await
            .map_err(|err| VizierError(err.to_string()))?;

        Ok(format!("Bundle '{}' deleted", args.bundle))
    }
}
