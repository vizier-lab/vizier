# Implementation Plan: Background Subagent Results

**Branch**: `012-background-subagent-results` | **Date**: 2026-10-09 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/012-background-subagent-results/spec.md`

## Summary

`paralel_subtasks` becomes non-blocking, and both it and `delegate_agent` now report back. A new `BackgroundJobs` service in `VizierDependencies` does the following:

- launches each job's pieces through the transport;
- awaits them under a per-piece time limit, aborting a piece that times out;
- records jobs and pieces in two new SQLite tables;
- delivers one structured report into the originating session, as a new request kind (`BackgroundReport`), which wakes the agent through the existing per-session queue.

**Delivery.** Each report is sent with a response sender that republishes its frames onto a new session-event broadcast, which WebUI sockets subscribe to and filter by topic. So a woken reply streams into an open WebUI topic. Everywhere else (Discord, Telegram, task, dream, inter-agent and subagent conversations) the agent is woken and its reply lands in history only. Rendering on Discord and Telegram is deferred.

**Cancelling.** Two new agent tools, `list_background_jobs` and `cancel_background_job`, plus a tray ✕ backed by `POST …/jobs/{id}/cancel`, stop a job's pieces with the existing `abort` command. The cancel cascades to nested jobs and never sends a report. A single guarded database transition ensures a job ends either reported or cancelled, never both.

**Nesting.** Depth travels on the request (`background_depth`) and is capped at 3.

**The WebUI.** It gets job frames and woken-turn frames over the topic's WebSocket, plus read and cancel routes. On top of those it renders the tray docked above the input (with ✕), a piece side panel, a report entry and a topic-list badge.

## Technical Context

**Language/Version**: Rust 2024 edition (stable toolchain as pinned by the repo); TypeScript 5 / React 19 for the WebUI

**Primary Dependencies**: no new crates. Uses `tokio` (the `sync` feature is added explicitly for `broadcast`; it is already compiled in through feature unification), `flume`, `axum`, `rusqlite`, `uuid`, `serde`, `rig-core`, `twilight`, `teloxide`. WebUI: React Router 7, Zustand, Tailwind 4, with the existing `SlideOver`/`ActivityTrail`/`groupHistory`.

**Storage**: embedded SQLite, with two new tables (`background_job`, `background_piece`) behind a new `BackgroundJobStorage` trait. No result text is stored: answers live in each piece's session history.

**Testing**: `cargo test` covers the pure parts: report rendering and truncation, the depth rule, mapping piece frames to states, and the storage round-trip and sweep. `cargo clippy`; `cd webui && npm run typecheck`. End-to-end: the dummyplug steps in `quickstart.md`, per the constitution's e2e gate.

**Target Platform**: the existing single binary (Linux, macOS, Windows; musl cross targets). No platform-specific code is added.

**Project Type**: single Rust binary with an embedded web frontend (`src/` + `webui/`)

**Performance Goals**: the tool call returns in under 1 second (SC-001). Tray updates reach an open tab within 2 seconds of a state change (SC-007). One broadcast send per job state change, filtered per socket.

**Constraints**:

- A report never interrupts a running turn.
- Exactly one report per job.
- No duplicate frames to a tab.
- Must keep working with zero configuration (Principle III), and must not touch Discord or Telegram behaviour for ordinary messages.

**Scale/Scope**: jobs in the tens per agent per day, with 1–20 pieces each. WebSocket connections at WebUI scale (a handful per instance).

## Constitution Check

*GATE: must pass before Phase 0 research. Re-checked after Phase 1 design (below).*

| Principle | Assessment |
|---|---|
| **I. Lean by Default** | Each addition answers a requirement. Two tables (needed for reload, lost state, drill-down authorization and the cancel race guard: research D3, D12). One broadcast, serving both job frames and woken-turn delivery (D1, D6). No `ReplySink` trait while it would have only one implementation (D1). The only in-memory state is one cancel signal per live runner (D12). One request field and one content variant (D4, D5). No new crates. The time limit and depth stay constants, plus an optional tool argument; there are no per-agent config knobs (the spec defers those). ✅ |
| **II. DRY via Trait-Based Extensibility** | Delivery is the same for every conversation (a broadcasting sender), with no `match` on `VizierChannelId` anywhere in the job runner. Both launching tools share one `BackgroundJobs::launch`. The agent tool and the HTTP route share one `BackgroundJobs::cancel`. Report entries and cancel results share one renderer. Storage follows the `VizierStorageProvider` supertrait pattern. ✅ |
| **III. Self-Contained Runtime** | Embedded SQLite only. No external service. WebUI assets are still embedded at build time. ✅ |
| **IV. Portability** | No OS-specific code. The quickstart's use of `shell_exec` goes through the existing shell abstraction. ✅ |
| **V. Unified Errors & Observability** | Tool and storage paths return `crate::Result` or follow the existing storage trait signatures (`anyhow::Result`, as `TaskRunStorage` does). Recording and delivery failures are logged with `tracing` and never abort a turn. No `unwrap` outside tests. ✅ |
| **Quality gates** | `cargo clippy`, `cargo test`, `npm run typecheck`, and the dummyplug quickstart, run against the running binary. Commit type is `feat:`. The `paralel_subtasks` output changes from a results list to an acknowledgement, which is a behaviour change for agents, so the commit is flagged `[**breaking**]`. ✅ |

**Post-design re-check**: the design artifacts introduce nothing beyond the table above, and Complexity Tracking stays empty.

Spec amendments made during planning:

- **D9**: an agent respawn yields a delivered report with the cut-off pieces marked `failed`, rather than lost work.
- **Decided 2026-10-09**: Discord and Telegram are woken but nothing is posted to them; cancelling was added as User Story 6, with the agent tools and the tray ✕, and with no wake on cancel.

## Project Structure

### Documentation (this feature)

```text
specs/012-background-subagent-results/
├── plan.md                      # this file
├── research.md                  # D1–D11
├── data-model.md                # tables, state machine, types, type changes
├── quickstart.md                # dummyplug e2e script
├── contracts/
│   ├── agent-tools.md           # paralel_subtasks / delegate_agent definitions
│   ├── background-report.md     # what the woken agent sees
│   ├── http-api.md              # job routes, topic badge, WebSocket frames
│   └── webui-tray.md            # store, tray, piece panel, report entry
├── checklists/requirements.md
└── tasks.md                     # /speckit-tasks (not created here)
```

### Source Code (repository root)

```text
src/
├── agents/
│   ├── background/                  # NEW — BackgroundJobs: launch, runner, cancel (+cascade), delivery
│   │   ├── mod.rs
│   │   └── report.rs                # pure: entries → BackgroundReport, truncation, Display; cancel result (unit-tested)
│   ├── process.rs                   # BackgroundReport joins the Chat arm and both ThinkingStart matches
│   ├── agent/mod.rs                 # ToolContext.background_depth from req.background_depth
│   └── tools/
│       ├── mod.rs                   # ToolContext + background_depth; tool constructors take deps
│       ├── subtasks/mod.rs          # thin: validate → background_jobs.launch(Batch)
│       ├── consult/mod.rs           # DelegateAgent thin: validate → launch(Delegation); ConsultAgent unchanged
│       └── background_jobs.rs       # NEW — list_background_jobs, cancel_background_job
├── channels/
│   └── http/api/v1/agents/
│       ├── channel.rs               # WS subscribes to session_events; TopicEntry.running_jobs
│       └── jobs.rs                  # NEW — GET jobs, job, piece history; POST cancel
├── schema/
│   ├── background.rs                # NEW — job/piece/report types, snapshot
│   ├── request.rs                   # + background_depth, + BackgroundReport variant, frontmatter
│   └── session.rs                   # (no change; Subagent/InterAgent reused)
├── storage/
│   ├── background_job.rs            # NEW — BackgroundJobStorage trait
│   ├── mod.rs                       # supertrait + VizierStorage forwarding
│   └── sqlite/{mod.rs, background_job.rs}   # DDL + impl
├── transport.rs                     # session_events broadcast
└── dependencies.rs                  # construct BackgroundJobs; startup sweep

webui/app/
├── hooks/backgroundJobStore.tsx     # NEW
├── hooks/connectionStore.tsx        # route {background_job} frames to the store
├── components/BackgroundJobTray.tsx # NEW — incl. ✕ with inline confirm
├── components/BackgroundReportItem.tsx  # NEW
├── components/MessageItem.tsx       # render background_report requests via BackgroundReportItem
├── routes/chat.tsx                  # mount tray above input; piece SlideOver; topic badge; queue report on live
├── services/vizier.ts               # job routes + cancel
└── interfaces/types.ts              # snapshot/report types, request union, running_jobs
```

**Structure Decision**: this is the existing single-project layout. The one new backend module is `src/agents/background/`. It sits beside `agents/tools/` because it is agent-runtime machinery that the tools call into, not a tool itself. The new HTTP routes go in their own `jobs.rs` so that `channel.rs` changes only in the WebSocket handler and `TopicEntry`.

## Implementation Order

Each step leaves the binary working:

1. **Schema and storage**: types, tables, the guarded transition and the sweep, with unit tests for the round-trip, the sweep, and the guard (one winner).
2. **Report rendering** (`report.rs`), the request variant, frontmatter, and `background_depth` → `ToolContext`, with unit tests.
3. **`BackgroundJobs`**: launch, runner, time-out and abort, `reporting` before delivery, delivery with retry and a broadcasting sender. Then switch both tools over and update their descriptions. *At this point quickstart steps 1–6 and 9 pass, with history-only delivery.*
4. **Cancel**: `BackgroundJobs::cancel` (signal, abort pieces, cascade) and the `list_background_jobs` and `cancel_background_job` tools. *Quickstart step 10, agent part.*
5. **Session-event broadcast** and the WebSocket fan-out, then the job routes (including cancel) and `TopicEntry.running_jobs`.
6. **WebUI**: types, store, tray with ✕, report entry, cancel entry, piece panel and badge. *Quickstart steps 7, 8 and 10 (tray part).*
7. **CLAUDE.md**: update the tools paragraph (the four background job tools, the session-event broadcast, the new tables), then run clippy, test, typecheck and the full quickstart.

## Complexity Tracking

No constitution violations to justify.
