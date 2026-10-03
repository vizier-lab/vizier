# Implementation Plan: Task Run Results, Requester and Framing

**Branch**: `011-task-completion-reports` | **Date**: 2026-10-04 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/011-task-completion-reports/spec.md`

## Summary

Scheduled tasks are write-only today. A one-time task is deleted *before* it is dispatched, so
nothing survives to hang a result on; every run's conversation is already persisted but nothing
links it to the task or shows it; and a scheduled run reaches the agent looking like a message a
person just sent, so models answer conversationally instead of reporting.

The implementation is four changes against existing seams:

1. **Stop destroying evidence.** A fired one-time task is deactivated, not deleted, and a new
   `task_run` table records one row per firing — opened before the agent starts, closed from the
   response channel the way `DreamScheduler` already does.
2. **Give each run its own conversation.** `VizierChannelId::Task::to_slug()` renders its
   timestamp in full. Today it renders sub-second nanos of a second-truncated time, which is
   always `0`, so every run of a task collides into one session.
3. **Surface runs.** Two routes on the existing task router, a paginated run list and one run's
   history, plus two agent tools (list, then fetch) that return responses only.
4. **Tell the agent where it is.** A constant system message appended only for a scheduled run,
   and a frontmatter that attributes the run to the scheduler on behalf of the requester — a new
   `Requester` enum replacing today's unvalidated `Task.user` string.
5. **Stop the names lying.** `VizierRequestContent::Task` is renamed `Unattended`, because two of
   its three construction sites are the dream cycle, and `VizierSession::is_scheduled_task()`
   gives the framing's real question one home.

No new dependency, no new background task, no new storage backend. One new table, one new storage
trait, one changed slug rendering.

## Technical Context

**Language/Version**: Rust (edition 2024, nightly features already in use — `Duration::from_mins`),
plus TypeScript for the WebUI

**Primary Dependencies**: all already present — `rusqlite` (bundled), `flume` (the transport and
the response channel), `chrono`, `croner`, `serde`/`serde_yaml`, `axum`, `utoipa`, `schemars`;
React Router v7 + React 19 + Tailwind v4 in `webui/`

**Storage**: embedded SQLite. One new table (`task_run`) in the existing
`CREATE TABLE IF NOT EXISTS` batch; `Task`'s field change needs no DDL because `task.data` is a
JSON blob

**Testing**: `cargo test` for the pure and storage-level pieces (slug rendering, framing
selection, cursor over same-millisecond runs, the startup sweep, the overlap invariant);
end-to-end via dummyplug agent steps scripted in [quickstart.md](./quickstart.md) per the
constitution's e2e gate; one step marked as requiring a live provider, because report-versus-reply
depends on real model output that dummyplug cannot produce

**Target Platform**: Linux/macOS/Windows single binary, same as today; nothing platform-specific

**Project Type**: single Rust binary serving a bundled React WebUI

**Performance Goals**: opening a task loads one page of runs regardless of history length
(SC-006); recording a run adds no observable delay to the agent starting work (SC-005). Note this
feature *reduces* a cost — per-run sessions stop a recurring task's context from compounding every
fire

**Constraints**: the framing string must be constant per run so the prompt prefix stays reusable
(FR-033); recording failures must never block a run (FR-017); every run is retained, nothing
trims (FR-011)

**Scale/Scope**: a minutely task accrues ~525k runs a year, which is why the run list is key-set
paginated and the agent-facing tools are bounded; ~33 FRs across the scheduler, storage, HTTP,
agent tools, system prompt and WebUI

## Constitution Check

*GATE: passed before Phase 0; re-checked after Phase 1 — see below.*

| Principle | Assessment |
|---|---|
| **I. Lean by Default** | One new table, one new trait, two new tools, one new prompt section. Three things were actively *removed* from scope for this principle: a second `origin` field (derivable from the call path), a retention/trim policy (pagination makes it unnecessary, and nothing else in the codebase trims), and the success/failure/blocked status taxonomy (the agent's response already says it). No new dependency; `flume`, `rusqlite` and `chrono` are already in the tree. |
| **II. DRY via Trait-Based Extensibility** | `TaskRunStorage` is a new trait composed into `VizierStorageProvider` and implemented for `SqliteStorage`, not a branch inside existing dispatch. The two tools implement `VizierTool` and are registered in `VizierTools::new()`. Run completion reuses the `send_request` response channel that `DreamScheduler` already uses rather than inventing a second completion mechanism. The run-list cursor deliberately copies `HistoryQuery`'s `before`/`before_seq` shape instead of introducing a second pagination idiom. **One risk flagged**: framing selection must branch on the session channel; branching on `VizierRequestContent::Task` would also catch the dream cycle, which sends that same content kind. |
| **III. Self-Contained Runtime** | Nothing added needs a network or an external service. Storage stays embedded SQLite. Runs are visible with zero configuration; no opt-in required. |
| **IV. Portability** | No OS-specific code, no new filesystem assumptions, no new system libraries, so `Cross.toml` targets are untouched. |
| **V. Unified Errors & Observability** | New fallible paths return `crate::Result<T>` via `VizierError`; external errors converted with `throw_vizier_error`. All new logging through `tracing`. Per FR-017, a recording failure is logged and swallowed rather than propagated into the run — the same posture `write_memory` already takes for chunking/indexing failures. |

**Gate result: PASS.** No violations to justify, so Complexity Tracking is omitted.

### Re-check after Phase 1

Design did not add surface. The one judgement worth recording: `TaskRun` stores a pointer to the
run's conversation and *not* the response text (data-model). That is Principle I applied to data —
a second copy would drift from the conversation it came from, and it is the same reason the spec
removed previews. The response is sliced from `session_history` on demand, mirroring how
`memory_passage` stores coordinates and never text.

## Project Structure

### Documentation (this feature)

```text
specs/011-task-completion-reports/
├── plan.md                              # This file
├── research.md                          # Phase 0 — 11 decisions, all unknowns resolved
├── data-model.md                        # Phase 1 — Requester, TaskRun, task_run, TaskRunStorage
├── quickstart.md                        # Phase 1 — dummyplug e2e script + live-provider step
├── contracts/
│   ├── http-api.md                      # run list, run history, changed TaskResponse, breaking POST
│   ├── agent-tools.md                   # list_task_runs, get_task_run_detail
│   └── scheduled-run-framing.md         # the system message, the frontmatter, the selection trap
├── checklists/requirements.md           # spec quality checklist (16/16)
└── tasks.md                             # Phase 2 — /speckit-tasks, NOT created here
```

### Source Code (repository root)

```text
src/
├── schema/
│   ├── task.rs                    # CHANGED  Task.user → Task.requester; + Requester, TaskRun, TaskRunState
│   ├── request.rs                 # CHANGED  Content::Task → Content::Unattended (breaking, no alias);
│   │                              #          frontmatter sender/task/requested_by for a scheduled run
│   └── session.rs                 # CHANGED  Task::to_slug() — full timestamp, not subsec nanos;
│                                  #          + VizierSession::is_scheduled_task()
├── storage/
│   ├── task_run.rs                # NEW      TaskRunStorage trait + VizierStorage forwarding
│   ├── mod.rs                     # CHANGED  compose TaskRunStorage into VizierStorageProvider
│   └── sqlite/
│       ├── mod.rs                 # CHANGED  task_run DDL + indices
│       ├── task_run.rs            # NEW      impl TaskRunStorage for SqliteStorage
│       └── task.rs                # CHANGED  delete_task also drops the task's runs
├── scheduler/
│   └── mod.rs                     # CHANGED  deactivate instead of delete; open/close runs; the
│                                  #          running row replaces the in-memory overlap set
├── dependencies.rs                # CHANGED  requester migration + startup sweep of open runs
├── agents/
│   ├── process.rs                 # CHANGED  the renamed variant in its match arm
│   ├── agent/
│   │   ├── mod.rs                 # CHANGED  prepare_system_prompts takes the run kind
│   │   └── system_prompt/
│   │       ├── mod.rs             # CHANGED  export the new section
│   │       └── scheduled_run.rs   # NEW      the constant framing text
│   └── tools/
│       ├── mod.rs                 # CHANGED  register both tools; add both to DREAM_TOOL_NAMES
│       └── scheduler/mod.rs       # CHANGED  + list_task_runs, get_task_run_detail
└── channels/http/api/v1/agents/
    └── task.rs                    # CHANGED  + /runs, + /runs/{run_id}/history; requester from
                                   #          the authenticated caller; TaskResponse gains last_run

webui/
└── app/
    ├── lib/trail.ts               # CHANGED  an unattended Request opens a turn without becoming
    │                              #          its request — makes the line-106 comment true
    ├── lib/trail.test.ts          # CHANGED  pin that: a run's trail has no request bubble
    ├── routes/chat.tsx            # CHANGED  renders only .chat/.audio_chat today, so any other
    │                              #          variant draws an empty bubble
    ├── interfaces/types.ts        # CHANGED  Task.requester, TaskRun, TaskRunState;
    │                              #          RequestContent `{ task }` → `{ unattended }`
    ├── services/vizier.tsx        # CHANGED  listTaskRuns, getTaskRunHistory; drop `user` from create
    └── routes/tasks.tsx           # CHANGED  last-run column; latest-run block; paginated run list
                                   #          with per-run trail via trail.ts's groupHistory;
                                   #          remove the free-text User input
```

**Structure Decision**: the existing layout absorbs this feature without a new module tree. Each
change lands in the module that already owns its concern — scheduler owns firing, storage owns
persistence, `system_prompt/` owns prompt sections, `tools/scheduler/` owns task tools, and
`routes/tasks.tsx` owns the task screen. The only genuinely new files are the storage trait and
its sqlite impl (a new storage concern, per Principle II) and the framing section.

## Risks

One risk, and one accepted consequence.

### Risk — framing must be selected on the channel, not the content kind

Two different things are called `Task`, and they answer different questions:

| | Question | Scheduled task | Dream cycle |
|---|---|---|---|
| `VizierRequestContent::Task` | *how was the prompt written?* | `Task` | **`Task`** |
| `VizierChannelId::Task` | *where is this happening?* | `Task` | `Dream` |

The content kind does not mean "a scheduled task". It means "a machine wrote this prompt rather
than a person typing", and the dream cycle qualifies too — it dispatches its work as
`VizierRequestContent::Task(EXTRACTION_PROMPT)` (`scheduler/dream/mod.rs:195`, `:343`).

So selecting the framing on the content kind double-frames every dream, which already carries its
own instructions in that prompt body. The agent would get two briefs that disagree: write up
insights for your own memory, and also your final message is a report a person will open from a
task screen.

Selection must therefore ask *where*, not *how* — and gets one home so it cannot be written any
other way by accident:

```rust
session.is_scheduled_task()     // matches!(self.1, VizierChannelId::Task(..))
```

It is easy to get wrong because the content kind is what you are already holding when handling a
request, and its name matches what you are looking for. `process.rs:1065` already groups `Task`
with `Prompt` in one arm for the same reason — the content kind was never a discriminator for
scheduled runs.

Two mitigations, in the order they matter: `is_scheduled_task()` removes the opportunity, and
renaming the variant to `Unattended` (research Decision 12) removes the temptation. Also covered
by the framing contract, the spec's edge cases, and a dedicated unit check in quickstart §9.

### Accepted — legacy task conversations keep their old key

`VizierChannelId::to_slug()` is the storage identity for sessions, history *and* session files
(`storage/sqlite/{session,history,session_file}.rs`), so changing the `Task` arm leaves every
pre-existing task conversation under its old `task__{slug}__0` key. Those surface as one legacy
run per task and are not split retroactively — splitting would mean inferring run boundaries from
timestamps inside a merged conversation (research Decision 2).

Accepted as part of the breaking release this feature ships in; no migration is planned.

### Accepted — legacy `Request` history rows for tasks and dreams stop parsing

Renaming `VizierRequestContent::Task` to `Unattended` without a serde alias changes the persisted
tag, so stored `Request` entries carrying the old `"task"` key no longer deserialize.
`parse_history_row` drops an unparseable row **silently** (`storage/sqlite/history.rs:32`), so this
is invisible rather than loud.

Bounded: only `Request` entries whose content was `Task` are affected — the opening row of each
legacy task run and each legacy dream request. Agent responses in those same sessions parse
unchanged, so a legacy task conversation keeps its reports and loses its prompt row.

Accepted by decision as part of the same breaking release (research Decision 12). The silence is
pre-existing behaviour; a `tracing::warn!` on a dropped row would be a cheap separate improvement
and is not required here.

### WebUI consequences of the rename

Three, found by inspection:

| Where | What |
|---|---|
| `interfaces/types.ts:532` | `{ task: string }` in the `VizierRequestContent` union. Nothing reads it, so `npm run typecheck` will not fail — it would simply describe a wire format that no longer exists |
| `routes/chat.tsx:1661` | reads only `request.content?.chat` and `audio_chat`, so any other variant renders an empty bubble. Never mattered while task sessions were undisplayed |
| `lib/trail.ts:106` | **the one that matters.** The comment states that an agent-initiated turn has no request, but `groupHistory` assigns `turn.request` for *any* `Request` entry. That holds today only because no task or dream history has ever been passed to it — and rendering run history through `groupHistory` is precisely why reusing it was attractive. A task run's history opens with a `Request` entry (`agents/agent/mod.rs:420`), so without a change every run's trail gains a nameless empty bubble, contradicting the spec's decision that the run view shows no request (the instruction is already above it) |

The fix for the third is to open the turn without adopting the entry as its request, which makes
the existing comment true rather than aspirational and keeps the turn boundary. `trail.ts` is pure
and already unit-tested (`lib/trail.test.ts`), so it is pinned there.

### Breaking-change summary

Three independent reasons this feature ships as a breaking release, all needing
`[**breaking**]` on their commits per the `git-cliff` convention:

| | What breaks |
|---|---|
| `Task.user` → `Task.requester`, and `POST /tasks` ignoring a caller-supplied `user` | the task-creation API's contract |
| `VizierChannelId::Task::to_slug()` rendering | legacy task conversations keep an orphaned key |
| `VizierRequestContent::Task` → `Unattended` | legacy `Request` history rows for tasks and dreams |
