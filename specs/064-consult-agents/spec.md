# Feature Specification: Consulting other agents (and removing Workspaces)

**Feature Branch**: `064-consult-agents`

**Created**: 2026-11-20

**Status**: Draft

## Clarifications

### Session 2026-11-20

- Q: Which agents count as "available to the user" for consulting? → A: Any agent the person may
  chat with (view rights). No ownership or allow-list is required (FR-010).
- Q: What happens to each member agent's own record of a Workspace topic on upgrade? → A: It is
  deleted with the Workspace (FR-033).

**Input**: User description: "let's remove workspace features entirely instead for inter-agent collaboration, we'll provide a `consult_agent` (let me know if you have better name) tool what it does allow agent to talk to prompt agent just like how it can prompt/hire a subagent, in fact it should behave just like how subagents features like, but instead connected to a "temporary" agent, i connect to a real separate agent available to the user"

## Context

Spec 053 made Workspaces the only way agents reach each other. Agents read a shared timeline
silently and wake only when mentioned with `@<agent-id>`, and `consult_agent` and
`delegate_agent` were retired in its favour. In practice an agent that needs another agent's help
in the middle of a task has to post into a Workspace it belongs to and hope it gets mentioned
back. The answer never comes back to the step that asked for it, and the hand-off convention,
loops between agents and membership management cost more than they give.

Spec 059 then gave agents a much better shape for delegated work: subagent workers. A worker gets
a handle at once, can be waited on, stopped and read, can ask its employer a question and get a
reply, survives the turn and restarts, and delivers its outcome exactly once.

This feature removes Workspaces and gives inter-agent work the subagent shape. The difference is
**who does the work**: a subagent is a temporary copy of the calling agent, while a
**consultation** goes to a real, separate agent with its own identity, memory, tools, provider and
owner.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - An agent consults another agent and gets the answer back (Priority: P1)

Agent A is helping a person and needs something agent B is better placed for, such as B's
knowledge, B's integrations, or B's judgment. A consults B with a request and gets a handle at
once. B works on it in a conversation of its own, as itself, and its answer comes back to A,
either when A waits or as a delivery into A's conversation, exactly as a subagent's result would.

**Why this priority**: This is the feature. It replaces Workspaces as the only way agents
collaborate, so without it agents lose that ability entirely.

**Independent Test**: With two offline test agents A and B, have a person ask A something that
makes A consult B and wait. B's turn runs in its own consultation conversation, under B's
identity and tools, and A receives B's answer in the same step and uses it in its reply to the
person.

**Acceptance Scenarios**:

1. **Given** agent A and an agent B that A may consult, **When** A consults B, **Then** the call
   returns a handle at once without waiting for B.
2. **Given** a consultation of B, **When** B runs, **Then** it runs as B (B's identity, memory,
   tools, provider and configured prompt), in a conversation of B's own that is separate from
   every other conversation of B.
3. **Given** B's consultation turn, **When** B reads its request, **Then** the request says it was
   sent by agent A as a consultation, names the person (if any) on whose behalf A is working, and
   tells B that its answer goes back to A, not to a person.
4. **Given** A waits on the handle, **When** B finishes, **Then** A receives B's answer in the
   same step.
5. **Given** A does not wait, **When** B finishes, **Then** the answer is delivered into A's
   conversation exactly once: at the next step boundary if A is mid-turn, or by starting a new
   turn of A if it is idle.

---

### User Story 2 - Managing consultations like workers (Priority: P1)

A consults several agents at once, waits for any or all of them, lists and reads them, and stops
one that is taking too long while the others carry on. Everything a main agent can do with its
workers it can do with its consultations.

**Why this priority**: The user asked for consultations to behave exactly like subagents. The
handle model is what makes waiting, stopping and the later stories possible.

**Independent Test**: A consults B and C, waits for "any", stops C while it is running, and
waits for "all". C reports "stopped" with the reason "stopped by the consulting agent", and B
reports its own outcome.

**Acceptance Scenarios**:

1. **Given** running consultations, **When** A waits for "any" or "all", **Then** the wait returns
   when that mode is met or the wait's own time runs out, reports every given consultation's
   state with its question or answer where there is one, and stops nothing.
2. **Given** a running consultation, **When** A stops it, **Then** B's turn on it stops, its state
   becomes "stopped" with the reason "stopped by the consulting agent", and other consultations
   are unaffected.
3. **Given** consultations in A's conversation, **When** A lists or reads them, **Then** it sees
   each one's agent, request, state, stop reason, question and answer.
4. **Given** a consultation with a time limit, **When** B has not answered in time, **Then** its
   state becomes "timed out" and B's turn on it stops.

---

### User Story 3 - The consulted agent asks back, and the conversation continues (Priority: P2)

B needs something from A to do the job, such as a missing detail or a choice between options. B
asks A a question, which ends B's step in "asking". A sees the question when it waits, reads it
or receives it as a delivery, and replies. B continues in the same conversation. When B has
answered, A may send a follow-up on the same consultation, and B continues with everything it
said before in view, as in a conversation between two people.

**Why this priority**: The user described this as "talking to" another agent. A one-shot request
covers much of the value, but questions and follow-ups are what make it a conversation. It
depends on Story 2's handles.

**Independent Test**: Script B to ask on its first step and answer on its second. A waits and
sees "asking" with the question, replies, then waits and receives an answer that shows B saw its
earlier steps and the reply. A then sends a follow-up on the answered consultation and receives a
second answer that refers to the first.

**Acceptance Scenarios**:

1. **Given** a consultation, **When** B asks A a question, **Then** its state becomes "asking", the
   question is available to A, and B does nothing further on it until replied to or stopped.
2. **Given** a consultation in "asking", **When** A replies, **Then** B continues in the same
   conversation with its earlier steps and the reply in view, and the state returns to
   "running".
3. **Given** an answered consultation, **When** A sends a follow-up, **Then** B continues the same
   conversation, the state returns to "running", and the new answer is delivered like the first.
4. **Given** a running, stopped, failed or timed-out consultation, **When** A replies or follows
   up, **Then** it is refused with a reason A can act on.

---

### User Story 4 - Only agents the person can use may be consulted (Priority: P1)

Consulting B uses B's tools, integrations, memory and provider budget, and shows B what A's
conversation is about. So A may consult only agents that are available to the person A is
working for, and A can find out which ones those are before choosing.

**Why this priority**: Without this, anyone who can talk to A can reach every agent in the
installation. That is a security boundary, so it has to land with the feature.

**Independent Test**: Use three fixture users, where one can use A only, one can use A and B, and
one is an administrator. Have each ask A to consult B. Only the users entitled to B succeed. For
the others, A's consult call is refused with the same answer as for an agent that does not
exist, and the list of consultable agents leaves B out.

**Acceptance Scenarios**:

1. **Given** a person who may use B, **When** A, working for that person, consults B, **Then** the
   consultation starts.
2. **Given** a person who may not use B, **When** A consults B, **Then** the call is refused
   exactly as for an unknown agent, and nothing reaches B.
3. **Given** A, **When** it lists the agents it may consult in this conversation, **Then** the list
   holds exactly the agents available under FR-010, with each one's name, id and description,
   and never A itself.
4. **Given** a turn with no person in it (a scheduled run, a dream), **When** A consults, **Then**
   the rule of FR-011 decides.

---

### User Story 5 - A person peeks at the consultations (Priority: P2)

A person chatting with A sees, in the chat's status strip, how many consultations are still open,
with an amber dot while one is asking. The panel lists them grouped by the step that started
them. Each row names the consulted agent, the request, the state and the time, and expanding a
row shows B's steps drawn with the chat's own activity trail, then B's answer. The view is
read-only.

**Why this priority**: Consultations run unseen and can start turns of A on their own, so a
person needs to see what they are waiting on. It mirrors the Subagents panel.

**Independent Test**: With two offline agents, have A consult B twice, one of them asking. The
strip counts 2 with an amber dot, the panel shows both rows with the asking one's question, and
expanding a row shows B's steps and, once answered, its answer. Another person's conversation and
an unknown id answer not found.

**Acceptance Scenarios**:

1. **Given** a conversation with open consultations, **Then** the strip shows a Consultations item
   counting running and asking ones, with an amber dot while one is asking.
2. **Given** the panel is open, **When** a consultation changes state, **Then** its row updates
   without a reload.
3. **Given** an expanded consultation, **Then** B's steps are drawn with the chat's activity trail
   and its answer follows once there is one.
4. **Given** any person, **Then** the panel offers no control that stops, replies to or follows up
   a consultation.

---

### User Story 6 - Consultations survive interruptions (Priority: P1)

The person aborts A's turn, A's turn ends, or A or B is restarted or updated while a consultation
is open. The consultation carries on, and its outcome reaches A once.

**Why this priority**: A consultation can be long-running work on another agent. Losing it to an
interruption would mean starting over at that agent's cost.

**Independent Test**: Have A consult a slow B, abort A's turn, and let B finish. A receives
exactly one delivery with B's answer. Repeat with a restart of B mid-work: B's turn resumes, and
the answer is still delivered once.

**Acceptance Scenarios**:

1. **Given** an open consultation, **When** the person aborts A's turn or the turn ends, **Then** the
   consultation keeps running.
2. **Given** B is updated or restarted mid-consultation, **Then** B's turn resumes on the next
   instance, and an asking consultation keeps its question.
3. **Given** A is updated or restarted, **Then** its open consultations keep going, and their
   outcomes are delivered to A's conversation once.
4. **Given** an outcome A already saw through a wait or a read, **Then** it is not delivered again,
   and a consultation A stopped is never delivered.

---

### User Story 7 - The consulted agent's people can see what it was asked (Priority: P3)

B's owner opens B and sees its consultations among its conversations, marked as consultations
with the agent that asked. They can read them but not write in them.

**Why this priority**: B acted, spent its budget and may have written its memory because of a
consultation, so the people responsible for B need a record of it. It is not needed for the
core flow.

**Independent Test**: After A consults B, a person who may edit B finds the consultation in B's
conversation list, labelled with A, and can read it. The composer is not offered, and a person
who may only chat with B does not see it.

**Acceptance Scenarios**:

1. **Given** a finished consultation, **When** someone who may edit B opens B, **Then** the consultation is listed, labelled as a consultation from A, and readable.
2. **Given** that conversation, **Then** no one can post in it from the WebUI.

---

### User Story 8 - Workspaces are removed (Priority: P1)

An operator upgrades. Workspaces are gone from the WebUI, the desktop app, the API, the agents'
tools and their prompts. Agents that still try the Workspace tools are told to consult instead,
and nothing else an agent or person owns is lost.

**Why this priority**: The user asked for removal, and it is a breaking change. It has to fail
helpfully and land together with Story 1, so agents always have one way to collaborate.

**Independent Test**: Upgrade a data set that has Workspaces with topics, members and
notifications. Afterwards the rail has no Workspace entries and no create choice for one, the
Workspace routes answer not found, a script calling any of the four Workspace tools gets
guidance naming the consultation tools, and agents' memory, other conversations and usage are
unchanged.

**Acceptance Scenarios**:

1. **Given** the upgraded system, **When** a person opens the WebUI or the desktop app, **Then** no
   Workspace entry, page, create choice, member search or Workspace notification is shown.
2. **Given** an agent's script calls `mention_agent`, `send_message_to_workspace`,
   `list_workspaces` or `read_workspace_messages`, **Then** the failure can be caught and points
   to the consultation tools.
3. **Given** a script calls `delegate_agent`, **Then** the guidance names the consultation tools.
   **Given** it calls `consult_agent`, **Then** that name is the live consult tool again (FR-001).
4. **Given** existing Workspace data, **When** the upgrade runs, **Then** what happens to it follows
   FR-033.
5. **Given** a Discord guild or Telegram group, **Then** silent reading there works exactly as
   before.

### Edge Cases

- **Consulting oneself** is refused. A agent that wants its own help hires a subagent.
- **Unknown, unavailable or failing agent.** An unknown agent, or one the person may not use, is
  refused as "not found". An agent that exists but is failing or starting is refused with its
  availability reason, so A can try another or tell the person. If B becomes unavailable after the
  consultation started, the request waits for B like any queued request, within the time limit.
- **B deleted** while a consultation is open: it stops with the reason "consulted agent deleted"
  and A is told through the usual delivery.
- **A's conversation deleted**: its open consultations stop with the reason "conversation deleted",
  B's turns on them stop, and B's consultation conversations are deleted, so the person's content
  does not stay behind in B.
- **A deleted**: its consultations stop with the reason "agent deleted", and B's consultation
  conversations are deleted.
- **Access lost mid-consultation.** If the person loses the right to use B while a consultation is
  open, the next reply or follow-up is refused, and the open turn finishes. Nothing new reaches B.
- **Chains and loops.** By default a consulted agent cannot consult (FR-015), so A → B → A cannot
  happen. If the depth is raised, consulting an agent already in the chain is still refused.
- **Many consultations of one agent.** B may be consulted by several agents, or several times by
  one, at the same time. Each is its own conversation, and they run independently as B's
  separate conversations do today.
- **B asks the person.** The person-facing question and sensitive-input tools are not offered in a
  consultation. Only asking the consulting agent is.
- **B hires subagents.** Allowed. B acts with its own tools, and its workers belong to the
  consultation conversation.
- **A consults from a subagent worker or a detached script.** A worker cannot consult under the
  default setting (it inherits the main agent's limits, FR-015). A detached script may consult,
  and the consultation belongs to the script's conversation like a worker would, so it outlives
  the script.
- **Unknown or foreign handle, empty request, invalid time limit**: refused as data, never as a
  failure of the script.
- **Stopping a settled consultation** is a no-op that reports its final state.

## Requirements *(mandatory)*

### Functional Requirements

**Consulting**

- **FR-001**: Agents MUST be able to consult another agent: send it a request and get a handle at
  once, without waiting for it. The tool that starts a consultation MUST be named `consult_agent`,
  which stops being a retired name.
- **FR-002**: A consultation MUST run as the consulted agent: its own identity, memory, tools,
  integrations, provider and configured system prompt. It MUST run in a conversation of the
  consulted agent's own, created for that consultation and separate from all its other
  conversations.
- **FR-003**: A consultation's request MUST tell the consulted agent that it comes from the named
  consulting agent, on behalf of which person (or that no person is involved), and that its answer
  goes back to that agent and not to a person. It MUST ask for a compact, direct answer.
- **FR-004**: A single call MUST be able to start consultations of one or more agents, each with
  its own request and an optional time limit (1 to 3,600 seconds, default 600). It MUST return one
  handle per consultation, in the order given.
- **FR-005**: Waiting, stopping, listing and reading consultations MUST behave as they do for
  subagent workers (spec 059, FR-010, FR-012, FR-013). A stop records "stopped by the consulting
  agent".
- **FR-006**: A consultation MUST be in exactly one of: running, asking, answered, failed, timed
  out, stopped. A stopped consultation MUST carry its reason: stopped by the consulting agent,
  conversation deleted, consulting agent deleted, or consulted agent deleted.
- **FR-007**: The consulted agent MUST be able to ask the consulting agent one question, ending its
  step in "asking". A reply MUST continue the same conversation. That asking tool MUST be offered
  only in a consultation conversation.
- **FR-008**: The consulting agent MUST be able to send a follow-up on an answered consultation. It
  continues the same conversation, and the new answer is delivered like the first. A reply or
  follow-up to a consultation in any other state MUST be refused with a reason.
- **FR-009**: Every refusal (unknown or foreign handle, unknown or not-allowed agent, self,
  invalid input, wrong state) MUST be returned as data the script can read, never as a failure of
  the script.

**Who may be consulted**

- **FR-010**: An agent MUST be able to consult only agents the person on whose behalf it is
  working may use. That is the person in the conversation, for a WebUI or project chat. Discord
  and Telegram identities are not linked to people, so those conversations fall under FR-011.
  An agent not in that set
  MUST be answered exactly as an agent that does not exist. "May use" means the person may chat
  with that agent, under the same view rule that decides whether it appears to them in the WebUI.
  Owning or being able to edit the agent is not required, and there is no separate allow-list.
- **FR-011**: In a conversation with no person (a scheduled run, a Discord or Telegram message),
  the consulting agent's owner MUST be the person whose access decides. An agent with no owner
  MUST NOT consult, and is told why.
  Dreams MUST NOT consult.
- **FR-012**: Agents MUST be able to list the agents they may consult in the current conversation,
  with each one's name, id, short description and availability.
- **FR-013**: Access MUST be checked when a consultation starts and again on every reply or
  follow-up.
- **FR-014**: Consulting MUST be visible to the consulted agent's people: a consultation
  conversation MUST be readable by those who may edit that agent (its owner, or the all-agents
  edit permission), and MUST be labelled with the consulting agent. No person MUST be able to post in it.

**Limits**

- **FR-015**: By default a consulted agent MUST NOT be offered the consultation tools, apart from
  asking back, so consultations do not chain. The allowed consultation depth MUST be a backend
  setting, independent of the subagent and background-work limits. At any depth, consulting
  oneself or an agent already in the chain MUST be refused.
- **FR-016**: A subagent worker MUST NOT be offered the consultation tools under the default
  setting, and a consulted agent MAY hire subagents under its own subagent setting.
- **FR-017**: The person-facing question and sensitive-input tools MUST NOT be offered in a
  consultation conversation.

**Lifecycle and delivery**

- **FR-018**: A consultation MUST keep running when the turn or detached script that started it
  ends, is aborted, is cancelled or times out. It lives as long as the consulting conversation.
- **FR-019**: A consultation MUST survive updates and restarts of either agent: B's turn resumes
  from its last step, and an asking consultation keeps its question.
- **FR-020**: Asking MUST NOT consume the time limit, and an asking consultation MUST wait until
  replied to, stopped or deleted.
- **FR-021**: Outcomes (answered, failed, timed out, asking) MUST be delivered into the consulting
  conversation exactly once, by the same mechanism and rules as subagent outcomes (spec 059,
  FR-024 to FR-026). One delivery MAY carry outcomes of both workers and consultations.
- **FR-022**: Deleting the consulting conversation or the consulting agent MUST stop its open
  consultations and delete their conversations on the consulted side. Deleting the consulted agent
  MUST stop its open consultations with that reason.
- **FR-023**: A consultation's model usage MUST be recorded as the consulted agent's, under its own
  activity named Consultations, linked to the consultation conversation.
- **FR-024**: A consulted agent MAY reflect on its consultation conversations in its dream cycle,
  as it does for other conversations.

**Tools and guidance**

- **FR-025**: The consultation tools MUST form their own tool category, and its name MUST be
  reserved, so an integration or tool server of the same name is given a distinct name.
- **FR-026**: The consulting agent's operating doctrine MUST explain the difference between a
  subagent (a temporary copy of itself, for work it could do) and a consultation (another agent,
  for that agent's knowledge, tools or judgment, at that agent's cost). It MUST tell the agent to
  give the consulted agent the goal, the context and the shape of the answer it wants, and never
  to pass on secrets.
- **FR-027**: A consulted agent's prompt MUST tell it that it is being consulted by the named agent,
  that it speaks for itself, and that it should answer for that agent rather than a person.

**People's view**

- **FR-028**: People MUST be able to list the consultations a conversation of their own started,
  read one with its request and answer, and read its steps, and nothing else. Any other id answers
  not found.
- **FR-029**: The chat's status strip MUST show a Consultations item, separate from Subagents and
  Background work. It MUST count the open consultations, with an attention dot while one is asking.
  Its panel MUST name each consulted agent, show an asking consultation's question, and draw an
  expanded consultation's steps with the existing activity trail. It MUST NOT offer stopping,
  replying or following up.
- **FR-030**: Every consultation snapshot sent to the conversation MUST say whether the consulting
  agent already has its newest outcome.

**Removing Workspaces**

- **FR-031**: Workspaces MUST be removed from the WebUI, the desktop app, the HTTP API, the agents'
  prompts and their tools: rail entries, pages, the create choice, member search, the topic
  stream, reactions and files in Workspaces, Workspace notifications, and the Workspace
  permissions.
- **FR-032**: `mention_agent`, `send_message_to_workspace`, `list_workspaces`,
  `read_workspace_messages` and `delegate_agent` MUST answer with guidance naming the consultation
  tools, and no guidance MUST name a retired tool.
- **FR-033**: On upgrade, Workspace data (timelines, topics, membership, read positions,
  Workspace notifications and permissions) MUST be removed. Agents' memory, usage already recorded,
  and their conversations outside Workspaces MUST be untouched. Each member agent's own record of
  a Workspace topic (its turns, what it read, and that conversation's files, scratch files and
  pending turns) MUST be deleted with the Workspace. Nothing about Workspaces is kept readable.
  What an agent already wrote to its memory from a Workspace stays, because memory is untouched.
- **FR-034**: Silent reading on Discord and Telegram, and the way mid-turn messages show in every
  WebUI chat, MUST be unchanged.
- **FR-035**: Any compatibility code kept for the removal (old names, old data, old notifications)
  MUST be listed on the pre-1.0 removal tracker (#82).
- **FR-036**: The behaviour MUST be verifiable end to end with the offline test provider: consult,
  wait, stop, asking back, follow-up, delivery after an aborted turn, and refusal of a
  not-allowed agent.

### Key Entities

- **Consultation**: one request from a consulting agent to a consulted agent. It holds both agents,
  the person it was made for, the consulting conversation and turn, the consulted agent's
  consultation conversation, its position in the call that started it, its state, stop reason,
  pending question, latest answer, whether its newest outcome was seen or delivered, its time
  limit, and its start and end times.
- **Consultation conversation**: a conversation of the consulted agent, created for one
  consultation, holding that agent's turns on it and every request, reply and follow-up. It is
  read-only to people and is deleted with the consulting conversation.
- **Consulting conversation**: the conversation the consultation was started from. It bounds the
  consultation's lifetime and receives its deliveries.
- **Consultation group**: the consultations started by one call, which the panel groups by.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: An agent can get another agent's answer back into the same step that needed it, with
  no shared space to set up first, in 100% of test runs.
- **SC-002**: In 100% of attempts, a person who may not use agent B cannot cause any request to
  reach B, and cannot tell B exists.
- **SC-003**: Stopping one consultation leaves every other consultation in its group to finish
  normally in 100% of test runs.
- **SC-004**: When the consulting turn is aborted or either agent restarts, 0 consultation outcomes
  are lost, and each unseen outcome reaches the consulting agent exactly once.
- **SC-005**: Under the default setting, 0 consulted agents can consult further.
- **SC-006**: After the upgrade, 0 Workspace surfaces remain reachable in the WebUI, the desktop app
  or the API, and every call to a retired name gets guidance naming a tool that exists.
- **SC-007**: After a consulting conversation is deleted, 0 of its consultations remain open and 0
  of their conversations remain on the consulted side.
- **SC-008**: A person can tell from the strip alone that a consultation is waiting on a question,
  and can read any consultation's steps without leaving the chat.

## Assumptions

- **Naming.** `consult_agent` is kept, since it says what it is (asking another agent, who answers
  as itself) and it was the name agents knew before spec 053. The rest of the family mirrors the
  subagent tools, and the exact names are settled in the plan.
- Consultations reuse the subagent machinery (handles, states, delivery, resumption, the peek
  panel) as far as possible. The differences are who runs the turn, the access check, follow-ups,
  and the consulted side's visibility.
- A consultation costs the consulted agent's provider budget, which is why access follows the
  person and is visible to the consulted agent's people.
- What the consulted agent writes to its own memory during a consultation is its own business, as
  in any other conversation it has.
- No limit on how many consultations an agent may open, beyond the time limit and the depth
  setting.
- Per-consultation overrides (a different model, a tool subset) are out of scope.
- People cannot start, stop, reply to or follow up a consultation. Both views are read-only.
- The UI parts (the Consultations strip item and panel, consultation labels in the consulted
  agent's conversation list, and the Workspace removal in the rail) are drawn before they are built,
  following the project's design-first rule.
