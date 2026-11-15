# Feature Specification: Steer a Busy Turn

**Feature Branch**: `062-steer-busy-turn`

**Created**: 2026-11-15

**Status**: Draft

**Input**: User description: "currently when agent is "busy", user message is queued until the agent is finished. can we make it so that a new message could be queued instead to the next immediate agent tool call turn? just like how background work do when agent is busy"

## Context

Today, when a person sends a message to a conversation whose agent is still working on an earlier one, the new message waits in line. The agent finishes the whole earlier turn (every remaining step, the final reply) and only then reads the new message as a turn of its own. If the person wanted to correct course ("no, use the staging database", "stop, that's the wrong file", "also check the logs"), the agent keeps going on the stale instruction until it is done.

Background work already behaves differently: when a detached script or a subagent worker finishes while the agent is mid-turn, its result is handed to the agent at the next step boundary (after the current round of tool calls, before the agent's next thought), as one message, exactly once. This feature gives a person's message the same treatment: a message sent while the agent is busy reaches the running turn at its next step boundary, so the agent can take it into account immediately.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Correct the agent mid-turn (Priority: P1)

A person asks the agent to do a multi-step job. While the agent is working (running tools), the person realises something is wrong or missing and sends a follow-up message. The agent reads that message as soon as its current step finishes and continues the same turn with it in mind, instead of finishing the old plan first.

**Why this priority**: This is the whole point of the feature: course corrections reach the agent while they still matter, saving time, tokens and wrong work.

**Independent Test**: Start a turn that makes several tool calls in a row (an offline test agent can be scripted to do so), send a second message while the first tool call is running, and verify that the agent's very next step shows it received the second message, and that no separate turn runs for it afterwards.

**Acceptance Scenarios**:

1. **Given** an agent in the middle of a turn with tool calls still to come, **When** the person sends a new message, **Then** the message is delivered to the agent right after the current round of tool calls ends and before the agent's next step, within the same turn.
2. **Given** a message was delivered into a running turn, **When** that turn ends, **Then** no separate turn is started for that message.
3. **Given** a message was delivered into a running turn, **When** the person (or anyone watching the conversation) looks at the conversation, **Then** the message appears at the point in the turn where the agent read it, attributed to its sender, with its original sent time.
4. **Given** the agent's reply to the turn, **When** it is produced, **Then** it may address both the original request and the follow-up, as one reply.

---

### User Story 2 - Fall back to the queue when there is no next step (Priority: P1)

A person sends a message while the agent is busy, but the agent is already composing its final answer (no further tool calls come). The message is not lost and is not swallowed: it runs as its own turn after the current one, exactly as messages do today.

**Why this priority**: Without this, a message sent late in a turn could be silently dropped or never answered. It is as essential as Story 1.

**Independent Test**: Send a message while the agent is producing a final answer with no further tool calls; verify the message runs as the next turn and gets its own reply.

**Acceptance Scenarios**:

1. **Given** a busy agent whose turn ends without another step boundary, **When** a message was waiting, **Then** it runs as the next turn, in arrival order, as today.
2. **Given** a message waiting for the next step boundary, **When** the turn is stopped by the person (abort), **Then** the waiting message is treated exactly as today's queued messages are treated on abort.
3. **Given** a waiting message, **When** it is both eligible for the running turn and next in the queue, **Then** it is delivered to exactly one of them, never both and never neither.

---

### User Story 3 - Several messages while busy (Priority: P2)

A person sends two or three quick messages while the agent works. At the next step boundary the agent receives all the messages that arrived meanwhile, together, in the order they were sent, alongside any background work that finished in the same window.

**Why this priority**: People often type in bursts. Delivering them together keeps the agent's view coherent and avoids several tiny interruptions.

**Independent Test**: Send three messages during one long tool call; verify they are delivered together at the next boundary, in sending order, each with its own sender and time.

**Acceptance Scenarios**:

1. **Given** several messages arrive during one round, **When** the round ends, **Then** all of them are delivered together, in arrival order, each kept as a separate message in the conversation record.
2. **Given** background work and a person's message both become ready during the same round, **When** the round ends, **Then** both are delivered at that boundary, and the person's message is clearly distinguishable from the background result.

---

### User Story 4 - Knowing where the message went (Priority: P3)

While a message waits for the agent's next step, the person can see that it is waiting and, once read, that the agent picked it up mid-turn (rather than it looking like it was ignored until the end).

**Why this priority**: Feedback builds trust in the feature, but the behaviour itself (Stories 1–2) delivers value without it.

**Independent Test**: In the WebUI, send a message during a turn; verify it shows as waiting, then moves into the running turn's activity at the point it was read.

**Acceptance Scenarios**:

1. **Given** a message sent during a busy turn, **When** it has not yet been read, **Then** the WebUI shows it below the live activity trail as waiting for the agent.
2. **Given** the agent read it mid-turn, **When** the conversation is viewed, **Then** the turn is drawn as: the activity trail up to that point, then the person's message, then a new activity trail continuing the turn, then the turn's one reply.
3. **Given** a turn split this way, **When** the page is reloaded or opened in a second tab, **Then** the order is exactly the same as it was live.
4. **Given** a turn split this way, **When** its trail parts are shown, **Then** each part carries the ordinary "Worked" label with its own step count and duration; no special wording marks the split (the message itself shows it).
5. **Given** a steered message that fell back to its own turn (FR-004), **When** the conversation is viewed, **Then** it appears after the earlier turn's reply as an ordinary new turn.

Sketch:

```
you:    refactor the auth module
  ▸ Worked · 3 steps · 12s
you:    wait, keep the old token format     ← where the agent read it
  ▸ Worked · 5 steps · 40s
agent:  Done. I kept the v1 token…           ← one reply for the whole turn
```

---

### User Story 5 - Line up a separate task instead (Priority: P2)

While the agent works, a person wants to line up an unrelated request without interrupting the current job. They choose to queue the message as a new turn, and it runs after the current turn finishes, exactly as today.

**Why this priority**: Steering is the default, but some messages are a new task, not a correction; mixing them into the running turn would confuse the agent's current work.

**Independent Test**: During a multi-step turn, send a message with the "queue as new turn" choice (Ctrl+Enter / Cmd+Enter in the WebUI, or `/queue` on a chat platform); verify it is not delivered mid-turn and runs as its own turn afterwards.

**Acceptance Scenarios**:

1. **Given** a busy agent, **When** the person sends a message choosing to queue it, **Then** it is not delivered at any step boundary of the running turn and runs as the next turn.
2. **Given** a busy agent, **When** the person sends a message without making a choice, **Then** it steers the running turn (Story 1).
3. **Given** the agent is idle, **When** the person sends a message, **Then** no choice is offered or needed; it starts a turn as today.

---

### Edge Cases

- **Commands and special messages**: `/abort` keeps stopping the turn immediately. Side chats (`/btw`), handovers/checkpoints, `/lobotomy`, `/clear-files`, explicit skill invocations (`/<skill>`), and answers to the agent's own questions keep their current behaviour and are never delivered mid-turn.
- **`/queue` and a skill named `queue`**: `/queue` is a built-in command like `/btw` and wins over an agent or global skill named `queue`; that skill stays reachable through `/skill queue`. `/queue` with no message is answered with a short usage note and queues nothing.
- **Turn-ending tools**: if the current round ends the turn (a successful turn-ending call such as a rendered UI or a question to the person), there is no next step; waiting messages fall back to the queue (Story 2).
- **Attachments**: files sent with a message travel with it into the running turn, as they would in a turn of its own.
- **Restart or agent update mid-turn**: a message already delivered into a turn is part of that turn's record and is resumed with it; a message not yet delivered is still waiting and is either delivered at the resumed turn's next boundary or run as its own turn — exactly once.
- **Agent deleted**: waiting messages are rejected as queued messages are today.
- **Conversations with no person in them** (scheduled tasks, dreams, subagent workers, inter-agent mentions): unchanged; this feature only concerns messages people send.
- **Group conversations** (Discord guild channels, Telegram groups, Workspaces): a message that addresses the agent while it is busy in that conversation is delivered mid-turn, attributed to whoever sent it; messages the agent only reads silently (not addressed to it) are unchanged.
- **Very long messages or many messages**: the delivery is subject to the same size handling as any person's message; nothing waiting is dropped.
- **Context compaction**: a delivered message survives a checkpoint/handover the same way any other message in the turn does.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: When a person sends an ordinary chat message to a conversation whose agent is running a turn, the system MUST hold the message for delivery into that running turn at its next step boundary (after the current round's tool results, before the agent's next step).
- **FR-002**: At each step boundary, the system MUST deliver every waiting person message of that conversation, in arrival order, together with any finished background work, before the agent's next step.
- **FR-003**: Each person message MUST be delivered exactly once: either into a running turn or as a turn of its own, never both and never neither, including across restarts, agent updates and concurrent arrivals.
- **FR-004**: If a running turn ends without reaching another step boundary, waiting messages MUST run as their own turns afterwards, in arrival order, as they do today.
- **FR-005**: A delivered message MUST be saved in the conversation record at the point it was read, with its sender, its original sent time and its attachments, and MUST be visible to every viewer of the conversation in that position (live, after reload, in other tabs).
- **FR-006**: The agent MUST be able to tell a person's mid-turn message apart from background-work results and from its own tool results, and see who sent it.
- **FR-007**: Commands and special message kinds (abort, side chats, handovers, lobotomy, clear-files, explicit skill invocations, question answers) MUST keep their current behaviour.
- **FR-008**: Conversations without a person (scheduled tasks, dreams, subagent workers, agent-to-agent activations) MUST keep their current behaviour.
- **FR-009**: Aborting a turn MUST treat messages still waiting for that turn exactly as queued messages are treated on abort today; messages already delivered into the turn remain in its record.
- **FR-010**: The WebUI MUST show a message as waiting (below the live trail) while it waits and, once read mid-turn, split the turn's activity trail at that point: trail before, the message, trail after, then the reply. Both trail parts use the ordinary "Worked" label. The order MUST come from the saved conversation record, so it is identical live, after reload and in every tab.
- **FR-010a**: Background results delivered at a step boundary MUST keep their current presentation inside the trail; only a person's message splits it.
- **FR-011**: Steering MUST be the default for a message sent while busy, and the person MUST be able to choose, per message, to queue it as a new turn instead (today's behaviour). In the WebUI the choice is a keyboard shortcut: while the agent is busy, Enter sends a message that steers and Ctrl+Enter (Cmd+Enter on macOS) sends one queued as a new turn; while the agent is idle both simply send, as today. The composer shows a short hint naming the shortcut only while the agent is busy, and the shortcut never fires while a mention or suggestion list is open in the composer (Enter there still picks the item). The WebUI also accepts `/queue <message>` on every screen size, so a phone (which has no shortcut) can queue too; a plain message steers; channels without such a control offer it as a `/queue` command — a Discord slash command `/queue` taking the message, `/queue <message>` on Telegram (listed in the bot's command menu), and `/queue @<agent-id> <message>` in Workspaces — and a message sent without it steers. `/queue` sent while the agent is idle simply starts a turn.
- **FR-011a**: A message queued as a new turn by choice MUST never be delivered into the running turn; it runs after it, in arrival order with other queued messages.
- **FR-011b**: The choice MUST be made at send time and is not changed afterwards by the system; a steered message that falls back to the queue (FR-004) runs as its own turn.
- **FR-012**: The reply of a turn that received mid-turn messages MUST be one reply, notified once, as for any turn.

### Key Entities

- **Waiting message**: a person's message sent while the conversation's agent is busy; attributes: conversation, sender, sent time, content, attachments, state (waiting → delivered into a turn | run as its own turn | dropped by abort).
- **Step boundary**: the moment in a running turn after a round of tool results and before the agent's next step; the only point where waiting messages and background results are delivered.
- **Delivery**: the single message the agent receives at a step boundary, grouping the waiting person messages and any finished background work, each part kept distinguishable.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A message sent while the agent is running a tool reaches the agent before its next step in 100% of cases where the turn has another step.
- **SC-002**: In repeated tests (including restarts and abort mid-turn), 0 messages are lost and 0 messages are answered twice.
- **SC-003**: For a correction sent during a multi-step turn, the agent acts on it at least one full step earlier than with today's queue-until-finished behaviour.
- **SC-004**: A person viewing the conversation can see where the agent read their mid-turn message, in every view of that conversation (live, reloaded, second tab), 100% of the time.
- **SC-005**: Behaviour of commands, side chats, scheduled tasks, dreams and subagent workers is unchanged (existing checks still pass).

## Assumptions

- "Busy" means the conversation already has a turn running; the change applies per conversation, not per agent (another conversation of the same agent is unaffected).
- The existing background-work delivery point is the model for this feature: same boundary, same exactly-once guarantee, same "one message per boundary" grouping.
- Messages are not interleaved into the middle of a round of tool calls; a round always finishes first.
- All person-facing channels (WebUI, Discord, Telegram, Workspaces, projects) get the behaviour; how each channel shows a waiting message beyond the WebUI is left to the current presentation of queued messages.
- The WebUI changes in FR-010 are a UI change and need a design pass (FigCraft, or an HTML/ASCII mockup if FigCraft is unavailable) before code, per the project's design-first rule.
- No new permissions: whoever may send a message to the conversation may steer its turn.
