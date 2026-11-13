# Feature Specification: Detached tool calls

**Feature Branch**: `058-detached-tool-calls`

**Created**: 2026-11-13

**Status**: Draft

**Input**: User description: "i want to replace "background jobs" with more general detached flag for execute python. what this flag does make any tool calling run in the background, this spec thus make something like background subagent no longer needed"

## Overview

Today an agent can run work in the background only through a dedicated feature: *background jobs*, started by the purpose-built `background_subagents` tool, which launches sub-agent conversations, waits for them and reports back into the originating conversation. Nothing else can run in the background: a slow shell command, a web fetch, a long search or an MCP call blocks the agent's turn until it finishes.

This feature replaces that special case with one general capability: a **detached** option on the agent's script execution. When a tool call in a script is marked detached, it runs on its own, the agent's turn continues at once, and the result is reported back when it is done. Any tool, including one that talks to another agent, can be detached, so a dedicated background sub-agent tool is no longer needed and is retired.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Run any slow tool without blocking the turn (Priority: P1)

An agent needs a result that takes minutes (a long shell command, a large download, a slow search). It marks that call detached. The agent gets an immediate acknowledgement with a handle, keeps talking to the person or doing other work, and is woken with the result when the call finishes.

**Why this priority**: This is the whole point of the feature and the capability background jobs only gave to sub-agents. It delivers value alone.

**Independent Test**: With an offline test agent, detach a tool call that takes several seconds; verify the script continues immediately with a handle, and that a follow-up turn arrives carrying the call's result.

**Acceptance Scenarios**:

1. **Given** an agent running a script, **When** it detaches a tool call, **Then** the script receives a handle immediately and does not wait for the tool.
2. **Given** a detached call that finishes successfully, **When** it completes, **Then** the originating conversation receives exactly one report containing the result, and the agent gets a turn to react to it.
3. **Given** a detached call that fails or times out, **When** it ends, **Then** the report says so with the reason, instead of the result being lost.
4. **Given** several detached calls started in one turn, **When** they finish at different times, **Then** the agent is not woken once per call in a storm of turns (see FR-009).

---

### User Story 2 - Delegate to other agents without a dedicated tool (Priority: P1)

An agent that used `background_subagents` to fan work out to sub-agents now does the same by detaching the ordinary calls that reach other agents. The behaviour people saw before (each piece runs in its own conversation, the results come back as one report) is preserved.

**Why this priority**: It is the stated reason the old tool can be removed; without it the replacement is a regression.

**Independent Test**: Detach three sub-agent calls in one group; verify each runs in its own conversation, one combined report is delivered, and the old tool is no longer offered to the model.

**Acceptance Scenarios**:

1. **Given** a script that detaches calls to other agents, **When** all finish, **Then** the originating conversation receives the answers, each attributed to its piece.
2. **Given** an existing agent, **When** it starts after the upgrade, **Then** it is not offered `background_subagents`, and its guidance describes detaching instead.

---

### User Story 3 - See, cancel and survive restarts (Priority: P2)

A person watching a conversation sees which detached calls are running, can cancel one, and does not lose track of them when the server restarts. The existing tray of background jobs in the WebUI keeps working, now fed by detached calls.

**Why this priority**: Parity with what background jobs already offer; it makes the feature safe to rely on but is not needed for the first demo.

**Independent Test**: Start a long detached call, cancel it from the interface, and verify no report is sent; start another, restart the server, and verify it is shown as interrupted rather than silently vanishing.

**Acceptance Scenarios**:

1. **Given** a running detached call, **When** a person or the agent cancels it, **Then** it stops, is shown as cancelled, and never produces a report.
2. **Given** a running detached call, **When** the server restarts, **Then** the call is shown as interrupted and the agent is told so, not left "running" forever.
3. **Given** a detached call that launched further detached calls, **When** it is cancelled, **Then** the nested ones are cancelled too.

---

### User Story 4 - Existing history and jobs keep working (Priority: P3)

Conversations and stored jobs created before the change still display correctly, and a job that was in flight at upgrade is not corrupted.

**Why this priority**: Compatibility only; no new capability.

**Independent Test**: Open a conversation containing saved background-job entries and verify it renders; verify list and cancel of a legacy job still work.

**Acceptance Scenarios**:

1. **Given** a stored background job from before the change, **When** its conversation is opened, **Then** it reads as it did.

---

### Edge Cases

- A call that depends on the live turn (a client tool bound to an open socket, a question for the person, a turn-ending tool, a sensitive-input request) cannot be detached meaningfully; the attempt is refused with a message naming why.
- Detaching inside a detached call (nesting) is allowed up to the same depth limit background jobs have; beyond it the call is refused.
- The agent's turn ends before the detached call finishes: the call keeps running and the report wakes the conversation later, queued behind any turn that is running.
- The originating conversation is deleted while a call runs: the call is cancelled and nothing is reported.
- A detached call produces a very large result: the report carries a bounded excerpt and says it was cut, as a normal tool result would.
- The agent is updated, restarted or deleted while calls run: behaviour matches background jobs today.
- A secret-bearing result (`get_secret`): it is redacted in the report exactly as in a non-detached call.
- Unattended contexts (dream cycle, scheduled tasks), where waking a conversation makes no sense: detaching is refused there.
- The time limit is hit: the call is aborted and reported as timed out.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Script execution MUST offer a `detached` option that makes a tool call run in the background; any tool reachable from a script MUST be detachable unless it falls in the refused set of FR-007.
- **FR-002**: A detached call MUST return to the script immediately with a handle that identifies the call and its state, without waiting for the tool.
- **FR-003**: When a detached call reaches a final state (succeeded, failed, timed out, cancelled, interrupted), the system MUST record it and, except when cancelled, deliver exactly one report to the originating conversation.
- **FR-004**: A report MUST wake the conversation with a turn of its own, queued behind any running turn, using the same wake-up behaviour background reports have today, on every channel (WebUI, Discord, Telegram, workspaces).
- **FR-005**: Each detached call MUST have a time limit, settable per call with a sensible default and a maximum, after which it is aborted and reported as timed out.
- **FR-006**: A detached call MUST run with the same permissions, hooks, per-tool timeout, redaction and telemetry as the same call made inline.
- **FR-007**: The system MUST refuse to detach a call that depends on the live turn (client tools, questions to the person, turn-ending tools, sensitive input) and MUST refuse detaching in unattended contexts, each with an explanatory message.
- **FR-008**: Detached calls MUST nest to a bounded depth; deeper attempts are refused.
- **FR-009**: Calls started together as a named group MUST report together: the report is held until every member is final, preserving the single combined report of `background_subagents`. Ungrouped calls report when each finishes, and reports that are ready at about the same time MUST reach the agent in one wake-up.
- **FR-010**: People MUST be able to list a conversation's detached calls and cancel one; the agent MUST be able to do the same for its own. Cancelling cascades to calls launched from the cancelled one and never sends a report.
- **FR-011**: Detached calls MUST survive turn boundaries but not a server restart; after a restart each in-flight call MUST be shown as interrupted and the agent informed, as background jobs are today.
- **FR-012**: Exactly-once reporting MUST hold: a call is either reported or cancelled, never both, and never reported twice, including across restarts and ownership changes in a fleet.
- **FR-013**: The `background_subagents` tool MUST be removed from what agents are offered; the guidance agents read MUST describe the detached option instead. `parallel_subagents` (blocking) is unaffected.
- **FR-014**: The existing job tray in the WebUI, the job list and cancel endpoints and the agent's list and cancel tools MUST keep working, backed by detached calls. Saved history and stored jobs from before the change MUST still read correctly.
- **FR-015**: Detached calls MUST be traceable: a detached call's work appears as a child of the trace that started it, and usage accounting attributes the cost to the same agent and conversation.
- **FR-016**: Documentation MUST describe the detached option, the refused cases, reporting and cancellation, and note the retirement of `background_subagents`.

### Key Entities

- **Detached call**: one tool call running in the background: who started it (agent, conversation, turn), which tool, its state, start and end times, time limit, optional group, optional parent call (for nesting). Holds no result text beyond what a normal tool result holds in history.
- **Group**: an optional label tying several detached calls from one turn so they report together.
- **Report**: the single wake-up delivered to the originating conversation, carrying each member's outcome.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: An agent can detach any non-refused tool and continue its script in under one second regardless of how long the tool takes.
- **SC-002**: 100% of detached calls that end in a non-cancelled state produce exactly one report; zero duplicate or missing reports across a restart test of 50 in-flight calls.
- **SC-003**: Every scenario previously done with `background_subagents` (fan-out, combined report, cancel, nesting, restart) can be reproduced with detached calls, and no agent guidance still refers to the old tool.
- **SC-004**: A person can see a running detached call in the conversation within 2 seconds of its start and cancel it, and the cancelled call produces no further activity.
- **SC-005**: Conversations and jobs created before the upgrade open and display without errors in 100% of the cases in the compatibility test set.
- **SC-006**: In an offline end-to-end run, detaching a 10-second call lets the agent answer the person in under 2 seconds, then deliver the result when the call ends.

## Assumptions

- "Detached" is a per-call option of script execution; marking a whole script detached is a convenience on the same mechanism, not a separate one.
- The existing background-job machinery (records, runner, report wake-up, cancel cascade, depth limit, exactly-once transition) is the foundation; this feature generalises its unit of work from "sub-agent piece" to "tool call" rather than building a second system.
- Sub-agent work in a detached call keeps today's behaviour of running in its own fresh conversation.
- Detached calls do not persist across a server restart (as today); durable resumption is out of scope.
- Default and maximum time limits stay the same as background jobs (600 s default, 3600 s maximum).
- Removal of `background_subagents` is a deliberate breaking change for agents that call it by name; a retired-name message tells the model to detach instead. Compatibility code is added to the pre-1.0 removal checklist (#82).
- Out of scope: new WebUI visuals (the existing tray is reused, so no FigCraft pass), a general job scheduler, and streaming partial output from a detached call.
