# Implementation Plan: Version History for CORE.md and Memories

**Branch**: `006-memory-version-history` | **Date**: 2026-09-15 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/006-memory-version-history/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command. See `.specify/templates/plan-template.md` for the execution workflow.

## Summary

Add an append-only, per-document revision log for two kinds of agent document — the agent's `CORE.md` (stored in the `agent_core` SQLite table) and memory concept documents (markdown files under the `DocumentStore`, addressed by `(agent_id, bundle, path)`). Every successful save through Vizier — the `WRITE_CORE`/`memory_write`/`memory_delete`/`memory_delete_bundle` agent tools (conversation or dream cycle), the WebUI/HTTP `PUT /core` and memory `POST`/`PUT`/`DELETE`/import routes, and rollbacks themselves — records a full-content snapshot tagged with actor and trigger. CORE and memory are treated as two separate things with two separate implementations — `core_revision` (on `AgentStorage`, next to `get/set_agent_core`) and `memory_revision` (on `MemoryStorage`, next to `write/delete_memory`) — sharing only the kind-agnostic value types (`RevisionOrigin`, diff hunks). Recording happens **inside the existing write path** of each kind (`SqliteStorage::set_agent_core`; `BundleMemoryStore::{write_memory, delete_memory, delete_bundle, import_bundle}`) so no caller can bypass it; callers only supply a small `RevisionOrigin` value describing who/what is saving. New HTTP endpoints expose paginated history, a single version, a server-computed line diff between any two versions, and rollback (a rollback is just a normal save with `trigger = rollback`, never a history rewrite). The WebUI gets one shared `VersionHistory` panel used from both the CORE page and the memory detail slide-over. Direct on-disk edits are explicitly not detected (clarified). No agent-facing history tools.

## Technical Context

**Language/Version**: Rust 1.85+ (edition 2024, per `Cargo.toml`); WebUI TypeScript 5 / React 19 / React Router v7

**Primary Dependencies**: `rusqlite` 0.39 (bundled; new `core_revision` + `memory_revision` tables), `serde`/`serde_json` (origin + revision JSON), `chrono` (timestamps), `axum` + `utoipa` (new routes, existing pattern), `async-trait`; **one new crate**: `similar` (pure-Rust line diff, see research Decision 4). WebUI: no new npm dependencies — diff hunks come from the server and are rendered with plain React + existing CSS tokens.

**Storage**: Embedded SQLite — two new append-only tables with two independent implementations: `core_revision` (keyed `agent_id, seq`; `src/storage/sqlite/core_revision.rs`) and `memory_revision` (keyed `agent_id, bundle, path, seq`, plus a `deleted` flag; `src/storage/sqlite/memory_revision.rs`), both created in `SqliteStorage::init_schema` via `CREATE TABLE IF NOT EXISTS`. Memory *documents* stay on disk via `DocumentStore` untouched (FR-021); history is DB-only (clarified).

**Testing**: `cargo test` — unit tests for the revision recording helpers (no-op detection, sequence numbering, baseline insertion, rollback provenance) and the diff-to-hunks conversion, using an in-memory `rusqlite::Connection` + a temp-dir `LocalDocumentStore`, matching the sparse existing style in `src/storage/memory.rs` / `src/storage/memory_bundle.rs`. WebUI: `cd webui && npm run typecheck`. Runtime behavior (tool → history → WebUI) verified manually via `just run` per constitution.

**Target Platform**: Same as the rest of the binary (Linux/macOS/Windows server, single embedded binary, `cross` musl targets)

**Project Type**: Single Rust project + bundled WebUI — additive change across storage, schema, tools, HTTP API, and WebUI

**Performance Goals**: History list for a 1,000-version document renders within 2 s (SC-005) — served by an indexed `(agent_id, kind, bundle, path, seq DESC)` query with offset/limit, list rows exclude content. Diff of two typical documents (< 2,000 lines) computed on request in well under 100 ms. Recording adds one indexed `SELECT` (latest) + one `INSERT` per save.

**Constraints**: Append-only (rollback never deletes rows); no retention cap (clarified); no-op saves skipped (FR-003); no history in bundle export/import (FR-021); users-only surface (FR-022); same access rules as the document (`user_can_view_agent`, FR-014); history removed with the agent (FR-019)

**Scale/Scope**: 2 new SQLite tables, each with its own sqlite module; **no new storage trait** — 4 history methods added to `AgentStorage` (CORE) and 4 to `MemoryStorage` (memory), plus `VizierStorage` forwarding; signature additions (`origin: RevisionOrigin`) on 5 `MemoryStorage`/`AgentStorage` methods and their ~10 call sites; 1 new `MemoryOpRequest` variant (`Rollback`); 8 new HTTP routes (4 for CORE, 4 for memory); 1 new WebUI component + service functions + wiring in 2 routes

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. Lean by Default**: PASS with one justified dependency. One new crate, `similar` (pure Rust, no transitive deps beyond `std` with default features trimmed), for the line diff — a correct, memory-bounded Myers diff is not "a few lines", and it is needed on both the CORE and memory paths (see research Decision 4 and Complexity Tracking). **No new trait and no new abstraction**: CORE history methods go on `AgentStorage`, memory history methods on `MemoryStorage`, each backed by its own plain sqlite module — a `DocRef`-style table-selecting abstraction was considered and rejected (research Decision 15) because it existed only to fold two different things into one code path. No new config surface. Snapshots are stored as plain text, no delta encoding.
- **II. DRY via Trait-Based Extensibility**: PASS. Principle II targets *variants of the same kind* (a second storage backend, a second channel); CORE and memory are two different document kinds with different identity, lifecycle, and snapshot format, so two implementations is the honest design, not duplication. What *is* the same — provenance types and the line-diff engine — is shared. Recording lives in the *one* write implementation per document kind (`BundleMemoryStore` for memory, `SqliteStorage::set_agent_core` for CORE) rather than at each of the ~10 call sites, so tools/HTTP/migrations/rollback all share it. Actor/trigger provenance is one `RevisionOrigin` type with two constructors (`from_session` for agents, `from_user` for HTTP) — no `match` over channel type spread across tools. The new storage concern is a new trait added to `VizierStorageProvider` and hand-forwarded on `VizierStorage`, exactly as the other twelve. Diff is computed in one server-side function used by both CORE and memory endpoints; the WebUI has one `VersionHistory` component used from both pages.
- **III. Self-Contained, Zero-Dependency Runtime**: PASS. Everything lives in the embedded SQLite DB; no external service, no network, no new config. `similar` is pure Rust and builds on every `Cross.toml` target.
- **IV. Portability by Default**: PASS. No OS-specific code; all persistence through `rusqlite` and the existing `DocumentStore`. Line diff treats `\n` as the separator and tolerates `\r\n` by normalizing before diffing.
- **V. Unified Errors & Observability**: PASS. Storage layer returns `anyhow::Result` like its neighbors in `src/storage/`; tools map to `VizierError` as `WriteCore` already does; HTTP handlers use `err_response`. A failed revision insert fails the save (the save and its revision are one SQLite transaction where the document itself is DB-side; for memory, the revision is written after the document `put` succeeds and a revision failure is returned as the save error — see research Decision 8). Logging via `tracing` only.

Post-design re-check (after Phase 1): unchanged — no additional violations introduced by the data model or contracts.

## Project Structure

### Documentation (this feature)

```text
specs/006-memory-version-history/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
│   ├── http-api.md      # New history/diff/rollback endpoints for CORE and memory
│   ├── storage-trait.md # AgentStorage/MemoryStorage history methods, sqlite modules, signature changes
│   └── webui.md         # VersionHistory component contract + service functions
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
Cargo.toml                                   # ADD: similar = "2" (default-features = false, features = ["text"])

src/schema/
├── revision.rs                              # NEW: shared RevisionActor/RevisionTrigger/RevisionOrigin,
│                                            #      RevisionDiff/DiffHunk/DiffLine, RollbackResponse;
│                                            #      CoreRevision{,Summary}, PaginatedCoreRevisions;
│                                            #      MemoryRevision{,Summary}, PaginatedMemoryRevisions
├── mod.rs                                   # MODIFY: pub mod revision; re-exports
└── commands.rs                              # MODIFY: MemoryOpRequest::{Write,Delete,DeleteBundle,ImportBundle}
                                             #         gain `origin: RevisionOrigin`; ADD Rollback variant;
                                             #         ADD ListRevisions/GetRevision/DiffRevisions read variants
                                             #         (memory history reads go through the memory-op channel too,
                                             #         see research Decision 6)

src/storage/
├── diff.rs                                  # NEW: shared `diff_lines(from, to)` → hunks (uses `similar`)
├── mod.rs                                   # MODIFY: forward the new/changed AgentStorage + MemoryStorage
│                                            #         methods on VizierStorage (no new trait)
├── agent.rs                                 # MODIFY: set_agent_core(agent_id, core, origin);
│                                            #         ADD list/get/diff_core_revisions, rollback_core
├── memory.rs                                # MODIFY: write_memory/delete_memory/delete_bundle/import_bundle
│                                            #         gain `origin`; ADD list/get/diff_memory_revisions,
│                                            #         rollback_memory
├── memory_bundle.rs                         # MODIFY: call memory_revision::record inside write/delete/
│                                            #         delete_bundle/import; implement the memory history
│                                            #         methods + rollback_memory
└── sqlite/
    ├── mod.rs                               # MODIFY: init_schema — CREATE TABLE core_revision,
    │                                        #         memory_revision + indexes
    ├── core_revision.rs                     # NEW: CORE-only helpers (ensure_baseline, record, latest,
    │                                        #      get, list, delete_agent) + CoreRevisionRow
    ├── memory_revision.rs                   # NEW: memory-only helpers (same names, own SQL, own row type)
    │                                        #      + memory_canonical / parse_memory_canonical
    └── agent.rs                             # MODIFY: set_agent_core records into core_revision in the same
                                             #         transaction; impl the CORE history methods;
                                             #         delete_agent clears both revision tables

src/agents/
├── memory_ops.rs                            # MODIFY: pass `origin` through; dispatch Rollback + history reads
├── mod.rs                                   # MODIFY: agent-creation CORE seed passes origin=system/baseline
└── tools/
    ├── workspace/mod.rs                     # MODIFY: WriteCore builds RevisionOrigin::from_session(&ctx.session)
    └── vector_memory/mod.rs                 # MODIFY: memory_write/memory_delete/memory_delete_bundle likewise

src/dependencies.rs                          # MODIFY: migrations' set_agent_core/write_migrated_memory calls
                                             #         pass origin=system/baseline

src/channels/http/
├── auth/mod.rs                              # MODIFY: AuthenticatedUser gains `auth_method: AuthMethod`
├── auth/middleware.rs                       # MODIFY: set auth_method (Jwt | ApiKey)
└── api/v1/agents/
    ├── core.rs                              # MODIFY: update_core passes origin; ADD history/get/diff/rollback
    │                                        #         handlers + routes under /core/history
    ├── memory.rs                            # MODIFY: write/delete/import pass origin; ADD history/get/diff/
    │                                        #         rollback handlers + routes under /history/{bundle}/{*path}
    └── mod.rs                               # (routes are nested via core()/memory(); no change expected)

webui/app/
├── interfaces/types.ts                      # ADD: Core*/Memory* revision types, RevisionDiff, RollbackResponse
├── services/vizier.tsx                      # ADD: getCoreHistory/getCoreRevision/diffCoreRevisions/rollbackCore
│                                            #      + getMemoryHistory/getMemoryRevision/diffMemoryRevisions/rollbackMemory
├── components/VersionHistory.tsx            # NEW: shared panel — paginated list, version viewer, diff viewer
│                                            #      (prev + pick-two), rollback with confirm
├── routes/agent-core.tsx                    # MODIFY: "History" button → VersionHistory in a SlideOver;
│                                            #         reload content after rollback
└── routes/memory.tsx                        # MODIFY: "History" button in the view slide-over → VersionHistory;
                                             #         refresh detail/list after rollback
```

**Structure Decision**: Single project, additive. No new storage concern is introduced: CORE history extends `AgentStorage` (`src/storage/agent.rs` ↔ `src/storage/sqlite/agent.rs`) and memory history extends `MemoryStorage` (`src/storage/memory.rs` ↔ `src/storage/memory_bundle.rs`), each with its own sqlite helper module. Provenance is a schema type (`src/schema/revision.rs`) because it crosses the transport boundary inside `MemoryOpRequest`. HTTP handlers are added inside the existing `core.rs`/`memory.rs` modules so the route nesting in `agents/mod.rs` stays untouched. The WebUI gets one new component consumed by both existing routes.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| New crate `similar` (Principle I — "hand-rolled few lines" test) | Line-level diff between two snapshots, served by the API and rendered by the WebUI (FR-008/FR-009) | A naive LCS table is O(n·m) memory (a 5k-line CORE vs 5k-line CORE = 25M cells) and a correct Myers implementation with hunk grouping is ~150 lines of subtle code that would then need its own tests; a client-side JS diff library would be a second dependency *and* would leave the API without a diff endpoint. `similar` is pure Rust, tiny with default features off, and already the de-facto standard in the ecosystem. |
