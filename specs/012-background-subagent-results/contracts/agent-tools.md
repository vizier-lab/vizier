# Contract: Agent Tools

**Feature**: `012-background-subagent-results`

These are the tool definitions the model sees. Tool names are unchanged, because they are the dispatch key.

## `paralel_subtasks`

**Description** (verbatim):

> Run several independent tasks in parallel, in the background. This call returns immediately with a job id; it does NOT return the results. When every task has finished, you will receive one message in this same conversation, marked as a background report, listing each task's result in the order given. Do not wait, poll, or claim the work is done before that report arrives. Tell the person what you have started if they are waiting.

**Input**:

```json
{
  "tasks": [{ "prompt": "string" }],
  "timeout_secs": 600
}
```

| Field | Required | Rule |
|---|---|---|
| `tasks` | yes | at least 1 entry; each `prompt` non-empty |
| `timeout_secs` | no | 1–3600, default 600. The time limit applies to each task separately |

**Output** (string):

```text
Started background batch b-7f3a9c with 3 tasks. Results will arrive as a background report in this conversation.
```

**Errors** (the tool call fails, no job is created):

- `tasks must not be empty`
- `timeout_secs must be between 1 and 3600`
- `background nesting limit (3) reached: this turn was itself started by a background result`

## `delegate_agent`

**Description** (verbatim, followed by the existing "Available Agent" list):

> Hand a task to another agent, in the background. This call returns immediately with a job id. When the other agent has answered, you will receive its answer as a background report in this same conversation. Do not wait or poll for it.

**Input**:

```json
{ "agent_id": "string", "prompt": "string", "timeout_secs": 600 }
```

`timeout_secs` follows the same rule as above.

**Output** (string):

```text
Delegated to agent 'archivist' as background job b-91c2e0. Its answer will arrive as a background report in this conversation.
```

**Errors**:

- `agent 'x' not found or not running`
- The `timeout_secs` error, as above.
- The nesting-limit error, as above.

## `list_background_jobs`

**Description** (verbatim):

> List the background jobs you have running (from paralel_subtasks or delegate_agent), across all your conversations, with their progress. Use it to find a job id to cancel.

**Input**: `{}`

**Output** (string; `No background jobs running.` when there are none):

```text
2 background jobs running:

b-7f3a9c · batch · from webui topic "Trip planning" · 1/3 done · 4m 10s
  1. answered — Research the history of the Silk Road
  2. running  — Summarise current shipping costs
  3. running  — Find three academic sources
b-91c2e0 · delegation to archivist · from discord channel 1234 · 0m 34s
  1. running  — Archive last week's notes
```

The list covers only jobs this agent launched, in the `running` or `reporting` state.

## `cancel_background_job`

**Description** (verbatim):

> Cancel one of your running background jobs. Its unfinished pieces are stopped, any background work they started is cancelled too, and no report will arrive for it. Returns the results of the pieces that had already finished.

**Input**:

```json
{ "job_id": "string", "reason": "string (optional)" }
```

**Output** (string):

```text
Cancelled background batch b-7f3a9c (reason: user asked to stop).

## 1. answered — Research the history of the Silk Road
The Silk Road was a network of…

## 2. cancelled — Summarise current shipping costs

## 3. cancelled — Find three academic sources
```

The entries use the same rendering as a report (`background-report.md`), including truncation. Nested jobs that were cancelled as a cascade are listed in a final line: `Also cancelled nested jobs: b-…, b-…`.

**Results that are not errors** (returned as text, nothing changed):

- `Background job b-… already finished (reported).` The state shown is `reported`, `undelivered`, `cancelled` or `interrupted`.

**Errors**:

- `no running job b-… launched by you`: the id is unknown, or the job was launched by another agent. The two cases are indistinguishable on purpose.

## `consult_agent`

This tool is unchanged: it is still blocking and returns the other agent's answer.

## Python sandbox

Called through `execute_python`, all four tools behave the same way and return their strings. A report wakes the conversation that owns the script's turn.

## Registration

All four tools are in `default_toolset`. None of them is in `DREAM_TOOL_NAMES`.
