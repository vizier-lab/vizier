# Implementation Plan: Detached tool calls

**Branch**: `058-detached-tool-calls` | **Date**: 2026-11-13 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/058-detached-tool-calls/spec.md`

## Summary

Replace "background jobs" with one general option: any tool call in a script can carry the reserved keyword `detached` and then runs in its own task while the agent continues. The call's outcome is stored (bounded, redacted) and **delivered exactly once** by an atomic database claim, taken either by the agent's inner loop at its next round boundary or, if no turn is running, by a queued delivery request that wakes the conversation. Agents inspect, list and cancel calls with three tools; people do the same from a collapsible **Background work** panel (drawn in `Vizier shell.figcraft`). The old feature (`background_subagents`, its job and piece tables, the report turn, the job routes and the tray code) is removed, not wrapped. See [research.md](research.md) for the decisions.

## Technical Context

**Language/Version**: Rust 2024 workspace (`vizier-core`, `vizier-http`, `vizier`); TypeScript, React 19, React Router v7 and Tailwind v4 in `webui/` and `design/`

**Primary Dependencies**: existing only: tokio, `tracing`, `serde_json`, `utoipa`, the SQL store (`SqlConn`/`Dialect`), the Fleet bus transport, Zustand, vitest. **No new crate or package.**

**Storage**: one new table `detached_call` through `SqlStorage` (SQLite and PostgreSQL by the shared trait, a numbered PostgreSQL migration `0008`, idempotent SQLite schema init); the `background_job` and `background_piece` tables are dropped. Outcomes live in the row, not in blobs.

**Testing**: `cargo test -p vizier-core` (bridge, claim, fence, recovery), `cargo test -p vizier-http` (routes, conformance vectors), `webui` vitest and `npm run typecheck`, `cargo clippy --workspace`; end to end: dummyplug steps in [quickstart.md](quickstart.md) per the constitution's e2e gate (the guidance-wording step is marked live).

**Target Platform**: the server (Linux, macOS, Docker) and the desktop app (Windows included); nothing platform-specific is added.

**Project Type**: Rust workspace, server plus embedded WebUI plus design-system submodule

**Performance Goals**: a detach returns in under 1 s (SC-001); a ready result reaches the agent in the first round that starts after it is final (SC-003); a claim is one indexed transaction.

**Constraints**: delivery exactly once across restarts and ownership changes (spec 057 fences on every write); no new required service (Principle III); the same hooks, redaction, timeout and trace as an inline call (FR-006, FR-009).

**Scale/Scope**: tens of calls per conversation at most (depth 3, outcomes capped at 32 KiB, the list route returns running calls plus the last 50 final ones). Removes about 4,000 lines of job code and adds an estimated 1,500.

## Constitution Check

*GATE: passed before Phase 0, re-checked after Phase 1.*

| Principle or rule | Verdict |
|---|---|
| **I. Lean by default** | Pass. One reserved argument instead of a second feature; one table replaces two; one shared invocation function serves both the inline and detached paths; no dependency, flag or config added; net lines go down. |
| **II. Trait-based extensibility** | Pass. "May this tool be detached" is the trait method `detachable()` (default true) surfaced on `CatalogueEntry`, so no code branches on a tool name; the existing `tool == "parallel_subagents"` name check in the bridge is **deleted**. New storage is a `DetachedCallStorage` supertrait member of `VizierStorageProvider`, implemented once on `SqlStorage`. |
| **III. Self-contained default** | Pass. SQLite default untouched; PostgreSQL gets the same table by the shared storage layer and an ordered migration. |
| **IV. Portability** | Pass. No OS-specific code. Quickstart gives the Windows sleep form. |
| **V. Unified errors and observability** | Pass. `crate::Result`, `tracing` only; spans `detached_call {tool}` as children of the turn's trace; refusals to the model are `status`/exception text, as the other code-mode tools. |
| Design before UI code | Pass. Drawn in FigCraft (`Vizier shell.figcraft`, Shell · chosen, frames "058 · A" and "058 · B", saved 2026-11-13), including the collapsible entries, trash cancel and Inspect view. |
| Optional capabilities behind a flag | N/A. Not optional; the capability is core engine behaviour and no flag changes an API shape. |
| Roles and components independent | Pass. `agents/detached/` depends on trait seams (`VizierStorage`, `VizierTransport`) as `background/` did; testable alone. The gateway side reaches it only through the `AgentsClient`/service seam. |
| Verify what a change touches | Pass. Touches `vizier-core`, `vizier-http`, `webui`, `design`; the PR runs those jobs and the `gate`. `Cargo.lock` and `Cargo.toml` are untouched. |
| Dummyplug e2e | Pass. Quickstart steps 2 to 11, 14. |
| Submodule branches | `design` and `webui` get branch `058-detached-tool-calls` (`just checkout`); commit order design → webui → vizier. |
| Conventional commits | Pass. Commits and the PR title: `feat(agents)!: replace background jobs with detached tool calls` (breaking). |
| Row ownership (CLAUDE.md, spec 057) | Pass. Every write of in-flight work calls `check_fence`; `fence_tests.rs` gets one row per method; recovery runs on claim. |

**Post-design re-check**: unchanged. The one place Principle I needed care is outcome storage (D4): a row column, 32 KiB cap, no blob.

## Project Structure

### Documentation (this feature)

```text
specs/058-detached-tool-calls/
├── plan.md              # This file
├── research.md          # Phase 0: D1–D13
├── data-model.md        # Phase 1: table, state machine, claim, views
├── quickstart.md        # Phase 1: dummyplug e2e and test commands
├── contracts/
│   ├── detached-flag.md       # the option, refusals, delivery text, the three tools
│   └── http-and-frames.md     # routes, the session frame, notification
├── checklists/requirements.md
└── tasks.md             # /speckit-tasks, not this command
```

### Source Code (repository root)

```text
crates/vizier-core/src/
├── agents/
│   ├── detached/                       # NEW (replaces agents/background/)
│   │   ├── mod.rs                      # DetachedCalls: launch, run task, cancel, cascade, claim wrapper
│   │   └── delivery.rs                 # render_delivery: the text of a round message and a wake-up turn
│   ├── tools/
│   │   ├── detached.rs                 # NEW: list/get/cancel_detached_call
│   │   ├── python/bridge.rs            # reserved kwarg, refusals, shared invocation function
│   │   ├── mod.rs                      # CatalogueEntry.detachable, VizierTool::detachable(), RETIRED_TOOL_NAMES
│   │   ├── subagents.rs                # drop background_subagents; keep parallel_subagents
│   │   └── background_jobs.rs          # DELETED
│   ├── agent/mod.rs                    # prompt_with: drain at the round boundary
│   ├── agent/system_prompt/boot.rs     # guidance: detach, deliveries, inspect/cancel
│   ├── process.rs                      # DetachedDelivery arm (claim, turn only if rows)
│   └── background/                     # DELETED
├── schema/{detached.rs (NEW), background.rs (DELETED), request.rs, mod.rs}
├── storage/{detached_call.rs (NEW), sqlite/detached_call.rs (NEW), fence.rs, mod.rs,
│            sqlite/fence_tests.rs, background_job.rs + sqlite/background_job.rs (DELETED),
│            sql/postgres/migrations/0008_detached_calls.sql (NEW)}
├── service/{detached.rs (NEW), jobs.rs (DELETED)}
├── recovery.rs, recovery_tests.rs      # interrupted sweep + delivery re-poke
├── notifications/{mod.rs, text.rs}     # background_work category
├── transport/mod.rs                    # SessionFrame::DetachedCall
├── channels/woken.rs                   # reads the new woken-turn frames
├── usage/mod.rs, utils/mod.rs, dependencies.rs
└── tests: tests/agent_host.rs, tests/dummyplug_e2e.rs

crates/vizier-http/src/
├── api/v1/agents/{detached.rs (NEW), jobs.rs (DELETED), mod.rs, channel.rs}
├── topic.rs, state.rs, lib.rs          # frame name, state wiring
└── tests/conformance/groups/detached.rs (NEW) + pending.txt

design/src/components/
├── BackgroundWork/                     # NEW (replaces BackgroundJob/): collapsible entry, trash, Inspect details + stories
└── StatusStrip/                        # id "jobs" -> "work"

webui/app/
├── screens/chat/ChatStatusStrip.tsx    # the Background work item and panel
├── lib/{frames.ts, statusFilters.ts, trail.ts, messages.ts} (+ tests)
├── api/{frames.ts, endpoints/chat.ts, endpoints/notifications.ts, schema.d.ts}  # regenerated
└── screens/chat/MessageList.tsx

docs/src/configuration/agents.md        # the detached option
CLAUDE.md                               # rewrite the Background jobs paragraphs; SPECKIT pointer
crates/vizier-core/public-api.txt       # regenerated
```

**Structure Decision**: a new `agents/detached/` module replaces `agents/background/` rather than growing it, since the unit of work changed from "sub-agent piece" to "tool call" and almost nothing of the old runner survives; `Canceller`, the depth constant and the timeout limits move over. The WebUI keeps its tray's screen and store; the design component is replaced because its shape (collapsible, trash, Inspect) differs from `BackgroundJob`.

## Phases and order of work

1. **Storage and schema**: `detached_call` table, `DetachedCallStorage`, fenced writes, the claim, migrations (drop old), fence tests. Landed first because everything reads it.
2. **Engine**: shared invocation function, reserved kwarg, `DetachedCalls`, task and cancel, `detachable()`, refusals, recovery. Bridge tests.
3. **Delivery**: claim at the round boundary, `DetachedDelivery` request, `render_delivery`, guidance text, retired names.
4. **Tools, service, HTTP, frames, notifications**, conformance vectors.
5. **Removal**: delete the old module, tools, tables, routes, frames, request kind and every reference (SC-007 grep).
6. **Design, then WebUI**: `design/` component and stories, then `webui/` screens, store, tests, regenerated OpenAPI; bump pins design → webui → vizier.
7. **Docs, CLAUDE.md, `public-api.txt`, #82 entries** (the two `DROP TABLE` migrations, the retired names), full quickstart run.

## Risks

- **Hooks after the turn ends** (D3): a detached task whose turn is gone must not fail on frame sends. Covered by quickstart step 6 and a unit test with a closed sender.
- **Unknown notification category** (D10): the WebUI must tolerate old `background_job` rows for the retention window; verify first, skip them at read time if it does not.
- **Breaking change**: agents whose saved guidance or users' habits call `background_subagents` get the retired-name message; release notes say so (`feat!`).
- **Group scope** is one script run, not a turn (D6); documented in the contract and guidance.

## Complexity Tracking

No constitution violations. The one structural addition, the `DetachedDelivery` request kind, replaces `BackgroundReport` one for one.
