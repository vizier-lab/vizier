# Contract: `MemoryStorage` trait

Location: `src/storage/memory.rs` (trait), implemented exactly once — `impl MemoryStorage for
SqliteStorage` in `src/storage/sqlite/memory.rs` — by delegating every method to the new shared
`BundleMemoryStore` (research.md §1). There is no `FileSystemStorage` implementation: the
`filesystem` `VizierStorageProvider` backend is removed entirely (research.md §10), so this is
not a multi-backend trait needing per-backend delegation anymore. `BundleMemoryStore` itself is
backend-agnostic in a different sense — it's generic over `DocumentStore` (document bytes,
`contracts/document-store-trait.md`) plus the sqlite connection it uses for the Memory Graph
Index (data-model.md's `memory_node`/`memory_edge` tables).

## Signature changes from today

Every method that identifies a memory by `agent_id` (+ `slug`) gains a `bundle: Option<String>`
parameter, positioned right after `agent_id`. `slug: Option<String>` on write and `slug: String`
on read/delete/etc. are reinterpreted as a `path` that may be multi-segment (e.g.
`"friends/bred"`), per data-model.md's Concept ID. The parameter name in code may stay
`slug`/`path` — the contract is the *semantics* (nested, bundle-scoped), not the identifier's
name.

**`bundle: None` is not one universal default at this layer either — it depends on whether the
method addresses a single concept or scans a collection.** Methods that identify *one* concept
(`write_memory`, `get_memory_detail`, `get_related_memories`, `has_incoming_links`,
`delete_memory`, `increment_read_count`) treat `None` as the agent's default bundle — the same
resolution rule on read and write, so an unqualified `path` always means "the thing in my
default bundle." Methods that scan *across* concepts (`query_memory`, `get_all_agent_memory`)
treat `None` as "every bundle" — this is the *broadest, internal* meaning these two keep
regardless of what any tool presents as its own default, because both are relied on internally
for things that must never accidentally miss a bundle: `query_memory`'s reranking step fetches
every memory via `get_all_agent_memory` to rank candidates against, and migration/graph-index
reconciliation need "truly everything" too. `get_memory_graph`'s `None` is a third shape again —
not "everything flattened" but "the bundle-level view" (bundles as nodes) — because a flattened
cross-bundle concept graph isn't what FR-019 asks for; naming a `bundle` there descends into that
bundle's concept-level graph instead.

**This storage-layer default is independent of what a tool presents as its own default** — see
research.md §12: the `memory_list` *tool*'s `bundle: None` means "show me the top-level bundle
view" (mirroring `memory_graph`), which it serves from `list_bundles`, not from calling
`get_all_agent_memory(None)`. A tool is free to map its own `bundle: None` onto whichever storage
call actually answers what the tool is presenting — it does not have to forward `bundle`
unchanged into a same-named storage parameter. See `contracts/memory-tools.md`'s table and
Exploration recipe for the full per-tool breakdown an agent actually sees.

```rust
#[async_trait::async_trait]
pub trait MemoryStorage {
    async fn write_memory(
        &self,
        agent_id: String,
        bundle: Option<String>,
        path: Option<String>,      // was `slug: Option<String>`
        title: String,
        content: String,
        tags: Vec<String>,
        attachments: Vec<VizierAttachment>,
        indexer: &VizierIndexer,
    ) -> Result<Memory>;           // Err on (bundle, path) collision — FR-011, no auto-rename

    async fn query_memory(
        &self,
        agent_id: String,
        bundle: Option<String>,    // None = search across all of the agent's bundles
        query: String,
        limit: usize,
        threshold: f64,
        indexer: &VizierIndexer,
    ) -> Result<Vec<Memory>>;

    async fn get_all_agent_memory(
        &self,
        agent_id: String,
        bundle: Option<String>,    // None = all bundles
    ) -> Result<Vec<Memory>>;

    async fn get_filtered_memories(&self, params: MemoryQueryParams) -> Result<PaginatedMemory>;
    // MemoryQueryParams gains `bundle: Option<String>`; drops `visibility` (removed, FR-015).

    async fn get_memory_detail(
        &self,
        agent_id: String,
        bundle: Option<String>,    // None = the agent's default bundle, same rule as write_memory
        path: String,
    ) -> Result<Option<Memory>>;

    async fn get_related_memories(
        &self,
        agent_id: String,
        bundle: Option<String>,    // None = the agent's default bundle
        path: String,
    ) -> Result<Vec<Memory>>;      // resolves same-bundle + cross-bundle links (FR-003, FR-013)

    async fn get_memory_graph(
        &self,
        agent_id: String,
        bundle: Option<String>,    // None = bundle-level graph (bundles as nodes)
        search: Option<String>,
    ) -> Result<MemoryGraph>;

    async fn has_incoming_links(
        &self,
        agent_id: String,
        bundle: Option<String>,    // None = the agent's default bundle
        path: String,
    ) -> Result<bool>;

    async fn delete_memory(
        &self,
        agent_id: String,
        bundle: Option<String>,    // None = the agent's default bundle
        path: String,
        indexer: &VizierIndexer,
    ) -> Result<()>;

    async fn increment_read_count(
        &self,
        agent_id: String,
        bundle: Option<String>,    // None = the agent's default bundle
        path: String,
    ) -> Result<()>;

    // New: bundle-level operations backing FR-006/FR-008/FR-020/FR-021, and the tool-layer
    // top-level view for memory_list/memory_graph (research.md §12).
    // (Bundle-level *graph* is NOT a separate method — get_memory_graph(agent_id, None, search)
    // already returns it; a distinct get_bundle_graph would just be a second way to ask the same
    // question. list_bundles returns richer summaries, not a graph — no edges, but enough to
    // render a flat top-level listing: used by memory_list's top-level view, the WebUI's bundle
    // picker, and import/export to validate/offer a destination bundle name.)
    async fn list_bundles(&self, agent_id: String) -> Result<Vec<BundleSummary>>;
    // BundleSummary { name: String, concept_count: usize, updated_at: Option<DateTime<Utc>> }
    // (data-model.md's Bundle Summary entity) — updated_at is None for a freshly-created, still-
    // empty bundle.
    // Deletes a bundle (its index.md/log.md). `force: false` (the memory_delete_bundle agent
    // tool always passes this): Err if it still holds any concept document — never a silent/
    // cascading delete of content, mirroring delete_memory's own refuse-if-linked pattern.
    // `force: true` (WebUI/HTTP operator path only, behind a confirmation modal — the agent
    // tool never sets it): every remaining concept document is deleted too (and its embedding
    // removed via `indexer`) before the bundle itself goes.
    async fn delete_bundle(
        &self,
        agent_id: String,
        bundle: String,
        force: bool,
        indexer: &VizierIndexer,
    ) -> Result<()>;
    async fn export_bundle(&self, agent_id: String, bundle: String) -> Result<Vec<u8>>; // zip bytes
    async fn import_bundle(
        &self,
        agent_id: String,
        bundle: String,           // destination bundle name, chosen by the caller
        zip_bytes: Vec<u8>,
        indexer: &VizierIndexer,
    ) -> Result<ImportReport>;    // per-concept skipped-on-collision report, FR-021
}
```

## `get_memory_graph` output shape, concretely

Both levels return the *same* `MemoryGraph { nodes: Vec<MemoryGraphNode>, edges:
Vec<MemoryGraphEdge>, initial_slugs: Vec<String> }` (data-model.md defines the revised node/edge
fields — `visibility` is removed, `bundle` and `boundary` are added). What differs is what a
node's `slug`/`bundle`/`title`/`tags` *mean*, and what an edge connects:

**`bundle: None`** (bundle-level graph):
- `nodes`: one per bundle the agent owns. `slug` = the bundle's name (its only identity at this
  level — reused as `slug` so `MemoryGraph.tsx` needs no changes to key nodes). `bundle` = the
  same name (a bundle "belongs to" itself). `title` = the bundle's name again (bundles have no
  separate title). `tags` = `[]` (bundles aren't tagged). `boundary` = `false` always.
- `edges`: one per distinct **bundle pair** with at least one cross-bundle reference between
  them — `source`/`target` are bundle names, deduplicated (ten concepts in bundle A all linking
  into bundle B still produce one edge, not ten). `broken` = `true` if `target` names a bundle
  that no longer exists.
- `initial_slugs`: today's `compute_initial_slugs` unchanged — since a bundle node's
  `slug`/`title` are both its name and `tags` is empty, search-filtering "by title, slug, or
  tags" degenerates to "by bundle name," which is exactly right at this level.

**`bundle: Some(name)`** (concept-level graph, scoped to `name`):
- `nodes`: one per concept document in `name`, flattened across any nesting — `slug` = the
  concept's path within the bundle (e.g. `friends/bred`), `bundle` = `name`, `title`/`tags` from
  its frontmatter, `boundary` = `false`. **Plus** one synthetic node per *other* bundle that at
  least one concept in `name` links out to — `slug`/`title` = that other bundle's name, `bundle`
  = that other bundle's name, `tags` = `[]`, `boundary` = `true`. This is the Edge Cases'
  required "boundary indicator" — an outward link is never dropped from the response, and the
  frontend can style `boundary: true` nodes distinctly (and route a click on one to that bundle's
  own concept-level graph, mirroring the bundle-level view's node-click behavior).
- `edges`: same-bundle links between two concept nodes (`source`/`target` = their paths,
  `broken` = `true` if the target path doesn't exist in `name`); plus one edge per cross-bundle
  reference, from the linking concept's `slug` to its target bundle's *boundary node* `slug`
  (both `[[bundle/slug]]` and bare `[[bundle]]` collapse to "this concept points out to bundle X"
  at this zoom level — the concept-level graph doesn't resolve *which* concept in the other
  bundle, only that the link leaves `name`; `memory_follow` is what actually resolves a specific
  cross-bundle concept target). `broken` = `true` if the target bundle doesn't exist at all.
- `initial_slugs`: unchanged `compute_initial_slugs` logic, run over this bundle's nodes
  (boundary nodes included, so a search matching another bundle's name can surface the pointer
  to it).

A collision between a concept's path and an external bundle's name (both would appear as a
`slug` in the same concept-level response) is possible only if a bundle happens to be named
exactly like a path inside a different bundle — accepted as a rare, cosmetic edge case (the two
nodes would render as one in the frontend's dedup-by-slug lookup) rather than solved with a
synthetic prefixing scheme, consistent with Principle I.

## Behavioral contracts (unit-testable, independent of storage backend)

1. **Collision rejection** — writing to an existing `(bundle, path)` without it being an
   explicit update of that same document returns `Err`, never silently overwrites or renames.
   Same `path` in a *different* bundle, or a different nested `path` in the same bundle, is not
   a collision (FR-011, Edge Cases).
2. **Implicit bundle/subdirectory creation** — a write naming a bundle or nested path that
   doesn't exist creates every missing directory, with no separate creation call (FR-007/FR-008).
3. **Link resolution parity** — `get_related_memories` returns a same-bundle markdown-link
   target exactly as it returns a cross-bundle `[[bundle/slug]]` or `[[bundle]]` target (FR-003,
   FR-013); a `[[bundle]]` reference resolves to *every* concept in that bundle (its index),
   not to a single node. This is deliberately more resolved than `get_memory_graph`'s
   concept-level view, which stops at a *boundary* node for any cross-bundle link (see above) —
   `get_related_memories`/`memory_follow` answer "what does this actually point to," while the
   concept-level graph answers "does anything here point outside this bundle, and where."
4. **Broken link tolerance** — a link to a missing concept, moved path, or nonexistent bundle
   does not error; it's simply absent from `get_related_memories` and marked `broken: true` on
   the corresponding `MemoryGraphEdge`.
5. **Read tolerance of external edits** — a concept document edited or deleted on disk between
   calls is reflected (or its absence reflected) on the next read; malformed frontmatter
   surfaces a `VizierError`, not a panic (Edge Cases).
6. **Index/log staleness reconciliation** — any directory-listing read regenerates that level's
   `index.md` in place if it disagrees with the concept documents actually present (research.md
   §5, FR-017); `log.md` is append-only and never rewritten wholesale except during migration.
7. **Graph-index-only reads** — `get_all_agent_memory`, `get_filtered_memories`,
   `get_related_memories`, `get_memory_graph` (at either level — `bundle: None` or
   `Some(name)`), and `has_incoming_links` are answered entirely from `memory_node`/`memory_edge`
   (data-model.md, research.md §9) and never
   call `DocumentStore::get`/`list` on their common path — only `get_memory_detail` (and the
   reconciliation fallback below) read a document body. If the document set found via
   `DocumentStore::list` for a given bundle/level disagrees with `memory_node` rows at that
   level, the affected documents are re-read once and the cache corrected before the result is
   returned (FR-024, Edge Cases).
