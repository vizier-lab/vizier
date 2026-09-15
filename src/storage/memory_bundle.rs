use std::collections::{BTreeSet, HashSet};
use std::io::{Read, Write};
use std::sync::Arc;

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use regex::Regex;
use rusqlite::{Connection, params};
use serde::{Serialize, de::DeserializeOwned};
use slugify::slugify;

use crate::{
    indexer::VizierIndexer,
    schema::{
        BundleSummary, ImportReport, Memory, MemoryFrontMatter, MemoryGraph, MemoryGraphEdge,
        MemoryGraphNode, MemoryQueryParams, MemoryRevision, MemoryRevisionSummary,
        PaginatedMemory, PaginatedMemoryRevisions, RevisionDiff, RevisionOrigin, RevisionTrigger,
        RollbackResponse, VizierAttachment, default_bundle,
    },
    storage::{
        diff::diff_lines,
        document::DocumentStore,
        memory::compute_initial_slugs,
        sqlite::memory_revision::{
            self, MemoryRevisionRow, memory_canonical, parse_memory_canonical,
        },
    },
};

/// The shared implementation behind `impl MemoryStorage for SqliteStorage`: bundle/concept
/// document read+write, link resolution, index/log maintenance, and the Memory Graph Index
/// (`memory_node`/`memory_edge`) that every listing/graph read is served from.
pub struct BundleMemoryStore {
    document_store: Arc<dyn DocumentStore>,
    conn: Arc<Mutex<Connection>>,
}

struct NodeRow {
    bundle: String,
    path: String,
    title: String,
    tags: Vec<String>,
    attachment_count: usize,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    read_count: u64,
}

struct ClassifiedLink {
    target_bundle: String,
    target_path: Option<String>,
    kind: &'static str,
}

fn normalize_path(raw: &str) -> String {
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

/// Cross-bundle wikilinks: `[[bundle/slug]]` or bare `[[bundle]]` (FR-013).
fn parse_wikilinks(content: &str) -> Vec<String> {
    let re = Regex::new(r"\[\[([^\]]+)\]\]").unwrap();
    re.captures_iter(content)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str().trim().to_string()))
        .filter(|s| !s.is_empty())
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
    let raw = String::from_utf8_lossy(bytes).to_string();
    let mut lines: Vec<&str> = raw.split(['\n', '\r']).collect();
    if lines.is_empty() || lines.remove(0) != "---" {
        return Err(anyhow!("missing frontmatter"));
    }
    let mut frontmatter_raw = Vec::new();
    loop {
        if lines.is_empty() {
            return Err(anyhow!("unterminated frontmatter"));
        }
        let line = lines.remove(0);
        if line == "---" {
            break;
        }
        frontmatter_raw.push(line);
    }
    let frontmatter: T = serde_yaml::from_str(&frontmatter_raw.join("\n"))?;
    let body = lines.join("\n");
    Ok((frontmatter, body))
}

/// The canonical snapshot text of an on-disk concept document (`None` when it is missing or
/// unparseable) — what its history compares against and what a baseline is seeded from.
fn canonical_of_bytes(bytes: &[u8]) -> Option<String> {
    let (fm, body) = parse_markdown_bytes::<MemoryFrontMatter>(bytes).ok()?;
    memory_canonical(&fm.title, &fm.tags, &fm.attachments, &body).ok()
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
        attachment_count: fm.attachments.len(),
        attachments: fm.attachments,
        read_count: fm.read_count,
    }
}

impl BundleMemoryStore {
    pub fn new(document_store: Arc<dyn DocumentStore>, conn: Arc<Mutex<Connection>>) -> Self {
        Self {
            document_store,
            conn,
        }
    }

    fn doc_key(agent_id: &str, bundle: &str, path: &str) -> String {
        format!("{agent_id}/memory/{bundle}/{path}.md")
    }

    fn bundle_prefix(agent_id: &str, bundle: &str) -> String {
        format!("{agent_id}/memory/{bundle}")
    }

    fn index_key(agent_id: &str, bundle: &str) -> String {
        format!("{agent_id}/memory/{bundle}/index.md")
    }

    fn log_key(agent_id: &str, bundle: &str) -> String {
        format!("{agent_id}/memory/{bundle}/log.md")
    }

    fn indexer_key(agent_id: &str, bundle: &str, path: &str) -> String {
        format!("{agent_id}/{bundle}/{path}")
    }

    fn parse_indexer_key(key: &str) -> Option<(String, String, String)> {
        let mut parts = key.splitn(3, '/');
        let agent_id = parts.next()?.to_string();
        let bundle = parts.next()?.to_string();
        let path = parts.next()?.to_string();
        Some((agent_id, bundle, path))
    }

    // ---- Memory Graph Index (sqlite) ----

    #[allow(clippy::too_many_arguments)]
    fn upsert_node(
        &self,
        agent_id: &str,
        bundle: &str,
        path: &str,
        slug_leaf: &str,
        title: &str,
        tags: &[String],
        attachment_count: usize,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        read_count: u64,
    ) -> Result<()> {
        let tags_json = serde_json::to_string(tags)?;
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO memory_node
                (agent_id, bundle, path, slug, title, tags_json, attachment_count, created_at, updated_at, read_count)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
             ON CONFLICT(agent_id, bundle, path) DO UPDATE SET
                slug = excluded.slug,
                title = excluded.title,
                tags_json = excluded.tags_json,
                attachment_count = excluded.attachment_count,
                created_at = excluded.created_at,
                updated_at = excluded.updated_at,
                read_count = excluded.read_count",
            params![
                agent_id,
                bundle,
                path,
                slug_leaf,
                title,
                tags_json,
                attachment_count as i64,
                created_at.to_rfc3339(),
                updated_at.to_rfc3339(),
                read_count as i64
            ],
        )?;
        Ok(())
    }

    fn upsert_node_from_frontmatter(
        &self,
        agent_id: &str,
        bundle: &str,
        path: &str,
        fm: &MemoryFrontMatter,
    ) -> Result<()> {
        self.upsert_node(
            agent_id,
            bundle,
            path,
            &leaf_of(path),
            &fm.title,
            &fm.tags,
            fm.attachments.len(),
            fm.created_at,
            fm.updated_at,
            fm.read_count,
        )
    }

    // ---- Version history (sqlite `memory_revision`) ----

    fn record_revision(
        &self,
        agent_id: &str,
        bundle: &str,
        path: &str,
        content: Option<&str>,
        origin: &RevisionOrigin,
        current_before_save: Option<&str>,
    ) -> Result<Option<i64>> {
        let conn = self.conn.lock();
        memory_revision::record(
            &conn,
            agent_id,
            bundle,
            path,
            content,
            origin,
            current_before_save,
        )
    }

    fn delete_node(&self, agent_id: &str, bundle: &str, path: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM memory_node WHERE agent_id = ?1 AND bundle = ?2 AND path = ?3",
            params![agent_id, bundle, path],
        )?;
        Ok(())
    }

    fn list_nodes(&self, agent_id: &str, bundle: Option<&str>) -> Result<Vec<NodeRow>> {
        let conn = self.conn.lock();
        let map_row = |row: &rusqlite::Row| -> rusqlite::Result<NodeRow> {
            let tags_json: String = row.get(2)?;
            let created_at: String = row.get(4)?;
            let updated_at: String = row.get(5)?;
            Ok(NodeRow {
                bundle: row.get(0)?,
                path: row.get(1)?,
                title: row.get(6)?,
                tags: serde_json::from_str(&tags_json).unwrap_or_default(),
                attachment_count: row.get::<_, i64>(3)? as usize,
                created_at: DateTime::parse_from_rfc3339(&created_at)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                updated_at: DateTime::parse_from_rfc3339(&updated_at)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                read_count: row.get::<_, i64>(7)? as u64,
            })
        };

        let rows = if let Some(b) = bundle {
            let mut stmt = conn.prepare(
                "SELECT bundle, path, tags_json, attachment_count, created_at, updated_at, title, read_count
                 FROM memory_node WHERE agent_id = ?1 AND bundle = ?2",
            )?;
            stmt.query_map(params![agent_id, b], map_row)?
                .filter_map(|r| r.ok())
                .collect()
        } else {
            let mut stmt = conn.prepare(
                "SELECT bundle, path, tags_json, attachment_count, created_at, updated_at, title, read_count
                 FROM memory_node WHERE agent_id = ?1",
            )?;
            stmt.query_map(params![agent_id], map_row)?
                .filter_map(|r| r.ok())
                .collect()
        };
        Ok(rows)
    }

    fn relations_for(&self, agent_id: &str, bundle: &str, path: &str) -> Result<Vec<String>> {
        let conn = self.conn.lock();
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
                "cross_bundle_concept" => {
                    relations.push(format!("{}/{}", target_bundle, target_path.unwrap_or_default()))
                }
                _ => relations.push(target_bundle),
            }
        }
        Ok(relations)
    }

    fn rewrite_edges(
        &self,
        agent_id: &str,
        source_bundle: &str,
        source_path: &str,
        relations: &[String],
    ) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM memory_edge WHERE agent_id = ?1 AND source_bundle = ?2 AND source_path = ?3",
            params![agent_id, source_bundle, source_path],
        )?;
        for relation in relations {
            let classified = Self::resolve_relation(&conn, agent_id, source_bundle, relation)?;
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
            "SELECT COUNT(*) FROM memory_node WHERE agent_id = ?1 AND bundle = ?2 AND path = ?3",
            params![agent_id, bundle, path],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    fn bundle_has_any_node(conn: &Connection, agent_id: &str, bundle: &str) -> Result<bool> {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM memory_node WHERE agent_id = ?1 AND bundle = ?2",
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

    fn delete_edges_from(&self, agent_id: &str, bundle: &str, path: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM memory_edge WHERE agent_id = ?1 AND source_bundle = ?2 AND source_path = ?3",
            params![agent_id, bundle, path],
        )?;
        Ok(())
    }

    fn recompute_broken(&self, agent_id: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE memory_edge SET broken = 1 WHERE agent_id = ?1
             AND target_kind IN ('same_bundle','cross_bundle_concept')
             AND NOT EXISTS (
                SELECT 1 FROM memory_node n
                WHERE n.agent_id = memory_edge.agent_id AND n.bundle = memory_edge.target_bundle
                  AND n.path = memory_edge.target_path)",
            params![agent_id],
        )?;
        conn.execute(
            "UPDATE memory_edge SET broken = 0 WHERE agent_id = ?1
             AND target_kind IN ('same_bundle','cross_bundle_concept')
             AND EXISTS (
                SELECT 1 FROM memory_node n
                WHERE n.agent_id = memory_edge.agent_id AND n.bundle = memory_edge.target_bundle
                  AND n.path = memory_edge.target_path)",
            params![agent_id],
        )?;
        conn.execute(
            "UPDATE memory_edge SET broken = 1 WHERE agent_id = ?1 AND target_kind = 'cross_bundle_bundle'
             AND NOT EXISTS (
                SELECT 1 FROM memory_node n
                WHERE n.agent_id = memory_edge.agent_id AND n.bundle = memory_edge.target_bundle)",
            params![agent_id],
        )?;
        conn.execute(
            "UPDATE memory_edge SET broken = 0 WHERE agent_id = ?1 AND target_kind = 'cross_bundle_bundle'
             AND EXISTS (
                SELECT 1 FROM memory_node n
                WHERE n.agent_id = memory_edge.agent_id AND n.bundle = memory_edge.target_bundle)",
            params![agent_id],
        )?;
        Ok(())
    }

    // ---- Bundle/document discovery + reconciliation ----

    async fn discover_bundles(&self, agent_id: &str) -> Result<Vec<String>> {
        let prefix = format!("{agent_id}/memory");
        let files = self.document_store.list(&prefix).await?;
        let mut bundles: HashSet<String> = files
            .into_iter()
            .filter_map(|f| f.split('/').next().map(|s| s.to_string()))
            .collect();

        let cached: Vec<String> = {
            let conn = self.conn.lock();
            let mut stmt =
                conn.prepare("SELECT DISTINCT bundle FROM memory_node WHERE agent_id = ?1")?;
            stmt.query_map(params![agent_id], |row| row.get::<_, String>(0))?
                .filter_map(|r| r.ok())
                .collect()
        };
        bundles.extend(cached);

        let mut result: Vec<String> = bundles.into_iter().collect();
        result.sort();
        Ok(result)
    }

    /// Diffs the concept documents actually present in a bundle against `memory_node` rows for
    /// that bundle, correcting added/removed documents (research.md §9, FR-024). Documents in
    /// both sets whose *content* was hand-edited without going through Vizier are refreshed the
    /// next time they're read individually (`get_memory_detail`), not by this scan — the
    /// `DocumentStore` trait doesn't expose mtimes/hashes to diff on cheaply.
    async fn reconcile_bundle(&self, agent_id: &str, bundle: &str) -> Result<()> {
        let prefix = Self::bundle_prefix(agent_id, bundle);
        let files = self.document_store.list(&prefix).await?;
        let doc_paths: HashSet<String> = files
            .into_iter()
            .filter(|f| f.ends_with(".md"))
            .map(|f| normalize_path(&f))
            .filter(|p| {
                let leaf = leaf_of(p);
                leaf != "index" && leaf != "log"
            })
            .collect();

        let cached_paths: HashSet<String> = {
            let conn = self.conn.lock();
            let mut stmt = conn
                .prepare("SELECT path FROM memory_node WHERE agent_id = ?1 AND bundle = ?2")?;
            stmt.query_map(params![agent_id, bundle], |row| row.get::<_, String>(0))?
                .filter_map(|r| r.ok())
                .collect()
        };

        let mut changed = false;

        for stale in cached_paths.difference(&doc_paths) {
            self.delete_node(agent_id, bundle, stale)?;
            self.delete_edges_from(agent_id, bundle, stale)?;
            changed = true;
        }

        for added in doc_paths.difference(&cached_paths) {
            let key = Self::doc_key(agent_id, bundle, added);
            if let Some(bytes) = self.document_store.get(&key).await? {
                if let Ok((fm, _)) = parse_markdown_bytes::<MemoryFrontMatter>(&bytes) {
                    self.upsert_node_from_frontmatter(agent_id, bundle, added, &fm)?;
                    self.rewrite_edges(agent_id, bundle, added, &fm.relations)?;
                    changed = true;
                }
            }
        }

        if changed {
            self.recompute_broken(agent_id)?;
        }

        Ok(())
    }

    async fn reconcile_all(&self, agent_id: &str) -> Result<()> {
        let bundles = self.discover_bundles(agent_id).await?;
        for bundle in bundles {
            self.reconcile_bundle(agent_id, &bundle).await?;
        }
        Ok(())
    }

    // ---- Index/log documents ----

    async fn regenerate_index(&self, agent_id: &str, bundle: &str) -> Result<()> {
        let mut nodes = self.list_nodes(agent_id, Some(bundle))?;
        nodes.sort_by(|a, b| a.path.cmp(&b.path));

        let mut body = format!(
            "# {bundle}\n\n{} concept(s) in this bundle.\n\n| Path | Title | Tags | Updated |\n|---|---|---|---|\n",
            nodes.len()
        );
        for n in &nodes {
            body += &format!(
                "| [{path}]({path}.md) | {title} | {tags} | {updated} |\n",
                path = n.path,
                title = n.title,
                tags = n.tags.join(", "),
                updated = n.updated_at.to_rfc3339(),
            );
        }

        self.document_store
            .put(&Self::index_key(agent_id, bundle), body.into_bytes())
            .await?;
        Ok(())
    }

    async fn append_log(&self, agent_id: &str, bundle: &str, action: &str, path: &str, title: &str) -> Result<()> {
        let key = Self::log_key(agent_id, bundle);
        let mut existing = match self.document_store.get(&key).await? {
            Some(bytes) => String::from_utf8_lossy(&bytes).to_string(),
            None => format!("# {bundle} log\n\n"),
        };
        existing += &format!(
            "- {} — {} `{}` \"{}\"\n",
            Utc::now().to_rfc3339(),
            action,
            path,
            title
        );
        self.document_store.put(&key, existing.into_bytes()).await?;
        Ok(())
    }

    fn node_to_memory(&self, agent_id: &str, n: &NodeRow) -> Result<Memory> {
        let relations = self.relations_for(agent_id, &n.bundle, &n.path)?;
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
            attachments: vec![],
            attachment_count: n.attachment_count,
            read_count: n.read_count,
        })
    }

    // ---- MemoryStorage-shaped methods (called by the trait impl in sqlite/memory.rs) ----

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
        attachments: Vec<VizierAttachment>,
        origin: &RevisionOrigin,
        indexer: &VizierIndexer,
    ) -> Result<Memory> {
        let bundle = bundle.unwrap_or_else(default_bundle);
        let path = normalize_path(&path.unwrap_or_else(|| slugify!(&title)));
        if path.is_empty() {
            return Err(anyhow!("resolved memory path is empty"));
        }

        let key = Self::doc_key(&agent_id, &bundle, &path);
        let existing = self.document_store.get(&key).await?;
        // This (bundle, path) collision check (FR-011) is also exactly what US3/FR-014's
        // portability story relies on: copying another deployment's bundle directory tree in
        // and writing a *new* concept can never silently clobber a concept that arrived via the
        // copy, because both are addressed by this same `(bundle, path)` key. No separate
        // id-collision mechanism exists or is needed for the copy scenario.
        if existing.is_some() && create_only {
            return Err(anyhow!(
                "a memory already exists at bundle '{bundle}' path '{path}'"
            ));
        }

        let now = Utc::now();
        let (created_at, read_count) = match &existing {
            Some(bytes) => match parse_markdown_bytes::<MemoryFrontMatter>(bytes) {
                Ok((fm, _)) => (fm.created_at, fm.read_count),
                Err(_) => (now, 0),
            },
            None => (now, 0),
        };

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
            attachment_count: attachments.len(),
            attachments: attachments.clone(),
            read_count,
        };

        let canonical_before = existing.as_deref().and_then(canonical_of_bytes);
        let canonical_new = memory_canonical(&title, &tags, &attachments, &content)?;

        let bytes = serialize_markdown(&frontmatter, &content)?;
        self.document_store.put(&key, bytes).await?;

        self.upsert_node_from_frontmatter(&agent_id, &bundle, &path, &frontmatter)?;
        // The revision is written right after the graph node, in the same failure domain: the
        // document is already on disk, so a failed insert here surfaces as the save's error and
        // the next successful save (or a lazy baseline) records it (research Decision 8).
        self.record_revision(
            &agent_id,
            &bundle,
            &path,
            Some(&canonical_new),
            origin,
            canonical_before.as_deref(),
        )?;
        self.rewrite_edges(&agent_id, &bundle, &path, &relations)?;
        self.recompute_broken(&agent_id)?;

        indexer
            .add_document_index(
                "memory".into(),
                Self::indexer_key(&agent_id, &bundle, &path),
                content.clone(),
            )
            .await?;

        self.regenerate_index(&agent_id, &bundle).await?;
        self.append_log(
            &agent_id,
            &bundle,
            if existing.is_some() { "updated" } else { "created" },
            &path,
            &title,
        )
        .await?;

        Ok(memory_from_frontmatter(frontmatter, content))
    }

    pub async fn query_memory(
        &self,
        agent_id: String,
        bundle: Option<String>,
        query: String,
        limit: usize,
        threshold: f64,
        indexer: &VizierIndexer,
    ) -> Result<Vec<Memory>> {
        let fetch_limit = limit * 5;
        let documents = indexer
            .search_document_index("memory".into(), query, fetch_limit, threshold)
            .await?;

        let mut candidates = Vec::new();
        for doc in documents {
            let Some((doc_agent, doc_bundle, doc_path)) = Self::parse_indexer_key(&doc.path)
            else {
                continue;
            };
            if doc_agent != agent_id {
                continue;
            }
            if let Some(want) = &bundle {
                if &doc_bundle != want {
                    continue;
                }
            }
            // A stale/invalid indexer entry degrades to "no candidate", not a hard error.
            if let Ok(Some(memory)) = self
                .get_memory_detail(agent_id.clone(), Some(doc_bundle), doc_path)
                .await
            {
                candidates.push(memory);
            }
        }

        let all_memories = self.get_all_agent_memory(agent_id, bundle).await?;
        let reranked = crate::storage::rerank::rerank_memories(candidates, &all_memories);
        Ok(reranked.into_iter().take(limit).collect())
    }

    pub async fn get_all_agent_memory(
        &self,
        agent_id: String,
        bundle: Option<String>,
    ) -> Result<Vec<Memory>> {
        match &bundle {
            Some(b) => self.reconcile_bundle(&agent_id, b).await?,
            None => self.reconcile_all(&agent_id).await?,
        }

        let nodes = self.list_nodes(&agent_id, bundle.as_deref())?;
        nodes
            .iter()
            .map(|n| self.node_to_memory(&agent_id, n))
            .collect()
    }

    pub async fn get_filtered_memories(&self, params: MemoryQueryParams) -> Result<PaginatedMemory> {
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

        filtered = filtered.into_iter().skip(params.offset).take(params.limit).collect();

        Ok(PaginatedMemory {
            memories: filtered,
            total,
            offset: params.offset,
            limit: params.limit,
        })
    }

    /// Reads a document fresh from `DocumentStore`, always re-deriving `relations` from its
    /// actual content rather than trusting whatever was last stored in the cache or the on-disk
    /// frontmatter — this is what makes a link-parsing fix (or any future one) self-healing for
    /// documents written before it landed, the moment each is next touched, with no separate
    /// migration needed. If the freshly parsed set differs from what's on disk, the frontmatter
    /// is repaired in place (relations only — `created_at`/`updated_at` are untouched, this
    /// isn't a real "edit"). Updates `memory_node`/`memory_edge` for this one document; does
    /// *not* call `recompute_broken` (callers do that once, after they're done touching
    /// whichever documents they needed to for the operation at hand).
    async fn read_and_sync_document(
        &self,
        agent_id: &str,
        bundle: &str,
        path: &str,
    ) -> Result<Option<Memory>> {
        let key = Self::doc_key(agent_id, bundle, path);

        match self.document_store.get(&key).await? {
            None => {
                self.delete_node(agent_id, bundle, path)?;
                self.delete_edges_from(agent_id, bundle, path)?;
                Ok(None)
            }
            Some(bytes) => {
                let (mut fm, content) = parse_markdown_bytes::<MemoryFrontMatter>(&bytes)?;

                let relations = parse_relations(&content);
                if relations != fm.relations {
                    fm.relations = relations.clone();
                    let out = serialize_markdown(&fm, &content)?;
                    self.document_store.put(&key, out).await?;
                }

                self.upsert_node_from_frontmatter(agent_id, bundle, path, &fm)?;
                self.rewrite_edges(agent_id, bundle, path, &relations)?;
                Ok(Some(memory_from_frontmatter(fm, content)))
            }
        }
    }

    pub async fn get_memory_detail(
        &self,
        agent_id: String,
        bundle: Option<String>,
        path: String,
    ) -> Result<Option<Memory>> {
        let bundle = bundle.unwrap_or_else(default_bundle);
        let path = normalize_path(&path);
        let result = self.read_and_sync_document(&agent_id, &bundle, &path).await?;
        self.recompute_broken(&agent_id)?;
        Ok(result)
    }

    pub async fn get_related_memories(
        &self,
        agent_id: String,
        bundle: Option<String>,
        path: String,
    ) -> Result<Vec<Memory>> {
        let bundle = bundle.unwrap_or_else(default_bundle);
        let path = normalize_path(&path);
        self.reconcile_bundle(&agent_id, &bundle).await?;
        // Refresh this document's own outgoing edges from its actual content before reading
        // them — `reconcile_bundle` only catches documents added/removed since the cache was
        // last built, not a relation-parsing drift on one that's already present in both.
        self.read_and_sync_document(&agent_id, &bundle, &path).await?;
        self.recompute_broken(&agent_id)?;

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
            let conn = self.conn.lock();
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
                    let nodes = self.list_nodes(&agent_id, Some(&edge.bundle))?;
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
                            .get_memory_detail(agent_id.clone(), Some(edge.bundle.clone()), target_path)
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
            let conn = self.conn.lock();
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
                self.reconcile_all(&agent_id).await?;
                let bundles = self.discover_bundles(&agent_id).await?;
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
                    let conn = self.conn.lock();
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
                self.reconcile_bundle(&agent_id, &name).await?;
                let node_rows = self.list_nodes(&agent_id, Some(&name))?;
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
                    let conn = self.conn.lock();
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

                let all_bundles: HashSet<String> =
                    self.discover_bundles(&agent_id).await?.into_iter().collect();
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
        let conn = self.conn.lock();
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
        let key = Self::doc_key(&agent_id, &bundle, &path);

        let existing = self.document_store.get(&key).await?;
        let title = match &existing {
            Some(bytes) => parse_markdown_bytes::<MemoryFrontMatter>(bytes)
                .map(|(fm, _)| fm.title)
                .unwrap_or_else(|_| path.clone()),
            None => path.clone(),
        };
        let canonical_before = existing.as_deref().and_then(canonical_of_bytes);

        // Recorded before the file goes so a deletion entry always follows the content it
        // removed (and a baseline is seeded first if the document pre-dates history).
        self.record_revision(
            &agent_id,
            &bundle,
            &path,
            None,
            origin,
            canonical_before.as_deref(),
        )?;

        self.document_store.delete(&key).await?;
        self.delete_node(&agent_id, &bundle, &path)?;
        self.delete_edges_from(&agent_id, &bundle, &path)?;
        self.recompute_broken(&agent_id)?;

        let _ = indexer
            .delete_index("memory".into(), Self::indexer_key(&agent_id, &bundle, &path))
            .await;

        self.regenerate_index(&agent_id, &bundle).await?;
        self.append_log(&agent_id, &bundle, "deleted", &path, &title).await?;

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
        let key = Self::doc_key(&agent_id, &bundle, &path);

        if let Some(bytes) = self.document_store.get(&key).await? {
            let (mut fm, content) = parse_markdown_bytes::<MemoryFrontMatter>(&bytes)?;
            fm.read_count += 1;
            let out = serialize_markdown(&fm, &content)?;
            self.document_store.put(&key, out).await?;
            self.upsert_node_from_frontmatter(&agent_id, &bundle, &path, &fm)?;
        }
        Ok(())
    }

    pub async fn list_bundles(&self, agent_id: String) -> Result<Vec<BundleSummary>> {
        self.reconcile_all(&agent_id).await?;
        let bundles = self.discover_bundles(&agent_id).await?;

        let conn = self.conn.lock();
        let mut result = Vec::new();
        for bundle in bundles {
            let (count, updated): (i64, Option<String>) = conn.query_row(
                "SELECT COUNT(*), MAX(updated_at) FROM memory_node WHERE agent_id = ?1 AND bundle = ?2",
                params![agent_id, bundle],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            let updated_at = updated
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|d| d.with_timezone(&Utc));
            result.push(BundleSummary {
                name: bundle,
                concept_count: count as usize,
                updated_at,
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
        self.reconcile_bundle(&agent_id, &bundle).await?;

        let concept_paths: Vec<String> = self
            .list_nodes(&agent_id, Some(&bundle))?
            .into_iter()
            .map(|n| n.path)
            .collect();

        if !concept_paths.is_empty() && !force {
            return Err(anyhow!(
                "bundle '{bundle}' still has {} concept(s); delete them first",
                concept_paths.len()
            ));
        }

        // Only reached with `force`: every remaining concept gets its own deletion entry, the
        // same as `delete_memory` would record. `index.md`/`log.md` are not versioned documents.
        for path in &concept_paths {
            let key = Self::doc_key(&agent_id, &bundle, path);
            let canonical_before = self
                .document_store
                .get(&key)
                .await?
                .as_deref()
                .and_then(canonical_of_bytes);
            self.record_revision(
                &agent_id,
                &bundle,
                path,
                None,
                origin,
                canonical_before.as_deref(),
            )?;
            self.document_store.delete(&key).await?;
            let _ = indexer
                .delete_index("memory".into(), Self::indexer_key(&agent_id, &bundle, path))
                .await;
        }

        let prefix = Self::bundle_prefix(&agent_id, &bundle);
        let files = self.document_store.list(&prefix).await?;
        for file in files {
            self.document_store.delete(&format!("{prefix}/{file}")).await?;
        }

        // Nothing should remain in memory_node/memory_edge for an already-empty bundle, but
        // clear defensively in case a concurrent write raced this call.
        {
            let conn = self.conn.lock();
            conn.execute(
                "DELETE FROM memory_node WHERE agent_id = ?1 AND bundle = ?2",
                params![agent_id, bundle],
            )?;
            conn.execute(
                "DELETE FROM memory_edge WHERE agent_id = ?1 AND source_bundle = ?2",
                params![agent_id, bundle],
            )?;
        }
        // Other bundles may have referenced this one via a whole-bundle [[bundle]] link or a
        // now-gone [[bundle/slug]] concept — those edges should flip to broken, not vanish.
        self.recompute_broken(&agent_id)?;

        Ok(())
    }

    pub async fn export_bundle(&self, agent_id: String, bundle: String) -> Result<Vec<u8>> {
        self.reconcile_bundle(&agent_id, &bundle).await?;
        self.regenerate_index(&agent_id, &bundle).await?;

        let prefix = Self::bundle_prefix(&agent_id, &bundle);
        let files = self.document_store.list(&prefix).await?;
        if files.is_empty() {
            return Err(anyhow!("bundle '{bundle}' does not exist or is empty"));
        }

        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buf);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            for file in files {
                let key = format!("{prefix}/{file}");
                if let Some(bytes) = self.document_store.get(&key).await? {
                    zip.start_file(file.clone(), options)?;
                    zip.write_all(&bytes)?;
                }
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
    ) -> Result<ImportReport> {
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
            let name = file.name().to_string();
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
            return Err(anyhow!("archive does not look like a memory bundle (no .md files)"));
        }

        let mut report = ImportReport::default();
        for (name, bytes) in entries {
            let leaf = leaf_of(&name);
            if leaf == "index.md" || leaf == "log.md" || !name.ends_with(".md") {
                continue;
            }

            let path = normalize_path(&name);
            let key = Self::doc_key(&agent_id, &bundle, &path);
            if self.document_store.get(&key).await?.is_some() {
                report.skipped.push(path);
                continue;
            }

            match parse_markdown_bytes::<MemoryFrontMatter>(&bytes) {
                Ok((mut fm, content)) => {
                    fm.bundle = bundle.clone();
                    fm.agent_id = agent_id.clone();
                    fm.slug = path.clone();
                    let out = serialize_markdown(&fm, &content)?;
                    self.document_store.put(&key, out).await?;
                    self.upsert_node_from_frontmatter(&agent_id, &bundle, &path, &fm)?;
                    // Import skips existing paths, so there is never a "before" to baseline.
                    let canonical = memory_canonical(&fm.title, &fm.tags, &fm.attachments, &content)?;
                    self.record_revision(&agent_id, &bundle, &path, Some(&canonical), origin, None)?;
                    self.rewrite_edges(&agent_id, &bundle, &path, &fm.relations)?;
                    let _ = indexer
                        .add_document_index(
                            "memory".into(),
                            Self::indexer_key(&agent_id, &bundle, &path),
                            content,
                        )
                        .await;
                    report.imported.push(path);
                }
                Err(_) => report.skipped.push(path),
            }
        }

        self.recompute_broken(&agent_id).ok();
        self.regenerate_index(&agent_id, &bundle).await?;
        self.append_log(
            &agent_id,
            &bundle,
            "imported",
            &bundle,
            &format!("{} concept(s) imported, {} skipped", report.imported.len(), report.skipped.len()),
        )
        .await?;

        Ok(report)
    }

    // ---- version history (specs/006-memory-version-history) ----

    fn revision_summary(row: &MemoryRevisionRow, latest_seq: i64) -> MemoryRevisionSummary {
        MemoryRevisionSummary {
            seq: row.seq,
            deleted: row.deleted,
            actor: row.actor.clone(),
            trigger: row.trigger.clone(),
            created_at: DateTime::<Utc>::from_timestamp_millis(row.created_at)
                .unwrap_or_else(Utc::now),
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
        let current = self
            .document_store
            .get(&Self::doc_key(&agent_id, &bundle, &path))
            .await?
            .as_deref()
            .and_then(canonical_of_bytes);

        let conn = self.conn.lock();
        memory_revision::ensure_baseline(&conn, &agent_id, &bundle, &path, current.as_deref())?;
        let latest_seq = memory_revision::latest(&conn, &agent_id, &bundle, &path)?
            .map(|r| r.seq)
            .unwrap_or(0);
        let (rows, total) =
            memory_revision::list(&conn, &agent_id, &bundle, &path, offset, limit)?;
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
        let conn = self.conn.lock();
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
        let conn = self.conn.lock();
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

    pub async fn rollback_memory(
        &self,
        agent_id: String,
        bundle: Option<String>,
        path: String,
        seq: i64,
        origin: &RevisionOrigin,
        indexer: &VizierIndexer,
    ) -> Result<RollbackResponse> {
        let bundle = bundle.unwrap_or_else(default_bundle);
        let path = normalize_path(&path);
        let (row, before) = {
            let conn = self.conn.lock();
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
        // (recreated if it was deleted), the graph index, links, embedding, index.md and
        // log.md all refresh through the one write path — and history only ever grows.
        let origin = origin
            .clone()
            .with_trigger(RevisionTrigger::Rollback { restored_from: seq });
        self.write_memory(
            agent_id.clone(),
            Some(bundle.clone()),
            Some(path.clone()),
            false,
            fm.title,
            body,
            fm.tags,
            fm.attachments,
            &origin,
            indexer,
        )
        .await?;

        let after = {
            let conn = self.conn.lock();
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
        attachments: Vec<VizierAttachment>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        read_count: u64,
        indexer: &VizierIndexer,
    ) -> Result<()> {
        let path = normalize_path(&path);
        let key = Self::doc_key(&agent_id, &bundle, &path);
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
            attachment_count: attachments.len(),
            attachments,
            read_count,
        };

        let bytes = serialize_markdown(&frontmatter, &content)?;
        self.document_store.put(&key, bytes).await?;
        self.upsert_node_from_frontmatter(&agent_id, &bundle, &path, &frontmatter)?;
        let canonical = memory_canonical(
            &frontmatter.title,
            &frontmatter.tags,
            &frontmatter.attachments,
            &content,
        )?;
        self.record_revision(
            &agent_id,
            &bundle,
            &path,
            Some(&canonical),
            &RevisionOrigin::system(RevisionTrigger::Baseline),
            None,
        )?;
        self.rewrite_edges(&agent_id, &bundle, &path, &relations)?;
        let _ = indexer
            .add_document_index("memory".into(), Self::indexer_key(&agent_id, &bundle, &path), content)
            .await;

        Ok(())
    }

    /// Recomputes broken-link flags and regenerates the bundle-root index for a bundle touched
    /// by a batch of `write_migrated_memory` calls (called once per touched bundle, not per
    /// document, since a bulk migration would otherwise regenerate the index N times over).
    pub async fn finalize_bundle(&self, agent_id: &str, bundle: &str) -> Result<()> {
        self.recompute_broken(agent_id)?;
        self.regenerate_index(agent_id, bundle).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indexer::noop::NoopIndexer;
    use crate::storage::document::LocalDocumentStore;

    fn origin() -> RevisionOrigin {
        RevisionOrigin::system(RevisionTrigger::Baseline)
    }

    fn setup() -> (BundleMemoryStore, tempfile::TempDir, VizierIndexer) {
        let dir = tempfile::tempdir().unwrap();
        let doc_store: Arc<dyn DocumentStore> =
            Arc::new(LocalDocumentStore::new(dir.path().to_path_buf()));
        let conn = Connection::open_in_memory().unwrap();
        crate::storage::sqlite::init_memory_graph_schema(&conn).unwrap();
        crate::storage::sqlite::init_revision_schema(&conn).unwrap();
        let conn = Arc::new(Mutex::new(conn));
        let store = BundleMemoryStore::new(doc_store, conn);
        let indexer = VizierIndexer::build(NoopIndexer);
        (store, dir, indexer)
    }

    #[tokio::test]
    async fn collision_rejected_on_create_only() {
        let (store, _dir, indexer) = setup();
        store
            .write_memory(
                "a1".into(),
                Some("default".into()),
                Some("note".into()),
                true,
                "Note".into(),
                "hello".into(),
                vec![],
                vec![],
                &origin(),
                &indexer,
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
                vec![],
                &origin(),
                &indexer,
            )
            .await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn update_same_path_is_allowed_when_not_create_only() {
        let (store, _dir, indexer) = setup();
        store
            .write_memory(
                "a1".into(),
                None,
                Some("note".into()),
                false,
                "Note".into(),
                "v1".into(),
                vec![],
                vec![],
                &origin(),
                &indexer,
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
                vec![],
                &origin(),
                &indexer,
            )
            .await
            .unwrap();
        assert_eq!(updated.content, "v2");
    }

    #[tokio::test]
    async fn implicit_bundle_and_subdirectory_creation() {
        let (store, _dir, indexer) = setup();
        let memory = store
            .write_memory(
                "a1".into(),
                Some("andy".into()),
                Some("friends/bred".into()),
                false,
                "Bred".into(),
                "hi".into(),
                vec![],
                vec![],
                &origin(),
                &indexer,
            )
            .await
            .unwrap();
        assert_eq!(memory.bundle, "andy");
        assert_eq!(memory.slug, "friends/bred");

        let detail = store
            .get_memory_detail("a1".into(), Some("andy".into()), "friends/bred".into())
            .await
            .unwrap();
        assert!(detail.is_some());
    }

    #[tokio::test]
    async fn same_bundle_and_cross_bundle_links_resolve() {
        let (store, _dir, indexer) = setup();
        store
            .write_memory(
                "a1".into(),
                Some("default".into()),
                Some("alpha".into()),
                false,
                "Alpha".into(),
                "alpha content".into(),
                vec![],
                vec![],
                &origin(),
                &indexer,
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
                vec![],
                &origin(),
                &indexer,
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
                vec![],
                &origin(),
                &indexer,
            )
            .await
            .unwrap();

        let related = store
            .get_related_memories("a1".into(), Some("default".into()), "gamma".into())
            .await
            .unwrap();
        let slugs: Vec<String> = related.iter().map(|m| format!("{}/{}", m.bundle, m.slug)).collect();
        assert!(slugs.contains(&"default/alpha".to_string()));
        assert!(slugs.contains(&"other/beta".to_string()));
    }

    #[tokio::test]
    async fn same_bundle_link_without_md_extension_still_resolves() {
        let (store, _dir, indexer) = setup();
        store
            .write_memory(
                "a1".into(),
                Some("default".into()),
                Some("alpha".into()),
                false,
                "Alpha".into(),
                "content".into(),
                vec![],
                vec![],
                &origin(),
                &indexer,
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
                vec![],
                &origin(),
                &indexer,
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
        let (store, _dir, indexer) = setup();
        store
            .write_memory(
                "a1".into(),
                Some("default".into()),
                Some("gamma".into()),
                false,
                "Gamma".into(),
                "see [screenshot](notes.png) and [site](https://example.com/page) and [mail](mailto:a@b.com)".into(),
                vec![],
                vec![],
                &origin(),
                &indexer,
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

    #[tokio::test]
    async fn stale_relations_self_heal_on_read() {
        let (store, dir, indexer) = setup();
        store
            .write_memory(
                "a1".into(),
                Some("default".into()),
                Some("alpha".into()),
                false,
                "Alpha".into(),
                "content".into(),
                vec![],
                vec![],
                &origin(),
                &indexer,
            )
            .await
            .unwrap();

        // Simulate a document written before the "no .md extension" fix landed: the content has
        // a bare link but the stored frontmatter's `relations` was parsed under the old, stricter
        // rule and missed it entirely.
        let path = dir.path().join("a1/memory/default/gamma.md");
        let stale = "---\nslug: gamma\ntitle: Gamma\ncreated_at: 2024-01-01T00:00:00Z\nupdated_at: 2024-01-01T00:00:00Z\nagent_id: a1\nbundle: default\ntags: []\nkeywords: []\nrelations: []\nattachments: []\nattachment_count: 0\nread_count: 0\n---\nsee [Alpha](alpha) for details";
        std::fs::write(&path, stale).unwrap();

        let memory = store
            .get_memory_detail("a1".into(), Some("default".into()), "gamma".into())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(memory.relations, vec!["alpha.md".to_string()]);

        // The on-disk frontmatter itself is repaired too, not just the in-memory result.
        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert!(on_disk.contains("alpha.md"));

        let related = store
            .get_related_memories("a1".into(), Some("default".into()), "gamma".into())
            .await
            .unwrap();
        assert!(related.iter().any(|m| m.slug == "alpha"));
    }

    /// `get_related_memories` must self-heal a stale source document's own outgoing edges even
    /// when called directly — i.e. without `get_memory_detail` having been called on it first.
    #[tokio::test]
    async fn get_related_memories_self_heals_without_a_prior_detail_call() {
        let (store, dir, indexer) = setup();
        store
            .write_memory(
                "a1".into(),
                Some("default".into()),
                Some("alpha".into()),
                false,
                "Alpha".into(),
                "content".into(),
                vec![],
                vec![],
                &origin(),
                &indexer,
            )
            .await
            .unwrap();

        let path = dir.path().join("a1/memory/default/gamma.md");
        let stale = "---\nslug: gamma\ntitle: Gamma\ncreated_at: 2024-01-01T00:00:00Z\nupdated_at: 2024-01-01T00:00:00Z\nagent_id: a1\nbundle: default\ntags: []\nkeywords: []\nrelations: []\nattachments: []\nattachment_count: 0\nread_count: 0\n---\nsee [Alpha](alpha) for details";
        std::fs::write(&path, stale).unwrap();

        // Note: no get_memory_detail call on "gamma" before this.
        let related = store
            .get_related_memories("a1".into(), Some("default".into()), "gamma".into())
            .await
            .unwrap();
        assert!(related.iter().any(|m| m.slug == "alpha"));
    }

    /// Real-world failure mode: an agent writes a cross-bundle reference using ordinary
    /// same-bundle relative-link syntax, treating the whole memory tree as one shared
    /// filesystem — `[label](books/great-gatsby.md)` from *inside* the `authors` bundle,
    /// meaning "the `great-gatsby` concept in the `books` bundle." Since that literal nested
    /// path doesn't exist within `authors`, but `books` is a real bundle containing a concept
    /// at that path, it should resolve there instead of staying permanently broken.
    #[tokio::test]
    async fn cross_bundle_reference_written_as_a_same_bundle_style_path_still_resolves() {
        let (store, _dir, indexer) = setup();
        store
            .write_memory(
                "viz".into(),
                Some("books".into()),
                Some("great-gatsby".into()),
                false,
                "The Great Gatsby".into(),
                "content".into(),
                vec![],
                vec![],
                &origin(),
                &indexer,
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
                vec![],
                &origin(),
                &indexer,
            )
            .await
            .unwrap();

        let related = store
            .get_related_memories("viz".into(), Some("authors".into()), "f-scott-fitzgerald".into())
            .await
            .unwrap();
        assert!(related.iter().any(|m| m.bundle == "books" && m.slug == "great-gatsby"));

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
                vec![],
                &origin(),
                &indexer,
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
                vec![],
                &origin(),
                &indexer,
            )
            .await
            .unwrap();
        let related2 = store
            .get_related_memories("viz".into(), Some("authors".into()), "pointer".into())
            .await
            .unwrap();
        assert!(related2.iter().any(|m| m.bundle == "authors" && m.slug == "nested/real-nested-concept"));
    }

    /// A bare legacy `[[slug]]` wikilink that doesn't name an existing bundle should resolve as
    /// a same-bundle concept reference instead of staying permanently broken, per research.md §6.
    #[tokio::test]
    async fn bare_wikilink_falls_back_to_same_bundle_concept_when_no_such_bundle_exists() {
        let (store, _dir, indexer) = setup();
        store
            .write_memory(
                "a1".into(),
                Some("default".into()),
                Some("alpha".into()),
                false,
                "Alpha".into(),
                "content".into(),
                vec![],
                vec![],
                &origin(),
                &indexer,
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
                vec![],
                &origin(),
                &indexer,
            )
            .await
            .unwrap();

        let related = store
            .get_related_memories("a1".into(), Some("default".into()), "gamma".into())
            .await
            .unwrap();
        assert!(related.iter().any(|m| m.bundle == "default" && m.slug == "alpha"));
    }

    #[tokio::test]
    async fn broken_links_are_tolerated_not_errors() {
        let (store, _dir, indexer) = setup();
        store
            .write_memory(
                "a1".into(),
                None,
                Some("gamma".into()),
                false,
                "Gamma".into(),
                "points to [missing](missing.md) and [[nowhere/nothing]]".into(),
                vec![],
                vec![],
                &origin(),
                &indexer,
            )
            .await
            .unwrap();

        let related = store
            .get_related_memories("a1".into(), None, "gamma".into())
            .await
            .unwrap();
        assert!(related.is_empty());

        let graph = store.get_memory_graph("a1".into(), Some("default".into()), None).await.unwrap();
        assert!(graph.edges.iter().any(|e| e.broken));
    }

    #[tokio::test]
    async fn reconciliation_picks_up_documents_copied_from_elsewhere() {
        let dir = tempfile::tempdir().unwrap();
        let doc_store: Arc<dyn DocumentStore> =
            Arc::new(LocalDocumentStore::new(dir.path().to_path_buf()));
        let conn = Connection::open_in_memory().unwrap();
        crate::storage::sqlite::init_memory_graph_schema(&conn).unwrap();
        let conn = Arc::new(Mutex::new(conn));

        // Simulate a bundle copied in from another deployment: write the raw markdown file
        // directly, bypassing BundleMemoryStore entirely (no memory_node row exists yet).
        let raw = "---\nslug: pre-existing\ntitle: Pre-existing\ncreated_at: 2024-01-01T00:00:00Z\nupdated_at: 2024-01-01T00:00:00Z\nagent_id: a1\nbundle: default\ntags: []\nkeywords: []\nrelations: []\nattachments: []\nread_count: 0\n---\nhello from another deployment";
        doc_store
            .put("a1/memory/default/pre-existing.md", raw.as_bytes().to_vec())
            .await
            .unwrap();

        let store = BundleMemoryStore::new(doc_store, conn);
        let all = store
            .get_all_agent_memory("a1".into(), Some("default".into()))
            .await
            .unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].slug, "pre-existing");
    }

    #[tokio::test]
    async fn index_and_log_documents_reflect_bundle_contents() {
        let (store, dir, indexer) = setup();
        store
            .write_memory(
                "a1".into(),
                Some("default".into()),
                Some("alpha".into()),
                false,
                "Alpha".into(),
                "content".into(),
                vec!["x".into()],
                vec![],
                &origin(),
                &indexer,
            )
            .await
            .unwrap();

        let index_path = dir.path().join("a1/memory/default/index.md");
        let index = std::fs::read_to_string(&index_path).unwrap();
        assert!(index.contains("alpha"));
        assert!(index.contains("Alpha"));
        assert!(index.contains('x'));

        let log_path = dir.path().join("a1/memory/default/log.md");
        let log = std::fs::read_to_string(&log_path).unwrap();
        assert!(log.contains("created"));
        assert!(log.contains("alpha"));

        store
            .write_memory(
                "a1".into(),
                Some("default".into()),
                Some("alpha".into()),
                false,
                "Alpha".into(),
                "updated content".into(),
                vec![],
                vec![],
                &origin(),
                &indexer,
            )
            .await
            .unwrap();
        let log = std::fs::read_to_string(&log_path).unwrap();
        assert!(log.contains("updated"));
    }

    #[tokio::test]
    async fn delete_bundle_rejected_when_not_empty() {
        let (store, _dir, indexer) = setup();
        store
            .write_memory(
                "a1".into(),
                Some("andy".into()),
                Some("note".into()),
                false,
                "Note".into(),
                "hello".into(),
                vec![],
                vec![],
                &origin(),
                &indexer,
            )
            .await
            .unwrap();

        let err = store.delete_bundle("a1".into(), "andy".into(), false, &origin(), &indexer).await;
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
        let (store, dir, indexer) = setup();
        store
            .write_memory(
                "a1".into(),
                Some("andy".into()),
                Some("note".into()),
                false,
                "Note".into(),
                "hello".into(),
                vec![],
                vec![],
                &origin(),
                &indexer,
            )
            .await
            .unwrap();
        store
            .delete_memory("a1".into(), Some("andy".into()), "note".into(), &origin(), &indexer)
            .await
            .unwrap();

        store.delete_bundle("a1".into(), "andy".into(), false, &origin(), &indexer).await.unwrap();

        assert!(!dir.path().join("a1/memory/andy/index.md").exists());
        assert!(!dir.path().join("a1/memory/andy/log.md").exists());

        let bundles = store.list_bundles("a1".into()).await.unwrap();
        assert!(!bundles.iter().any(|b| b.name == "andy"));
    }

    #[tokio::test]
    async fn delete_bundle_with_force_removes_remaining_concepts_too() {
        let (store, dir, indexer) = setup();
        store
            .write_memory(
                "a1".into(),
                Some("andy".into()),
                Some("note".into()),
                false,
                "Note".into(),
                "hello".into(),
                vec![],
                vec![],
                &origin(),
                &indexer,
            )
            .await
            .unwrap();

        // Not forced: still rejected.
        assert!(store.delete_bundle("a1".into(), "andy".into(), false, &origin(), &indexer).await.is_err());

        // Forced: the whole bundle, concept included, is gone.
        store.delete_bundle("a1".into(), "andy".into(), true, &origin(), &indexer).await.unwrap();

        assert!(!dir.path().join("a1/memory/andy/note.md").exists());
        assert!(!dir.path().join("a1/memory/andy/index.md").exists());
        assert!(!dir.path().join("a1/memory/andy/log.md").exists());
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
                vec![],
                origin,
                indexer,
            )
            .await
            .unwrap()
    }

    fn history(store: &BundleMemoryStore) -> Vec<memory_revision::MemoryRevisionRow> {
        let conn = store.conn.lock();
        memory_revision::list(&conn, "a1", "default", "note", 0, 50)
            .unwrap()
            .0
    }

    #[tokio::test]
    async fn every_save_records_a_revision_with_its_origin_and_no_op_saves_are_skipped() {
        let (store, _dir, indexer) = setup();
        let agent = RevisionOrigin {
            actor: crate::schema::RevisionActor::Agent,
            trigger: RevisionTrigger::Conversation,
        };
        let user = RevisionOrigin {
            actor: crate::schema::RevisionActor::User {
                user_id: "u1".into(),
                username: "alice".into(),
            },
            trigger: RevisionTrigger::WebUi,
        };

        write_note(&store, &indexer, "Note", "v1", vec![], &agent).await;
        write_note(&store, &indexer, "Note", "v2", vec![], &user).await;
        // identical content, title and tags: no new revision (FR-003)
        write_note(&store, &indexer, "Note", "v2", vec![], &user).await;
        // a tag-only change is a real change
        write_note(&store, &indexer, "Note", "v2", vec!["t".into()], &agent).await;

        let rows = history(&store);
        assert_eq!(rows.iter().map(|r| r.seq).collect::<Vec<_>>(), vec![3, 2, 1]);
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
        let (store, dir, indexer) = setup();
        let agent = RevisionOrigin {
            actor: crate::schema::RevisionActor::Agent,
            trigger: RevisionTrigger::Conversation,
        };

        // A document that pre-dates history: written directly on disk, then reconciled.
        std::fs::create_dir_all(dir.path().join("a1/memory/default")).unwrap();
        let fm = MemoryFrontMatter {
            slug: "note".into(),
            title: "Old".into(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            agent_id: "a1".into(),
            bundle: "default".into(),
            tags: vec![],
            keywords: vec![],
            relations: vec![],
            attachment_count: 0,
            attachments: vec![],
            read_count: 0,
        };
        std::fs::write(
            dir.path().join("a1/memory/default/note.md"),
            serialize_markdown(&fm, "pre-history body").unwrap(),
        )
        .unwrap();
        assert!(history(&store).is_empty());

        // First tracked edit seeds seq 1 (baseline) from the on-disk content, then seq 2.
        write_note(&store, &indexer, "Old", "edited", vec![], &agent).await;
        let rows = history(&store);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].trigger, RevisionTrigger::Baseline);
        assert_eq!(rows[1].actor, crate::schema::RevisionActor::System);
        assert!(rows[1].content.as_deref().unwrap().contains("pre-history body"));

        store
            .delete_memory("a1".into(), None, "note".into(), &agent, &indexer)
            .await
            .unwrap();
        let rows = history(&store);
        assert_eq!(rows.len(), 3);
        assert!(rows[0].deleted);
        assert!(rows[0].content.is_none());
        assert!(!dir.path().join("a1/memory/default/note.md").exists());

        // Deleting the agent wipes its history.
        {
            let conn = store.conn.lock();
            memory_revision::delete_agent(&conn, "a1").unwrap();
        }
        assert!(history(&store).is_empty());
    }
}
