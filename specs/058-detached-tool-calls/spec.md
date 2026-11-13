# Feature Specification: Detached tool calls

**Feature Branch**: `058-detached-tool-calls`

**Created**: 2026-11-13

**Status**: Draft

**Input**: User description: "i want to replace "background jobs" with more general detached flag for execute python. what this flag does make any tool calling run in the background, this spec thus make something like background subagent no longer needed"

## Overview

A tool call normally blocks the agent until it finishes. A slow shell command, a large download, a long search, an MCP call or a question to another agent all hold the agent still.

This feature adds one general capability: a **detached** option on the agent's script execution. A tool call marked detached runs on its own, the script gets a handle at once, and the agent keeps working. When the call finishes, its result is delivered to the agent in its next inner loop iteration (its next round of thinking and acting), without waiting for the whole conversation turn to end. If the agent is idle when a call finishes, the conversation is woken with a turn of its own.

This is a **full replacement** of the old "background jobs" feature. Its dedicated tools (`background_subagents` and the job list/cancel tools), its report mechanism and its stored job records are removed, not wrapped or kept compatible. Delegating to another agent in the background is simply a detached call to the tool that reaches that agent. `parallel_subagents` (blocking) is a separate feature and is unaffected.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Run any slow tool without blocking the agent (Priority: P1)

An agent needs a result that takes minutes. It marks that call detached. It gets a handle immediately, keeps talking to the person or doing other work, and sees the result in its next inner loop iteration after the call finishes.

**Why this priority**: This is the whole point of the feature and delivers value alone.

**Independent Test**: With an offline test agent, detach a call that takes several seconds, then keep the agent working through further rounds; verify the script continued at once with a handle, and that the result shows up at the start of the first round after the call finished.

**Acceptance Scenarios**:

1. **Given** an agent running a script, **When** it detaches a tool call, **Then** the script receives a handle immediately and does not wait for the tool.
2. **Given** a detached call that finishes while the agent is mid-turn, **When** the agent's current round ends, **Then** the result is part of the input of its next round in the same turn, not held until the conversation's next turn.
3. **Given** a detached call that fails or times out, **When** it ends, **Then** the agent is told so with the reason, instead of the result being lost.
4. **Given** the agent is idle (no turn running) when a call finishes, **When** it completes, **Then** the conversation is woken with a turn that carries the result.

---

### User Story 2 - Delegate to other agents without a dedicated tool (Priority: P1)

An agent that needs several other agents to work in parallel detaches the ordinary calls that reach them, as a group if it wants one combined answer.

**Why this priority**: It is the stated reason the dedicated background sub-agent tool can go.

**Independent Test**: Detach three calls to other agents as one group; verify each runs in its own conversation, the agent receives one combined delivery once all three are final, and no background sub-agent tool is offered.

**Acceptance Scenarios**:

1. **Given** a group of detached calls to other agents, **When** the last member finishes, **Then** the agent receives all members' outcomes together, each attributed to its call.
2. **Given** an ungrouped detached call, **When** it finishes, **Then** it is delivered on its own.

---

### User Story 3 - Inspect, cancel and see what is running (Priority: P2)

The agent can list its detached calls, inspect any one of them, and cancel one. A person can see a conversation's running detached calls and cancel one. After a server restart, calls that were in flight are shown as interrupted instead of running forever.

**Why this priority**: Control and visibility make detaching safe to rely on, but the first demo does not need them.

**Independent Test**: Start a long detached call; inspect it by handle (state, tool, elapsed time); cancel it as the agent, then repeat as a person; verify no result is ever delivered for it. Start another, restart the server, and verify it reads as interrupted.

**Acceptance Scenarios**:

1. **Given** a running detached call, **When** the agent inspects it by handle, **Then** it sees the tool, its arguments summary, state, start time, elapsed time and time limit, and for a finished call its outcome.
2. **Given** a running detached call, **When** the agent or a person cancels it, **Then** it stops, is shown as cancelled, and nothing is delivered for it.
3. **Given** a detached call that launched further detached calls, **When** it is cancelled, **Then** the nested ones are cancelled too.
4. **Given** a running detached call, **When** the server restarts, **Then** it is shown as interrupted and the agent is told so on its next opportunity.

---

### Edge Cases

- A call that depends on the live turn (a client tool bound to an open socket, a question for the person, a turn-ending tool, a sensitive-input request) cannot be detached meaningfully; the attempt is refused with a message naming why.
- Detaching inside a detached call (nesting) is allowed up to a depth limit; beyond it the call is refused.
- A call finishes while the agent is in the middle of a long script: nothing is injected into the running script; the result waits for the next round boundary.
- A call finishes just as the agent is about to end its turn: the agent gets one more round to see it rather than the result being deferred to a later turn.
- A group where one member is cancelled or fails: the group still delivers once every member is final, with that member's outcome stated.
- The conversation is deleted while calls run: they are cancelled and nothing is delivered.
- A detached call produces a very large result: the agent receives a bounded excerpt and is told it was cut, as for an inline call.
- The agent is updated, restarted or deleted while calls run: calls are interrupted along with its running turns.
- A secret-bearing result (`get_secret`): redacted in everything the agent, history and logs see, exactly as for an inline call.
- Unattended contexts (dream cycle, scheduled tasks) have no one to deliver to: detaching is refused there.
- The time limit is hit: the call is aborted and the agent is told it timed out.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Script execution MUST offer a `detached` option that makes a tool call run in the background; any tool reachable from a script MUST be detachable unless it falls in the refused set of FR-007.
- **FR-002**: A detached call MUST return to the script immediately with a handle that identifies the call and its state, without waiting for the tool.
- **FR-003**: When a detached call reaches a final state (succeeded, failed, timed out, cancelled, interrupted), the system MUST record it and deliver its outcome to the agent exactly once, except for a cancelled call, which is never delivered.
- **FR-004**: **Delivery while a turn is running.** An outcome that is ready while the agent has a turn running MUST be made part of the agent's input at its next inner loop iteration in that same turn. It MUST NOT wait for the conversation's next turn, and it MUST NOT be injected into a script that is still running. If the agent is about to end its turn while outcomes are pending delivery, it MUST get one more round to see them.
- **FR-005**: **Delivery while idle.** An outcome that is ready when the agent has no turn running MUST wake the conversation with a turn of its own, on every channel (WebUI, Discord, Telegram, workspaces), queued behind any turn that starts in the meantime.
- **FR-006**: Outcomes ready at the same moment MUST be delivered together in one iteration or one wake-up, never as a storm of turns.
- **FR-007**: The system MUST refuse to detach a call that depends on the live turn (client tools, questions to the person, turn-ending tools, sensitive input) and MUST refuse detaching in unattended contexts, each with an explanatory message.
- **FR-008**: Each detached call MUST have a time limit, settable per call with a default and a maximum, after which it is aborted and reported as timed out.
- **FR-009**: A detached call MUST run with the same permissions, hooks, per-tool timeout, redaction and telemetry as the same call made inline, and its work MUST appear as a child of the trace that started it with cost attributed to the same agent and conversation.
- **FR-010**: Detached calls MUST nest to a bounded depth; deeper attempts are refused.
- **FR-011**: Calls started in a named group MUST be delivered together once every member is final. Ungrouped calls are delivered as each finishes (subject to FR-006).
- **FR-012**: The agent MUST be able to list its conversation's detached calls, inspect any single call by handle (tool, arguments summary, state, start and elapsed time, time limit, and the outcome once final) and cancel a call. People MUST be able to see a conversation's detached calls and cancel one. Cancelling cascades to calls launched from the cancelled one and never delivers anything.
- **FR-013**: Detached calls MUST survive turn boundaries but not a server restart; after a restart each in-flight call MUST be shown as interrupted and the agent informed.
- **FR-014**: Exactly-once delivery MUST hold: a call is delivered or cancelled, never both and never twice, including across restarts and ownership changes in a fleet.
- **FR-015**: The old background-jobs feature MUST be removed entirely: its dedicated tools, report mechanism, records and the guidance that describes them. Agents MUST be told in their guidance how to detach instead. No compatibility with old jobs or their saved entries is provided.
- **FR-016**: Documentation MUST describe the detached option, the refused cases, delivery timing (mid-turn and idle), inspection and cancellation.

### Key Entities

- **Detached call**: one tool call running in the background: who started it (agent, conversation, turn), which tool, its state, start and end times, time limit, optional group, optional parent call (for nesting). Holds no result text beyond what a normal tool result holds in history.
- **Group**: an optional label tying several detached calls from one turn so they are delivered together.
- **Delivery**: the hand-off of one or more final outcomes to the agent, either into its next inner round or as a wake-up turn.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: An agent can detach any non-refused tool and continue its script in under one second regardless of how long the tool takes.
- **SC-002**: 100% of detached calls that end in a non-cancelled state are delivered exactly once; zero duplicate or missing deliveries across a restart test of 50 in-flight calls.
- **SC-003**: A result that is ready during a running turn is seen by the agent in the first round that starts after it became ready, in 100% of test runs, with no extra conversation turn created.
- **SC-004**: A person can see a running detached call within 2 seconds of its start and cancel it, and a cancelled call produces no further activity.
- **SC-005**: The agent can inspect any of its detached calls by handle and get the state in under one second.
- **SC-006**: In an offline end-to-end run, detaching a 10-second call lets the agent answer the person in under 2 seconds, then act on the result when the call ends.
- **SC-007**: After the change, no tool, endpoint, stored record or guidance text of the old background-jobs feature remains.

## Assumptions

- "Detached" is a per-call option of script execution; marking a whole script detached is a convenience on the same mechanism, not a separate one.
- The old background-jobs design does not constrain this one: its records, report turns, per-piece sessions and tray are replaced, and any part worth keeping is re-specified above.
- Sub-agent work in a detached call runs in its own fresh conversation, as any call to another agent does.
- Detached calls do not persist across a server restart; durable resumption is out of scope.
- Default time limit 600 s and maximum 3600 s are carried over as sensible defaults, not as compatibility.
- The person-facing view of running calls is the existing job tray of the WebUI repointed at detached calls; no new visual design is needed, so no FigCraft pass.
- Deleting the old feature is a breaking change for anything calling it by name; the removal and any migration code are listed on #82 when planned.
- Out of scope: a general job scheduler, durable resumption, and streaming partial output from a detached call.
