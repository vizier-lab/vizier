# Implementation Plan: Subagents as temporary workers

**Branch**: `059-subagent-workers` | **Date**: 2026-11-14 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/059-subagent-workers/spec.md`

## Summary

Turn subagents from a blocking fan-out into **workers the main agent manages**. A worker stays a
turn of the main agent on its own `Subagent` session (so it inherits every tool, permission and
attribution), but its prompt is reframed: a worker boot replaces the conversational one, the soul
and owner profile are presented as *the agent you work for* and *the person it serves*, and its
request says `via: subagent`, `task: 2 of 4`. A new `subagents` category gives the main agent
`spawn_subagents`, `wait_subagents`, `reply_subagent`, `stop_subagent`, `list_subagents` and
`get_subagent`, and gives workers only `ask_main_agent` (turn-ending). Each worker has a fenced
`subagent` row; its turns become durable, so it survives the parent's `/abort`, the parent turn's
end and agent restarts, and lives until it settles, is stopped, or its conversation or agent is
deleted. Outcomes the main agent has not already seen are delivered exactly once through the
spec 058 delivery path, generalised into one `Deliveries` seam with two sources. `parallel_subagents`
is retired with guidance. See [research.md](research.md) D1–D16.

## Technical Context

**Language/Version**: Rust 2024 workspace (`vizier-core`; `vizier` only for the seed-config field)

**Primary Dependencies**: existing only: tokio, `tracing`, `serde`/`serde_json`, `schemars`, the
SQL store (`SqlConn`/`Dialect`), the Fleet bus transport. **No new crate.**

**Storage**: one new table `subagent` through `SqlStorage` (idempotent SQLite init, PostgreSQL
migration `0009_subagents.sql`), fenced on `agent:{id}`. Worker turns use the existing
`session_turn` journal; worker history stays in the existing session history.

**Testing**: `cargo test -p vizier-core` (prompt framing, frontmatter, availability by depth, the
state machine and its guarded updates, the delivery claim racing a wait, recovery re-arming
timers, fence rows), the same storage tests against PostgreSQL (`VIZIER_TEST_DATABASE_URL`),
`cargo test -p vizier-http` (conformance unchanged), `cargo clippy --workspace`; end to end:
dummyplug steps in [quickstart.md](quickstart.md) per the constitution's e2e gate (report wording
and "no greeting" checks are marked **live**).

**Target Platform**: server (Linux, macOS, Docker) and the desktop app (Windows included); nothing
platform-specific.

**Project Type**: Rust workspace (engine library + binary); no WebUI change in this spec.

**Performance Goals**: `spawn_subagents` returns in under 1 s for up to 10 tasks; a settled
outcome wakes a `wait_subagents` within 1 s; a delivery reaches the main agent at its next step
boundary or within 1 s of settling when it is idle (as spec 058).

**Constraints**: delivery exactly once across restarts and owners (spec 057 fencing on every
write); no new required service (Principle III); a worker must never be offered the management
tools under the default setting (FR-018).

**Scale/Scope**: tens of workers per conversation; a result is cut at today's limit with a marker;
the list tool returns running and asking workers plus the last 50 settled ones.

## Constitution Check

*GATE: passed before Phase 0, re-checked after Phase 1.*

| Principle or rule | Verdict |
|---|---|
| **I. Lean by default** | Pass. One table, one service (`Subagents` grows; nothing parallel to it), two request kinds, one config field. No new crate, flag or route. The blocking path and its `WaitGuard` are deleted, not kept beside the new one. Per-call worker definitions were cut to a later spec. |
| **II. Trait-based extensibility** | Pass, and it drives D9: the second delivery source becomes a `DeliverySource` trait with an aggregator instead of a copy of spec 058's three claim sites. Who sees which tool is `VizierTool::available(ctx)`; no dispatch branches on a tool name. Storage is a `SubagentStorage` member of `VizierStorageProvider` on the one `SqlStorage`. The one session branch (`Subagent` in `prepare_system_prompts`, `generate_frontmatter`) follows the existing scheduled-run pattern, which selects on the session by design. |
| **III. Self-contained default** | Pass. SQLite default untouched; PostgreSQL by the shared layer and an ordered migration. |
| **IV. Portability** | Pass. No OS-specific code; quickstart gives the Windows sleep form. |
| **V. Unified errors and observability** | Pass. `crate::Result`, `tracing` only. A worker turn is already an `invoke_agent` span continuing the spawning tool call's trace (request trace propagation); it gains `vizier.subagent.id` and `vizier.subagent.position`. Refusals to the model are `status` results. |
| Design before UI code | N/A. No UI in this spec; the peek spec carries the FigCraft pass. |
| Optional capabilities behind a flag | N/A. Core engine behaviour; no API shape changes by flag. |
| Roles and components independent | Pass. `Subagents` depends on `VizierStorage` and `VizierTransport` only; the gateway side is untouched. |
| Verify what a change touches | Pass. Touches `vizier-core` and one config field read by `vizier`; `Cargo.toml`/`Cargo.lock` untouched. The PR runs those jobs and `gate`. |
| Dummyplug e2e | Pass. Quickstart steps 2–10. |
| Submodule branches | No submodule changes in this spec; the peek spec opens matching branches with `just checkout`. |
| Conventional commits | Pass. PR title `feat(agents)!: subagents as temporary workers` (breaking: `parallel_subagents` retired). |
| Row ownership (spec 057) | Pass. Every write of in-flight work calls `check_fence`; recovery runs on claim (re-arm timers, poke deliverable sessions); `fence_tests.rs` gets one row per method. |
| Breaking-change tracking (#82) | Pass. New `RETIRED_TOOL_NAMES` row and rewritten hints get #82 entries. |

**Post-design re-check**: unchanged. The only place Principle I needed care is the seen/delivered
bookkeeping (D8): two integers on the row instead of an outcome-history table.

## Project Structure

### Documentation (this feature)

```text
specs/059-subagent-workers/
├── plan.md              # This file
├── research.md          # Phase 0: D1–D16
├── data-model.md        # Phase 1: the row, the state machine, the claim
├── quickstart.md        # Phase 1: tests and dummyplug e2e
├── contracts/
│   ├── subagent-tools.md      # the seven tools: input, output, refusals, availability
│   └── worker-turn.md         # worker prompt layout, frontmatter, delivery text, session frame
├── checklists/requirements.md
└── tasks.md             # /speckit-tasks, not this command
```

### Source Code (repository root)

```text
crates/vizier-core/src/
├── agents/
│   ├── subagents.rs                    # REWRITTEN: Subagents service: spawn, settle, ask, reply,
│   │                                   #   stop, timers, wait, recover, delete; no WaitGuard
│   ├── delivery.rs                     # NEW: DeliverySource trait + Deliveries aggregator, poke
│   ├── detached/mod.rs                 # implements DeliverySource; its poke moves to Deliveries
│   ├── process.rs                      # SubagentTask/SubagentReply in the Chat arm; settle
│   │                                   #   answered after chat, failed on Err/panic; claim sites
│   │                                   #   call deps.deliveries
│   ├── agent/mod.rs                    # prepare_system_prompts worker branch; round-boundary
│   │                                   #   claim through deps.deliveries; ToolContext depth
│   ├── agent/system_prompt/
│   │   ├── worker.rs                   # NEW: worker_md, employer/owner headings
│   │   └── boot.rs                     # "Large tasks" rewritten (FR-022)
│   └── tools/
│       ├── subagents.rs                # REWRITTEN: the seven tools, category Subagents
│       └── mod.rs                      # ToolCategory::Subagents, Agents description,
│                                       #   RETIRED_TOOL_NAMES, registration, category table test,
│                                       #   ToolContext.subagent_depth
├── schema/
│   ├── request.rs                      # SubagentTask/SubagentReply kinds, subagent_depth,
│   │                                   #   frontmatter branch
│   ├── subagent.rs                     # NEW: Subagent, SubagentState, StopReason, snapshot
│   └── session.rs                      # SessionFrame::Subagent
├── storage/
│   ├── subagent.rs                     # NEW: SubagentStorage trait
│   ├── sqlite/subagent.rs              # NEW: SQL impl (shared by both dialects)
│   ├── sqlite/fence_tests.rs           # one row per new method
│   └── sql/postgres/migrations/0009_subagents.sql   # NEW
├── transport/wire.rs                   # carry subagent_depth
├── recovery.rs                         # step 7: deps.subagents.recover_agent; pokes via Deliveries
├── dependencies.rs                     # Deliveries on VizierDependencies
└── config/mod.rs                       # SubagentsConfig { max_depth } (default 1)
```

**Structure Decision**: everything lives in `vizier-core`'s existing agent, schema and storage
modules; the binary only gains the seed-config key through `VizierConfig`.

## Complexity Tracking

No constitution violations to justify.
