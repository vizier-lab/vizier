# Data Model: Detached tool calls

## DetachedCall (table `detached_call`, behind `DetachedCallStorage`)

One row per detached tool call. Replaces `background_job` and `background_piece`, which are dropped.

| Field | Type | Notes |
|---|---|---|
| `id` | text, primary key | `d_` + 8 lowercase base36 characters, unique across the install. This is the **handle** the script receives. |
| `agent_id` | text | The agent whose scope owns the row (`agent:{id}`, the fence scope). |
| `session` | JSON (`VizierSession`) | The conversation the call was launched from, and where its delivery goes. Indexed by its slug for list and claim. |
| `parent_id` | text, null | The detached call this one was launched from (nesting), else null. |
| `depth` | integer | `ToolContext.background_depth` of the launching turn; a call launched at `MAX_BACKGROUND_DEPTH` (3) is refused. |
| `tool` | text | Catalogue name of the tool, as the script called it. |
| `arguments` | text | Redacted JSON of the arguments (the reserved `detached` key removed), kept whole for the task, summarised to 500 characters wherever it is shown. |
| `group_id` | text, null | `{script run uuid}/{label}` when the call was started with a `group`, else null. |
| `state` | text | See state machine. |
| `timeout_secs` | integer | 1..=3600, default 600. |
| `created_at`, `finished_at` | epoch ms / null | `finished_at` is set with the final state. |
| `outcome` | text, null | Redacted JSON result, or the error or timeout message, cut to 32 KiB (the cut is marked). Set with the final state; kept until the conversation is deleted. |
| `delivered_at` | epoch ms, null | Set by the delivery claim. Null on a final, non-cancelled call means "waiting to be delivered". |
| `cancelled_by` | JSON (`Canceller`), null | A person or the agent, set only with state `cancelled`. |
| `owner_node`, `owner_token` | text, integer | Row ownership (spec 057): the fence the row was written under. |

**Validation**

- `depth` < 3 at launch; `timeout_secs` in 1..=3600; `group` a label of 1..=64 characters of `[A-Za-z0-9_-]`.
- A call to a tool whose entry is not `detachable` is refused before any row exists.
- `outcome` and `arguments` pass the turn's `SecretRedactor` before they are stored.

**Indexes**: `(agent_id, state)` (recovery, running list); `(session slug, state, delivered_at)` (the claim and the tray); `(parent_id)` (cancel cascade).

## State machine

```
            ┌────────── cancel (person or agent) ──────────► cancelled   (never delivered)
            │
 running ───┼── tool returns ok ───────────────────────────► succeeded
            ├── tool errors ───────────────────────────────► failed
            ├── limit reached ─────────────────────────────► timed_out
            └── owner released / restart ──────────────────► interrupted
```

- Every transition out of `running` is one guarded, fenced update (`WHERE id = ? AND state = 'running'`): of two racing causes exactly one wins, which is what makes "delivered or cancelled, never both" (FR-014) a database fact and not a convention.
- A final call that is not `cancelled` is **deliverable** until `delivered_at` is set. `cancelled` rows are never deliverable.
- `delivered_at` is set only by the delivery claim, inside the same fenced transaction that reads the rows.

## Delivery claim

`claim_deliverable(session) -> Vec<DetachedCall>`: one transaction that selects the deliverable rows of the session, drops any whose `group_id` has a member still `running`, sets `delivered_at` on the rest, and returns them in `finished_at` order. Called by the inner loop at a round boundary and by the `DetachedDelivery` handler; whichever runs first takes the rows.

## Views

- **`DetachedCallSnapshot`** (HTTP and `SessionFrame::DetachedCall`): `id`, `tool`, `arguments_summary`, `state`, `created_at`, `finished_at`, `timeout_secs`, `group` (the label only), `parent_id`, `delivered` (bool), and `outcome_excerpt` (500 characters) for a final call. The full outcome is `GET …/{id}` only.
- **Delivery message** (what the model reads): one user message headed `[Background work finished]`, one block per call (`id`, tool, elapsed, state, outcome or reason), grouped calls under their label. Rendered by `render_delivery` in `agents/detached/delivery.rs`; its first line is the same in a round-boundary message and a wake-up turn, so the agent's guidance describes one thing.

## Request kind

`VizierRequestContent::DetachedDelivery` replaces `BackgroundReport`. It carries no payload (the rows are the payload): the handler claims, and runs a turn only if the claim returned rows. It is non-durable (`is_durable` false) and is re-sent by `recover_agent` for any session with a deliverable row.

## Removed

`BackgroundJob`, `BackgroundPiece`, `BackgroundReport`, `JobKind`, `JobState`, `PieceState`, `ReportEntry`, `Canceller` stays (reused), the `background_job`/`background_piece` tables, and the `background` notification category.
