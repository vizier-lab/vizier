# Contract: Agent Tools

Category `workspaces`. Called from `execute_python`, like every tool. Refusals are `status` results a script can handle, never `Err`, and post nothing (FR-017, FR-030).

## `mention_agent`

Available only during a turn in a `Workspace` session.

```json
{ "agents": ["archivist", "scribe"], "message": "have we seen this lock before?" }
```

Posts `@archivist @scribe have we seen this lock before?` to the current topic, unless the message already contains those mentions. A message starting with a skill puts them after it: `"/status-note outage"` posts `/status-note @archivist @scribe outage`. The result of posting is the same as if the text had been written (FR-014).

| `status` | When | Extra |
|---|---|---|
| `posted` | Posted | `seq`, `woken`, `unavailable` |
| `not_a_member` | Any named agent isn't a member. Nothing posts | `members`: agents that can be mentioned |
| `self_mention` | The caller named itself. Nothing posts | |
If any named agent is down it still posts, and those agents are listed in `unavailable` (the topic shows a note for each). Nothing is posted only for `not_a_member` and `self_mention`.

## `send_message_to_workspace`

Available in every conversation except a `Dream` or `Subagent` session.

```json
{ "workspace_name": "ops", "workspace_topic": "daily-incidents", "message": "@pathfinder pull last night's failed deploys" }
```

| `status` | When | Extra |
|---|---|---|
| `posted` | Posted | `seq`, `woken`, `unavailable`, `topic_created` (bool) |
| `not_found` | No such Workspace, **or the caller isn't a member** (identical) | |
| `invalid_name` | The topic name breaks the rule, so one can't be created | `rule` |

Origin is set from the caller's own session: a scheduled task's slug, or the kind of channel. A message from one Workspace to another has origin `workspace` and its name. Called from the current topic it behaves like a plain post, with no origin.

No return path: the conversation that sent it is not woken (spec Assumptions).

## `list_workspaces`

Same availability. No arguments.

```json
{ "workspaces": [{ "name": "ops", "topics": ["daily-incidents", "general"], "members": { "agents": ["pathfinder"], "people": ["rafi"] } }] }
```

An agent in no Workspace gets an empty list.

## `read_workspace_messages`

Same availability.

```json
{ "workspace_name": "ops", "workspace_topic": "daily-incidents", "limit": 50, "before_seq": null }
```

`limit` defaults to 50, max 200. Returns text entries only, oldest first, with `seq`, `author`, `body`, `at`, and `next_before_seq`. Status `not_found` is as above; `topic_not_found` for a missing topic, which is not created (FR-031). The result is not added to the read position.

## Other tools in a Workspace

| Tool | In a Workspace turn |
|---|---|
| `ask_question` | Offered. Any person who is a member may answer |
| `ask_sensitive_input` | Not offered |
| Client tools, generative UI | Absent, because no client binding is ever set |
| `background_subagents`, `parallel_subagents` | Offered. Pieces get none of the `workspaces` tools. A report wakes the turn in the same session |

## Retired

`consult_agent` and `delegate_agent` are removed and added to `RETIRED_TOOL_NAMES`:

> `consult_agent` and `delegate_agent` were replaced: use `mention_agent` inside a Workspace, and `send_message_to_workspace` outside one. Both agents must be members.
