# Data Model: Fleet Agent Service Seam

**Feature**: #163 - agent service seam
**Date**: 2026-11-03

The entities below are the ones this feature introduces or changes. Everything else in the storage
schema stays as it is.

---

## 1. Agent

The unit a person talks to. Unchanged by this feature, except that it now has an *owner*.

| Field | Type | Rules |
|---|---|---|
| `agent_id` | string | The stored id. Percent-encoded into exactly one subject token (`Subject::encode_token`), so an id containing a dot cannot widen a subscription. |
| `config` | `AgentConfig` | Stored in the database. The source of truth for what an agent *is*. |
| `owner` | owner reference | Derived, not stored here: the node whose lease `lease.agent.{id}` it holds. At most one owner at a time. |

**Validation**: an agent with no owner is *unavailable* - it is not an error state of the agent, it
is a state of the fleet. A gateway may not learn which node owns it (FR-007).

**State transitions** (the availability state machine the seam exposes):

```text
                 placement assigns
   [no owner] ------------------> [owned] --------> [owner lost] --placement--> [owned]
       |                             |
       |                             | owner releases / dies
       +-----------------------------+--> [no owner]
```

- **[no owner]** - a request fails **at once** with *agent unavailable / no owner* (FR-014). It is
  never a timeout.
- **[owned]** - requests are served.
- **[owner lost]** - a request in flight ends with *agent unavailable / owner lost* within the
  request deadline (FR-015). It never hangs.

## 2. Owner

The host that currently holds an agent. One per agent.

| Field | Type | Rules |
|---|---|---|
| `agent_id` | string | The agent held. |
| `lease` | lease reference | `lease.agent.{id}` (the epic's naming). Carries a fencing token. |
| `node` | node id | The node the host runs on. **Never** exposed to a gateway. |

**Validation**: a host may hold an agent only if it holds the agent's lease. A host that loses the
lease stops the agent's work immediately (the epic's fencing rule).

## 3. Agent host

The worker-side component. Built from a bus handle, a storage handle and the list of agents to
hold.

| Field | Type | Rules |
|---|---|---|
| `held` | set of agent ids | The agents this host runs. Empty is legal. |
| `processes` | map agent id -> supervisor | The running instances. **Only the host may read this.** |
| `builds` | in-flight builds | A build in flight refuses other creates, updates and deletes of that agent. |

**Rules**
- The host builds and runs **only** the agents it is told to hold (FR-004).
- It is asked about an agent it does not hold: it refuses with *agent unavailable*. It never builds
  the agent on the spot (US3 scenario 4).
- The guarantees that hold today hold here: build before persist or swap; requests queued during a
  restart are not lost; an update resumes the running turns; a delete drops them (FR-006).

## 4. Placement

The control-side decision of which host owns which agent.

| Field | Type | Rules |
|---|---|---|
| `assigned` | map agent id -> owner | In a single node: every stored agent -> the one local host. |

**Rules**: placement is the only thing that may answer "which node owns this agent", and it answers
only to the control role, never to a gateway.

## 5. AgentsClient (the seam)

A trait, not a record. Its operations and their errors are in
[contracts/agents-client.md](contracts/agents-client.md).

| Operation | Addresses | Answers with |
|---|---|---|
| `request` | `agent.{id}.req` | A stream of frames until the turn ends. |
| `abort` | `agent.{id}.req` | The abort request's own acknowledgement. |
| `create` | `agents.cmd` (fleet-wide) | The agent summary. |
| `update` | `agent.{id}.cmd` | The agent summary. |
| `delete` | `agent.{id}.cmd` | The agent summary. |
| `restart` | `agent.{id}.cmd` | The agent summary. |
| `health` | `agent.{id}.cmd` | That agent's health. |
| `integration_status` | `agent.{id}.cmd` | The agent's view of its integrations. |
| `refresh_integrations` | `agent.{id}.cmd` | Nothing; the agent rebuilds its tools. |

**Validation**: every operation that addresses an agent takes the agent id and nothing else. There
is no operation that takes a host, a node or a lease (FR-007).

## 6. Published view (shared state)

What the host publishes so a gateway can read it without knowing any owner.

| Field | Type | Rules |
|---|---|---|
| `key` | string | `agent.{id}.health` or `agent.{id}.setup`. The id is one encoded token. |
| `expires_at_ms` | integer | Epoch milliseconds. The reader treats the entry as absent once the clock passes it. |
| `data` | JSON | The view's payload. |

**Validation**
- The deadline is **at most 10 s** after the publish (FR-010, SC-008).
- A missing entry, or an expired one, reads as **unknown**, never as a stale value.
- The value is the whole entry: the backend has no TTL of its own (research R6).

**The two views**
- `agent.{id}.health` - the agent's run state, last error, failure count, next retry, model and
  warnings. The reader may add the channel-reader health it holds separately.
- `agent.{id}.setup` - the agent's build phase, message, error and download progress.

## 7. Deferred client call (changed)

The stored row that records a client tool call's outcome. **One row per call**, keyed by the history
entry's uid. The status vocabulary is unchanged.

| Column | Type | Rules |
|---|---|---|
| `history_uid` | text, primary key | The call id. |
| `status` | text | Unchanged: `pending`, `deferred`, `ok`, `error`, `disconnected`, `timeout`, `cancelled`, `interrupted`. |
| `claimed_by` | text, **new** | The gateway that holds the delivery claim. Null when unclaimed. |
| `claimed_at` | text, **new** | When the claim was taken. A claim older than the claim timeout is free again. |
| ... | | The rest of the row is unchanged. |

**How the columns arrive**
- **SQLite**: `add_column_if_missing(conn, "client_tool_call", "claimed_by", "TEXT")` and the
  same for `claimed_at`, called at startup next to the table's schema init
  (`storage/sqlite/mod.rs:613-633`). An existing desktop workspace gets them on first start.
- **PostgreSQL**: a new migration file, `migrations/0002_client_call_claim.sql`, appended to
  `MIGRATIONS` (`storage/sql/postgres/migrate.rs:20-24`). `0001_initial.sql` is never edited - the
  runner skips applied versions (`:76-78`), so editing it would leave an existing deployment
  without the columns.

**Rules**
- The claim is one conditional update - the row's compare-and-swap (FR-011):

  ```sql
  UPDATE client_tool_call SET claimed_by = ?1, claimed_at = ?2
   WHERE history_uid = ?3 AND status = 'deferred'
     AND (claimed_by IS NULL OR claimed_at < ?4)
  ```

  Exactly one gateway's update changes a row, so exactly one gateway claims a given call.
- A claim is released when the delivery ends, or when it lapses: `claimed_at` older than the
  timeout. A lapsed claim leaves the call `deferred`, so the next gateway finds it.
- The claim is **not** a status. `ClientCallOutcome` is the final-outcome vocabulary the WebUI
  reads, and an unknown status reads as `interrupted` (research R7).

## Relationships

```text
  Placement ──assigns──> Owner ──leases──> Agent
                                              │
                            ┌─────────────────┴──────────────────┐
                            │                                    │
                     Agent host holds                    Published view
                            │                                    │
                    supervisors, builds                      agent.{id}.health
                    running instances                       agent.{id}.setup
                            │
                            └── reaches over the bus ──> agent.{id}.req
                                                     └──> agent.{id}.cmd

  Deferred client call ──claimed by──> one gateway at a time
```

## What does not change

- `AgentConfig` and every other stored row.
- The HTTP API's request and response shapes (FR-019).
- The bus contract's existing subjects, envelope, lease and shared-state traits.
- The `ClientCallOutcome` status vocabulary.
