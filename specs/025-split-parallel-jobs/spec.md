# Feature Specification: Blocking and Background Parallel Subagents

**Feature Branch**: `025-split-parallel-jobs`

**Created**: 2026-10-24

**Status**: Draft

**Input**: User description: "let's split paralel_subtask into blocking and non blocking one, paralel_jobs and async_paralel_jobs. let me know if you have better names"

## Summary

Since spec 012, `paralel_subtasks` is non-blocking only: it returns a job id at once and the results come back later as a background report that wakes the agent for a new turn. That suits long work, but it is the wrong shape when the agent needs the answers to finish the reply it is writing now (compare three sources, then answer). The agent has to end its turn, say "working on it", and pick the thread up again in a second turn. That costs an extra turn, splits one answer into two messages, and loses the agent's working state in between.

This feature replaces `paralel_subtasks` with two tools that take the same input:

| Tool | Behaviour |
|------|-----------|
| `parallel_subagents` (blocking) | Runs the tasks in parallel and **waits**: the call returns every task's result, in order, inside the current turn |
| `background_subagents` (non-blocking) | Exactly today's `paralel_subtasks`: returns a job id at once, results arrive later as a background report |

`delegate_agent` (non-blocking) and `consult_agent` (blocking) are unchanged.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Fan out and answer in the same turn (Priority: P1)

A person asks the agent something that splits into independent parts ("compare these three libraries"). The agent runs one subagent per part, waits for all of them, and writes a single answer from their results, all in the same turn. The person gets one reply, not "I've started some work" followed by a second message later.

**Why this priority**: This is the capability that is missing today and the reason for the feature.

**Independent Test**: On the offline `dummyplug` provider, have an agent call `parallel_subagents` with three tasks from a script. Check that the call returns three results in the order given, that the turn continues after the call and produces its reply, and that no background report or job entry appears afterwards.

**Acceptance Scenarios**:

1. **Given** an agent in a conversation, **When** it calls `parallel_subagents` with N tasks, **Then** the N tasks run at the same time (total wait close to the slowest task, not their sum) and the call returns one result per task, in the order given.
2. **Given** the call has returned, **When** the turn ends, **Then** no background report is ever delivered for that call and it never appears in `list_background_jobs` or the WebUI jobs tray.
3. **Given** one task fails or runs past its time limit, **When** the call returns, **Then** that task's entry says it failed or timed out (with the reason), every other task's result is still returned, and the entries stay aligned with the tasks.
4. **Given** the call is made from a Python script, **When** it returns, **Then** the script receives the results as data it can loop over, so it can combine them before the agent replies.

---

### User Story 2 - Keep background fan-out for long work (Priority: P1)

For work that takes minutes (research across many sources, large file processing), the agent still starts it in the background, tells the person, and is woken by one report when it is done — exactly what `paralel_subtasks` does today.

**Why this priority**: Splitting must not lose the existing behaviour; every agent currently relying on it needs a direct replacement.

**Independent Test**: Re-run spec 012's batch checks (launch, report wakes the agent, cancel, failures aligned, nesting limit) against `background_subagents`; all pass unchanged.

**Acceptance Scenarios**:

1. **Given** an agent, **When** it calls `background_subagents` with N tasks, **Then** it behaves exactly as `paralel_subtasks` did under spec 012: immediate acknowledgement with a job id, one report when every task is final, listed and cancellable as a background job.
2. **Given** an agent calls `paralel_subtasks`, **When** the call is dispatched, **Then** it is refused as an unknown tool and any suggestion it offers names both new tools.

---

### User Story 3 - The agent picks the right one (Priority: P2)

The agent's guidance explains when to wait and when to go to the background, so it waits for quick fan-outs it needs for the current answer and backgrounds long ones instead of keeping the person waiting on a silent turn.

**Why this priority**: Two tools with the same input are only useful if the model chooses between them sensibly; without guidance it will pick one at random.

**Independent Test**: Read the agent's boot guidance and both tool descriptions as the model receives them; each states its behaviour (waits vs returns at once) and when to prefer it.

**Acceptance Scenarios**:

1. **Given** a new turn, **When** the agent reads its "Large tasks" guidance, **Then** it names both tools and says: wait (`parallel_subagents`) when the results are needed for this reply and the parts are quick; go to the background (`background_subagents`) when the work is long or the person need not wait.
2. **Given** the agent lists the `agents` category, **When** it reads the two descriptions, **Then** the blocking one says it waits and returns the results, and is bounded by the agent's tool time limit; the background one keeps today's "returns immediately, do not wait or poll" wording.

---

### User Story 4 - Old conversations still read correctly (Priority: P3)

Conversations saved before the split contain `paralel_subtasks` calls. They still show as job launches in the WebUI trail and still link to their background report.

**Why this priority**: Only affects display of history, but history is kept indefinitely.

**Independent Test**: Open a topic with a saved `paralel_subtasks` call in the WebUI; it renders as before (job launch, links to its job/report).

**Acceptance Scenarios**:

1. **Given** saved history with a `paralel_subtasks` call, **When** the topic is opened, **Then** it is shown as a background job launch, as before.
2. **Given** a `parallel_subagents` call in history, **When** the topic is opened, **Then** it is shown as an ordinary tool call with its results, not as a job launch.

---

### Edge Cases

- **Tool time limit shorter than the tasks**: the blocking call is bounded by the agent's tool time limit as well as its own per-task limit. When the tool limit would cut in first, tasks still running are stopped and reported as timed out, and the call returns what finished, rather than the whole call failing with a bare tool timeout.
- **Turn aborted or agent restarted mid-wait** (`/abort`, update, restart, crash): the running tasks are stopped with the turn; nothing is reported later. On resume, the open call is sealed with the usual stop notice (spec 023), so the agent can decide to run it again.
- **Nesting**: a task run by `parallel_subagents` may itself fan out. The existing nesting limit counts both tools, so a chain of either kind stops at the same depth.
- **Called from a woken background-report turn**: allowed, subject to the same nesting limit.
- **Dream cycle**: neither tool is available to dreams, as today.
- **Empty task list or an empty prompt**: refused with the same message for both tools, before anything runs.
- **Many tasks**: the same task-count and timeout validation applies to both tools; the blocking tool does not allow more than the background one.
- **Cost visibility**: subagent turns of a blocking call are counted in usage as background work for the agent, the same as today's batch pieces.

## Requirements *(mandatory)*

### Functional Requirements

**Two tools**

- **FR-001**: The system MUST provide `parallel_subagents` (blocking) and `background_subagents` (non-blocking), both in the `agents` category, both taking the same input as today's `paralel_subtasks` (a list of self-contained task prompts and an optional per-task time limit with the same range and default).
- **FR-002**: `paralel_subtasks` MUST be retired, not aliased (tool names are the dispatch key, matching the `memory_detail` precedent). A call to it MUST get the standard unknown-tool refusal; wherever that refusal offers suggestions, both new names MUST be among them.
- **FR-003**: `delegate_agent` and `consult_agent` MUST be unchanged.

**Blocking tool**

- **FR-004**: `parallel_subagents` MUST run all its tasks concurrently, each in its own fresh subagent conversation, and return only when every task is final (answered, failed, or timed out).
- **FR-005**: Its result MUST contain one entry per task, in the order given, each with the task's status and either its answer or its failure reason. A failed or timed-out task MUST NOT drop or shift any other entry.
- **FR-006**: Its total wait MUST be bounded by the agent's tool time limit. If that limit is reached first, unfinished tasks MUST be stopped and marked timed out, and the finished results returned.
- **FR-007**: A blocking run MUST NOT create a background job: no entry in `list_background_jobs` or the jobs tray, never cancellable through `cancel_background_job`, and never followed by a background report.
- **FR-008**: When the turn that made the call stops for any reason (abort, instance stop, panic), its running tasks MUST stop too, and no result of theirs may reach the conversation later.
- **FR-009**: The subagent conversations of a blocking run MUST remain inspectable afterwards in the same way background batch pieces are (their sessions are kept).

**Background tool**

- **FR-010**: `background_subagents` MUST behave exactly as `paralel_subtasks` does under spec 012 (acknowledgement, report, cancellation, failures, nesting, startup sweep). Jobs it launches MUST keep the existing batch job kind, so jobs launched before the split display the same way.

**Shared rules**

- **FR-011**: The background nesting limit MUST count nesting through either tool.
- **FR-012**: Neither tool may be available to the dream cycle.

**Guidance and display**

- **FR-013**: Both tool descriptions and the agent's "Large tasks" boot guidance MUST state which tool waits, which returns immediately, and when to prefer each (User Story 3).
- **FR-014**: Everything that names `paralel_subtasks` as a user- or model-facing label — the running-tool status text, the WebUI jobs tray and job labels, the user documentation's tool table and observability example — MUST name the new tool(s) instead.
- **FR-015**: The WebUI MUST keep recognising `paralel_subtasks` in saved history as a job launch (User Story 4), and MUST treat `background_subagents` as a job launch and `parallel_subagents` as an ordinary tool call.

### Key Entities

- **Parallel run (blocking)**: one call's set of tasks with their per-task status and answers; lives only for the duration of the call, its results returned as the tool's output. Not persisted as a job.
- **Background batch job**: unchanged from spec 012; now launched only by `background_subagents` (and historically by `paralel_subtasks`).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A request that splits into three quick independent parts is answered in **one** agent reply and **one** turn using `parallel_subagents`, where today it takes two turns and two messages.
- **SC-002**: The blocking call's wall time for N parallel tasks is within 20% of the slowest single task's time, not the sum.
- **SC-003**: 100% of spec 012's existing batch acceptance checks pass against `background_subagents` without change.
- **SC-004**: After the change, no model-facing text or user-facing label (tool list, guidance, status text, docs, WebUI labels) mentions `paralel_subtasks`, except where showing saved history.
- **SC-005**: Saved conversations containing `paralel_subtasks` calls render identically before and after the change.

## Assumptions

- **Naming** (chosen in clarification, 2026-10-24): `parallel_subagents` / `background_subagents`, over the proposed `paralel_jobs` / `async_paralel_jobs`, because:
  - The rename already breaks the old name, so it is the moment to fix the `paralel` misspelling instead of carrying it into two new names.
  - "background" already names this family for the model: `list_background_jobs`, `cancel_background_job`, and the "background report" it receives. "async" is ambiguous inside a Python sandbox where scripts cannot `await`.
  - "jobs" would make the blocking tool sound like it creates entries for `list_background_jobs`, which it does not; "subagents" says what actually runs (each task is a fresh conversation of the same agent, the `Subagent` session kind).
  - A shared suffix makes the pair read as one choice: wait or background.
- Only the same agent runs the tasks (as today); running tasks on other agents stays with `delegate_agent` / `consult_agent`.
- The blocking tool's result is structured data (list of entries) rather than one prose blob, since it is consumed mostly from scripts in code mode.
- Removing `paralel_subtasks` is a breaking change, flagged `[**breaking**]` in the changelog. It is a rename with no data migration, so nothing is added to the pre-1.0 removal tracker (#82); the WebUI's recognition of the old name in history is display code, not a migration.
- Existing agents' skills or CORE.md text that mention `paralel_subtasks` are not rewritten; the unknown-tool suggestion steers the model to the new names.
