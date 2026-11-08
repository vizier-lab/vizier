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
7. **Given** agents A and B, **When** B runs tools and writes a mid-turn and a final message, **Then** A's record holds those two messages and none of B's tool calls.
8. **Given** A has a skill `greet` and B doesn't, **When** a member posts `/greet @A Rafi` and `/greet @B`, **Then** A runs the skill and B answers that it has no such skill, with no model call.
9. **Given** a member posts `/checkpoint @A`, **When** it runs, **Then** only A's record of this topic changes, members see a note, and no other agent reads it.
10. **Given** an agent writes `/abort @B`, **When** it posts, **Then** B's running turn continues.
11. **Given** a Workspace with two topics, **When** A is mentioned in one, **Then** A's context includes what it read in that topic and not what was said in the other.

---

### User Story 2 - Agents coordinate with each other through mentions (Priority: P1)

While working on something in a Workspace, an agent mentions another member agent, in the open, in the shared conversation, by writing `@<agent-id>` in a message or by using `mention_agent`, a structured way to post the same message. The other agent wakes and answers there. Agents coordinate the way people do: to get someone's attention, you mention them, so an agent that wants the first one to act on its answer mentions it back. People watching can read the whole exchange and step in at any point.

**Why this priority**: This replaces `consult_agent` and `delegate_agent`, so it must land with their removal. Otherwise agents lose the ability to coordinate at all.

**Independent Test**: In a Workspace with offline test agents A and B, have a person mention A. Have A mention B once in reply text and once through `mention_agent`. Confirm both wake B in the same way, that B's reply wakes A only when it mentions A, that A has read B's reply either way, and that a person can stop two agents that keep mentioning each other.

**Acceptance Scenarios**:

1. **Given** A is running a turn in a Workspace and B is a member, **When** A posts a message containing `@<B's agent-id>`, with or without `mention_agent`, **Then** the message appears in the shared conversation attributed to A with B mentioned, and B is activated.
2. **Given** B was activated by A, **When** B replies and mentions A, **Then** A is activated. **When** B replies without mentioning A, **Then** A silently reads the reply and is not activated.
3. **Given** A is running a turn in a Workspace, **When** A calls `mention_agent` for an agent that is not a member, **Then** the call returns a refusal A can act on, listing the agents that can be mentioned, and nothing is posted.
4. **Given** an agent turn in any other conversation (a direct chat, Discord, Telegram, a project chat, a scheduled task, a dream, a subagent), **When** the agent lists its tools, **Then** `mention_agent` is not among them, and `@<agent-id>` in its text wakes no one.
5. **Given** two agents that keep mentioning each other, **When** a member stops all turns in the Workspace, **Then** both turns end, mentions still queued are dropped, and neither agent wakes again until it is next mentioned.
6. **Given** B is stopped or failing, **When** A mentions it, **Then** the conversation shows B is unavailable, and `mention_agent` also tells A so, rather than the mention waiting forever.

---

### User Story 3 - Be told about new messages, and mention people (Priority: P2)

Everyone in a Workspace is notified of each message someone else posts there, and can jump straight to it, even when the Workspace is not open. That includes an agent's answer, so someone who asked a question and left hears back. Mentioning a person highlights it for them, in the conversation.

**Why this priority**: The conversation is useful with the Workspace open, but a mix of people and agents only works asynchronously if new messages actually reach someone. Agents often need a human decision.

**Independent Test**: Have one member post, and an agent reply mid-turn and finally, while another member does not have the Workspace open. Confirm that the other member gets one notification per message, each linking to its message, and that the author gets none for their own.

**Acceptance Scenarios**:

1. **Given** member P does not have the Workspace open, **When** another member or an agent posts a message, **Then** P receives one notification linking to that message, whether or not it mentions P.
2. **Given** P posts a message, **When** it is saved, **Then** P gets no notification for it, and everyone else in the Workspace does.
3. **Given** P has that topic open, **When** a message arrives, **Then** no unread notification is left behind, and a mention of P is highlighted.
4. **Given** P has unread notifications from a topic, **When** P opens that topic, **Then** they are marked read and the rail's count falls.
5. **Given** someone composes a message, **When** they type the mention marker, **Then** they get suggestions drawn from that Workspace's members, both people and agents, and the two are visually distinct.
6. **Given** a message contains an at-sign followed by something that is not a member, **When** it is posted, **Then** it remains plain text and activates no one.
7. **Given** P is removed from a Workspace, **When** it takes effect, **Then** P no longer sees its notifications or receives new ones.

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
2. **Given** an agent's script or saved skill still calls either old name, **When** it runs, **Then** the failure is catchable and says the tool was replaced by `mention_agent` inside a Workspace and `send_message_to_workspace` outside one.
3. **Given** past inter-agent conversations and delegation jobs, **When** someone with access views them, **Then** they read as they did before the upgrade.
4. **Given** a delegation job was running when the upgrade restarted the server, **When** the server is back, **Then** that job is shown as interrupted, like any job a restart cuts off.

---

### User Story 6 - Orchestrate a Workspace from outside it (Priority: P2)

An agent belonging to a Workspace acts on it from somewhere else. It creates a scheduled task with its existing scheduling tools, and each run posts into a Workspace topic, mentioning the member agents that should act. For example, a morning task posts "@researcher collect yesterday's incidents" to `ops` / `daily`. It can also post from a direct chat when a person asks, and can check what a topic has said before it acts.

**Why this priority**: With `consult_agent` and `delegate_agent` gone, this is the only way for work that starts outside a Workspace to reach other agents, and it is what makes unattended orchestration possible. It ranks below the core flow because it depends on that flow.

**Independent Test**: With offline test agents A and B in one Workspace, have a scheduled task of A's call `send_message_to_workspace` with a message mentioning B. Confirm the message appears in that topic attributed to A with its origin shown, that B wakes and replies there, and that an agent outside the Workspace is refused.

**Acceptance Scenarios**:

1. **Given** A is a member of a Workspace, **When** A's scheduled run sends a message there mentioning B, **Then** the message appears in that topic attributed to A, shows that it came from the scheduled task, and B is activated.
2. **Given** A sends a message with no mention, **When** it is posted, **Then** members and agents read it and no agent is activated.
3. **Given** A is not a member of a Workspace, **When** A sends to it or reads it, **Then** A gets the same not-found result as for a Workspace that does not exist.
4. **Given** the named topic does not exist, **When** A sends to it, **Then** the topic is created and the result says so.
5. **Given** A is in a direct chat with a person, **When** A calls `list_workspaces`, **Then** it gets only the Workspaces it belongs to, with their topics and members.
6. **Given** a dream or a subagent piece, **When** the agent lists its tools, **Then** none of the three Workspace tools appears.

---

### User Story 7 - See what an agent says while it works (Priority: P2)

A person watches an agent work in any WebUI or desktop chat, Workspace or not. Text the agent writes between tool rounds appears as a message in the conversation, not hidden inside the activity trail, so a turn reads as *trail, message, trail, final message*. In a Workspace, this is also how `mention_agent` appears.

**Why this priority**: A long turn reads as silence today unless someone opens the trail. In a Workspace it matters more, because a mention made part-way through a turn wakes another agent, and members need to see what caused that. It ships with this feature, but it works without any Workspace.

**Independent Test**: With an offline test agent in a direct chat, run a turn that writes text between two tool rounds. Confirm the text shows as its own message between two separate trails, live and after a reload, and that a conversation saved before this change renders the same way.

**Acceptance Scenarios**:

1. **Given** an agent writes text between two tool rounds, **When** the turn runs, **Then** the text appears as a message as soon as that round's text is saved, the finished trail folds above it, and a new trail opens below it for the next work.
2. **Given** a finished turn, **When** it is reloaded, **Then** it renders the same as it did live, with one trail per stretch of work.
3. **Given** a conversation saved before this change, **When** it is opened, **Then** its mid-turn text renders as messages, with no data change.
4. **Given** a turn with several messages, **When** it ends, **Then** stats, total time and reactions belong to the final message only.
5. **Given** an agent's text comes before any tool call, **When** the turn runs, **Then** the message appears with no empty trail above it.
6. **Given** a turn is stopped or fails, **When** it ends, **Then** its mid-turn messages stay in the conversation.
7. **Given** an agent in a Workspace calls `mention_agent`, including from inside a script, **When** the message posts, **Then** it appears as a mid-turn message at that point in the turn, not as a row in the trail, and the script's own detail still lists the call.
8. **Given** an agent's turn in a Workspace, **When** another member's message lands part-way through it, **Then** the agent's name appears again where its work continues.
9. **Given** a conversation on Discord or Telegram, **When** an agent writes mid-turn text, **Then** what is posted there does not change.

---

### Edge Cases

- **Mention storms**: a person's single message may mention many agents. Each is activated once, however many times it is named in that message.
- **Self-mention**: an agent calling `mention_agent` on itself is refused; a self-mention in reply text does nothing.
- **An agent named in passing**: a bare display name wakes no one, and neither does `@<agent-id>` inside a code span, a code block or a quoted passage, so agents and people can talk *about* an agent without waking it. Outside those, the exact `@<agent-id>` does wake it, even if the author meant it as an aside. The parser does not and cannot infer intent, so the only defence is telling the author (FR-009a): a mention is a hand-off, and an agent referred to without acting is named without the `@`.
- **Mid-turn mentions, then a stop**: a mid-turn message is complete once it is saved, so a mention in it wakes that agent straight away, while the author's turn continues. If the author's turn is later stopped or fails, that message and the wake stand. A message still being written when the stop lands was never posted, so it wakes and notifies no one.
- **Filler text**: an agent's "let me check that" now shows at full size and is read by other agents in a Workspace. No filtering is applied: what members see and what agents read stay the same.
- **An answer with no mention back**: if B answers A without mentioning it, A does not wake, and its task may rest there until someone mentions it. A has still read the answer, so nothing is lost. People can see this in the timeline, and agents are told about the convention (FR-009).
- **Busy agent**: a mention of an agent already working in this topic queues behind its running turn, in order, as any request does today. Messages it reads in the meantime are not lost.
- **Stopping**: any member can stop an agent's running turn in the Workspace, and can stop all of them at once, which also ends any exchange between agents in progress.
- **Editing and deleting**: messages cannot be edited in this version, so what an agent read is what was posted. Deleting a Workspace is limited to its owner or an administrator.
- **Restart**: an activated turn resumes after a restart as durable turns do today. A message posted by a person is never lost to a restart, and is not read twice.
- **Agent in a failed state**: it stays a listed member, shown as unavailable; messages posted meanwhile still count as read on its recovery.
- **Attachments**: a file posted to a Workspace is available to every member, agents included. A person is told so before they send it.
- **Questions**: when an agent asks a question card in a Workspace, any human member may answer it, and the card records who did. A question cannot be answered by another agent.
- **Secrets**: an agent cannot ask for sensitive input in a Workspace, because the request would play out in front of the whole room. It can tell the person to use a direct chat.
- **Long conversations**: an agent's own context limits still apply. A long-idle agent is not billed for messages it read until it is activated, and then only for what its normal context handling keeps.
- **Agents that never stop mentioning each other**: nothing ends this automatically (FR-016). It runs and spends provider budget until a member stops it.
- **Same agent, several topics**: an agent can work in different topics at once. A mention queues only behind its running turn in the same topic.
- **Sender wakes without its earlier context**: an agent that sends from a scheduled task or a direct chat and is later mentioned wakes in the Workspace topic, with that topic's context, not the conversation it sent from. The originating conversation is not woken.
- **Renamed Workspace**: a saved task that sends to the old name gets the not-found result, and the agent can call `list_workspaces` to find the current name.

## Requirements *(mandatory)*

### Functional Requirements

**Workspace and membership**

- **FR-001**: The system MUST offer Workspaces: named, persistent shared spaces with any number of member people and member agents, each holding one or more topics. An installation can have many. A Workspace name MUST be unique in the installation, ignoring case, because agents address Workspaces by name.
- **FR-002**: Each topic MUST keep one timeline in one order, shown identically to all members, with each message attributed to the person or agent who posted it. Membership belongs to the Workspace, so every member is in every topic. A new Workspace starts with one topic, and members can add more.
- **FR-003**: Members with management rights MUST be able to rename a Workspace, add and remove people and agents, and delete it. The creator is the first owner.
- **FR-004**: Only members and those holding an all-Workspaces administrative permission MUST be able to read or post in a Workspace. A non-member MUST NOT be able to tell that a given Workspace exists. Removing a member MUST take effect immediately, including on open connections.
- **FR-040**: Adding a person or an agent MUST start from one search, by name, username or agent id, and MUST NOT require scrolling a full list, so it stays usable with hundreds of people and agents. Results MUST be capped, say when more match, list who can be added first, and show who is already a member. An agent the searcher may view but not edit MUST appear as not addable, with the reason. An agent the searcher may not view MUST NOT appear.
- **FR-005**: Adding an agent to a Workspace MUST be restricted to people who may edit that agent (its owner, or the all-agents edit permission). Adding is the act of sharing it: from then on every member may mention it, including members who could not otherwise view it. The person adding it MUST be told that members can use its tools and integrations, spend its provider budget, and see what it recalls from memory.
- **FR-006**: Workspace access rules MUST behave identically on every surface, including the desktop app.

**Reading and activation**

- **FR-007**: A member agent MUST silently read every message posted in its Workspace, other agents' mid-turn messages included: it is recorded as seen by that agent, with no reply and no model call.
- **FR-008**: An agent MUST be activated only by a mention: `@<agent-id>` in a completed message from a person or from another agent, whether that message is mid-turn or final. Agents' per-agent random chance to answer unmentioned messages MUST NOT apply in a Workspace, and neither do any other implicit triggers (a reply to its message, or a skill command without a mention).
- **FR-009**: An activated agent MUST have access to what it silently read in that topic, and only that topic, subject to its normal context limits, and MUST be told who is in the Workspace with the mention to use for each, who mentioned it, and that an agent wakes only when mentioned, so it must mention whoever it needs to act next.
- **FR-009a**: An agent MUST be told, wherever it can wake another agent, that a mention is a hand-off and not a way of naming an agent: the woken agent cannot tell a hand-off from a passing remark, so a mention MUST be written only to have an agent act, and an agent referred to without acting MUST be named without the `@`. This applies to the `## Workspace` section and to the `mention_agent` and `send_message_to_workspace` tool descriptions. The rule MUST be stated as its own paragraph in the `## Workspace` section, and MUST name the cost of an unnecessary mention: the agent answers only if it mentions back, so an unneeded mention leaves a thread with no reply.
- **FR-010**: A message mentioning several agents MUST activate each exactly once, concurrently, independent of the others.
- **FR-011**: A mention MUST be resolved when the message is complete, against current members. Text that resolves to no member, or that sits inside a code span, a code block or a quoted passage, MUST stay plain text and activate no one.
- **FR-012**: Mentions MUST work the same whoever writes them and however the message was built: a person's message, an agent's reply text and `mention_agent` activate agents and highlight people by the same rule.
- **FR-013**: The format for skills and commands MUST be `/<command> @<agent-id> [input]`: a message that starts with `/name`, `/skill name` or `/skills`, followed directly by one or more mentions, MUST run it on each mentioned agent, resolved against that agent's own skills, from a person or an agent. Any other message MUST NOT run a skill: `/name` with no mention right after it is plain text, and `@a /name` is a plain mention of a.
- **FR-041**: Each agent MUST keep its own record of each topic, as on Discord: its own turns (thoughts, tool calls and results), plus the mid-turn and final messages shown in that topic, in order. Another agent's thoughts, tool calls and results MUST NOT enter its record. An agent's record MAY be brought up to date when it is activated or starts a dream, as long as it never misses or repeats a message it was entitled to read (FR-007).
- **FR-042**: People MUST be able to run commands in the same format, acting on that agent and topic only and leaving a visible note instead of a message: `/abort @a` (any member), `/checkpoint @a` (any member), and `/lobotomy @a` (an owner, or someone who may edit that agent). `/abort` with no mention MUST act as stopping everything (FR-023). `/btw` and `/clear-files` MUST be refused with an explanation and post nothing. Commands MUST NOT enter other agents' records, and MUST have no effect when written by an agent.

**`mention_agent`**

- **FR-014**: The system MUST provide a `mention_agent` tool: a structured way to post a message into the current Workspace, attributed to the calling agent, that contains the `@<agent-id>` mention of one or more member agents. It MUST add no activation behaviour of its own: the resulting message is treated and displayed exactly as if the same text had been written, as a mid-turn message (FR-034), and any answer reaches the caller only by mentioning it. The call MUST NOT end the calling turn and MUST return without waiting for the mentioned agent.
- **FR-015**: `mention_agent` MUST be available only during an agent's own turn in a Workspace. It MUST NOT appear or work in any other conversation, nor in dreams or in subagent pieces launched from a Workspace turn. `@<agent-id>` in text MUST activate no one outside a Workspace.
- **FR-016**: The system MUST NOT limit how many times agents may activate each other in this version. Stopping turns (FR-023) is the control over an exchange that does not end on its own.
- **FR-017**: A refused `mention_agent` call (non-member, self, unavailable agent) MUST return as data the calling script can handle, MUST NOT fail the turn, and MUST post nothing. A mention written in text has no caller to refuse: an unresolved one stays plain text (FR-011), and a mention of an unavailable member shows in the timeline.

**Reaching a Workspace from elsewhere**

- **FR-028**: The system MUST provide `send_message_to_workspace(workspace_name, workspace_topic, message)`, which posts a message to that topic, attributed to the calling agent, from any conversation the agent is running in, scheduled tasks included. Mentions in the message MUST follow the same rule as any other message (FR-012). The call MUST NOT end the calling turn and MUST return without waiting for anyone to respond.
- **FR-029**: The system MUST provide `list_workspaces`, returning the Workspaces the calling agent belongs to with their topics and members, and `read_workspace_messages(workspace_name, workspace_topic)`, returning that topic's recent messages, bounded and pageable.
- **FR-030**: These three tools MUST act only on Workspaces the calling agent is a member of. A Workspace the agent does not belong to MUST be answered the same as one that does not exist, as data the calling script can handle, without failing the turn.
- **FR-031**: Sending to a topic that does not exist MUST create it, and the result MUST say so. Reading a topic that does not exist MUST return a result saying so and create nothing.
- **FR-032**: A message sent from outside a Workspace MUST show its origin to members, such as the scheduled task or kind of channel it came from.
- **FR-033**: These three tools MUST NOT be available in dreams or in subagent pieces.

**Removal of consult and delegate**

- **FR-018**: `consult_agent` and `delegate_agent` MUST be removed from every conversation. A call to either name MUST fail with guidance pointing to `mention_agent` inside a Workspace and `send_message_to_workspace` outside one.
- **FR-019**: Existing inter-agent conversations and delegation job records MUST remain readable, and any compatibility code kept for them MUST be listed on the pre-1.0 removal tracker (#82).
- **FR-020**: `background_subagents`, `parallel_subagents`, `list_background_jobs` and `cancel_background_job` MUST keep working, for an agent's own subagents.

**Mid-turn messages**

- **FR-034**: In every WebUI and desktop conversation, text an agent writes between tool rounds MUST appear as a message in the conversation, at the same reading size as the final message, and MUST NOT appear inside the activity trail. Thoughts and tool calls stay in the trail.
- **FR-035**: A turn MUST show one trail per stretch of work, each with its own time and counts. The stretch running now stays open, and finished ones fold. A stretch with no work MUST show no trail.
- **FR-036**: Stats, total time and reactions MUST appear on the final message only, and outside a Workspace a completed turn MUST notify as it does today, once. In a Workspace, FR-022 applies instead.
- **FR-037**: A turn MUST render the same live and after a reload. Conversations saved before this change MUST pick up this rendering with no data migration.
- **FR-038**: Where another member's message lands within a turn in a Workspace, the agent's name MUST appear again where its work continues.
- **FR-039**: Mid-turn messages MUST NOT change what is posted to Discord or Telegram.

**Experience**

- **FR-021**: People MUST be able to reach their Workspaces from the WebUI and the desktop app, with mention suggestions while composing and visible status for each agent that is working. Each Workspace the person belongs to MUST have its own entry in the app rail, grouped apart from Home and from agents by separators and visually distinct from an agent, showing how many of that person's notifications from it are unread and whether an agent is working there. Selecting one MUST show that Workspace's topics. The rail MUST have one create control, which asks whether to create a Workspace or an agent. Someone allowed to create only one of them MUST go straight to it, and someone allowed neither MUST NOT see the control.
- **FR-022**: Every message posted to a Workspace, by a person or an agent, mid-turn or final, MUST notify every person who is a member, except its author, linked to the message. Being mentioned adds no second notification. Notes from commands MUST NOT notify. A person with that topic open MUST NOT accumulate unread notifications for it, and opening a topic MUST mark its notifications read. Who is notified MUST follow current membership (FR-004).
- **FR-023**: Members MUST be able to stop one agent's turn, or every running turn in the Workspace. Stopping everything MUST also drop mentions still queued in that Workspace, so that an exchange between agents actually ends.
- **FR-024**: Model usage from Workspace turns MUST be attributed to the agent that ran them and reported as its own activity in usage reports. Silent reading MUST record no model usage.
- **FR-025**: Deleting a Workspace MUST remove its timeline, stop its running turns, and remove its files, but MUST NOT affect its member agents' memory, or usage already recorded.
- **FR-026**: Agents MUST be able to reflect on Workspace conversations in their dream cycle, as they do for other conversations with people.
- **FR-027**: Agent behaviour in a Workspace MUST be verifiable end to end with the offline test provider, covering silent read, activation by mention, `mention_agent`, stopping all turns, and the tools of FR-028 and FR-029.

### Key Entities

- **Workspace**: A uniquely named shared space. Has an owner, members and one or more topics.
- **Topic**: A named conversation inside a Workspace, with its own timeline. Separates subjects, and each member agent's context, within one membership.
- **Workspace Member**: A person or an agent belonging to a Workspace. A person may hold management rights. An agent may be available or unavailable.
- **Workspace Message**: One entry on a topic's timeline: its author (person or agent), content, attachments, resolved mentions, time and, when sent from outside the Workspace, its origin.
- **Mention**: A resolved reference from a message to a member. Activates an agent, or highlights the message for a person.
- **Agent Record**: One agent's own history of one topic: its own turns, plus the messages shown there.
- **Note**: A line on the timeline saying that someone ran a command. Not a message, and not read by agents.
- **Read Position**: What a given agent has seen of a topic's timeline, so nothing is missed or read twice, including across restarts.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of messages with no mention produce no agent reply and no recorded model usage, whatever the number of member agents.
- **SC-002**: 100% of messages mentioning an available agent activate that agent exactly once, and activate no one else.
- **SC-003**: A member sees a new message, or a reply as it streams, within 1 second of it being produced, under normal conditions, with up to 20 members and 10 agents in a Workspace.
- **SC-004**: An agent activated after reading 50 unmentioned messages can correctly answer a question about any of them that fits within its context limits.
- **SC-005**: A member can end an exchange between agents with one action: within 5 seconds, no agent in that Workspace is running or queued, and none wakes until it is next mentioned.
- **SC-006**: A person who is not a member gets no Workspace content through any surface, in all tested permission combinations, and a removed member loses access within 1 second.
- **SC-007**: After upgrade, 100% of past inter-agent conversations and delegation records stay readable, and neither removed tool is offered to any agent in any conversation.
- **SC-008**: A person can create a Workspace, add an agent and get a reply from it in under 2 minutes, on their first try.
- **SC-009**: 100% of runs of a scheduled task that sends a mention to a Workspace activate the mentioned member, with the message's origin visible to members.
- **SC-010**: An agent that is not a member gets no Workspace content and posts nothing, in all tested cases, and cannot tell a Workspace exists.
- **SC-012**: In an installation with 500 people and 500 agents, a person can find and add a member they know by name in under 15 seconds.
- **SC-011**: 100% of an agent's mid-turn text is visible in the conversation without opening anything, live and after a reload, including in conversations saved before this feature.

## Assumptions

- **Surfaces**: Workspaces are a WebUI and desktop-app feature backed by the HTTP API. A Workspace is not bridged to Discord or Telegram in this version.
- **Consequence of the removal, as requested**: an agent can reach another agent only through a Workspace they both belong to. From a direct chat, Discord, Telegram or a scheduled task, it does so with `send_message_to_workspace`. Two agents sharing no Workspace cannot coordinate.
- **Answers stay in the Workspace**: `send_message_to_workspace` has no return path to the conversation it was sent from. An orchestrating agent follows up by being mentioned in the topic, or by calling `read_workspace_messages` on a later run.
- **Orchestration uses existing scheduling**: no new task type is needed. An agent creates a task with its existing scheduling tools, and the run calls `send_message_to_workspace`.
- **Reach from outside**: a person talking to a member agent on Discord or Telegram, who is not a Workspace member, can ask it to read or post in that Workspace. Membership of the agent is the boundary, and origin display (FR-032) makes posts visible. This is accepted for this version; tightening it is left to a separate spec.
- **Strict activation**: "activate only by mentioning" overrides the per-agent random initiative setting used on Discord and Telegram. Random replies from several agents in one room would add noise and cost.
- **Mentions use ids**: an agent wakes on its exact `@<agent-id>`, and a person is mentioned by `@<username>`. The composer inserts these from suggestions while showing display names.
- **No automatic reply routing**: the system does not track which message answers which, so B's answer reaches A's attention only if B mentions A. This is deliberate, for simplicity, and accepts that a task can rest until someone mentions the next agent.
- **Agents are private to their own memory**: membership does not merge agents' memory or secrets. But what an agent says in a Workspace is seen by every member, and memories it recalls can surface in its replies.
- **Naming**: "Workspace" is already the term for the data folder on disk, and project tools use `workspace_shell`. The user-visible name is kept as requested; docs must not let the two be confused, and internal names must not use the bare word.
- **Generative UI and client tools**: tools declared by one person's browser are out of scope in a Workspace, since they bind to one connection in a shared room. Agents reply with text and attachments.
- **`@everyone`-style group mentions**: out of scope for this version.
- **No chain limit for now, as requested**: a limit can be added later if runaway exchanges turn out to be a problem in use.
- **Mid-turn text appears whole**: it appears when its round's text is saved, not word by word. Streaming it is out of scope.
- **Scope of mid-turn messages**: they change every WebUI and desktop chat, not only Workspaces, as requested. They reach other agents only in a Workspace.
- **Notification volume**: every message notifies, by design, so a busy Workspace, or agents talking at length, can fill the inbox. Muting is not in this version.
- **A turn with no message**: a turn that fails or is stopped before writing anything creates no notification, since no message was posted.
- **Design**: the agreed layout is [mockup.html](mockup.html), in place of a FigCraft drawing, which was not reachable (constitution v2.2.0).
