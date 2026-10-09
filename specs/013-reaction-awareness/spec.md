# Feature Specification: Reaction Awareness

**Feature Branch**: `013-reaction-awareness`

**Created**: 2026-10-09

**Status**: Draft

**Input**: User description: "iirc we implemented agent to received reaction, how's the state of that feature iirc we only implemented it for webui, even then i believed it is buggy (on the ui side), and the discord and telegram side is not yet implemented" — followed by "go ahead write the spec" after the state was reviewed.

## Clarifications

### Session 2026-10-09

- Q: Should a reaction ever wake the agent, or only be shown on its next turn? → A: Never wake. Reactions are stored and shown on the agent's next turn in that conversation, on every channel, with no per-agent override.

## Context

People already react to messages with emoji on every channel an agent lives on. A 👍 or ❤️ on a reply is the cheapest feedback a person can give, and a 👎 is often the only feedback they give. Today almost all of it is lost:

| Channel | A person can react | The reaction is kept | The agent ever learns of it |
|---|---|---|---|
| WebUI | Yes | Only on messages that were loaded from history; reactions on messages that arrived in the current page session vanish on reload | **No** |
| Discord | Yes (native) | No | **No** |
| Telegram | Yes (native) | No | **No** |

The agent side is also partial. On Discord an agent can put a real reaction on a message. On Telegram its "react" tool actually posts a new text message saying `Reaction: 👍`, which is worse than not reacting at all.

This feature closes the loop. A reaction a person makes on any of the three channels is kept with the message it was made on and shown to the agent the next time it reads that conversation. The WebUI shows reactions correctly and keeps them. Agents on Telegram react the same way they do on Discord.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - The agent knows how its replies landed (Priority: P1)

A person reacts to one of the agent's replies with 👎, or with ✅ to confirm a suggestion. The next time the agent works in that conversation, it can see which of its messages got which reactions from whom, and it can take that into account: correct itself, stop doing something the person disliked, or treat a ✅ as a go-ahead.

**Why this priority**: This is the point of the feature. Every other story is only useful if reactions eventually reach the agent. Today nothing does, on any channel.

**Independent Test**: On the offline `dummyplug` provider in the WebUI, have the agent reply, react to that reply, then send another message. Check that what the agent receives for its next turn includes the reaction, attributed to the person and tied to the message it was made on.

**Acceptance Scenarios**:

1. **Given** the agent has replied in a conversation, **When** a person adds a reaction to that reply and later sends a new message, **Then** the agent's view of the conversation for that turn shows the reaction, who made it, and which of its messages it was on.
2. **Given** a person added a reaction and then removed it before the agent's next turn, **When** the agent next reads the conversation, **Then** the reaction is not shown as present.
3. **Given** several people react to the same reply in a group conversation, **When** the agent next reads the conversation, **Then** it sees each person's reactions separately, not only an anonymous count.
4. **Given** a reaction is made, **When** no further message arrives in that conversation, **Then** the agent takes no turn, and the reaction waits until the agent next works in that conversation.

---

### User Story 2 - WebUI reactions stick and are honest (Priority: P1)

A person reacts to a message in the WebUI, including one the agent sent seconds ago. The reaction stays: it is still there after a reload, on another device, and in another tab already open on the same conversation. Removing a reaction removes it everywhere. Reacting works right after the connection drops and comes back, without a page reload.

**Why this priority**: The WebUI is the only channel where reactions exist today, and it silently loses them. The most common case, reacting to the reply that just arrived, is exactly the one that is lost. A reaction that looks saved but isn't is worse than no reactions at all.

**Independent Test**: In the WebUI, send a message, wait for the reply, react to the reply, then reload the page. Check the reaction is still there. Open the same topic in a second tab, react in the first, and check the second updates without a reload.

**Acceptance Scenarios**:

1. **Given** a reply arrived during the current page session, **When** the person reacts to it and reloads the page, **Then** the reaction is still shown.
2. **Given** the person's own message was sent during the current page session, **When** the agent's reply to it arrives, **Then** both messages can be reacted to (subject to FR-012) and those reactions persist.
3. **Given** a reaction the person made is shown, **When** they click it again to remove it, **Then** it is removed from the display and from storage, and the agent is told it was a removal, not an addition.
4. **Given** the WebUI lost its connection and reconnected, **When** the person reacts to any message already on screen, **Then** the reaction is sent and saved, with no reload needed.
5. **Given** the same topic is open in two tabs or on two devices, **When** a reaction is added or removed in one, **Then** the other shows the change within a few seconds without a reload.
6. **Given** a reaction cannot be saved (for example the connection is down), **When** the person reacts, **Then** the UI does not show it as saved. It either refuses with a visible message or rolls back.

---

### User Story 3 - Discord reactions reach the agent (Priority: P2)

A person on Discord reacts to the agent's reply with a native emoji reaction, the same way they would to anyone else's message. The reaction is recorded against that reply and the agent sees it per Story 1. Removing the reaction on Discord removes it on the agent's side too.

**Why this priority**: Discord is where most group traffic is. Reactions are the dominant form of feedback there, and none of it is captured today. It is P2 only because it depends on Story 1's agent-side handling.

**Independent Test**: In a Discord channel with a running agent, have the agent reply, add 👍 to the reply, then mention the agent again. Check the agent's next turn shows the 👍 on that reply. Remove the 👍 and check it disappears.

**Acceptance Scenarios**:

1. **Given** the agent replied in a Discord channel, **When** a person adds a reaction to that reply, **Then** the reaction is recorded against that reply, attributed to that person.
2. **Given** the agent's reply was long enough to be split into several Discord messages, **When** a person reacts to any one of them, **Then** the reaction is recorded against that reply.
3. **Given** a reaction was recorded, **When** the person removes it on Discord, **Then** it is removed on the agent's side too.
4. **Given** a custom (server-specific) emoji is used, **When** it is recorded, **Then** the agent sees a readable name for it rather than an opaque number.
5. **Given** the agent itself adds a reaction with its react tool, **When** Discord reports that reaction back, **Then** it is not recorded as feedback from a person.

---

### User Story 4 - Telegram reactions reach the agent (Priority: P2)

The same as Story 3, for Telegram private chats and groups.

**Why this priority**: Same as Story 3. It is a separate story because Telegram only reports reactions under conditions the operator has to set up (see Assumptions), so it can ship independently.

**Independent Test**: In a Telegram private chat with a running agent, react to the agent's reply, then send a new message. Check the agent's next turn shows the reaction. Change the reaction to a different emoji and check the agent sees the new one, not both.

**Acceptance Scenarios**:

1. **Given** the agent replied in a Telegram chat, **When** a person reacts to that reply, **Then** the reaction is recorded against it, attributed to that person.
2. **Given** a person already reacted to a message, **When** they replace it with a different emoji, **Then** the old reaction is removed and the new one recorded.
3. **Given** the agent is in a group where it cannot receive reactions, **When** people react there, **Then** nothing breaks. The agent simply doesn't see those reactions, and the operator can find out why from the agent settings guidance.

---

### User Story 5 - Agents react for real on Telegram (Priority: P3)

When an agent decides to react to a person's Telegram message, the reaction appears as a native reaction on that message, the way it already does on Discord. No extra "Reaction: 👍" message is posted to the chat.

**Why this priority**: This is the agent-to-person direction and is separate from awareness. But the current behaviour actively spams the chat, so it belongs in the same pass.

**Independent Test**: Have an agent on Telegram call its react tool on a message. Check that a native reaction appears on that message and no new message is posted.

**Acceptance Scenarios**:

1. **Given** an agent on Telegram reacts to a message, **When** the tool runs, **Then** a native reaction appears on that message and no new chat message is sent.
2. **Given** the emoji the agent chose is not one Telegram allows as a reaction, **When** the tool runs, **Then** the tool reports a clear failure to the agent and nothing is posted to the chat.

---

### Edge Cases

- **Reaction on a message that's not in the agent's history**: for example a Discord message from before the agent joined, a message from another bot, or a person's message the agent only silently read. It is ignored and not recorded (see FR-012).
- **Reaction storms**: many people react to the same message in a busy channel. Each reaction is recorded, but there is never one agent turn per reaction (FR-008). What the agent sees is a short summary, not an unbounded list (FR-007).
- **Rapid toggling**: a person adds and removes the same emoji several times quickly. The final recorded state matches the final state on the platform, and no add or remove is lost to two updates racing each other.
- **Reaction on a message whose conversation was reset** (e.g. after `/lobotomy`) or deleted: it is ignored and nothing errors.
- **The reacting person is the agent itself**, or another agent's bot account: never recorded as human feedback.
- **The platform drops a removal event** (bot was offline): the stale reaction may stay recorded. This is accepted, and there is no back-fill of missed events.
- **The server restarts between the reaction and the agent's next turn**: the reaction is still shown, because it was stored when it was made.
- **A reaction arrives while the agent is mid-turn in that conversation**: it is stored at once and shown from the next turn on. It never interrupts or alters the running turn.

## Requirements *(mandatory)*

### Functional Requirements

**Recording**

- **FR-001**: The system MUST record each person's reaction against the specific message it was made on, as the pair (person, emoji), for messages on the WebUI, Discord and Telegram.
- **FR-002**: Adding and removing a reaction MUST be distinct operations. A removal MUST NOT be able to create a reaction, and an addition MUST NOT be able to remove one.
- **FR-003**: Concurrent reaction changes on the same message MUST NOT overwrite each other. After any sequence of changes, the recorded set MUST equal the platform's final set for events the system received.
- **FR-004**: Reactions MUST survive a server restart.
- **FR-005**: Every message the agent sends on Discord and Telegram MUST be linkable back to the agent's reply it belongs to, including replies split across several platform messages, so that reactions on them can be recorded.
- **FR-006**: Reactions by the agent's own account, or by any account the system knows to be one of its agents, MUST NOT be recorded as feedback from a person.

**Agent awareness**

- **FR-007**: When the agent reads a conversation for a turn, it MUST see the reactions currently present on the messages it can see, tied to those messages and attributed to the people who made them. When a message has many reactions, they MUST be summarised (counts per emoji plus a bounded list of who reacted) so the context stays a predictable size.
- **FR-008**: A reaction MUST NOT on its own start an agent turn. It is surfaced the next time the agent takes a turn in that conversation for any other reason. This holds on every channel, including one-to-one conversations, and there is no per-agent override.
- **FR-009**: Removing a reaction before the agent's next turn MUST mean the agent never sees it as present.
- **FR-010**: Reactions MUST NOT be shown to the agent as if they were messages the person typed, and MUST NOT appear in the agent's view as instructions.
- **FR-011**: The dream cycle MUST see reactions on the conversations it reflects on, so feedback can make it into the agent's long-term memory.
- **FR-012**: Only reactions on messages the agent sent MUST be recorded and shown to the agent. Reactions on people's messages are ignored on every channel. In the WebUI, people MUST NOT be offered a reaction control on their own messages.

**WebUI**

- **FR-013**: The WebUI MUST be able to react to any agent message on screen, including messages that arrived during the current page session, and those reactions MUST persist.
- **FR-014**: The WebUI MUST NOT show a reaction as saved unless the server accepted it. If saving fails, the display MUST roll back or show a visible error.
- **FR-015**: Reaction changes MUST appear in every open WebUI view of the same conversation within a few seconds, without a reload.
- **FR-016**: Reacting MUST work after a lost connection is re-established, without a page reload.
- **FR-017**: The WebUI MUST show reactions made on Discord and Telegram when a person views those conversations in the WebUI.

**Platforms**

- **FR-018**: On Discord, adding or removing a reaction on an agent's message MUST be reflected per FR-001 and FR-002, for standard and custom emoji. Custom emoji MUST be recorded with a human-readable name.
- **FR-019**: On Telegram, adding, removing or replacing a reaction on an agent's message MUST be reflected per FR-001 and FR-002, in private chats and in groups where the bot is able to receive reactions.
- **FR-020**: The agent's Telegram react tool MUST place a native reaction on the target message and MUST NOT post a chat message. If the platform rejects the emoji, the tool MUST return a clear error to the agent.
- **FR-021**: The agent settings MUST state, for Telegram, what the operator has to do for reactions in groups to reach the agent.

### Key Entities

- **Reaction**: one person's emoji on one message. Attributes: the message it is on, the person (channel-specific identity plus a display name where known), the emoji (a standard emoji or a named custom one), and when it was added. A person has at most one record per emoji per message.
- **Agent message link**: ties a message the agent sent on a platform to the agent reply it is part of. One reply may have several links when the platform split it.
- **Reaction summary**: what the agent sees for a message, built from the current reactions. Per-emoji counts plus a bounded list of reactors.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of reactions made on agent messages in the WebUI, including on messages received in the current page session, are still present after a page reload.
- **SC-002**: In a conversation on each of the three channels, a reaction made on the agent's reply is visible to the agent on its next turn in 100% of trials (Telegram groups: where the bot is set up to receive reactions).
- **SC-003**: A reaction removed before the agent's next turn is visible to that turn in 0% of trials.
- **SC-004**: With two WebUI views open on one conversation, a reaction change in one appears in the other within 3 seconds.
- **SC-005**: 50 reactions on one message by 50 different people add no more than a fixed, small amount of context to the agent's turn (on the order of one short paragraph), regardless of how many more reactions are added.
- **SC-006**: 50 reactions arriving in a busy channel within one minute cause zero extra agent turns. Reactions never add turns (FR-008).
- **SC-007**: The agent's Telegram react tool posts zero chat messages.

## Assumptions

- **Only agent messages count.** Reactions on people's messages carry little signal for the agent and are a large source of noise in group channels, so they are ignored on every channel (FR-012). Removing the reaction control from a person's own WebUI messages follows from this.
- **Reactions don't wake the agent by default.** A reaction storm in a busy channel would otherwise cost one model call per reaction, the same reason automatic context on silently-read messages is off by default. This was confirmed as the policy for every channel, with no per-agent toggle (FR-008).
- **Telegram limits.** Telegram only reports reactions to bots in groups where the bot is an administrator, and only if the bot asks for reaction updates. In private chats they are delivered normally. The feature does not attempt to work around this. It documents it (FR-021).
- **Telegram's reaction set.** Telegram allows only a fixed set of emoji as reactions, so the agent's react tool can fail for some emoji, and it reports that failure (FR-020).
- **No back-fill.** Reactions made while the server was offline, or on messages sent before this feature shipped, are not recovered. Messages the agent sent before this feature have no platform link, so reactions on them are ignored.
- **Existing WebUI reactions are kept.** Reactions already stored on WebUI history entries stay valid and remain visible.
- **Agents' outbound reactions** on Discord are already native and are out of scope, except for making sure they are not recorded as human feedback (FR-006).
- **Reactions on other bots' messages** and on messages in conversations the agent is not part of are out of scope.
