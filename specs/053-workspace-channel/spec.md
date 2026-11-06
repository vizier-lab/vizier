# Feature Specification: Workspace Channel

**Feature Branch**: `053-workspace-channel`

**Created**: 2026-11-06

**Status**: Draft

**Input**: User description: "let's add a new Channel called "Workspace" its basically a channel for multiple agents and users. user (and agent) can mention each others. and only agent only "activate" by mentioning, and just silent read by default just like discord/telegram. because of this, delegate and consult agent will be removed, and instead add "mention_agent" a workspace exclusive agent to coordinate with each others"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Talk to several agents in one shared conversation (Priority: P1)

A person creates a Workspace, adds a few colleagues and a few agents, and posts messages that everyone in it can see. Agents stay quiet and read along. An agent replies only when someone mentions it, and when it does, it already knows what was said while it was reading.

**Why this priority**: This is the feature. Without one shared conversation that agents read silently and join on a mention, nothing else in this spec has anywhere to happen.

**Independent Test**: Create a Workspace with two people and two offline test agents. Post an unmentioned message, then a message mentioning one agent. Confirm that the first message gets no reply and starts no model call, that only the mentioned agent replies, that its reply shows it knew the earlier message, and that both people see the same conversation.

**Acceptance Scenarios**:

1. **Given** a Workspace with agents A and B, **When** a member posts a message that mentions neither, **Then** the message appears for every member, both agents record it as read, neither replies, and no model usage is recorded.
2. **Given** the same Workspace, **When** a member posts a message mentioning A, **Then** only A is activated, and its reply appears in the shared conversation attributed to A.
3. **Given** a message that mentions both A and B, **When** it is posted, **Then** each is activated once, independently, and neither waits for the other to finish.
4. **Given** A has silently read ten messages, **When** A is mentioned, **Then** A's reply can draw on those ten messages without anyone repeating them.
5. **Given** two members with the Workspace open, **When** either posts, or an agent replies or is working, **Then** both see it live, with no reload.
6. **Given** a member reopens a Workspace while an agent is mid-reply, **When** the page loads, **Then** the member sees the conversation so far and the rest of the reply as it arrives.

---

### User Story 2 - Agents coordinate with each other through mentions (Priority: P1)

While working on something in a Workspace, an agent mentions another member agent, in the open, in the shared conversation, by writing `@<agent-id>` in a message or by using `mention_agent`, a structured way to post the same message. The other agent wakes and answers there. Agents coordinate the way people do: to get someone's attention, you mention them, so an agent that wants the first one to act on its answer mentions it back. People watching can read the whole exchange and step in at any point.

**Why this priority**: This replaces `consult_agent` and `delegate_agent`, so it must land with their removal. Otherwise agents lose the ability to coordinate at all.

**Independent Test**: In a Workspace with offline test agents A and B, have a person mention A. Have A mention B once in reply text and once through `mention_agent`. Confirm both wake B in the same way, that B's reply wakes A only when it mentions A, that A has read B's reply either way, and that a chain of mutual mentions stops at the limit.

**Acceptance Scenarios**:

1. **Given** A is running a turn in a Workspace and B is a member, **When** A posts a message containing `@<B's agent-id>`, with or without `mention_agent`, **Then** the message appears in the shared conversation attributed to A with B mentioned, and B is activated.
2. **Given** B was activated by A, **When** B replies and mentions A, **Then** A is activated. **When** B replies without mentioning A, **Then** A silently reads the reply and is not activated.
3. **Given** A is running a turn in a Workspace, **When** A calls `mention_agent` for an agent that is not a member, **Then** the call returns a refusal A can act on, listing the agents that can be mentioned, and nothing is posted.
4. **Given** an agent turn in any other conversation (a direct chat, Discord, Telegram, a project chat, a scheduled task, a dream, a subagent), **When** the agent lists its tools, **Then** `mention_agent` is not among them, and `@<agent-id>` in its text wakes no one.
5. **Given** two agents that keep mentioning each other, **When** the chain reaches the limit in FR-016, **Then** further mentions by agents activate no one, however they were written, the conversation shows that the chain stopped, and a person's next message starts a new chain.
6. **Given** B is stopped or failing, **When** A mentions it, **Then** the conversation shows B is unavailable, and `mention_agent` also tells A so, rather than the mention waiting forever.

---

### User Story 3 - Mention a person and be told about it (Priority: P2)

A member, or an agent, mentions a person in a Workspace. The person is notified and can jump straight to that message, even when the Workspace is not open.

**Why this priority**: The conversation is useful with the Workspace open, but a mix of people and agents only works asynchronously if a mention actually reaches someone. Agents often need a human decision.

**Independent Test**: Have an agent's reply mention a member who does not have the Workspace open. Confirm that the member gets one notification which links to the message, and that mentioning a person never activates an agent.

**Acceptance Scenarios**:

1. **Given** member P does not have the Workspace open, **When** another member or an agent mentions P, **Then** P receives one notification linking to that message.
2. **Given** P has that Workspace open, **When** P is mentioned, **Then** the mention is highlighted and no separate unread notification is left behind.
3. **Given** someone composes a message, **When** they type the mention marker, **Then** they get suggestions drawn from that Workspace's members, both people and agents, and the two are visually distinct.
4. **Given** a message contains an at-sign followed by something that is not a member, **When** it is posted, **Then** it remains plain text, and it activates and notifies no one.

---

### User Story 4 - Manage a Workspace and who is in it (Priority: P2)

A person creates, renames and removes Workspaces, adds and removes people and agents, and can see at a glance who is in one. Access is respected: nobody reads a Workspace they are not in, and bringing an agent in follows the access rule in FR-005.

**Why this priority**: The shared conversation makes agents reachable by people who could not reach them before, so membership is the safety boundary. It can follow the core flow only because a first version could start with a fixed membership.

**Independent Test**: Using three fixture users with different permissions, attempt each management action and each read. Confirm that non-members get nothing, including a request for a known Workspace id, and that removed members lose access immediately.

**Acceptance Scenarios**:

1. **Given** a person with permission to create Workspaces, **When** they create one, **Then** they are its owner and first member.
2. **Given** a person who is not a member, **When** they request a Workspace's conversation by id, **Then** they are refused and cannot tell whether it exists.
3. **Given** a member is removed, **When** the removal takes effect, **Then** they immediately stop receiving live updates and can no longer read the Workspace, including on connections already open.
4. **Given** an agent is removed from a Workspace, **When** it is mid-reply there, **Then** that reply stops, and it can no longer be mentioned or read new messages there.
5. **Given** an agent is deleted, **When** members open a Workspace it belonged to, **Then** its past messages still show with its name, marked as no longer available.

---

### User Story 5 - Upgrade from `consult_agent` and `delegate_agent` (Priority: P2)

An operator upgrades. The two old tools are gone, agents that still try to call them are told what to do instead, and past conversations and jobs that used them remain readable.

**Why this priority**: This is a deliberate breaking change, so it has to fail helpfully and lose nothing. It ranks below the flows that make the removal acceptable.

**Independent Test**: Upgrade a data set holding old consult conversations and delegation jobs. Confirm that history and job records still read correctly, and that a script calling either old name gets guidance rather than an unexplained failure.

**Acceptance Scenarios**:

1. **Given** the upgraded system, **When** an agent lists its tools in any conversation, **Then** neither `consult_agent` nor `delegate_agent` appears.
2. **Given** an agent's script or saved skill still calls either old name, **When** it runs, **Then** the failure is catchable and says the tool was replaced by `mention_agent` in a Workspace.
3. **Given** past inter-agent conversations and delegation jobs, **When** someone with access views them, **Then** they read as they did before the upgrade.
4. **Given** a delegation job was running when the upgrade restarted the server, **When** the server is back, **Then** that job is shown as interrupted, like any job a restart cuts off.

---

### Edge Cases

- **Mention storms**: a person's single message may mention many agents. Each is activated once, however many times it is named in that message.
- **Self-mention**: an agent calling `mention_agent` on itself is refused; a self-mention in reply text does nothing.
- **An agent named in passing**: a bare display name wakes no one, and neither does `@<agent-id>` inside a code span, a code block or a quoted passage, so agents and people can talk *about* an agent without waking it. Outside those, the exact `@<agent-id>` does wake it, even if the author meant it as an aside.
- **Streaming and stopped replies**: mentions count only once a message is complete. A partial reply still streaming wakes no one, and neither does a reply that was stopped or failed before finishing. A person mentioned in one is not notified.
- **An answer with no mention back**: if B answers A without mentioning it, A does not wake, and its task may rest there until someone mentions it. A has still read the answer, so nothing is lost. People can see this in the timeline, and agents are told about the convention (FR-009).
- **Busy agent**: a mention of an agent already working in this Workspace queues behind its running turn, in order, as any request does today. Messages it reads in the meantime are not lost.
- **Stopping**: any member can stop an agent's running turn in the Workspace, and can stop all of them at once, which also ends any mention chain in progress.
- **Editing and deleting**: messages cannot be edited in this version, so what an agent read is what was posted. Deleting a Workspace is limited to its owner or an administrator.
- **Restart**: an activated turn resumes after a restart as durable turns do today. A message posted by a person is never lost to a restart, and is not read twice.
- **Agent in a failed state**: it stays a listed member, shown as unavailable; messages posted meanwhile still count as read on its recovery.
- **Attachments**: a file posted to a Workspace is available to every member, agents included. A person is told so before they send it.
- **Questions**: when an agent asks a question card in a Workspace, any human member may answer it, and the card records who did. A question cannot be answered by another agent.
- **Secrets**: an agent cannot ask for sensitive input in a Workspace, because the request would play out in front of the whole room. It can tell the person to use a direct chat.
- **Long conversations**: an agent's own context limits still apply. A long-idle agent is not billed for messages it read until it is activated, and then only for what its normal context handling keeps.
- **Long mention chain with a person watching**: a person's message anywhere in the chain resets the limit, including one that does not mention an agent.

## Requirements *(mandatory)*

### Functional Requirements

**Workspace and membership**

- **FR-001**: The system MUST offer Workspaces: named, persistent conversations with a shared timeline and any number of member people and member agents. An installation can have many.
- **FR-002**: Each Workspace MUST keep one timeline in one order, shown identically to all members, with each message attributed to the person or agent who posted it.
- **FR-003**: Members with management rights MUST be able to rename a Workspace, add and remove people and agents, and delete it. The creator is the first owner.
- **FR-004**: Only members and those holding an all-Workspaces administrative permission MUST be able to read or post in a Workspace. A non-member MUST NOT be able to tell that a given Workspace exists. Removing a member MUST take effect immediately, including on open connections.
- **FR-005**: Adding an agent to a Workspace MUST be restricted to people who may edit that agent (its owner, or the all-agents edit permission). Adding is the act of sharing it: from then on every member may mention it, including members who could not otherwise view it. The person adding it MUST be told that members can use its tools and integrations, spend its provider budget, and see what it recalls from memory.
- **FR-006**: Workspace access rules MUST behave identically on every surface, including the desktop app.

**Reading and activation**

- **FR-007**: A member agent MUST silently read every message posted in its Workspace: it is recorded as seen by that agent, with no reply and no model call.
- **FR-008**: An agent MUST be activated only by a mention: `@<agent-id>` in a completed message from a person or from another agent. Agents' per-agent random chance to answer unmentioned messages MUST NOT apply in a Workspace, and neither do any other implicit triggers (a reply to its message, or a skill command without a mention).
- **FR-009**: An activated agent MUST have access to what it silently read in that Workspace, subject to its normal context limits, and MUST be told who is in the Workspace with the mention to use for each, who mentioned it, and that an agent wakes only when mentioned, so it must mention whoever it needs to act next.
- **FR-010**: A message mentioning several agents MUST activate each exactly once, concurrently, independent of the others.
- **FR-011**: A mention MUST be resolved when the message is complete, against current members. Text that resolves to no member, or that sits inside a code span, a code block or a quoted passage, MUST stay plain text and activate no one.
- **FR-012**: Mentions MUST work the same whoever writes them and however the message was built: a person's message, an agent's reply text and `mention_agent` activate agents and notify people by the same rule.
- **FR-013**: Skills MUST remain invokable by mention (a mention followed by the skill command). `/skills` and side chats are out of scope in a Workspace.

**`mention_agent`**

- **FR-014**: The system MUST provide a `mention_agent` tool: a structured way to post a message into the current Workspace, attributed to the calling agent, that contains the `@<agent-id>` mention of one or more member agents. It MUST add no activation behaviour of its own: the resulting message is treated exactly as if the same text had been written, and any answer reaches the caller only by mentioning it. The call MUST NOT end the calling turn and MUST return without waiting for the mentioned agent.
- **FR-015**: `mention_agent` MUST be available only during an agent's own turn in a Workspace. It MUST NOT appear or work in any other conversation, nor in dreams or in subagent pieces launched from a Workspace turn. `@<agent-id>` in text MUST activate no one outside a Workspace.
- **FR-016**: The system MUST bound agent-to-agent chains. A chain counts consecutive activations caused by agents with no message from a person in between. At 8 by default, a mention by an agent MUST activate no one, however it was written, and the timeline MUST show that the chain stopped. Any message from a person MUST reset the count.
- **FR-017**: A refused `mention_agent` call (non-member, self, unavailable agent, chain limit) MUST return as data the calling script can handle, MUST NOT fail the turn, and except for the chain notice MUST post nothing. A mention written in text has no caller to refuse: an unresolved one stays plain text (FR-011), and a mention of an unavailable member or one over the chain limit shows in the timeline.

**Removal of consult and delegate**

- **FR-018**: `consult_agent` and `delegate_agent` MUST be removed from every conversation. A call to either name MUST fail with guidance pointing to `mention_agent` in a Workspace.
- **FR-019**: Existing inter-agent conversations and delegation job records MUST remain readable, and any compatibility code kept for them MUST be listed on the pre-1.0 removal tracker (#82).
- **FR-020**: `background_subagents`, `parallel_subagents`, `list_background_jobs` and `cancel_background_job` MUST keep working, for an agent's own subagents.

**Experience**

- **FR-021**: People MUST be able to reach their Workspaces from the WebUI and the desktop app, with mention suggestions while composing and visible status for each agent that is working.
- **FR-022**: Being mentioned MUST create one notification for that person, linked to the message, following the visibility rules of FR-004. A person viewing that Workspace MUST NOT accumulate an unread notification for it.
- **FR-023**: Members MUST be able to stop one agent's turn, or every running turn in the Workspace.
- **FR-024**: Model usage from Workspace turns MUST be attributed to the agent that ran them and reported as its own activity in usage reports. Silent reading MUST record no model usage.
- **FR-025**: Deleting a Workspace MUST remove its timeline, stop its running turns, and remove its files, but MUST NOT affect its member agents' memory, or usage already recorded.
- **FR-026**: Agents MUST be able to reflect on Workspace conversations in their dream cycle, as they do for other conversations with people.
- **FR-027**: Agent behaviour in a Workspace MUST be verifiable end to end with the offline test provider, covering silent read, activation by mention, `mention_agent`, and the chain limit.

### Key Entities

- **Workspace**: A named shared conversation. Has an owner, members and one timeline.
- **Workspace Member**: A person or an agent belonging to a Workspace. A person may hold management rights. An agent may be available or unavailable.
- **Workspace Message**: One entry on the timeline: its author (person or agent), content, attachments, resolved mentions and time.
- **Mention**: A resolved reference from a message to a member. Activates an agent, or notifies a person.
- **Mention Chain**: The run of agent-caused activations since a person last posted, which FR-016 limits.
- **Read Position**: What a given agent has seen of the timeline, so nothing is missed or read twice, including across restarts.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of messages with no mention produce no agent reply and no recorded model usage, whatever the number of member agents.
- **SC-002**: 100% of messages mentioning an available agent activate that agent exactly once, and activate no one else.
- **SC-003**: A member sees a new message, or a reply as it streams, within 1 second of it being produced, under normal conditions, with up to 20 members and 10 agents in a Workspace.
- **SC-004**: An agent activated after reading 50 unmentioned messages can correctly answer a question about any of them that fits within its context limits.
- **SC-005**: A runaway exchange between two agents always stops at the chain limit, with no person acting, and leaves a visible notice.
- **SC-006**: A person who is not a member gets no Workspace content through any surface, in all tested permission combinations, and a removed member loses access within 1 second.
- **SC-007**: After upgrade, 100% of past inter-agent conversations and delegation records stay readable, and neither removed tool is offered to any agent in any conversation.
- **SC-008**: A person can create a Workspace, add an agent and get a reply from it in under 2 minutes, on their first try.

## Assumptions

- **Surfaces**: Workspaces are a WebUI and desktop-app feature backed by the HTTP API. A Workspace is not bridged to Discord or Telegram in this version.
- **Consequence of the removal, as requested**: an agent outside a Workspace (a direct chat, Discord, Telegram, a scheduled task) has no way to reach another agent. It can only suggest that a person use a Workspace.
- **Strict activation**: "activate only by mentioning" overrides the per-agent random initiative setting used on Discord and Telegram. Random replies from several agents in one room would add noise and cost.
- **Mentions use ids**: an agent wakes on its exact `@<agent-id>`, and a person is mentioned by `@<username>`. The composer inserts these from suggestions while showing display names.
- **No automatic reply routing**: the system does not track which message answers which, so B's answer reaches A's attention only if B mentions A. This is deliberate, for simplicity, and accepts that a task can rest until someone mentions the next agent.
- **Agents are private to their own memory**: membership does not merge agents' memory or secrets. But what an agent says in a Workspace is seen by every member, and memories it recalls can surface in its replies.
- **Naming**: "Workspace" is already the term for the data folder on disk, and project tools use `workspace_shell`. The user-visible name is kept as requested; docs must not let the two be confused, and internal names must not use the bare word.
- **One timeline, no topics**: a Workspace has no threads in this version. Separate subjects use separate Workspaces.
- **Generative UI and client tools**: tools declared by one person's browser are out of scope in a Workspace, since they bind to one connection in a shared room. Agents reply with text and attachments.
- **`@everyone`-style group mentions**: out of scope for this version.
- **Chain limit of 8**: a starting value, tuned from use. It is deliberately above the existing nesting limit for background jobs, since a back-and-forth takes more hops than a nested job.
- **Design first**: per the project constitution, new screens are drawn or mocked up before any UI code, which is the first task after planning.
