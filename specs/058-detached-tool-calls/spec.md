# Feature Specification: Detached scripts

**Feature Branch**: `058-detached-tool-calls`

**Created**: 2026-11-13

**Status**: Draft

**Input**: User description: "i want to replace "background jobs" with more general detached flag for execute python. what this flag does make any tool calling run in the background, this spec thus make something like background subagent no longer needed" (refined: only `execute_python` carries the flag, since every other tool is called from a script)

## Overview

Every agent reaches its tools through `execute_python`: a script that calls tool functions. Today that script blocks the agent until it finishes, so a slow shell command, a large download, a long search, an MCP call or a question to another agent holds the agent still. The only way to work in the background is the dedicated *background jobs* feature, started by `background_subagents`, which can launch sub-agents and nothing else.

This feature adds one option to `execute_python`: **`detached`**. A detached script runs on its own, the agent gets a handle at once and keeps working, and when the script ends its result (the same report an ordinary run returns) is delivered back to the agent in its next inner loop iteration. Because every tool, including those that reach other agents, is called from a script, detaching the script makes *any* tool call run in the background. A dedicated background sub-agent tool is no longer needed and is retired.

This is a **full replacement** of the old background-jobs feature: its tools, report mechanism, stored records and routes are removed, not wrapped or kept compatible. `parallel_subagents` (blocking) is a separate feature and is unaffected.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Run a slow script without blocking the agent (Priority: P1)

An agent needs a result that takes minutes (a long shell command, a large download, a slow search). It runs the script with `detached` on. It gets a handle immediately, keeps talking to the person or doing other work, and sees the script's result in its next inner loop iteration after the script ends.

**Why this priority**: This is the whole point of the feature and delivers value alone.

**Independent Test**: With an offline test agent, run a script that takes several seconds with `detached` on and keep the agent working through further rounds; verify the call returned at once with a handle, and the result shows up at the start of the first round after the script ended.

**Acceptance Scenarios**:

1. **Given** an agent, **When** it runs `execute_python` with `detached` on, **Then** it receives a handle immediately and does not wait for the script.
2. **Given** a detached script that ends while the agent is mid-turn, **When** the agent's current round ends, **Then** the script's result is part of the input of its next round in the same turn, not held until the conversation's next turn.
3. **Given** a detached script that fails, raises or runs out of time, **When** it ends, **Then** the agent is told so with the reason and the script's traceback, instead of the result being lost.
4. **Given** the agent is idle (no turn running) when a detached script ends, **When** it completes, **Then** the conversation is woken with a turn that carries the result.

---

### User Story 2 - Delegate to other agents without a dedicated tool (Priority: P1)

An agent that needs other agents to work in parallel writes one detached script that calls the tool that reaches them (for example several `parallel_subagents` pieces), and receives one combined result when the script ends. Nothing else about delegation changes: each piece runs in its own conversation.

**Why this priority**: It is the stated reason the dedicated background sub-agent tool can go.

**Independent Test**: Run one detached script that fans out to two other agents; verify each runs in its own conversation, one delivery arrives with both answers once the script ends, and no background sub-agent tool is offered.

**Acceptance Scenarios**:

1. **Given** a detached script that calls other agents, **When** it ends, **Then** the originating conversation receives the answers together, attributed to their pieces, as the script returned them.
2. **Given** an existing agent, **When** it starts after the upgrade, **Then** it is not offered `background_subagents`, and its guidance describes detaching instead.

---

### User Story 3 - Inspect, cancel and see what is running (Priority: P2)

The agent can list its detached scripts, inspect any one of them, and cancel one. A person can see a conversation's running detached scripts and cancel one. After a server restart, scripts that were in flight are shown as interrupted instead of running forever.

**Why this priority**: Control and visibility make detaching safe to rely on, but the first demo does not need them.

**Independent Test**: Start a long detached script; inspect it by handle (intent, state, tool calls so far, elapsed time); cancel it as the agent, then repeat as a person; verify no result is ever delivered for it. Start another, restart the server, and verify it reads as interrupted.

**Acceptance Scenarios**:

1. **Given** a running detached script, **When** the agent inspects it by handle, **Then** it sees its intent, state, start time, elapsed time and time limit, the tool calls it has made so far, and for a finished script its outcome.
2. **Given** a running detached script, **When** the agent or a person cancels it, **Then** it stops (including a tool call or sub-agent it was waiting on), is shown as cancelled, and nothing is delivered for it.
3. **Given** a running detached script, **When** the server restarts, **Then** it is shown as interrupted and the agent is told so on its next opportunity.

---

### Edge Cases

- A tool that depends on the live turn (client tools, a question for the person, sensitive input, a turn-ending tool) is not available inside a detached script; calling one raises a catchable error naming why.
- A detached script's sub-agent work can itself run detached scripts; the nesting depth is bounded and a launch beyond it is refused.
- A script ends while the agent is in the middle of a long script of its own: nothing is injected into the running script; the result waits for the next round boundary.
- A script ends just as the agent ends its turn: the turn closes as planned and the result wakes the conversation as a new turn; it is never lost or delivered twice.
- The conversation is deleted while a script runs: it is cancelled and nothing is delivered.
- A script produces a very large result: the agent receives a bounded excerpt and is told it was cut, as for an inline run.
- A script produces files: they are saved as session files of the conversation and named in the delivery.
- The agent is updated, restarted or deleted while scripts run: they are interrupted along with its running turns.
- A secret-bearing result (`get_secret`): redacted in everything the agent, history and logs see, exactly as for an inline run.
- Unattended contexts (dream cycle, scheduled tasks) have no one to deliver to: `detached` is refused there.
- The time limit is hit: the script is aborted and the agent is told it timed out.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: `execute_python` MUST accept a `detached` option (boolean, default off) and an optional time limit for it. It is the only tool with the option; a detached script may call any tool its context allows.
- **FR-002**: A detached run MUST return to the agent immediately with a handle that identifies the script and its state, without waiting for the script.
- **FR-003**: When a detached script reaches a final state (succeeded, failed, timed out, cancelled, interrupted), the system MUST record it and deliver its outcome to the agent exactly once, except for a cancelled script, which is never delivered.
- **FR-004**: **Delivery while a turn is running.** An outcome that is ready while the agent has a turn running MUST be made part of the agent's input at its next inner loop iteration in that same turn. It MUST NOT wait for the conversation's next turn, and it MUST NOT be injected into a script that is still running. An outcome that is not delivered into a round before the turn closes is delivered by FR-005 as a wake-up turn; at the moment a turn closes, each ready outcome goes to exactly one of the two, never both and never neither.
- **FR-005**: **Delivery while idle.** An outcome that is ready when the agent has no turn running MUST wake the conversation with a turn of its own, on every channel (WebUI, Discord, Telegram, workspaces), queued behind any turn that starts in the meantime.
- **FR-006**: Outcomes that are ready at the same moment MUST be delivered together in one iteration or one wake-up, never as a storm of turns.
- **FR-007**: Inside a detached script the tools that depend on the live turn (client tools, questions to the person, sensitive input, turn-ending tools) MUST be unavailable, and `detached` MUST be refused in unattended contexts, each with an explanatory message.
- **FR-008**: Each detached script MUST have a time limit, settable per run with a default and a maximum, after which it is aborted and reported as timed out. The limit covers the whole script, including the tool calls and sub-agents it waits on.
- **FR-009**: A detached script MUST run with the same permissions, hooks, per-tool timeout, redaction and telemetry as the same script run inline, and its work MUST appear as a child of the trace that started it with cost attributed to the same agent and conversation.
- **FR-010**: Detached scripts MUST nest to a bounded depth (a script that causes sub-agent turns which themselves detach); a launch beyond the limit is refused.
- **FR-011**: The agent MUST be able to list its conversation's detached scripts, inspect any single one by handle (intent, state, start and elapsed time, time limit, the tool calls made, and the outcome once final) and cancel one. People MUST be able to see a conversation's detached scripts and cancel one. Cancelling stops the script and anything it is waiting on, cascades to detached scripts launched from the cancelled one, and never delivers anything.
- **FR-012**: Detached scripts MUST survive turn boundaries but not a server restart; after a restart each in-flight script MUST be shown as interrupted and the agent informed.
- **FR-013**: Exactly-once delivery MUST hold: a script is delivered or cancelled, never both and never twice, including across restarts and ownership changes in a fleet.
- **FR-014**: The old background-jobs feature MUST be removed entirely: its dedicated tools (`background_subagents` and the job list and cancel tools), report mechanism, records and the guidance that describes them. Agents MUST be told in their guidance how to detach instead. No compatibility with old jobs or their saved entries is provided.
- **FR-015**: Documentation MUST describe the detached option, what is unavailable inside a detached script, delivery timing (mid-turn and idle), inspection and cancellation.

### Key Entities

- **Detached script**: one `execute_python` run in the background: who started it (agent, conversation), its stated intent, its state, start and end times, time limit, optional parent script (nesting). Keeps a bounded (32 KiB), redacted copy of its outcome until the conversation is deleted.
- **Delivery**: the hand-off of one or more final outcomes to the agent, either into its next inner round or as a wake-up turn.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: An agent can detach a script and continue in under one second regardless of how long the script runs.
- **SC-002**: 100% of detached scripts that end in a non-cancelled state are delivered exactly once; zero duplicate or missing deliveries across a restart test of 50 in-flight scripts.
- **SC-003**: A result that is ready during a running turn is seen by the agent in the first round that starts after it became ready, in 100% of test runs, with no extra conversation turn created.
- **SC-004**: A person can see a running detached script within 2 seconds of its start and cancel it, and a cancelled script produces no further activity.
- **SC-005**: The agent can inspect any of its detached scripts by handle and get the state in under one second.
- **SC-006**: In an offline end-to-end run, detaching a 10-second script lets the agent answer the person in under 2 seconds, then act on the result when the script ends.
- **SC-007**: After the change, no tool, endpoint, stored record or guidance text of the old background-jobs feature remains.

## Assumptions

- Only `execute_python` has the option. Per-tool detaching and named groups are out of scope: several things that should come back together are done by putting them in one detached script.
- The old background-jobs design does not constrain this one: its records, report turns, per-piece sessions and tray are replaced, and any part worth keeping is re-specified above.
- Sub-agent work inside a detached script runs in its own fresh conversation, as any call to another agent does.
- Detached scripts do not persist across a server restart; durable resumption is out of scope.
- Default time limit 600 s and maximum 3600 s are carried over as sensible defaults, not as compatibility.
- People never see the word "detached": the WebUI calls the feature **Background work** (the status strip item and its panel). The panel is the existing job tray repointed at detached scripts, with collapsible entries (header: the script's intent, handle, elapsed time, a trash icon to cancel a running script, a chevron to expand), the script's full detail inside each entry, and the search and filter tabs the tray already has. It is drawn in `Vizier shell.figcraft` (Shell · chosen page, frames "058 · A" and "058 · B"): each entry's title is the script's intent, with its handle and elapsed time on the right, and an expanded entry shows the same detail the activity trail shows for a script (its code, the calls it made, and its output and result or error).
- Deleting the old feature is a breaking change for anything calling it by name; the removal and any migration code are listed on #82 when planned.
- Out of scope: a general job scheduler, durable resumption, and streaming partial output from a detached script.
