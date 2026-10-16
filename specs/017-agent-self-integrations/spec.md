# Feature Specification: Agent Self-Managed Integrations

**Feature Branch**: `017-agent-self-integrations`

**Created**: 2026-10-16

**Status**: Draft

**Input**: User description: "let's add configs and tools to allow agent to edit their own mcp, addons, and channel. this configs can only be enabled when the user doing the config have the integration permission"

## Context

Since 016, an agent's MCP servers, addons (Brave Search) and Discord/Telegram connections are **integrations**: defined once, attached to agents, and managed by people holding `all_agents:integrations:manage` or `owned_agents:integrations:manage`. An agent cannot change any of them itself. If someone asks an agent in conversation to "hook yourself up to this MCP server" or "turn on web search", a person has to leave the conversation and do it in the WebUI.

This feature adds three per-agent switches, one for each integration family (**MCP servers**, **Addons**, **Channels**), and a set of agent tools that let the agent view and change its own integrations of a family whose switch is on. Turning a switch on is a grant of integration authority to the agent, so only a person who could manage that agent's integrations themselves may turn it on.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Grant an agent self-management of one integration family (Priority: P1)

An operator who manages an agent's integrations opens that agent's configuration and turns on "Agent may manage its own MCP servers". The other two switches (Addons, Channels) stay off. Another person who can edit the agent's configuration but holds no integration permission sees the switches but cannot turn them on.

**Why this priority**: The switches are the safety boundary of the whole feature. Without them the tools cannot be offered safely, and they are useful (as a visible, auditable grant) before any tool exists.

**Independent Test**: As a user with the integration permission over an agent, enable one switch and save; reload and confirm it persisted. As a user who can edit the agent but lacks the permission, try to enable a switch through the WebUI and through the HTTP API; confirm both are refused and the stored value is unchanged.

**Acceptance Scenarios**:

1. **Given** a user who may manage integrations on agent A, **When** they enable "manage own MCP servers" on A and save, **Then** the setting is stored and shown as on, and the other two switches are unchanged.
2. **Given** a user who may edit agent A's configuration but may not manage its integrations, **When** they view A's configuration, **Then** the three switches are shown read-only with an explanation of the permission required.
3. **Given** the same user, **When** they submit a configuration change to A that turns a switch on (directly through the API), **Then** the request is refused with a clear permission error and nothing in the request is saved.
4. **Given** the same user, **When** they save an unrelated change to A (for example its model), **Then** the save succeeds and the switches keep their stored values.
5. **Given** a user with `owned_agents:integrations:manage` who does not own agent A, **When** they try to enable a switch on A, **Then** it is refused.

---

### User Story 2 - Agent manages its own MCP servers in conversation (Priority: P1)

With the MCP switch on, a person tells the agent "connect yourself to the GitHub MCP server at https://…". The agent lists its current MCP servers, adds the new one, and from its next turn can use the new server's tools. Later it can edit that server's settings, disable it, or remove it.

**Why this priority**: MCP is the most common reason to change an agent's capabilities, and the highest-value self-service case.

**Independent Test**: Run an agent on the offline `dummyplug` provider with the MCP switch on, invoke the self-management tools through the sandbox to add, list, edit and remove an MCP server, and confirm each change appears in the WebUI's Integrations › MCP page for that agent and that the new server's tools appear in the next turn.

**Acceptance Scenarios**:

1. **Given** agent A with the MCP switch on, **When** A adds an MCP server, **Then** it is created, attached to A only, marked as created by A, and its tools become available from A's next turn without restarting A or interrupting any turn.
2. **Given** agent A with the MCP switch on, **When** A lists its MCP servers, **Then** it sees the servers attached to it with name, transport, enabled state and health, and secret values are shown redacted.
3. **Given** agent A with the MCP switch off, **When** A calls any MCP self-management tool, **Then** the call fails with a message saying the capability is not enabled for this agent, and nothing changes.
4. **Given** agent A with the MCP switch on, **When** A tries to add a server whose name is already used by another server attached to A, **Then** the call fails with the same rule violation a person would get.
5. **Given** agent A with the MCP switch on, **When** A tries to change or delete an MCP server attached to A that a person created, **Then** the change is allowed if that server is attached to A and to no other agent.
6. **Given** agent A with the MCP switch on and an MCP server attached to both A and agent B, **When** A tries to edit, disable or delete it, **Then** the call is refused because the change would affect B; **When** A detaches it from itself, **Then** it is detached from A only and B is unaffected.
7. **Given** agent A with the MCP switch on, **When** A adds a local (command-launched) MCP server, **Then** it is accepted with the same options a person has, and the server's command runs on the vizier host.

---

### User Story 3 - Agent manages its own addons (Priority: P2)

With the Addons switch on, the agent can see the addon catalogue, install an addon on itself with a configuration (for example Brave Search with an API key it was given), change that configuration, enable/disable it, and uninstall it.

**Why this priority**: Addons are few today (Brave Search only), so the value is lower than MCP, but the mechanics are the same.

**Independent Test**: With the Addons switch on, have the agent install Brave Search on itself through its tools; confirm Integrations › Addons shows it installed for that agent and the `brave_search_*` tools appear in the next turn; uninstall and confirm they disappear.

**Acceptance Scenarios**:

1. **Given** agent A with the Addons switch on, **When** A lists addons, **Then** it sees every addon vizier offers and whether each is installed on A.
2. **Given** agent A with the Addons switch on, **When** A installs an addon with valid settings, **Then** it is attached to A only, replacing any configuration of that addon A already had (the existing one-per-addon-per-agent rule).
3. **Given** agent A with the Addons switch off, **When** A calls an addon self-management tool, **Then** it is refused and nothing changes.

---

### User Story 4 - Agent manages its own channel connections (Priority: P3)

With the Channels switch on, the agent can see its Discord and Telegram connections and their health, add a connection with a bot token it was given, replace a token, enable/disable a connection, and delete it.

**Why this priority**: Channel connections are rarely changed and a token mistake can take the agent offline on that platform, so it is the least urgent and the most sensitive to hand over.

**Independent Test**: With the Channels switch on, have the agent add a Telegram connection with a test token through its tools; confirm Integrations › Channels shows it for that agent with its health; delete it and confirm it is gone.

**Acceptance Scenarios**:

1. **Given** agent A with the Channels switch on and no Telegram connection, **When** A adds a Telegram connection, **Then** it is created as A's connection and its reader starts, without affecting A's other channels.
2. **Given** agent A with the Channels switch on, **When** A tries to add a second Discord connection, or use a token already saved on another connection, **Then** the call fails with the same rule violation a person would get.
3. **Given** agent A with the Channels switch on, **When** A disables or deletes the connection through which the current conversation is taking place, **Then** the change is made and the agent is told, before doing it, that it will stop receiving messages on that connection (see Edge Cases).

---

### Edge Cases

- **Permission lost after granting**: the person who enabled a switch later loses their integration permission, or is deleted. The switch stays on; it is a property of the agent, not of the person's session. Anyone who may manage the agent's integrations can turn it off.
- **Turning a switch off**: allowed for anyone who may manage the agent's integrations. Turning it off does not undo changes the agent already made; the integrations it created remain, and become managed by people only.
- **Change mid-turn**: the agent adds an MCP server during a turn. The current turn keeps the tool set it started with; the new tools appear from the next turn (016's one-consistent-set-per-turn rule).
- **Self-lockout via channels**: the agent deletes or disables the connection it is being spoken to on. The change is allowed (the switch is the operator's consent), but the tool's description and result warn that the agent will stop receiving messages there; the reply to the current message may not be delivered.
- **Secrets**: an agent never reads back a secret in full (bot tokens, API keys, MCP environment values), including ones it saved itself. It can replace them.
- **Scheduled and dream runs**: self-management tools are not available in the dream cycle, so unattended reflection never changes integrations. In scheduled tasks they are available like any other tool when the switch is on.
- **Delegated and subagent runs**: a subagent or a delegated agent acts under its own agent's switches, never the launching agent's.
- **Agent deleted**: the integrations it created follow the existing rules (its channel connections are deleted with it; MCP servers and addon configurations it created remain and are manageable by all-scope managers).
- **Local MCP servers**: a local MCP server runs a command on the vizier host. With the MCP switch on, an agent may add and edit local servers just as a person can, so enabling the MCP switch amounts to letting the agent run commands on the host. The switch's description in the configuration form MUST say so.
- **Integration attached to several agents**: an agent may detach itself from it but may not edit, enable/disable or delete it. If it later becomes attached to that agent alone, the agent may manage it fully.

## Requirements *(mandatory)*

### Functional Requirements

**Switches**

- **FR-001**: Each agent MUST have three independent settings, all off by default: *may manage its own MCP servers*, *may manage its own addons*, *may manage its own channel connections*.
- **FR-002**: A change to any of the three settings MUST be accepted only from a person who may manage integrations on that agent (holds `all_agents:integrations:manage`, or holds `owned_agents:integrations:manage` and owns the agent). Any other request that would change one of them MUST be refused as a whole with a permission error naming the required permission.
- **FR-003**: Saving an agent configuration without changing these settings MUST NOT require the integration permission and MUST leave the stored values unchanged.
- **FR-004**: The agent configuration form MUST show the three settings to everyone who can view the agent's configuration, editable only by people allowed by FR-002, with an explanation for the others.
- **FR-005**: Existing agents MUST start with all three settings off.
- **FR-006**: Changing these settings MUST NOT restart the agent or interrupt its turns; the change takes effect from the agent's next turn.

**Agent tools**

- **FR-007**: For each family, the agent MUST have tools to list its integrations of that family (with status, enabled state and redacted settings), create one attached to itself, edit one, enable/disable one, and remove one from itself. For addons, listing MUST also show the addons it does not have yet. For MCP servers, it MUST be able to see the tools a server offers.
- **FR-008**: Every self-management tool MUST check, at the time of the call, that the agent's switch for that family is on, and MUST refuse otherwise without changing anything.
- **FR-009**: An agent MUST only ever change integrations on itself. It MUST NOT attach anything to, or detach anything from, another agent, and MUST NOT change an integration attached to any other agent.
- **FR-010**: An agent MUST be able to edit, enable/disable and delete any integration of an enabled family that is attached to it and to no other agent, whoever created it. For an integration also attached to another agent, it MUST only be able to detach itself; any other change MUST be refused with a message naming why (the change would affect other agents). Whether an integration is attached elsewhere MUST be judged at the time of the call.
- **FR-011**: Every rule enforced for people (one connection per platform per agent, a token used once, one configuration per addon per agent, unique MCP server names per agent, kind-specific validation) MUST be enforced identically for agents, and violations MUST be returned to the agent as readable errors.
- **FR-012**: Integrations an agent creates MUST record the agent as their creator and MUST be visible in the Integrations menus to people who may see that agent's integrations, marked as created by the agent.
- **FR-013**: Integrations an agent creates MUST be private to that agent: they MUST NOT appear as attachable to other agents unless a person with `all_agents:integrations:manage` makes them so.
- **FR-014**: Secrets MUST never be returned to the agent in full.
- **FR-015**: Self-management tools MUST be offered through the existing tool categories so that an agent with all switches off sees no difference in its tool list, and an agent with a switch on sees only that family's tools.
- **FR-016**: Self-management tools MUST NOT be available to the dream cycle.
- **FR-017**: Changes made by an agent MUST take effect exactly as the same change made by a person does: no agent restart, no aborted turn, new tools from the next turn, channel changes on reconnect.
- **FR-018**: Every change an agent makes to an integration MUST be recorded with the agent as actor and the time, and MUST be visible to people who may manage that agent's integrations.
- **FR-019**: MCP self-management MUST offer both transports (local command-launched and HTTP) with the same options a person has. The MCP switch in the configuration form MUST warn that enabling it lets the agent run commands on the vizier host.

### Key Entities

- **Self-management settings**: three on/off values on an agent, one per integration family (MCP servers, addons, channel connections). Changed only by people allowed to manage that agent's integrations.
- **Integration** (existing, from 016): gains the possibility of an agent as its creator, in addition to a person.
- **Integration change record**: who (person or agent) changed which integration, how (created, edited, enabled, disabled, attached, detached, deleted), and when.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A person can get an agent connected to a new MCP server by asking it in conversation, with no WebUI visit, in a single exchange; the server's tools are usable on the very next message.
- **SC-002**: 100% of attempts by a person without the integration permission to turn a switch on are refused, through every route (WebUI and API), and leave no partial change.
- **SC-003**: 100% of self-management tool calls on a family whose switch is off are refused with no change.
- **SC-004**: No integration change made by an agent restarts that agent, aborts any of its turns, or affects any other agent.
- **SC-005**: Every integration an agent creates or changes is identifiable in the WebUI as agent-made within one page view.
- **SC-006**: An agent with all three switches off has the same tool list as before this feature.

## Assumptions

- "Integration permission" means the 016 permissions: `all_agents:integrations:manage`, or `owned_agents:integrations:manage` on an agent the person owns. Owned-scope holders may grant switches only on agents they own.
- The switches are part of the agent's configuration (the Misc › Config page), not of the Integrations pages, because they describe the agent's capabilities.
- No human approval step is required for each agent-made change: turning the switch on is the operator's consent. The change record (FR-018) is the oversight mechanism.
- An agent never sees or attaches integrations it did not create unless they are already attached to it; it cannot browse other people's MCP servers or addon configurations, including shared-scope ones.
- "Shared" in FR-010 means attached to more than one agent, not the shared *scope* (created by an all-scope manager). Scope does not affect what an agent may change on itself.
- Channel self-management covers the existing platforms (Discord, Telegram) only.
- The tools follow existing naming and category conventions (snake_case names, one category per family) and are reached through code mode like every other tool.
