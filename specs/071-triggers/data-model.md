# Data Model: Triggers

## trigger
| Field | Notes |
|---|---|
| `id` | random, global; the webhook address |
| `agent_id`, `slug` | unique together |
| `kind` | trigger kind id |
| `title`, `instruction` | |
| `settings` | JSON; `Secret` paths encrypted |
| `requester` | existing `Requester` |
| `is_active` | |
| `awaiting_approval` | set for every agent-made trigger (FR-036) |
| `delivers_to` | user id resolved from the requester (FR-046) |
| `needs_attention` | reason, e.g. triage model removed |
| `tool_scope` | JSON list, null = all |
| `overlap` | `coalesce` (default) or `drop` |
| `hourly_limit` (20, null = off), `cooldown_secs` (0) | runs; per trigger, either kind |
| `triage` | null = off; criteria, provider, model, calls per hour (200) |
| `destination` | null = inbox only; a conversation reference |
| `summary_hours` (5) | null = off; `last_summary_at` |
| `created_at`, `last_fired_at` | |

## trigger_event
| Field | Notes |
|---|---|
| `id`, `trigger_id`, `agent_id`, `received_at` | |
| `payload` | redacted, capped; null on a collapsed drop record, and after 3 days |
| `status` | `triage_pending`, `unimportant`, `waiting`, `handled`, `ignored`, `dropped`, `refused`, `failed` |
| `attempts` | runs it was given to (at most 2) |
| `summarised_at` | set once it appeared in a summary |
| `reason` | filter, triage reason, overlap, queue full |
| `untriaged` | triage failed, counted as important |
| `count` | events folded into a drop record |
| `run_id` | set when handled |

At most 500 `triage_pending` and 100 `waiting` per trigger.

## trigger_run
| Field | Notes |
|---|---|
| `id`, `trigger_id`, `agent_id` | |
| `started_at`, `finished_at` | cursor `(started_at, id)` |
| `state` | `running`, `answered`, `no_response`, `interrupted` |
| `session_key` | |
| `delivered_at` | set once, claimed before sending |
| `quiet_note` | set when the run ended with nothing to report |
| `owner_node`, `owner_token` | scope `control` |

Unique where `state = 'running'` per trigger. A run takes up to 20 waiting events.

```
event -> ignored | dropped
event -> triage_pending -> unimportant
event -> [triage_pending ->] waiting -> handled (by a run)
run: running -> answered | no_response | interrupted
failed or interrupted run: its events -> waiting (attempts 1) -> failed (attempts 2)
```

## trigger_triage_call
`trigger_id`, `at`: counted for the hourly triage limit.

## trigger_delivery
`(trigger_id, delivery_id)` unique, `event_id`, `expires_at` (24 h or more).

## trigger_work_scope

`work_id` (primary key: a worker, consultation or detached script id), `agent_id`, `scope` (a JSON
list). Written only for work a scoped turn starts, and read when its outcome is delivered, so the
delivery turn keeps the scope (FR-030). Deleted with the agent.

## trigger_monitor
`trigger_id`, `state` (<= 64 KiB), `last_check_at`, `next_check_at`, `last_error`,
`failures`, `failed_at`. Editing or reactivating clears the failure.

Deleting a trigger removes its events, runs, deliveries, triage calls and monitor row. Deleting an agent removes
them all.

## Schema changes elsewhere
- `VizierChannelId::Trigger(slug, fired_at)`, slug `trigger__...`, time serialised in full UTC.
- `VizierRequest.tool_scope`, `trigger` (slug), both `serde(default)`.
- `UsageActivity::Trigger` and `TriggerTriage`; notification category `trigger_run`.
- `ToolCategory::Triggers`, appended.

## Settings by kind
- webhook: secret, optional signature (header, algorithm, secret), optional timestamp check (header,
  tolerance 5 min), filter, delivery id (header or
  JSON field), forwarded headers.
- monitor: script, interval (default 60 s, minimum 5 s), tools the script may call.
