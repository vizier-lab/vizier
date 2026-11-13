# Contract: the `detached` option and the agent's tools

What a model sees and does. Everything here is written into the agent's guidance (`SANDBOX.md`'s sibling text in `system_prompt/boot.rs`, and `execute_python`'s description), not into a skill.

## 1. Calling

Any tool function in a script accepts the reserved keyword `detached`:

```python
a = web_fetch(url=url_a, detached=True)
t = shell_exec(command="make test", detached={"timeout": 900, "group": "checks"})
```

- `True` uses the defaults. A dict may carry `timeout` (seconds, 1..=3600, default 600) and `group` (a label, 1..=64 characters of `[A-Za-z0-9_-]`); any other key is an error naming the key.
- The call returns at once with a **handle**, a dict the script can keep or return:

```json
{ "detached": true, "id": "d_k3x9a1b2", "tool": "web_fetch", "state": "running", "timeout_secs": 600, "group": null }
```

- The tool's own result never reaches the script. It arrives later, as a delivery (§3).
- A detached call is not an error and does not end the script; a script may detach many calls and finish.

## 2. Refusals

A refused call raises a catchable exception, and no row is written:

| Cause | Message (starts with) |
|---|---|
| The tool needs the live turn (a client tool, `ask_question`, `ask_sensitive_input`, any turn-ending tool) | `cannot detach {tool}: it needs this turn` |
| Unattended context (dream cycle, scheduled task) | `cannot detach here: nobody is waiting to be told` |
| Nesting limit | `nesting limit (3) reached` |
| Bad option value | `detached must be true or an object with timeout and group` |
| The tool declares its own `detached` argument | `{tool} takes an argument named detached, so it cannot be detached` |

## 3. Delivery

A final, non-cancelled call is delivered once, as a user message that begins:

```text
[Background work finished]
d_k3x9a1b2 web_fetch  succeeded in 6s
<outcome, cut to 32 KiB, with a marker if cut>
```

A failed or timed-out or interrupted call says so with the reason. A group is one message with every member, once all are final.

**When**: at the start of the agent's next inner round if a turn is running (never into a running script); otherwise as a turn of its own that wakes the conversation. The same text either way. Calls that are final together arrive together.

## 4. Tools

All three are in category `agents`, code-mode only, and are not available to dreams.

| Tool | Input | Result |
|---|---|---|
| `list_detached_calls` | `state?` (`running` \| `done` \| `stopped` \| `all`, default `all`) | `{ "calls": [ {id, tool, state, elapsed_secs, group, delivered} ] }`, oldest first, this conversation only |
| `get_detached_call` | `id` | `{ id, tool, arguments_summary, state, started_at, elapsed_secs, timeout_secs, group, parent_id, delivered, outcome? }` (`outcome` once final); an unknown or other conversation's id is `{ "status": "not_found" }` |
| `cancel_detached_call` | `id` | `{ "status": "cancelled" }`, `{ "status": "already_finished", "state": … }` or `{ "status": "not_found" }`; cancels the call and every call launched from it, and nothing is delivered |

Refusals are `status` results, never errors (the convention of the scratchpad and time tools).

## 5. Retired names

`background_subagents`, `list_background_jobs` and `cancel_background_job` are in `RETIRED_TOOL_NAMES`; calling one raises:

> `background_subagents was replaced: pass detached=True to any tool call, for example parallel_subagents(..., detached=True); see list_detached_calls, get_detached_call and cancel_detached_call.`
