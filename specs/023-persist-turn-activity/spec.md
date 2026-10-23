# Feature Specification: Durable Turns: Live Progress That Survives Navigation and Restarts

**Feature Branch**: `023-persist-turn-activity`

**Created**: 2026-10-23

**Status**: Draft

**Input**: User description: "currently thinking/activity/toolcall list state is not persists, so when agent is still thinking, go to another page, and go back. the list will be reset" — extended: "another issue on this system durability is when there's a downtime, any ongoing conversation, task, queue progress, etc. is lost and not automatically continue because we did not persists those and the queue is in memory. let's fold this too into the spec and plan"

## Context: what is already true

Both problems come from the same gap: **work in progress lives only in memory.**

1. **A turn's steps are written down only when it ends.** The person's message is saved when the
   turn starts. The reasoning, tool calls and results are saved together at the end. Until then
   they exist only as live updates to whichever pages are open, so a page that opens mid-turn,
   whether by navigating back, reloading or opening a second tab, shows only the steps that happen
   after it opens.
2. **The queue is in memory.** Messages sent while an agent is busy wait in memory. A restart
   loses them without a trace.
3. **A restart abandons everything in flight.** A running turn stops wherever it was and never
   answers. Background jobs (subtasks, delegations), scheduled task runs and pending client
   tool calls are marked "interrupted" at startup, and nothing continues them.
4. **Missed schedules already catch up.** A recurring task or dream cycle whose time passed
   during downtime runs once when vizier starts. A one-time task whose time passed runs once.
   This must keep working, and resumed work must not be run a second time by it.

Recording each step of a turn durably, as it happens, fixes both problems: the same record is
what a page shows when it opens mid-turn, and what a restart resumes from.

## Clarifications

### Session 2026-10-23

- Q: After a restart, what should pick up again automatically? → A: Everything in progress:
  running turns and queued messages on every channel, background jobs, scheduled task runs, and
  schedules missed during the downtime (run once on startup).
- Q: What does a resumed turn do with a tool call that was cut off and has no recorded result?
  → A: It is never re-run. The agent is told the call was interrupted by a restart and its
  outcome is unknown, and decides what to do.
- Q: Is there a limit on how long the downtime can be? → A: No. Interrupted work is always
  resumed, whatever the length of the downtime.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Leaving and returning mid-turn keeps the activity list (Priority: P1)

A person asks an agent something that takes a while. While it works, they open another page in
the WebUI, reload, or open the conversation in a second tab. Today the activity list comes back
empty, or holds only the steps taken since, so they can't tell what the agent has done or
whether it is stuck. They expect the list exactly as it would be had they never left, and still
updating.

**Why this priority**: This is the originally reported problem, and it is the most visible
everyday symptom of the gap.

**Independent Test**: Start a turn that makes several tool calls. Mid-turn, navigate away, wait
for another step, and come back. Separately, reload mid-turn and open a second tab. Each view
shows every step so far, in order, once, and keeps updating.

**Acceptance Scenarios**:

1. **Given** an agent is mid-turn, **When** the person navigates away and back, reloads, or
   opens the conversation in another tab or device, **Then** every step recorded so far is
   shown, in the order it happened, with nothing duplicated.
2. **Given** the person was away while the agent took further steps, **When** they return,
   **Then** those steps are in the list too.
3. **Given** the person has returned mid-turn, **When** the agent takes further steps, **Then**
   they appear live, as for a person who never left.
4. **Given** the turn ends, **When** the answer arrives, **Then** its folded trail is the same for
   every viewer: one who stayed, one who returned, and one who reloads afterwards.
5. **Given** a step is still running (for example a long script), **When** the person returns,
   **Then** that step is shown as in progress, not missing.

---

### User Story 2 - A conversation interrupted by a restart carries on (Priority: P1)

Vizier restarts while an agent is mid-answer: after a crash, a deploy, `vizier shutdown`, or the
host rebooting. When it comes back, the agent picks up from the last step it completed and
finishes the answer. The reply arrives where the person asked, whether in the WebUI, Discord or
Telegram, marked as having continued after a restart.

**Why this priority**: Today a restart silently swallows the answer. The person has no reply and
no indication that they need to ask again. This is the core of the durability problem.

**Independent Test**: Start a turn that makes a tool call, and stop vizier after the call's
result is recorded but before the answer. Start vizier. The turn continues without repeating
the completed call, the answer appears in the conversation, and it is marked as resumed.

**Acceptance Scenarios**:

1. **Given** a turn was running when vizier stopped, **When** vizier starts again, **Then** the
   turn resumes after the last completed step without anyone resending the message.
2. **Given** a tool call had completed before the stop, **When** the turn resumes, **Then** that
   call is not run again and its result is still available to the agent.
3. **Given** a tool call was cut off with no recorded result, **When** the turn resumes,
   **Then** the call is not re-run, and the agent is told it was interrupted by a restart and its
   outcome is unknown.
4. **Given** the turn came from the WebUI, Discord or Telegram, **When** it completes after the
   restart, **Then** the reply is delivered to the same conversation on the same channel, and
   is visibly marked as continued after a restart.
5. **Given** a page with the conversation is opened after the restart, **When** the turn is
   still resuming, **Then** it shows the steps from before and after the restart as one
   activity list.

---

### User Story 3 - Queued messages survive a restart (Priority: P2)

A person sends several messages while the agent is busy. Vizier restarts before they are
answered. After the restart they are answered in the order they were sent, after the turn that
was running finishes.

**Why this priority**: The same loss as US2, but quieter: a queued message disappears with no
trace at all.

**Independent Test**: Hold a turn open, send two more messages, restart vizier. Both queued
messages are answered, in order, after the resumed turn.

**Acceptance Scenarios**:

1. **Given** messages were waiting behind a running turn, **When** vizier restarts, **Then**
   they run after the resumed turn, in their original order.
2. **Given** a queued checkpoint or lobotomy command, **When** vizier restarts, **Then** it keeps
   its place ahead of the messages queued behind it, as it would have without the restart.
3. **Given** the person aborts after the restart, **When** the abort arrives, **Then** the
   resumed turn stops and the queue is discarded, exactly as an abort does today.

---

### User Story 4 - Work started on the agent's behalf resumes (Priority: P2)

An agent launched background subtasks or delegated to another agent, or a scheduled task was
running, when vizier restarted. After the restart, each piece of that work continues. A
background job still reports back to the conversation that launched it, and a task run still
ends with its result recorded.

**Why this priority**: Agents increasingly do long, unattended work, and that is exactly the
work most likely to be under way when a restart happens.

**Independent Test**: Launch a background job whose pieces are still running, restart vizier,
and confirm the job reports back to the launching conversation. Do the same with a running
scheduled task, and confirm its run ends as answered with its result.

**Acceptance Scenarios**:

1. **Given** a background job was running, **When** vizier restarts, **Then** its unfinished
   pieces resume, and when every piece is final the job reports to the launching conversation
   exactly once.
2. **Given** a background piece had already answered before the stop, **When** vizier restarts,
   **Then** it is not run again, and its answer is part of the report.
3. **Given** a scheduled task run was in progress, **When** vizier restarts, **Then** the run
   resumes and ends with a recorded result, and the task is not fired again for the same
   occurrence.
4. **Given** an agent was waiting on a blocking consultation with another agent, **When** vizier
   restarts, **Then** the consulted agent's own turn resumes in its conversation. The waiting
   agent's call is reported as interrupted with unknown outcome (US2 scenario 3), so it is never
   answered twice.

---

### User Story 5 - Missed schedules still run once (Priority: P3)

Scheduled tasks and dream cycles whose time came during the downtime run once when vizier
starts, as they do today, and never on top of a run of the same task that is resuming.

**Why this priority**: Already true, so this story is about keeping it true alongside resumption.

**Independent Test**: Stop vizier across a recurring task's scheduled time, start it, and the
task runs once. Stop vizier during a task's run, start it, and the run resumes with no extra
run of that task.

**Acceptance Scenarios**:

1. **Given** a recurring task missed one or more occurrences during the downtime, **When** vizier
   starts, **Then** it runs once, not once per missed occurrence.
2. **Given** a one-time task's time passed during the downtime, **When** vizier starts, **Then**
   it runs once.
3. **Given** a task run is resuming, **When** the scheduler starts, **Then** the task is not
   fired again while that run is in progress.

---

### Edge Cases

- **Restart during a resumed turn**: it resumes again from its latest completed step.
- **A turn that brings vizier down every time it runs** (for example a tool that crashes the
  process): it is not resumed forever. After 3 resumptions without completing a new step, it
  ends as failed with a visible error, and the queue behind it carries on.
- **The agent can't start after the restart** (its provider is unreachable, its configuration
  is now invalid): its interrupted and queued work waits and resumes once the agent starts,
  like any other request made while it is unavailable.
- **The agent was deleted** while work was pending: that work is dropped with the agent.
- **The agent's settings were changed** mid-turn: unchanged from today. Running turns are
  stopped and queued messages carry on with the new settings. This feature does not resume a
  turn stopped by a settings change.
- **A client tool call was pending** at the stop: it is reported to the agent as interrupted
  with unknown outcome. A *deferrable* call keeps its current behaviour and is delivered to the
  next client that connects.
- **The person aborts while a tool call is running**: the call is stopped, and its step is
  recorded as stopped by the person with an unknown outcome. The trail stays in the
  conversation, folded, showing what the agent had done before the abort. Today an aborted
  turn's trail disappears. The next message is answered normally.
- **A turn is stopped by a settings change, or by an internal error, while a tool call is
  running**: same as an abort, with the reason it was stopped.
- **A blocking HTTP chat request** was waiting for the answer: the connection is gone, and the
  answer is in the conversation's history.
- **A dream cycle was in progress**: it is not resumed. Its next cycle runs when due, and that
  cycle covers the conversations the interrupted one would have.
- **Background piece time limits**: a resumed piece gets its full time limit again from the
  moment it resumes, because downtime is not the piece's fault.
- **Messages that may be ignored** (unmentioned group messages that the agent reads silently)
  are resumed like any other request. They usually end without a reply, as they do today.
- **Long downtime**: work is always resumed, however long vizier was down. Because the answer
  is marked as continued after a restart, a late reply doesn't look like an ordinary one.
- **Conversations recorded before this feature**: displayed as today. Turns cut off by a restart
  before upgrading are not resumed.

## Requirements *(mandatory)*

### Functional Requirements

**Recording progress**

- **FR-001**: Every step of a turn (reasoning, intermediate narration, each tool call, and each
  tool result) MUST be recorded durably as it happens: a call when it starts, a result when it
  completes.
- **FR-002**: The record written step by step MUST be the same, in content and order, as the
  record a finished turn produces today, so finished conversations read and replay exactly as
  before.
- **FR-003**: Every request accepted for an agent (person messages from any channel, background
  pieces, delegations, consultations, scheduled runs, queued commands) MUST be recorded durably
  before it waits in any queue, together with its place in that conversation's queue.

**Showing progress (WebUI)**

- **FR-004**: A WebUI view of a conversation with a running turn MUST show every step recorded
  so far, in order, whether the view was opened by navigation, reload, a second tab or another
  device.
- **FR-005**: After that, steps MUST continue to arrive live. A step MUST never be shown twice,
  or be missed because it landed while the view was opening.
- **FR-006**: The in-progress list MUST look the same as the list of a viewer who never left,
  and the folded trail after the turn ends MUST be identical for every viewer.
- **FR-007**: A running turn's activity MUST only be shown in its own conversation, to those
  allowed to view that conversation.

**Resuming after a restart**

- **FR-008**: On startup, every turn that was running when vizier stopped MUST resume from its
  last recorded step, without the original request being sent again.
- **FR-009**: A completed tool call MUST NOT be run again on resumption. A cut-off call (no
  recorded result) MUST NOT be re-run. The agent MUST receive, as that call's result, a notice
  that it was interrupted by a restart and its outcome is unknown.
- **FR-010**: Queued requests MUST run after the resumed turn, in their original order,
  preserving today's priority of checkpoint/lobotomy commands.
- **FR-011**: A resumed turn's reply MUST be delivered to the conversation and channel the turn
  belongs to (WebUI, Discord or Telegram), the same way a reply woken by a background report is
  delivered today, and MUST be marked as continued after a restart.
- **FR-012**: Background jobs in progress MUST resume their unfinished pieces and report to the
  launching conversation exactly once when every piece is final. Pieces that already answered
  MUST NOT run again.
- **FR-013**: A scheduled task run in progress MUST resume and be closed with its outcome, and
  the task MUST NOT be fired again for that occurrence.
- **FR-014**: Missed schedules MUST keep running once on startup (recurring tasks once, however
  many occurrences were missed; one-time tasks once; dream cycles when due), and never while a
  run of the same task is resuming.
- **FR-015**: Interrupted work MUST be resumed regardless of how long vizier was down.
- **FR-016**: A turn that has been resumed 3 times without recording a new step MUST end as
  failed, with an error in the conversation, and the queue behind it MUST carry on.
- **FR-017**: An `/abort` after a restart MUST stop a resumed turn and discard the queue, as it
  does today.
- **FR-018**: Resumption MUST work the same after a crash and after a clean shutdown.

**Stopping a turn**

- **FR-021**: Whenever a turn ends without finishing its own steps (aborted by the person,
  including the commands that abort; stopped for an agent settings change, restart or
  deletion; or ended by an internal error), every tool call it started without a recorded
  result MUST be given a result stating why it stopped and that its outcome is unknown. Calls
  stopped this way MUST NOT be run again.
- **FR-022**: After such a stop, no view MUST show a call of that turn as still running, and the
  next turn in that conversation MUST proceed normally.
- **FR-023**: An aborted turn's recorded steps MUST stay in the conversation as a folded trail,
  marked as aborted.

**Unchanged**

- **FR-019**: Stopping an agent instance for a settings change, restart or deletion MUST keep
  today's behaviour (running turns stopped, queued requests carried on or dropped with the agent).
- **FR-020**: Dream cycles in progress are not resumed. Conversations and trails recorded
  before this feature MUST display as they do today.

### Key Entities

- **Turn**: one request being handled for one conversation. Records the request, where it came
  from (a person on a channel, a background job, a delegation or consultation, a scheduled run),
  its place in the conversation's queue, its state (waiting, running, finished, failed,
  cancelled), when it was accepted and started, and how many times it has been resumed.
- **Step**: one entry of a turn's progress (reasoning, narration, tool call, tool result),
  recorded as it happens and in the conversation's history, in the same form finished turns
  already use.
- **Stop notice**: the result recorded for a tool call that never finished, which says why it
  stopped (a restart, an abort, an agent restart, an internal error) and that its outcome is
  unknown. Also, the label that marks a reply as continued after a restart.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: In 100% of trials, opening a conversation mid-turn shows every step recorded so far,
  none missing and none duplicated, within 1 second of the conversation opening.
- **SC-002**: In 100% of trials across crash and clean shutdown, a turn running at the stop
  produces its answer after the restart, without the person resending anything.
- **SC-003**: Zero completed or cut-off tool calls are executed a second time because of a
  resumption.
- **SC-004**: 100% of queued messages present at a stop are answered after the restart, in order.
- **SC-005**: 100% of background jobs running at a stop report back exactly once after the
  restart, and 100% of running task runs end with a recorded outcome.
- **SC-006**: The folded trail of a finished turn is identical whether the viewer stayed, left
  and returned, reloaded, or opened the conversation after a restart.
- **SC-007**: A turn that crashes the process every time is given up after at most 3
  resumptions, and the conversation's queue carries on.
- **SC-008**: After any abort or stop mid-call, zero tool calls in that conversation are left
  without a recorded result, and the next message is answered normally.

## Assumptions

- Scope covers all channels for resumption (WebUI, Discord, Telegram, HTTP, internal sessions).
  The activity list (US1) is a WebUI feature, since Discord and Telegram have no activity list.
- The current step-by-step progress is not resumed *inside* a single model call or tool call. A
  turn resumes at step boundaries: the next model call after the last recorded step.
- Context the agent receives automatically with a message (related memories, reactions) is
  computed again when the turn resumes, from the state at that time.
- The working indicator shows no elapsed time today, and none is added.
- Stopping an agent instance for a settings change is a deliberate decision (#52) and keeps
  stopping its turns. Only process restarts are covered.
- Anything recorded for durability is subject to the same deletion rules as the conversation it
  belongs to (deleted with the topic or the agent).
