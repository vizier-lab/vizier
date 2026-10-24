# Data Model: Blocking and Background Parallel Subagents

**Feature**: `025-split-parallel-jobs`

No storage change. There are no new tables or columns and no migration. Background batch jobs keep `JobKind::Batch` and their existing rows. A blocking run is never stored as a job (research R2).

## In-process types

### `SubagentsArgs` (tool input, shared by both tools)

| Field | Type | Rule |
|-------|------|------|
| `tasks` | `[{prompt: string}]` | At least one entry. Every `prompt` is non-empty after trimming. Same as `paralel_subtasks` today. |
| `timeout_secs` | `u64?` | 1..=3600, default 600 (`validate_timeout`). Applies to each task. |

### `SubagentResult` (the `parallel_subagents` output, one per task)

| Field | Type | Meaning |
|-------|------|---------|
| `task` | `u32` | 0-based position in `tasks`; entries are in this order, one per task |
| `state` | `PieceState` | `answered`, `failed` or `timed_out`, never `running`/`cancelled`/`interrupted` |
| `text` | `string` | The answer, or why there is none; cut at `REPORT_TEXT_LIMIT` |
| `truncated` | `bool` | Whether `text` was cut |
| `topic` | `string` | The piece's `Subagent` conversation topic, where its full history and answer are kept |

The output is `{"results": [SubagentResult, …]}`.

### `ToolContext.deadline` (new field)

`Option<tokio::time::Instant>`. The agent loop sets it on each direct tool call to the moment `tools.timeout` will cut that call off. Calls made from a script share their `execute_python` call's deadline. It is `None` wherever no deadline is set (dream calls through `dream_call`, tests), and then only the per-task limit applies.

### `VizierRequest.transient` (new field)

`bool`, `#[serde(skip)]`, default `false`. If true, `is_durable` is `false`, so the request is never journaled as a `session_turn`, never resumed, and never re-sent at startup. Only blocking pieces set it.

## State of one blocking piece

```text
dispatched ──first terminal frame──▶ answered | failed
     │
     ├──per-task limit or deadline──▶ timed_out   (abort sent)
     ├──dispatch error──────────────▶ failed      (reason = the send error)
     ├──channel closed, no frame────▶ failed      ("the agent was restarted")
     └──call dropped (turn stopped)─▶ (nothing returned; abort sent by the guard)
```

## Nesting

Each piece request has `background_depth = ctx.background_depth + 1`. Both tools refuse when `ctx.background_depth >= MAX_BACKGROUND_DEPTH` (3), using the shared `check_depth`.
