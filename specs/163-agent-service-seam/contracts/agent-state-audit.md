# Contract: the agent-state audit

**Feature**: #163 | **Date**: 2026-11-03 (re-verified against the code after slices 1-6)

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
| The agent list | `service/agents.rs:337` (`list`), `:784` (`failed_agents`) | every supervisor's `SharedStatus` + the channel board | running agent | **Published view**: `agent.{id}.health` per agent, read from shared state |
| The health endpoint | `service/agents.rs:800` (`health`) | the same | running agent | **Published view** |
| The ping endpoint | `service/agents.rs:817` (`ping`) | the same | running agent | **Seam**: `AgentsClient::health(agent_id)` for that one agent |
| The restart endpoint | `service/agents.rs:596` (`restart`) | the same, after the restart | running agent | **Seam**: `AgentsClient::restart`, then `health` |
| The overview counts | `service/overview.rs:66` (`failed_agents`) | the same | running agent | **Published view** |
| An agent's integrations | `service/integrations.rs:206` | `status_of(&deps.channel_board, id)` - the running reader's health slot | running agent | **Open** - the channel board belongs to the `channels` role; the reader's health must reach the gateway. Tracked in #167's scope. In single node the host folds this same health into the published health view, so nothing is lost there ([shared-state-views.md](shared-state-views.md), rule 7). This is the one read the mechanical check still allows, as an audited exception. |
| An agent's integration status | `service/integrations.rs:267` -> `integrations/service.rs:106` (`agent_statuses`) | `agent_statuses` -> the running agent's live tool snapshot + `deps.integration_health` | running agent | **Seam**: `AgentsClient::integration_status(agent_id)` |
| The setup state | `service/setup_state.rs:19` | `deps.setup.snapshot()` - the live build board | running agent | **Published view**: `agent.{id}.setup`, published by the board. This process's own board is read as well - and stays an audited exception - because an agent being created is not in storage yet, so its setup would otherwise be invisible. |
| The deferred client-call claim | `agents/client/mod.rs:499` (`deliver_deferred`), `:510` | `DeferredClaims` - the set of calls being delivered right now | running agent | **Stored row**: a lease on `client_tool_call` (`claimed_by`, `claimed_at`) |
| The tool listing | - | **No gateway-side read exists.** The only live listing (`ToolRouter::catalogue`, `agents/tools/mod.rs:919`) is called from inside the agent | - | **Covered**: the gateway serves stored config flags (`AgentDetail`); the live listing stays in the agent |
| The MCP server status | `integrations/mod.rs:243` -> `integrations/service.rs:106-121` | the running agent's MCP server statuses | running agent | **Covered**: the seam's `integration_status` |
| The side-chat rate limiter | `service/agents.rs`; `deps.side_chat_limits` | the rate limiter shared with the channel readers | process-wide registry | **Stays per node** (FR-012): the audit records it as a deliberate decision |
| The client's open sockets | `vizier-http/src/topic.rs`; `deps.client_connections` | the registry of open sockets by id | process-wide registry | **Stays**: a person's socket lives in the gateway they are connected to |
| The questions registry | `vizier-http/src/topic.rs`; `deps.questions` | the open asks and the waiting replies | process-wide registry | **Stays**: keyed by session, and the ask is answered by the socket that holds it |
| The notification inbox | `service/notifications.rs` | `deps.notifications.project_replies(now)` | stored | **Stays**: the rows are in the database |
| The project read ledger | `service/topics.rs`; `service/projects.rs` | `deps.project_ledger.forget_session(slug)` - drops stale-read hashes | process-wide registry | **Stays**: a write, not a read of agent state |
| The open-turn set | `service/agents.rs`, `service/overview.rs` | `storage.thinking_agents()` | stored | **Stays**: it is already in the database |
| The provider/model catalogue | `service/catalogue.rs`; `vizier-http/src/api/v1/catalogue.rs` | `deps.catalogue` | process-wide registry | **Stays**: it is the catalogue of providers, not of agents |

**Open entries**: one - the channel-reader health. It is marked open rather than left blank
(US7 scenario 3).

---

## The mechanical check

FR-003 requires that no code outside the agent host reads a running agent's state, and that the
check be *proven to fail* on a deliberate violation.

**The check**: `scripts/163-agent-state-read-check.sh`, a step in the CI `policy` job, next to
"Roles stay independent". It scans `crates/vizier-http/src` and `crates/vizier-core/src/service`
for the handles that only the host may touch - the fleet-wide health read, the setup board's
live snapshot, the channel board's status lookup and the deferred-claims registry - and fails
with the file and the line when it finds one outside the host.

**The proof**: a deliberate read of `deps.transport.agent_health()` in `service/agents.rs` was
added, the check was run, and it failed naming `crates/vizier-core/src/service/agents.rs:789`.
The read was removed and the check passed again. A check that has never been seen to fail is not
a check.

**The audited exceptions**: the check's exception list holds the reads that stay where they are,
each naming the row above that explains it. There are two: the channel-reader health (open,
#167) and this process's own setup board (an agent being created is not in storage yet). A new
read outside that list is refused.

**What it does not check**: the check is about *where* the read is. It cannot tell a correct read of
the published views from a wrong one; that is the audit's job and the tests'.

---

## Adding an entry

A new gateway-side read of agent state is a defect unless it goes through the seam, the published
views or storage. When one appears, it gets a row here and a reason - not a blank cell.
