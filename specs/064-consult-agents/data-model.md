# Data model: Consulting other agents (and removing Workspaces)

## `subagent` (extended)

This is spec 059's table, with three new columns. A row is a **worker** when `consultee_id` is
null and a **consultation** when it is set (research D1).

| Column | Type | Notes |
|---|---|---|
| `id` | text PK | `s_…` worker, `c_…` consultation |
| `agent_id` | text | the consulting (main) agent; bounds the row's life with `hiring_session` |
| `consultee_id` | text, **new**, nullable | the consulted agent; its turns run the work |
| `principal_user_id` | text, **new**, nullable | the person whose view right admitted it (D5); null for workers |
| `hiring_session` | json | the consulting conversation; receives deliveries |
| `worker_topic` | text | the topic of the worker session: `(agent_id, Subagent, topic)` for a worker, `(consultee_id, Consultation(agent_id), topic)` for a consultation |
| `group_id`, `position`, `group_size` | | one `spawn_subagents` / `consult_agent` call |
| `task` | text | the task, or the consultation's first request |
| `timeout_secs`, `deadline_at`, `remaining_ms` | | as 059; a follow-up sets a new deadline from `timeout_secs` |
| `state` | text | `running \| asking \| answered \| failed \| timed_out \| stopped` |
| `stop_reason` | text | 059's three, plus **new** `consulting_agent_deleted` and `consultee_deleted` |
| `question`, `result`, `result_truncated`, `error` | | as 059; `result` is the latest answer |
| `follow_ups` | integer, **new**, default 0 | how many follow-ups were sent; shown in snapshots |
| `outcome_seq`, `seen_seq`, `delivered_seq` | | as 059 |
| `owner_node`, `owner_token` | | the consulting side's fence (D3) |
| `parent_detached_id`, `created_at`, `updated_at`, `ended_at` | | as 059 |

New index: `(consultee_id)` where not null, for consultee deletion and the consulted side's list.

SQLite: `add_column_if_missing` in `init_subagent_schema`. PostgreSQL: `0013_consultations.sql`.
The new columns are not compatibility code, so they get no #82 entry.

### Transitions

Every transition is one guarded update. The **side** says which agent's fence it checks (D3).

| From | To | Cause | Side |
|---|---|---|---|
| — | `running` | `consult_agent` (open) | consulting |
| `running` | `asking` | `ask_consulting_agent` | consulted |
| `asking` | `running` | `reply_consultation` (reply) | consulting |
| `answered` | `running` | `reply_consultation` (follow-up), **new**, consultations only; `follow_ups + 1`, `outcome_seq` unchanged until the next answer, `ended_at` cleared | consulting |
| `running` | `answered \| failed` | consultee's turn ended | consulted |
| `running` | `timed_out` | timer on the consulting host | consulting |
| `running \| asking` | `stopped(by_agent)` | `stop_consultation` | consulting |
| `running \| asking` | `stopped(conversation_deleted \| consulting_agent_deleted)` | delete on A's side | consulting |
| `running \| asking` | `stopped(consultee_deleted)` | B deleted | consulted |

`is_deliverable` is unchanged. A follow-up's answer increments `outcome_seq` when it settles, so it
is delivered or seen once, like the first answer.

## `VizierChannelId::Consultation(String)` (new)

This holds the consulting agent's id, with the topic UUID in `session.2`.

| Aspect | Value |
|---|---|
| slug | `consultation__{consulting agent}` |
| `kind_of` | `consultation` (listed for `can_edit_agent(consultee)` only) |
| `usage_activity()` | `Consultation` (label **Consultations**), place `consultation:{consulting}:{topic}` |
| interactive | never |
| framing | consultee's own layout + `consultation_md` appended last (D6) |
| durable | yes |

## `VizierRequest` (extended)

| Field | Serde | Notes |
|---|---|---|
| `consultation_depth: u8` | journaled, like `subagent_depth` | 0 for a person's turn; a consultation request is `+1` |
| `consultation_chain: Vec<AgentId>` | journaled | the consulting agents above this turn, oldest first |

`ToolContext` gets `consultation_depth` and `consultation_chain`, copied from the request.

## Seed config

```yaml
vizier:
  consultations:
    max_depth: 1   # a consulted agent cannot consult further
```

## Snapshot (`SessionFrame::Subagent`, HTTP DTO)

This is spec 059's snapshot, plus:

- `consultee: { id, name, available } | null` (null for a worker)
- `follow_ups: number`

## Removed (Workspaces)

These tables are dropped: every `workspace*` table from `init_workspace_schema` / PostgreSQL
`0004_workspaces.sql` and `0006_workspace_shown.sql`.

These rows are deleted: the sessions with a `Workspace` channel and everything keyed on them,
`notification` rows of category `workspace_message`, and the permissions `workspaces:create` and
`all_workspaces:manage` in every role. All of it happens once, under marker
`migration.drop_workspaces_v1` (#82 lists the marker and the cleanup).

Kept: the `VizierChannelId::Workspace(String)` variant (never constructed; old `usage_event` rows
parse, labelled "Workspaces (removed)", #82).
