# Feature Specification: Waking an agent in any conversation

**Feature Branch**: `067-wake-session`

**Created**: 2026-11-26

**Status**: Draft

## Clarifications

### Session 2026-11-26

- Q: Should this be a target on the scheduled task, or a general tool an agent can call? → A: A
  general tool. The user's call: be flexible now, harden later (FR-001, FR-020).
- Q: Which conversations may be woken? → A: Only conversation channels — WebUI, project chat,
  Discord, Telegram. Conversations the system's own machinery owns (scheduled runs, subagent
  workers, consultations, dream cycles) keep their own owners and are not wakeable this way
  (FR-006).
- Q: May an agent wake a conversation that belongs to another agent? → A: Yes, under the same rule
  that governs consulting: only agents the person on whose behalf it works may use (FR-007).
- Q: What are the tools called? → A: `list_sessions` and `wake_session`. "Wake" says it starts a
  turn there, unlike posting a message into it (FR-015).
- Q: May a woken turn be started in the caller's own conversation? → A: Yes, under the name
  "current", so an agent can hand work to the conversation it is already in (FR-003).

**Input**: User description: "currently what tools available for agent to send/message to other
channel?" … "prefer jadi generalize tool aja daripada pake target session gitu, biar lebih
flexible, it's better to be flexible now, kalo perlu hardening bisa nyusul nanti" — the feature is a
general way for an agent to start work in any conversation it can reach, so that scheduled work
happens in the person's own conversation instead of a throwaway one.

## Context

An agent's work happens in a conversation. Today there are four ways for one conversation to hand
work to another, and each of them creates a *new* conversation for the work:

- A **scheduled run** happens in its own conversation, one per firing, and its result reaches the
  person as a notification. The person can read it; they cannot continue it there, because nobody
  was in that conversation.
- A **subagent worker** happens in its own conversation, owned by the machinery that hired it.
- A **consultation** happens in the consulted agent's own conversation, owned by the same machinery.
- A **detached script** delivers its outcome into the conversation that started it, without
  starting a turn.

None of them can start a turn in a conversation that already exists. The tools that do exist for
reaching another conversation — `webui_send_message`, `discord_send_message`,
`telegram_send_message` — post a message into it; they do not start work there, and they are
limited to one platform each.

The consequence shows up in the everyday case. A person asks for a recurring job — a morning
summary, a weekly review — and the work runs in a conversation nobody is in. The result arrives as
a notification, disconnected from the conversation where the person actually lives. The person
wanted the work to happen *there*.

This feature gives the agent a general way to start a turn in any conversation it can reach: find
the conversation, then wake it with an instruction. The work happens in that conversation, with its
history in view, and the answer lands there too. A scheduled run that wants this simply does it.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - An agent starts work in another conversation (Priority: P1)

An agent is in one conversation and needs work done in another — a person's thread, a project chat,
a channel on a chat platform. It names the conversation, gives the instruction, and the work runs
there. The agent of that conversation does the work, in that conversation, with that conversation's
history in view, and its answer appears there.

**Why this priority**: This is the feature. Everything else is a guardrail or a convenience around
it.

**Independent Test**: With two offline test agents, have agent A wake one of B's conversations
with an instruction. B's turn runs in that conversation, under B, and B's reply is in that
conversation's history afterwards.

**Acceptance Scenarios**:

1. **Given** agent A and a conversation A may wake, **When** A wakes it with an instruction,
   **Then** the call returns at once without waiting, and a turn starts in that conversation.
2. **Given** a woken conversation, **When** its agent runs, **Then** it runs as that agent (its
   identity, memory, tools, integrations, provider and configured prompt), in that conversation,
   with that conversation's earlier turns in view.
3. **Given** the woken turn, **When** it finishes, **Then** its reply is saved into the target
   conversation and shown there, exactly as a turn started by a person would be.
4. **Given** a target conversation that already has a turn running, **When** it is woken,
   **Then** the woken turn waits behind the running one, as a person's message would.
5. **Given** agent A waking a conversation of agent B, **When** B's turn reads its request,
   **Then** it is told which agent woke it, that the request is not from a person, and that nobody
   is waiting on the answer in that conversation.

---

### User Story 2 - An agent finds the conversation to work in (Priority: P1)

An agent that wants to work in a conversation has to be able to find it. It lists the
conversations it may reach, sees each one's reference, title, channel, owning agent and whether
something is running there, and picks one — by narrowing the list to a channel, or by searching
titles. The conversation it is in right now is always among them.

**Why this priority**: Without discovery the wake tool is unusable: references cannot be invented.
This story is what makes Story 1 usable.

**Independent Test**: With an agent holding several conversations across two channels, list them
and check the references, titles and channels. Narrow by channel and by a word in a title and check
the result narrows with it. The caller's own conversation appears with the name "current".

**Acceptance Scenarios**:

1. **Given** an agent with several conversations, **When** it lists them, **Then** each entry has a
   reference it can pass to a wake, the owning agent, the channel, the topic where the channel has
   one, a title, and whether a turn is running there.
2. **Given** a channel filter, **When** the agent lists with it, **Then** only conversations on that
   channel are returned.
3. **Given** a text to search for, **When** the agent lists with it, **Then** only conversations
   whose title contains it are returned.
4. **Given** any listing, **When** the agent reads it, **Then** the conversation the call itself is
   in is present, under the name "current".
5. **Given** an agent that may not reach another agent, **When** it lists, **Then** that agent's
   conversations are absent, exactly as if the agent did not exist.

---

### User Story 3 - Only conversations the caller may reach can be woken (Priority: P1)

Waking a conversation spends another agent's budget, writes into that conversation's history, and
shows up in front of the person who owns it. So an agent may only wake a conversation it is
entitled to reach: a conversation of its own agent, or one of an agent the person on whose behalf it
works may use. A conversation belonging to a different person is out of reach even when the
address is known.

**Why this priority**: This is the boundary that keeps the feature from being a way to reach
anything. It has to land with the feature, not after it.

**Independent Test**: With three people, one of whom may use agent B and one of whom may not, have
each ask their agent to wake a conversation of B. Only the entitled one succeeds; the other is
refused without revealing whether B exists.

**Acceptance Scenarios**:

1. **Given** a conversation of the calling agent, **When** the agent wakes it, **Then** it starts.
2. **Given** a conversation of an agent the person may use, **When** the calling agent wakes it,
   **Then** it starts.
3. **Given** a conversation of an agent the person may not use, **When** the calling agent wakes
   it, **Then** it is refused exactly as for an agent that does not exist, and nothing reaches it.
4. **Given** a WebUI or project conversation of another person, **When** the calling agent wakes it,
   **Then** it is refused, whatever the address.
5. **Given** a turn with no person in it, such as a scheduled run, **When** the agent wakes a
   conversation, **Then** the calling agent's owner is the person whose access decides.

---

### User Story 4 - A scheduled run puts its work in the person's own conversation (Priority: P2)

A person sets up a recurring job and wants the work where they can see and continue it — in their
own thread, not in a throwaway one. The job's instruction says which conversation to work in, and
the work happens there: the agent picks the conversation, wakes it, and the answer lands in it
beside the person's own messages.

**Why this priority**: This is the everyday case the feature exists for, and it needs no change to
how jobs are scheduled — the agent does it with the same tools. It depends on Story 1.

**Independent Test**: Set up a recurring job whose instruction names the person's thread. When it
fires, the work runs in that thread and its answer is in that thread's history. The job's own
conversation is not where the answer is.

**Acceptance Scenarios**:

1. **Given** a recurring job whose instruction names a conversation, **When** the job fires,
   **Then** the work runs in the named conversation and the answer is in that conversation.
2. **Given** the same job, **When** the person opens that conversation afterwards, **Then** the
   answer is there in order, with the person's own messages around it.
3. **Given** a recurring job whose instruction names no conversation, **When** it fires, **Then** it
   behaves exactly as it does today: in its own conversation, with its result as a notification.

---

### User Story 5 - The caller waits for the answer (Priority: P2)

Sometimes the caller needs the answer in the same step, not just in the other conversation. It
asks to wait, and the woken turn's reply comes back to it, so it can carry on with everything it
knows.

**Why this priority**: A convenience on top of the wake, and the reason the feature can replace
one-shot question-asking. It depends on Story 1.

**Independent Test**: Wake a slow conversation with waiting on, and check the call returns with the
woken turn's answer. Then wake one without waiting, and check the call returns at once with no
answer, and that the answer still arrives in the target conversation.

**Acceptance Scenarios**:

1. **Given** a wake with waiting on, **When** the woken turn finishes, **Then** the call returns
   with its answer, in the same call.
2. **Given** a wake with waiting on, **When** the caller is interrupted while waiting, **Then** the
   waiting ends, the woken turn carries on, and its answer still reaches the target conversation.
3. **Given** a wake without waiting, **When** the call returns, **Then** it returns at once with
   the target's reference and no answer, and the woken turn is not stopped by the call returning.

---

### User Story 6 - The person sees what woke their conversation (Priority: P3)

The person whose conversation was woken can see, in that conversation, that another agent started
work there, what it was asked, and what came of it — the same as any other turn in that
conversation.

**Why this priority**: The person's own conversation is affected by the feature, so they should be
able to see it. It follows from Story 1 rather than adding to it.

**Independent Test**: Wake a person's thread and read its trail: the request that woke it and the
reply it produced are both there, and the request reads as coming from an agent rather than from
the person.

**Acceptance Scenarios**:

1. **Given** a conversation that was woken, **When** the person reads it, **Then** the request that
   woke it appears in the conversation's trail, attributed to the waking agent rather than to the
   person.
2. **Given** the same conversation, **When** the person reads it, **Then** the woken turn's reply
   follows the request, as any reply would.

### Edge Cases

- **Waking the caller's own conversation.** Allowed, under the name "current". It starts a second
  turn in a conversation that may already be busy; the woken turn waits behind it.
- **The caller's own conversation in a scheduled run.** It is the task's own conversation, which
  the machinery owns: it is listed under "current" and refused. A scheduled run reaches the
  person's own conversation by naming it, not by naming "current".
- **Waking a conversation that is itself machinery.** Refused: a scheduled run's, a subagent
  worker's, a consultation's and a dream cycle's conversations belong to the machinery that owns
  them, and a foreign turn in them breaks that machinery's own bookkeeping.
- **Waking a conversation of an agent the caller may not consult.** Refused exactly as for an
  agent that does not exist, so the refusal leaks nothing.
- **A conversation that was deleted between listing and waking.** Refused as unknown, never as a
  failure of the call.
- **An address that does not parse, or names a channel that has no topic.** Refused as bad data,
  never as a failure of the call.
- **A target that is not a conversation at all.** A reference that names no conversation is refused
  with the reason, so the agent can correct the address and try again.
- **Chains and loops.** A woken turn counts one hop deeper than the turn that woke it, and wakes
  beyond the hop limit are refused, so A → B → A cannot run away. Waking a conversation of an
  agent already in the chain is refused at any hop count.
- **The woken turn asks the person something.** The person-facing question and sensitive-input
  tools are not offered in a woken turn, because nobody is there to answer.
- **The woken turn hires subagents.** Allowed. It acts with its own tools, and its workers belong
  to the target conversation.
- **The woken turn fails.** The failure is recorded in the target conversation, as any turn's
  failure is. The caller learns of it only if it was waiting.
- **Two wakes of the same conversation at once.** Both are accepted and both run, in order, behind
  whatever was already running.

## Requirements *(mandatory)*

### Functional Requirements

**Finding a conversation**

- **FR-001**: An agent MUST be able to list the conversations it may reach. The tool MUST be named
  `list_sessions`.
- **FR-002**: A listing MUST be able to be narrowed to one channel and to a text to look for in
  titles. A channel filter and a text filter MAY be given together.
- **FR-003**: Every entry MUST carry a reference the agent can hand to a wake unchanged, the
  owning agent, the channel, the topic where the channel has topics, a title, and whether a turn is
  running there.
- **FR-004**: The reference of a conversation MUST be stable: the same conversation always has the
  same reference, and a reference taken from a listing is accepted by a wake.
- **FR-005**: The conversation the call is made in MUST always be listed, under the name "current".
  It MUST be wakeable under that name when it is a conversation of a kind that may be woken; in a
  conversation the system's own machinery owns, it is listed and refused.

**Waking a conversation**

- **FR-006**: An agent MUST be able to start a turn in a conversation it names, with an
  instruction. The tool MUST be named `wake_session`.
- **FR-007**: A wake MUST name one target: the name "current", or a conversation reference. It
  MUST carry an instruction, and it MAY ask to wait for the answer.
- **FR-008**: The woken turn MUST run as the agent that owns the target conversation: its own
  identity, memory, tools, integrations, provider and configured prompt, in that conversation,
  with that conversation's earlier turns in view.
- **FR-009**: The woken turn's request MUST be marked as unattended: it states which agent woke
  it, that it is not a person's message, and that nobody is waiting on the answer in that
  conversation.
- **FR-010**: The woken turn's reply MUST be saved into the target conversation and shown there,
  and the request that woke it MUST appear in that conversation's trail attributed to the waking
  agent.
- **FR-011**: A woken turn MUST wait behind a turn already running in the target conversation, as
  a person's message does.
- **FR-012**: A wake MUST return at once with the target's reference, and MUST NOT stop the woken
  turn when it returns.
- **FR-013**: A wake that asks to wait MUST return the woken turn's answer in the same call, and
  MUST end the wait without stopping the woken turn if the caller is interrupted.
- **FR-014**: Only conversation channels MAY be woken: WebUI conversations, project chats, Discord
  channels and Telegram chats. A reference to any other kind of conversation MUST be refused.
- **FR-015**: Conversations owned by the system's own machinery — scheduled runs, subagent
  workers, consultations and dream cycles — MUST NOT be woken, and a reference to one MUST be
  refused.

**Who may be woken**

- **FR-016**: An agent MUST be able to wake a conversation of its own agent, and a conversation of
  an agent the person on whose behalf it is working may use. "May use" means the person may chat
  with that agent, under the same view rule that decides whether it appears to them in the WebUI.
- **FR-017**: An agent that may not use another agent MUST NOT be able to wake one of its
  conversations, and MUST be answered about it exactly as about an agent that does not exist.
- **FR-018**: A WebUI or project conversation MUST belong to the person on whose behalf the calling
  agent is working. A conversation of another person MUST be refused.
- **FR-019**: In a conversation with no person in it, such as a scheduled run, the calling agent's
  owner MUST be the person whose access decides.
- **FR-020**: Access MUST be checked when the conversation is listed and again when it is woken.

**Limits**

- **FR-021**: A woken turn MUST count as one hop deeper than the turn that woke it. Wakes beyond
  the hop limit MUST be refused. The limit MUST be a backend setting, independent of the
  subagent and consultation limits.
- **FR-022**: At any hop count, waking a conversation of an agent already in the chain MUST be
  refused.
- **FR-023**: The person-facing question and sensitive-input tools MUST NOT be offered in a woken
  turn.
- **FR-024**: No limit on how many conversations an agent may wake, beyond the hop limit.

**Tools and guidance**

- **FR-025**: The two tools MUST form their own tool category, and its name MUST be reserved, so
  an integration or tool server of the same name is given a distinct name.
- **FR-026**: Every refusal — unknown or foreign reference, not-allowed agent, another person's
  conversation, a conversation of the system's own machinery, a bad instruction, a broken hop
  limit — MUST be returned as data the calling agent can read, never as a failure of the call.
- **FR-027**: The calling agent's operating doctrine MUST explain the difference between waking a
  conversation (starting work there, at that agent's cost, into that conversation's history) and
  posting a message into it (saying something there, at the caller's cost), and MUST tell the
  agent to name the target conversation explicitly rather than guess.

**Jobs and other work**

- **FR-028**: A scheduled run MUST be able to wake the person's own conversation, so that the work
  a job does happens where the person sees it, with no change to how jobs are scheduled.
- **FR-029**: A scheduled run that names no conversation MUST behave exactly as it does today.

**People's view**

- **FR-030**: A person MUST be able to see, in a conversation of their own, that another agent
  woke it and what it produced. Nothing else about the wake MUST be visible to them, and they
  MUST NOT be able to stop a woken turn from the conversation.

### Key Entities

- **Conversation**: the place a conversation of work happens. It has an owning agent, a channel
  (WebUI, project chat, Discord channel, Telegram chat, or one of the system's own kinds), a topic
  where the channel has one, a title, and a state that says whether a turn is running in it.
- **Conversation reference**: the stable name of a conversation, used to address it. It names the
  channel and, where the channel has topics, the topic. "Current" names the conversation of the
  turn that uses it.
- **Wake**: one instruction that starts a turn in another conversation. It names the waking agent,
  the target conversation, the instruction, whether the caller waits, and the answer when it does.
  It lives only for the call; the result lives in the target conversation.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: An agent can start work in any conversation it may reach, and the result appears in
  that conversation, in 100% of test runs.
- **SC-002**: In 100% of attempts, a person who may not use another agent cannot cause any turn to
  reach one of that agent's conversations, and cannot tell the agent exists.
- **SC-003**: In 100% of attempts, a person cannot cause a turn to reach a WebUI or project
  conversation belonging to somebody else.
- **SC-004**: 0 conversations owned by the system's own machinery are ever woken.
- **SC-005**: 0 wake chains exceed the hop limit, and 0 chains wake an agent already in the chain.
- **SC-006**: A scheduled run can place its work in the person's own conversation, with no change
  to how jobs are scheduled, in 100% of test runs.
- **SC-007**: A caller that waits receives the woken turn's answer in the same call, in 100% of
  test runs; a caller that does not wait receives no answer and its target is unaffected.
- **SC-008**: Every refusal is readable data, and 0 refusals fail the call they came from.

## Assumptions

- **Naming.** `list_sessions` and `wake_session`. "Wake" says it starts a turn there; the tools
  that post a message into a conversation keep their existing names and their existing meaning.
- The conversation vocabulary the system already uses for naming conversations is reused, so a
  reference from a listing is exactly what a wake takes, and the same reference appears wherever
  a conversation is named.
- A wake is transient: it is a call, not a record with a handle, states and a panel. The answer
  lives in the target conversation, which is where the person wants it, and there is no second copy
  of it to keep in step. If a handle, a stop and a panel are wanted later, that is a follow-up
  feature and not part of this one.
- One target per call. Waking several conversations in one call is out of scope; the agent may call
  the tool again.
- Waking is offered where the agent can act for a person. A turn with no person in it may wake a
  conversation of an agent its owner may use, and of its own owner's conversations, but no
  conversation of another person.
- The hop limit reuses the machinery that already bounds background work, so the two limits are
  one number rather than two.
- What the woken agent writes to its own memory during the woken turn is its own business, as in
  any other conversation it has.
- The woken turn's model usage is recorded as the target agent's, under its own activity, linked
  to the target conversation.
- A woken turn may hire subagents, and its workers belong to the target conversation.
- Nobody may stop, redirect or answer a woken turn from the conversation it woke.
- Changing how a scheduled run finds the conversation to work in is not needed: the agent in the
  run finds it with the same two tools a person-facing agent has.
