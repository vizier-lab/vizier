# Contract: consultation tools (category `consultations`)

All of these are reached from `execute_python`. Every refusal is a result
`{"status": "<code>", "message": "…"}`, never a raised error (FR-009). Snapshots use the shape in
`data-model.md`.

## `consult_agent`

```json
{ "consultations": [ { "agent": "researcher", "request": "…" } ], "timeout_secs": 600 }
```

- `consultations` takes 1 to 8 entries. `request` is non-empty and at most 16 KiB. `timeout_secs`
  is 1..=3600, default 600.
- It returns `{"consultations": [ {id, agent, state: "running", position, group_size} ]}` at once,
  in the order given.
- Either every entry starts or none does: each one is checked first.
- Refusals: `unknown_agent` (it doesn't exist or the person may not use it, with the same message
  for both), `self`, `in_chain`, `unavailable` (it exists and is allowed, but is failing or
  starting; the message carries the `AgentUnavailable` reason), `no_owner`, `invalid`.

## `wait_consultations`

```json
{ "ids": ["c_…"], "until": "any" }
```

As `wait_subagents`, it returns `{met, consultations: [snapshot]}`. An `s_` id is
`unknown_consultation`.

## `reply_consultation`

```json
{ "id": "c_…", "text": "…" }
```

- `asking` → reply, `answered` → follow-up (`follow_ups + 1`). Both re-check access (FR-013). It
  returns the snapshot, now `running`.
- Refusals: `unknown_consultation`, `not_waiting` (running, failed, timed out or stopped; the
  message names the state), `unknown_agent` (access lost).

## `stop_consultation`

`{ "id" }` returns `{stopped: bool, consultation: snapshot}`. A final row comes back with
`stopped: false`.

## `list_consultations`

`{}` returns this conversation's open consultations, then its last 50 final ones, oldest first.
It marks nothing seen.

## `get_consultation`

`{ "id" }` returns the snapshot plus `conversation` (the consultee's topic) and marks the outcome
seen.

## `list_consultable_agents`

`{}` returns `{"agents": [ {id, name, description, available} ]}`: the agents the person may use,
minus the caller and its chain. `available` is the published health view (unknown counts as
`true`).

## `ask_consulting_agent` (consulted side only)

```json
{ "question": "…" }
```

It is **turn-ending**. The row goes `running → asking`, the question is delivered to A, and the
turn ends. It is offered only in a `Consultation` session.

## Offering

| Context | `consult_agent` … `list_consultable_agents` | `ask_consulting_agent` |
|---|---|---|
| a WebUI or project chat, a scheduled task, a detached script from one of them | yes, if `consultation_depth < max_depth` and `subagent_depth == 0` | no |
| a Discord or Telegram conversation, or a detached script started there | no (deferred until #116) | no |
| a consultation (depth 1, default max 1) | no | yes |
| a subagent worker | no | no |
| a dream | no | no |

## Retired names

| Name | Answer |
|---|---|
| `delegate_agent`, `mention_agent`, `send_message_to_workspace`, `list_workspaces`, `read_workspace_messages` | "Workspaces were removed: consult another agent with consult_agent, then wait_consultations for its answer." |
| `consult_agent` | no longer retired |

## The consultee's request (frontmatter)

```
sender: <consulting agent id>
via: consultation | consultation_reply | consultation_followup
on_behalf_of: <person display name> | none
consultation: 2 of 3
sent_at: …
```

## The consultee's appended system section (`consultation_md`)

```
## Consultation
You are being consulted by the agent <A name> (<A id>). You answer as yourself, with your own
knowledge and tools. Your answer is read by <A name>, not by a person: be compact and direct, say
what you could not do and why, and how sure you are. If you need something from <A name> to do
this, call ask_consulting_agent. You cannot ask the person. Never pass on secret values.
```
