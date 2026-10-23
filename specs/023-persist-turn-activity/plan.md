# Implementation Plan: Durable Turns: Live Progress That Survives Navigation and Restarts

**Branch**: `023-persist-turn-activity` | **Date**: 2026-10-23 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/023-persist-turn-activity/spec.md`

## Summary

Work in progress lives only in memory. A turn's steps reach history only when it ends,
queued requests sit in a `HashMap` and a flume channel, and a restart sweeps background jobs and
task runs to `interrupted`. As a result, a WebUI page reopened mid-turn shows only later steps,
and a restart silently loses running turns, queues and agent work.

**Approach**: make progress durable at two levels, and read everything back from them.
1. **Steps**: `prompt_with` saves each step to history as it happens (same entries, same order)
   and publishes it as a `history_entry` frame. The WebUI builds the running trail from
   history, merging by uid, so any page, tab or reload shows it all exactly once.
2. **Requests**: `send_request` records every durable request in a new `session_turn` table
   (`queued` → `running` → deleted at the end). On startup, remaining rows are re-sent through
   the normal loop. A running row resumes from its last recorded step: completed calls are
   kept, and cut-off calls are sealed with a restart notice, never re-run. Background jobs and
   task runs backed by a row re-attach by session instead of being swept. Replies go out on
   the session broadcast (the woken-turn path) marked `resumed`.

## Technical Context

**Language/Version**: Rust 2024 (crate edition); TypeScript 5 / React 19

**Primary Dependencies**: existing only: `rusqlite` (bundled), `tokio` broadcast, `flume`, `serde`. WebUI: React Router v7 and the store pattern.

**Storage**: SQLite. One new table, `session_turn`, created in an `init_turn_schema`. It is a new table, not a data migration. Session history gets the same entries, written earlier.

**Testing**: `cargo test`, with unit tests for `TurnStorage`, `note_resume`, the resume history split and seal, `is_durable`, and per-step entries matching `messages_to_history_entries`. `trail.test.ts`. End-to-end: dummyplug steps in [quickstart.md](quickstart.md), scripted as `scripts/durable-turns-e2e.mjs`, using `shell_exec sleep` to hold turns and `kill -9`/`just shutdown` to restart.

**Target Platform**: the vizier binary (all `Cross.toml` targets) and the bundled WebUI

**Project Type**: web service + bundled web app

**Performance Goals**: one extra SQLite insert per step and two row writes per turn. That is negligible next to model and tool latency. Startup recovery is one indexed scan per agent.

**Constraints**: no re-execution of a tool call because of a restart (SC-003); exactly-once job reports (FR-012); durability must never block or fail a request (log and continue)

**Scale/Scope**: about 15 files across `transport.rs`, `agents/process.rs`, `agents/agent/mod.rs`, `agents/background/`, `scheduler/`, `storage/`, `dependencies.rs`, the channel renderers, and four WebUI files

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle / gate | Status |
|---|---|
| I. Lean by Default | ✅ No new dependency. One table, and finished rows are deleted rather than kept. The WebUI *loses* its `liveTrail`/`closedTrails` machinery, because the trail comes from history in one way. Resumption reuses the existing loop, `seal_tool_calls`, the woken-turn delivery and `background::classify`. |
| II. Trait-based extensibility | ✅ `TurnStorage` is a storage concern added to the `VizierStorageProvider` supertrait like every other. Durability sits at `send_request` and the process loop, never per channel, provider or tool. `broadcast_sender` moves to the transport so there is one copy, and `await_session_outcome` is shared by background jobs and the scheduler. |
| III. Self-contained runtime | ✅ Embedded SQLite only. |
| IV. Portability | ✅ No platform-specific code. The e2e script takes start/stop commands from the environment. |
| V. Unified errors & observability | ✅ Journal and step-save failures are `tracing::warn!`/`error!` and never fail the request. Recovery logs counts like the existing sweeps. |
| Migration/back-compat tracking (#82) | ✅ Nothing legacy is added. Pre-upgrade work has no rows and falls through to the existing sweeps, which stay. |
| `cargo clippy` / `cargo test` / `npm run typecheck` | Required (tasks). |
| Dummyplug e2e gate | ✅ quickstart §1–§10. §11 is a WebUI and channel walkthrough on a live provider, marked as such. |

**Post-design re-check**: unchanged. Complexity Tracking is empty.

## Project Structure

### Documentation (this feature)

```text
specs/023-persist-turn-activity/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/websocket-and-replies.md
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
src/
├── storage/turn.rs, storage/sqlite/turn.rs      # NEW TurnStorage + session_turn table
├── storage/mod.rs                               # supertrait + VizierStorage forwarding; delete_session/delete_agent clean-up
├── transport.rs                                 # holds storage; send_request journals; resend; broadcast_sender (moved); await_session_outcome; SessionFrame::HistoryEntry
├── schema/request.rs, schema/response.rs        # is_durable; VizierResponse.resumed
├── agents/agent/mod.rs                          # step recorder in prompt_with; chat saves only Response/Error; resume(); RESTART_NOTICE
├── agents/process.rs                            # envelope turn_id/resumed; start/finish/cancel rows; resumed jobs; crash-loop guard; hand_on_queued via resend
├── agents/mod.rs                                # boot: re-send pending turns before supervisors build
├── agents/background/mod.rs                     # recover(): re-attach running/reporting jobs; uses transport.broadcast_sender
├── scheduler/mod.rs                             # recover(): close resumed task runs via await_session_outcome
├── dependencies.rs                              # build transport with storage; sweeps skip work backed by rows; scratch/is_thinking clean-up respects rows
├── channels/http/api/v1/agents/channel.rs       # forward HistoryEntry as `history_entry`
└── channels/{discord,telegram}/                 # render the `resumed` line

webui/app/
├── hooks/connectionStore.tsx                    # history_entry frames → listeners
├── lib/trail.ts (+ trail.test.ts)               # mergeEntries by uid; open turn's trail
├── routes/chat.tsx                              # read history after socket opens; merge entries; trail from open turn; drop liveTrail/closedTrails
└── components/chat_bubble.tsx                   # "continued after a restart" label

docs/src/api-integration/rest-api.md             # history_entry, resumed, durability note
scripts/durable-turns-e2e.mjs                    # quickstart §1–§10
```

**Structure Decision**: progress is made durable where it already flows (history writes in the
round loop, requests at `send_request`), and recovery re-enters the existing process loop.
Nothing branches per channel except the one-line rendering of `resumed` in each renderer.

## Phasing (for /speckit-tasks)

1. **Foundation**: `session_turn` + `TurnStorage`, envelope fields, `is_durable`, `VizierResponse.resumed`, `SessionFrame::HistoryEntry`.
2. **US1 (P1)**: per-step history writes plus `history_entry` frames, then the WebUI merge by uid and the trail from history. This can ship on its own: it fixes the reported bug with no restart work.
3. **US2 + US3**: journaling in `send_request`, row lifecycle in the loop, startup re-send, `resume()`, the crash-loop guard, resumed delivery and rendering, and the startup clean-up changes.
4. **US4 + US5**: `await_session_outcome`, background job and task run recovery, and sweeps limited to unbacked work.
5. **Polish**: docs, e2e script, `cargo clippy`/`test`, typecheck, quickstart.

## Complexity Tracking

No violations.
