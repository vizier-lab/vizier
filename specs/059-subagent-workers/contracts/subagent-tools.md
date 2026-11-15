# Contract: the `subagents` tools

**Feature**: `059-subagent-workers`

All tools are code-mode functions (called from `execute_python`). Every refusal is a
`{"status": "<code>", "message": "…"}` result, never an exception, so a script can branch on it.
A worker summary is:

```json
{"id": "s_…", "position": 2, "group_size": 4, "task": "…(≤200 chars)",
 "state": "running|asking|answered|failed|timed_out|stopped",
 "stop_reason": null, "question": null, "result": null, "truncated": false,
 "error": null, "started_at": "…", "ended_at": null, "topic": "<worker topic>"}
```

## Availability

| Tool | Offered when |
|---|---|
| `spawn_subagents`, `wait_subagents`, `reply_subagent`, `stop_subagent`, `list_subagents`, `get_subagent` | `ctx.subagent_depth < subagents.max_depth` (default: main agent only) and not a dream session |
| `ask_main_agent` | the turn is a worker's (`Subagent` session) |

A tool that is not offered is absent from the catalogue, the category line and a script's reach.
`ask_question` and `ask_sensitive_input` stay unavailable in worker turns (not interactive).

## `spawn_subagents`

Input: `{"tasks": [{"task": "…"}], "timeout_secs": 600}` — one or more tasks (no new upper bound,
as today), each non-empty and ≤ 16 KiB;
`timeout_secs` 1..=3600, per worker.

Output: `{"group": "g_…", "subagents": [<summary>, …]}` in the order given, all `running`. Returns
without waiting.

Refusals: `invalid_input` (no tasks, empty task, too long, bad timeout).

## `wait_subagents`

Input: `{"ids": ["s_…"], "until": "any" | "all"}` (default `all`).

Output: `{"met": true|false, "subagents": [<summary>, …]}`. Returns when `any`: at least one given
worker is settled or asking; `all`: every given worker is. Otherwise returns at the call's own
deadline with `met: false`; workers are not touched. Every settled or asking outcome returned is
marked seen (not delivered later).

Refusals: `unknown_subagent` (id not found, or not spawned in this conversation), `invalid_input`.

## `reply_subagent`

Input: `{"id": "s_…", "text": "…"}` (non-empty, ≤ 16 KiB).

Output: `{"subagent": <summary>}` with state `running`. The text reaches the worker as a
`SubagentReply` in its own conversation.

Refusals: `unknown_subagent`, `not_asking` (message names the current state).

## `stop_subagent`

Input: `{"id": "s_…"}`. Output: `{"subagent": <summary>}`, `stopped` / `by_agent`. A settled worker
returns its summary unchanged with `status: "already_finished"`. Refusals: `unknown_subagent`.

## `list_subagents`

Input: `{}`. Output: `{"subagents": [<summary>, …]}` — running and asking first, then the last 50
settled, newest last. Does not mark anything seen.

## `get_subagent`

Input: `{"id": "s_…"}`. Output: `{"subagent": <summary with full task, result up to the limit>}`;
marks a settled or asking outcome seen. Refusals: `unknown_subagent`.

## `ask_main_agent` (worker only, turn-ending)

Input: `{"question": "…"}` (non-empty, ≤ 4 KiB). On success the worker's turn ends; the worker's
state is `asking` and the question is on its row. Output to the worker script (seen only if the
turn did not end): `{"status": "asked"}`.

## Retired names

| Name | Guidance |
|---|---|
| `parallel_subagents` | "parallel_subagents was retired: call spawn_subagents, then wait_subagents(ids, until='all') (category subagents)." |
| `paralel_subtasks` | same text, naming the old name |
| `background_subagents`, `list_background_jobs`, `cancel_background_job` | "…run execute_python with detached=true for background work; for workers use spawn_subagents (category subagents)." |
