# Feature Specification: /btw Ephemeral Side-Chat

**Feature Branch**: `033-btw-side-chat`

**Created**: 2026-10-28

**Status**: Draft

**Input**: User description: "Add /btw: an ephemeral side-chat command for the WebUI, Discord and Telegram. Full context is in https://github.com/vizier-lab/vizier/issues/108 — the issue body captures the agreed design (no tools, no persistence, same-agent identity/model, reuses the existing synchronous chat transport via a new VizierRequestContent::Btw dispatched like /checkpoint and /lobotomy — never through VizierAgent::chat() or save_history_or_log — WebUI modal / Discord ephemeral interaction reply / Telegram self-deleting reply, per-user rate limiting, usage still accounted). Use that issue as the source of truth for the feature description."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Ask a quick off-the-record question in the WebUI (Priority: P1)

While chatting with an agent, a person wants to ask it something on the side — sanity-check a draft reply, ask it to clarify something it just said, test a hypothesis out loud — without that question or its answer becoming part of the agent's permanent record. They type `/btw` followed by their question. A side panel opens showing the exchange; the agent answers using its usual identity and knowledge of the conversation so far, but does no other work (no searching memory, no running tools). The person can ask a few follow-ups in the same panel, and the agent remembers the earlier questions and answers *of this side chat* while it is open. When they close it, the exchange is gone — nothing about it remains anywhere the agent or anyone else can later see.

**Why this priority**: This is the core value of the feature and the only surface that has no feasible workaround today (opening a disposable second conversation doesn't guarantee it's never reflected on or seen later). It must work before the other channels are worth building.

**Independent Test**: Can be fully tested by opening the side-chat in an existing conversation, asking a question, verifying the answer reflects the agent's identity and the conversation's recent context, closing the panel, and confirming the exchange does not appear anywhere in the conversation's history or search.

**Acceptance Scenarios**:

1. **Given** an open conversation with an agent, **When** the person invokes the side-chat and asks a question, **Then** the agent answers in the side-chat using its own identity/knowledge and the conversation's recent messages for context.
2. **Given** an open side-chat exchange, **When** the person asks a follow-up question within it, **Then** the agent answers with awareness of the earlier side-chat turns in that same exchange.
3. **Given** a closed side-chat panel, **When** the person later reviews the conversation's history or the agent reflects on past conversations, **Then** no trace of the side-chat exchange appears anywhere.
4. **Given** an open side-chat, **When** the person asks it to do something requiring a tool (e.g. "search my notes for X" or "run this command"), **Then** the agent explains it cannot use tools in this mode rather than silently failing or performing the action.

---

### User Story 2 - Ask a quick off-the-record question on Discord (Priority: P2)

A person chatting with an agent in a Discord server wants the same off-the-record aside, but visible only to themselves — not posted publicly to the channel for everyone to see.

Discord shows a reply to only one person through private ("only you can see this") replies, and such a reply can't be replied to: answering it with an ordinary message would post that message publicly. So each private answer carries a **Follow up** button, which opens a small private form for the next question. Running the side-chat command again always starts a new, empty side chat.

**Why this priority**: Extends the core value to a channel where conversations are often semi-public, where the privacy guarantee matters even more than in a private WebUI session. Depends on the core mechanism from Story 1 existing.

**Independent Test**: Can be fully tested by invoking the side-chat command in a Discord channel and confirming only the invoking person can see the question and answer, while other channel members see nothing.

**Acceptance Scenarios**:

1. **Given** a person in a Discord channel with an agent, **When** they invoke the side-chat command, **Then** the question and the agent's reply are visible only to that person, not to other channel members.
2. **Given** a side-chat reply only the invoker can see, **When** the person chooses **Follow up** on it and asks a follow-up, **Then** the follow-up and its answer are likewise private to them, and the answer is aware of the earlier turns of that side chat.
3. **Given** a side-chat exchange on Discord, **When** enough time passes, **Then** the exchange is no longer accessible to anyone, including the person who started it.

---

### User Story 3 - Ask a quick off-the-record question on Telegram (Priority: P3)

A person chatting with an agent on Telegram wants the same aside. Telegram has no built-in way to show a message to only one person in a group, so the exchange is instead automatically removed shortly after it ends, and the person is told plainly that this is how privacy is provided there.

**Why this priority**: Completes channel parity, but Telegram's lack of a private-reply primitive makes this the weakest privacy guarantee of the three, so it's lowest priority and should not block the other two.

**Independent Test**: Can be fully tested by invoking the side-chat command on Telegram, confirming the person is told the exchange will be removed, and confirming it is in fact removed after the stated period of inactivity.

**Acceptance Scenarios**:

1. **Given** a person chatting with an agent on Telegram, **When** they invoke the side-chat command, **Then** they receive a reply along with a clear notice that the exchange will be automatically removed.
2. **Given** an active Telegram side-chat exchange, **When** the person is inactive for the stated period, **Then** all of the agent's replies in the exchange are deleted from the chat, and so are the person's command and follow-ups wherever Telegram lets the agent delete them (always in a private chat; in a group only when the agent's bot is an admin allowed to delete messages).
3. **Given** a Telegram side-chat invoked in a group, **When** the exchange is posted, **Then** it is visible to the group until removed (not private to the invoker), consistent with the notice given.
4. **Given** an active Telegram side-chat exchange, **When** the person replies to one of its answers, **Then** the reply is answered as a follow-up of that side chat, aware of its earlier turns, and is removed with the rest of it.

---

### Edge Cases

- What happens if the person tries to ask the side-chat to do something only a tool can do (e.g. search memory, run a command, read a file)? The agent must explain it can't, not attempt or fake the action.
- What happens if the agent, server, or connection becomes unavailable mid-exchange? The person sees a clear failure and may retry; no partial exchange is recoverable or resumed.
- What happens if the person invokes the side-chat many times in quick succession? Requests beyond a reasonable rate are declined with a clear "slow down" notice rather than silently dropped or queued indefinitely.
- What happens if the person closes the side-chat (or it's interrupted) mid-answer? The partial answer is discarded along with the rest of the exchange; nothing is salvaged or saved.
- What happens on Discord if the exchange runs long enough that the platform's own ephemeral-reply window expires? The person is shown that the exchange has expired rather than a silent failure.
- What happens if the side-chat is invoked in a brand-new conversation with no prior messages? It still works, simply with no prior conversation to draw context from.
- What happens on Telegram if the person replies to a side-chat message after the exchange has ended but before it has been removed (including after a server restart)? The reply is not treated as an ordinary message and is never kept; the person is told the side chat has ended and can start a new one, and the reply is removed with the rest.
- What happens on Telegram if someone else in a group replies to a person's side-chat message? It is neither answered nor kept, since it belongs to an exchange that is about to disappear; it is removed with the exchange where the agent is allowed to.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST let a person start a side-chat exchange with an agent from within an existing conversation, on each of the WebUI, Discord, and Telegram.
- **FR-002**: The side-chat MUST answer as the same agent the person is already talking to — same identity and knowledge of itself — not a different or generic assistant.
- **FR-003**: The side-chat MUST have awareness of the host conversation's recent messages, so the person can ask about something just said.
- **FR-004**: The side-chat MUST NOT have access to the agent's tools (searching memory, running commands, reading or writing files, contacting other services, etc.) — it answers from conversation and identity alone.
- **FR-005**: The system MUST let the person ask follow-up questions within one side chat, answered with awareness of that side chat's earlier turns, up to 9 questions in all. Invoking the side-chat command again MUST always start a new, empty side chat.
- **FR-006**: The system MUST NOT retain any record of a side-chat exchange once it ends — not in the conversation's visible history, not in anything the agent can later recall, reflect on, or search. The only things kept are the exchange's cost (FR-011) and, on Telegram, a note of which chat messages are still waiting to be removed (identifiers and a time only — no text, no sender), which is discarded once removal has been attempted.
- **FR-007**: On the WebUI, the side-chat MUST be presented in a way that is clearly separate from the main conversation (e.g. a distinct panel), so the person cannot mistake it for a message that will be remembered.
- **FR-008**: On Discord, a side-chat exchange MUST be visible only to the person who started it, not to other participants in the same channel.
- **FR-009**: On Telegram, a side-chat exchange MUST be automatically removed from the chat after a period of inactivity, and the person MUST be told this will happen when they start the exchange.
- **FR-010**: The system MUST limit how often one person can start or continue side-chat exchanges, to prevent it being used as an unthrottled way to generate answers.
- **FR-011**: The system MUST still record the computational cost of a side-chat exchange for usage/billing purposes, even though the exchange's content is not retained.
- **FR-012**: If a side-chat exchange is interrupted (connection lost, server restarts, person closes it) before completing, the system MUST NOT attempt to resume or recover it — the person simply sees a failure and may start again.
- **FR-013**: If the person asks the side-chat to perform an action that requires a tool, the agent MUST explain that it cannot in this mode, rather than attempting the action or appearing to succeed.
- **FR-014**: A side chat MUST end, and be discarded in full, at a point the person can predict on each channel: on the WebUI when its panel is closed or the page is left; on Discord after 5 minutes without activity, or 14 minutes after it started, whichever comes first; on Telegram after 2 minutes without activity. While it lasts, its earlier turns MUST live only in the person's browser (WebUI) or in the running server's memory (Discord, Telegram), never in storage.

### Key Entities

- **Side-chat exchange** ("side chat", "aside"): A short-lived, unrecorded conversation between a person and an agent. It is started by one invocation of the side-chat command and continued by that person's follow-ups (up to 9 questions), so it has memory *within itself*, unlike a one-shot question. It is never part of the conversation's history: it ends as described in FR-014 and is then discarded in full. Not an entity that persists beyond its own lifetime — it has no stored identity once closed.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A person can get an answer to a quick off-the-record question without leaving their current conversation, in under 10 seconds of perceived wait for a typical question, when the agent is not already working on a reply in that conversation. A side chat asked while the agent is mid-reply waits for that reply to finish first, within the agent's usual response time limit.
- **SC-002**: 100% of completed side-chat exchanges leave no retrievable trace in the agent's conversation history, search, or any later reflection on past conversations.
- **SC-003**: On Discord, 100% of side-chat replies are visible only to the person who started them, verified across multiple concurrent users in the same channel.
- **SC-004**: On Telegram, side-chat exchanges are removed from the chat within a predictable, stated time window after the person stops interacting with them, with zero exchanges left behind indefinitely.
- **SC-005**: A person attempting to use a side-chat for tool-requiring work (e.g. "search my files") is told it isn't possible, with zero cases of the system silently performing or faking the action.
- **SC-006**: The cost of every side-chat exchange is reflected in usage accounting, with zero exchanges missing from cost totals.

## Assumptions

- The person is already authenticated and has an existing, permitted conversation with the agent before starting a side-chat — side-chat does not introduce a new way to reach an agent a person couldn't already talk to.
- The side chat is meant for a short back-and-forth, not an extended working session: 9 questions (FR-005) and the channel lifetimes (FR-014) are sized for that, and both apply on top of the rate limit (FR-010).
- The model, provider, and general behavior tone the agent uses in a side-chat exchange match whatever is currently configured for that agent elsewhere — no new configuration surface is introduced for choosing a different model specifically for side-chat.
- "Automatically removed" on Telegram (FR-009) means removed from the chat transcript visible to users; it does not claim removal from Telegram's own servers or logs, which are outside this system's control.
- On Telegram, an agent's bot can always remove its own messages, and the person's messages in a private chat, but can remove other people's messages in a group only when it is a group admin allowed to delete messages. Without that right, the person's own side-chat messages stay in the group; the notice given at the start says so.
- Rate limiting (FR-010) applies per person as each platform identifies them: across all of a WebUI account's tabs and API keys, and per Discord or Telegram account. A single limit across platforms needs a verified link between a WebUI account and a Discord/Telegram account, which doesn't exist today (the platform ids in a profile are self-declared), so it is out of scope until such links are verified.
