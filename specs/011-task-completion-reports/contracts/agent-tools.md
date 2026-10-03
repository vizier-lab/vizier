# Contract: Agent Tools

**Feature**: `011-task-completion-reports`

Two tools added to `default_toolset` in `VizierTools::new()`, implemented in
`src/agents/tools/scheduler/mod.rs` alongside the five that are already there. Both go into
`VizierTools::DREAM_TOOL_NAMES` (FR-026).

Naming follows the family's own convention — `list_task` / `get_task_detail`
(`tools/scheduler/mod.rs:183,253`) — and the split follows the memory tools' list-then-fetch
idiom, where `memory_search` returns addressed results and `memory_read` returns one whole thing
by address. Same reason in both places: one call must not be able to spend the whole context.

---

## `list_task_runs`

> List past runs of one of your own tasks, newest first. Returns when each ran and how it ended,
> not what it said — use `get_task_run_detail` with a run's address to read its report.

**Input**

| Field | Type | Notes |
|---|---|---|
| `slug` | string | one of this agent's tasks |
| `limit` | integer? | default 10, capped at 50 |
| `before` | string? | RFC3339 `run_id` of the oldest run already seen, to page further back |

**Output**

```json
{
  "runs": [
    { "run_id": "2026-10-04T09:00:00Z", "ran_at": "2026-10-04T09:00:00Z", "state": "answered" },
    { "run_id": "2026-10-03T09:00:00Z", "ran_at": "2026-10-03T09:00:00Z", "state": "no_response" }
  ],
  "has_more": true
}
```

**No response text, no previews** (FR-020). A `no_response` run is listed, not omitted — a task
that has been failing for a week must be visible as such (FR-024).

---

## `get_task_run_detail`

> Read what one run of your own task reported. Takes a run address from `list_task_runs`.

**Input**: `slug`, `run_id` (RFC3339, as listed).

**Output**

```json
{
  "run_id": "2026-10-04T09:00:00Z",
  "ran_at": "2026-10-04T09:00:00Z",
  "state": "answered",
  "response": "Posted to #eng. 4 PRs merged since yesterday. One needed a note: #412 …",
  "truncated": false
}
```

- `response` is `null` when the state is not `answered`.
- Truncated past a byte budget, with `truncated: true` saying so (FR-023) — an agent reading its
  own history must not be able to crowd out its current work.
- **Never the trail.** No reasoning, no tool calls, no tool results (FR-022). The agent's own past
  reasoning is the bulk of the text and the least use to it.

---

## Scoping

Both tools are constructed with the owning `agent_id`, like every other tool in this module, and
query by it. An agent cannot name another agent's task because it cannot address one (FR-025) —
the scope is structural, not a check that could be forgotten.

---

## Dream availability

Both names join `DREAM_TOOL_NAMES` (`tools/mod.rs:376`), where four of the five existing scheduler
tools already sit. This is what lets a task's outcome reach memory at all:
`is_non_user_channel` excludes `task__` from the sessions the dream cycle reflects on
(`storage/sqlite/history.rs:63`), so without these tools scheduled work is the one category of an
agent's activity it can never learn from.

Paired with the `list_task` the dream set already has, the cycle can walk from "what tasks do I
have" to "how have they been going".

**Known limitation, not addressed here**: the cycle early-returns when no conversations occurred
in the window (`scheduler/dream/mod.rs:37`), so an agent that only runs tasks never dreams and
would never reach these tools. Pre-existing gating.
