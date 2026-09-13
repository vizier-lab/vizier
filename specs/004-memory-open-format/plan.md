# Implementation Plan: Agent Memory as Open Documents

**Branch**: `004-memory-open-format` | **Date**: 2026-08-25 | **Spec**: `specs/004-memory-open-format/spec.md`

**Input**: Feature specification from `/specs/004-memory-open-format/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command. See `.specify/templates/plan-template.md` for the execution workflow.

## Summary

Replace agent memory's dual, backend-specific representation (markdown files under the
filesystem backend, JSON-in-a-row under the sqlite backend — two ~90%-duplicated `MemoryStorage`
implementations) with a single implementation that organizes an agent's memory as one or more
named **bundles**: hierarchical directories of markdown concept documents (Open Knowledge Format
style: YAML frontmatter + body), each bundle carrying auto-maintained `index.md`/`log.md`
documents, supporting arbitrarily nested subdirectories, same-bundle markdown links and
cross-bundle `[[bundle/slug]]`/`[[bundle]]` wikilinks, and dropping the existing
private/global/shared visibility model in favor of "everything private to its owning agent."

Document bytes are read/written through a new pluggable `DocumentStore` trait (default impl:
local filesystem), so a future deployment can swap in a different medium (S3, a remote
filesystem) without touching the bundle model or any caller. The embedded database keeps a
derived, reconcilable cache of memory node identity, listing metadata, and link edges (the
**Memory Graph Index**) so listing/filtering/sorting/graph queries never need to read every
document. With memory off the filesystem backend, that backend has no remaining reason to exist
— every other entity it stores is already implemented by the sqlite backend today — so this
feature also **removes the standalone `filesystem` `VizierStorageProvider` backend**, making
sqlite the sole supported backend. A one-time startup migration carries forward every existing
memory and, for deployments that were on the filesystem backend, every other entity too; the
WebUI gains a two-level (bundle → concept) memory graph and a bundle export/import-as-`.zip`
flow.

## Technical Context

**Language/Version**: Rust, edition 2024 (existing `Cargo.toml`)

**Primary Dependencies**: `serde_yaml` (frontmatter, existing), `vizier-derive`'s `MarkdownDoc`
derive + `utils::markdown::{read_markdown, write_markdown}` (existing, reused as-is — research.md
§2), `glob` (existing, recursive `**` bundle traversal, and backing `DocumentStore::list` —
research.md §3), `regex` (existing, link extraction), `rusqlite`/`sqlite-vec` (existing, now the
sole storage backend plus the new Memory Graph Index tables), **`zip`** (new — research.md §4,
needed only for WebUI bundle export/import)

**Storage**: Two-tier. (1) Memory document bytes (concept/index/log documents) go through the
new `DocumentStore` trait (research.md §1, contracts/document-store-trait.md), defaulting to
`LocalDocumentStore` rooted at `{workspace}/agents/{agent_id}/memory/...`. (2) Every other entity,
plus a derived Memory Graph Index (`memory_node`/`memory_edge` tables, research.md §9,
data-model.md) caching memory listing metadata and link edges, lives in the embedded SQLite
database — now the **sole** `VizierStorageProvider` backend; the `filesystem` backend is removed
(research.md §10, FR-025).

**Testing**: `cargo test` (extend `src/storage/memory.rs`'s existing unit tests plus new tests
for bundle path resolution, link-form parsing, collision rejection, `DocumentStore`/graph-index
reconciliation, migration behavior); `cd webui && npm run typecheck` for the WebUI graph/export-
import changes. No new test framework.

**Target Platform**: Same cross-compiled single-binary targets already in `Cross.toml`
(`x86_64`/`aarch64`, `gnu`/`musl`, macOS, Windows). No new system library requirement — `zip`
(pure Rust `deflate` by default) must not require a new `pre-build` step in `Cross.toml`;
verified in Phase 1 before merge.

**Project Type**: Single Rust binary with embedded WebUI (existing architecture, no new
project/service).

**Performance Goals**: Memory search and related-memory/graph lookups stay under 1 second for an
agent with up to 5,000 memory documents (SC-002), served from the Memory Graph Index without
per-call document reads (SC-013). Bundle/index/log/graph-index maintenance work happens
synchronously on write, matching today's pattern (frontmatter rewrite + indexer update).

**Constraints**: Principle III (self-contained runtime) — no new external service; `zip` is the
only new dependency and is a build-time/runtime library, not a service; removing the filesystem
backend does not add any external dependency, sqlite remains embedded. Principle IV
(portability) — all new path handling (in `LocalDocumentStore` and `BundleMemoryStore`) must use
`PathBuf` joins, not raw string concatenation (the current `fs`/`sqlite` memory code's
`format!("{}/{}", ...)` path-building predates this plan and is being replaced anyway — see
Complexity Tracking). Must not break `cross build` for any existing `Cross.toml` target.
Constitution Distribution & Technology Constraints currently names both sqlite and filesystem as
supported backends — this text becomes stale the moment `filesystem` is removed and needs a
companion constitution amendment (see Constitution Check below).

**Scale/Scope**: Per-agent, multiple bundles per agent, arbitrarily nested subdirectories per
bundle, up to thousands of concept documents per agent (SC-002's 5,000 is the explicit target).
This plan's scope now spans two axes:
- **Memory bundles** (Stories 1-4): `src/storage/memory.rs` (trait), new `src/storage/document/`
  (`DocumentStore` + `LocalDocumentStore`), new `src/storage/memory_bundle.rs`
  (`BundleMemoryStore`), `src/schema/storage.rs` (Memory/frontmatter shape), new Memory Graph
  Index tables in `src/storage/sqlite/mod.rs::init_schema`, `src/agents/tools/vector_memory/`
  (tool schemas + descriptions), `src/agents/agent/system_prompt/boot.rs` (doctrine text),
  `src/channels/http/api/v1/agents/memory.rs` (REST contract + new export/import routes),
  `webui/app/components/MemoryGraph.tsx` + a new page-level wrapper (two-level graph),
  `webui/app/interfaces/types` (bundle-aware types).
- **Backend consolidation** (research.md §10): delete `src/storage/fs/` (12 files: `mod.rs` +
  11 trait impls), `StorageKind::Filesystem`/`StorageConfig::Filesystem`, the `--storage
  filesystem`/`VIZIER_STORAGE=filesystem` CLI/env values, and simplify
  `VizierDependencies::new`'s backend `match` and `sqlite_conn: Option<...>` field. Plus a
  constitution amendment to the Distribution & Technology Constraints section (via
  `/speckit-constitution`, tracked as a companion action, not performed inside this plan).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Assessment |
|---|---|
| I. Lean by Default | **Pass.** One new dependency (`zip`), justified by a concrete FR (research.md §4) with no existing crate covering it. `DocumentStore` is one trait + one implementation, justified by an explicit spec requirement (FR-023) for a swappable storage medium, not speculative generality. |
| II. DRY via Trait-Based Extensibility | **Pass, and actively fixes two existing violations.** (a) `src/storage/fs/memory.rs` and `src/storage/sqlite/memory.rs` were near-duplicate `MemoryStorage` impls; collapsed into one implementation (research.md §1). (b) All eleven other `src/storage/fs/*.rs` trait impls duplicate behavior `src/storage/sqlite/*.rs` already provides; removing the filesystem backend entirely (research.md §10) closes that out project-wide, not just for memory. |
| III. Self-Contained, Zero-Dependency Runtime | **Pass.** No new external service — sqlite remains embedded, `zip` is a library not a service, and `DocumentStore`'s default (only, for this feature) implementation is local disk. Removing the filesystem backend doesn't add any runtime requirement; sqlite was already the config-less default. |
| IV. Portability by Default | **Pass, with a cleanup captured below.** New path-building code (`LocalDocumentStore`, `BundleMemoryStore`) must go through `PathBuf`, matching the abstraction this principle asks for; existing string-concatenated paths in the code being replaced are addressed as part of this same change, not deferred. |
| V. Unified Errors & Observability | **Pass.** All new fallible paths (collision rejection, malformed frontmatter, malformed zip, `DocumentStore` unreachable, migration per-item failure) return `crate::Result`/`VizierError` or log via `tracing`, per existing convention — no new error type, no `println!`. |

**Flagged, not a gate failure**: removing the `filesystem` backend (FR-025) makes the
constitution's Distribution & Technology Constraints section ("the only supported storage
backends are embedded SQLite ... and the filesystem backend") factually stale. This doesn't
violate Principle III itself (no external service is introduced or required either way), but the
section's wording needs updating. Per the user's explicit direction, this is folded into the
same effort rather than split into a follow-up — **run `/speckit-constitution` to amend that
section (a MINOR version bump per the constitution's own versioning policy) alongside this
implementation**, not silently left to diverge. This plan does not edit the constitution itself.

Complexity Tracking below records one additional pre-existing observation carried into this
change.

## Project Structure

### Documentation (this feature)

```text
specs/004-memory-open-format/
├── plan.md                        # This file (/speckit-plan command output)
├── research.md                    # Phase 0 output
├── data-model.md                  # Phase 1 output
├── quickstart.md                  # Phase 1 output
├── contracts/                     # Phase 1 output
│   ├── document-store-trait.md
│   ├── memory-storage-trait.md
│   ├── memory-tools.md
│   ├── http-api.md
│   ├── boot-doctrine.md
│   └── migration.md
└── tasks.md                       # Phase 2 output (/speckit-tasks — not created by /speckit-plan)
```

### Source Code (repository root)

This is an existing single-binary Rust project (`src/`) with an embedded WebUI (`webui/`); no
new top-level project or service is introduced. Relevant existing/changed/removed paths:

```text
src/
├── storage/
│   ├── memory.rs                  # MemoryStorage trait — signatures gain bundle/path (contracts/memory-storage-trait.md)
│   ├── document/                  # NEW: DocumentStore trait + LocalDocumentStore impl (contracts/document-store-trait.md)
│   │   └── mod.rs
│   ├── memory_bundle.rs           # NEW: BundleMemoryStore (bundles, index/log, links, collisions, Memory Graph Index upkeep)
│   ├── fs/                        # REMOVED: FileSystemStorage + its 11 trait impls (research.md §10)
│   ├── sqlite/
│   │   ├── mod.rs                  # init_schema gains memory_node/memory_edge tables, drops the flat `memory` table's future use; sqlite_conn becomes non-Option in VizierDependencies
│   │   └── memory.rs                # impl MemoryStorage for SqliteStorage delegates entirely to BundleMemoryStore
├── schema/storage.rs               # Memory/MemoryFrontMatter: drop visibility/shared_to, add created_at, rename timestamp->updated_at, add bundle/path
├── agents/tools/vector_memory/mod.rs # tool Input schemas + descriptions gain `bundle`, drop visibility fields (contracts/memory-tools.md)
├── agents/agent/system_prompt/boot.rs # `## Memory` doctrine text (contracts/boot-doctrine.md)
├── channels/http/api/v1/agents/memory.rs # routes gain bundle scoping + bundle/export/import routes (contracts/http-api.md)
├── cli/run.rs                      # StorageKind loses Filesystem variant
├── config/storage.rs                # StorageConfig loses Filesystem variant
└── dependencies.rs                 # new migrations (contracts/migration.md); backend match collapses to sqlite-only

webui/app/
├── components/MemoryGraph.tsx      # reused unmodified for both graph levels
├── routes/ (or equivalent)          # new page-level wrapper: bundle-level view <-> concept-level view
└── interfaces/types                 # MemoryGraphNode/Edge, Memory: bundle-aware fields
```

**Structure Decision**: No new crate, service, or top-level directory. This is a change
contained within the existing single Rust binary (`src/storage/`, `src/agents/`,
`src/channels/http/`, `src/cli/`, `src/config/`, `src/dependencies.rs`) plus its embedded WebUI
(`webui/app/`), consistent with Principle III (single self-contained binary) — and it *reduces*
the binary's internal surface area by deleting an entire redundant storage backend.

## Complexity Tracking

> No Constitution Check violations require justification beyond the flagged amendment above. One
> pre-existing pattern is called out here for visibility since this change touches it directly:

| Observation | Why it's touched here | Resolution |
|---|---|---|
| Today's `fs`/`sqlite` memory code builds paths with `format!("{}/{}", ...)` rather than `PathBuf::join`, a latent Principle IV gap. | Both implementations are being collapsed into `BundleMemoryStore`/`LocalDocumentStore` as part of this feature (research.md §1), touching every path-building call site anyway. | Fixed in place as part of the rewrite — the new code uses `PathBuf` joins throughout; not tracked as a separate follow-up. |
