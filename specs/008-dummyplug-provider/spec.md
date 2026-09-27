# Feature Specification: Dummyplug Test Provider

**Feature Branch**: `008-dummyplug-provider`

**Created**: 2026-09-27

**Status**: Draft

**Input**: User description: "let's make a dummy provider named "dummyplug", it mainly used for us to be able to testing without requiring a live provider, it behaviour should be simple when user sends "tools" it will list all tools, when player sends a tool name it will reply with sample tool request json, then user can use the json to trigger the tool, otherwise the provider will just reply with random lorem ipsum text"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Chat with an agent without a live provider (Priority: P1)

A developer or tester creates an agent that uses the "dummyplug" provider and talks to it through any channel (WebUI, HTTP, Discord, Telegram). Without any API key, network access, or running model server, the agent answers every ordinary message with random lorem ipsum text, so the full request/response path (channels, sessions, history, UI rendering) can be exercised.

**Why this priority**: This is the minimum that removes the dependency on a live provider. Everything else builds on the agent being able to answer at all.

**Independent Test**: Configure a dummyplug provider with no credentials, create an agent that uses it, send "hello" from the WebUI, and confirm a lorem ipsum reply appears and is saved to the session history.

**Acceptance Scenarios**:

1. **Given** an agent that uses the dummyplug provider, **When** the user sends any message that is not a command, tool name, or tool request, **Then** the agent replies with non-empty lorem ipsum text.
2. **Given** the same agent, **When** the user sends the same ordinary message twice, **Then** the two replies may differ (text is randomly generated).
3. **Given** a machine with no network access and no provider credentials, **When** the dummyplug agent is started and messaged, **Then** it replies normally with no errors about missing keys or unreachable services.

---

### User Story 2 - Discover the agent's tools (Priority: P2)

A tester sends "tools" to a dummyplug agent and gets back a list of every tool that agent can use, each with its name and a short description. This includes the always-on tools, any tools enabled by the agent's configuration, and tools from the agent's connected external tool servers.

**Why this priority**: The tester must know which tools exist, and their exact names, before they can trigger any of them.

**Independent Test**: Send "tools" to a dummyplug agent and compare the reply against the agent's configured toolset. Every tool should appear once.

**Acceptance Scenarios**:

1. **Given** a dummyplug agent, **When** the user sends "tools", **Then** the reply lists the name and description of every tool available to that agent.
2. **Given** two dummyplug agents with different tool configurations, **When** "tools" is sent to each, **Then** each reply shows only that agent's tools.
3. **Given** a dummyplug agent, **When** the user sends " Tools " (different case or extra whitespace), **Then** it is treated the same as "tools".

---

### User Story 3 - Get a sample request for a specific tool (Priority: P2)

A tester sends the exact name of a tool, for example `read_core`, and receives a ready-to-use sample tool request in JSON. The sample names the tool and includes example arguments that match the tool's expected input, with every required field filled in using placeholder values of the right type.

**Why this priority**: This lets the tester trigger a tool without reading its input definition by hand. It turns tool testing into copy, edit, and send.

**Independent Test**: Send a known tool name. Confirm the reply contains a JSON block that names the tool and has placeholder values for every required argument.

**Acceptance Scenarios**:

1. **Given** a dummyplug agent that has a tool named X, **When** the user sends "X", **Then** the reply contains a JSON tool request that names X and includes example values for all required arguments.
2. **Given** a tool that takes no arguments, **When** the user sends its name, **Then** the sample JSON has an empty argument set.
3. **Given** a tool with optional arguments, **When** the user sends its name, **Then** the sample shows the optional arguments too, clearly separable from the required ones (for example, through a short note beside the JSON).

---

### User Story 4 - Trigger a tool by sending the JSON request (Priority: P1)

A tester pastes a tool request JSON (usually the sample from Story 3, possibly edited) as a message. The dummyplug agent turns it into a real tool call: the agent runs the tool exactly as it would for a live model, the tool call and its result are recorded in the conversation, and the agent's final reply shows the tool's result.

**Why this priority**: Exercising tools end to end (execution, hooks, history, UI display of tool calls and results) without a live model is the main testing value of this feature.

**Independent Test**: Send a valid tool request JSON for a harmless tool, such as reading CORE. Confirm the tool actually runs, the tool call and result appear in the session history and the UI, and the final reply contains the tool's output.

**Acceptance Scenarios**:

1. **Given** a dummyplug agent, **When** the user sends a valid tool request JSON for one of its tools, **Then** that tool is executed with the supplied arguments and its result is returned in the agent's reply.
2. **Given** a tool that changes state (for example, writing a memory), **When** it is triggered through a tool request JSON, **Then** the change actually happens, just as it would when a live model called the tool.
3. **Given** a tool that fails because of bad arguments, **When** it is triggered, **Then** the tool's error is shown in the reply and the agent stays usable.
4. **Given** a tool request JSON that names a tool the agent doesn't have, **When** it is sent, **Then** the agent replies with a clear message saying the tool is unknown and suggesting "tools". Nothing is executed.
5. **Given** a message that looks like a tool request but is malformed JSON, **When** it is sent, **Then** the agent replies with a clear message that the request couldn't be parsed. Nothing is executed.

---

### Edge Cases

- A tool name that is also an ordinary word, for example "fetch" as the whole message: an exact match to a tool name counts as a tool name. Messages that only contain a tool name among other words get lorem ipsum.
- An agent that has no tools at all: "tools" replies with a clear "no tools available" message.
- Messages sent with attachments (images, audio, files): the text part is interpreted using the same rules, and attachments are ignored.
- A tool that runs a long time or triggers another agent (consult/delegate): the dummyplug agent waits for the result like it would with a live model. If the other agent also uses dummyplug, the other agent replies following its own rules.
- Non-interactive runs (scheduled tasks, dream cycles, subtasks): these prompts are not commands or tool requests, so the provider returns lorem ipsum and the run completes without error.
- Streaming versus non-streaming delivery: the replies are the same either way.
- A very large toolset (for example, many external tool servers): the "tools" listing still includes every tool.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST offer a provider type named "dummyplug" that can be configured and assigned to agents the same way as any other provider.
- **FR-002**: The dummyplug provider MUST work with no credentials, no network access, and no external service.
- **FR-003**: When the user's latest message, trimmed and ignoring case, is exactly "tools", the provider MUST reply with the name and description of every tool available to that agent.
- **FR-004**: When the user's latest message, trimmed, exactly matches the name of one of the agent's tools, the provider MUST reply with a sample tool request in JSON that names the tool and includes a type-appropriate placeholder value for each input argument.
- **FR-005**: The sample tool request format MUST be the same format that FR-006 accepts, so the sample can be sent back unchanged to trigger the tool.
- **FR-006**: When the user's latest message is a valid tool request JSON that names one of the agent's tools, the provider MUST cause the agent to execute that tool with the given arguments through the normal tool-execution path. Hooks, history recording, and UI display of tool calls and results MUST behave the same as with a live provider.
- **FR-007**: After a tool triggered under FR-006 returns, the provider MUST produce a final reply that contains the tool's result or error, which ends the turn. It MUST NOT start further tool calls on its own.
- **FR-008**: When a tool request names an unknown tool, or is not valid JSON while clearly being meant as a tool request, the provider MUST reply with a clear, human-readable explanation and MUST NOT execute anything.
- **FR-009**: For every other input, the provider MUST reply with randomly generated, non-empty lorem ipsum text.
- **FR-010**: The dummyplug provider MUST work in every context where agents use a provider: interactive chat on all channels, scheduled tasks, dream cycles, subtasks, and agent-to-agent consult/delegate.
- **FR-011**: The dummyplug provider MUST be clearly identifiable as a testing provider in provider selection and configuration screens, so it isn't mistaken for a real model.
- **FR-012**: Choosing the dummyplug provider MUST NOT change the behavior of agents that use other providers.

### Key Entities

- **Dummyplug provider**: A provider configuration of type "dummyplug". It needs no credentials and exposes a single model identity.
- **Tool listing**: The reply to "tools". It contains one entry per available tool, with the tool's name and description.
- **Tool request (JSON)**: A message that names a tool and supplies its arguments. It is produced as a sample by the provider and accepted back as input to trigger the tool.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A developer can go from a fresh install with no provider credentials to a working, replying agent in under 2 minutes.
- **SC-002**: 100% of the tools available to an agent appear in its "tools" listing.
- **SC-003**: For 100% of tools, the sample request returned for the tool's name is accepted when sent back unchanged. The tool is invoked, even if it then fails because of placeholder argument values.
- **SC-004**: Every dummyplug reply arrives in under 1 second, excluding time spent running a triggered tool.
- **SC-005**: A tester can trigger and observe any single tool end to end (list, sample, send, see result) in 4 messages or fewer.

## Assumptions

- The provider is available in all builds and is opt-in: it is only used when someone explicitly configures it and assigns it to an agent.
- Only the user's most recent message decides the reply. Earlier conversation history is ignored except to detect that a tool result has just come back (FR-007).
- The exact wording and length of the lorem ipsum text doesn't matter, as long as it is non-empty and varies between replies.
- Placeholder argument values only need the right type and shape. They don't need to be meaningful, so running a sample unchanged may legitimately fail inside the tool.
- One tool request per message is enough. Several tool calls in one message are out of scope.
- Token usage and cost reporting for dummyplug replies can be zero or a rough estimate. Accurate accounting is not required.
- Voice (speech-to-text / text-to-speech) and image generation stay separate capabilities. This feature does not add a dummy for them.
