# Data Model: Agent Memory as Open Documents

## Document Store

The pluggable abstraction (`DocumentStore` trait, FR-023) all concept/index/log document bytes
are read and written through — never a hard-coded filesystem call inside `BundleMemoryStore`
itself. One default implementation ships with this feature:

| Implementation | Backing medium | Notes |
|---|---|---|
| `LocalDocumentStore` | Local filesystem, rooted at `{workspace}/agents/{agent_id}/memory/` | The only implementation this feature ships. A future implementation (e.g. S3, a remote filesystem) is a second `impl DocumentStore` — no change to `BundleMemoryStore`, the bundle model, or any caller. |

Interface shape (see `contracts/document-store-trait.md`): `get(path) -> bytes`,
`put(path, bytes)`, `delete(path)`, `list(prefix) -> Vec<path>`. Keys are the same `(bundle,
path)`-derived strings used throughout this data model (e.g. `andy/notes.md`,
`default/friends/bred.md`).

## Memory Graph Index

The sqlite-resident cache (FR-024) that every listing/filtering/sorting/graph/related-memory
read is served from, so those operations never call into the `DocumentStore` (SC-013). Derived
from, and reconciled against, the concept documents — never independently authoritative (a
document's content always wins on conflict; see Concept Document's Validation rules and
research.md §9's reconciliation behavior).

**`memory_node`** — one row per concept document:

| Column | Type | Notes |
|---|---|---|
| `agent_id`, `bundle`, `path` | `String` (composite PK) | Concept ID, see below. |
| `slug` | `String` | Leaf filename stem, carried through for display/back-compat. |
| `title` | `String` | Mirrors the document's frontmatter `title`. |
| `tags_json` | `String` (JSON array) | Mirrors `tags`. |
| `created_at`, `updated_at` | `DateTime<Utc>` | Mirror the document's frontmatter. |
| `read_count` | `u64` | Mirrors the document's frontmatter. |

**`memory_edge`** — one row per parsed link (both directions of a bundle-level `[[bundle]]`
reference expand to one edge per target concept when rendering the concept-level graph, but are
stored as a single whole-bundle edge):

| Column | Type | Notes |
|---|---|---|
| `agent_id` | `String` | Scopes the edge to one agent (no cross-agent edges, ever). |
| `source_bundle`, `source_path` | `String` | The linking concept. |
| `target_bundle`, `target_path` | `String` (nullable `target_path`) | `target_path` is `NULL` for a whole-bundle (`[[bundle]]`) reference. |
| `target_kind` | enum: `same_bundle` \| `cross_bundle_concept` \| `cross_bundle_bundle` | Which of the three Memory Link forms this is. |
| `broken` | `bool` | Recomputed whenever the source document is rewritten or a target's existence changes. |

## Memory Bundle

A named, hierarchical directory of concept documents owned by one agent, stored via the
`DocumentStore` under the key prefix `{agent_id}/memory/{bundle}/` (on `LocalDocumentStore`, this
is the literal directory `{workspace}/agents/{agent_id}/memory/{bundle}/`).

| Field | Type | Notes |
|---|---|---|
| `name` | `String` | Directory name; the bundle's identity. `"default"` is the implicit bundle for writes that don't name one. |
| `agent_id` | `String` | Owning agent. A bundle is never shared across agents. |
| root `index.md` | Index Document | Auto-maintained, see below. |
| `log.md` | Log Document | Auto-maintained, see below. |
| concept documents | `Vec<ConceptDocument>` | Arbitrarily nested under the bundle root. |
| attachments | non-document files | Live alongside the concept document(s) that reference them. |

**Lifecycle**: created implicitly the first time a write names it (FR-008); never auto-deleted
even if emptied of concepts (spec Assumption) — an operator/future feature may clean it up.

## Bundle Summary (the `list_bundles` output shape)

A lightweight, per-bundle row — no edges, no concept detail — read entirely from the Memory
Graph Index (`memory_node`, grouped by bundle):

| Field | Type | Notes |
|---|---|---|
| `name` | `String` | The bundle's name. |
| `concept_count` | `usize` | Count of `memory_node` rows in this bundle. |
| `updated_at` | `Option<DateTime<Utc>>` | Most recent `updated_at` across its concepts; `None` for a bundle that exists but is still empty. |

Backs three different consumers with the same shape: the `memory_list` tool's top-level view
(`bundle: None`, research.md §12), the WebUI's bundle picker, and import/export's destination-
bundle validation (FR-020/FR-021) — none of which need graph edges, just "what bundles are
there and roughly how big are they."

## Memory Concept Document

One markdown file = one memory. Frontmatter fields (YAML, via the existing `MarkdownDoc`
derive/`read_markdown`/`write_markdown` convention):

| Field | Type | Change vs. today's `Memory`/`MemoryFrontMatter` |
|---|---|---|
| `slug` | `String` | Leaf filename stem (no longer globally unique by itself — see Concept ID). |
| `title` | `String` | Unchanged. |
| `created_at` | `DateTime<Utc>` | **New.** Set once, on first write. |
| `updated_at` | `DateTime<Utc>` | **Renamed from `timestamp`.** Set on every write. FR-005 asks for both creation *and* update timestamps; today there is only one (`timestamp`, which today is actually overwritten as "updated" on every rewrite). |
| `agent_id` | `String` | Unchanged — owning agent. |
| `tags` | `Vec<String>` | Unchanged. |
| `keywords` | `Vec<String>` | Unchanged (currently always empty; out of scope to populate). |
| `relations` | `Vec<String>` | **Reinterpreted.** Now a list of resolved link targets in canonical form: `path/to/concept.md` for same-bundle, `bundle/slug` or `bundle` for cross-bundle. Parsed from content on every write (as today), using the two link forms below instead of one. |
| `attachments` | `Vec<VizierAttachment>` | Unchanged. |
| `read_count` | `u64` | Unchanged. |
| ~~`visibility`~~ | — | **Removed** (FR-015 / spec clarification: no more private/global/shared). |
| ~~`shared_to`~~ | — | **Removed.** |

**Concept ID**: `(bundle: String, path: String)` — `path` is the file's path relative to the
bundle root, without extension segments collapsed (e.g. `friends/bred`). This, not `slug` alone,
is the addressable/collision-checked identity (FR-011).

**Validation rules**:
- Write rejected if `(bundle, path)` already exists and the write is not an explicit update of
  that same document (FR-011) — no auto-rename.
- Writing to a nested path whose parent directories don't exist creates them (FR-007).
- Malformed frontmatter on read is surfaced as a clear error, not a crash or silent data loss
  (Edge Cases) — mirrors today's `VizierError` propagation from `read_markdown`.

## Index Document

Reserved file `index.md` at a bundle's root, and optionally within any subdirectory. Not
user-authored — regenerated by `BundleMemoryStore` on every write/update/delete at that level,
and reconciled lazily on read if stale (see research.md §5). Content: a listing (table or list)
of the concept documents at that directory level — path, title, tags, `updated_at`.

## Log Document

Reserved file `log.md` at a bundle's root only. Append-only chronological history of that
bundle's memory writes/updates (FR-018): timestamp, action (`created` / `updated` / `deleted`),
concept path, title. Regenerated (appended-to) as part of every write/update/delete in that
bundle.

## Memory Link

A reference from one concept document's content to another concept or to a whole bundle.

| Form | Syntax | Resolves to |
|---|---|---|
| Same-bundle, specific concept | `[label](path/to/concept.md)` — standard markdown relative link | Another concept document in the *same* bundle, at that relative path. Natively clickable outside Vizier (FR-004, SC-011). |
| Cross-bundle, specific concept | `[[bundle/slug]]` | A specific concept document in a *different* bundle owned by the same agent (FR-013). |
| Cross-bundle, whole bundle | `[[bundle]]` | That bundle's root index document (FR-013). |

A link whose target doesn't currently exist (moved, renamed, deleted, or a nonexistent bundle)
is a **broken link**, not an error — degrades gracefully in related-memory lookups and is flagged
in the graph edge (`broken: bool`, already present on `MemoryGraphEdge`) (Edge Cases).

## Memory Graph (the `get_memory_graph`/`memory_graph` output shape)

One type serves both levels of FR-019's graph (WebUI and the `memory_graph` tool alike) — no
separate bundle-level type. Field meaning differs by level; see
`contracts/memory-storage-trait.md`'s "`get_memory_graph` output shape, concretely" for the full
walkthrough. Schema changes from today's `MemoryGraphNode`/`MemoryGraphEdge`:

| Field | Type | Change |
|---|---|---|
| `MemoryGraphNode.slug` | `String` | Unchanged field name. Bundle-level: the bundle's name. Concept-level: the concept's path within the open bundle, or (for a boundary node) the *other* bundle's name. |
| `MemoryGraphNode.bundle` | `String` | **New.** Bundle-level: same as `slug` (a bundle belongs to itself). Concept-level: the open bundle's name for a normal node, or the referenced bundle's name for a boundary node. |
| `MemoryGraphNode.title` | `String` | Unchanged field name. Bundle-level: the bundle's name (no separate title exists). Concept-level: the concept's frontmatter title, or the referenced bundle's name for a boundary node. |
| `MemoryGraphNode.tags` | `Vec<String>` | Unchanged field name. Bundle-level and boundary nodes: always `[]`. |
| `MemoryGraphNode.agent_id` | `String` | Unchanged. |
| `MemoryGraphNode.boundary` | `bool` | **New.** `true` only for a concept-level node standing in for a link that crosses out of the open bundle (Edge Cases' "boundary indicator" requirement) — never `true` at the bundle level. |
| ~~`MemoryGraphNode.visibility`~~ | — | **Removed** (FR-015). |
| `MemoryGraphEdge.{source,target,broken}` | unchanged shape | Bundle-level: connect bundle names, deduplicated per bundle-pair. Concept-level: connect concept paths for same-bundle links, or a concept path to its target's *boundary* node for cross-bundle links (both `[[bundle/slug]]` and `[[bundle]]` collapse to one edge into that bundle's boundary node at this zoom level — resolving to a specific cross-bundle concept is `get_related_memories`'/`memory_follow`'s job, not the graph's). |
| `MemoryGraph.initial_slugs` | `Vec<String>` | Unchanged — `compute_initial_slugs` runs unmodified at either level, since a bundle/boundary node's `title`/`tags` already mirror its `slug`. |

## Attachment

Unchanged shape (`VizierAttachment`) — a non-document file stored within a bundle, associated
with the concept document that references it, so it travels with the bundle on copy/export.

## Agent

Owner of one or more Memory Bundles; the sole reader/writer of every concept document within
them. No cross-agent visibility of any kind (spec Assumption; drops the old
`MemoryVisibility`/`shared_to` model entirely).

## On-disk layout (as materialized by `LocalDocumentStore`, the default `DocumentStore` impl)

```text
{workspace}/agents/{agent_id}/memory/
├── default/                  # implicit bundle for un-named writes
│   ├── index.md
│   ├── log.md
│   ├── project-architecture.md
│   └── friends/
│       ├── index.md          # optional per-subdirectory index
│       └── bred.md
└── andy/                     # a named bundle
    ├── index.md
    ├── log.md
    └── notes.md
```

## Storage trait shape (Phase 1 contract, see contracts/)

`MemoryStorage` methods gain a `bundle: Option<String>` parameter and address concepts by
`(bundle, path)` instead of bare `slug`. `None` is *not* one universal default: single-concept
methods (write/read/detail/delete/follow) treat it as the agent's default bundle, while scanning
methods (list/query) treat it as "every bundle" — preserving today's single-collection call
sites exactly (FR-001) — and `get_memory_graph`'s `None` means the bundle-level view. See
`contracts/memory-tools.md`'s table and research.md §11 for the full per-method breakdown and
why this distinction matters. The single `impl MemoryStorage for SqliteStorage`
(`BundleMemoryStore`) is the only code that touches both the `DocumentStore` (document bytes) and
the Memory Graph Index (`memory_node`/`memory_edge` rows) — no other caller reaches either
directly. See `contracts/memory-storage-trait.md` and `contracts/document-store-trait.md`.
