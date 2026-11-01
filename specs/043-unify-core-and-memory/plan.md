# Implementation Plan: Unify Core and Memory

**Branch**: `043-unify-core-and-memory` | **Date**: 2026-11-01 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/043-unify-core-and-memory/spec.md`

## Summary

Identity stops being a separate document with its own tools, its own history table and its own page. It becomes the memory document `core/soul` inside a reserved bundle, read and written by the memory tools, injected once per turn as an addressed `<memory>` block, and reviewed through the memory surfaces. The dedicated Core page is deleted; the `/core` API is kept as a deprecated projection over the same document. Memory writes gain a hard content ceiling (8 KiB) that refuses with the overage and a split pattern, and a soft limit (4 KiB) that saves and returns a separate warning field. Identity is tighter (4 KiB / 1.5 KiB) because it is paid for on every turn.

## Technical Context

**Language/Version**: Rust 2024 edition, workspace at 0.14.0-rc.3; TypeScript 5 / React 19 for the WebUI submodule

**Primary Dependencies**: `rusqlite` (bundled) + `sqlite-vec` for the derived memory index; `anyhow` inside the storage layer, `VizierError` at the library boundary; `schemars`/`utoipa` for tool and API schemas. No new dependency is added by this feature.

**Storage**: markdown documents through `DocumentStore` (`{workspace}/agents/{id}/memory/{bundle}/{path}.md`), with the derived SQLite index (`memory_node`, `memory_edge`, `memory_passage`, `document_index`). Identity additionally has `agent_core` (content) and `core_revision` (history) today; this feature makes the memory document the source of truth and leaves both tables in place as the read-only legacy source and the deprecated API's projection.

**Testing**: `cargo test -p vizier-core --lib` for storage and tool units (the two pre-existing Windows-only failures in `projects::path` and `skill::scopes` remain, and are not this feature's). WebUI: `cd webui && npm run typecheck && npm test`, plus `design`'s `tsc --noEmit`. End-to-end: dummyplug agent steps in [quickstart.md](./quickstart.md) — this feature is agent-observable (prompt content, memory writes, dream journal), so the constitution's e2e gate applies.

**Target Platform**: the same matrix as today — Linux/macOS/Windows servers, the Tauri desktop app, and the embedded WebUI. Nothing here is platform-specific; the Windows-only hazards are the existing `LocalDocumentStore` path rules, which this feature does not touch.

**Project Type**: Rust library + binary workspace (`vizier-core` → `vizier-http` → `vizier`), with a React submodule (`webui/`) and a component library submodule (`design/`).

**Performance Goals**: no measurable change to a memory write or a turn. The cap is a `content.len()` comparison before writing; the identity injection replaces an existing system message one-for-one; the bundle guard adds one string compare per write.

**Constraints**: the prompt prefix carrying identity must stay byte-stable across turns (FR-007) — the injected block is built from the document alone, with no timestamp or per-turn data. Migration must be idempotent and must not delete the legacy record (FR-003, FR-026). No upgrade may be blocked by the new ceiling (FR-025).

**Scale/Scope**: per-agent documents, single-digit thousands at most. Touches roughly 20 files across `crates/vizier-core`, `crates/vizier-http`, `webui/` and `design/`, and removes two WebUI routes.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Assessment | Evidence |
|---|---|---|
| **I. Lean by default** | **Pass.** No new crate, trait, generic or config surface beyond one small struct next to `ChunkLimits`. The deprecated `/core` API is not new code — it already exists, and keeping it is less change than deleting it. | `config::ChunkLimits` precedent; `MemoryOpSettings` already carries per-agent memory config |
| **II. DRY via trait-based extensibility** | **Pass.** Nothing branches on a type tag. The bundle rule and the size rule are both enforced in **one** place, `BundleMemoryStore::write_memory_inner`, which every write path already funnels through (tool, HTTP, restore, import) — exactly what FR-022 demands. The `Identity` tool category is removed, collapsing two vocabulary into one. | `storage/memory_bundle.rs::write_memory_inner` is the single write path |
| **III. Self-contained runtime** | **Pass.** No external service, no new network path. Identity moves from a SQLite row to a markdown document on the same disk; the default identity comes from an embedded template, so a fresh offline install still gets one. | `constant::CORE_MD` already embeds the template |
| **IV. Portability by default** | **Pass.** Paths go through `DocumentStore::resolve`; the reserved bundle is a plain string, not a path. Nothing touches `cfg`, line endings or process invocation. | `LocalDocumentStore::resolve` unchanged |
| **V. Unified errors & observability** | **Pass, needs care.** The storage layer keeps returning `anyhow::Result` and the tool boundary keeps wrapping into `VizierError`. The new refusal must carry a structured message so `op_error` can map it to a status without string-matching something generic, and the warning must reach the agent, so it cannot be a log line. | `service/memory.rs::op_error`; research Decision 4 |
| **Gate: conventional commits** | **Pass.** Six commits on this branch already follow it; later phases keep `feat:`/`doc:`/`chore:` and stage by path. | `git log --oneline` |
| **Gate: matching submodule branches** | **Blocking — resolve before implementation.** `043-unify-core-and-memory` does not yet exist in `webui/` or `design/`, and this feature edits both. | `just checkout 043-unify-core-and-memory` |
| **Gate: manual verification for migrations** | **Pass by design.** The migration is a startup path, so quickstart step 1 runs the real binary against a prepared workspace instead of trusting `cargo test`. | [quickstart.md](./quickstart.md) |
| **Gate: dummyplug e2e** | **Pass by design.** Every agent-observable behaviour is scripted as a dummyplug step. | [quickstart.md](./quickstart.md) |

**Gate result: one blocking item — the submodule branches.** Everything else passes without a Complexity Tracking entry.

### Re-evaluated after Phase 1 design

| concern | verdict |
|---|---|
| **I. Lean** | Holds. The design added one struct (`MemoryLimits`), one result variant (`Written`), one optional response field and one boolean (`protected`). No trait, no crate, no config key beyond the struct. |
| **II. DRY** | Strengthened. The whole rule set collapsed into `write_memory_inner`, and Phase 0 Decision 8 *deleted* the second dream query rather than adding a parallel path — the design removes a duplicated branch instead of adding one. |
| **III. Self-contained** | Holds. Nothing in the contracts reaches outside the process. |
| **IV. Portability** | Holds. `contracts/storage-behaviour.md` adds no path handling; all addressing stays inside `DocumentStore`. |
| **V. Errors** | Holds, and is now pinned: `contracts/storage-behaviour.md` fixes the message prefixes and their status mapping, so the string-based classification is a contract rather than an implementation accident. |
| **New risk found in design** | `rollback_memory` inherits the cap, so restoring an over-cap historical revision is refused. Recorded in the contract as accepted (the content is still recoverable by editing), not silently allowed. |

## Project Structure

### Documentation (this feature)

```text
specs/043-unify-core-and-memory/
├── plan.md                  # This file
├── spec.md                  # Feature specification (35 FRs, 13 SCs)
├── research.md              # Phase 0 decisions
├── data-model.md            # Phase 1 entities and invariants
├── quickstart.md            # Phase 1 dummyplug + manual verification
├── contracts/
│   ├── tool-surface.md      # memory tools, the reserved bundle, the warning field
│   ├── http-api.md          # deprecated /core, memory endpoints extended
│   ├── storage-behaviour.md # the single write path's rules
│   └── webui.md             # page deletion, memory-explorer reachability
├── checklists/requirements.md
└── tasks.md                 # Phase 2 output (/speckit-tasks)
```

### Source Code (repository root)

```text
crates/vizier-core/src/
├── constant.rs                     # CORE_MD stays the default identity source
├── config/mod.rs                   # + MemoryLimits beside ChunkLimits
├── schema/
│   ├── agent.rs                    # + MemoryLimits on AgentConfig
│   ├── commands.rs                 # MemoryOpResponse::Written { memory, warning }
│   ├── dream_journal.rs            # ChangeKind::Core by origin address, not table
│   ├── revision.rs                 # CoreRevision types kept (deprecated projection)
│   └── storage.rs                  # + CORE_BUNDLE / SOUL_PATH constants
├── storage/
│   ├── memory_bundle.rs            # THE write path: bundle guard, cap, warning, bypass
│   ├── memory.rs                   # trait: write_memory takes the limits, returns Written
│   ├── agent.rs                    # get/set_agent_core → deprecated identity projection
│   ├── sqlite/agent.rs             # projection + legacy read
│   ├── sqlite/dream_journal.rs     # classify by (bundle, path), not by table
│   └── fs/agent.rs                 # legacy on-disk read source only
├── dependencies.rs                 # migrate_agent_cores → seeds core/soul
├── agents/
│   ├── agent/mod.rs                # prompt: addressed <memory> block from the document
│   ├── agent/system_prompt/boot.rs # doctrine: identity is a memory at its address
│   ├── tools/mod.rs                # - Identity category, - read_core/write_core, dream lists
│   ├── tools/workspace/mod.rs      # read_core / write_core deleted
│   ├── tools/vector_memory/mod.rs  # memory_write returns message + warning
│   ├── supervisor.rs               # identity backfill at spawn, beside passage reconcile
│   └── memory_ops.rs               # carries the limits to every memory op
├── service/core.rs                 # deprecated projection over the identity document
├── service/memory.rs               # warning passthrough; cap refusal → 422
└── scheduler/dream/prompts.rs      # write_core → memory_write on the identity document

crates/vizier-http/src/api/v1/agents/
├── core.rs                         # deprecated marker; response shapes unchanged
└── documents.rs                    # retired per the spec's naming-history assumption

webui/app/
├── routes.ts                       # - core and core/edit routes
├── routes/agent-core.tsx           # deleted
├── routes/agent-core-edit.tsx      # deleted
├── routes/memory.tsx               # reserved bundle rendered as protected
├── screens/shell/pages.ts          # - core page entry
├── lib/dreams.ts                   # identity changes; link into Memory
└── api/endpoints/agentPages.ts     # core* calls removed

design/src/components/
├── Shell/ContextSidebar.tsx        # - the menu group that held only Core
├── NightStrip/                     # ring + aria text: identity changes
└── DreamJournalEntry/              # the identity change kind
```

**Structure Decision**: one workspace, four touch points. The engine change concentrates in a single function (`write_memory_inner`) plus the prompt builder and the migration — which is why every rule lives there rather than being distributed across tools, HTTP handlers and imports. The WebUI change is mostly subtraction.

## Requirement traceability

Every requirement maps to the artefact that satisfies it. "storage" = `contracts/storage-behaviour.md`, "tools" = `contracts/tool-surface.md`, "http" = `contracts/http-api.md`, "webui" = `contracts/webui.md`, "model" = `data-model.md`, "qs" = `quickstart.md`.

| FR | where | FR | where | FR | where |
|---|---|---|---|---|---|
| 001 | model inv. 1 | 013 | storage rule 1, http (422) | 025 | storage rule 2, research D3 |
| 002 | model rule 14, qs 1 | 014 | model inv. 1, storage (import) | 026 | research D3, issue #82 |
| 003 | model rules 14–15, qs 1 | 015 | model inv. 2 | 027 | webui (memory surfaces), qs 6 |
| 004 | tools (removed table) | 016 | model rule 4 | 028 | http (`base_seq`), qs 6.4 |
| 005 | model inv. 2 | 017 | model rule 10 | 029 | webui rules 1–3 |
| 006 | qs 2 | 018 | tools (refusal), model rule 11 | 030 | tools (dangling table), webui |
| 007 | research D6, qs 2 | 019 | model rule 5, qs 5 | 031 | research D8, model rule 12 |
| 008 | research D10, qs 4 | 020 | tools (result contract) | 032 | model rule 12, webui rule 4 |
| 009 | research D9, qs 2 | 021 | model rule 8 | 033 | model rule 13 |
| 010 | tools (doctrine), qs 3 | 022 | research D1 | 034 | http (deprecated section) |
| 011 | model inv. 2, qs 4 | 023 | model rule 8 | 035 | research D11, http |
| 012 | model inv. 3, storage (delete_bundle), qs 4 | 024 | tools (doctrine), research D6 | | |

| SC | verified by |
|---|---|
| SC-001–005 | qs 1, qs 6, qs 8 |
| SC-006–010 | qs 5 |
| SC-011 | qs 7 |
| SC-012 | qs 6 |
| SC-013 | qs 8 |

## Complexity Tracking

No violations to justify. The one gate that fails — the missing submodule branches — is a repository-hygiene action (`just checkout 043-unify-core-and-memory`), not a design compromise.
