# Research: Agent Memory as Open Documents

## 1. A pluggable `DocumentStore` trait for document bytes, backed by sqlite for the graph/listing cache

**Decision**: Introduce a new trait, `DocumentStore` (new module `src/storage/document/mod.rs`),
with `get`/`put`/`delete`/`list` operations over raw bytes keyed by a path-like string, plus a
default implementation `LocalDocumentStore` backing it directly onto a directory under the
workspace (`std::fs`/`tokio::fs`, `PathBuf` joins throughout). A new shared component
`BundleMemoryStore` (new module `src/storage/memory_bundle.rs`) implements bundle/concept-
document read, write, link resolution, index/log maintenance, and graph construction on top of
*any* `DocumentStore`, plus a `SqliteStorage`-held connection for the Memory Graph Index (§9
below). `BundleMemoryStore` becomes the single `impl MemoryStorage for SqliteStorage` (see §10 —
`FileSystemStorage` is removed, not given its own delegate).

**Rationale**:
- *Pluggable backend, not hard-coded filesystem*: the spec now explicitly asks for the ability
  to substitute a different storage medium (S3, a remote filesystem) for memory documents later
  without touching the bundle model or any caller (FR-023). A trait + one default impl is the
  minimal shape that satisfies this — exactly Principle II's "behavior that varies by kind MUST
  be expressed as one implementation behind a shared trait," applied to storage medium instead of
  `VizierStorageProvider` backend choice.
- *There's already a precedent for this split in the codebase*: `FileManager`
  (`src/file_manager/mod.rs`) already stores uploaded attachment bytes on the local filesystem
  under the workspace *regardless* of whether `--storage filesystem` or `--storage sqlite` is
  configured, while `SessionFileStorage` (both backends) only ever persists a small JSON
  *metadata record* (filename, mime type, size, a `file_id` pointing at what `FileManager`
  holds) through whichever `VizierStorageProvider` is active. `DocumentStore` generalizes that
  same "raw bytes live in one filesystem-shaped place, structured metadata lives in the
  configured backend" split — just behind a trait instead of a single hard-coded struct, and
  applied to memory concept documents instead of uploaded files.
- *Documents remain human-readable and portable regardless of backend*: this is what makes memory
  "documents you can open in a text editor" (Story 2) and "copyable between deployments with
  `cp`" (Story 3) true unconditionally, once the sole surviving `VizierStorageProvider` is
  sqlite (§9) — `LocalDocumentStore`'s files are real files on disk either way.

**Alternatives considered**:
- *Keep two separate document-producing `MemoryStorage` impls, one per `VizierStorageProvider`
  backend* — this was the Phase-0-draft-1 decision; superseded once the fs backend itself is
  being removed (§10), which makes a second impl moot rather than merely duplicated.
- *Store bundles as blobs in sqlite, materialize to temp files only for zip export* — rejected:
  satisfies Story 4 (WebUI export) but not Stories 2/3, which need a real file to exist on disk
  as a day-to-day property, not just at export time.
- *Hard-code memory documents onto `std::fs` with no trait* — rejected: fails FR-023 outright: a
  future S3/remote implementation would require rewriting `BundleMemoryStore` itself instead of
  adding a second `impl DocumentStore`.

## 2. Reuse existing frontmatter/markdown mechanics as-is

**Decision**: Keep using `utils::markdown::{read_markdown, write_markdown}` and the existing
`#[derive(MarkdownDoc)]` macro (`vizier-derive`) for concept documents, index documents, and log
documents. No new frontmatter/markdown-parsing crate.

**Rationale**: `Memory` and `Skill` already use this exact convention (YAML frontmatter between
`---` fences + markdown body), and it's the same style the spec's own Assumptions section points
to for CORE.md. Principle I (Lean by Default) forbids introducing an alternative frontmatter
library (e.g. `gray_matter`) to re-solve an already-solved, already-shipped problem.

## 3. Directory/subdirectory traversal

**Decision**: Use the already-present `glob` crate with recursive `**/*.md` patterns for
whole-bundle scans (index rebuild, migration, `get_all_agent_memory`), and direct `PathBuf`
joins for addressed reads by `(bundle, path)`. No new dependency.

**Rationale**: `glob` 0.3 supports `**` recursive matching and is already a dependency, already
used today in `src/storage/fs/memory.rs`'s `build_glob_path` calls. A `walkdir`-style crate would
be a second, redundant way to do the same thing (Principle I; Distribution constraint against
adding a second crate that overlaps an existing one).

## 4. Zip export/import needs a new dependency

**Decision**: Add the `zip` crate (read + write, deflate) scoped to the HTTP channel's bundle
export/import handlers.

**Rationale**: Nothing in the current dependency tree produces or reads `.zip` — `tar` + `bzip2`
produce `.tar.bz2`, a different format the spec does not ask for. FR-020/FR-021 and SC-010
explicitly require a `.zip` an operator can download/upload through the WebUI and expect any
standard OS zip tool to open; this is the concrete, spec-mandated need Principle I requires
before adding a crate.

**Alternatives considered**: Reuse `tar`+`bzip2` and simply name the output file `.zip` — rejected,
the contents wouldn't be a real zip archive, breaking "no filesystem/CLI access required" the
moment an operator tries to open it outside Vizier.

## 5. Index and log documents are reconciled lazily, not watched

**Decision**: `BundleMemoryStore` regenerates a bundle-root `index.md` (and, if that
subdirectory has ever had a write, a per-subdirectory `index.md`) and the bundle-root `log.md`
synchronously as part of every write/update/delete. On any read path that lists a
directory level (index open, graph build, `get_all_agent_memory`), if the concept documents
actually present on disk don't match what an index claims, the index is regenerated in place
before the result is returned.

**Rationale**: Matches FR-017/FR-018 and the spec's explicit Assumption that reconciliation is
"lazy, on the next relevant read/query/write... rather than through continuous real-time
filesystem watching" — i.e., no new file-watcher dependency or background task (Principle I,
Principle III: no new required runtime process).

## 6. Concept addressing and link syntax

**Decision**: A concept's identity becomes `(bundle, path)` where `path` may be multi-segment
(e.g. `friends/bred.md`); `slug` remains the leaf filename stem, kept in frontmatter for
continuity with today's field but no longer sufficient on its own to address a memory. Same-
bundle links are parsed as standard markdown relative-path links (`[label](path/to/concept.md)`)
via a new regex parallel to today's `parse_wikilinks`. Cross-bundle references keep the existing
`[[...]]` wikilink regex, but the captured group is now parsed as `bundle/slug` (specific
concept) or bare `bundle` (whole-bundle reference), per FR-004/FR-013. A legacy bare `[[slug]]`
written before this change is not rewritten (FR-015) and is reinterpreted under the new rule:
a whole-bundle reference if `slug` matches a bundle name, a broken link otherwise (Edge Cases).

**Rationale**: Directly encodes the spec's clarified answers with the smallest change to the
existing link-extraction approach (one more regex alongside the one already there), rather than
a general-purpose markdown AST parser.

**Update (found in practice)**: agents frequently write a same-bundle link without the `.md`
extension (`[label](path/to/concept)`). Since every concept document is always a `.md` file by
construction, there's no real ambiguity — `path/to/concept` and `path/to/concept.md` are treated
identically, normalized to the canonical `.md`-suffixed form for storage in `relations` and
`memory_edge`. Only a URL, a `mailto:` link, a bare `#anchor`, or a relative path ending in some
*other* extension (an attachment, an image) is excluded from this same-bundle-link matching.
Tool descriptions still teach the `.md` form as canonical — this is tolerance for what agents
actually write, not a second blessed syntax.

Relatedly, `get_memory_detail` and `get_related_memories` always re-derive a document's
`relations` from its actual current content on read, rather than trusting whatever was last
cached or written to the on-disk frontmatter — repairing the frontmatter in place (relations
only, not `created_at`/`updated_at`) when it disagrees. This makes a link-parsing fix like this
one (or any future one) self-healing for documents written before it shipped, the next time each
is individually opened or followed, without a dedicated migration. It does **not** retroactively
fix a bundle's cached graph/listing view for documents nobody has touched yet — those stay
governed by `memory_node`/`memory_edge` (research.md §9) until something reads that specific
document.

**Update 2 (found in practice)**: `classify_relation`'s syntactic split (same-bundle vs.
cross-bundle) can't distinguish "a nested path within my own bundle" from "a different bundle
name that happens to look like a path segment," because both are written with identical
relative-link syntax. In practice, agents very commonly write a cross-bundle reference as if the
whole memory tree were one shared filesystem — `[label](books/great-gatsby.md)` from *inside* a
different bundle (e.g. `authors`), meaning "the `great-gatsby` concept in the `books` bundle,"
not a nested path inside the source's own bundle. `rewrite_edges` now resolves each relation with
existence checks against `memory_node` rather than trusting the syntactic classification alone:
it tries the literal interpretation first (so a real nested same-bundle path keeps working
exactly as before — never overridden once it actually resolves), and only reinterprets as
cross-bundle when the literal target doesn't exist *and* the reinterpreted one does. The
symmetric fallback also applies to a bare `[[slug]]` legacy wikilink that doesn't name an
existing bundle, per this section's original "reinterpreted... a broken link otherwise" note —
it now additionally tries a same-bundle concept match before giving up. Because this resolution
happens fresh on every `rewrite_edges` call (every write, and via the self-healing read path
above), an edge that was unresolvable when first written self-heals automatically once the
bundle/concept it was actually pointing at comes to exist — no migration needed here either.

## 7. Migration: row/flat-file memory into bundles, and `filesystem`-backend deployments onto sqlite

**Decision**: Add one more one-time startup migration in `dependencies.rs`, alongside the
existing seed-users / YAML-providers / per-agent MCP-shell-config / CORE.md-backfill migrations,
gated by a version/marker check. It has two parts, both of which must run for a deployment that
was previously on the `filesystem` backend (FR-025):

- **Memory → bundles** (every deployment): read every existing memory — from the legacy sqlite
  `memory` table, and/or from a `filesystem`-backend deployment's flat `agents/{id}/memory/*.md`
  files — and write each as a concept document into that memory's owning agent's **default**
  bundle via `BundleMemoryStore` (which writes through `LocalDocumentStore` and populates the
  Memory Graph Index, §9). `visibility`/`shared_to` are dropped from the frontmatter on rewrite
  (FR-015); a memory that was `global` or `shared` is written once into its `agent_id`-owning
  agent's default bundle as private — not fanned out to every agent it was previously visible to.
  Embedded wikilink syntax inside existing memory content is **not** rewritten (FR-015, Edge
  Cases) — an accepted breaking change to link resolution, not a content-mutation step.
- **Other entities → sqlite** (`filesystem`-backend deployments only, FR-025): every other
  `VizierStorageProvider` entity a `filesystem`-backend deployment holds (agents, tasks,
  sessions, users, providers, global config, dream journal, dream state, session file records)
  is read via `FileSystemStorage`'s existing trait impls and re-written through the equivalent
  `SqliteStorage` trait impls — which already exist today and need no new code, since both
  backends already implement every `VizierStorageProvider` trait. Only the *reading* side
  (`FileSystemStorage`) is removed from the codebase after this migration ships; nothing new is
  built to receive the data.
- The legacy sqlite `memory` table is left in place but unused after migration (a rollback
  option, not a required cleanup — no FR asks for the table to be dropped).

**Rationale**: Matches FR-014/FR-015/FR-025 and reuses the exact migration pattern this codebase
already has three examples of in `dependencies.rs::migrate_*`. Because `SqliteStorage` already
implements every non-memory trait, this migration's "other entities" half is a data-copy loop
over already-existing read/write methods, not new storage logic.

## 8. WebUI two-level graph reuses the existing component; one `get_memory_graph`, not two methods

**Decision**: `get_memory_graph(agent_id, bundle: Option<String>, search)` serves *both* levels —
there is no separate bundle-level-graph method (an earlier draft's `get_bundle_graph` was
removed as redundant, see `contracts/memory-storage-trait.md`). `bundle: None` returns
bundle-as-node/cross-bundle-link-as-edge; `bundle: Some(name)` returns `name`'s concepts as
nodes plus one synthetic **boundary node** per other bundle it links out to (Edge Cases'
"indicate the outward link" requirement), never a bare dangling edge. Full field-level shape is
in `contracts/memory-storage-trait.md` and data-model.md's "Memory Graph" entity. The WebUI's
`GET /bundles/graph` and `GET /{bundle}/graph` routes are both this same call, bundle filled in
or not (`contracts/http-api.md`). Reuse `webui/app/components/MemoryGraph.tsx` unmodified for
rendering both levels; add a thin page-level wrapper that tracks "currently open bundle" (or
none, for the top level) and swaps which endpoint it fetches from — a click on a `boundary: true`
node opens *that* bundle's concept-level graph, the same action as clicking a bundle node from
the top level.

**Rationale**: `MemoryGraph.tsx` is already schema-driven off `slug`/`title`/`tags`/`initial_slugs`
and has no single-collection assumption baked into its rendering — it's generic force-directed
graph rendering over whatever `MemoryGraph` payload it's given. Adding `bundle`/`boundary` fields
to the existing node type (rather than inventing a second graph type for the bundle level) means
the frontend component needs zero new prop-shape handling — a boundary node is just a node with
one extra flag it can choose to style differently. No new charting/graph library or component
rewrite is needed (Principle I). Collapsing to one storage method instead of two
(`get_memory_graph` alone, no `get_bundle_graph`) is the same Principle II reasoning as
research.md §11's "no new bundle-listing tool" — `bundle: None` on the one method that already
exists answers the question a second method would only duplicate.

## 9. Memory Graph Index: what sqlite caches, and how it stays honest

**Decision**: Add two new sqlite tables (replacing the current flat `memory` table), owned by
`BundleMemoryStore`, never touched directly by any other code:

- `memory_node(agent_id, bundle, path, slug, title, tags_json, created_at, updated_at,
  read_count, PRIMARY KEY(agent_id, bundle, path))` — one row per concept document, holding
  everything `get_filtered_memories`, `get_all_agent_memory`, and graph-node rendering need
  without touching the `DocumentStore`.
- `memory_edge(agent_id, source_bundle, source_path, target_bundle, target_path, target_kind,
  broken)` — one row per parsed link (`target_kind` distinguishes a same-bundle concept link, a
  cross-bundle concept link, and a cross-bundle whole-bundle link); `broken` is recomputed
  whenever the source or a target's row changes.

Every `BundleMemoryStore` write path (write/update/delete) does both: (a) write/update/delete the
document via `DocumentStore`, then (b) upsert/delete the corresponding `memory_node` row and
recompute that document's outgoing `memory_edge` rows, in the same call. Every *read* path that
serves listing, filtering, sorting, related-memory, or graph queries reads `memory_node`/
`memory_edge` only — it does not call into `DocumentStore` at all, satisfying SC-013. The only
paths that read a document's full body through `DocumentStore` are: `query_memory` (semantic
search needs the actual text to match and return — backs the `memory_read` tool),
`get_memory_detail` (the agent/operator asked to see one concept's full content — backs
`memory_detail`), export, and the reconciliation path below.

**Reconciliation** (Edge Cases, FR-024): a document can still be edited or deleted directly on
disk out-of-band (Story 2, FR-012), which the cache doesn't observe. Rather than a filesystem
watcher (rejected — see §5's reasoning, which applies identically here), reconciliation is
triggered the same way index/log reconciliation is: on the *listing* read path for a given
bundle/subdirectory (index open, graph build, `get_all_agent_memory`), the actual document set
present in the `DocumentStore` for that directory level is diffed against `memory_node` rows for
that level; any mismatch (added, removed, or content-changed document) is re-read once through
`DocumentStore` and the cache is corrected before the result is returned. This keeps the common
path (cache-only reads) fast while still making FR-012/FR-024's "no stale data indefinitely"
guarantee hold.

**Rationale**: The user's explicit direction was that the database should cache the *full*
listing metadata (not just bare edges), so tag filtering, sorting by title/timestamp, and
pagination (`get_filtered_memories`) — none of which need document content — never touch the
`DocumentStore`. This matters most once a future non-local `DocumentStore` implementation (S3,
remote fs) is in play, where a per-document network round-trip on every list/filter call would
be the dominant cost; the local-filesystem default doesn't strictly need this for SC-002/SC-013
at 5,000 documents, but building the cache now avoids re-deriving it later when a remote backend
actually lands (FR-023's whole premise is that a remote backend is expected eventually).

**Alternatives considered**:
- *Edges + minimal node identity only, re-read documents for listing metadata* — the leaner
  option, rejected in favor of the fuller cache once the explicit goal is a swappable, possibly
  remote `DocumentStore`; deferred metadata reads would reintroduce a per-document read exactly
  where the abstraction is designed to make that expensive.
- *No sqlite cache at all — scan the `DocumentStore` via `list` on every query* — rejected
  outright: this is today's `glob`-and-parse-every-file approach, the thing FR-024/SC-013 exist
  to move away from, and would make a remote `DocumentStore` impl unusably slow for graph/listing
  operations.

## 10. Removing the `filesystem` `VizierStorageProvider` backend

**Decision**: Delete `src/storage/fs/` (`FileSystemStorage` and its eleven trait impls:
`agent`, `dream`, `dream_journal`, `global_config`, `history`, `memory`, `provider`, `session`,
`session_file`, `state`, `task`, `user`), the `StorageKind::Filesystem` CLI variant and
`StorageConfig::Filesystem` config variant, and the `filesystem` value for `VIZIER_STORAGE`/
`--storage`. `SqliteStorage` becomes the only `VizierStorageProvider`; `VizierDependencies`'s
`(storage, sqlite_conn)` match on `config.storage` collapses to a single sqlite-only path, and
`sqlite_conn: Option<...>` can become non-`Option` (it was only ever `None` in the filesystem
branch).

**Rationale**: Once memory (the entity that most needed "everything is a file" — the whole
reason a filesystem backend's document-shaped storage existed) moves onto `DocumentStore` (§1)
instead, nothing left in `VizierStorageProvider` has a reason to prefer flat files over sqlite
rows — `SqliteStorage` already implements every other trait (`AgentStorage`, `TaskStorage`,
`SessionStorage`, `UserStorage`, etc.) today, so removing `FileSystemStorage` deletes a fully
redundant, already-parallel-maintained code path rather than requiring new work. This also
directly resolves the exact class of duplication Principle II warns about, project-wide, not
just for memory: every one of those eleven trait impls in `src/storage/fs/` is today a second,
independently-maintained implementation of behavior `src/storage/sqlite/` already provides. It
does narrow the constitution's Distribution & Technology Constraints section (which currently
names both backends as supported) — flagged in `plan.md`'s Constitution Check as a required
amendment to land alongside this change, not a silent divergence.

**Alternatives considered**:
- *Keep the filesystem backend for non-memory entities, only move memory off it* — rejected:
  leaves eleven trait impls duplicated for no remaining reason once memory (their last unique
  justification for representing state as loose files) no longer depends on the mechanism; keeps
  paying the Principle II maintenance cost the removal would otherwise close out.
- *Deprecate but don't delete (mark `--storage filesystem` as legacy, keep the code)* — rejected:
  the constitution and CLAUDE.md are explicit that this codebase prefers deleting code once it's
  confidently unused over indefinite backwards-compatibility shims; a config-less fresh install
  already defaults to sqlite today, so `filesystem` was already the less-used path.

## 11. Agent exploration of bundles and the graph reuses the existing tool set — no new tool

**Decision**: An agent explores its own memory with the same seven tools it has today
(`memory_list`, `memory_read`, `memory_write`, `memory_detail`, `memory_follow`, `memory_graph`,
`memory_delete`), now bundle-aware, rather than adding a dedicated `memory_bundles`/`list_bundles`
tool. The key move is giving `bundle: Option<String>` **different, explicit semantics per tool
depending on whether it addresses one concept or scans a collection** (full table in
`contracts/memory-tools.md`):

There are three distinct `bundle: None` shapes across the seven tools, not two — corrected from
an earlier draft that incorrectly lumped `memory_read` (semantic search) in with the
single-concept tools:

- **Single-concept** (`memory_write`, `memory_detail`, `memory_delete`, `memory_follow`) treat
  `bundle: None` as "my default bundle" — the same resolution rule on read and write, so an
  unqualified reference always means "the thing in my default bundle."
- **Search** (`memory_read`, backed by `query_memory`) treats `bundle: None` as "search across
  every bundle" — the relevance-ranked result set an agent almost always wants, since a bundle is
  an organizational container with no content of its own to match a query against; only its
  concepts do. Naming a `bundle` narrows the same search to just that one.
- **Browse** (`memory_list`, `memory_graph`) treats `bundle: None` as **the top-level view** —
  `memory_list` returns one summary row per bundle (name, concept count, last-updated, via the
  new `list_bundles`); `memory_graph` returns the bundle-level graph (bundles as nodes,
  cross-bundle links as edges — FR-019's top level). Naming a `bundle` **focuses** both tools on
  that bundle's contents instead — `memory_list` lists its concepts (flattened across nesting,
  paginated); `memory_graph` returns its concept-level graph plus boundary nodes for outward
  links (§8). `memory_list` and `memory_graph` deliberately zoom the same way, since they're the
  two ways of browsing the same two-level structure — this is themselves calling
  `list_bundles`/`get_memory_graph(None)` under the hood, not a special case bolted onto each.

The resulting recipe (`contracts/memory-tools.md`'s Exploration recipe, taught in `BOOT.md` per
`contracts/boot-doctrine.md`): `memory_list()`/`memory_graph()` → see bundles → `memory_list(
bundle)`/`memory_graph(bundle)` → see one bundle's concepts and links → `memory_follow`/
`memory_detail` → traverse or open a specific one; separately, `memory_read(query)` → search
everywhere at once, narrowing with `bundle` only once you already suspect where the answer lives.

**Rationale**: Principle I — the two-level graph (FR-019) and a matching two-level listing
already answer every step of "what exists, what's in it, follow a link, open it" once `bundle` is
threaded through correctly; a new tool would duplicate what `memory_list()`/`memory_graph()` (no
argument) already return. The harder problem this surfaced wasn't *which tool* — it was that
`bundle: None` needs a documented, per-tool-*shape* meaning (single-concept vs. search vs.
browse), because a single uniform rule is wrong for at least one of the three: uniformly
"default bundle" would silently scope an agent's own search/list calls down to a fraction of what
they could see today (an FR-001 regression), while uniformly "everything" would make `memory_read`
default to full-corpus search *only* — which happens to be right for search but is not what
`memory_list`/`memory_graph`'s FR-019 two-level model calls for at the top level (a flattened
cross-bundle concept dump isn't "the bundle-level view").

Note that this is purely a **tool-layer presentation choice** — the underlying storage methods
(`get_all_agent_memory`, `query_memory`) keep their own `None` meaning "every bundle" regardless
(they're relied on internally for reranking, migration, and cache reconciliation, where "top
level only" would be actively wrong). `memory_list`'s top-level view calls the new
`list_bundles` instead of `get_all_agent_memory(None)` — see `contracts/memory-storage-trait.md`.

This zoom convention is **deliberately not mirrored onto the HTTP API's `GET /`** route
(`contracts/http-api.md`). A tool's default can be redefined freely because its only caller is an
LLM re-reading the tool description from context every turn; `GET /` has real compiled callers
(the WebUI, potential external API integrations) that hard-code an expected response shape, and
having one route switch between returning `Memory[]` and `BundleSummary[]` based on a query
param is a REST anti-pattern a fixed HTTP contract shouldn't take on just to mirror a
tool-layer convenience. So `GET /` keeps its pre-existing shape and default (flat `Memory[]`,
`?bundle=` narrowing it), and the top-level view gets its own dedicated route, `GET /bundles`.

**Alternatives considered**:
- *Add a dedicated `memory_bundles` tool* — rejected: `memory_list()`/`memory_graph()` with no
  `bundle` already return the bundle set; a third tool returning a subset of the same information
  is duplication with no behavior it uniquely enables.
- *Make every tool's `bundle: None` mean "default bundle," uniformly* — rejected: this is exactly
  the FR-001 regression risk above for `memory_list`/`memory_read` — an agent that doesn't yet
  know about bundles would find its own search/list calls silently scoped to a fraction of what
  it could see today.
- *Make `memory_list`'s `None` mean "every bundle, flattened"* (this feature's earlier draft) —
  superseded: it broke the deliberate symmetry with `memory_graph` (browsing two ways should zoom
  the same way) for no compensating benefit, since there's no compiled call site for an unmodified
  `memory_list()` call to protect — the agent's calling convention comes entirely from the tool
  description/`BOOT.md` text in its context each turn, which is updated in the same change.
- *Require every tool call to always name a bundle explicitly (no default at all)* — rejected:
  breaks every existing agent-authored tool call and few-shot example the moment this ships,
  which is exactly the regression Story 1 exists to prevent.
