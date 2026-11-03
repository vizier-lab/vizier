# Contract: the agent-state audit

**Feature**: #163 | **Date**: 2026-11-03

Every place the gateway side reads a running agent's state, where it lived before, and where it
lives now. FR-008 requires the record; FR-009 names the surfaces to cover.

---

## The rule the audit enforces

A gateway-side read of state that lives **inside a running agent** is a bug. It is legal to read
**stored** state (the database) and legal to read a **process-wide registry** that belongs to the
gateway process itself. It is not legal to read a live supervisor, a running instance, or a
per-agent in-memory registry owned by the worker.

The three kinds are distinguished in the table below, because they get different verdicts.

---

## The audit

| Surface | Where | What it reads | Kind | Now |
|---|---|---|---|---|
| The agent list | `service/agents.rs:346-356` | `transport.agent_health()` -> every supervisor's `SharedStatus` + the channel board | running agent | **Published view**: `agent.{id}.health` per agent, read from shared state |
| The health endpoint | `service/agents.rs:799` | the same | running agent | **Published view** |
| The ping endpoint | `service/agents.rs:815` | the same | running agent | **Seam**: `AgentsClient::health(agent_id)` for that one agent |
| The restart endpoint | `service/agents.rs:644` | the same, after the restart | running agent | **Seam**: `AgentsClient::restart`, then `health` |
| The overview counts | `service/overview.rs:70` | the same | running agent | **Published view** |
| An agent's integrations | `service/integrations.rs:206` | `status_of(&deps.channel_board, id)` - the running reader's health slot | running agent | **Open** - the channel board belongs to the `channels` role; the reader's health must reach the gateway. Tracked in #167's scope. |
| An agent's integration status | `service/integrations.rs:267` | `agent_statuses` -> the running agent's live tool snapshot + `deps.integration_health` | running agent | **Seam**: `AgentsClient::integration_status(agent_id)` |
| The setup state | `service/setup_state.rs:14` | `deps.setup.snapshot()` - the live build board | running agent | **Published view**: `agent.{id}.setup` |
| The deferred client-call claim | `vizier-http/src/topic.rs:101`, `:110`, `:443`; `agents/client/mod.rs:533` | `DeferredClaims` - the set of calls being delivered right now | running agent | **Stored row**: a lease on `client_tool_call` (`claimed_by`, `claimed_at`) |
| The tool listing | - | **No gateway-side read exists.** The only live listing (`ToolRouter::catalogue`, `agents/tools/mod.rs:919`) is called from inside the agent | - | **Covered**: the gateway serves stored config flags (`AgentDetail`); the live listing stays in the agent |
| The MCP server status | `integrations/mod.rs:243` -> `integrations/service.rs:107-121` | the running agent's MCP server statuses | running agent | **Covered**: the seam's `integration_status` |
| The side-chat rate limiter | `service/agents.rs:966`; `deps.side_chat_limits` | the rate limiter shared with the channel readers | process-wide registry | **Stays per node** (FR-012): the audit records it as a deliberate decision |
| The client's open sockets | `vizier-http/src/topic.rs:145`, `:169`, `:310`; `deps.client_connections` | the registry of open sockets by id | process-wide registry | **Stays**: a person's socket lives in the gateway they are connected to |
| The questions registry | `vizier-http/src/topic.rs:220-226`, `:330-333`; `deps.questions` | the open asks and the waiting replies | process-wide registry | **Stays**: keyed by session, and the ask is answered by the socket that holds it |
| The notification inbox | `service/notifications.rs:30` | `deps.notifications.project_replies(now)` | stored | **Stays**: the rows are in the database |
| The project read ledger | `service/topics.rs:419`; `service/projects.rs:1263-1265` | `deps.project_ledger.forget_session(slug)` - drops stale-read hashes | process-wide registry | **Stays**: a write, not a read of agent state |
| The open-turn set | `service/agents.rs:359`, `:801`; `service/overview.rs:89` | `storage.thinking_agents()` | stored | **Stays**: it is already in the database |
| The provider/model catalogue | `service/catalogue.rs`; `vizier-http/src/api/v1/catalogue.rs` | `deps.catalogue` | process-wide registry | **Stays**: it is the catalogue of providers, not of agents |

**Open entries**: one - the channel-reader health. It is marked open rather than left blank
(US7 scenario 3).

---

## The mechanical check

FR-003 requires that no code outside the agent host reads a running agent's state, and that the
check be *proven to fail* on a deliberate violation.

**The check**: a step in the CI `policy` job, next to "Roles stay independent". It scans
`crates/vizier-http/src` and `crates/vizier-core/src/service` for the handles that only the host may
touch - the supervisor's process map, the setup board's live snapshot, the channel board's status
lookup, the deferred-claims registry - and fails with the file and line when it finds one outside
the host.

**The proof**: a test adds a deliberate read, shows the check failing and naming it, and removes it
again.

**What it does not check**: the check is about *where* the read is. It cannot tell a correct read of
the published views from a wrong one; that is the audit's job and the tests'.

---

## Adding an entry

A new gateway-side read of agent state is a defect unless it goes through the seam, the published
views or storage. When one appears, it gets a row here and a reason - not a blank cell.
