# Feature Specification: User-Invokable Skills

**Feature Branch**: `021-user-invokable-skills`

**Created**: 2026-10-18

**Status**: Draft

**Input**: User description: "can we make skill invokable by user from webui, telegram, and discord?"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Run a skill by name from chat (Priority: P1)

A person talking to an agent on the WebUI, Telegram or Discord types a skill's name as a command (for example `/summarize-meeting`) followed by optional free text. Today the agent decides on its own whether to use a skill; with this feature the person can say "use this skill now" explicitly. The agent runs that turn following the named skill's instructions, treating any text after the command as the task input, and replies in the same conversation as usual.

**Why this priority**: This is the whole point of the request: giving people a deterministic way to start a skill instead of hoping the agent picks it.

**Independent Test**: With an agent that has a skill installed, send `/<skill> some input` on one channel and confirm the reply visibly follows that skill's instructions and uses "some input" as its subject.

**Acceptance Scenarios**:

1. **Given** an agent with an available skill "release-notes", **When** a person sends `/release-notes v1.2 changes` in the WebUI, **Then** the agent answers that turn by following the skill with "v1.2 changes" as the input.
2. **Given** the same agent in a Telegram chat or a Discord channel, **When** the person sends the same command, **Then** the behaviour and reply are equivalent to the WebUI.
3. **Given** a command with no extra text, **When** it is sent, **Then** the skill still runs, using the recent conversation as its context.

---

### User Story 2 - Discover which skills can be invoked (Priority: P2)

A person who does not remember the exact skill names can see the list of skills available to the agent they are talking to, with a short description of each, from within the channel they are using.

**Why this priority**: Invocation is only useful if skill names can be found; but it is secondary to invocation itself.

**Independent Test**: In each channel, request the skill list and confirm it matches the agent's skills.

**Acceptance Scenarios**:

1. **Given** an agent with three skills, **When** a person asks for the skill list (typing `/` in the WebUI composer, a `/skills` command, or the platform's command menu where one exists), **Then** they see each skill's name and description.
2. **Given** an agent with no skills, **When** the list is requested, **Then** the person is told no skills are available.

---

### User Story 3 - Clear feedback for bad invocations (Priority: P3)

A person mistypes a skill name or calls a skill that is not available to this agent. They get a short message saying so, with the closest matching names, and no agent turn is spent.

**Why this priority**: Polish that prevents confusion and wasted model calls.

**Independent Test**: Send `/no-such-skill` and confirm a "not found" reply with suggestions and no model usage recorded.

**Acceptance Scenarios**:

1. **Given** no skill named "relase-notes", **When** a person sends `/relase-notes`, **Then** they are told it was not found and shown close matches such as "release-notes".

---

### Edge Cases

- A skill name collides with a built-in command (such as `/abort`): built-in commands always win, and the skill remains reachable another way (e.g. `/skill <name>`).
- Skill exists both globally and for the specific agent: the agent-specific one is used, matching how the agent already resolves skills.
- On Discord each skill is a native slash command of that agent's bot, so the person picks which bot to call and no mention is needed; two bots with a same-named skill never clash. A skill whose name Discord cannot accept as a command name, or that would exceed Discord's command limit, is still reachable by typed text (mention or DM).
- On Telegram, no mention is needed in groups: a message starting with `/<skill-name>` where the name exactly matches a skill of that agent runs it. Unknown names, `/skills` and not-found replies still need the bot to be addressed (mention, or `/cmd@bot`), so unrelated slash text does not make every bot in the group reply. If several agents in one group have a skill of the same name, each of them runs it.
- A skill is removed or changed between listing and invoking: the current state wins; a removed skill gives the not-found reply.
- Very long input after the command is handled like any other long message.
- A command sent while the agent is already mid-turn is queued like any other message.
- Messages that merely begin with `/` but are not a skill (e.g. a file path) are treated as ordinary chat when no skill matches (FR-007).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: People MUST be able to invoke any skill available to an agent by name from the WebUI, Telegram and Discord, optionally with free-text input.
- **FR-002**: An invoked skill MUST be applied to that turn deterministically, without depending on the agent choosing to look it up.
- **FR-003**: Text following the command MUST be passed to the agent as the input for the skill; with none, the preceding conversation serves as context.
- **FR-004**: People MUST be able to list the skills available to the agent they are talking to, with descriptions, on every supported channel.
- **FR-005**: The set of invokable skills MUST match the skills the agent itself can use (agent-specific overriding global), and update when skills are installed or removed, without restarting the agent.
- **FR-006**: Built-in commands MUST take precedence over skill names; a documented alternate form MUST exist to reach a skill whose name is shadowed.
- **FR-007**: A leading-slash message that does not match a built-in command or a skill MUST either get a not-found reply with suggestions (when it looks like a skill name) or be handled as ordinary chat (when it clearly is not a command); no model turn is spent on a not-found reply.
- **FR-008**: In multi-party chats, invoking a skill MUST NOT require mentioning the agent. On Discord, every skill the platform can accept MUST be offered as a slash command of that agent's bot and the offered set MUST follow skill changes. On Telegram, a message beginning with the exact name of one of the agent's skills MUST invoke it unmentioned; every other command-shaped message (unknown name, skill list, not-found reply) MUST only act when addressed to the agent.
- **FR-009**: The invocation MUST be recorded in the conversation history so that the agent and the person can see which skill was run in that turn.
- **FR-010**: Invoking a skill MUST NOT grant any permission the person did not already have in that conversation; access rules for who may talk to the agent still apply.
- **FR-011**: The user-facing documentation MUST describe how to invoke and list skills on each channel.

### Key Entities

- **Skill**: A named, described set of instructions an agent can follow; has a name used as the command, a description, and optional keywords. Either global or specific to one agent.
- **Skill invocation**: A person's explicit request to run one skill in a conversation, consisting of the skill name and optional free-text input, tied to the conversation turn it started.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A person can start a named skill in one message, with no more than one extra token (the command) compared with a normal message.
- **SC-002**: In testing, 100% of invocations of an available skill result in a turn following that skill, on all three channels.
- **SC-003**: A person can find the available skills from within their channel in a single action.
- **SC-004**: 100% of invocations of unknown skill names produce a not-found reply without a model call.
- **SC-005**: Newly installed or removed skills are reflected in invocation and listing within one minute, with no restart.

## Assumptions

- The invocation form is a slash command named after the skill (`/<skill-name> [input]`), consistent with the existing `/abort` command.
- Only skills already available to the agent in that conversation are invokable; no new skill authoring or installing flow is part of this feature.
- The agent still sees the skill the way it does today; the change is that the person, not the agent, chooses it for the turn.
- Plain-text commands must work everywhere. Discord additionally gets native slash commands per skill; Telegram's native command menu is out of scope (it is per bot and not per conversation).
- Scheduled tasks, dreams, background jobs and inter-agent turns are out of scope; only people-initiated chat turns can invoke skills.
