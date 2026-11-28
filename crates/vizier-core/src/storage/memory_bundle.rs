use crate::time::Timestamp;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::{Read, Write};

use anyhow::{Result, anyhow};
use chrono::{DateTime, Local};
use regex::Regex;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use slugify::slugify;

use crate::{
    config::{ChunkLimits, MemoryLimits},
    indexer::VizierIndexer,
    schema::{
        BundleSummary, CORE_BUNDLE, ImportReport, Memory, MemoryFrontMatter,
        MemoryGraph, MemoryGraphEdge, MemoryGraphNode, MemoryPassageResult, MemoryQueryParams,
        MemoryRevision, MemoryRevisionSummary, PaginatedMemory, PaginatedMemoryRevisions,
        RevisionDiff, RevisionOrigin, RevisionTrigger, RollbackResponse, SOUL_PATH,
        VizierAttachment, default_bundle, is_core_bundle, is_identity_address,
    },
    storage::{
        chunk::{chunk_markdown, embedded_text, heading_breadcrumb},
        blob::BlobKey,
        diff::diff_lines,
        memory::compute_initial_slugs,
        rerank::SourceSignals,
        sql::{self, Connection, OptionalExtension, Row, SqlStore, params},
        sqlite::memory_revision::{
            self, MemoryRevisionRow, memory_canonical, parse_memory_canonical,
        },
    },
};

/// How one memory write is judged against the size limits
/// (specs/043-unify-core-and-memory). Carried as one struct rather than three positional
/// arguments, and attached to the *single* write path so every caller — the agent's tool, the
/// HTTP service, a rollback and an import — is judged identically (research Decision 1).
#[derive(Debug, Clone)]
pub struct WritePolicy {
    pub limits: MemoryLimits,
    /// Set only by the one-time identity migration, which must never be blocked by a ceiling
    /// introduced in the same change (FR-025). Skips the ceiling, never the bundle guard.
    pub bypass_cap: bool,
}

impl WritePolicy {
    pub fn new(limits: MemoryLimits) -> Self {
        Self {
            limits,
            bypass_cap: false,
        }
    }

    /// The identity migration's policy: the identity pair, cap bypassed.
    pub fn migrating() -> Self {
        Self {
            limits: MemoryLimits::identity(),
            bypass_cap: true,
        }
    }

    /// The limits that apply to one address: the identity document is paid for on every turn,
    /// so it is held to a tighter pair (FR-023). Selected by address, not by a second config
    /// field, so the two can never disagree (data-model rule 8).
    pub fn limits_for(&self, bundle: &str, path: &str) -> MemoryLimits {
        if is_identity_address(bundle, path) {
            MemoryLimits::identity()
        } else {
            self.limits.clone()
        }
    }
}

/// What a successful write leaves to report, beyond the document itself: a non-error warning
/// when the content is over the soft limit (FR-019, research Decision 4). Never merged into the
/// success message, so an agent cannot read the write as a failure and retry it (FR-020).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WriteWarning {
    pub bytes: usize,
    pub limit: usize,
}

impl WriteWarning {
    /// The sentence rendered wherever a caller wants to show it. Kept in one place so the tool
    /// and the HTTP service cannot drift.
    pub fn describe(&self) -> String {
        format!(
            "{} bytes — over the {}-byte comfortable size. One memory, one topic: consider \
             splitting it and linking the parts.",
            self.bytes, self.limit
        )
    }
}

/// The outcome of one write: the stored document, plus the warning if the content went over
/// the soft limit.
#[derive(Debug, Clone)]
pub struct WrittenMemory {
    pub memory: Memory,
    pub warning: Option<WriteWarning>,
}

/// The shared implementation behind `impl MemoryStorage for SqlStorage`: bundle/concept
/// document read+write, link resolution, and the memory graph. A document is one `memory_node`
/// row (its body in `content`), its links are `memory_edge` rows (specs/070-memory-in-database).
pub struct BundleMemoryStore {
    conn: SqlStore,
}

struct NodeRow {
    bundle: String,
    path: String,
    title: String,
    tags: Vec<String>,
    created_at: Timestamp,
    updated_at: Timestamp,
    read_count: u64,
}

/// What the file migration did with one stored memory file (specs/070-memory-in-database, #82).
#[derive(Debug, PartialEq)]
pub(crate) enum MigratedFile {
    Moved,
    AlreadyThere,
    DeletedSince,
    Unreadable(String),
}

/// One index hit against a document, before merging. `ordinal` is `None` for a legacy
/// whole-document entry written before this feature.
struct PassageHit {
    ordinal: Option<usize>,
    score: f64,
}

struct ClassifiedLink {
    target_bundle: String,
    target_path: Option<String>,
    kind: &'static str,
}

pub(crate) fn normalize_path(raw: &str) -> String {
    let trimmed = raw.trim().replace('\\', "/");
    let trimmed = trimmed.trim_start_matches('/').trim_end_matches('/');
    trimmed.strip_suffix(".md").unwrap_or(trimmed).to_string()
}

fn leaf_of(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

/// Same-bundle links: ordinary markdown relative links (FR-004). `[label](path/to/concept.md)`
/// and `[label](path/to/concept)` (agents frequently forget the extension) are treated
/// identically — every concept document is always a `.md` file by construction, so there's no
/// ambiguity in filling it in. A link to something else entirely (a URL, a `mailto:`, a bare
/// `#anchor`, or a relative path with some *other* extension — an attachment, an image) is left
/// alone rather than misclassified as a concept reference.
fn parse_same_bundle_links(content: &str) -> Vec<String> {
    let re = Regex::new(r"\[[^\]]*\]\(([^)\s]+)\)").unwrap();
    re.captures_iter(content)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
        .filter_map(|href| same_bundle_link_target(&href))
        .collect()
}

fn same_bundle_link_target(href: &str) -> Option<String> {
    if href.is_empty() || href.starts_with('#') {
        return None;
    }
    // A URI scheme (http:, https:, mailto:, ...) always appears before any '/' — a relative
    // path segment never legitimately contains ':', so this is a safe, simple exclusion.
    if href.split('/').next().unwrap_or("").contains(':') {
        return None;
    }
    let path_only = href.split(['?', '#']).next().unwrap_or(href);
    if let Some(stripped) = path_only.strip_suffix(".md") {
        return Some(normalize_path(stripped) + ".md");
    }
    let leaf = path_only.rsplit('/').next().unwrap_or(path_only);
    if leaf.contains('.') || path_only.is_empty() {
        // Some other extension (an attachment, an image, ...) — not a concept link.
        return None;
    }
    Some(normalize_path(path_only) + ".md")
}

/// Cross-bundle wikilinks: `[[bundle/slug]]` or bare `[[bundle]]` (FR-013). Entity wiki
/// links of the WebUI's `[[kind:name]]` grammar (spec 066 — `artifact:`, `function:`,
/// `skill:`, `task:`) are not memory references, so they are skipped here rather than
/// classified as bundle references; nothing in a bundle name can collide because the
/// reserved kinds are matched before any bundle interpretation happens.
const ENTITY_LINK_KINDS: [&str; 4] = ["artifact", "function", "skill", "task"];

fn is_entity_link_target(target: &str) -> bool {
    match target.split_once(':') {
        Some((kind, _)) => ENTITY_LINK_KINDS.contains(&kind),
        None => false,
    }
}

fn parse_wikilinks(content: &str) -> Vec<String> {
    let re = Regex::new(r"\[\[([^\]]+)\]\]").unwrap();
    re.captures_iter(content)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str().trim().to_string()))
        .filter(|s| !s.is_empty())
        .filter(|s| !is_entity_link_target(s))
        .collect()
}

fn parse_relations(content: &str) -> Vec<String> {
    let mut relations = parse_same_bundle_links(content);
    relations.extend(parse_wikilinks(content));
    relations
}

fn classify_relation(source_bundle: &str, relation: &str) -> ClassifiedLink {
    if let Some(path) = relation.strip_suffix(".md") {
        return ClassifiedLink {
            target_bundle: source_bundle.to_string(),
            target_path: Some(normalize_path(path)),
            kind: "same_bundle",
        };
    }
    if let Some((bundle, slug)) = relation.split_once('/') {
        return ClassifiedLink {
            target_bundle: bundle.to_string(),
            target_path: Some(normalize_path(slug)),
            kind: "cross_bundle_concept",
        };
    }
    ClassifiedLink {
        target_bundle: relation.to_string(),
        target_path: None,
        kind: "cross_bundle_bundle",
    }
}

pub(crate) fn serialize_markdown<T: Serialize>(frontmatter: &T, content: &str) -> Result<Vec<u8>> {
    let yaml = serde_yaml::to_string(frontmatter)?;
    Ok(format!("---\n{}---\n{}", yaml, content).into_bytes())
}

pub(crate) fn parse_markdown_bytes<T: DeserializeOwned>(bytes: &[u8]) -> Result<(T, String)> {
    crate::utils::markdown::parse_markdown_str::<T>(&String::from_utf8_lossy(bytes))
        .map_err(|e| anyhow!(e.0))
}

/// Slice `[start, end)` out of a document body, clamped into range and nudged onto UTF-8
/// character boundaries.
///
/// Stored spans are coordinates against the body they were derived from, and the vectors they
/// answer for can lag behind a write (passages are indexed after the commit). So a span can
/// outrun its document, and slicing it naively would panic — including mid-character on emoji
/// or CJK content, which agent memories do contain.
fn slice_passage(content: &str, start: usize, end: usize) -> String {
    let mut start = start.min(content.len());
    let mut end = end.clamp(start, content.len());
    while start > 0 && !content.is_char_boundary(start) {
        start -= 1;
    }
    while end < content.len() && !content.is_char_boundary(end) {
        end += 1;
    }
    content[start..end].to_string()
}

/// The canonical snapshot text of a document: what its history compares against and what a
/// baseline is seeded from.
fn canonical_of(memory: &Memory) -> Option<String> {
    memory_canonical(&memory.title, &memory.tags, &memory.content).ok()
}

/// A memory address is held to the same segment rules as a file path (no `.` or `..` segment,
/// bounded length), so an export archive can never carry an unsafe entry name.
fn check_address(agent_id: &str, bundle: &str, path: &str) -> Result<()> {
    let key = format!("{agent_id}/memory/{bundle}/{path}.md")
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("/");
    BlobKey::new(&key).map(|_| ())
}

/// A timestamp column read back.
fn read_time(text: &str) -> Timestamp {
    DateTime::parse_from_rfc3339(text)
        .map(|d| d.with_timezone(&Local))
        .unwrap_or_else(|_| crate::time::now())
}

/// The refusal for content above the ceiling (FR-018). The first token is the stable marker
/// `memory too large:`, which the service maps to 422 (research Decision 5); the rest names the
/// actual size, the ceiling, the overage and a concrete split pattern. It deliberately does not
/// choose the seam — which subject moves out is the agent's judgement.
fn too_large_message(bundle: &str, path: &str, len: usize, limits: &MemoryLimits) -> String {
    let over = len.saturating_sub(limits.content_cap);
    let leaf = path.rsplit('/').next().unwrap_or(path);
    format!(
        "memory too large: content is {len} bytes, {over} over the {}-byte limit.\n\
         One memory, one topic. Split it, then link the parts:\n  \
         - write the part that belongs to another subject to '{bundle}/{leaf}-<subject>' in the same bundle\n  \
         - replace the moved text in this memory with [<subject>]({leaf}-<subject>.md)\n\
         Same-bundle links are ordinary markdown links; across bundles use [[bundle/slug]].",
        limits.content_cap
    )
}

fn memory_from_frontmatter(fm: MemoryFrontMatter, content: String) -> Memory {
    Memory {
        slug: fm.slug,
        title: fm.title,
        content,
        created_at: fm.created_at,
        updated_at: fm.updated_at,
        agent_id: fm.agent_id,
        bundle: fm.bundle,
        tags: fm.tags,
        keywords: fm.keywords,
        relations: fm.relations,
        read_count: fm.read_count,
    }
}

impl BundleMemoryStore {
    pub fn new(conn: SqlStore) -> Self {
        Self { conn }
    }

    fn indexer_key(agent_id: &str, bundle: &str, path: &str) -> String {
        format!("{agent_id}/{bundle}/{path}")
    }

    /// The key a single passage is indexed under: the document key plus `#{ordinal}`
    /// (research Decision 2). Unambiguous because document paths carry no extension and no `#`.
    fn passage_indexer_key(agent_id: &str, bundle: &str, path: &str, ordinal: usize) -> String {
        format!("{agent_id}/{bundle}/{path}#{ordinal}")
    }

    /// Splits an indexer key back into `(agent_id, bundle, path, ordinal)`. The `splitn(3, '/')`
    /// is what lets `path` keep its own slashes for a nested concept ("friends/bred"); the
    /// trailing `#{ordinal}` is stripped off the end. A key with no `#` suffix yields `None` for
    /// the ordinal, which is how a pre-chunking document-level row is recognised.
    fn parse_indexer_key(key: &str) -> Option<(String, String, String, Option<usize>)> {
        let mut parts = key.splitn(3, '/');
        let agent_id = parts.next()?.to_string();
        let bundle = parts.next()?.to_string();
        let rest = parts.next()?;

        match rest.rsplit_once('#') {
            Some((path, ordinal)) => match ordinal.parse::<usize>() {
                Ok(ordinal) => Some((agent_id, bundle, path.to_string(), Some(ordinal))),
                // A `#` that isn't an ordinal belongs to the path, odd as that is.
                Err(_) => Some((agent_id, bundle, rest.to_string(), None)),
            },
            None => Some((agent_id, bundle, rest.to_string(), None)),
        }
    }

    // ---- passages (specs/009-memory-semantic-chunking) ----

    /// Hash of the document body the stored passages were derived from, repeated on every row of
    /// that document. Comparing this against the live body is how drift is detected, rather than
    /// re-chunking and diffing spans (research Decision 12).
    fn content_hash(content: &str) -> String {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        content.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    /// Ordinals currently stored for a document, ascending. Deletion is driven by these rather
    /// than by re-deriving spans: a document shrinking from twelve passages to four must clear
    /// all twelve index rows, and only the stored ordinals know there were twelve.
    fn stored_ordinals(&self, agent_id: &str, bundle: &str, path: &str) -> Result<Vec<usize>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare(
            "SELECT ordinal FROM memory_passage WHERE agent_id = ?1 AND bundle = ?2 AND path = ?3 ORDER BY ordinal",
        )?;
        let rows = stmt
            .query_map(params![agent_id, bundle, path], |row| {
                row.get::<_, i64>(0).map(|o| o as usize)
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    /// The stored `content_hash` for a document, or `None` when it has no passages at all — which
    /// is the marker that it has not been converted yet (FR-040, research Decision 8).
    fn stored_content_hash(
        &self,
        agent_id: &str,
        bundle: &str,
        path: &str,
    ) -> Result<Option<String>> {
        let conn = self.conn.lock()?;
        let hash: Option<String> = conn
            .query_row(
                "SELECT content_hash FROM memory_passage WHERE agent_id = ?1 AND bundle = ?2 AND path = ?3 LIMIT 1",
                params![agent_id, bundle, path],
                |row| row.get(0),
            )
            .optional()?;
        Ok(hash)
    }

    /// Drop a document's passage rows and every matching `document_index` entry.
    ///
    /// The index entries are removed by *stored* ordinal, one `delete_index` call each (research
    /// Decision 5 — not a new trait method for one call site). Getting this wrong is the worst
    /// failure this feature can produce: a shrunk document would leave stale vectors pointing at
    /// spans that no longer exist, and searches would return text the document no longer has.
    async fn clear_passages(
        &self,
        agent_id: &str,
        bundle: &str,
        path: &str,
        indexer: &VizierIndexer,
    ) -> Result<()> {
        for ordinal in self.stored_ordinals(agent_id, bundle, path)? {
            let _ = indexer
                .delete_index(
                    "memory".into(),
                    Self::passage_indexer_key(agent_id, bundle, path, ordinal),
                )
                .await;
        }
        // A document written before this feature has one row under the unsuffixed key; clearing
        // it keeps whole-document relevance data from surviving alongside passages (FR-039).
        let _ = indexer
            .delete_index("memory".into(), Self::indexer_key(agent_id, bundle, path))
            .await;
        {
            let conn = self.conn.lock()?;
            conn.execute(
                "DELETE FROM memory_passage WHERE agent_id = ?1 AND bundle = ?2 AND path = ?3",
                params![agent_id, bundle, path],
            )?;
        }
        Ok(())
    }

    /// Chunk a document, store its passage coordinates, and index every passage in one batch.
    ///
    /// Replaces whatever was there before (FR-033). Returns the number of passages written.
    /// Errors are the caller's to swallow — FR-036 requires that a failure here never fails the
    /// save, and every caller logs rather than propagates.
    async fn write_passages(
        &self,
        agent_id: &str,
        bundle: &str,
        path: &str,
        title: &str,
        tags: &[String],
        content: &str,
        limits: &ChunkLimits,
        indexer: &VizierIndexer,
    ) -> Result<usize> {
        self.clear_passages(agent_id, bundle, path, indexer).await?;

        let spans = chunk_markdown(content, limits);
        if spans.is_empty() {
            return Ok(0);
        }
        let hash = Self::content_hash(content);

        {
            let conn = self.conn.lock()?;
            let tx = conn.transaction()?;
            for span in &spans {
                tx.execute(
                    "INSERT INTO memory_passage
                        (agent_id, bundle, path, ordinal, line_start, line_end, char_start, char_end, continues, content_hash)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                     ON CONFLICT (agent_id, bundle, path, ordinal) DO UPDATE SET
                        line_start = excluded.line_start, line_end = excluded.line_end,
                        char_start = excluded.char_start, char_end = excluded.char_end,
                        continues = excluded.continues, content_hash = excluded.content_hash",
                    params![
                        agent_id,
                        bundle,
                        path,
                        span.ordinal as i64,
                        span.line_start as i64,
                        span.line_end as i64,
                        span.char_start as i64,
                        span.char_end as i64,
                        span.continues_previous as i64,
                        hash,
                    ],
                )?;
            }
            tx.commit()?;
        }

        // Each passage is embedded with its document's title, tags and heading breadcrumb ahead
        // of its body, so a query matching only metadata still surfaces the document now that no
        // whole-document embedding exists to carry it (FR-006, data-model.md §7).
        let entries: Vec<(String, String)> = spans
            .iter()
            .map(|span| {
                let body = &content[span.char_start..span.char_end];
                let breadcrumb = heading_breadcrumb(content, span.char_start);
                (
                    Self::passage_indexer_key(agent_id, bundle, path, span.ordinal),
                    embedded_text(title, tags, &breadcrumb, body),
                )
            })
            .collect();

        indexer
            .add_document_indexes("memory".into(), entries)
            .await?;
        Ok(spans.len())
    }

    /// FR-036: a chunking or indexing failure must leave the document readable and the save
    /// successful. Logged, never propagated — the document becomes retrievable at passage level
    /// the next time it is written or the next time reconcile notices it has no rows.
    async fn write_passages_lenient(
        &self,
        agent_id: &str,
        bundle: &str,
        path: &str,
        title: &str,
        tags: &[String],
        content: &str,
        limits: &ChunkLimits,
        indexer: &VizierIndexer,
    ) {
        match self
            .write_passages(
                agent_id, bundle, path, title, tags, content, limits, indexer,
            )
            .await
        {
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(
                    agent_id,
                    bundle,
                    path,
                    "failed to build or index passages, memory saved without passage-level \
                     retrieval: {e}"
                );
            }
        }
    }

    // ---- documents and the memory graph (`memory_node` / `memory_edge`) ----

    /// One agent's memory writes take turns. On SQLite the transaction already holds the write
    /// lock; on PostgreSQL this also keeps two writes from deadlocking in `recompute_broken`.
    fn lock_agent(conn: &Connection, agent_id: &str) -> Result<()> {
        conn.lock_key(&format!("memory:{agent_id}"))?;
        Ok(())
    }

    /// The document at an address: its row, when that row holds a body. A row with a `NULL` body
    /// (left only by the file migration, for a file it could not move) is absent to every reader.
    fn load_document(
        conn: &Connection,
        agent_id: &str,
        bundle: &str,
        path: &str,
    ) -> Result<Option<Memory>> {
        let row = conn
            .query_row(
                "SELECT content, title, tags_json, created_at, updated_at, read_count
                 FROM memory_node
                 WHERE agent_id = ?1 AND bundle = ?2 AND path = ?3 AND content IS NOT NULL",
                params![agent_id, bundle, path],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, i64>(5)?,
                    ))
                },
            )
            .optional()?;
        Ok(row.map(
            |(content, title, tags_json, created_at, updated_at, read_count)| Memory {
                slug: path.to_string(),
                title,
                relations: parse_relations(&content),
                content,
                created_at: read_time(&created_at),
                updated_at: read_time(&updated_at),
                agent_id: agent_id.to_string(),
                bundle: bundle.to_string(),
                tags: serde_json::from_str(&tags_json).unwrap_or_default(),
                keywords: vec![],
                read_count: read_count as u64,
            },
        ))
    }

    /// `None` when there is no row at the address, otherwise whether that row holds a body.
    fn row_state(
        conn: &Connection,
        agent_id: &str,
        bundle: &str,
        path: &str,
    ) -> Result<Option<bool>> {
        let (rows, bodies): (i64, i64) = conn.query_row(
            "SELECT COUNT(*), COUNT(content) FROM memory_node
             WHERE agent_id = ?1 AND bundle = ?2 AND path = ?3",
            params![agent_id, bundle, path],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        Ok((rows > 0).then_some(bodies > 0))
    }

    /// Write a whole document: every column of its row, body included.
    fn store_document(
        conn: &Connection,
        agent_id: &str,
        bundle: &str,
        path: &str,
        fm: &MemoryFrontMatter,
        content: &str,
    ) -> Result<()> {
        let tags_json = serde_json::to_string(&fm.tags)?;
        conn.execute(
            "INSERT INTO memory_node
                (agent_id, bundle, path, slug, title, tags_json, created_at, updated_at,
                 read_count, content)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
             ON CONFLICT(agent_id, bundle, path) DO UPDATE SET
                slug = excluded.slug,
                title = excluded.title,
                tags_json = excluded.tags_json,
                created_at = excluded.created_at,
                updated_at = excluded.updated_at,
                read_count = excluded.read_count,
                content = excluded.content",
            params![
                agent_id,
                bundle,
                path,
                leaf_of(path),
                fm.title,
                tags_json,
                crate::time::stored(fm.created_at),
                crate::time::stored(fm.updated_at),
                fm.read_count as i64,
                content
            ],
        )?;
        Ok(())
    }

    fn delete_node(conn: &Connection, agent_id: &str, bundle: &str, path: &str) -> Result<()> {
        conn.execute(
            "DELETE FROM memory_node WHERE agent_id = ?1 AND bundle = ?2 AND path = ?3",
            params![agent_id, bundle, path],
        )?;
        Ok(())
    }

    /// The documents of an agent (or one of its bundles), without their bodies.
    fn list_nodes(conn: &Connection, agent_id: &str, bundle: Option<&str>) -> Result<Vec<NodeRow>> {
        let map_row = |row: &Row| -> sql::Result<NodeRow> {
            let tags_json: String = row.get(2)?;
            let created_at: String = row.get(3)?;
            let updated_at: String = row.get(4)?;
            Ok(NodeRow {
                bundle: row.get(0)?,
                path: row.get(1)?,
                title: row.get(5)?,
                tags: serde_json::from_str(&tags_json).unwrap_or_default(),
                created_at: read_time(&created_at),
                updated_at: read_time(&updated_at),
                read_count: row.get::<_, i64>(6)? as u64,
            })
        };

        let rows = if let Some(b) = bundle {
            let mut stmt = conn.prepare(
                "SELECT bundle, path, tags_json, created_at, updated_at, title, read_count
                 FROM memory_node WHERE agent_id = ?1 AND bundle = ?2 AND content IS NOT NULL",
            )?;
            stmt.query_map(params![agent_id, b], map_row)?
                .filter_map(|r| r.ok())
                .collect()
        } else {
            let mut stmt = conn.prepare(
                "SELECT bundle, path, tags_json, created_at, updated_at, title, read_count
                 FROM memory_node WHERE agent_id = ?1 AND content IS NOT NULL",
            )?;
            stmt.query_map(params![agent_id], map_row)?
                .filter_map(|r| r.ok())
                .collect()
        };
        Ok(rows)
    }

    fn relations_for(
        conn: &Connection,
        agent_id: &str,
        bundle: &str,
        path: &str,
    ) -> Result<Vec<String>> {
        let mut stmt = conn.prepare(
            "SELECT target_bundle, target_path, target_kind FROM memory_edge
             WHERE agent_id = ?1 AND source_bundle = ?2 AND source_path = ?3",
        )?;
        let rows = stmt.query_map(params![agent_id, bundle, path], |row| {
            let target_bundle: String = row.get(0)?;
            let target_path: Option<String> = row.get(1)?;
            let kind: String = row.get(2)?;
            Ok((target_bundle, target_path, kind))
        })?;

        let mut relations = Vec::new();
        for row in rows {
            let (target_bundle, target_path, kind) = row?;
            match kind.as_str() {
                "same_bundle" => relations.push(format!("{}.md", target_path.unwrap_or_default())),
                "cross_bundle_concept" => relations.push(format!(
                    "{}/{}",
                    target_bundle,
                    target_path.unwrap_or_default()
                )),
                _ => relations.push(target_bundle),
            }
        }
        Ok(relations)
    }

    fn rewrite_edges(
        conn: &Connection,
        agent_id: &str,
        source_bundle: &str,
        source_path: &str,
        relations: &[String],
    ) -> Result<()> {
        conn.execute(
            "DELETE FROM memory_edge WHERE agent_id = ?1 AND source_bundle = ?2 AND source_path = ?3",
            params![agent_id, source_bundle, source_path],
        )?;
        for relation in relations {
            let classified = Self::resolve_relation(conn, agent_id, source_bundle, relation)?;
            conn.execute(
                "INSERT INTO memory_edge
                    (agent_id, source_bundle, source_path, target_bundle, target_path, target_kind, broken)
                 VALUES (?1,?2,?3,?4,?5,?6,0)",
                params![
                    agent_id,
                    source_bundle,
                    source_path,
                    classified.target_bundle,
                    classified.target_path,
                    classified.kind
                ],
            )?;
        }
        Ok(())
    }

    fn node_exists(conn: &Connection, agent_id: &str, bundle: &str, path: &str) -> Result<bool> {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM memory_node
             WHERE agent_id = ?1 AND bundle = ?2 AND path = ?3 AND content IS NOT NULL",
            params![agent_id, bundle, path],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    fn bundle_has_any_node(conn: &Connection, agent_id: &str, bundle: &str) -> Result<bool> {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM memory_node
             WHERE agent_id = ?1 AND bundle = ?2 AND content IS NOT NULL",
            params![agent_id, bundle],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// `classify_relation` is purely syntactic — it can't tell "a nested path within my own
    /// bundle" from "a different bundle name that happens to look like a path segment," because
    /// both are written with the exact same relative-link syntax. In practice, agents very
    /// commonly write a cross-bundle reference as if the whole memory tree were one shared
    /// filesystem — `[label](books/great-gatsby.md)` from *inside* a different bundle, meaning
    /// "the `great-gatsby` concept in the `books` bundle," not "the nested path
    /// `books/great-gatsby` inside my own bundle." Resolve with existence checks: try the
    /// literal classification first (never overridden if it actually resolves, so a real nested
    /// path keeps working exactly as before); only fall back to a smarter reinterpretation when
    /// the literal target doesn't exist *and* the fallback's target does. Symmetric fallback for
    /// a bare `[[slug]]` legacy wikilink that doesn't name an existing bundle, matching
    /// research.md §6. Never returns an `Err` for an unresolvable link — an unresolved relation
    /// simply keeps its literal (and therefore broken) classification, exactly as before this
    /// existed.
    fn resolve_relation(
        conn: &Connection,
        agent_id: &str,
        source_bundle: &str,
        relation: &str,
    ) -> Result<ClassifiedLink> {
        let literal = classify_relation(source_bundle, relation);

        match literal.kind {
            "same_bundle" => {
                let path = literal.target_path.clone().unwrap_or_default();
                if Self::node_exists(conn, agent_id, source_bundle, &path)? {
                    return Ok(literal);
                }
                if let Some((maybe_bundle, rest)) = path.split_once('/') {
                    if !rest.is_empty()
                        && Self::bundle_has_any_node(conn, agent_id, maybe_bundle)?
                        && Self::node_exists(conn, agent_id, maybe_bundle, rest)?
                    {
                        return Ok(ClassifiedLink {
                            target_bundle: maybe_bundle.to_string(),
                            target_path: Some(rest.to_string()),
                            kind: "cross_bundle_concept",
                        });
                    }
                } else if Self::bundle_has_any_node(conn, agent_id, &path)? {
                    // No concept literally named `path` here, but a whole bundle by that name
                    // exists (e.g. content wrote `[books](books/)`, which parses to the
                    // same-bundle-shaped relation "books.md") — a whole-bundle reference.
                    return Ok(ClassifiedLink {
                        target_bundle: path,
                        target_path: None,
                        kind: "cross_bundle_bundle",
                    });
                }
                Ok(literal)
            }
            "cross_bundle_bundle" => {
                let bundle_name = literal.target_bundle.clone();
                if Self::bundle_has_any_node(conn, agent_id, &bundle_name)? {
                    return Ok(literal);
                }
                if Self::node_exists(conn, agent_id, source_bundle, &bundle_name)? {
                    return Ok(ClassifiedLink {
                        target_bundle: source_bundle.to_string(),
                        target_path: Some(bundle_name),
                        kind: "same_bundle",
                    });
                }
                Ok(literal)
            }
            _ => Ok(literal),
        }
    }

    fn delete_edges_from(
        conn: &Connection,
        agent_id: &str,
        bundle: &str,
        path: &str,
    ) -> Result<()> {
        conn.execute(
            "DELETE FROM memory_edge WHERE agent_id = ?1 AND source_bundle = ?2 AND source_path = ?3",
            params![agent_id, bundle, path],
        )?;
        Ok(())
    }

    fn recompute_broken(conn: &Connection, agent_id: &str) -> Result<()> {
        conn.execute(
            "UPDATE memory_edge SET broken = 1 WHERE agent_id = ?1
             AND target_kind IN ('same_bundle','cross_bundle_concept')
             AND NOT EXISTS (
                SELECT 1 FROM memory_node n
                WHERE n.agent_id = memory_edge.agent_id AND n.bundle = memory_edge.target_bundle
                  AND n.path = memory_edge.target_path AND n.content IS NOT NULL)",
            params![agent_id],
        )?;
        conn.execute(
            "UPDATE memory_edge SET broken = 0 WHERE agent_id = ?1
             AND target_kind IN ('same_bundle','cross_bundle_concept')
             AND EXISTS (
                SELECT 1 FROM memory_node n
                WHERE n.agent_id = memory_edge.agent_id AND n.bundle = memory_edge.target_bundle
                  AND n.path = memory_edge.target_path AND n.content IS NOT NULL)",
            params![agent_id],
        )?;
        conn.execute(
            "UPDATE memory_edge SET broken = 1 WHERE agent_id = ?1 AND target_kind = 'cross_bundle_bundle'
             AND NOT EXISTS (
                SELECT 1 FROM memory_node n
                WHERE n.agent_id = memory_edge.agent_id AND n.bundle = memory_edge.target_bundle
                  AND n.content IS NOT NULL)",
            params![agent_id],
        )?;
        conn.execute(
            "UPDATE memory_edge SET broken = 0 WHERE agent_id = ?1 AND target_kind = 'cross_bundle_bundle'
             AND EXISTS (
                SELECT 1 FROM memory_node n
                WHERE n.agent_id = memory_edge.agent_id AND n.bundle = memory_edge.target_bundle
                  AND n.content IS NOT NULL)",
            params![agent_id],
        )?;
        Ok(())
    }

    // ---- bundles ----

    /// The bundles that hold at least one document, sorted.
    fn bundle_names(conn: &Connection, agent_id: &str) -> Result<Vec<String>> {
        let mut stmt = conn.prepare(
            "SELECT DISTINCT bundle FROM memory_node WHERE agent_id = ?1 AND content IS NOT NULL",
        )?;
        let mut names: Vec<String> = stmt
            .query_map(params![agent_id], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();
        names.sort();
        Ok(names)
    }

    /// Bring a bundle's passages back in step with its documents, rebuilding only what actually
    /// needs it (FR-037, FR-038).
    ///
    /// Two conditions call for a rebuild, and both are answered without an embedder and without
    /// re-chunking: a document with **no** passage rows has never been converted, and a document
    /// whose stored `content_hash` differs from its body has drifted — saved while indexing was
    /// failing, or written by a release that predates passages. Comparing one hash answers the
    /// real question in one step; re-deriving spans to diff them would be strictly more
    /// expensive and would rebuild nothing extra (research Decision 12).
    ///
    /// Returns `(converted, failed)`. Never returns `Err` for a single document's failure — one
    /// bad document must not stop the scan.
    pub async fn reconcile_passages(
        &self,
        agent_id: &str,
        bundle: &str,
        limits: &ChunkLimits,
        indexer: &VizierIndexer,
    ) -> Result<(usize, usize)> {
        let paths: Vec<String> = {
            let conn = self.conn.lock()?;
            Self::list_nodes(&conn, agent_id, Some(bundle))?
                .into_iter()
                .map(|n| n.path)
                .collect()
        };

        let mut converted = 0usize;
        let mut failed = 0usize;

        for path in paths {
            let document = {
                let conn = self.conn.lock()?;
                Self::load_document(&conn, agent_id, bundle, &path)
            };
            let document = match document {
                Ok(Some(document)) => document,
                Ok(None) => continue,
                Err(e) => {
                    tracing::warn!(agent_id, bundle, path, "could not read document: {e}");
                    failed += 1;
                    continue;
                }
            };

            let stored_hash = self
                .stored_content_hash(agent_id, bundle, &path)
                .ok()
                .flatten();
            let live_hash = Self::content_hash(&document.content);
            match &stored_hash {
                // Absence of rows is the only resume marker: already converted, no drift, skip.
                Some(h) if *h == live_hash => continue,
                Some(_) => tracing::debug!(agent_id, bundle, path, "passages drifted, rebuilding"),
                None => {}
            }

            match self
                .write_passages(
                    agent_id,
                    bundle,
                    &path,
                    &document.title,
                    &document.tags,
                    &document.content,
                    limits,
                    indexer,
                )
                .await
            {
                Ok(_) => converted += 1,
                Err(e) => {
                    tracing::warn!(agent_id, bundle, path, "failed to build passages: {e}");
                    failed += 1;
                }
            }
        }

        Ok((converted, failed))
    }

    /// Drop every passage coordinate of one agent, so the next reconcile treats its whole corpus as
    /// unconverted and indexes it again. Called when the embedding model's vector width changes and
    /// the agent's vectors move to another table.
    pub async fn reset_passages(&self, agent_id: &str) -> Result<()> {
        self.conn
            .lock()?
            .execute("DELETE FROM memory_passage WHERE agent_id = ?1", [agent_id])?;
        Ok(())
    }

    /// `reconcile_passages` across every bundle the agent has. This is what the per-agent
    /// conversion task at spawn calls (FR-038).
    pub async fn reconcile_all_passages(
        &self,
        agent_id: &str,
        limits: &ChunkLimits,
        indexer: &VizierIndexer,
    ) -> Result<(usize, usize)> {
        let bundles = {
            let conn = self.conn.lock()?;
            Self::bundle_names(&conn, agent_id)?
        };
        let mut converted = 0usize;
        let mut failed = 0usize;
        for bundle in bundles {
            match self
                .reconcile_passages(agent_id, &bundle, limits, indexer)
                .await
            {
                Ok((c, f)) => {
                    converted += c;
                    failed += f;
                }
                Err(e) => {
                    tracing::warn!(agent_id, bundle, "passage reconcile failed for bundle: {e}");
                    failed += 1;
                }
            }
            // Yield between bundles so a large corpus cannot monopolise the runtime while the
            // agent is serving requests (FR-040).
            tokio::task::yield_now().await;
        }
        Ok((converted, failed))
    }

    // ---- the exported index and log documents ----

    /// `index.md` of an exported bundle: a table of its documents, sorted by path.
    fn render_index(bundle: &str, nodes: &[NodeRow]) -> String {
        let mut body = format!(
            "# {bundle}\n\n{} concept(s) in this bundle.\n\n| Path | Title | Tags | Updated |\n|---|---|---|---|\n",
            nodes.len()
        );
        for n in nodes {
            body += &format!(
                "| [{path}]({path}.md) | {title} | {tags} | {updated} |\n",
                path = n.path,
                title = n.title,
                tags = n.tags.join(", "),
                updated = n.updated_at.to_rfc3339(),
            );
        }
        body
    }

    /// `log.md` of an exported bundle: one line per revision, oldest first.
    fn render_log(bundle: &str, revisions: &[(String, MemoryRevisionRow)]) -> String {
        let mut body = format!("# {bundle} log\n\n");
        // A deletion entry holds no content, so it names the title its document had before.
        let mut titles: HashMap<&str, String> = HashMap::new();
        let mut present: HashSet<&str> = HashSet::new();
        for (path, row) in revisions {
            let action = if row.deleted {
                present.remove(path.as_str());
                "deleted"
            } else if present.insert(path.as_str()) {
                "created"
            } else {
                "updated"
            };
            if let Some(Ok((fm, _))) = row.content.as_deref().map(parse_memory_canonical) {
                titles.insert(path, fm.title);
            }
            let title = titles.get(path.as_str()).unwrap_or(path);
            let at = crate::time::from_millis_opt(row.created_at)
                .unwrap_or_else(crate::time::now)
                .to_rfc3339();
            body += &format!("- {at} — {action} `{path}` \"{title}\"\n");
        }
        body
    }

    fn node_to_memory(conn: &Connection, agent_id: &str, n: &NodeRow) -> Result<Memory> {
        let relations = Self::relations_for(conn, agent_id, &n.bundle, &n.path)?;
        Ok(Memory {
            slug: n.path.clone(),
            title: n.title.clone(),
            content: String::new(),
            created_at: n.created_at,
            updated_at: n.updated_at,
            agent_id: agent_id.to_string(),
            bundle: n.bundle.clone(),
            tags: n.tags.clone(),
            keywords: vec![],
            relations,
            read_count: n.read_count,
        })
    }

    // ---- MemoryStorage-shaped methods (called by the trait impl in sqlite/memory.rs) ----

    /// A write judged against the default size limits — the shape every existing caller and
    /// test uses. Callers that carry per-agent limits use [`Self::write_memory_with`].
    #[allow(clippy::too_many_arguments)]
    pub async fn write_memory(
        &self,
        agent_id: String,
        bundle: Option<String>,
        path: Option<String>,
        create_only: bool,
        title: String,
        content: String,
        tags: Vec<String>,
        origin: &RevisionOrigin,
        indexer: &VizierIndexer,
        limits: &ChunkLimits,
    ) -> Result<WrittenMemory> {
        self.write_memory_with(
            agent_id,
            bundle,
            path,
            create_only,
            title,
            content,
            tags,
            origin,
            indexer,
            limits,
            &WritePolicy::new(MemoryLimits::default()),
        )
        .await
    }

    /// A write judged against `policy`. Both this and [`Self::write_memory`] land in
    /// [`Self::write_memory_inner`], which is the only place any of the rules live.
    #[allow(clippy::too_many_arguments)]
    pub async fn write_memory_with(
        &self,
        agent_id: String,
        bundle: Option<String>,
        path: Option<String>,
        create_only: bool,
        title: String,
        content: String,
        tags: Vec<String>,
        origin: &RevisionOrigin,
        indexer: &VizierIndexer,
        limits: &ChunkLimits,
        policy: &WritePolicy,
    ) -> Result<WrittenMemory> {
        self.write_memory_inner(
            agent_id,
            bundle,
            path,
            create_only,
            title,
            content,
            tags,
            origin,
            indexer,
            limits,
            policy,
        )
        .await
    }

    /// The one write path.
    #[allow(clippy::too_many_arguments)]
    async fn write_memory_inner(
        &self,
        agent_id: String,
        bundle: Option<String>,
        path: Option<String>,
        create_only: bool,
        title: String,
        content: String,
        tags: Vec<String>,
        origin: &RevisionOrigin,
        indexer: &VizierIndexer,
        limits: &ChunkLimits,
        policy: &WritePolicy,
    ) -> Result<WrittenMemory> {
        let bundle = bundle.unwrap_or_else(default_bundle);
        let path = normalize_path(&path.unwrap_or_else(|| slugify!(&title)));
        if path.is_empty() {
            return Err(anyhow!("resolved memory path is empty"));
        }

        check_address(&agent_id, &bundle, &path)?;

        let memory_limits = policy.limits_for(&bundle, &path);

        // The document, its revision, its links and the broken-link flags change together or not
        // at all. Nothing is awaited while the transaction is open.
        let (frontmatter, warning) = {
            let conn = self.conn.lock()?;
            let tx = conn.transaction()?;
            Self::lock_agent(&tx, &agent_id)?;
            let existing = Self::load_document(&tx, &agent_id, &bundle, &path)?;

            // The reserved-bundle guard runs before anything is **persisted**, so a refused write
            // leaves the document, its passages and its revisions untouched (contract rule 1,
            // data-model invariant 1). It reads `existing` first on purpose: `core` was an
            // ordinary bundle name before it was reserved, so an install may already hold
            // unrelated documents there. Refusing to *create* another one is FR-013; refusing to
            // keep an existing one maintainable would make it readable and permanently
            // un-editable, which is a data-access trap rather than protection.
            if is_core_bundle(&bundle)
                && !is_identity_address(&bundle, &path)
                && existing.is_none()
            {
                return Err(anyhow!(
                    "the core bundle holds only the identity document, at path '{SOUL_PATH}' — \
                     '{path}' cannot be written there"
                ));
            }
            // This (bundle, path) collision check (FR-011) is also what keeps a new concept from
            // silently clobbering one that already holds the address.
            if existing.is_some() && create_only {
                return Err(anyhow!(
                    "a memory already exists at bundle '{bundle}' path '{path}'"
                ));
            }

            // The ceiling applies before anything is written, so a refusal cannot leave a partial
            // document, replaced passages or a revision (contract rule 7, SC-009).
            // Grandfathering: a document already above the ceiling accepts a write that brings it
            // closer, which is how a legacy over-cap memory ever gets fixed (FR-019, FR-021).
            if !policy.bypass_cap && memory_limits.exceeds_cap(content.len()) {
                let stored_len = existing.as_ref().map(|m| m.content.len()).unwrap_or(0);
                if !(stored_len > memory_limits.content_cap && content.len() < stored_len) {
                    return Err(anyhow!(too_large_message(
                        &bundle,
                        &path,
                        content.len(),
                        &memory_limits
                    )));
                }
            }
            let warning = if memory_limits.warns_at(content.len()) {
                Some(WriteWarning {
                    bytes: content.len(),
                    limit: memory_limits.warn_threshold(),
                })
            } else {
                None
            };

            let now = crate::time::now();
            let (created_at, read_count) = existing
                .as_ref()
                .map(|m| (m.created_at, m.read_count))
                .unwrap_or((now, 0));

            let relations = parse_relations(&content);
            let frontmatter = MemoryFrontMatter {
                slug: path.clone(),
                title: title.clone(),
                created_at,
                updated_at: now,
                agent_id: agent_id.clone(),
                bundle: bundle.clone(),
                tags: tags.clone(),
                keywords: vec![],
                relations: relations.clone(),
                read_count,
            };

            let canonical_before = existing.as_ref().and_then(canonical_of);
            let canonical_new = memory_canonical(&title, &tags, &content)?;

            Self::store_document(&tx, &agent_id, &bundle, &path, &frontmatter, &content)?;
            memory_revision::record(
                &tx,
                &agent_id,
                &bundle,
                &path,
                Some(&canonical_new),
                origin,
                canonical_before.as_deref(),
            )?;
            Self::rewrite_edges(&tx, &agent_id, &bundle, &path, &relations)?;
            Self::recompute_broken(&tx, &agent_id)?;
            tx.commit()?;
            (frontmatter, warning)
        };

        // Passages replace the whole-document index entry entirely (FR-033, FR-039): the old
        // rows and their vectors go, the new ones are inserted, and the batch is indexed in one
        // embedding call. It runs after the commit, and a failure here is logged and swallowed —
        // the document is saved and must stay readable (FR-036).
        self.write_passages_lenient(
            &agent_id, &bundle, &path, &title, &tags, &content, limits, indexer,
        )
        .await;

        Ok(WrittenMemory {
            memory: memory_from_frontmatter(frontmatter, content),
            warning,
        })
    }

    /// Seed the identity document at `core/soul` from whatever the agent had before this
    /// change (specs/043-unify-core-and-memory, FR-002/FR-003/FR-025).
    ///
    /// Order: the document already exists → nothing to do; else the legacy `agent_core` row;
    /// else the embedded template. The content is carried **verbatim**, with one provenance
    /// line appended, written **once** and never rewritten. The write carries the cap bypass,
    /// so an identity larger than the new ceiling still migrates — an upgrade must never be
    /// blocked or made lossy by a limit introduced in the same change.
    ///
    /// Idempotent: a second call with the document present does nothing (data-model rule 16).
    /// Returns `true` when it wrote.
    pub async fn seed_identity(
        &self,
        agent_id: &str,
        legacy: Option<String>,
        indexer: &VizierIndexer,
        limits: &ChunkLimits,
    ) -> Result<bool> {
        // Any row counts, even one without a body: that is an identity the file migration has
        // not moved yet, and writing the template over it would lose it.
        let recorded = {
            let conn = self.conn.lock()?;
            Self::row_state(&conn, agent_id, CORE_BUNDLE, SOUL_PATH)?.is_some()
        };
        if recorded {
            return Ok(false);
        }

        let (body, source) = match legacy.filter(|l| !l.trim().is_empty()) {
            Some(core) => (
                core,
                "the identity record this agent had before memory was unified",
            ),
            None => (
                crate::constant::CORE_MD.to_string(),
                "the default identity template, because this agent had none",
            ),
        };

        let content = format!(
            "{}

<!-- carried over from {source} when identity moved into memory \
(specs/043-unify-core-and-memory). Edit it like any other memory. -->
",
            body.trim_end()
        );

        self.write_memory_with(
            agent_id.to_string(),
            Some(CORE_BUNDLE.to_string()),
            Some(SOUL_PATH.to_string()),
            false,
            "Core".to_string(),
            content,
            vec![],
            &RevisionOrigin::system(RevisionTrigger::Baseline),
            indexer,
            limits,
            &WritePolicy::migrating(),
        )
        .await?;
        Ok(true)
    }

    /// Semantic search over passages (FR-008–FR-016).
    ///
    /// Over-fetch `limit * 5`, cap per document, merge adjacent ordinals, then take `limit` — in
    /// that order. The over-fetch is forced by two existing behaviours: the vec0 query applies its
    /// `LIMIT` *before* the threshold filter runs in Rust, and one long document can now occupy
    /// many of the fetched rows. Capping before merging would count passages that are about to
    /// become one result, which is why the merge comes second (research Decision 7).
    #[allow(clippy::too_many_arguments)]
    pub async fn query_memory(
        &self,
        agent_id: String,
        bundle: Option<String>,
        query: String,
        limit: usize,
        threshold: f64,
        per_document: usize,
        indexer: &VizierIndexer,
        limits: &ChunkLimits,
    ) -> Result<Vec<MemoryPassageResult>> {
        let fetch_limit = (limit * 5).max(limit);
        let hits = indexer
            .search_document_index("memory".into(), query, fetch_limit, threshold)
            .await?;

        // Group the surviving hits by source document, so each document is read exactly once no
        // matter how many of its passages matched.
        let mut by_document: Vec<(String, Vec<PassageHit>)> = Vec::new();
        for hit in hits {
            let Some((hit_agent, hit_bundle, hit_path, ordinal)) =
                Self::parse_indexer_key(&hit.path)
            else {
                continue;
            };
            if hit_agent != agent_id {
                continue;
            }
            if let Some(want) = &bundle {
                if &hit_bundle != want {
                    continue;
                }
            }
            let doc_key = format!("{hit_bundle}\u{0}{hit_path}");
            match by_document.iter_mut().find(|(k, _)| *k == doc_key) {
                Some((_, ords)) => ords.push(PassageHit {
                    ordinal,
                    score: hit.score,
                }),
                None => by_document.push((
                    doc_key,
                    vec![PassageHit {
                        ordinal,
                        score: hit.score,
                    }],
                )),
            }
        }

        let mut candidates: Vec<(MemoryPassageResult, SourceSignals)> = Vec::new();
        for (doc_key, mut ordinals) in by_document {
            let (doc_bundle, doc_path) = doc_key
                .split_once('\u{0}')
                .map(|(b, p)| (b.to_string(), p.to_string()))
                .expect("doc_key was built with the separator");

            // A stale or invalid indexer entry degrades to "no candidate", not a hard error.
            let Ok(Some(memory)) = self
                .get_memory_detail(agent_id.clone(), Some(doc_bundle.clone()), doc_path.clone())
                .await
            else {
                continue;
            };

            // Best score first, then cap how much of the result set this one document may take
            // (FR-012). Ordinals are deduplicated on the way, since the same passage can arrive
            // twice when both a passage key and a legacy document key matched.
            ordinals.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            let mut seen = HashSet::new();
            ordinals.retain(|hit| seen.insert(hit.ordinal));
            ordinals.truncate(per_document.max(1));

            let spans = self.passage_spans_for(
                &agent_id,
                &doc_bundle,
                &doc_path,
                &memory.content,
                limits,
            );

            // Merge consecutive ordinals of this document into one result (FR-011), applied after
            // the cap. A merged result spans the first passage's start to the last one's end and
            // keeps the best score of the group.
            let mut kept: Vec<(usize, f64)> = ordinals
                .into_iter()
                .filter_map(|hit| hit.ordinal.map(|o| (o, hit.score)))
                .collect();
            // A hit on a legacy whole-document key carries no ordinal; represent it by the
            // document's first passage so it still returns a passage rather than a whole body.
            if kept.is_empty() {
                continue;
            }
            kept.sort_by_key(|(o, _)| *o);

            let mut groups: Vec<(usize, usize, f64)> = Vec::new();
            for (ordinal, score) in kept {
                match groups.last_mut() {
                    Some((_, end, best)) if *end + 1 == ordinal => {
                        *end = ordinal;
                        if score > *best {
                            *best = score;
                        }
                    }
                    _ => groups.push((ordinal, ordinal, score)),
                }
            }

            for (start, end, score) in groups {
                let Some(first) = spans.iter().find(|s| s.ordinal == start) else {
                    continue;
                };
                let last = spans.iter().find(|s| s.ordinal == end).unwrap_or(first);
                // Slice out of the document body already read above — no passage text is stored
                // (research Decision 3). The bounds are clamped and nudged onto character
                // boundaries because a vector can lag behind the latest write, and slicing
                // mid-character would panic.
                let text = slice_passage(&memory.content, first.char_start, last.char_end);
                candidates.push((
                    MemoryPassageResult {
                        bundle: doc_bundle.clone(),
                        path: doc_path.clone(),
                        title: memory.title.clone(),
                        ordinal: start,
                        ordinal_end: end,
                        line_start: first.line_start,
                        line_end: last.line_end,
                        text,
                        score,
                        truncated: false,
                    },
                    SourceSignals {
                        updated_at: memory.updated_at,
                        read_count: memory.read_count,
                    },
                ));
            }
        }

        let all_memories = self.get_all_agent_memory(agent_id, bundle).await?;
        let ranked = crate::storage::rerank::rerank_passages(candidates, &all_memories);
        Ok(ranked.into_iter().take(limit).collect())
    }

    /// The spans a search slices a document by: the stored ones while they match its body,
    /// otherwise spans derived on the spot. A search writes nothing: `reconcile_passages`, run
    /// at agent start, rebuilds missing or drifted rows and their vectors (FR-037).
    fn passage_spans_for(
        &self,
        agent_id: &str,
        bundle: &str,
        path: &str,
        content: &str,
        limits: &ChunkLimits,
    ) -> Vec<crate::storage::chunk::PassageSpan> {
        let stored = self
            .load_passage_spans(agent_id, bundle, path)
            .unwrap_or_default();
        let hash = Self::content_hash(content);
        let fresh = self
            .stored_content_hash(agent_id, bundle, path)
            .ok()
            .flatten()
            .is_some_and(|h| h == hash);

        if !stored.is_empty() && fresh {
            return stored;
        }
        chunk_markdown(content, limits)
    }

    fn load_passage_spans(
        &self,
        agent_id: &str,
        bundle: &str,
        path: &str,
    ) -> Result<Vec<crate::storage::chunk::PassageSpan>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare(
            "SELECT ordinal, line_start, line_end, char_start, char_end, continues
             FROM memory_passage
             WHERE agent_id = ?1 AND bundle = ?2 AND path = ?3
             ORDER BY ordinal",
        )?;
        let spans = stmt
            .query_map(params![agent_id, bundle, path], |row| {
                Ok(crate::storage::chunk::PassageSpan {
                    ordinal: row.get::<_, i64>(0)? as usize,
                    line_start: row.get::<_, i64>(1)? as usize,
                    line_end: row.get::<_, i64>(2)? as usize,
                    char_start: row.get::<_, i64>(3)? as usize,
                    char_end: row.get::<_, i64>(4)? as usize,
                    continues_previous: row.get::<_, i64>(5)? != 0,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(spans)
    }

    pub async fn get_all_agent_memory(
        &self,
        agent_id: String,
        bundle: Option<String>,
    ) -> Result<Vec<Memory>> {
        let conn = self.conn.lock()?;
        Self::list_nodes(&conn, &agent_id, bundle.as_deref())?
            .iter()
            .map(|n| Self::node_to_memory(&conn, &agent_id, n))
            .collect()
    }

    pub async fn get_filtered_memories(
        &self,
        params: MemoryQueryParams,
    ) -> Result<PaginatedMemory> {
        let all = self
            .get_all_agent_memory(params.agent_id.clone(), params.bundle.clone())
            .await?;

        let mut filtered: Vec<Memory> = all
            .into_iter()
            .filter(|m| {
                if let Some(tags) = &params.tags {
                    if !tags.is_empty() && !tags.iter().any(|t| m.tags.contains(t)) {
                        return false;
                    }
                }
                true
            })
            .collect();

        let total = filtered.len();

        filtered.sort_by(|a, b| {
            let ord = match params.sort_by.as_deref() {
                Some("title") => a.title.cmp(&b.title),
                Some("slug") => a.slug.cmp(&b.slug),
                _ => b.updated_at.cmp(&a.updated_at),
            };
            if params.sort_order.as_deref() == Some("asc") {
                ord.reverse()
            } else {
                ord
            }
        });

        filtered = filtered
            .into_iter()
            .skip(params.offset)
            .take(params.limit)
            .collect();

        Ok(PaginatedMemory {
            memories: filtered,
            total,
            offset: params.offset,
            limit: params.limit,
        })
    }

    /// One document with its body. Its relations are parsed from that body.
    pub async fn get_memory_detail(
        &self,
        agent_id: String,
        bundle: Option<String>,
        path: String,
    ) -> Result<Option<Memory>> {
        let bundle = bundle.unwrap_or_else(default_bundle);
        let path = normalize_path(&path);
        check_address(&agent_id, &bundle, &path)?;
        let conn = self.conn.lock()?;
        Self::load_document(&conn, &agent_id, &bundle, &path)
    }

    pub async fn get_related_memories(
        &self,
        agent_id: String,
        bundle: Option<String>,
        path: String,
    ) -> Result<Vec<Memory>> {
        let bundle = bundle.unwrap_or_else(default_bundle);
        let path = normalize_path(&path);

        let mut result = Vec::new();
        let mut seen: HashSet<(String, String)> = HashSet::new();

        struct EdgeTarget {
            bundle: String,
            path: Option<String>,
            kind: String,
        }
        struct EdgeSource {
            bundle: String,
            path: String,
        }

        let outgoing: Vec<EdgeTarget> = {
            let conn = self.conn.lock()?;
            let mut stmt = conn.prepare(
                "SELECT target_bundle, target_path, target_kind FROM memory_edge
                 WHERE agent_id = ?1 AND source_bundle = ?2 AND source_path = ?3",
            )?;
            stmt.query_map(params![agent_id, bundle, path], |row| {
                Ok(EdgeTarget {
                    bundle: row.get(0)?,
                    path: row.get(1)?,
                    kind: row.get(2)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect()
        };

        for edge in outgoing {
            match edge.kind.as_str() {
                "cross_bundle_bundle" => {
                    let nodes = {
                        let conn = self.conn.lock()?;
                        Self::list_nodes(&conn, &agent_id, Some(&edge.bundle))?
                    };
                    for n in nodes {
                        // A broken/invalid target degrades to "absent," never a hard error
                        // (FR-013's broken-link tolerance).
                        if let Ok(Some(memory)) = self
                            .get_memory_detail(agent_id.clone(), Some(edge.bundle.clone()), n.path)
                            .await
                        {
                            if seen.insert((memory.bundle.clone(), memory.slug.clone())) {
                                result.push(memory);
                            }
                        }
                    }
                }
                _ => {
                    if let Some(target_path) = edge.path {
                        if let Ok(Some(memory)) = self
                            .get_memory_detail(
                                agent_id.clone(),
                                Some(edge.bundle.clone()),
                                target_path,
                            )
                            .await
                        {
                            if seen.insert((memory.bundle.clone(), memory.slug.clone())) {
                                result.push(memory);
                            }
                        }
                    }
                }
            }
        }

        let incoming: Vec<EdgeSource> = {
            let conn = self.conn.lock()?;
            let mut stmt = conn.prepare(
                "SELECT source_bundle, source_path FROM memory_edge
                 WHERE agent_id = ?1 AND (
                    (target_kind IN ('same_bundle','cross_bundle_concept') AND target_bundle = ?2 AND target_path = ?3)
                    OR (target_kind = 'cross_bundle_bundle' AND target_bundle = ?2)
                 )",
            )?;
            stmt.query_map(params![agent_id, bundle, path], |row| {
                Ok(EdgeSource {
                    bundle: row.get(0)?,
                    path: row.get(1)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect()
        };

        for edge in incoming {
            if let Ok(Some(memory)) = self
                .get_memory_detail(agent_id.clone(), Some(edge.bundle.clone()), edge.path)
                .await
            {
                if seen.insert((memory.bundle.clone(), memory.slug.clone())) {
                    result.push(memory);
                }
            }
        }

        Ok(result)
    }

    pub async fn get_memory_graph(
        &self,
        agent_id: String,
        bundle: Option<String>,
        search: Option<String>,
    ) -> Result<MemoryGraph> {
        match bundle {
            None => {
                let bundles = {
                    let conn = self.conn.lock()?;
                    Self::bundle_names(&conn, &agent_id)?
                };
                let bundle_set: HashSet<String> = bundles.iter().cloned().collect();

                let mut nodes: Vec<MemoryGraphNode> = bundles
                    .iter()
                    .map(|b| MemoryGraphNode {
                        slug: b.clone(),
                        bundle: b.clone(),
                        title: b.clone(),
                        tags: vec![],
                        agent_id: agent_id.clone(),
                        boundary: false,
                    })
                    .collect();

                let pairs: Vec<(String, String)> = {
                    let conn = self.conn.lock()?;
                    let mut stmt = conn.prepare(
                        "SELECT DISTINCT source_bundle, target_bundle FROM memory_edge
                         WHERE agent_id = ?1 AND target_kind IN ('cross_bundle_concept','cross_bundle_bundle')",
                    )?;
                    stmt.query_map(params![agent_id], |row| Ok((row.get(0)?, row.get(1)?)))?
                        .filter_map(|r| r.ok())
                        .collect()
                };

                let mut seen_pairs = HashSet::new();
                let mut edges = Vec::new();
                for (source, target) in pairs {
                    if source == target {
                        continue;
                    }
                    if !seen_pairs.insert((source.clone(), target.clone())) {
                        continue;
                    }
                    edges.push(MemoryGraphEdge {
                        source,
                        broken: !bundle_set.contains(&target),
                        target,
                    });
                }

                nodes.sort_by(|a, b| a.slug.cmp(&b.slug));
                edges.sort_by(|a, b| a.source.cmp(&b.source).then(a.target.cmp(&b.target)));
                let initial_slugs = compute_initial_slugs(&nodes, search.as_deref());

                Ok(MemoryGraph {
                    nodes,
                    edges,
                    initial_slugs,
                })
            }
            Some(name) => {
                let node_rows = {
                    let conn = self.conn.lock()?;
                    Self::list_nodes(&conn, &agent_id, Some(&name))?
                };
                let path_set: HashSet<String> = node_rows.iter().map(|n| n.path.clone()).collect();

                let mut nodes: Vec<MemoryGraphNode> = node_rows
                    .iter()
                    .map(|n| MemoryGraphNode {
                        slug: n.path.clone(),
                        bundle: name.clone(),
                        title: n.title.clone(),
                        tags: n.tags.clone(),
                        agent_id: agent_id.clone(),
                        boundary: false,
                    })
                    .collect();

                let edge_rows: Vec<(String, String, Option<String>, String)> = {
                    let conn = self.conn.lock()?;
                    let mut stmt = conn.prepare(
                        "SELECT source_path, target_bundle, target_path, target_kind FROM memory_edge
                         WHERE agent_id = ?1 AND source_bundle = ?2",
                    )?;
                    stmt.query_map(params![agent_id, name], |row| {
                        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
                    })?
                    .filter_map(|r| r.ok())
                    .collect()
                };

                let mut edges = Vec::new();
                let mut boundary_bundles: BTreeSet<String> = BTreeSet::new();
                for (source_path, target_bundle, target_path, kind) in edge_rows {
                    if kind == "same_bundle" {
                        let target = target_path.unwrap_or_default();
                        let broken = !path_set.contains(&target);
                        edges.push(MemoryGraphEdge {
                            source: source_path,
                            target,
                            broken,
                        });
                    } else {
                        boundary_bundles.insert(target_bundle.clone());
                        edges.push(MemoryGraphEdge {
                            source: source_path,
                            target: target_bundle,
                            broken: false,
                        });
                    }
                }

                let all_bundles: HashSet<String> = {
                    let conn = self.conn.lock()?;
                    Self::bundle_names(&conn, &agent_id)?.into_iter().collect()
                };
                for b in &boundary_bundles {
                    nodes.push(MemoryGraphNode {
                        slug: b.clone(),
                        bundle: b.clone(),
                        title: b.clone(),
                        tags: vec![],
                        agent_id: agent_id.clone(),
                        boundary: true,
                    });
                }
                for edge in edges.iter_mut() {
                    if boundary_bundles.contains(&edge.target) {
                        edge.broken = !all_bundles.contains(&edge.target);
                    }
                }

                nodes.sort_by(|a, b| a.slug.cmp(&b.slug));
                edges.sort_by(|a, b| a.source.cmp(&b.source).then(a.target.cmp(&b.target)));
                let initial_slugs = compute_initial_slugs(&nodes, search.as_deref());

                Ok(MemoryGraph {
                    nodes,
                    edges,
                    initial_slugs,
                })
            }
        }
    }

    pub async fn has_incoming_links(
        &self,
        agent_id: String,
        bundle: Option<String>,
        path: String,
    ) -> Result<bool> {
        let bundle = bundle.unwrap_or_else(default_bundle);
        let path = normalize_path(&path);
        let conn = self.conn.lock()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM memory_edge WHERE agent_id = ?1 AND (
                (target_kind IN ('same_bundle','cross_bundle_concept') AND target_bundle = ?2 AND target_path = ?3)
                OR (target_kind = 'cross_bundle_bundle' AND target_bundle = ?2)
             )",
            params![agent_id, bundle, path],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub async fn delete_memory(
        &self,
        agent_id: String,
        bundle: Option<String>,
        path: String,
        origin: &RevisionOrigin,
        indexer: &VizierIndexer,
    ) -> Result<()> {
        let bundle = bundle.unwrap_or_else(default_bundle);
        let path = normalize_path(&path);
        check_address(&agent_id, &bundle, &path)?;

        {
            let conn = self.conn.lock()?;
            let tx = conn.transaction()?;
            Self::lock_agent(&tx, &agent_id)?;
            let before = Self::load_document(&tx, &agent_id, &bundle, &path)?
                .as_ref()
                .and_then(canonical_of);

            // Recorded before the row goes, so a deletion entry always follows the content it
            // removed (and a baseline is seeded first if the document pre-dates history).
            memory_revision::record(
                &tx,
                &agent_id,
                &bundle,
                &path,
                None,
                origin,
                before.as_deref(),
            )?;
            Self::delete_node(&tx, &agent_id, &bundle, &path)?;
            Self::delete_edges_from(&tx, &agent_id, &bundle, &path)?;
            Self::recompute_broken(&tx, &agent_id)?;
            tx.commit()?;
        }

        // Clears the passage rows and every one of their index entries, by stored ordinal
        // (FR-034). `clear_passages` also removes the legacy unsuffixed document key, so this is
        // the whole of the index cleanup for a deleted memory.
        if let Err(e) = self
            .clear_passages(&agent_id, &bundle, &path, indexer)
            .await
        {
            tracing::warn!(
                agent_id,
                bundle,
                path,
                "failed to clear passages for a deleted memory: {e}"
            );
        }

        Ok(())
    }

    pub async fn increment_read_count(
        &self,
        agent_id: String,
        bundle: Option<String>,
        path: String,
    ) -> Result<()> {
        let bundle = bundle.unwrap_or_else(default_bundle);
        let path = normalize_path(&path);
        self.conn.lock()?.execute(
            "UPDATE memory_node SET read_count = read_count + 1
             WHERE agent_id = ?1 AND bundle = ?2 AND path = ?3 AND content IS NOT NULL",
            params![agent_id, bundle, path],
        )?;
        Ok(())
    }

    pub async fn list_bundles(&self, agent_id: String) -> Result<Vec<BundleSummary>> {
        let conn = self.conn.lock()?;
        let mut bundles = Self::bundle_names(&conn, &agent_id)?;
        // The reserved bundle is always listed, even while its document is absent — an empty
        // `core` is a real state, and dropping it from the listing would make a deleted soul
        // look like a deleted bundle (FR-012, spec edge case).
        if !bundles.iter().any(|b| is_core_bundle(b)) {
            bundles.push(crate::schema::CORE_BUNDLE.to_string());
        }

        let mut result = Vec::new();
        for bundle in bundles {
            let (count, updated): (i64, Option<String>) = conn.query_row(
                "SELECT COUNT(*), MAX(updated_at) FROM memory_node
                 WHERE agent_id = ?1 AND bundle = ?2 AND content IS NOT NULL",
                params![agent_id, bundle],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            let updated_at = updated
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|d| d.with_timezone(&Local));
            let links: i64 = conn.query_row(
                "SELECT COUNT(*) FROM memory_edge WHERE agent_id = ?1 AND source_bundle = ?2",
                params![agent_id, bundle],
                |row| row.get(0),
            )?;
            result.push(BundleSummary {
                protected: is_core_bundle(&bundle),
                name: bundle,
                concept_count: count as usize,
                updated_at,
                link_count: links as usize,
            });
        }
        Ok(result)
    }

    pub async fn delete_bundle(
        &self,
        agent_id: String,
        bundle: String,
        force: bool,
        origin: &RevisionOrigin,
        indexer: &VizierIndexer,
    ) -> Result<()> {
        // The reserved bundle cannot be removed at all — not even with `force`, which is why
        // this check sits above the non-empty branch rather than in it (FR-012, data-model
        // invariant 3). The document inside it stays deletable, as an ordinary memory.
        if is_core_bundle(&bundle) {
            return Err(anyhow!(
                "bundle '{bundle}' is reserved and cannot be deleted; delete its documents \
                 individually instead"
            ));
        }
        let concept_paths: Vec<String> = {
            let conn = self.conn.lock()?;
            let tx = conn.transaction()?;
            Self::lock_agent(&tx, &agent_id)?;
            let concept_paths: Vec<String> = Self::list_nodes(&tx, &agent_id, Some(&bundle))?
                .into_iter()
                .map(|n| n.path)
                .collect();

            if !concept_paths.is_empty() && !force {
                return Err(anyhow!(
                    "bundle '{bundle}' still has {} concept(s); delete them first",
                    concept_paths.len()
                ));
            }

            // Only reached with `force`: every remaining concept gets its own deletion entry,
            // the same as `delete_memory` would record.
            for path in &concept_paths {
                let before = Self::load_document(&tx, &agent_id, &bundle, path)?
                    .as_ref()
                    .and_then(canonical_of);
                memory_revision::record(
                    &tx,
                    &agent_id,
                    &bundle,
                    path,
                    None,
                    origin,
                    before.as_deref(),
                )?;
            }
            tx.execute(
                "DELETE FROM memory_node WHERE agent_id = ?1 AND bundle = ?2",
                params![agent_id, bundle],
            )?;
            tx.execute(
                "DELETE FROM memory_edge WHERE agent_id = ?1 AND source_bundle = ?2",
                params![agent_id, bundle],
            )?;
            // Other bundles may have referenced this one via a whole-bundle [[bundle]] link or a
            // now-gone [[bundle/slug]] concept — those edges should flip to broken, not vanish.
            Self::recompute_broken(&tx, &agent_id)?;
            tx.commit()?;
            concept_paths
        };

        for path in &concept_paths {
            if let Err(e) = self.clear_passages(&agent_id, &bundle, path, indexer).await {
                tracing::warn!(
                    agent_id,
                    bundle,
                    path,
                    "failed to clear passages while deleting a bundle: {e}"
                );
            }
        }
        // Nothing should remain for the bundle; clear defensively in case a write raced this.
        self.conn.lock()?.execute(
            "DELETE FROM memory_passage WHERE agent_id = ?1 AND bundle = ?2",
            params![agent_id, bundle],
        )?;

        Ok(())
    }

    /// A zip of markdown files: one per document (frontmatter and body), plus the generated
    /// `index.md` and, when the bundle has history, `log.md`.
    pub async fn export_bundle(&self, agent_id: String, bundle: String) -> Result<Vec<u8>> {
        let files: Vec<(String, Vec<u8>)> = {
            let conn = self.conn.lock()?;
            let mut nodes = Self::list_nodes(&conn, &agent_id, Some(&bundle))?;
            nodes.sort_by(|a, b| a.path.cmp(&b.path));

            let mut files = Vec::new();
            for n in &nodes {
                let Some(memory) = Self::load_document(&conn, &agent_id, &bundle, &n.path)? else {
                    continue;
                };
                let content = memory.content.clone();
                let bytes = serialize_markdown(&MemoryFrontMatter::from(memory), &content)?;
                files.push((format!("{}.md", n.path), bytes));
            }
            // A document filed at `index` or `log` keeps its name: an archive holds each name
            // once.
            let taken = |name: &str| nodes.iter().any(|n| n.path == name);
            if !taken("index") {
                files.push((
                    "index.md".to_string(),
                    Self::render_index(&bundle, &nodes).into_bytes(),
                ));
            }
            let revisions = memory_revision::list_bundle(&conn, &agent_id, &bundle)?;
            if !revisions.is_empty() && !taken("log") {
                files.push((
                    "log.md".to_string(),
                    Self::render_log(&bundle, &revisions).into_bytes(),
                ));
            }
            files
        };

        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buf);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            for (name, bytes) in files {
                zip.start_file(name, options)?;
                zip.write_all(&bytes)?;
            }
            zip.finish()?;
        }

        Ok(buf.into_inner())
    }

    pub async fn import_bundle(
        &self,
        agent_id: String,
        bundle: String,
        zip_bytes: Vec<u8>,
        origin: &RevisionOrigin,
        indexer: &VizierIndexer,
        limits: &ChunkLimits,
    ) -> Result<ImportReport> {
        // Importing into the reserved bundle would let a foreign document land beside the
        // identity one, which is exactly what the bundle guard refuses one write at a time
        // (FR-014, data-model invariant 1).
        if is_core_bundle(&bundle) {
            return Err(anyhow!(
                "the core bundle cannot receive an imported bundle; it holds only the identity \
                 document at path '{SOUL_PATH}'"
            ));
        }
        let cursor = std::io::Cursor::new(zip_bytes);
        let mut archive =
            zip::ZipArchive::new(cursor).map_err(|e| anyhow!("malformed zip archive: {e}"))?;

        let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
        for i in 0..archive.len() {
            let mut file = archive
                .by_index(i)
                .map_err(|e| anyhow!("malformed zip archive: {e}"))?;
            if file.is_dir() {
                continue;
            }
            let name = file
                .name()
                .map_err(|e| anyhow!("malformed zip archive: {e}"))?
                .to_string();
            if name.contains("..") {
                return Err(anyhow!("malformed archive: unsafe path '{name}'"));
            }
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            entries.push((name, bytes));
        }

        if entries.is_empty() {
            return Err(anyhow!("archive contains no files"));
        }
        if !entries.iter().any(|(name, _)| name.ends_with(".md")) {
            return Err(anyhow!(
                "archive does not look like a memory bundle (no .md files)"
            ));
        }

        let mut report = ImportReport::default();
        for (name, bytes) in entries {
            let leaf = leaf_of(&name);
            if leaf == "index.md" || leaf == "log.md" || !name.ends_with(".md") {
                continue;
            }

            let path = normalize_path(&name);
            check_address(&agent_id, &bundle, &path)?;

            // One transaction per document: its row, revision and links arrive together.
            let written = {
                let conn = self.conn.lock()?;
                let tx = conn.transaction()?;
                Self::lock_agent(&tx, &agent_id)?;
                if Self::load_document(&tx, &agent_id, &bundle, &path)?.is_some() {
                    None
                } else {
                    match parse_markdown_bytes::<MemoryFrontMatter>(&bytes) {
                        Ok((mut fm, content)) => {
                            fm.bundle = bundle.clone();
                            fm.agent_id = agent_id.clone();
                            fm.slug = path.clone();
                            fm.relations = parse_relations(&content);
                            Self::store_document(&tx, &agent_id, &bundle, &path, &fm, &content)?;
                            // Import skips existing paths, so there is never a "before".
                            let canonical = memory_canonical(&fm.title, &fm.tags, &content)?;
                            memory_revision::record(
                                &tx,
                                &agent_id,
                                &bundle,
                                &path,
                                Some(&canonical),
                                origin,
                                None,
                            )?;
                            Self::rewrite_edges(&tx, &agent_id, &bundle, &path, &fm.relations)?;
                            tx.commit()?;
                            Some((fm, content))
                        }
                        Err(_) => None,
                    }
                }
            };

            match written {
                Some((fm, content)) => {
                    // Every imported document gets passages with no extra manual step (FR-035).
                    self.write_passages_lenient(
                        &agent_id, &bundle, &path, &fm.title, &fm.tags, &content, limits, indexer,
                    )
                    .await;
                    report.imported.push(path);
                }
                None => report.skipped.push(path),
            }
        }

        // A link may point at a document imported after it.
        {
            let conn = self.conn.lock()?;
            Self::recompute_broken(&conn, &agent_id)?;
        }

        Ok(report)
    }

    // ---- version history (specs/006-memory-version-history) ----

    fn revision_summary(row: &MemoryRevisionRow, latest_seq: i64) -> MemoryRevisionSummary {
        MemoryRevisionSummary {
            seq: row.seq,
            deleted: row.deleted,
            actor: row.actor.clone(),
            trigger: row.trigger.clone(),
            created_at: crate::time::from_millis_opt(row.created_at)
                .unwrap_or_else(crate::time::now),
            is_current: row.seq == latest_seq,
            size_bytes: row.content.as_ref().map(|c| c.len()).unwrap_or(0),
        }
    }

    pub async fn list_memory_revisions(
        &self,
        agent_id: String,
        bundle: Option<String>,
        path: String,
        offset: usize,
        limit: usize,
    ) -> Result<PaginatedMemoryRevisions> {
        let bundle = bundle.unwrap_or_else(default_bundle);
        let path = normalize_path(&path);
        // A document that pre-dates history gets its baseline on first listing (FR-018).
        let conn = self.conn.lock()?;
        let current = Self::load_document(&conn, &agent_id, &bundle, &path)?
            .as_ref()
            .and_then(canonical_of);
        memory_revision::ensure_baseline(&conn, &agent_id, &bundle, &path, current.as_deref())?;
        let latest_seq = memory_revision::latest(&conn, &agent_id, &bundle, &path)?
            .map(|r| r.seq)
            .unwrap_or(0);
        let (rows, total) = memory_revision::list(&conn, &agent_id, &bundle, &path, offset, limit)?;
        Ok(PaginatedMemoryRevisions {
            revisions: rows
                .iter()
                .map(|r| Self::revision_summary(r, latest_seq))
                .collect(),
            total,
            offset,
            limit: limit.clamp(1, memory_revision::MAX_LIMIT),
        })
    }

    pub async fn get_memory_revision(
        &self,
        agent_id: String,
        bundle: Option<String>,
        path: String,
        seq: i64,
    ) -> Result<Option<MemoryRevision>> {
        let bundle = bundle.unwrap_or_else(default_bundle);
        let path = normalize_path(&path);
        let conn = self.conn.lock()?;
        let Some(row) = memory_revision::get(&conn, &agent_id, &bundle, &path, seq)? else {
            return Ok(None);
        };
        let latest_seq = memory_revision::latest(&conn, &agent_id, &bundle, &path)?
            .map(|r| r.seq)
            .unwrap_or(0);
        let s = Self::revision_summary(&row, latest_seq);
        let (title, tags) = match row.content.as_deref().map(parse_memory_canonical) {
            Some(Ok((fm, _))) => (Some(fm.title), fm.tags),
            _ => (None, vec![]),
        };
        Ok(Some(MemoryRevision {
            seq: s.seq,
            deleted: s.deleted,
            actor: s.actor,
            trigger: s.trigger,
            created_at: s.created_at,
            is_current: s.is_current,
            size_bytes: s.size_bytes,
            content: row.content,
            title,
            tags,
        }))
    }

    pub async fn diff_memory_revisions(
        &self,
        agent_id: String,
        bundle: Option<String>,
        path: String,
        from: Option<i64>,
        to: i64,
    ) -> Result<RevisionDiff> {
        let bundle = bundle.unwrap_or_else(default_bundle);
        let path = normalize_path(&path);
        let conn = self.conn.lock()?;
        let to_row = memory_revision::get(&conn, &agent_id, &bundle, &path, to)?
            .ok_or_else(|| anyhow!("unknown version {to}"))?;
        let from_seq = from.unwrap_or(to - 1);
        // Diffing seq 1 "against its previous" means against nothing; a deletion entry on
        // either side likewise diffs as empty content.
        let from_content = if from_seq < 1 {
            None
        } else {
            memory_revision::get(&conn, &agent_id, &bundle, &path, from_seq)?
                .ok_or_else(|| anyhow!("unknown version {from_seq}"))?
                .content
        };
        let (hunks, additions, deletions) = diff_lines(
            from_content.as_deref().unwrap_or(""),
            to_row.content.as_deref().unwrap_or(""),
        );
        Ok(RevisionDiff {
            from_seq: from_seq.max(0),
            to_seq: to,
            additions,
            deletions,
            hunks,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn rollback_memory(
        &self,
        agent_id: String,
        bundle: Option<String>,
        path: String,
        seq: i64,
        origin: &RevisionOrigin,
        indexer: &VizierIndexer,
        limits: &ChunkLimits,
        policy: &WritePolicy,
    ) -> Result<RollbackResponse> {
        let bundle = bundle.unwrap_or_else(default_bundle);
        let path = normalize_path(&path);
        let (row, before) = {
            let conn = self.conn.lock()?;
            let row = memory_revision::get(&conn, &agent_id, &bundle, &path, seq)?
                .ok_or_else(|| anyhow!("unknown version {seq}"))?;
            let before = memory_revision::latest(&conn, &agent_id, &bundle, &path)?.map(|r| r.seq);
            (row, before)
        };
        let Some(content) = row.content.filter(|_| !row.deleted) else {
            return Err(anyhow!(
                "version {seq} is a deletion entry and cannot be restored; pick a content version"
            ));
        };
        let (fm, body) = parse_memory_canonical(&content)?;

        // A rollback is a normal save with rollback provenance: the document is rewritten
        // (recreated if it was deleted), its links and embedding refresh through the one write
        // path — and history only ever grows.
        let origin = origin
            .clone()
            .with_trigger(RevisionTrigger::Rollback { restored_from: seq });
        self.write_memory_inner(
            agent_id.clone(),
            Some(bundle.clone()),
            Some(path.clone()),
            false,
            fm.title,
            body,
            fm.tags,
            &origin,
            indexer,
            limits,
            policy,
        )
        .await?;

        let after = {
            let conn = self.conn.lock()?;
            memory_revision::latest(&conn, &agent_id, &bundle, &path)?.map(|r| r.seq)
        };
        let no_change = after == before;
        Ok(RollbackResponse {
            no_change,
            new_seq: if no_change { None } else { after },
            restored_from: seq,
        })
    }

    /// Used only by the one-time startup migration (`VizierDependencies::migrate_memory_to_bundles`)
    /// to carry a legacy memory's original timestamps and read count forward — `write_memory`'s
    /// public signature always treats a new document as `created_at = now`, `read_count = 0`,
    /// which is right for every other caller but wrong for migrating pre-existing data.
    #[allow(clippy::too_many_arguments)]
    pub async fn write_migrated_memory(
        &self,
        agent_id: String,
        bundle: String,
        path: String,
        title: String,
        content: String,
        tags: Vec<String>,
        created_at: Timestamp,
        updated_at: Timestamp,
        read_count: u64,
        indexer: &VizierIndexer,
    ) -> Result<()> {
        let path = normalize_path(&path);
        check_address(&agent_id, &bundle, &path)?;
        let relations = parse_relations(&content);

        let frontmatter = MemoryFrontMatter {
            slug: path.clone(),
            title,
            created_at,
            updated_at,
            agent_id: agent_id.clone(),
            bundle: bundle.clone(),
            tags,
            keywords: vec![],
            relations: relations.clone(),
            read_count,
        };
        let canonical = memory_canonical(&frontmatter.title, &frontmatter.tags, &content)?;

        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        Self::lock_agent(&tx, &agent_id)?;
        Self::store_document(&tx, &agent_id, &bundle, &path, &frontmatter, &content)?;
        memory_revision::record(
            &tx,
            &agent_id,
            &bundle,
            &path,
            Some(&canonical),
            &RevisionOrigin::system(RevisionTrigger::Baseline),
            None,
        )?;
        Self::rewrite_edges(&tx, &agent_id, &bundle, &path, &relations)?;
        tx.commit()?;
        // Deliberately **not** indexed here. This runs from `dependencies.rs`, where every other
        // migration lives but no embedder does — its callers pass `NoopIndexer`, so an index call
        // would silently no-op (research Decision 8). Leaving the document with zero
        // `memory_passage` rows is exactly the marker the per-agent conversion task at spawn looks
        // for, so the document is picked up there, by that agent's own real indexer.
        let _ = indexer;

        Ok(())
    }

    /// Recomputes an agent's broken-link flags after a migration wrote a batch of documents,
    /// which may link to documents written after them.
    pub async fn finalize_migration(&self, agent_id: &str) -> Result<()> {
        let conn = self.conn.lock()?;
        Self::recompute_broken(&conn, agent_id)
    }

    // ---- the file migration (specs/070-memory-in-database, #82) ----

    /// Every row of an agent as `(bundle, path, holds a body)`.
    pub(crate) fn memory_rows(&self, agent_id: &str) -> Result<Vec<(String, String, bool)>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare(
            "SELECT bundle, path, CASE WHEN content IS NULL THEN 'no' ELSE 'yes' END
             FROM memory_node WHERE agent_id = ?1",
        )?;
        let rows = stmt
            .query_map(params![agent_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)? == "yes",
                ))
            })?
            .collect::<sql::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Move one stored memory file into its row, in one transaction. A document already in the
    /// database is never overwritten, no revision is recorded (moving is not editing), and a
    /// file that cannot be parsed leaves a row without a body, so nothing mistakes the address
    /// for an empty one that may be seeded.
    pub(crate) fn migrate_file(
        &self,
        agent_id: &str,
        bundle: &str,
        path: &str,
        bytes: &[u8],
    ) -> Result<MigratedFile> {
        let conn = self.conn.lock()?;
        let tx = conn.transaction()?;
        Self::lock_agent(&tx, agent_id)?;
        let outcome = match Self::row_state(&tx, agent_id, bundle, path)? {
            Some(true) => MigratedFile::AlreadyThere,
            // No row, and history says the document was deleted: it was deleted after an earlier,
            // unfinished run moved it, and must not come back.
            None if memory_revision::latest(&tx, agent_id, bundle, path)?
                .is_some_and(|r| r.deleted) =>
            {
                MigratedFile::DeletedSince
            }
            state => match parse_markdown_bytes::<MemoryFrontMatter>(bytes) {
                Ok((mut fm, content)) => {
                    fm.slug = path.to_string();
                    fm.bundle = bundle.to_string();
                    fm.agent_id = agent_id.to_string();
                    fm.relations = parse_relations(&content);
                    Self::store_document(&tx, agent_id, bundle, path, &fm, &content)?;
                    Self::rewrite_edges(&tx, agent_id, bundle, path, &fm.relations)?;
                    MigratedFile::Moved
                }
                Err(e) => {
                    if state.is_none() {
                        let now = crate::time::stored_now();
                        tx.execute(
                            "INSERT INTO memory_node
                                (agent_id, bundle, path, slug, title, tags_json, created_at,
                                 updated_at, read_count, content)
                             VALUES (?1,?2,?3,?4,?4,'[]',?5,?5,0,NULL)",
                            params![agent_id, bundle, path, leaf_of(path), now],
                        )?;
                    }
                    MigratedFile::Unreadable(e.to_string())
                }
            },
        };
        tx.commit()?;
        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indexer::noop::NoopIndexer;
    use std::sync::Arc;

    /// Default chunk limits for the tests. A real agent carries its own on `AgentConfig`.
    fn limits() -> ChunkLimits {
        ChunkLimits::default()
    }

    fn origin() -> RevisionOrigin {
        RevisionOrigin::system(RevisionTrigger::Baseline)
    }

    /// The SQLite store the bundle tests run on, or PostgreSQL inside `on_postgres`.
    fn sql_store() -> crate::storage::sql::SqlStore {
        crate::tests_support::test_store(|| {
            let conn = rusqlite::Connection::open_in_memory().unwrap();
            crate::storage::sqlite::init_memory_graph_schema(&conn).unwrap();
            crate::storage::sqlite::init_revision_schema(&conn).unwrap();
            crate::storage::sql::SqlStore::sqlite(conn)
        })
    }

    fn setup() -> (BundleMemoryStore, VizierIndexer) {
        let store = BundleMemoryStore::new(sql_store());
        let indexer = VizierIndexer::build(NoopIndexer);
        (store, indexer)
    }

    #[tokio::test]
    async fn collision_rejected_on_create_only() {
        let (store, indexer) = setup();
        store
            .write_memory(
                "a1".into(),
                Some("default".into()),
                Some("note".into()),
                true,
                "Note".into(),
                "hello".into(),
                vec![],
                &origin(),
                &indexer,
                &limits(),
            )
            .await
            .unwrap();

        let err = store
            .write_memory(
                "a1".into(),
                Some("default".into()),
                Some("note".into()),
                true,
                "Note Again".into(),
                "hello again".into(),
                vec![],
                &origin(),
                &indexer,
                &limits(),
            )
            .await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn update_same_path_is_allowed_when_not_create_only() {
        let (store, indexer) = setup();
        store
            .write_memory(
                "a1".into(),
                None,
                Some("note".into()),
                false,
                "Note".into(),
                "v1".into(),
                vec![],
                &origin(),
                &indexer,
                &limits(),
            )
            .await
            .unwrap();

        let updated = store
            .write_memory(
                "a1".into(),
                None,
                Some("note".into()),
                false,
                "Note".into(),
                "v2".into(),
                vec![],
                &origin(),
                &indexer,
                &limits(),
            )
            .await
            .unwrap();
        assert_eq!(updated.memory.content, "v2");
    }

    #[tokio::test]
    async fn implicit_bundle_and_subdirectory_creation() {
        let (store, indexer) = setup();
        let memory = store
            .write_memory(
                "a1".into(),
                Some("andy".into()),
                Some("friends/bred".into()),
                false,
                "Bred".into(),
                "hi".into(),
                vec![],
                &origin(),
                &indexer,
                &limits(),
            )
            .await
            .unwrap();
        assert_eq!(memory.memory.bundle, "andy");
        assert_eq!(memory.memory.slug, "friends/bred");

        let detail = store
            .get_memory_detail("a1".into(), Some("andy".into()), "friends/bred".into())
            .await
            .unwrap();
        assert!(detail.is_some());
    }

    #[tokio::test]
    async fn same_bundle_and_cross_bundle_links_resolve() {
        let (store, indexer) = setup();
        store
            .write_memory(
                "a1".into(),
                Some("default".into()),
                Some("alpha".into()),
                false,
                "Alpha".into(),
                "alpha content".into(),
                vec![],
                &origin(),
                &indexer,
                &limits(),
            )
            .await
            .unwrap();

        store
            .write_memory(
                "a1".into(),
                Some("other".into()),
                Some("beta".into()),
                false,
                "Beta".into(),
                "beta content".into(),
                vec![],
                &origin(),
                &indexer,
                &limits(),
            )
            .await
            .unwrap();

        store
            .write_memory(
                "a1".into(),
                Some("default".into()),
                Some("gamma".into()),
                false,
                "Gamma".into(),
                "same bundle [Alpha](alpha.md), cross bundle [[other/beta]]".into(),
                vec![],
                &origin(),
                &indexer,
                &limits(),
            )
            .await
            .unwrap();

        let related = store
            .get_related_memories("a1".into(), Some("default".into()), "gamma".into())
            .await
            .unwrap();
        let slugs: Vec<String> = related
            .iter()
            .map(|m| format!("{}/{}", m.bundle, m.slug))
            .collect();
        assert!(slugs.contains(&"default/alpha".to_string()));
        assert!(slugs.contains(&"other/beta".to_string()));
    }

    #[tokio::test]
    async fn same_bundle_link_without_md_extension_still_resolves() {
        let (store, indexer) = setup();
        store
            .write_memory(
                "a1".into(),
                Some("default".into()),
                Some("alpha".into()),
                false,
                "Alpha".into(),
                "content".into(),
                vec![],
                &origin(),
                &indexer,
                &limits(),
            )
            .await
            .unwrap();

        // Agent forgot the `.md` extension — should be treated identically to `alpha.md`.
        store
            .write_memory(
                "a1".into(),
                Some("default".into()),
                Some("gamma".into()),
                false,
                "Gamma".into(),
                "see [Alpha](alpha) for details".into(),
                vec![],
                &origin(),
                &indexer,
                &limits(),
            )
            .await
            .unwrap();

        let related = store
            .get_related_memories("a1".into(), Some("default".into()), "gamma".into())
            .await
            .unwrap();
        assert!(related.iter().any(|m| m.slug == "alpha"));
    }

    #[tokio::test]
    async fn extensionless_link_is_ignored_when_it_has_a_different_extension() {
        let (store, indexer) = setup();
        store
            .write_memory(
                "a1".into(),
                Some("default".into()),
                Some("gamma".into()),
                false,
                "Gamma".into(),
                "see [screenshot](notes.png) and [site](https://example.com/page) and [mail](mailto:a@b.com)".into(),
                vec![],
                &origin(),
                &indexer,
                &limits(),
            )
            .await
            .unwrap();

        let memory = store
            .get_memory_detail("a1".into(), Some("default".into()), "gamma".into())
            .await
            .unwrap()
            .unwrap();
        assert!(memory.relations.is_empty());
    }

    /// Real-world failure mode: an agent writes a cross-bundle reference using ordinary
    /// same-bundle relative-link syntax, treating the whole memory tree as one shared
    /// filesystem — `[label](books/great-gatsby.md)` from *inside* the `authors` bundle,
    /// meaning "the `great-gatsby` concept in the `books` bundle." Since that literal nested
    /// path doesn't exist within `authors`, but `books` is a real bundle containing a concept
    /// at that path, it should resolve there instead of staying permanently broken.
    #[tokio::test]
    async fn cross_bundle_reference_written_as_a_same_bundle_style_path_still_resolves() {
        let (store, indexer) = setup();
        store
            .write_memory(
                "viz".into(),
                Some("books".into()),
                Some("great-gatsby".into()),
                false,
                "The Great Gatsby".into(),
                "content".into(),
                vec![],
                &origin(),
                &indexer,
                &limits(),
            )
            .await
            .unwrap();

        store
            .write_memory(
                "viz".into(),
                Some("authors".into()),
                Some("f-scott-fitzgerald".into()),
                false,
                "F. Scott Fitzgerald".into(),
                "wrote [The Great Gatsby](books/great-gatsby.md)".into(),
                vec![],
                &origin(),
                &indexer,
                &limits(),
            )
            .await
            .unwrap();

        let related = store
            .get_related_memories(
                "viz".into(),
                Some("authors".into()),
                "f-scott-fitzgerald".into(),
            )
            .await
            .unwrap();
        assert!(
            related
                .iter()
                .any(|m| m.bundle == "books" && m.slug == "great-gatsby")
        );

        // A genuine same-bundle nested path must keep working exactly as before — the literal
        // interpretation always wins when it actually resolves.
        store
            .write_memory(
                "viz".into(),
                Some("authors".into()),
                Some("nested/real-nested-concept".into()),
                false,
                "Real Nested Concept".into(),
                "content".into(),
                vec![],
                &origin(),
                &indexer,
                &limits(),
            )
            .await
            .unwrap();
        store
            .write_memory(
                "viz".into(),
                Some("authors".into()),
                Some("pointer".into()),
                false,
                "Pointer".into(),
                "see [nested](nested/real-nested-concept.md)".into(),
                vec![],
                &origin(),
                &indexer,
                &limits(),
            )
            .await
            .unwrap();
        let related2 = store
            .get_related_memories("viz".into(), Some("authors".into()), "pointer".into())
            .await
            .unwrap();
        assert!(
            related2
                .iter()
                .any(|m| m.bundle == "authors" && m.slug == "nested/real-nested-concept")
        );
    }

    /// A bare legacy `[[slug]]` wikilink that doesn't name an existing bundle should resolve as
    /// a same-bundle concept reference instead of staying permanently broken, per research.md §6.
    #[tokio::test]
    async fn bare_wikilink_falls_back_to_same_bundle_concept_when_no_such_bundle_exists() {
        let (store, indexer) = setup();
        store
            .write_memory(
                "a1".into(),
                Some("default".into()),
                Some("alpha".into()),
                false,
                "Alpha".into(),
                "content".into(),
                vec![],
                &origin(),
                &indexer,
                &limits(),
            )
            .await
            .unwrap();
        store
            .write_memory(
                "a1".into(),
                Some("default".into()),
                Some("gamma".into()),
                false,
                "Gamma".into(),
                "see [[alpha]]".into(),
                vec![],
                &origin(),
                &indexer,
                &limits(),
            )
            .await
            .unwrap();

        let related = store
            .get_related_memories("a1".into(), Some("default".into()), "gamma".into())
            .await
            .unwrap();
        assert!(
            related
                .iter()
                .any(|m| m.bundle == "default" && m.slug == "alpha")
        );
    }

    #[tokio::test]
    async fn broken_links_are_tolerated_not_errors() {
        let (store, indexer) = setup();
        store
            .write_memory(
                "a1".into(),
                None,
                Some("gamma".into()),
                false,
                "Gamma".into(),
                "points to [missing](missing.md) and [[nowhere/nothing]]".into(),
                vec![],
                &origin(),
                &indexer,
                &limits(),
            )
            .await
            .unwrap();

        let related = store
            .get_related_memories("a1".into(), None, "gamma".into())
            .await
            .unwrap();
        assert!(related.is_empty());

        let graph = store
            .get_memory_graph("a1".into(), Some("default".into()), None)
            .await
            .unwrap();
        assert!(graph.edges.iter().any(|e| e.broken));
    }

    /// Entity wiki links (spec 066, `[[kind:name]]`) are WebUI navigation, not memory
    /// relations: they must be skipped by `parse_wikilinks` entirely — never a node, an
    /// edge, or a broken edge in the memory graph — while ordinary wikilinks in the same
    /// document keep working exactly as before.
    #[tokio::test]
    async fn entity_wikilinks_are_not_memory_relations() {
        let (store, indexer) = setup();
        store
            .write_memory(
                "a1".into(),
                Some("default".into()),
                Some("alpha".into()),
                false,
                "Alpha".into(),
                "alpha content".into(),
                vec![],
                &origin(),
                &indexer,
                &limits(),
            )
            .await
            .unwrap();

        store
            .write_memory(
                "a1".into(),
                Some("default".into()),
                Some("gamma".into()),
                false,
                "Gamma".into(),
                "cites [[artifact:report]], [[function:fetch_weather]], \
                 [[skill:summarize|doc]], [[task:weekly-report]] — and [[alpha]]"
                    .into(),
                vec![],
                &origin(),
                &indexer,
                &limits(),
            )
            .await
            .unwrap();

        let related = store
            .get_related_memories("a1".into(), Some("default".into()), "gamma".into())
            .await
            .unwrap();
        assert_eq!(related.len(), 1);
        assert_eq!(
            format!("{}/{}", related[0].bundle, related[0].slug),
            "default/alpha"
        );

        let graph = store
            .get_memory_graph("a1".into(), None, None)
            .await
            .unwrap();
        assert!(graph.edges.iter().all(|e| {
            !e.target.contains("artifact:report")
                && !e.target.contains("function:fetch_weather")
                && !e.target.contains("skill:summarize")
                && !e.target.contains("task:weekly-report")
        }));
    }

    #[tokio::test]
    async fn delete_bundle_rejected_when_not_empty() {
        let (store, indexer) = setup();
        store
            .write_memory(
                "a1".into(),
                Some("andy".into()),
                Some("note".into()),
                false,
                "Note".into(),
                "hello".into(),
                vec![],
                &origin(),
                &indexer,
                &limits(),
            )
            .await
            .unwrap();

        let err = store
            .delete_bundle("a1".into(), "andy".into(), false, &origin(), &indexer)
            .await;
        assert!(err.is_err());

        // Bundle must still be fully intact after a rejected delete.
        let detail = store
            .get_memory_detail("a1".into(), Some("andy".into()), "note".into())
            .await
            .unwrap();
        assert!(detail.is_some());
    }

    #[tokio::test]
    async fn delete_bundle_succeeds_when_empty() {
        let (store, indexer) = setup();
        store
            .write_memory(
                "a1".into(),
                Some("andy".into()),
                Some("note".into()),
                false,
                "Note".into(),
                "hello".into(),
                vec![],
                &origin(),
                &indexer,
                &limits(),
            )
            .await
            .unwrap();
        store
            .delete_memory(
                "a1".into(),
                Some("andy".into()),
                "note".into(),
                &origin(),
                &indexer,
            )
            .await
            .unwrap();

        store
            .delete_bundle("a1".into(), "andy".into(), false, &origin(), &indexer)
            .await
            .unwrap();


        let bundles = store.list_bundles("a1".into()).await.unwrap();
        assert!(!bundles.iter().any(|b| b.name == "andy"));
    }

    #[tokio::test]
    async fn delete_bundle_with_force_removes_remaining_concepts_too() {
        let (store, indexer) = setup();
        store
            .write_memory(
                "a1".into(),
                Some("andy".into()),
                Some("note".into()),
                false,
                "Note".into(),
                "hello".into(),
                vec![],
                &origin(),
                &indexer,
                &limits(),
            )
            .await
            .unwrap();

        // Not forced: still rejected.
        assert!(
            store
                .delete_bundle("a1".into(), "andy".into(), false, &origin(), &indexer)
                .await
                .is_err()
        );

        // Forced: the whole bundle, concept included, is gone.
        store
            .delete_bundle("a1".into(), "andy".into(), true, &origin(), &indexer)
            .await
            .unwrap();

        let detail = store
            .get_memory_detail("a1".into(), Some("andy".into()), "note".into())
            .await
            .unwrap();
        assert!(detail.is_none());

        let bundles = store.list_bundles("a1".into()).await.unwrap();
        assert!(!bundles.iter().any(|b| b.name == "andy"));
    }

    // ---- version history (specs/006-memory-version-history) ----

    async fn write_note(
        store: &BundleMemoryStore,
        indexer: &VizierIndexer,
        title: &str,
        body: &str,
        tags: Vec<String>,
        origin: &RevisionOrigin,
    ) -> Memory {
        store
            .write_memory(
                "a1".into(),
                Some("default".into()),
                Some("note".into()),
                false,
                title.into(),
                body.into(),
                tags,
                origin,
                indexer,
                &limits(),
            )
            .await
            .unwrap()
            .memory
    }

    fn history(store: &BundleMemoryStore) -> Vec<memory_revision::MemoryRevisionRow> {
        let conn = store.conn.lock().unwrap();
        memory_revision::list(&conn, "a1", "default", "note", 0, 50)
            .unwrap()
            .0
    }

    #[tokio::test]
    async fn every_save_records_a_revision_with_its_origin_and_no_op_saves_are_skipped() {
        let (store, indexer) = setup();
        let agent = RevisionOrigin {
            actor: crate::schema::RevisionActor::Agent,
            trigger: RevisionTrigger::Conversation,
            dream_cycle: None,
        };
        let user = RevisionOrigin {
            actor: crate::schema::RevisionActor::User {
                user_id: "u1".into(),
                username: "alice".into(),
            },
            trigger: RevisionTrigger::WebUi,
            dream_cycle: None,
        };

        write_note(&store, &indexer, "Note", "v1", vec![], &agent).await;
        write_note(&store, &indexer, "Note", "v2", vec![], &user).await;
        // identical content, title and tags: no new revision (FR-003)
        write_note(&store, &indexer, "Note", "v2", vec![], &user).await;
        // a tag-only change is a real change
        write_note(&store, &indexer, "Note", "v2", vec!["t".into()], &agent).await;

        let rows = history(&store);
        assert_eq!(
            rows.iter().map(|r| r.seq).collect::<Vec<_>>(),
            vec![3, 2, 1]
        );
        assert_eq!(rows[2].actor, crate::schema::RevisionActor::Agent);
        assert_eq!(rows[2].trigger, RevisionTrigger::Conversation);
        assert_eq!(rows[1].actor, user.actor);
        assert_eq!(rows[1].trigger, RevisionTrigger::WebUi);
        assert!(rows[0].content.as_deref().unwrap().contains("tags:\n- t\n"));
        // snapshots are canonical: no bookkeeping fields leak into history
        assert!(!rows[0].content.as_deref().unwrap().contains("updated_at"));
    }

    #[tokio::test]
    async fn delete_records_a_deletion_entry_and_a_pre_history_document_gets_a_baseline() {
        let (store, indexer) = setup();
        let agent = RevisionOrigin {
            actor: crate::schema::RevisionActor::Agent,
            trigger: RevisionTrigger::Conversation,
            dream_cycle: None,
        };

        // A document that pre-dates history: a row with no revision.
        let fm = MemoryFrontMatter {
            slug: "note".into(),
            title: "Old".into(),
            created_at: crate::time::now(),
            updated_at: crate::time::now(),
            agent_id: "a1".into(),
            bundle: "default".into(),
            tags: vec![],
            keywords: vec![],
            relations: vec![],
            read_count: 0,
        };
        {
            let conn = store.conn.lock().unwrap();
            BundleMemoryStore::store_document(
                &conn,
                "a1",
                "default",
                "note",
                &fm,
                "pre-history body",
            )
            .unwrap();
        }
        assert!(history(&store).is_empty());

        // First tracked edit seeds seq 1 (baseline) from the stored content, then seq 2.
        write_note(&store, &indexer, "Old", "edited", vec![], &agent).await;
        let rows = history(&store);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].trigger, RevisionTrigger::Baseline);
        assert_eq!(rows[1].actor, crate::schema::RevisionActor::System);
        assert!(
            rows[1]
                .content
                .as_deref()
                .unwrap()
                .contains("pre-history body")
        );

        store
            .delete_memory("a1".into(), None, "note".into(), &agent, &indexer)
            .await
            .unwrap();
        let rows = history(&store);
        assert_eq!(rows.len(), 3);
        assert!(rows[0].deleted);
        assert!(rows[0].content.is_none());

        // Deleting the agent wipes its history.
        {
            let conn = store.conn.lock().unwrap();
            memory_revision::delete_agent(&conn, "a1").unwrap();
        }
        assert!(history(&store).is_empty());
    }
    // ---- passage lifecycle (specs/009-memory-semantic-chunking, task T024) ----

    /// A `DocumentIndexer` that remembers which keys it holds, so a test can assert on the index
    /// itself rather than on the passage table alone. `NoopIndexer` cannot answer the question
    /// these tests exist to ask — whether a *vector* was orphaned.
    #[derive(Default)]
    struct RecordingIndexer {
        keys: std::sync::Mutex<std::collections::BTreeSet<String>>,
    }

    impl RecordingIndexer {
        fn snapshot(&self) -> Vec<String> {
            self.keys.lock().unwrap().iter().cloned().collect()
        }
    }

    #[async_trait::async_trait]
    impl crate::indexer::DocumentIndexer for Arc<RecordingIndexer> {
        async fn add_document_index(
            &self,
            context: String,
            path: String,
            _content: String,
        ) -> anyhow::Result<crate::schema::DocumentIndex> {
            self.keys.lock().unwrap().insert(path.clone());
            Ok(crate::schema::DocumentIndex {
                path,
                embedding: vec![],
                context,
                score: 0.0,
            })
        }

        async fn add_document_indexes(
            &self,
            context: String,
            entries: Vec<(String, String)>,
        ) -> anyhow::Result<Vec<crate::schema::DocumentIndex>> {
            let mut out = Vec::new();
            for (path, _) in entries {
                self.keys.lock().unwrap().insert(path.clone());
                out.push(crate::schema::DocumentIndex {
                    path,
                    embedding: vec![],
                    context: context.clone(),
                    score: 0.0,
                });
            }
            Ok(out)
        }

        async fn search_document_index(
            &self,
            _context: String,
            _query: String,
            _limit: usize,
            _threshold: f64,
        ) -> anyhow::Result<Vec<crate::schema::DocumentIndex>> {
            Ok(vec![])
        }

        async fn delete_index(&self, _context: String, path: String) -> anyhow::Result<()> {
            self.keys.lock().unwrap().remove(&path);
            Ok(())
        }
    }

    fn setup_recording() -> (BundleMemoryStore, VizierIndexer, Arc<RecordingIndexer>) {
        let store = BundleMemoryStore::new(sql_store());
        let recorder = Arc::new(RecordingIndexer::default());
        let indexer = VizierIndexer::build(recorder.clone());
        (store, indexer, recorder)
    }

    fn passage_rows(store: &BundleMemoryStore, path: &str) -> Vec<(usize, usize, usize, bool)> {
        let conn = store.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT ordinal, char_start, char_end, continues FROM memory_passage
                 WHERE agent_id = 'a1' AND bundle = 'default' AND path = ?1 ORDER BY ordinal",
            )
            .unwrap();
        stmt.query_map(params![path], |row| {
            Ok((
                row.get::<_, i64>(0)? as usize,
                row.get::<_, i64>(1)? as usize,
                row.get::<_, i64>(2)? as usize,
                row.get::<_, i64>(3)? != 0,
            ))
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
    }

    async fn write_at(
        store: &BundleMemoryStore,
        indexer: &VizierIndexer,
        path: &str,
        body: &str,
    ) -> Memory {
        store
            .write_memory(
                "a1".into(),
                Some("default".into()),
                Some(path.into()),
                false,
                format!("Doc {path}"),
                body.into(),
                vec!["ops".into()],
                &origin(),
                indexer,
                &limits(),
            )
            .await
            .unwrap()
            .memory
    }

    /// A long body: `sections` headed sections, each comfortably over `min_size`, so the chunker
    /// produces roughly one passage per section.
    fn long_body(sections: usize) -> String {
        (1..=sections)
            .map(|i| format!("## Section {i}\n\n{}", "word ".repeat(120)))
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    #[tokio::test]
    async fn a_write_produces_passage_rows_and_one_index_entry_per_passage() {
        let (store, indexer, recorder) = setup_recording();
        write_at(&store, &indexer, "note", &long_body(4)).await;

        let rows = passage_rows(&store, "note");
        assert!(rows.len() >= 2, "a four-section document chunks: {rows:?}");
        assert_eq!(rows[0].0, 0, "ordinals start at zero");
        assert_eq!(rows[0].1, 0, "the first passage starts at byte zero");

        let keys = recorder.snapshot();
        assert_eq!(
            keys.len(),
            rows.len(),
            "one index entry per passage, no more and no fewer: {keys:?}"
        );
        for (ordinal, _, _, _) in &rows {
            assert!(
                keys.contains(&format!("a1/default/note#{ordinal}")),
                "passage {ordinal} is indexed under its own key: {keys:?}"
            );
        }
        assert!(
            !keys.contains(&"a1/default/note".to_string()),
            "no whole-document relevance entry survives alongside passages (FR-039)"
        );
    }

    #[tokio::test]
    async fn a_second_write_replaces_passages_rather_than_appending() {
        let (store, indexer, recorder) = setup_recording();
        write_at(&store, &indexer, "note", &long_body(4)).await;
        let first = passage_rows(&store, "note");

        write_at(&store, &indexer, "note", &long_body(4)).await;
        let second = passage_rows(&store, "note");

        assert_eq!(first, second, "rewriting the same content is idempotent");
        assert_eq!(
            recorder.snapshot().len(),
            second.len(),
            "the index did not accumulate a second copy"
        );
    }

    /// The regression that matters most. A document shrinking from many passages to few must not
    /// leave the surplus vectors behind: a stale passage returning text the document no longer has
    /// is the worst failure this feature can produce, and it is invisible to the passage table —
    /// only the index shows it.
    #[tokio::test]
    async fn a_document_shrinking_from_twelve_passages_to_four_leaves_no_orphaned_index_rows() {
        let (store, indexer, recorder) = setup_recording();

        write_at(&store, &indexer, "note", &long_body(12)).await;
        let big = passage_rows(&store, "note");
        let big_keys = recorder.snapshot();
        assert!(
            big.len() >= 8,
            "the twelve-section document needs many passages to make this test mean anything: {}",
            big.len()
        );
        assert_eq!(big_keys.len(), big.len());

        write_at(&store, &indexer, "note", &long_body(2)).await;
        let small = passage_rows(&store, "note");
        let small_keys = recorder.snapshot();

        assert!(
            small.len() < big.len(),
            "the document really did shrink: {} -> {}",
            big.len(),
            small.len()
        );
        assert_eq!(
            small_keys.len(),
            small.len(),
            "every surplus vector is gone, not just the passage rows: {small_keys:?}"
        );
        for ordinal in small.len()..big.len() {
            assert!(
                !small_keys.contains(&format!("a1/default/note#{ordinal}")),
                "ordinal {ordinal} was orphaned in the index"
            );
        }
    }

    #[tokio::test]
    async fn deleting_a_memory_clears_both_the_passage_rows_and_the_index() {
        let (store, indexer, recorder) = setup_recording();
        write_at(&store, &indexer, "note", &long_body(5)).await;
        assert!(!passage_rows(&store, "note").is_empty());
        assert!(!recorder.snapshot().is_empty());

        store
            .delete_memory(
                "a1".into(),
                Some("default".into()),
                "note".into(),
                &origin(),
                &indexer,
            )
            .await
            .unwrap();

        assert!(
            passage_rows(&store, "note").is_empty(),
            "passage rows cleared"
        );
        assert!(
            recorder.snapshot().is_empty(),
            "index entries cleared: {:?}",
            recorder.snapshot()
        );
    }

    #[tokio::test]
    async fn deleting_a_bundle_clears_passages_for_every_concept_in_it() {
        let (store, indexer, recorder) = setup_recording();
        write_at(&store, &indexer, "one", &long_body(3)).await;
        write_at(&store, &indexer, "two", &long_body(3)).await;
        assert!(!recorder.snapshot().is_empty());

        store
            .delete_bundle("a1".into(), "default".into(), true, &origin(), &indexer)
            .await
            .unwrap();

        assert!(passage_rows(&store, "one").is_empty());
        assert!(passage_rows(&store, "two").is_empty());
        assert!(
            recorder.snapshot().is_empty(),
            "no bundle concept left a vector behind: {:?}",
            recorder.snapshot()
        );
    }

    #[tokio::test]
    async fn importing_a_bundle_produces_passages_for_every_document() {
        let (store, indexer, recorder) = setup_recording();
        write_at(&store, &indexer, "one", &long_body(3)).await;
        write_at(&store, &indexer, "two", &long_body(3)).await;
        let zip = store
            .export_bundle("a1".into(), "default".into())
            .await
            .unwrap();

        let (dest, dest_indexer, dest_recorder) = setup_recording();
        let report = dest
            .import_bundle(
                "a1".into(),
                "imported".into(),
                zip,
                &origin(),
                &dest_indexer,
                &limits(),
            )
            .await
            .unwrap();
        assert_eq!(
            report.imported.len(),
            2,
            "both concepts imported: {report:?}"
        );

        let keys = dest_recorder.snapshot();
        assert!(
            !keys.is_empty(),
            "import indexed passages with no extra step (FR-035)"
        );
        for path in &report.imported {
            let conn = dest.conn.lock().unwrap();
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM memory_passage WHERE agent_id = 'a1' AND bundle = 'imported' AND path = ?1",
                    params![path],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(count > 0, "imported '{path}' has passages");
        }
        assert!(keys.iter().all(|k| k.starts_with("a1/imported/")));
    }

    /// FR-036: the document must survive a chunking or indexing failure, readable and
    /// re-convertible. The save must not even report an error.
    #[tokio::test]
    async fn an_indexing_failure_does_not_fail_the_save() {
        struct FailingIndexer;

        #[async_trait::async_trait]
        impl crate::indexer::DocumentIndexer for FailingIndexer {
            async fn add_document_index(
                &self,
                _context: String,
                _path: String,
                _content: String,
            ) -> anyhow::Result<crate::schema::DocumentIndex> {
                Err(anyhow!("embedder unavailable"))
            }
            async fn add_document_indexes(
                &self,
                _context: String,
                _entries: Vec<(String, String)>,
            ) -> anyhow::Result<Vec<crate::schema::DocumentIndex>> {
                Err(anyhow!("embedder unavailable"))
            }
            async fn search_document_index(
                &self,
                _context: String,
                _query: String,
                _limit: usize,
                _threshold: f64,
            ) -> anyhow::Result<Vec<crate::schema::DocumentIndex>> {
                Ok(vec![])
            }
            async fn delete_index(&self, _context: String, _path: String) -> anyhow::Result<()> {
                Ok(())
            }
        }

        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::storage::sqlite::init_memory_graph_schema(&conn).unwrap();
        crate::storage::sqlite::init_revision_schema(&conn).unwrap();
        let store = BundleMemoryStore::new(crate::storage::sql::SqlStore::sqlite(conn));
        let indexer = VizierIndexer::build(FailingIndexer);

        let saved = write_at(&store, &indexer, "note", &long_body(4)).await;
        assert_eq!(saved.slug, "note", "the save succeeded despite the indexer");

        let read = store
            .get_memory_detail("a1".into(), Some("default".into()), "note".into())
            .await
            .unwrap()
            .expect("the document is still readable");
        assert!(read.content.contains("Section 1"));
    }

    /// Conversion is resumable because absence of rows is the only progress marker: a document that
    /// already has passages is skipped, and one that has none is converted (FR-040).
    #[tokio::test]
    async fn reconcile_converts_only_what_needs_it_and_rebuilds_on_drift() {
        let (store, indexer, _recorder) = setup_recording();
        write_at(&store, &indexer, "note", &long_body(4)).await;

        // Nothing to do on a second pass — this is what makes an interrupted run resumable
        // rather than a restart.
        let (converted, failed) = store
            .reconcile_all_passages("a1", &limits(), &indexer)
            .await
            .unwrap();
        assert_eq!((converted, failed), (0, 0), "already converted, so skipped");

        // An unconverted document: rows removed behind the store's back, as a corpus written by
        // the previous release looks.
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("DELETE FROM memory_passage", []).unwrap();
        }
        let (converted, failed) = store
            .reconcile_all_passages("a1", &limits(), &indexer)
            .await
            .unwrap();
        assert_eq!(
            (converted, failed),
            (1, 0),
            "unconverted document picked up"
        );
        assert!(!passage_rows(&store, "note").is_empty());

        // Drift: the stored hash no longer matches the body, which is what a save while indexing
        // was failing looks like. Detected by hash comparison, not by re-deriving spans (research Decision 12).
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("UPDATE memory_passage SET content_hash = 'stale'", [])
                .unwrap();
        }
        let (converted, failed) = store
            .reconcile_all_passages("a1", &limits(), &indexer)
            .await
            .unwrap();
        assert_eq!((converted, failed), (1, 0), "drifted document rebuilt");
        let rows = passage_rows(&store, "note");
        assert!(!rows.is_empty());
        let conn = store.conn.lock().unwrap();
        let hash: String = conn
            .query_row("SELECT content_hash FROM memory_passage LIMIT 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_ne!(hash, "stale", "the rebuilt rows carry the real hash");
    }

    /// A different embedding width moves the agent to another index table, where none of its
    /// passages exist; forgetting the coordinates makes the next reconcile rebuild them.
    #[tokio::test]
    async fn resetting_passages_makes_the_next_reconcile_reindex_the_corpus() {
        let (store, indexer, _recorder) = setup_recording();
        write_at(&store, &indexer, "note", &long_body(4)).await;
        assert!(!passage_rows(&store, "note").is_empty());

        store.reset_passages("a1").await.unwrap();
        assert!(
            passage_rows(&store, "note").is_empty(),
            "coordinates dropped"
        );

        let (converted, failed) = store
            .reconcile_all_passages("a1", &limits(), &indexer)
            .await
            .unwrap();
        assert_eq!((converted, failed), (1, 0), "the corpus is indexed again");
        assert!(!passage_rows(&store, "note").is_empty());

        // Another agent's passages are untouched.
        write_at(&store, &indexer, "other", &long_body(2)).await;
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "UPDATE memory_passage SET agent_id = 'a2' WHERE path = 'other'",
                [],
            )
            .unwrap();
        }
        store.reset_passages("a1").await.unwrap();
        let kept: i64 = store
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM memory_passage WHERE agent_id = 'a2'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(kept > 0, "one agent's reset leaves another's rows alone");
    }

    #[test]
    fn an_indexer_key_round_trips_with_and_without_an_ordinal() {
        assert_eq!(
            BundleMemoryStore::parse_indexer_key("a1/work/ops/deploys#3"),
            Some(("a1".into(), "work".into(), "ops/deploys".into(), Some(3))),
            "a nested path keeps its slashes and gives up its ordinal"
        );
        assert_eq!(
            BundleMemoryStore::parse_indexer_key("a1/work/ops/deploys"),
            Some(("a1".into(), "work".into(), "ops/deploys".into(), None)),
            "a legacy document key parses with no ordinal"
        );
        assert_eq!(
            BundleMemoryStore::parse_indexer_key("a1/work/odd#name"),
            Some(("a1".into(), "work".into(), "odd#name".into(), None)),
            "a `#` that is not an ordinal stays part of the path"
        );
        assert_eq!(
            BundleMemoryStore::passage_indexer_key("a1", "work", "ops/deploys", 3),
            "a1/work/ops/deploys#3"
        );
    }

    // ---- the reserved bundle and the size limits (specs/043-unify-core-and-memory) ----

    /// Default limits, so a test only names the pair it actually cares about.
    fn policy() -> WritePolicy {
        WritePolicy::new(MemoryLimits::default())
    }

    /// A body of exactly `bytes` bytes, so a limit test can sit on a boundary rather than
    /// near one.
    fn body_of(bytes: usize) -> String {
        "x".repeat(bytes)
    }

    /// Writes `(bundle, path)` under an explicit policy.
    async fn write_with(
        store: &BundleMemoryStore,
        indexer: &VizierIndexer,
        bundle: Option<&str>,
        path: Option<&str>,
        content: &str,
        policy: &WritePolicy,
    ) -> Result<WrittenMemory> {
        store
            .write_memory_with(
                "a1".into(),
                bundle.map(str::to_string),
                path.map(str::to_string),
                false,
                "Note".into(),
                content.into(),
                vec![],
                &origin(),
                indexer,
                &limits(),
                policy,
            )
            .await
    }

    #[tokio::test]
    async fn the_identity_document_is_an_ordinary_round_tripping_memory() {
        // T008: (core, soul) writes, reads back through get_memory_detail, is listed among
        // the bundles, and is found by a search like any other document (FR-005).
        let (store, indexer) = setup();
        write_with(
            &store,
            &indexer,
            Some(CORE_BUNDLE),
            Some(SOUL_PATH),
            "I am Oz.",
            &policy(),
        )
        .await
        .unwrap();

        let detail = store
            .get_memory_detail("a1".into(), Some(CORE_BUNDLE.into()), SOUL_PATH.into())
            .await
            .unwrap()
            .expect("the identity document is readable like any other");
        assert_eq!(detail.content, "I am Oz.");
        assert_eq!(detail.bundle, CORE_BUNDLE);

        let bundles = store.list_bundles("a1".into()).await.unwrap();
        let core = bundles
            .iter()
            .find(|b| b.name == CORE_BUNDLE)
            .expect("the reserved bundle appears in the listing");
        assert!(core.protected, "a client must be able to disable delete");
        assert_eq!(core.concept_count, 1);
    }

    #[tokio::test]
    async fn the_reserved_bundle_refuses_a_foreign_document() {
        // T017 / FR-013: the refusal must name the path the bundle accepts.
        let (store, indexer) = setup();
        let err = write_with(
            &store,
            &indexer,
            Some(CORE_BUNDLE),
            Some("values"),
            "nope",
            &policy(),
        )
        .await
        .unwrap_err()
        .to_string();
        assert!(err.starts_with("the core bundle holds only"), "{err}");
        assert!(err.contains(SOUL_PATH), "the message names soul: {err}");
    }

    #[tokio::test]
    async fn an_import_into_the_reserved_bundle_is_refused_but_an_export_is_allowed() {
        // FR-014 and the spec's edge case: an import that would place anything but a soul in
        // `core` is refused, an export of `core` is permitted. Importing would otherwise put a
        // foreign document beside the identity one, which is exactly what the write guard
        // refuses one document at a time.
        let (store, indexer) = setup();
        write_at(&store, &indexer, "note", "some ordinary content").await;
        write_with(
            &store,
            &indexer,
            Some(CORE_BUNDLE),
            Some(SOUL_PATH),
            "I am Oz.",
            &policy(),
        )
        .await
        .unwrap();

        let ordinary = store
            .export_bundle("a1".into(), "default".into())
            .await
            .unwrap();
        let err = store
            .import_bundle(
                "a1".into(),
                CORE_BUNDLE.into(),
                ordinary,
                &origin(),
                &indexer,
                &limits(),
            )
            .await
            .unwrap_err()
            .to_string();
        assert!(err.starts_with("the core bundle cannot receive"), "{err}");
        assert!(
            err.contains(SOUL_PATH),
            "it names the path it accepts: {err}"
        );

        // The refusal changed nothing: the soul is still the bundle's only document.
        let bundles = store.list_bundles("a1".into()).await.unwrap();
        let core = bundles.iter().find(|b| b.name == CORE_BUNDLE).unwrap();
        assert_eq!(core.concept_count, 1);

        // Exporting the reserved bundle is fine — a person may take a copy.
        let exported = store
            .export_bundle("a1".into(), CORE_BUNDLE.into())
            .await
            .expect("the reserved bundle can be exported");
        assert!(!exported.is_empty());
    }

    #[tokio::test]
    async fn a_pre_reservation_document_in_a_legacy_core_bundle_stays_maintainable() {
        // `core` was an ordinary bundle name before it was reserved, so an install may already
        // hold unrelated documents there. FR-013 refuses *creating* another one; leaving an
        // existing one un-editable would be a data-access trap, so it stays writable.
        let (store, indexer) = setup();
        let now = crate::time::now();
        store
            .write_migrated_memory(
                "a1".into(),
                CORE_BUNDLE.into(),
                "legacy".into(),
                "Legacy".into(),
                "written before core was reserved".into(),
                vec![],
                now,
                now,
                0,
                &indexer,
            )
            .await
            .unwrap();

        let written = write_with(
            &store,
            &indexer,
            Some(CORE_BUNDLE),
            Some("legacy"),
            "still editable",
            &policy(),
        )
        .await
        .expect("an existing document in the bundle can still be maintained");
        assert_eq!(written.memory.content, "still editable");

        // And a *new* one is still refused: the reservation is about creation.
        let err = write_with(
            &store,
            &indexer,
            Some(CORE_BUNDLE),
            Some("brand-new"),
            "nope",
            &policy(),
        )
        .await
        .unwrap_err()
        .to_string();
        assert!(err.starts_with("the core bundle holds only"), "{err}");
    }

    #[tokio::test]
    async fn the_reserved_bundle_cannot_be_deleted_even_with_force() {
        // T017 / FR-012: `force` must not bypass the refusal.
        let (store, indexer) = setup();
        write_with(
            &store,
            &indexer,
            Some(CORE_BUNDLE),
            Some(SOUL_PATH),
            "I am Oz.",
            &policy(),
        )
        .await
        .unwrap();

        let err = store
            .delete_bundle("a1".into(), CORE_BUNDLE.into(), true, &origin(), &indexer)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.starts_with("bundle 'core' is reserved"), "{err}");
    }

    #[tokio::test]
    async fn the_soul_is_deletable_and_the_bundle_survives_it() {
        // T017 / FR-009, FR-015, data-model invariant 3: deletion is allowed, records a
        // deletion revision, and leaves the reserved bundle listed.
        let (store, indexer) = setup();
        write_with(
            &store,
            &indexer,
            Some(CORE_BUNDLE),
            Some(SOUL_PATH),
            "I am Oz.",
            &policy(),
        )
        .await
        .unwrap();

        store
            .delete_memory(
                "a1".into(),
                Some(CORE_BUNDLE.into()),
                SOUL_PATH.into(),
                &origin(),
                &indexer,
            )
            .await
            .expect("the soul is removable");

        let history = store
            .list_memory_revisions(
                "a1".into(),
                Some(CORE_BUNDLE.into()),
                SOUL_PATH.into(),
                0,
                10,
            )
            .await
            .unwrap();
        assert!(
            history.revisions.iter().any(|r| r.deleted),
            "the removal is visible in the document's own history"
        );

        let bundles = store.list_bundles("a1".into()).await.unwrap();
        let core = bundles
            .iter()
            .find(|b| b.name == CORE_BUNDLE)
            .expect("an emptied reserved bundle is still listed");
        assert_eq!(core.concept_count, 0);
        assert!(core.protected);
    }

    #[tokio::test]
    async fn below_the_soft_limit_a_write_carries_no_warning() {
        // T023 / FR-016, data-model rule 6.
        let (store, indexer) = setup();
        let written = write_with(
            &store,
            &indexer,
            Some("work"),
            Some("small"),
            &body_of(2048),
            &policy(),
        )
        .await
        .unwrap();
        assert!(written.warning.is_none());
    }

    #[tokio::test]
    async fn between_the_soft_limit_and_the_ceiling_a_write_succeeds_with_a_warning() {
        // T023 / FR-019: saved in full, and the warning carries the real numbers.
        let (store, indexer) = setup();
        let written = write_with(
            &store,
            &indexer,
            Some("work"),
            Some("big"),
            &body_of(6144),
            &policy(),
        )
        .await
        .unwrap();
        let warning = written.warning.expect("over the soft limit warns");
        assert_eq!(warning.bytes, 6144);
        assert_eq!(warning.limit, 4096);
        assert_eq!(
            written.memory.content.len(),
            6144,
            "the whole content is stored, never truncated"
        );
    }

    #[tokio::test]
    async fn exactly_at_the_ceiling_is_accepted_and_one_byte_over_is_refused() {
        // T023 / FR-017, data-model rule 4: equality is accepted.
        let (store, indexer) = setup();
        let limits = MemoryLimits::default();

        write_with(
            &store,
            &indexer,
            Some("work"),
            Some("edge"),
            &body_of(limits.content_cap),
            &policy(),
        )
        .await
        .expect("a document exactly at the ceiling is accepted");

        let err = write_with(
            &store,
            &indexer,
            Some("work"),
            Some("over"),
            &body_of(limits.content_cap + 1),
            &policy(),
        )
        .await
        .unwrap_err()
        .to_string();
        assert!(err.starts_with("memory too large:"), "{err}");
    }

    #[tokio::test]
    async fn the_refusal_states_size_ceiling_overage_and_a_split_pattern() {
        // T023 / FR-018, data-model rule 11.
        let (store, indexer) = setup();
        let err = write_with(
            &store,
            &indexer,
            Some("vizier"),
            Some("workqueue"),
            &body_of(11240),
            &policy(),
        )
        .await
        .unwrap_err()
        .to_string();
        assert!(err.contains("11240 bytes"), "names the actual size: {err}");
        assert!(err.contains("8192-byte limit"), "names the ceiling: {err}");
        assert!(err.contains("3048 over"), "names the overage: {err}");
        assert!(
            err.contains("vizier/workqueue-<subject>"),
            "names a sibling path in the same bundle: {err}"
        );
        assert!(
            err.contains("[<subject>](workqueue-<subject>.md)"),
            "names the link syntax: {err}"
        );
    }

    #[tokio::test]
    async fn a_refusal_leaves_the_document_its_history_and_its_passages_untouched() {
        // T023 / FR-017, SC-009: no partial writes.
        let (store, indexer) = setup();
        write_with(
            &store,
            &indexer,
            Some("work"),
            Some("kept"),
            "the original",
            &policy(),
        )
        .await
        .unwrap();
        let before = store
            .list_memory_revisions("a1".into(), Some("work".into()), "kept".into(), 0, 10)
            .await
            .unwrap();

        assert!(
            write_with(
                &store,
                &indexer,
                Some("work"),
                Some("kept"),
                &body_of(20_000),
                &policy(),
            )
            .await
            .is_err()
        );

        let detail = store
            .get_memory_detail("a1".into(), Some("work".into()), "kept".into())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(detail.content, "the original");
        let after = store
            .list_memory_revisions("a1".into(), Some("work".into()), "kept".into(), 0, 10)
            .await
            .unwrap();
        assert_eq!(before.total, after.total, "no revision was recorded");
    }

    #[tokio::test]
    async fn a_legacy_over_cap_document_accepts_a_shorter_write_and_refuses_a_longer_one() {
        // T024 / FR-021: the ceiling must not become a trap on the documents it exists to fix.
        let (store, indexer) = setup();
        // The legacy state: above the ceiling, written before the limit existed.
        write_with(
            &store,
            &indexer,
            Some("work"),
            Some("legacy"),
            &body_of(10_000),
            &WritePolicy::migrating(),
        )
        .await
        .expect("the migration bypass can carry over-cap content");

        let err = write_with(
            &store,
            &indexer,
            Some("work"),
            Some("legacy"),
            &body_of(11_000),
            &policy(),
        )
        .await
        .unwrap_err()
        .to_string();
        assert!(
            err.starts_with("memory too large:"),
            "growing is refused: {err}"
        );

        let written = write_with(
            &store,
            &indexer,
            Some("work"),
            Some("legacy"),
            &body_of(3000),
            &policy(),
        )
        .await
        .expect("shrinking is accepted");
        assert_eq!(written.memory.content.len(), 3000);
    }

    #[tokio::test]
    async fn the_seed_carries_the_legacy_identity_verbatim_with_a_provenance_line() {
        // T010 / FR-003: copied byte for byte, plus one appended note, never split.
        let (store, indexer) = setup();
        let legacy = "# My core\n\nI am Oz, a software engineer.".to_string();
        assert!(
            store
                .seed_identity("a1", Some(legacy.clone()), &indexer, &limits())
                .await
                .unwrap(),
            "the first run writes"
        );

        let detail = store
            .get_memory_detail("a1".into(), Some(CORE_BUNDLE.into()), SOUL_PATH.into())
            .await
            .unwrap()
            .unwrap();
        assert!(
            detail.content.starts_with(&legacy),
            "the legacy text is carried verbatim at the front of the body"
        );
        assert!(
            detail.content.contains("carried over from"),
            "one provenance line is appended"
        );
    }

    #[tokio::test]
    async fn the_seed_falls_back_to_the_template_and_is_idempotent() {
        // T010 / FR-002, data-model rule 16: no legacy means the template, and a second run
        // changes nothing at all.
        let (store, indexer) = setup();
        assert!(
            store
                .seed_identity("a1", None, &indexer, &limits())
                .await
                .unwrap()
        );
        let first = store
            .get_memory_detail("a1".into(), Some(CORE_BUNDLE.into()), SOUL_PATH.into())
            .await
            .unwrap()
            .unwrap();
        // The template is the default identity. Asserted on a distinctive line rather than
        // the raw constant: the markdown layer normalises line endings on the way in.
        assert!(
            first.content.contains("## Operating Framework"),
            "the embedded template is the fallback identity"
        );

        assert!(
            !store
                .seed_identity("a1", Some("something else".into()), &indexer, &limits())
                .await
                .unwrap(),
            "a document already present means nothing to do"
        );
        let second = store
            .get_memory_detail("a1".into(), Some(CORE_BUNDLE.into()), SOUL_PATH.into())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            first.content, second.content,
            "no second provenance line, no rewrite"
        );
    }

    #[tokio::test]
    async fn an_over_ceiling_legacy_identity_still_migrates_in_full() {
        // T010 / FR-025: the upgrade can never be blocked by the ceiling introduced beside it.
        let (store, indexer) = setup();
        let legacy = body_of(6000); // over the identity ceiling of 4096
        store
            .seed_identity("a1", Some(legacy.clone()), &indexer, &limits())
            .await
            .expect("an upgrade is never refused by the new ceiling");

        let detail = store
            .get_memory_detail("a1".into(), Some(CORE_BUNDLE.into()), SOUL_PATH.into())
            .await
            .unwrap()
            .unwrap();
        assert!(detail.content.starts_with(&legacy), "carried without loss");
    }

    #[tokio::test]
    async fn the_identity_document_is_judged_against_the_tighter_pair() {
        // T023 / FR-023, data-model rule 8: the address decides, not a second config field.
        let (store, indexer) = setup();
        // 5000 bytes is comfortably under the ordinary ceiling…
        write_with(
            &store,
            &indexer,
            Some("work"),
            Some("ordinary"),
            &body_of(5000),
            &policy(),
        )
        .await
        .expect("under the ordinary ceiling");

        // …and over the identity one.
        let err = write_with(
            &store,
            &indexer,
            Some(CORE_BUNDLE),
            Some(SOUL_PATH),
            &body_of(5000),
            &policy(),
        )
        .await
        .unwrap_err()
        .to_string();
        assert!(
            err.contains("4096-byte limit"),
            "quotes the identity ceiling: {err}"
        );
    }

    #[tokio::test]
    async fn the_identity_document_warns_at_its_own_soft_limit() {
        // T023 / FR-023: the tighter pair applies to the warning band too.
        let (store, indexer) = setup();
        let written = write_with(
            &store,
            &indexer,
            Some(CORE_BUNDLE),
            Some(SOUL_PATH),
            &body_of(2000),
            &policy(),
        )
        .await
        .unwrap();
        let warning = written.warning.expect("over the identity soft limit");
        assert_eq!(warning.limit, 1536);
    }

    // ---- memory in the database (specs/070-memory-in-database) ----

    /// Every row and revision of the store, for "nothing changed" assertions.
    fn snapshot(store: &BundleMemoryStore) -> Vec<String> {
        let conn = store.conn.lock().unwrap();
        let mut out = Vec::new();
        for sql in [
            "SELECT agent_id || '|' || bundle || '|' || path || '|' || title || '|' || tags_json
                 || '|' || created_at || '|' || updated_at || '|' || COALESCE(content, '<none>')
             FROM memory_node ORDER BY 1",
            "SELECT source_bundle || '|' || source_path || '|' || target_bundle || '|'
                 || COALESCE(target_path, '') FROM memory_edge ORDER BY 1",
            "SELECT bundle || '|' || path || '|' || COALESCE(content, '<deleted>')
             FROM memory_revision ORDER BY 1",
            "SELECT bundle || '|' || path || '|' || content_hash FROM memory_passage ORDER BY 1",
        ] {
            let mut stmt = conn.prepare(sql).unwrap();
            out.extend(
                stmt.query_map(params![], |row| row.get::<_, String>(0))
                    .unwrap()
                    .map(|r| r.unwrap()),
            );
        }
        out
    }

    async fn put(
        store: &BundleMemoryStore,
        indexer: &VizierIndexer,
        bundle: &str,
        path: &str,
        body: &str,
    ) -> anyhow::Result<Memory> {
        Ok(store
            .write_memory(
                "a1".into(),
                Some(bundle.into()),
                Some(path.into()),
                false,
                format!("Title {path}"),
                body.into(),
                vec!["t".into()],
                &origin(),
                indexer,
                &limits(),
            )
            .await?
            .memory)
    }

    #[tokio::test]
    async fn a_read_writes_nothing() {
        let (store, indexer) = setup();
        put(&store, &indexer, "default", "a", "see [b](b) and [[other/c]]").await.unwrap();
        put(&store, &indexer, "default", "b", "b").await.unwrap();
        put(&store, &indexer, "other", "c", "c").await.unwrap();
        // A document whose passages are missing is still not rebuilt by a read.
        store.conn.lock().unwrap().execute("DELETE FROM memory_passage", []).unwrap();
        let before = snapshot(&store);

        let a1 = || "a1".to_string();
        store.get_memory_detail(a1(), None, "a".into()).await.unwrap().unwrap();
        assert_eq!(store.get_all_agent_memory(a1(), None).await.unwrap().len(), 3);
        assert_eq!(store.get_related_memories(a1(), None, "a".into()).await.unwrap().len(), 2);
        store.get_memory_graph(a1(), None, None).await.unwrap();
        store.get_memory_graph(a1(), Some("default".into()), None).await.unwrap();
        assert_eq!(store.list_bundles(a1()).await.unwrap().len(), 3, "two, and core");
        store
            .query_memory(a1(), None, "b".into(), 5, 0.0, 3, &indexer, &limits())
            .await
            .unwrap();
        store.list_memory_revisions(a1(), None, "a".into(), 0, 10).await.unwrap();
        store.export_bundle(a1(), "default".into()).await.unwrap();
        assert_eq!(snapshot(&store), before);

        store.increment_read_count(a1(), None, "a".into()).await.unwrap();
        let read = store.get_memory_detail(a1(), None, "a".into()).await.unwrap().unwrap();
        assert_eq!(read.read_count, 1);
        assert_eq!(snapshot(&store), before, "only the counter moved");
    }

    /// Everything a write changes is one transaction: a failure at the last step (the links)
    /// leaves the document, its revisions and its passages as they were.
    #[tokio::test]
    async fn a_write_that_fails_after_the_node_leaves_nothing_behind() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::storage::sqlite::init_memory_graph_schema(&conn).unwrap();
        crate::storage::sqlite::init_revision_schema(&conn).unwrap();
        let store = BundleMemoryStore::new(crate::storage::sql::SqlStore::sqlite(conn));
        let indexer = VizierIndexer::build(NoopIndexer);
        put(&store, &indexer, "default", "a", "first, with [b](b)").await.unwrap();
        let before = snapshot(&store);

        store
            .conn
            .lock()
            .unwrap()
            .execute_batch(
                "CREATE TRIGGER no_new_edges BEFORE INSERT ON memory_edge
                 BEGIN SELECT RAISE(ABORT, 'boom'); END;
                 CREATE TRIGGER no_dropped_edges BEFORE DELETE ON memory_edge
                 BEGIN SELECT RAISE(ABORT, 'boom'); END;",
            )
            .unwrap();

        assert!(put(&store, &indexer, "default", "a", "second, with [c](c)").await.is_err());
        assert_eq!(snapshot(&store), before, "a failed update changes nothing");
        assert!(
            store
                .delete_memory("a1".into(), None, "a".into(), &origin(), &indexer)
                .await
                .is_err()
        );
        assert_eq!(snapshot(&store), before, "a failed delete changes nothing");
    }

    #[tokio::test]
    async fn an_unsafe_address_is_refused() {
        let (store, indexer) = setup();
        for (bundle, path) in [("..", "a"), ("default", "../a"), ("default", "a/../../b")] {
            assert!(
                put(&store, &indexer, bundle, path, "x").await.is_err(),
                "{bundle}/{path} was accepted"
            );
        }
        assert!(store.get_all_agent_memory("a1".into(), None).await.unwrap().is_empty());
    }

    /// A row without a body (a file the migration could not move) is absent everywhere, and a
    /// write fills it.
    #[tokio::test]
    async fn a_row_without_a_body_is_absent_until_written() {
        let (store, indexer) = setup();
        let outcome = store
            .migrate_file("a1", "default", "bad", b"---\nnot: [valid\n---\nx")
            .unwrap();
        assert!(matches!(outcome, MigratedFile::Unreadable(_)), "{outcome:?}");
        assert!(
            store.get_memory_detail("a1".into(), None, "bad".into()).await.unwrap().is_none()
        );
        assert!(store.get_all_agent_memory("a1".into(), None).await.unwrap().is_empty());
        let bundles = store.list_bundles("a1".into()).await.unwrap();
        assert_eq!(bundles.len(), 1, "only the reserved bundle: {bundles:?}");

        store
            .write_memory(
                "a1".into(),
                None,
                Some("bad".into()),
                true,
                "Fixed".into(),
                "fixed".into(),
                vec![],
                &origin(),
                &indexer,
                &limits(),
            )
            .await
            .expect("the address is free to create");
        assert!(
            store.get_memory_detail("a1".into(), None, "bad".into()).await.unwrap().is_some()
        );
    }

    fn unzip(bytes: &[u8]) -> std::collections::BTreeMap<String, String> {
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes.to_vec())).unwrap();
        (0..archive.len())
            .map(|i| {
                let mut file = archive.by_index(i).unwrap();
                let name = file.name().unwrap().to_string();
                let mut text = String::new();
                file.read_to_string(&mut text).unwrap();
                (name, text)
            })
            .collect()
    }

    #[tokio::test]
    async fn export_round_trips_through_import() {
        let (store, indexer) = setup();
        put(&store, &indexer, "notes", "a", "see [b](b)").await.unwrap();
        put(&store, &indexer, "notes", "a", "see [b](b), again").await.unwrap();
        put(&store, &indexer, "notes", "nested/b", "deep").await.unwrap();
        put(&store, &indexer, "notes", "gone", "bye").await.unwrap();
        store
            .delete_memory(
                "a1".into(),
                Some("notes".into()),
                "gone".into(),
                &origin(),
                &indexer,
            )
            .await
            .unwrap();

        let zip = store.export_bundle("a1".into(), "notes".into()).await.unwrap();
        let files = unzip(&zip);
        assert_eq!(
            files.keys().map(String::as_str).collect::<Vec<_>>(),
            ["a.md", "index.md", "log.md", "nested/b.md"]
        );
        assert!(files["a.md"].starts_with("---\n"));
        assert!(files["index.md"].contains("| [a](a.md) | Title a | t |"));
        let log = &files["log.md"];
        assert!(log.starts_with("# notes log\n\n"), "{log}");
        let actions: Vec<&str> = log
            .lines()
            .filter(|l| l.starts_with("- "))
            .map(|l| l.split(" — ").nth(1).unwrap())
            .collect();
        assert_eq!(
            actions,
            [
                "created `a` \"Title a\"",
                "updated `a` \"Title a\"",
                "created `nested/b` \"Title nested/b\"",
                "created `gone` \"Title gone\"",
                "deleted `gone` \"Title gone\"",
            ]
        );

        let report = store
            .import_bundle(
                "a2".into(),
                "notes".into(),
                zip,
                &origin(),
                &indexer,
                &limits(),
            )
            .await
            .unwrap();
        assert_eq!(report.imported.len(), 2, "{report:?}");
        for path in ["a", "nested/b"] {
            let source = store
                .get_memory_detail("a1".into(), Some("notes".into()), path.into())
                .await
                .unwrap()
                .unwrap();
            let copy = store
                .get_memory_detail("a2".into(), Some("notes".into()), path.into())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                (copy.title, copy.content, copy.tags, copy.relations, copy.created_at),
                (
                    source.title,
                    source.content,
                    source.tags,
                    source.relations,
                    source.created_at
                )
            );
        }
    }

    /// A memory titled "Log" or "Index" is filed under the name of a generated file.
    #[tokio::test]
    async fn a_document_named_like_a_generated_file_is_still_exported() {
        let (store, indexer) = setup();
        put(&store, &indexer, "notes", "log", "my own log").await.unwrap();
        put(&store, &indexer, "notes", "index", "my own index").await.unwrap();

        let files = unzip(&store.export_bundle("a1".into(), "notes".into()).await.unwrap());
        assert_eq!(
            files.keys().map(String::as_str).collect::<Vec<_>>(),
            ["index.md", "log.md"]
        );
        assert!(files["log.md"].ends_with("my own log"), "{files:?}");
        assert!(files["index.md"].ends_with("my own index"), "{files:?}");
    }

    /// An archive as releases before the move wrote it: the files of the bundle's folder, with
    /// the retired attachment keys and a stale `relations` list in the frontmatter.
    #[tokio::test]
    async fn an_archive_from_the_previous_release_imports() {
        let old = "---\nslug: gamma\ntitle: Gamma\ncreated_at: 2024-01-01T00:00:00Z\n\
                   updated_at: 2024-01-02T00:00:00Z\nagent_id: someone\nbundle: elsewhere\n\
                   tags:\n- x\nkeywords: []\nrelations: []\nattachments: []\n\
                   attachment_count: 0\nread_count: 7\n---\nsee [Alpha](alpha)";
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buf);
            let options = zip::write::SimpleFileOptions::default();
            for (name, text) in [
                ("gamma.md", old),
                ("index.md", "# elsewhere\n"),
                ("log.md", "# elsewhere log\n"),
            ] {
                zip.start_file(name, options).unwrap();
                zip.write_all(text.as_bytes()).unwrap();
            }
            zip.finish().unwrap();
        }
        let (store, indexer) = setup();
        let report = store
            .import_bundle(
                "a1".into(),
                "here".into(),
                buf.into_inner(),
                &origin(),
                &indexer,
                &limits(),
            )
            .await
            .unwrap();
        assert_eq!(report.imported, vec!["gamma".to_string()]);
        let gamma = store
            .get_memory_detail("a1".into(), Some("here".into()), "gamma".into())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(gamma.title, "Gamma");
        assert_eq!(gamma.content, "see [Alpha](alpha)");
        assert_eq!(gamma.read_count, 7);
        assert_eq!(gamma.relations, vec!["alpha.md".to_string()]);
        let listed = store.get_all_agent_memory("a1".into(), None).await.unwrap();
        assert_eq!(listed[0].relations, vec!["alpha.md".to_string()]);
    }

    crate::postgres_tests!(
        collision_rejected_on_create_only,
        update_same_path_is_allowed_when_not_create_only,
        implicit_bundle_and_subdirectory_creation,
        same_bundle_and_cross_bundle_links_resolve,
        same_bundle_link_without_md_extension_still_resolves,
        extensionless_link_is_ignored_when_it_has_a_different_extension,
        cross_bundle_reference_written_as_a_same_bundle_style_path_still_resolves,
        bare_wikilink_falls_back_to_same_bundle_concept_when_no_such_bundle_exists,
        broken_links_are_tolerated_not_errors,
        delete_bundle_rejected_when_not_empty,
        delete_bundle_succeeds_when_empty,
        delete_bundle_with_force_removes_remaining_concepts_too,
        every_save_records_a_revision_with_its_origin_and_no_op_saves_are_skipped,
        delete_records_a_deletion_entry_and_a_pre_history_document_gets_a_baseline,
        a_write_produces_passage_rows_and_one_index_entry_per_passage,
        a_second_write_replaces_passages_rather_than_appending,
        a_document_shrinking_from_twelve_passages_to_four_leaves_no_orphaned_index_rows,
        deleting_a_memory_clears_both_the_passage_rows_and_the_index,
        deleting_a_bundle_clears_passages_for_every_concept_in_it,
        importing_a_bundle_produces_passages_for_every_document,
        reconcile_converts_only_what_needs_it_and_rebuilds_on_drift,
        resetting_passages_makes_the_next_reconcile_reindex_the_corpus,
        the_identity_document_is_an_ordinary_round_tripping_memory,
        the_reserved_bundle_refuses_a_foreign_document,
        an_import_into_the_reserved_bundle_is_refused_but_an_export_is_allowed,
        a_pre_reservation_document_in_a_legacy_core_bundle_stays_maintainable,
        the_reserved_bundle_cannot_be_deleted_even_with_force,
        the_soul_is_deletable_and_the_bundle_survives_it,
        below_the_soft_limit_a_write_carries_no_warning,
        between_the_soft_limit_and_the_ceiling_a_write_succeeds_with_a_warning,
        exactly_at_the_ceiling_is_accepted_and_one_byte_over_is_refused,
        the_refusal_states_size_ceiling_overage_and_a_split_pattern,
        a_refusal_leaves_the_document_its_history_and_its_passages_untouched,
        a_legacy_over_cap_document_accepts_a_shorter_write_and_refuses_a_longer_one,
        the_seed_carries_the_legacy_identity_verbatim_with_a_provenance_line,
        the_seed_falls_back_to_the_template_and_is_idempotent,
        an_over_ceiling_legacy_identity_still_migrates_in_full,
        the_identity_document_is_judged_against_the_tighter_pair,
        the_identity_document_warns_at_its_own_soft_limit,
        a_read_writes_nothing,
        an_unsafe_address_is_refused,
        a_row_without_a_body_is_absent_until_written,
        export_round_trips_through_import,
        a_document_named_like_a_generated_file_is_still_exported,
        an_archive_from_the_previous_release_imports,
    );
}
