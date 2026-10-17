# Implementation Plan: Scratchpad Tools

**Branch**: `020-scratchpad-tools` | **Date**: 2026-10-17 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/020-scratchpad-tools/spec.md`

## Summary

Every agent gets a `scratchpad` tool category with `write_tmp`, `append_tmp` and `read_tmp`: named plain-text files private to one conversation. They live in a new SQLite table `scratch_file` behind a `ScratchStorage` trait. Limits (1 MiB per file, 64 files per conversation, a safe name rule) are checked atomically in storage, and every refusal comes back as data. The agent's process loop, which already knows when a session has no running turn and no queue, clears the session's scratch files at each idle transition, on `/abort`, and when an instance stops with nothing to hand on. Leftovers are cleared at startup and when a session or agent is deleted. The dream cycle gets the three tools in `DREAM_TOOL_NAMES`.

The decisions are in [research.md](research.md) (R1–R10), the design in [data-model.md](data-model.md) and [contracts/agent-surface.md](contracts/agent-surface.md).

## Technical Context

**Language/Version**: Rust 2024 (stable)

**Primary Dependencies**: existing only: rusqlite, serde/schemars, async-trait, tokio. **No new crates.**

**Storage**: embedded SQLite, one new table `scratch_file` (data-model §1); no change to the memory `DocumentStore`.

**Testing**: `cargo test`. Unit tests for `storage/sqlite/scratch.rs` (limits, atomicity, isolation, `None` topic, clears), the name rule, the category table (`every_tool_is_in_its_category`) and the dream tool list. End-to-end: dummyplug steps in [quickstart.md](quickstart.md) §1–§9; §10 (a dream run) is marked live-provider.

**Target Platform**: the vizier binary on every `Cross.toml` target (unchanged).

**Project Type**: single Rust binary (no WebUI change).

**Performance Goals**: read or write of up to 1 MiB well under 1 s (SC-006). Append is a single `content || ?` update. The idle clear is one primary-key-prefix `DELETE` per idle transition, awaited inline in the process loop.

**Constraints**: refusals never `Err` (FR-012); scratch never in memory, history (beyond the script), revisions or usage (FR-014); the idle clear must not race a new turn of the same session (research R4).

**Scale/Scope**: about 10 Rust files: 3 new (`storage/scratch.rs`, `storage/sqlite/scratch.rs`, `agents/tools/scratchpad.rs`), 7 edited (`storage/mod.rs`, `storage/sqlite/{mod,session,agent}.rs`, `agents/tools/mod.rs`, `agents/process.rs`, `dependencies.rs`), plus a docs section in `docs/src/configuration/tools-embedding.md`.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design: still passes.*

| Principle / Gate | Assessment |
|---|---|
| **I. Lean by Default** | No dependency, no config, no new abstraction beyond the storage trait every entity has. Limits are constants. No list/delete tools (spec). The idle clear reuses `session_is_busy` instead of adding session-lifecycle events. ✅ |
| **II. DRY via traits (NON-NEGOTIABLE)** | The tools implement `VizierTool` and register on `default_toolset`. Storage follows the `ClientCallStorage` shape: trait, `VizierStorage` forwarding, `SqliteStorage` impl, and a composition into `VizierStorageProvider`. The category is one new `ToolCategory` variant, not a branch in dispatch. The three tools share one name check and one result type. ✅ |
| **III. Self-contained runtime** | Embedded SQLite only. ✅ |
| **IV. Portability** | No filesystem paths; all storage is in SQLite (research R1 rejected on-disk files partly for this). ✅ |
| **V. Unified errors & observability** | `crate::Result`, `tracing` for the sweep and clear failures. Tool calls get `execute_tool` spans from `ToolRouter::call` as today, and content is recorded only under `capture_content()`. ✅ |
| Gate: clippy/test | Planned per task. ✅ |
| Gate: e2e through dummyplug | quickstart.md §1–§9 dummyplug; §10 marked live-provider; no protocol change. ✅ |
| Gate: conventional commits | `feat:`. Not breaking for agents. The commit notes that `scratchpad` is now a reserved category name (an MCP server keyed `scratchpad` becomes `scratchpad_mcp`, research R6). ✅ |
| Migrations tracked in #82 | None: a new table and a startup sweep are not compatibility code. ✅ |

No violations. Complexity Tracking is empty.

## Project Structure

### Documentation (this feature)

```text
specs/020-scratchpad-tools/
├── plan.md                   # this file
├── research.md               # R1–R10
├── data-model.md
├── quickstart.md             # dummyplug e2e (§10 live-provider)
├── contracts/
│   └── agent-surface.md      # category, tools, result shapes
├── checklists/requirements.md
└── tasks.md                  # /speckit-tasks
```

### Source Code (repository root)

```text
src/
├── storage/
│   ├── scratch.rs                 # NEW: ScratchStorage trait, ScratchWrite, limits, VizierStorage forwarding
│   ├── mod.rs                     # + pub mod scratch; ScratchStorage in VizierStorageProvider
│   └── sqlite/
│       ├── scratch.rs             # NEW: impl ScratchStorage for SqliteStorage + unit tests
│       ├── mod.rs                 # + mod scratch; init_scratch_schema, called from init_schema
│       ├── session.rs             # delete_session also deletes scratch_file rows
│       └── agent.rs               # delete_agent also deletes scratch_file rows
├── agents/
│   ├── process.rs                 # clear on idle (join_next), /abort, instance stop (research R4)
│   └── tools/
│       ├── scratchpad.rs          # NEW: WriteTmp, AppendTmp, ReadTmp, name rule, result types
│       └── mod.rs                 # ToolCategory::Scratchpad (after Files, ALL = 15), registration on
│                                  # default_toolset, DREAM_TOOL_NAMES, category test table
└── dependencies.rs                # clear_orphaned_scratch startup sweep

docs/src/configuration/tools-embedding.md   # short "Scratchpad" section
```

**Structure Decision**: single Rust project. The feature touches storage, the tool layer and the per-agent process loop only. No HTTP endpoint, WebUI or channel change: scratch files are agent-private (spec Assumptions).

## Complexity Tracking

None.
