# Feature Specification: Agent Secrets

**Feature Branch**: `027-agent-secrets`

**Created**: 2026-10-24

**Status**: Draft

**Input**: User description: "let's make agent secret tools, it allows agent to set(replacing) and getting a secret. user should be able to manage secret in the webui. need permissions for this all_agents:secret and owned_agents:secret"

## Context

Agents routinely need a credential that is not part of their identity or memory: an API token for a third-party service, a webhook URL, a service password. Today there is no general place to keep one. An agent either has the value pasted into the conversation every time it is needed (so it is repeated in history, logs and model context), writes it into CORE or a memory document (where it is ordinary content, readable by any memory search), or — only if the service happens to be an integration — stores it in that integration's settings.

This feature adds a small **per-agent secret store**. An agent gets two tools: one to **set** a secret, which creates it or replaces the value if the name already exists, and one to **get** a secret by name. The same store is managed by people in the WebUI, where the names are listed and values can be created, replaced and deleted, subject to two new permissions: `all_agents:secret` (every agent) and `owned_agents:secret` (the agents a person owns).

The store is deliberately narrow. Secrets belong to exactly one agent and are never shared across agents. Values are **write-only for people**: once saved, a value is never shown again in the WebUI or returned by a management request — a person replaces it rather than reading it, the same rule integrations already follow. Every change is recorded with who made it and which name changed, never the value.

An agent's own `get` is the only route by which a value leaves the store. It passes the value into the agent's running `execute_python` script, where the script can use it (for example by passing it to `http_client`), but the value is **not shown to the model** and is redacted from the script's report, stdout, nested tool arguments, saved history, logs and telemetry before any of them leave the turn. Setting a secret is necessarily different: the value appears in the message or script that supplies it before it is stored. The non-disclosure guarantee covers the stored value and every later retrieval, not that original input.

Out of scope for this feature: sharing a secret between agents, version history or expiry of old values, automatic rotation, injecting secrets into an agent's environment or configuration, and encrypting values in the application (at-rest protection remains the host's responsibility, as it is for provider credentials today).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - An agent saves a credential and uses it later (Priority: P1)

A person tells the agent "save this Linear token as `LINEAR_TOKEN`" and pastes the token. The agent stores it under that name. Days later, in a different conversation and after a restart, the person asks the agent to file an issue in Linear; the agent retrieves `LINEAR_TOKEN` and uses it without the person pasting it again. When the token is rotated, the person gives the new one and the agent replaces the old value under the same name.

**Why this priority**: This is the core of the feature. Without agent-facing set and get there is no secret store at all.

**Independent Test**: On the offline `dummyplug` provider, have an agent call the set tool for a name, then call the get tool for the same name in a later turn. Prove the script received the exact value with a side effect the script can trigger (for example sending it to a local echo endpoint and checking the received header), and confirm the report and saved history show `[redacted]`. Restart vizier and confirm it still does. Set the same name again and confirm the value changed and the old one is gone.

**Acceptance Scenarios**:

1. **Given** an agent with no secret named `LINEAR_TOKEN`, **When** the agent sets it to a value, **Then** the value is stored for that agent and a later get inside an `execute_python` script receives exactly that value, including whitespace and line breaks, while the report shows it redacted.
2. **Given** `LINEAR_TOKEN` already exists, **When** the agent sets it to a new value, **Then** there is still one secret of that name, its value is the new one, and the previous value is no longer returned to any agent script.
3. **Given** the agent is asked to replace a secret, **When** it sets the existing name, **Then** it does not need to know or supply the previous value.
4. **Given** the agent asks for a name that does not exist, **When** it calls get, **Then** it receives a clear "not found" result naming the requested secret, nothing is created, and no other secret changes.
5. **Given** a secret was set in one turn, **When** the agent gets it in a later turn or after a vizier restart, **Then** the script receives the stored value and the report shows it redacted.
6. **Given** a scheduled task or a background job runs for the agent, **When** it retrieves a secret, **Then** it uses the same store and sees the current value.
7. **Given** the dream cycle runs for the agent, **When** its tool set is offered, **Then** neither secret tool is available.

---

### User Story 2 - A person manages an agent's secrets in the WebUI (Priority: P1)

A person who is allowed to manage an agent's secrets opens the agent's Secrets page. They see every secret name for that agent with when it was last changed and by whom, and a masked value. They add a new secret, replace an existing one, or delete one. They never see the stored value after saving. Someone without either secret permission cannot open the page, and a direct request from them is refused.

**Why this priority**: The request explicitly asks for WebUI management and for the two permissions; without them the only way to inspect or clean up the store is through the agent.

**Independent Test**: As a user holding `all_agents:secret`, open an agent's Secrets page, create a secret, reload and confirm it is listed with a masked value, replace it, delete it, and confirm each step persisted. Repeat the writes as a user with no secret permission and confirm every one is refused and nothing changed.

**Acceptance Scenarios**:

1. **Given** a person holding `all_agents:secret`, **When** they open any agent's Secrets page, **Then** they see that agent's secret names, their last change time and actor, and a masked value, and may create, replace and delete.
2. **Given** a person holding `owned_agents:secret` who owns agent A, **When** they open A's Secrets page, **Then** they may manage A's secrets; **When** they open an agent they do not own, **Then** they may not see or change its secrets.
3. **Given** a person holding neither permission, **When** they view an agent or call the secrets surface directly, **Then** the page is not offered to them and every read or write attempt is refused with a clear permission error and no change.
4. **Given** a person creates a secret in the WebUI, **When** they save it, **Then** the name is listed and the value is shown masked; **When** they reload the page or open it in another tab, **Then** the value is still not shown.
5. **Given** a secret created by the agent, **When** a permitted person opens the page, **Then** it is listed with the agent as the actor of the change.
6. **Given** the role editor, **When** a person with role-management rights edits a role, **Then** both secret permissions appear with readable labels and can be granted or revoked like other permissions.
7. **Given** the `superadmin` role, **When** it uses the secrets surface, **Then** it is allowed for every agent as it is for every other protected surface.

---

### User Story 3 - Rotate or remove a secret without disruption (Priority: P2)

A token leaks. A person replaces the secret in the WebUI (or asks the agent to replace it). The agent's very next retrieval returns the new token, with no restart and no interrupted turn. When a credential is no longer needed, a permitted person deletes the secret after a confirmation; the name disappears from the list and the agent's next get reports it as not found.

**Why this priority**: Rotation is why a secret store exists rather than a one-off store; it must be safe and immediate, but the feature is already useful without it.

**Independent Test**: Have a running agent retrieve a secret, replace the value from the WebUI while the agent is idle, and confirm the next retrieval returns the new value without the agent restarting or any turn being aborted. Then delete it and confirm retrieval fails as not found.

**Acceptance Scenarios**:

1. **Given** a stored secret, **When** a permitted person or the agent replaces its value, **Then** the next agent retrieval returns the new value and no restart, reconfiguration or turn cancellation occurs.
2. **Given** a turn already retrieved a value, **When** the secret is replaced or deleted before that turn ends, **Then** the in-flight turn keeps the value it already read; only later retrievals see the change.
3. **Given** a permitted person deletes a secret, **When** they confirm, **Then** the name is gone from the list and the agent's next get reports it as not found.
4. **Given** a deleted name, **When** a permitted person or the agent sets it again, **Then** it is created as a fresh secret with no trace of the old value.
5. **Given** a delete request for a name that does not exist, **When** it is submitted, **Then** it is refused as not found and nothing changes.
6. **Given** a person without either secret permission, **When** they attempt a replace or delete, **Then** it is refused and the stored value is unchanged.

---

### User Story 4 - Secrets stay private to their agent and accountable (Priority: P2)

An agent can only ever reach its own secrets: if it asks for a name another agent owns, the result is the same as a name that does not exist. Values never appear in a list, a change record, a log line, an error message or telemetry. Every create, replace and delete is recorded with the actor, the secret name and the time, so a permitted person can see who last changed what.

**Why this priority**: A secret store that leaks between agents or into logs is worse than no store; these guarantees are what make the feature safe to use.

**Independent Test**: Create the same secret name on agents A and B with different values; confirm each agent's script reads only its own. Attempt cross-agent names and confirm "not found". After a set and a get, search the log output, telemetry, change records, every list response, the execution report, its stdout and recorded nested tool arguments, and confirm the stored value appears in none of them.

**Acceptance Scenarios**:

1. **Given** agents A and B both have a secret named `TOKEN`, **When** each retrieves it, **Then** each receives its own value.
2. **Given** agent A knows a secret name that exists only on agent B, **When** A calls get, **Then** it gets "not found" and cannot tell the name exists elsewhere.
3. **Given** any secret value, **When** it is displayed, exported or recorded by the management surface, **Then** it is masked, and no human-facing read returns it in full.
4. **Given** any secret create, replace, delete or failed attempt, **When** it is recorded or logged, **Then** the record names the actor, action, secret name and time, and never contains the value.
5. **Given** a person loses their secret permission while their page is open, **When** they next read or write, **Then** the operation is refused and no secret changes.

---

### Edge Cases

- **Invalid names**: an empty name, a name of only whitespace, a name with unsupported characters, or a name beyond the length limit is refused with a message explaining the rule; nothing is created.
- **Invalid values**: an empty value or a value beyond the size limit is refused with a clear message; an existing secret is left unchanged by such a failed replace.
- **Name matching**: names are case-sensitive, so `Token` and `token` are different secrets. The WebUI shows names exactly as stored.
- **Duplicate set is replace, not conflict**: setting an existing name replaces its value atomically; there are never two entries with the same name for one agent.
- **Concurrent changes**: two permitted people replacing the same secret at the same time leave one of the two values and two change records; the store never holds a partial or mixed value.
- **Very long or multi-line values**: any text up to the size limit is accepted, including line breaks and leading/trailing spaces, and is returned byte-for-byte.
- **Agent update or restart**: secrets survive an agent update, restart or a vizier restart; no migration or manual step is needed.
- **Agent deletion**: deleting an agent deletes all of its secrets; none is left orphaned or available to another agent.
- **Agent created before this feature**: it starts with an empty secret store, and the two tools are available without configuration.
- **Unattended turns**: scheduled tasks and background jobs use the agent's current secrets; the dream cycle cannot read or write them.
- **WebUI permissions change**: revoking a permission takes effect on the next request, even if the person's page is already open; the page becomes unavailable on reload.
- **Direct requests**: hiding the page is not the only protection; every read and write through the server is checked independently and fails closed.
- **Name collision with other configuration**: a secret name does not collide with integration settings, environment variables or memory paths; the store is a separate namespace.

## Requirements *(mandatory)*

### Functional Requirements

**The store**

- **FR-001**: Every agent MUST have its own secret store. A secret MUST belong to exactly one agent and MUST NOT be visible to, shared with or transferable to another agent.
- **FR-002**: A secret MUST be identified by a name that is unique within its agent; names MUST be case-sensitive. Setting an existing name MUST replace its value rather than create a second entry.
- **FR-003**: Secret names and values MUST be validated against documented limits (1–128 characters from letters, digits, `.`, `_` and `-`; 8–65,536 bytes of value text). A violation MUST be refused with a message that explains the rule, and MUST NOT change any stored secret.
- **FR-004**: Secrets MUST persist across agent updates, agent restarts and vizier restarts.
- **FR-005**: Deleting an agent MUST delete all of its secrets. No secret may outlive its agent or be reachable by another agent.

**Agent tools**

- **FR-006**: An agent MUST have a tool to set a secret, which creates it if the name is new and replaces the value if it exists, and which reports only the name and whether it was created or replaced.
- **FR-007**: An agent MUST have a tool to get a secret by name, which returns the stored value into the running `execute_python` script so the script can use it. The value MUST NOT be exposed to the model or shown in the tool report; exact occurrences of it in the script's result, stdout, errors and nested tool-call arguments MUST be redacted before the report is saved or displayed.
- **FR-008**: A get for a name that does not exist MUST return a clear "not found" result naming it and MUST NOT create or change anything. The agent is not required to be able to list its secrets.
- **FR-009**: Secret tools MUST only ever act on the calling agent's own store. A name that exists only on another agent MUST behave exactly like a name that does not exist.
- **FR-010**: Secret tools MUST be available in ordinary, scheduled-task and background turns, and MUST NOT be offered to the dream cycle.
- **FR-011**: A set or replacement MUST take effect immediately: the agent's next get MUST return the new value, with no agent restart, reconfiguration or turn cancellation. A value already read by an in-flight turn is unaffected by later changes.
- **FR-012**: Invalid input, a missing name and any storage failure MUST come back to the agent as a readable error result, never as a silent success or a partial write.

**People and permissions**

- **FR-013**: An agent's Secrets page in the WebUI MUST list every secret name for that agent with its masked value, last change time and actor, and MUST offer create, replace and delete.
- **FR-014**: A person MUST be allowed to manage an agent's secrets if and only if they hold `all_agents:secret`, or hold `owned_agents:secret` and own that agent. The built-in `superadmin` role MUST retain its existing bypass.
- **FR-015**: A person who holds neither permission MUST NOT be offered the Secrets page and MUST have every read and write request refused with a clear permission error, leaving all secrets unchanged. Merely hiding the page MUST NOT be the protection.
- **FR-016**: Once a secret is saved, its value MUST NOT be returned in full by any human-facing read, list or page, even to a permitted person. The value MUST be shown masked, and a permitted person replaces it without having to know the old value.
- **FR-017**: Deleting a secret MUST require confirmation in the WebUI and MUST remove it immediately.
- **FR-018**: Both secret permissions MUST be listed with readable labels in role management and MUST be grantable and revocable like every other permission.

**Accountability and non-disclosure**

- **FR-019**: Every create, replace and delete — by a person or by an agent — MUST record the actor, the action, the secret name and the time, and MUST NOT record the value.
- **FR-020**: Once stored, a secret value MUST NOT appear in application logs, telemetry, error messages, change records, management responses, the WebUI or an agent turn's saved report/history. The script-only get is the only route by which the value leaves the store, and any exact occurrence of a retrieved value in the script's result, stdout, error or nested tool arguments MUST be replaced with a redaction marker before the report leaves the sandbox. A value supplied in the message or script that sets a secret is necessarily visible in that turn's input before it is stored; that input is outside this guarantee.
- **FR-021**: Change records MUST be visible to people allowed to manage that agent's secrets, so agent-made changes can be told apart from people's.

### Key Entities

- **Secret**: a name and a value owned by exactly one agent, plus when it was created and last changed and by whom. The value is write-only for people and usable only by its agent's scripts; the model and transcript never see it.
- **Secret change record**: who (a person or the agent) created, replaced or deleted which secret name, and when. Contains no value.
- **Secret permission**: `all_agents:secret` (every agent) and `owned_agents:secret` (agents the person owns), held through roles like existing permissions.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A person can store a credential and have the agent use it in a later message within 2 minutes, without editing agent configuration or restarting anything.
- **SC-002**: 100% of secret read and write attempts by people without either secret permission are refused through every route (WebUI and direct requests), and leave no partial change.
- **SC-003**: Zero stored secret values appear in any list, management response, change record, log line, telemetry output, execution report, stdout or recorded nested tool-call arguments when a secret is created, replaced, read or deleted. The sole exception is the message or script that supplied the value when it was set.
- **SC-004**: 100% of attempts by one agent to retrieve another agent's secret fail as "not found"; no cross-agent value is ever returned.
- **SC-005**: A replaced secret is visible to the agent's next retrieval 100% of the time, with zero restarts, reconfigurations or aborted turns.
- **SC-006**: Deleting an agent leaves zero orphaned secrets.
- **SC-007**: A stored secret remains retrievable by its agent after a full vizier restart in 100% of checks.
- **SC-008**: An authorized person can see every secret name for an agent in a single page view and can complete a create, replace and delete each in under 1 minute.

## Assumptions

- **Permission scope**: `all_agents:secret` covers every agent; `owned_agents:secret` covers agents the person owns. Holding the all-agents permission implies the owned one, and `superadmin` bypasses both, matching the other agent permissions.
- **Permissions govern people, not the agent's own tools**: the two tools act only on the calling agent's store and are not gated by the role of whoever is chatting. Anyone who can talk to an agent can ask it to use or reveal its secrets, so access to a conversation is effectively access to that agent's secrets; channel and conversation access is the trust boundary.
- **Write-only for people**: consistent with integration secrets, a stored value is never shown to a person again. There is no "reveal" action in the WebUI for this release.
- **Script-only retrieval**: `get_secret` returns the value to the running script, not to the model or the transcript. Redaction replaces exact occurrences of the stored value; a script that transforms or re-encodes the value (base64, URL-encoding, hashing) can defeat it. The model is told to use the value through another tool, not to print it.
- **Set input is already visible**: a value pasted into chat, or written into a script that calls `set_secret`, exists in that turn's input before it is stored. The guarantee starts with the stored value and every later retrieval.
- **Limits**: names are 1–128 characters from letters, digits, `.`, `_` and `-`; values are 8–65,536 bytes of text. The minimum keeps substring redaction from corrupting ordinary report text, and the exact limits are surfaced in errors.
- **Storage protection**: values are stored as the operator supplied them in the embedded storage and rely on host and disk protection, as provider credentials do today. Application-level encryption, key management, expiry, versioning and rotation automation are out of scope.
- **No per-agent switch**: secret tools are always available to the agent; there is no grant like the integration self-management switches in this release.
- **No default role change**: the new permissions are available for admins to assign; existing roles keep their current permissions apart from `superadmin`'s inherent bypass.
- **No sharing**: a secret is never readable by another agent, and there is no shared or global scope in this release.
- **Dream cycle excluded**: unattended reflection must not read or write secrets, matching the exclusion of self-management tools from dreams.
