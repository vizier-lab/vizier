# Feature Specification: Integrations Section

**Feature Branch**: `016-integrations-section`

**Created**: 2026-10-16

**Status**: Draft

**Input**: User description: "i want to add a new separate sections "integrations" on webui sidebar, on this sections is 3 menu: "channels", "services", "mcp". all related agent config will be moved there and require no agent restart, leaving tool section on agent config for builtin tool(s). as for "services", it was new menu for "first party" integration from us, for now it will contains brave search"

## Context

Today everything that connects an agent to the outside world is part of that agent's configuration form: Discord and Telegram bot tokens ("Channel Tokens"), the Discord/Telegram tool toggles, Brave Search (toggle + API key + safesearch), and MCP servers. Every save of that form rebuilds the agent, and a rebuild aborts the agent's running turns (#52). Configuring the same MCP server or Brave key for three agents means entering it three times.

This feature moves those connections out of the agent form into a dedicated **Integrations** area of the WebUI, with three menus — **Channels**, **Services**, **MCP** — and makes changing an integration take effect without restarting the agents that use it. The agent form keeps only the agent's built-in tools (fetch, HTTP client, text-to-speech, speech-to-text, read image, image generation, shell) and tool settings.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Manage MCP servers in one place and attach them to agents (Priority: P1)

An operator opens **Integrations › MCP**, adds an MCP server once (name, command or URL, environment, headers), and chooses which agents may use it. Later they edit the server's environment (e.g. rotate a token) or detach it from one agent. The agents pick up the change on their next turn; a conversation in progress is not cut off.

**Why this priority**: MCP is the integration most often shared between agents and most often edited, and it is the one whose edits most often interrupt work today. It alone proves the full model: defined once, attached to many, changed live.

**Independent Test**: With two agents running on the offline test provider, add an MCP server attached to both, confirm both see its tools; edit it while one agent is mid-turn; confirm that turn completes and the next turn of each agent uses the edited server, and that neither agent's health shows a restart.

**Acceptance Scenarios**:

1. **Given** no MCP servers are configured, **When** the operator adds a server and attaches it to agents A and B, **Then** both agents can list and call that server's tools on their next turn, without either agent restarting.
2. **Given** a server attached to agents A and B, **When** the operator detaches it from B, **Then** B's next turn no longer offers those tools and A is unaffected.
3. **Given** agent A is in the middle of a turn, **When** the operator edits a server A uses, **Then** A's current turn finishes normally and the edit applies from A's next turn.
4. **Given** a server that cannot connect, **When** an attached agent takes a turn, **Then** the turn proceeds without that server's tools and the MCP page shows that server as unreachable with the last error.
5. **Given** an agent's configuration form, **When** the operator opens it, **Then** there is no MCP server editor there; instead it shows which MCP servers are attached, linking to Integrations › MCP.

---

### User Story 2 - Connect an agent to Discord or Telegram from the Channels menu (Priority: P1)

An operator opens **Integrations › Channels**, adds a Discord or Telegram bot connection (token), and assigns it to an agent. The bot comes online for that agent without restarting the agent. Replacing the token, pausing the connection, or reassigning it to a different agent only restarts that connection.

**Why this priority**: Channel tokens are the other piece of config people change most, and a bad token today makes the whole agent rebuild and can refuse the save. Separating them makes a channel problem a channel problem.

**Independent Test**: Assign a Telegram connection to a running agent, confirm the bot answers; replace the token with an invalid one and confirm only the connection shows failed while the agent keeps answering in the WebUI; restore it and confirm the bot answers again — all without the agent's health showing a restart.

**Acceptance Scenarios**:

1. **Given** a running agent with no channels, **When** the operator adds a Discord connection assigned to it, **Then** the bot comes online for that agent and the agent's ongoing WebUI conversations are not interrupted.
2. **Given** a connection with a token the platform rejects, **When** it is saved, **Then** the connection is shown as failed with the platform's reason, and the agent itself stays running.
3. **Given** a connection assigned to agent A, **When** the operator reassigns it to agent B, **Then** messages to that bot are answered by B from then on, and A keeps running.
4. **Given** a connection, **When** the operator disables it, **Then** the bot goes offline and stays saved, ready to be re-enabled.
5. **Given** a channel connection, **When** the operator views it, **Then** they can see its status (connected, reconnecting, failed with reason, disabled) and which agent it serves.
6. **Given** a Discord or Telegram connection assigned to an agent, **When** that agent takes a turn, **Then** it has that platform's messaging tools; an agent with no such connection does not.

---

### User Story 3 - Configure Brave Search as a first-party service (Priority: P2)

An operator opens **Integrations › Services**, sees the first-party services vizier offers (for now: Brave Search), enters the Brave API key and its options once, and chooses which agents may use it. Agents gain or lose web search on their next turn.

**Why this priority**: Valuable and simple, but a single service today; the Services menu is mainly the home for future first-party integrations.

**Independent Test**: Configure Brave Search for agent A only; confirm A can search on its next turn and agent B cannot; change the key; confirm A's next search uses the new key with no restart.

**Acceptance Scenarios**:

1. **Given** Brave Search is not configured, **When** the operator opens Services, **Then** Brave Search is listed as available but not set up, with what it does and what it needs.
2. **Given** the operator enters a key and enables the service for agent A, **When** A takes its next turn, **Then** A can search the web; agents not enabled cannot.
3. **Given** Brave Search is configured, **When** the operator changes the key or safesearch setting, **Then** the next search by any enabled agent uses the new setting, without a restart.
4. **Given** the key is invalid, **When** an agent searches, **Then** the agent receives a clear failure for that search only, and the Services page shows the last error.

---

### User Story 4 - Existing setups carry over without re-entry (Priority: P1)

An operator upgrades vizier. Every agent's existing Discord/Telegram tokens, Brave Search settings and MCP servers appear under Integrations, attached to the same agents, and work exactly as before.

**Why this priority**: Without this, the upgrade silently disconnects bots and drops tools.

**Independent Test**: Start the new version against a data directory from the previous release with agents using each kind of integration; confirm every one appears under Integrations, attached to the right agent, and still works.

**Acceptance Scenarios**:

1. **Given** agents with channel tokens, Brave Search and MCP servers configured the old way, **When** vizier starts after the upgrade, **Then** each appears once under the matching Integrations menu, attached to the agent(s) it came from, and the agents behave as before.
2. **Given** two agents with byte-identical MCP server definitions, **When** migrated, **Then** they become one MCP integration attached to both. Definitions that differ in any way stay separate.
3. **Given** two agents with different Brave API keys, **When** migrated, **Then** no agent loses search and no agent starts using another agent's key.

---

### User Story 5 - Agent config shows only built-in tools (Priority: P3)

An operator opens an agent's configuration and finds only that agent's own settings and built-in tools; a read-only summary shows which integrations the agent uses, each linking to where it is managed.

**Why this priority**: The clean-up is the visible result of the change but has no value on its own.

**Independent Test**: Open the config of an agent with all integration kinds attached; confirm no token, MCP or Brave editors are present and the summary links resolve.

**Acceptance Scenarios**:

1. **Given** any agent, **When** its configuration is opened, **Then** no channel tokens, Discord/Telegram tool toggles, Brave Search settings or MCP servers are editable there.
2. **Given** an agent with integrations attached, **When** its configuration is opened, **Then** it lists them (kind, name, status) with links into the Integrations area.

### Edge Cases

- **Integration deleted while attached**: the operator is shown the agents it is attached to and must confirm; after deletion those agents simply lose it from their next turn.
- **Agent deleted**: its attachments are removed; integrations it used remain (a channel connection becomes unassigned and goes offline).
- **One bot token used twice**: a token already used by another channel connection is refused, since two readers on one bot would compete for its messages.
- **Unassigned channel connection**: allowed (saved but offline), shown as unassigned.
- **Change during a turn**: a turn always runs with the integration set it started with; changes apply from the next turn. A channel connection being restarted does not drop the agent's in-flight reply on other channels.
- **Agent currently failed/retrying**: attaching or editing integrations is still allowed and is picked up when the agent is next running.
- **MCP tool-name collisions** between two servers attached to one agent are resolved the same way as today (suffixes), and remain stable for an agent as long as its attached set does not change.
- **Secrets**: tokens, API keys and secret-bearing MCP environment values are never shown back in full after saving.
- **Isolation**: an MCP server attached to several agents still runs as a separate connection per agent; one agent's crash or hang of that server does not affect the others.
- **Service unconfigured but enabled for an agent** (e.g. key removed): agents no longer get the search tool and the Services page flags the service as incomplete.

## Requirements *(mandatory)*

### Functional Requirements

**Navigation**

- **FR-001**: The WebUI sidebar MUST have a separate **Integrations** section, distinct from the per-agent menu, containing three menus: **Channels**, **Services**, **MCP**.
- **FR-002**: The Integrations section MUST be usable without an agent selected, since integrations are not owned by a single agent.

**Integrations model**

- **FR-003**: Integrations MUST be defined independently of agents and then attached to agents. MCP servers and services MAY be attached to any number of agents; a channel connection MUST serve at most one agent at a time.
- **FR-004**: Each integration MUST show its current status and, when unhealthy, its last error, and MUST list the agents it is attached to.
- **FR-005**: Managing integrations MUST be governed by two role permissions, following the existing `all_agents:*` / `owned_agents:*` scheme:
  - **`all_agents:integrations:manage`**: may view, create, edit, enable/disable and delete every integration, and attach or detach any integration to or from any agent.
  - **`owned_agents:integrations:manage`**: may create integrations, and view, edit, enable/disable and delete the integrations they created. They may attach and detach integrations only on agents they own. What they may attach is their own integrations and the shared ones created by holders of `all_agents:integrations:manage`. They MUST NOT see, edit or attach integrations created by other owned-scope users.
  - A user with neither permission can still see, read-only, which integrations are attached to agents they can view. Without either permission the Integrations menus are hidden from the sidebar.
- **FR-005a**: Every integration MUST record who created it, so the owned scope can be enforced. When an integration is detached from or deleted from an agent, the operator MUST see which agents (and, for all-scope managers, whose agents) are affected.
- **FR-006**: Secrets (bot tokens, API keys, secret-bearing MCP environment values and headers) MUST NOT be displayed in full after saving; operators MUST be able to replace them without retyping other fields.

**Channels**

- **FR-007**: Operators MUST be able to add, edit, enable/disable, reassign and delete Discord and Telegram connections from Integrations › Channels.
- **FR-008**: A connection's platform messaging tools (Discord / Telegram tools) MUST be available to the agent the connection is assigned to, and MUST NOT be configurable separately in the agent form.
- **FR-009**: The same bot token MUST NOT be saved on two connections.
- **FR-010**: A rejected token or failed connection MUST be reported on the connection and MUST NOT prevent the save, stop the agent, or block other connections.

**Services**

- **FR-011**: Integrations › Services MUST list every first-party service vizier offers, configured or not. Initially this is **Brave Search** (API key, safesearch).
- **FR-012**: Operators MUST be able to configure a service once and choose which agents may use it.
- **FR-013**: The Services menu MUST be able to take future first-party services without changes to the agent configuration form.

**MCP**

- **FR-014**: Operators MUST be able to add, edit, delete and attach/detach MCP servers from Integrations › MCP, with the same server options available today.
- **FR-015**: Operators MUST be able to see, per MCP server, whether it is reachable and which tools it offers.

**Live changes — no restart**

- **FR-016**: Creating, editing, attaching, detaching, enabling, disabling or deleting any integration MUST NOT restart, rebuild or interrupt any agent. Agents' running turns MUST NOT be aborted and queued requests MUST NOT be dropped because of an integration change.
- **FR-017**: Changes to services and MCP servers MUST take effect from each affected agent's next turn. Changes to a channel connection MUST take effect as soon as that connection reconnects, which MUST happen without affecting the agent's other channels.
- **FR-018**: A turn MUST use one consistent set of integrations from start to end.

**Agent config**

- **FR-019**: The agent configuration form MUST no longer contain channel tokens, Discord/Telegram tool toggles, Brave Search settings or MCP servers. It MUST keep the agent's built-in tools and their settings (fetch, HTTP client, text-to-speech, speech-to-text, read image, image generation, shell, tool timeout).
- **FR-020**: The agent configuration form MUST show a read-only summary of the agent's attached integrations, linking to where each is managed.
- **FR-021**: Agent create/update requests that still carry the moved fields MUST be rejected or ignored with a clear message pointing to Integrations; they MUST NOT silently overwrite integration state.

**Migration**

- **FR-022**: On first start after upgrade, every existing per-agent channel token, Brave Search setting and MCP server MUST be migrated into Integrations and attached to the agent(s) it came from, so every agent keeps exactly the capabilities it had.
- **FR-023**: Identical MCP definitions across agents MUST be merged into one integration; non-identical definitions, and different Brave keys, MUST NOT be merged.
- **FR-024**: The migration MUST be one-time and idempotent: restarting again MUST NOT create duplicates.
- **FR-025**: The migration MUST preserve who can manage what. Every role that could configure these integrations through the agent form before the upgrade MUST get the matching integration permission:
  - roles with `all_agents:edit` gain `all_agents:integrations:manage`;
  - roles with `owned_agents:edit` gain `owned_agents:integrations:manage`.

  Each migrated integration is recorded as created by the owner of the agent it came from. An integration merged across agents with different owners is recorded as created by an all-scope manager, which makes it shared.

### Key Entities

- **Integration**: something outside vizier an agent can use. Has a kind (channel, service, MCP server), a creator (determining owned vs shared scope), a name, its settings (secrets included), an enabled flag, and a status (healthy / degraded / failed with last error / disabled / unassigned).
- **Channel connection**: a Discord or Telegram bot. Platform, token, assigned agent (zero or one), status. Brings its platform's messaging tools to its agent.
- **Service**: a first-party integration vizier ships. Initially Brave Search (API key, safesearch). Fixed catalogue; operators configure, they don't define new kinds.
- **MCP server**: an operator-defined tool server (launch command or URL, environment, headers, timeouts as today).
- **Attachment**: the link between an integration and an agent (many-to-many for services and MCP; one agent per channel connection).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of integration changes (create, edit, attach, detach, enable, disable, delete) complete with zero agent restarts and zero aborted turns.
- **SC-002**: An integration change is in effect for an affected agent's next turn in 100% of cases; a channel connection change is reflected within 30 seconds.
- **SC-003**: Adding one MCP server to 5 agents takes one definition and one attach step, instead of 5 separate entries.
- **SC-004**: After upgrade, 100% of previously configured channel tokens, Brave Search settings and MCP servers appear under Integrations attached to the same agents, with no operator re-entry and no loss of capability.
- **SC-005**: A broken integration (bad token, unreachable server, invalid key) never takes an agent offline; the agent keeps answering through its other channels.
- **SC-006**: An operator can find where to configure a bot token, web search key or MCP server from the sidebar in one click from any page.

## Assumptions

- **Channels in scope are Discord and Telegram.** The WebUI/HTTP channel is always on and is not an integration; it is not listed under Channels.
- **Built-in tools stay on the agent**: fetch, HTTP client, TTS, STT, read image, image generation and shell are not integrations even though some call external providers — those providers already have their own home in Settings › Providers.
- **Per-agent isolation is preserved**: sharing an MCP definition between agents shares configuration, not a running connection (constitution: no global MCP singletons).
- **Brave Search is a single service configuration** (one key for all agents enabled for it). Where the migration finds agents with different Brave keys, it keeps their keys distinct (e.g. as separate named Brave Search configurations) so nothing changes behaviour; how that is presented is a planning detail.
- **Dream cycle**: an agent's dream cycle sees the same attached integrations it saw before, subject to its existing restricted tool list.
- **`agents:mcp_config`**: once MCP leaves the agent form, this permission no longer guards MCP; MCP management is covered by the two integration permissions. Whether to retire it or keep it as an extra gate on MCP integrations is a planning decision. Either way, no role loses an ability it had before.
- **"Turn" granularity** for live changes is acceptable; changes need not reach a turn already in progress.
- The HTTP API gains endpoints for integrations; the old agent fields are retired from the agent API in the same release (breaking API change, flagged in the changelog).
