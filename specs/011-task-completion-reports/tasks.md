---

description: "Task list for task run results, requester and scheduled-run framing"
---

# Tasks: Task Run Results, Requester and Framing

**Input**: Design documents from `/specs/011-task-completion-reports/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md),
[data-model.md](./data-model.md), [contracts/](./contracts/)

**Tests**: No TDD suite was requested. The test tasks below are exactly the ones
[plan.md](./plan.md)'s Testing field and [quickstart.md](./quickstart.md) §9 name — pure functions
and storage-level invariants where `cargo test` is the right home. Everything else is verified
end to end through the dummyplug steps, per the constitution's e2e gate.

**Organization**: Grouped by user story. Note the priority order is **US1, US5, US2, US4, US3** —
US5 is P1 alongside US1 by the spec's own reckoning, because the display and the framing only
deliver value together.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel — different files, no dependency on an incomplete task
- **[Story]**: US1, US2, US3, US4, US5
- Exact file paths are given in every task

## Path Conventions

Existing single-binary Rust project with a bundled React WebUI: `src/` and `webui/app/` at the
repository root. No new module tree — see plan.md's Structure Decision.

---

## Phase 1: Setup

**Purpose**: a known-good baseline before three breaking changes land

- [ ] T001 Establish a green baseline: `just install`, then confirm `cargo clippy`, `cargo test` and `cd webui && npm run typecheck` all pass on `c340e2b` before any change

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: the record of a run, and the renames every story reads from. Every user story needs
runs to exist, so run *recording* lives here rather than inside US1 — that is what keeps US2, US3
and US4 independently testable instead of all waiting on US1.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

### Schema

- [ ] T002 Add the `Requester` enum (`User(String)` / `Agent(AgentId)`) and replace `Task.user` with `Task.requester` in `src/schema/task.rs` per data-model.md
- [ ] T003 Add `TaskRun` and `TaskRunState` (`Running`/`Answered`/`NoResponse`/`Interrupted`) to `src/schema/task.rs` per data-model.md (same file as T002)
- [ ] T004 [P] Rename `VizierRequestContent::Task` to `Unattended` in `src/schema/request.rs` with **no serde alias** (research D12), and update every call site: `src/scheduler/mod.rs:175`, `src/scheduler/dream/mod.rs:195` and `:343`, `src/agents/process.rs:1065`, and the `Display` arm at `src/schema/request.rs:85`
- [ ] T005 [P] In `src/schema/session.rs`, change the `Task` arm of `to_slug()` to render the full timestamp instead of `timestamp_subsec_nanos()`, and add `VizierSession::is_scheduled_task()` per data-model.md

### Storage

- [ ] T006 Add the `task_run` table and both indices to the `CREATE TABLE IF NOT EXISTS` batch in `src/storage/sqlite/mod.rs`, using `INTEGER PRIMARY KEY AUTOINCREMENT` so `id` is monotonic for the page cursor
- [ ] T007 Create the `TaskRunStorage` trait and its `VizierStorage` forwarding impl in `src/storage/task_run.rs` with the seven methods in data-model.md
- [ ] T008 Implement `TaskRunStorage` for `SqliteStorage` in `src/storage/sqlite/task_run.rs`, with `list_task_runs` ordering `ran_at DESC, id DESC` and paging on `(before, before_id)`
- [ ] T009 Compose `TaskRunStorage` into `VizierStorageProvider` and register the module in `src/storage/mod.rs`
- [ ] T010 [P] Make `delete_task` also drop that task's runs in `src/storage/sqlite/task.rs` (FR-012, which also gives FR-013)

### Startup

- [ ] T011 [P] Add the requester migration to `src/dependencies.rs` beside the existing one-time migrations: every stored task's `user: "x"` becomes `requester: {"user": "x"}` (FR-031)
- [ ] T012 Add the startup sweep to `src/dependencies.rs`: every `task_run` left `running` becomes `interrupted` (FR-018, research D6)

### Scheduler

- [ ] T013 In `src/scheduler/mod.rs`, set `is_active = false` on a fired one-time task instead of calling `delete_task` (FR-001, FR-002) — the regression that started this feature
- [ ] T014 In `src/scheduler/mod.rs`, open a `task_run` before dispatch, pass a `flume::Sender<VizierResponse>` to `send_request`, and close the run from the final response in a spawned task, following `DreamScheduler`'s pattern at `src/scheduler/dream/mod.rs:178-220` (FR-002, FR-003, research D3)
- [ ] T015 In `src/scheduler/mod.rs`, close the run as `NoResponse` when `send_request` returns `Err` (agent not registered) or the channel closes with no message content (FR-024)
- [ ] T016 In `src/scheduler/mod.rs`, replace the `running: HashSet` overlap check with a `running_task_run` lookup, and delete the now-dead set (FR-010, research D5)
- [ ] T017 In `src/scheduler/mod.rs`, ensure a recording failure is logged via `tracing` and never aborts the run (FR-017)

### Foundational tests

- [ ] T018 [P] Unit tests in `src/schema/session.rs`: `to_slug()` yields a distinct key per timestamp, a legacy `task__{slug}__0` key still parses, and `is_scheduled_task()` is true for `Task` and false for `Dream`, `HTTP`, `System` and `Subagent`
- [ ] T019 [P] Unit tests in `src/storage/sqlite/task_run.rs`: a page cursor over several runs sharing one millisecond neither duplicates nor skips, and `has_more` is false only on the last page
- [ ] T020 [P] Unit tests in `src/storage/sqlite/task_run.rs`: at most one `Running` row per `(agent_id, task_slug)`, and the sweep turns every `Running` row into `Interrupted`

**Checkpoint**: runs are recorded and survive a restart; all five stories can now proceed in parallel

---

## Phase 3: User Story 1 - Read what a task produced (Priority: P1) 🎯 MVP

**Goal**: a fired task still exists and shows the agent's last response, with its requester.

**Independent Test**: schedule a one-time task and a recurring task on a dummyplug agent, let both
fire, then open each — both still exist, each shows the agent's final response and when it ran
(quickstart §1).

### Backend

- [ ] T021 [US1] Add a helper that resolves a `TaskRun`'s response — the last `SessionHistoryContent::Response` carrying `VizierResponseContent::Message` in that run's session — in `src/channels/http/api/v1/agents/task.rs`, reading through `list_session_history` with a `Task(slug, ran_at)` session
- [ ] T022 [US1] Extend `TaskResponse` in `src/channels/http/api/v1/agents/task.rs` with `requester` and `last_run` (`run_id`, `ran_at`, `finished_at`, `state`, `response`) per contracts/http-api.md, keeping `last_executed_at` for compatibility
- [ ] T023 [US1] Make `last_run` distinguish the three non-results in `src/channels/http/api/v1/agents/task.rs`: `null` for never-run, `state: "running"` with `response: null` while in flight, and `state: "no_response"` with `response: null` for a run that answered nothing (FR-005)
- [ ] T024 [US1] Take the requester from `AuthenticatedUser` in `create_task` and `update_task` in `src/channels/http/api/v1/agents/task.rs`, ignoring any `user` in the body rather than rejecting it (FR-028, **breaking**)
- [ ] T025 [US1] Set the requester from the agent's declaration in `schedule_one_time_task` and `schedule_cron_task` in `src/agents/tools/scheduler/mod.rs`: either the agent itself or a named person, with no account lookup (FR-029, FR-030)

### WebUI

- [ ] T026 [P] [US1] Add `Requester`, `TaskRun` and `TaskRunState` types and the `requester`/`last_run` fields to `Task` in `webui/app/interfaces/types.ts`, and rename the `{ task: string }` member of `VizierRequestContent` to `{ unattended: string }`
- [ ] T027 [P] [US1] Drop `user` from `createTask`/`updateTask` payloads in `webui/app/services/vizier.tsx`
- [ ] T028 [US1] Add the latest-run block to the task slide-over in `webui/app/routes/tasks.tsx`: `ran_at`, duration, and the response rendered as markdown, with the never-run / running / no-response states each reading distinctly (FR-015)
- [ ] T029 [US1] Show the requester in the task slide-over in `webui/app/routes/tasks.tsx`, distinguishing a person from an agent's own initiative, and rendering a no-longer-resolvable person as recorded (FR-032, FR-033)
- [ ] T030 [US1] Remove the free-text `User` input and its `formUser` state from the create/edit form in `webui/app/routes/tasks.tsx` (`:49`, `:102`, `:118`, `:161`, `:349-350`) — the requester is no longer caller-supplied

**Checkpoint**: a fired one-time task is still there and its report is readable — the original complaint is fixed

---

## Phase 4: User Story 5 - A scheduled run answers with a report (Priority: P1) 🎯 MVP

**Goal**: the agent is told its run is unattended, and the request stops implying a person sent it.

**Independent Test**: fire a task on a dummyplug agent and inspect what the agent was given — it
states the run is unattended and names no person as sender; then run the same instruction on a
small live model and confirm the answer reads as a report (quickstart §10).

**Depends on**: T004 (the rename) and T005 (`is_scheduled_task`) from Phase 2.

- [ ] T031 [P] [US5] Create `src/agents/agent/system_prompt/scheduled_run.rs` with `scheduled_run_md()` returning the constant framing text from contracts/scheduled-run-framing.md — no task name, requester or timestamp in it, so it stays identical between runs (FR-033)
- [ ] T032 [US5] Export the new module from `src/agents/agent/system_prompt/mod.rs`
- [ ] T033 [US5] Thread the run kind into `prepare_system_prompts` in `src/agents/agent/mod.rs:282` and append `scheduled_run_md()` last — after CORE and documents — **selected via `session.is_scheduled_task()`, never on the request content kind** (FR-031, research D8)
- [ ] T034 [US5] Update `prepare_system_prompts`'s callers in `src/agents/agent/mod.rs` to pass the session through
- [ ] T035 [P] [US5] Change `generate_frontmatter` in `src/schema/request.rs:239` so a scheduled run emits `sender: scheduler`, the task slug, and `requested_by` (the person's identity, or `self` for an agent's own initiative) instead of `sender: <user>` (FR-030)
- [ ] T036 [US5] Pass a rendering of the requester into `VizierRequest.user` in `src/scheduler/mod.rs` so the frontmatter has something honest to attribute to
- [ ] T037 [P] [US5] Unit tests in `src/agents/agent/system_prompt/`: the framing is present for a `Task` session, **absent for a `Dream` session** and absent for `HTTP`/`Discord`/`Telegram` (FR-031, FR-032), and the string is byte-identical across two calls (FR-033)

**Checkpoint**: a scheduled run's final message is a report; interactive turns and dream cycles are untouched

---

## Phase 5: User Story 2 - Look back at earlier runs (Priority: P2)

**Goal**: a task's past runs are listed newest-first with pagination, and any one expands to its
full exchange.

**Independent Test**: let a recurring task fire three times, confirm three distinct runs are listed
and each expands to only its own exchange; with a longer history, page back and confirm no run is
duplicated or skipped (quickstart §2, §3).

### Backend

- [ ] T038 [US2] Add `GET /agents/{agent_id}/tasks/{slug}/runs` to `src/channels/http/api/v1/agents/task.rs` per contracts/http-api.md: `before`/`before_id`/`limit`, newest first, `has_more`, and **no response text in any entry** (FR-007, FR-009, FR-014)
- [ ] T039 [US2] Add `GET /agents/{agent_id}/tasks/{slug}/runs/{run_id}/history` to `src/channels/http/api/v1/agents/task.rs`, building `VizierSession(agent_id, Task(slug, run_id), None)` and returning `Vec<SessionHistory>` through `list_session_history` with the same `before`/`before_seq`/`limit` query as `get_topic_history` (FR-010)
- [ ] T040 [US2] Register both routes in the `task()` router and gate both on `user_can_view_agent` in `src/channels/http/api/v1/agents/task.rs`, so run results are reachable only by those already permitted to view the task (FR-016)
- [ ] T041 [P] [US2] Add `utoipa` path annotations for both new routes in `src/channels/http/api/v1/agents/task.rs`, matching the style of the existing task handlers

### WebUI

- [ ] T042 [P] [US2] Add `listTaskRuns` and `getTaskRunHistory` to `webui/app/services/vizier.tsx`, carrying the page cursor
- [ ] T043 [US2] In `webui/app/lib/trail.ts:126`, open a turn without adopting the entry as its `request` when the request content is `unattended` — which makes the comment at `:106` true rather than aspirational and keeps a run's trail free of a nameless empty bubble (plan.md, WebUI consequences)
- [ ] T044 [P] [US2] Add a `trail.test.ts` case in `webui/app/lib/` pinning T043: a history list opening with an `unattended` request produces one turn whose `request` is undefined and whose trail and outcome are intact
- [ ] T045 [US2] Add the past-runs list to the task slide-over in `webui/app/routes/tasks.tsx`: newest first, each row showing `ran_at` and state, with a "load older runs" control driven by the cursor and hidden once `has_more` is false
- [ ] T046 [US2] Make a listed run expand in place in `webui/app/routes/tasks.tsx`, rendering its history through `trail.ts`'s `groupHistory` and the existing trail component — no navigation to the chat view, which cannot address a task session

**Checkpoint**: run history is browsable and each run's trail is readable in the task view

---

## Phase 6: User Story 4 - An agent reads its own task reports (Priority: P2)

**Goal**: an agent can list its own task's runs and fetch one's report, responses only.

**Independent Test**: let a recurring task run twice on a dummyplug agent, then drive both tools by
hand through the dummyplug protocol — the listing is newest-first with no response text, the fetch
returns that run's response, and neither carries reasoning or tool activity (quickstart §8).

- [ ] T047 [P] [US4] Implement `list_task_runs` as a `VizierTool` in `src/agents/tools/scheduler/mod.rs` per contracts/agent-tools.md: `slug`, optional `limit` (default 10, cap 50) and `before`; returns `run_id`, `ran_at`, `state` and `has_more`, with **no response text and no previews** (FR-020)
- [ ] T048 [US4] Implement `get_task_run_detail` as a `VizierTool` in `src/agents/tools/scheduler/mod.rs`: `slug` plus `run_id`; returns that run's response, `null` when the state is not `Answered`, truncated past a byte budget with `truncated: true` (FR-022, FR-023)
- [ ] T049 [US4] Ensure both tools include a `NoResponse` run in their listing rather than omitting it, so a failing task is visible, in `src/agents/tools/scheduler/mod.rs` (FR-024)
- [ ] T050 [US4] Scope both tools to the owning `agent_id` held on the struct, as the other tools in `src/agents/tools/scheduler/mod.rs` do, so another agent's task cannot be addressed at all (FR-025)
- [ ] T051 [US4] Register both tools on `default_toolset` in `VizierTools::new()` in `src/agents/tools/mod.rs`
- [ ] T052 [US4] Add both tool names to `VizierTools::DREAM_TOOL_NAMES` in `src/agents/tools/mod.rs:376`, beside the four scheduler tools already there (FR-026)
- [ ] T053 [P] [US4] Add human-readable labels for both tools to the tool-label switch in `webui/app/routes/chat.tsx:135-149`, matching the existing task-tool entries

**Checkpoint**: an agent can review how its own scheduled work has been going, awake or dreaming

---

## Phase 7: User Story 3 - See each task's last run in the list (Priority: P3)

**Goal**: each task-list row shows when it last ran and the state that run reached.

**Independent Test**: with tasks in each state present, open the task list and confirm every row
shows its last run time and state, with never-run, running and no-response each distinguishable
(quickstart §5).

- [ ] T054 [US3] Include `requester` and a `last_run` **without** `response` on each entry of `get_tasks` in `src/channels/http/api/v1/agents/task.rs` (FR-014)
- [ ] T055 [US3] Add a last-run column to the task table in `webui/app/routes/tasks.tsx`, showing `ran_at` and state, replacing the bare `last_executed_at` cell at `:264`
- [ ] T056 [US3] Render never-run as "not yet run" rather than blank or an error, and show a run in flight as running, in `webui/app/routes/tasks.tsx` (FR-005)
- [ ] T057 [P] [US3] Show the requester in the task table in `webui/app/routes/tasks.tsx`, distinguishing a person from an agent's own initiative (FR-032)

**Checkpoint**: all five stories independently functional

---

## Phase 8: Polish & Cross-Cutting Concerns

- [ ] T058 Run `cargo clippy` and fix every new warning; confirm no `unwrap()`/`expect()` was added outside tests (constitution Principle V)
- [ ] T059 Run `cargo test` and `cd webui && npm run typecheck`, both green
- [ ] T060 Walk quickstart.md §1-§8 against a running binary with a dummyplug agent (constitution e2e gate) — the fired one-time task surviving, per-run session separation, pagination, the no-response run, deletion cascade, requester, and both agent tools
- [ ] T061 Walk quickstart.md §9's manual check: kill the process mid-run, restart, and confirm the run reads `interrupted` and the task is not permanently blocked
- [ ] T062 Walk quickstart.md §10 against a small **live** model: confirm a scheduled run's answer reads as a report, and that an interactive turn and a dream extraction are both unchanged
- [ ] T063 [P] Update `CLAUDE.md` where it describes tasks and the scheduler, so the docs state that runs are recorded, one-time tasks are deactivated rather than deleted, and the content variant is `Unattended`
- [ ] T064 Confirm the commits carrying the three breaking changes are flagged `[**breaking**]` per the `git-cliff` convention: the requester replacing `Task.user`, the session slug rendering, and the renamed content variant

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies
- **Foundational (Phase 2)**: depends on Setup — **blocks all five stories**
- **User Stories (Phases 3-7)**: all depend only on Phase 2, and can then run in parallel
- **Polish (Phase 8)**: depends on every story being complete

### Within Phase 2

```
T002 → T003                        (same file, Requester before TaskRun)
T004, T005                         parallel with each other and with T002/T003
T006 → T008 → T009                 (table, then impl, then composition)
T007 → T008                        (trait before impl)
T009 → T010, T011, T012            (storage reachable before migrations use it)
T009 → T013..T017                  (scheduler needs TaskRunStorage)
T013..T017                         sequential — all in src/scheduler/mod.rs
T018, T019, T020                   parallel, after the code they cover
```

### User Story Dependencies

Every story depends on Phase 2 and on nothing else. That is deliberate: run recording was placed
in Foundational rather than inside US1 precisely so US2, US3 and US4 are not all queued behind
US1. Two incidental couplings, neither breaking independence:

- **US5** needs T004 and T005 specifically, both in Phase 2
- **US3** touches the same WebUI file as US1 (`routes/tasks.tsx`), so the two should not be edited concurrently by different people

### Parallel Opportunities

- Phase 2: `T004`, `T005`, `T010`, `T011` and the three test tasks `T018`-`T020`
- Phase 3: `T026`, `T027` while the backend tasks proceed
- Phase 4: `T031`, `T035`, `T037`
- Phase 5: `T041`, `T042`, `T044`
- Phase 6: `T047`, `T053`
- All five story phases, once Phase 2 closes

---

## Parallel Example: Phase 2

```bash
# After T002/T003 land, these four touch different files:
Task: "Rename VizierRequestContent::Task to Unattended in src/schema/request.rs"
Task: "Fix the Task arm of to_slug() and add is_scheduled_task() in src/schema/session.rs"
Task: "Make delete_task drop its runs in src/storage/sqlite/task.rs"
Task: "Add the requester migration in src/dependencies.rs"
```

## Parallel Example: Phase 4 (US5)

```bash
Task: "Create scheduled_run_md() in src/agents/agent/system_prompt/scheduled_run.rs"
Task: "Change generate_frontmatter for a scheduled run in src/schema/request.rs"
Task: "Unit-test framing selection in src/agents/agent/system_prompt/"
```

---

## Implementation Strategy

### MVP — US1 + US5 together

The spec makes both P1 and says why: US1 puts the response on screen, US5 makes it worth the
screen space. Shipping US1 alone would display conversational filler, and a person reading a
greeting followed by a question still cannot tell whether their task worked.

1. Phase 1 — baseline green
2. Phase 2 — Foundational (**blocks everything**)
3. Phase 3 — US1
4. Phase 4 — US5
5. **Stop and validate** against quickstart §1 and §10, then demo

### Incremental delivery after the MVP

- **US2** — run history and per-run trails, for *when did this start going wrong*
- **US4** — the agent's own view of its runs, which also restores the continuity per-run sessions remove
- **US3** — last-run state in the list, pure convenience over information already reachable

### Parallel team strategy

Phase 2 is the bottleneck and is worth doing together; its scheduler tasks (T013-T017) are all one
file and do not parallelise. Afterwards: one person on US1+US3 (shared WebUI file), one on US5, one
on US2+US4.

---

## Notes

- **The one mistake to avoid** is in T033: select the framing on `session.is_scheduled_task()`, not
  on the request content kind. The dream cycle sends the same content kind with its own framing, so
  branching on content double-frames every dream. T037 is the test that catches it.
- **T005 is the highest-leverage line in the feature.** Until `to_slug()` renders the full
  timestamp, every run of a task collides into one session, so there is nothing for US2 or US4 to
  list. It is also the change with the widest blast radius — `to_slug()` is the storage identity
  for sessions, history and session files.
- Three commits want `[**breaking**]`; T064 is the check.
- Not in scope, noted in research D12: `parse_history_row` drops an unparseable row silently
  (`src/storage/sqlite/history.rs:32`). A `tracing::warn!` there would make the legacy-row loss
  visible and is a cheap separate improvement.
- Commit after each task or logical group; stop at any checkpoint to validate a story on its own.
