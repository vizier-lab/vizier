# Data Model: Detached scripts

## DetachedCall (table `detached_call`, behind `DetachedCallStorage`)

One row per detached `execute_python` run. Replaces `background_job` and `background_piece`, which are dropped.

| Field | Type | Notes |
|---|---|---|
| `id` | text, primary key | `d_` + 8 lowercase base36 characters, unique across the install. The **handle** the agent receives. |
| `agent_id` | text | The agent whose scope owns the row (`agent:{id}`, the fence scope). |
| `session` | JSON (`VizierSession`) | The conversation the script was launched from, and where its delivery goes. Indexed by its slug. |
| `parent_id` | text, null | The detached script whose sub-agent turn launched this one (nesting), else null. |
| `depth` | integer | `ToolContext.background_depth` of the launching turn; a launch at `MAX_BACKGROUND_DEPTH` (3) is refused. |
| `intent` | text | The `execute_python` `intent` sentence, cut to 500 characters. What people read. |
| `state` | text | See state machine. |
| `timeout_secs` | integer | 1..=3600, default 600. |
| `tool_calls` | JSON | List of `{name, ok, duration_ms}` for the tool calls made so far (no arguments). Written when a call finishes, at most once per second. |
| `created_at`, `finished_at` | epoch ms / null | `finished_at` is set with the final state. |
| `outcome` | text, null | Redacted JSON `ExecutionReport`, or the failure or timeout message, cut to 32 KiB (the cut is marked). Set with the final state; kept until the conversation is deleted. |
| `delivered_at` | epoch ms, null | Set by the delivery claim. Null on a final, non-cancelled row means "waiting to be delivered". |
| `cancelled_by` | JSON (`Canceller`), null | A person or the agent, set only with state `cancelled`. |
| `owner_node`, `owner_token` | text, integer | Row ownership (spec 057): the fence the row was written under. |

**Validation**

- `depth` < 3 at launch; `timeout_secs` in 1..=3600 and only with `detached`.
- Refused at launch with no row: a scheduled-task or dream session.
- `intent`, `tool_calls` and `outcome` pass the turn's `SecretRedactor` before they are stored.

**Indexes**: `(agent_id, state)` (recovery, running list); `(session slug, state, delivered_at)` (the claim and the panel); `(parent_id)` (cancel cascade).

## State machine

```
            ┌────────── cancel (person or agent) ──────────► cancelled   (never delivered)
            │
 running ───┼── script returns ────────────────────────────► succeeded
            ├── script raises / hits a limit / is refused ─► failed
            ├── time limit reached ────────────────────────► timed_out
            └── owner released / restart ──────────────────► interrupted
```

- Every transition out of `running` is one guarded, fenced update (`WHERE id = ? AND state = 'running'`): of two racing causes exactly one wins, which makes "delivered or cancelled, never both" (FR-013) a database fact.
- A final call that is not `cancelled` is **deliverable** until `delivered_at` is set.

## Delivery claim

`claim_deliverable(session) -> Vec<DetachedCall>`: one transaction that selects the session's deliverable rows, sets `delivered_at` on them and returns them in `finished_at` order. Called by the inner loop at a round boundary and by the `DetachedDelivery` handler; whichever runs first takes the rows.

## Views

- **`DetachedCallSnapshot`** (HTTP and `SessionFrame::DetachedCall`): `id`, `intent`, `state`, `created_at`, `finished_at`, `timeout_secs`, `parent_id`, `delivered` (bool), `tool_calls`, and `outcome_excerpt` (500 characters) for a final script. The full outcome is `GET …/{id}` only.
- **Delivery message** (what the model reads): one user message headed `[Background work finished]`, one block per script (`id`, intent, elapsed, state, then the report's `result`, `stdout` and error, or the reason). Rendered by `render_delivery` in `agents/detached/delivery.rs`; the first line is the same in a round-boundary message and a wake-up turn.
- **Handle** (what `execute_python` returns when detached): `{"detached": true, "id", "state": "running", "timeout_secs"}`.

## Request kind

`VizierRequestContent::DetachedDelivery` replaces `BackgroundReport`. It carries no payload (the rows are the payload): the handler claims, and runs a turn only if the claim returned rows. It is non-durable (`is_durable` false) and is re-sent by `recover_agent` for any session with a deliverable row. `VizierRequest` gains an optional `parent_detached_id` (never accepted from clients, `serde(skip)` like `trace`).

## Context

`ToolContext.detached: bool` (default false; true only inside a detached script). With `interactive: false` and `client: None` it is what removes the live-turn tools from the script's catalogue.

## Removed

`BackgroundJob`, `BackgroundPiece`, `BackgroundReport`, `JobKind`, `JobState`, `PieceState`, `ReportEntry`, the `background_job`/`background_piece` tables, and the `background` notification category. `Canceller` is reused.
