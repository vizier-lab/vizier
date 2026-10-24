# Implementation Plan: Blocking and Background Parallel Subagents

**Branch**: `025-split-parallel-jobs` | **Date**: 2026-10-24 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/025-split-parallel-jobs/spec.md`

## Summary

Replace `paralel_subtasks` with two `agents`-category tools that take the same input:

- `background_subagents`: today's tool under a new name, unchanged (spec 012).
- `parallel_subagents`: dispatches the same subagent pieces but **waits**, and returns `{"results": [...]}` in task order inside the current tool call.

The blocking tool reuses the batch job's piece dispatch, its await-with-timeout and its abort, pulled out of `BackgroundJobs` into two shared functions. It creates no job rows, tray entries or reports. Its wait is bounded by a new per-call `ToolContext.deadline`, so it returns partial results before `tools.timeout` would drop the call. A drop guard aborts unfinished pieces when the turn stops, and a new `VizierRequest.transient` flag keeps blocking pieces out of the turn journal, so restarts and crashes never resume them. The old name gets a retired-name hint instead of an alias. Labels, guidance, docs and four WebUI files are updated.

## Technical Context

**Language/Version**: Rust (edition 2024, the crate's toolchain); TypeScript 5 / React 19 for the WebUI submodule

**Primary Dependencies**: existing only: tokio, flume, serde/schemars, futures. No new crates.

**Storage**: N/A. No schema change. Blocking runs are not persisted; their piece sessions use the existing history tables.

**Testing**: `cargo test` (unit tests for the shared argument validation, result ordering under mixed outcomes, the deadline arithmetic, `is_durable` for transient requests, the retired-name hint and the category table); `cd webui && npm run typecheck` plus vitest for `lib/trail.ts`. End-to-end: the dummyplug agent steps in [quickstart.md](quickstart.md).

**Target Platform**: as the crate (Linux/macOS/Windows, musl cross targets); nothing platform-specific.

**Project Type**: single Rust binary with an embedded WebUI (git submodule).

**Performance Goals**: a blocking call's wall time is within 20% of its slowest piece (SC-002). Pieces already run concurrently through `send_request`, so this needs no new mechanism.

**Constraints**: tool names are dispatch keys (no alias). Code mode only (reached from scripts). A blocking call must return before the outer `tools.timeout` drop (2s grace). It must not change spec 012 behaviour of the background tool.

**Scale/Scope**: about 10 Rust files, 4 WebUI files and 3 docs pages. One new tool and one renamed tool.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle / gate | Assessment |
|---|---|
| I. Lean by default | Pass. No new crate, table, config flag or trait. Two small fields (`ToolContext.deadline`, `VizierRequest.transient`), each with one concrete reason (R3, R4). Dead `Subtask`/`SubtaskArgs` structs are removed. |
| II. DRY via traits | Pass. Both tools implement `VizierTool` and are registered on `default_toolset`. Piece dispatch/await/abort move into shared functions used by both the job runner and the blocking runner (R1). The two tools share one argument type and validation (R9). There is no branching on tool kind in dispatch code; the retired-name hint is a data table consulted where unknown names are already handled. |
| III. Self-contained runtime | Pass. No external service. |
| IV. Portability | Pass. Pure async Rust on existing abstractions. |
| V. Unified errors & observability | Pass. Refusals are `VizierError`, logging uses `tracing`. The existing `execute_tool` span and trace propagation cover the pieces (R12). |
| Gate: clippy, test, typecheck | Planned in tasks. |
| Gate: dummyplug e2e | quickstart.md scripts every FR as dummyplug steps. The one live-model check (tool choice) is marked. |
| Gate: conventional commits | `feat: …[**breaking**]` for the rename. WebUI commit lands in `vizier-lab/webui` before the pin bump. |
| Convention: #82 tracker | `RETIRED_TOOL_NAMES` and the WebUI's old-name entry in `JOB_LAUNCHING_TOOLS` get a checklist entry (R13). |

**Post-design re-check**: unchanged, all pass. Complexity Tracking stays empty.

## Project Structure

### Documentation (this feature)

```text
specs/025-split-parallel-jobs/
├── spec.md
├── plan.md              # this file
├── research.md          # Phase 0: R1–R13
├── data-model.md        # Phase 1: in-process types, no storage change
├── quickstart.md        # Phase 1: dummyplug e2e steps
├── contracts/
│   └── agent-tools.md   # Phase 1: both tools' input/output/descriptions, retired name, guidance
├── checklists/requirements.md
└── tasks.md             # Phase 2 (/speckit-tasks)
```

### Source Code (repository root)

```text
src/
├── agents/
│   ├── background/
│   │   ├── mod.rs              # extract dispatch_piece / await_piece / check_depth; run_job and launch use them
│   │   └── wait.rs             # NEW: BackgroundJobs::run_and_wait + drop guard (abort unfinished pieces)
│   ├── tools/
│   │   ├── subagents.rs        # NEW (replaces subtasks/): SubagentsArgs, ParallelSubagentsTool, BackgroundSubagentsTool
│   │   ├── subtasks/           # REMOVED
│   │   ├── mod.rs              # ToolContext.deadline; register both tools; category table; RETIRED_TOOL_NAMES
│   │   ├── background_jobs.rs  # list_background_jobs description names background_subagents
│   │   └── python/
│   │       ├── bridge.rs       # retired-name hint on unknown function
│   │       └── docs_tools.rs   # retired-name hint in describe_tool_function not-found
│   └── agent/
│       ├── mod.rs              # set ctx.deadline per tool call; remove dead Subtask/SubtaskArgs
│       └── system_prompt/boot.rs  # "Large tasks" guidance
├── schema/request.rs           # VizierRequest.transient; is_durable; doc comment
├── utils/mod.rs                # status text for both tools
└── usage/mod.rs                # SUBAGENT sublabel → "Subagent"

docs/src/configuration/
├── tools-embedding.md          # tool table
├── agents.md                   # usage "Background" row
└── observability.md            # span example, background_job paragraph

webui/ (submodule vizier-lab/webui)
└── app/
    ├── lib/trail.ts, lib/trail.test.ts
    ├── screens/chat/BackgroundJobsPanel.tsx, screens/chat/MessageList.tsx
    └── api/frames.ts, api/schema.d.ts (regenerated)
```

**Structure Decision**: the single existing crate layout. The new tool file replaces `tools/subtasks/` (a one-file directory) with `tools/subagents.rs`, and the blocking runner sits next to the job runner it shares primitives with.

## Complexity Tracking

No violations.
