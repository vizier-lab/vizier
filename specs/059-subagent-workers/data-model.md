# Data model: Subagents as temporary workers

**Feature**: `059-subagent-workers` | [plan.md](plan.md) | [research.md](research.md)

## `subagent` (new table)

One row per worker. Written only through `SubagentStorage`; every write that advances a worker is
fenced on `agent:{agent_id}` (spec 057) and every state change is one guarded `UPDATE … WHERE
state IN (…)`.

| Column | Type | Notes |
|---|---|---|
| `id` | text PK | `s_` + 8 base36 chars, as `d_` ids |
| `agent_id` | text | the main agent (also the worker's agent) |
| `hiring_channel` / `hiring_topic` | text / text null | the conversation it was spawned in (FR-001 lifetime, deliveries), stored like `detached_script`'s session |
| `parent_detached_id` | text null | set when spawned inside a detached script |
| `group_id` | text | `g_` + 8 base36, one per `spawn_subagents` call |
| `position` | integer | 1-based within the group |
| `group_size` | integer | tasks in the group |
| `worker_topic` | text | the worker's own `Subagent` topic (UUID) |
| `task` | text | the task text as given (≤ 16 KiB, refused above) |
| `timeout_secs` | integer | 1..=3600, default 600 |
| `state` | text | see state machine |
| `stop_reason` | text null | set only with `stopped` |
| `question` | text null | the open question while `asking`; kept after a reply as the last one |
| `result` | text null | the last answer text, cut at the existing limit |
| `result_truncated` | integer | 0/1 |
| `error` | text null | why it failed or timed out |
| `outcome_seq` | integer | +1 on every settle and every ask |
| `seen_seq` | integer | highest outcome returned to the main agent by `wait`/`get` |
| `delivered_seq` | integer | highest outcome delivered into the hiring conversation |
| `deadline_at` | integer null | ms; only while `running` |
| `remaining_ms` | integer null | ms of time limit left; set on entering `asking` |
| `created_at` / `updated_at` / `ended_at` | integer | ms; `ended_at` on a final state |
| `owner_node` / `owner_token` | text / integer | spec 057 |

Indexes: `(agent_id, hiring_channel, hiring_topic, created_at)`; `(agent_id, state)`; `(worker_topic)` unique.

The spawning turn's id is not stored: tools never see it, and nothing reads it (implementation
note, 2026-11-14).

Text timestamps are not used, so `time::stored` does not apply.

## State machine

```
            spawn
              │
              ▼
   ┌──────► running ───────────────► answered   (turn ended with a reply)
   │          │  │ ╲────────────────► failed     (turn errored or panicked)
   │   reply  │  │  ╲───────────────► timed_out  (deadline_at passed)
   │          │  ▼
   └────── asking                     stopped    (from running or asking:
              │                                    by_agent | conversation_deleted | agent_deleted)
              └────────────────────►
```

- `answered`, `failed`, `timed_out`, `stopped` are final.
- `asking → running` only by `reply_subagent`.
- An agent restart moves nothing: a `running` worker's `session_turn` is resumed; an `asking`
  worker keeps waiting.

## Deliverability

```
deliverable(row) := state ∈ {answered, failed, timed_out, asking}
                    ∧ max(seen_seq, delivered_seq) < outcome_seq
```

- **Claim** (one transaction, fenced): for the hiring session's deliverable rows,
  `UPDATE … SET delivered_seq = outcome_seq WHERE id = ? AND <still deliverable>`; returns the rows
  whose update applied, oldest `updated_at` first.
- **Seen**: `wait_subagents` and `get_subagent` set `seen_seq = outcome_seq` on every settled or
  asking row they return, with the same guard; whichever of a claim or a read applies first owns the
  outcome.

## `SubagentStorage` (new trait, member of `VizierStorageProvider`)

| Method | Fenced | Does |
|---|---|---|
| `open_subagents(rows)` | yes | insert a group in one transaction |
| `ask_subagent(id, question, remaining_ms)` | yes | `running → asking`, `outcome_seq+1`, clear deadline |
| `reply_subagent(id, deadline_at)` | yes | `asking → running`, set deadline; false if not asking |
| `settle_subagent(id, state, result/error)` | yes | `running → answered/failed/timed_out`, `outcome_seq+1`, `ended_at` |
| `stop_subagent(id, reason)` | yes | `running|asking → stopped`; returns the prior state or `AlreadyFinished` |
| `mark_subagents_seen(ids)` | yes | `seen_seq = outcome_seq` where deliverable |
| `claim_subagent_deliveries(session)` | yes | the claim above |
| `subagent_by_worker(topic)` | no | the row for a worker conversation (used at settle) |
| `get_subagent(id)` / `list_subagents(session, final_limit)` | no | reads |
| `reown_running_subagents(agent, below)` | yes | re-own `running` rows under an older fence, to re-arm their timers |
| `subagent_delivery_sessions(agent)` | no | hiring sessions with a deliverable row |
| `stop_session_subagents(session, reason)` | yes | stop all unfinished, return their worker sessions |
| `delete_session_subagents(session)` / `delete_agent_subagents(agent)` | no | deletion paths |

## Schema additions outside the table

- `VizierRequestContent::SubagentTask(SubagentBrief { subagent_id, position, group_size, text })`
  and `VizierRequestContent::SubagentReply(SubagentReplyText { subagent_id, text })`; kinds `subagent_task`,
  `subagent_reply`. Both `records_session_activity`, both durable.
- `VizierRequest.subagent_depth: u8` (`#[serde(default)]`, on the wire); `ToolContext.subagent_depth`.
- `SessionFrame::Subagent(SubagentSnapshot)` (socket name `subagent`): the tool summary (task cut
  at 200 characters, result at 500) plus `group`, `parent_detached_id`, `updated_at` — the peek
  spec's input.
- `VizierConfig.subagents: SubagentsConfig { max_depth: u8 }`, default `1`.
- `ToolCategory::Subagents`, name `subagents`.
