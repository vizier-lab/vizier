# Feature Specification: Subagents as temporary workers

**Feature Branch**: `059-subagent-workers`

**Created**: 2026-11-14

**Status**: Draft

**Input**: User description: "A subagent is a temporary worker with a specific task, serving the main agent. Temporary means it has no stored configuration of its own; it works on behalf of the main agent, inheriting all of its tools and attributions, and may write the main agent's soul, schedule tasks and so on. Only the main agent may spawn subagents, with the limit configurable on the backend. The main agent must be able to stop a running subagent, and a subagent that hits a blocker must be able to ask the main agent a question and get a reply in the same subagent conversation. Subagent tools get a dedicated category; `parallel_subagents` is retired."

## Context

Today a subagent piece is the same agent, sent a prompt on a separate conversation. It receives the
full prompt of a person-facing turn (the agent's identity written as "you", the owner profile, the
conversational operating doctrine), and its message arrives attributed to its own agent with no
explanation. Nothing tells it that it is a worker, that its reader is the main agent rather than a
person, or what its job is beyond the prompt text. The fan-out tool blocks until every piece
finishes, so the main agent cannot stop one piece, read a question from it or answer one.

This feature changes what a subagent *understands itself to be* and how the main agent *manages*
it, and lets people peek at what the workers are doing in the WebUI (Story 7), read-only.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A worker knows it is a worker (Priority: P1)

The main agent hires a subagent for a task. The subagent understands that it is a temporary worker
for that agent, that its answer goes back to the main agent and not to a person, and that the
identity and owner described in its prompt belong to the agent it works for. It answers with a
compact, direct report instead of a friendly message addressed to the owner.

**Why this priority**: This is the core problem. Every other story builds on a worker that reads
its situation correctly; without it, a worker writes for the wrong reader and confuses its
employer's identity with its own.

**Independent Test**: With an offline test agent, spawn one subagent, then inspect the prompt and
request the subagent received: it carries the worker framing, the identity under "the agent you
work for", and the request marks it as sent by the main agent as worker task 1 of 1. The report
returned to the main agent contains no greeting or offer addressed to a person.

**Acceptance Scenarios**:

1. **Given** an agent with an identity and an owner profile, **When** it spawns a subagent,
   **Then** the subagent's prompt presents the identity as the agent it works for and the owner as
   the person that agent serves, never as "you".
2. **Given** a spawned subagent, **When** it reads its request, **Then** the request states that it
   came from its main agent, that it is a subagent task, and its position among the tasks spawned
   together (for example "task 2 of 4").
3. **Given** a subagent that has finished, **When** its result reaches the main agent, **Then** the
   result states what was done, what could not be done and why, and how certain the worker is.

---

### User Story 2 - The main agent hires, waits for and stops workers (Priority: P1)

The main agent spawns several workers and immediately gets a handle for each. It can wait for any
one or all of them to settle, list them, read one's state and result, and stop one that is going
nowhere while the others keep running.

**Why this priority**: Stopping a single worker is a stated need, and the handle model is what
makes it and Story 3 possible. It replaces the retired blocking fan-out.

**Independent Test**: Spawn three workers with an offline test agent, wait for any, stop one that is
still running, wait for all: the stopped one reports state "stopped" with the reason "stopped by
the main agent", and the other two report their own outcomes.

**Acceptance Scenarios**:

1. **Given** the main agent spawns N tasks, **When** the spawn returns, **Then** it returns N handles
   at once, in the order given, without waiting for any worker.
2. **Given** running workers, **When** the main agent waits for "any", **Then** the wait returns as
   soon as one worker settles (answered, failed, timed out, stopped or asking) and reports every
   given worker's current state.
3. **Given** running workers, **When** the main agent waits for "all", **Then** the wait returns when
   every given worker has settled.
4. **Given** a running worker, **When** the main agent stops it, **Then** it stops, its state becomes
   "stopped" with the reason "stopped by the main agent", and other workers are unaffected.
5. **Given** a wait whose time runs out before the workers settle, **Then** the wait returns with
   the workers' current states and the workers keep running.

---

### User Story 3 - A worker asks, the main agent replies (Priority: P2)

A worker reaches a blocker it cannot resolve alone (an ambiguous instruction, a missing
credential, a choice between options). It asks the main agent a question, which ends its current
step in an "asking" state. The main agent sees the question when it waits or reads the worker,
replies, and the worker continues in the same conversation with everything it did before.

**Why this priority**: It turns a dead end into a recoverable one, but it depends on Story 2's
handles.

**Independent Test**: Script an offline worker to ask a question on its first step and answer on its
second; the main agent waits, sees state "asking" with the question, replies, waits again and
receives an answer that shows the worker saw its own earlier steps and the reply.

**Acceptance Scenarios**:

1. **Given** a worker, **When** it asks the main agent a question, **Then** its state becomes
   "asking", the question is available to the main agent, and the worker does nothing further
   until replied to or stopped.
2. **Given** a worker in "asking", **When** the main agent replies, **Then** the worker continues in
   the same conversation with its earlier steps and the reply in view, and its state returns to
   "running".
3. **Given** a worker that is not asking, **When** the main agent replies to it, **Then** the reply
   is refused with a reason the main agent can act on.

---

### User Story 4 - Only the main agent hires (Priority: P2)

A worker cannot hire workers of its own. The limit is a backend setting so it can be loosened later
without changing the agent's tools.

**Why this priority**: It keeps "workers serve the main agent" simple and bounded, but the feature
is usable without it.

**Independent Test**: With the default setting, a worker's tool catalogue contains no subagent
tools other than asking the main agent; raising the setting makes the spawning tools appear for
workers.

**Acceptance Scenarios**:

1. **Given** the default setting, **When** a worker lists its tools, **Then** the subagent
   category offers it only the tool to ask the main agent.
2. **Given** the main agent, **When** it lists its tools, **Then** the subagent category offers
   spawn, wait, reply, stop, list and read, and not the tool to ask the main agent.
3. **Given** an operator raises the nesting setting, **When** a worker lists its tools, **Then** it
   is offered the spawning tools up to the configured depth.

---

### User Story 5 - The old fan-out tool is retired with guidance (Priority: P3)

An agent that still calls the retired fan-out tool, or the older name it replaced, is told which
tools to use instead.

**Why this priority**: Breaking-change hygiene; agents with old habits or saved skills recover on
their own.

**Independent Test**: Call the retired names from a script: each answers with guidance naming the
spawn and wait tools, and neither points at another retired name.

**Acceptance Scenarios**:

1. **Given** the retired fan-out name, **When** an agent calls it, **Then** it receives guidance to
   spawn and then wait for all.
2. **Given** the name the fan-out tool itself replaced, **When** an agent calls it, **Then** the
   guidance names the new tools, not the retired fan-out tool.

---

### User Story 6 - Work survives the turn that started it (Priority: P1)

The main agent spawns a long-running worker. The person interrupts the main agent's turn, or the
turn simply ends, or the agent restarts. The worker keeps going, and when it finishes or needs an
answer, its outcome arrives in the conversation and the main agent picks it up, either in the step
it is on or in a new turn if it was idle.

**Why this priority**: Without it, an interruption throws away a worker's progress and the agent
has to start the work again, which is the main cost of long-running workers.

**Independent Test**: With an offline test agent, spawn a slow worker, abort the main turn, and let
the worker finish: the worker's state is "answered", the main agent receives one delivery naming
the worker and carrying its result, and a second delivery never arrives. Repeat with a restart of
the agent mid-work: the worker resumes and its result is still delivered once.

**Acceptance Scenarios**:

1. **Given** a running worker, **When** the person aborts the main agent's turn, **Then** the
   worker keeps running and its state is unchanged.
2. **Given** a worker that settles while the main agent is idle, **When** it settles, **Then** the
   main agent is started with one delivery carrying the worker's task, position and result.
3. **Given** a worker that settles while the main agent is mid-turn, **When** the main agent reaches
   its next step boundary, **Then** the delivery is added there and no new turn is started.
4. **Given** a worker that starts asking after the main turn ended, **Then** the question is
   delivered the same way, and the main agent can reply to it.
5. **Given** a worker whose outcome the main agent already read or waited on, **Then** that outcome
   is not delivered.
6. **Given** a running worker, **When** the agent is updated or restarted, **Then** the worker
   resumes on the next instance and its outcome is delivered once.

### User Story 7 - A person peeks at the workers (Priority: P2)

A person chatting with the agent sees, in the chat's status strip, how many workers are still at
work, with an amber dot while one of them is waiting on a question. Opening it shows a panel of the
conversation's workers, grouped by the spawn that hired them: each row gives the task, its position
("2 · …"), the handle, the elapsed time and what it is doing (running, waiting for the agent with
its question, answered, failed, ran out of time, stopped). Expanding a row shows the task in full,
the worker's steps drawn with the same activity trail the chat uses, and its result. The panel is
separate from Background work, which shows detached scripts.

**Why this priority**: Workers outlive the turn and run unseen; without a view a person cannot tell
whether the agent is waiting on a stuck worker or why a turn started on its own.

**Independent Test**: With an offline test agent, spawn two workers, one of which asks: the strip
counts 2 with an amber dot, the panel shows one group of two, the asking row shows its question,
and expanding a row shows its steps and, once answered, its result. Another person's topic and an
unknown id answer not found.

**Acceptance Scenarios**:

1. **Given** a conversation with unfinished workers, **Then** the strip's Subagents item counts
   the running and asking ones, and carries an amber dot while one is asking.
2. **Given** the panel is open, **When** a worker changes state, **Then** its row updates without a
   reload (from the `subagent` frame).
3. **Given** an expanded worker, **Then** its steps are drawn with the chat's activity trail, read
   from the worker's own conversation, and its result follows once it has one.
4. **Given** any person, **Then** the panel has no control that stops or replies to a worker.

### Edge Cases

- **The main turn ends first.** When the turn that spawned workers ends, or the person aborts it,
  the workers keep running; their results and questions are delivered into the conversation later
  (Story 6). Aborting stops only the main agent's turn, never a worker.
- **The agent is updated or restarted** while workers run: the workers resume on the next instance
  and are not lost.
- **A worker times out** while running: state "timed out". A worker in "asking" does not consume
  its time limit and waits indefinitely, until it is replied to, stopped, or its conversation or
  agent is deleted.
- **The conversation or the agent is deleted**: every unfinished worker in it is stopped with the
  matching reason, and nothing is delivered.
- **The main agent already saw a result** through a wait or a read: that result is not delivered
  again. A worker the main agent stopped is never delivered.
- **Unknown or foreign handle.** Waiting on, replying to, stopping or reading a handle that does not
  exist or belongs to another conversation is refused as data the agent can act on, never a crash.
- **Stopping a settled worker** is a no-op that reports the worker's final state.
- **A worker writes the main agent's identity or schedules a task.** Allowed, exactly as if the main
  agent did it, and attributed to the main agent. Several workers writing the same memory is not
  guarded: the last write wins, as for any two writes today.
- **A worker asks while the main agent is not waiting.** The question is delivered into the
  conversation (Story 6) and also stays readable on the worker's record.
- **Workers spawned in a scheduled run** deliver into that run's conversation, waking the agent
  there, since no person is in it.
- **Workers spawned from a detached script** belong to the script's conversation like any other
  worker: the script ending, being cancelled or timing out does not stop them.
- **A worker launches a detached script.** Allowed (it acts with the main agent's tools); that
  script cannot spawn workers under the default setting, because it inherits the worker's depth.
- **A worker tries to ask the person.** The person-facing question tools stay unavailable to
  workers; only asking the main agent is offered.
- **A spawn of zero tasks, an empty task, or an invalid time limit** is refused before any worker
  starts.

## Requirements *(mandatory)*

### Functional Requirements

**What a worker is**

- **FR-001**: A subagent MUST be a temporary worker for exactly one task, with no stored
  configuration of its own. "Temporary" refers to its configuration, not its lifetime: a worker
  lives as long as the conversation it was spawned in, independent of the turn or detached script
  that spawned it.
- **FR-002**: A worker MUST act on behalf of the main agent with the main agent's full tool set,
  memory and permissions, minus the restrictions in FR-014 and FR-018; actions it takes MUST be
  attributed to the main agent exactly as today. No additional protection is placed on what it may
  write.
- **FR-003**: A worker's prompt MUST open with a worker framing that names the agent it works for,
  says its answer goes to that agent and not to a person, and asks for a compact, direct report.
- **FR-004**: A worker's prompt MUST present the main agent's identity as "the agent you work for"
  and the owner profile as "the person that agent serves", not as the worker's own identity.
- **FR-005**: A worker's prompt MUST NOT include the parts of the operating doctrine written for a
  person-facing conversation (keeping the person updated, offering follow-ups, fanning work out),
  and MUST include the parts it needs to do the job (sandbox, project context when spawned from a
  project chat, the agent's configured reference documents).
- **FR-006**: A worker's request MUST state that it was sent by its main agent as a subagent task
  and its position among the tasks spawned together.
- **FR-007**: A worker's final answer MUST be shaped as a report for the main agent: what was done,
  what could not be done and why, and how certain the worker is.

**The subagent tools**

- **FR-008**: The subagent tools MUST form their own tool category, separate from the background
  work (detached script) tools.
- **FR-009**: Spawning MUST accept one or more tasks, each with its instructions, and an optional
  per-worker time limit (1 to 3,600 seconds, default 600), and MUST return one handle per task at
  once, in the order given.
- **FR-010**: Waiting MUST accept a set of handles and a mode, "any" or "all", MUST return when the
  mode is met or the call's own time runs out, and MUST report each given worker's state, and its
  question or result where it has one. A wait that runs out of time MUST NOT stop the workers.
- **FR-011**: Replying MUST deliver the main agent's text to a worker in "asking", which then
  continues in the same conversation with its earlier steps in view; a reply to a worker in any
  other state MUST be refused with a reason.
- **FR-012**: Stopping MUST stop one running or asking worker without affecting the others, and MUST
  record the stop reason "stopped by the main agent".
- **FR-013**: Listing MUST show the workers spawned in the current conversation with their task,
  state and times; reading one MUST show its task, state, stop reason, question, result and the
  conversation where its full history remains.
- **FR-014**: A worker MUST be able to ask the main agent one question, which ends its current step
  and sets its state to "asking"; this tool MUST be offered only to workers. The person-facing
  question and sensitive-input tools MUST remain unavailable to workers.
- **FR-015**: Every refusal (unknown or foreign handle, invalid input, reply to a worker not asking)
  MUST be returned as data the agent can read, never as a failure of the script.

**Lifecycle and states**

- **FR-016**: A worker MUST be in exactly one of: running, asking, answered, failed, timed out,
  stopped. A stopped worker MUST carry a stop reason: stopped by the main agent, conversation
  deleted, or agent deleted.
- **FR-017**: A worker MUST keep running when the turn or detached script that spawned it ends, is
  aborted by a person, is cancelled or times out. Aborting a turn MUST stop only the main agent's
  turn.
- **FR-017a**: A worker MUST survive an update or restart of its agent: running workers resume on
  the next instance from their last step, and asking workers keep their question.
- **FR-017b**: A worker in "asking" MUST wait indefinitely and MUST NOT consume its time limit
  while waiting.
- **FR-017c**: Deleting a conversation or an agent MUST stop its unfinished workers with the
  matching stop reason.

**Delivery**

- **FR-024**: When a worker settles (answered, failed, timed out) or starts asking, and the main
  agent has not already seen that outcome through a wait or a read, the outcome MUST be delivered
  into the worker's conversation exactly once: at the next step boundary if the main agent is
  mid-turn, otherwise by starting a new turn of the main agent.
- **FR-025**: A delivery MUST identify the worker (its task and position) and carry its result or
  question; several outcomes ready at the same moment MAY be delivered together.
- **FR-026**: An outcome the main agent saw through a wait or a read MUST NOT be delivered again, and
  a worker the main agent stopped MUST NOT be delivered.

**Peeking in the WebUI**

- **FR-027**: People MUST be able to list the workers a conversation hired (running and asking
  first, then the most recent final ones), read one with its whole task and result, and read its
  own conversation, each only for a conversation of their own; any other id answers not found.
- **FR-028**: Every worker snapshot sent to the conversation MUST say whether the main agent already
  has the newest outcome, so the panel can say "the agent read it" or "waiting to be delivered".
- **FR-029**: The chat's status strip MUST show a Subagents item separate from Background work,
  counting unfinished workers, with an attention dot while one is asking; its panel MUST group
  workers by spawn, show the question of an asking worker, and draw an expanded worker's steps with
  the existing activity trail. The panel MUST NOT offer stopping or replying.
- **FR-018**: By default only the main agent MAY spawn workers: the spawning, waiting, replying,
  stopping, listing and reading tools MUST NOT be offered inside a worker or inside work a worker
  started. The allowed nesting depth MUST be a backend setting (default: workers cannot spawn),
  independent of the existing background-work nesting limit.
- **FR-019**: Each worker MUST have a record holding its task, its hiring conversation and turn, its
  own conversation, its state, its stop reason, its question and result, whether its outcome has
  been seen or delivered, and its start and end times, readable for as long as the hiring
  conversation exists.

**Retirement and guidance**

- **FR-020**: The blocking fan-out tool MUST be retired: calling its name MUST return guidance to
  spawn and then wait for all.
- **FR-021**: The guidance for the older name the fan-out tool replaced MUST be updated to name the
  new tools.
- **FR-022**: The main agent's operating doctrine MUST describe workers as temporary helpers who know
  only what their task says, cannot talk to the person, can ask the main agent, and cannot hire
  workers; and MUST tell it to give each worker the goal, the context and the shape of the answer
  it wants.
- **FR-023**: The new category name MUST be reserved, so an integration or tool server of the same
  name is given a distinct name.

### Key Entities

- **Worker (subagent)**: one temporary task performed on behalf of the main agent. Holds its task,
  its hiring conversation and turn, its own conversation, its position among the tasks spawned with
  it, its state, its stop reason, its pending question, its result, whether its outcome was seen or
  delivered, and start and end times.
- **Spawn group**: the set of workers created by one spawn call; gives each worker its position
  ("task 2 of 4") and is what the WebUI panel groups by.
- **Hiring conversation**: the main agent's conversation a worker was spawned in. It bounds the
  worker's lifetime, receives its deliveries, and its deletion stops the worker. The turn or
  detached script that issued the spawn is recorded for reference but does not bound the worker.
- **Delivery**: one message into the hiring conversation carrying one or more worker outcomes
  (results or questions) the main agent has not yet seen; each outcome is delivered at most once.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: In a test set of worker tasks, no worker report addresses the owner or a person
  (greetings, "let me know", offers of follow-up), versus the current behaviour where such
  phrasing appears.
- **SC-002**: In 100% of spawned workers, the prompt and request identify the worker as a worker of
  the named main agent, with its task position.
- **SC-003**: The main agent can stop any single running worker and the other workers in the same
  group complete normally in 100% of test runs.
- **SC-004**: A worker that asks a question and is replied to completes its task in the same
  conversation, with its earlier steps intact, in 100% of test runs.
- **SC-005**: Under the default setting, 0 workers are able to spawn workers.
- **SC-006**: When the main turn is aborted or ends, or the agent restarts, 0 worker results are
  lost: every worker outcome the main agent had not already seen reaches it exactly once.
- **SC-008**: After a conversation or agent is deleted, 0 of its workers remain running.
- **SC-007**: Calling a retired name always yields guidance that names a tool that exists.
- **SC-009**: A person can tell, from the strip alone, that a worker is waiting on a question, and
  can read any worker's steps from the panel without leaving the chat.

## Assumptions

- Workers outlive the turn that spawned them and live as long as their conversation; aborting a
  turn stops only the main agent. Delivery reuses the same "claimed exactly once" behaviour that
  background work already has, so a worker's outcome reaches the main agent once whether it was
  mid-turn or idle.
- A worker in "asking" waits indefinitely; there is no expiry for an unanswered question.
- No guard is added against concurrent writes by workers (to memory, the identity or elsewhere):
  workers act exactly as the main agent would, and the last write wins.
- Per-call worker definitions (a role, a tool subset or a different model per worker) are out of
  scope and will be a separate spec.
- People cannot stop or reply to workers; the WebUI view of workers (Story 7) is read-only.
- Workers are allowed wherever the main agent could use the retired fan-out tool today (an
  interactive turn, a scheduled run, a detached script), and not in the dream cycle.
- The per-worker time limit keeps today's bounds (1 to 3,600 seconds, default 600).
- A worker's own conversation keeps being saved, as today's pieces are, so its history can be
  inspected and a reply continues it.
- Removing the retired tool is a breaking change and is released as one.
