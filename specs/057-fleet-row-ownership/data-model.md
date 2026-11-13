# Data model: Fleet row ownership, fencing and lease-based recovery

**Feature**: `057-fleet-row-ownership` | **Research**: [research.md](research.md)

## New table: `owner_fence`

One row per ownership scope. Created on the first claim of a scope.

| Column | Type (SQLite / PostgreSQL) | Null | Meaning |
|---|---|---|---|
| `scope` | `TEXT` / `TEXT COLLATE "C"` | no, **PK** | `agent:{agent_id}` or `control` |
| `node` | `TEXT` / `TEXT COLLATE "C"` | no | node id of the current owner (the id `OwnerRef` already carries) |
| `token` | `INTEGER` / `BIGINT` | no | the **fence**: database-assigned, `+1` on every claim, never decreases |
| `lease_token` | `INTEGER` / `BIGINT` | yes | the bus `FencingToken` the claim was made under; `NULL` for a single-node claim |
| `claimed_at` | `TEXT` | no | UTC, through `time::stored_now` (sortable text, per the Time rules) |

**Rules**
- **Claim, Fleet** (`lease_token = t`): succeeds only if the stored `lease_token` is `NULL` or less than `t`; sets `token = token + 1` (or `1` for a new row), `node`, `lease_token = t`, `claimed_at`. Refused otherwise.
- **Claim, single node** (`lease_token = NULL`): always succeeds and bumps `token`.
- A claim updates the row, so it waits for every fenced write of the scope that holds `FOR SHARE` on it (PostgreSQL). On SQLite, `BEGIN IMMEDIATE` already serialises them.
- Rows are deleted with the agent (`delete_agent` deletes `agent:{id}`). `control` is never deleted.

## Columns added to in-flight tables

Added to `session_turn`, `task_run`, `background_job`, `client_tool_call` and `sensitive_ask`:

| Column | Type | Null | Meaning |
|---|---|---|---|
| `owner_node` | `TEXT` / `TEXT COLLATE "C"` | yes | the node that claimed the row; for diagnostics only, never used in a decision |
| `owner_token` | `INTEGER` / `BIGINT` | yes | the fence of the scope at claim time. `NULL` means not claimed yet (an accepted turn) or written before this feature |

**Scope of each table** (derived from columns each row already has, so no `scope` column is stored):

| Table | Scope | Written with the owner's token at |
|---|---|---|
| `session_turn` | `agent:{agent_id}` | **receipt** by the agent's owner (claim on receipt, research R6); `NULL` while only accepted by a sender |
| `client_tool_call` | `agent:{agent_id}` | `open_client_call` |
| `sensitive_ask` | `agent:{agent_id}` | when the ask is opened |
| `background_job` | `agent:{origin_agent}` | `open_background_job` |
| `task_run` | `control` | `open_task_run` |

**Indexes** for the recovery predicates:
- `session_turn (agent_id, id)` exists already and covers the per-agent scan.
- New: `client_tool_call (agent_id, status)`, `sensitive_ask (agent_id, state)`. `background_job (origin_agent, state)` (`idx_bg_job_agent`) and `task_run (state)` exist.

## State transitions and their fence

Every transition below is a **fenced write** (research R2/R3): checked against `owner_fence` in its own transaction when the writing process holds a fence for the scope.

### `session_turn`

```
(sender) accept ──► queued, owner NULL
                        │  receipt: claim (owner_token NULL or < mine → mine; 0 rows = duplicate, dropped)
                        ▼
                    queued, owner = mine ──start──► running ──(steps, note_resume)──► finished (row deleted)
                        ▲                               │
                        └──── recovery re-send ◄────────┘  (when the scope is claimed by a newer owner)
```

- `resume_count`/`steps_at_resume` are kept by the receipt claim (research R7).
- `cancel_session_turns` (`/abort`) is fenced when issued by the owner, unfenced from a gateway (an abort from a person is always allowed).

### `task_run`, `background_job`, `client_tool_call`, `sensitive_ask`

The existing state machines are unchanged. Their guarded compare-and-swap transitions (`running → reporting`, `pending → answered`, …) gain the fence check. Recovery adds one transition to each: **re-own** (`owner_token < new → new`, in the same statement as the recovery outcome: interrupted, abandoned, or picked up).

## Process-local state: `Fences`

Not stored. Held by `SqlStorage` as `Arc<Fences>`:

| Field | Meaning |
|---|---|
| `held: HashMap<Scope, Fence>` | the fence this process obtained by its last claim of each scope; never removed while the process runs |
| `lost: HashMap<Scope, watch::Sender<bool>>` | flipped to `true` the first time a fenced write of the scope is refused |

`Fence { scope, node, token }` is what `claim` returns and what recovery receives.

## Legacy rows

`owner_token IS NULL` on a row that is not an unclaimed accepted turn means the row predates this feature. Recovery predicates (`owner_token IS NULL OR owner_token < ?new`) select it, so the first claim after the upgrade recovers it once and re-owns it. This null-means-legacy reading and the `add_column_if_missing` calls are listed in #82.
