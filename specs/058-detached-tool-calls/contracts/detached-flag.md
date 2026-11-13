# Contract: `execute_python` detached, and the agent's tools

What a model sees and does. All of it is written into the agent's guidance (`system_prompt/boot.rs` and `execute_python`'s description), not into a skill.

## 1. Calling

`execute_python` gains two optional fields:

| Field | Type | Meaning |
|---|---|---|
| `detached` | boolean, default `false` | Run the script in the background and return a handle at once. |
| `timeout_secs` | integer 1..=3600, default 600 | The whole script's time limit, tool and sub-agent waits included. Only with `detached`; without it, an error. |

```json
{ "intent": "check both vendor pages and run the tests", "detached": true,
  "code": "a = web_fetch(url=url_a)\nb = web_fetch(url=url_b)\nt = shell_exec(command='make test')\n{'a': a, 'b': b, 't': t}" }
```

Returned at once:

```json
{ "detached": true, "id": "d_k3x9a1b2", "state": "running", "timeout_secs": 600 }
```

The script's own result never comes back in this reply. It arrives later as a delivery (§3).

To get several things back together, put them in one script, as above. To have several things run at once and arrive separately, run several detached scripts.

## 2. Inside a detached script and refusals

The script is the same Python as an inline one with the same tools and the same limits except:

- **Not available**: the tools that need the live turn: client tools (the WebUI's `ui_render` and the like), `ask_question`, `ask_sensitive_input` and any turn-ending tool. They are absent from `list_tools()`; calling one is a `NameError`.
- **Own limit**: `timeout_secs` replaces the agent's tool timeout for this script.

A launch is refused (the `execute_python` reply is an error, no handle) when:

| Cause | Message (starts with) |
|---|---|
| Scheduled-task or dream session | `cannot detach here: nobody is waiting to be told` |
| Nesting limit reached | `nesting limit (3) reached` |
| `timeout_secs` without `detached`, or out of range | `timeout_secs must be between 1 and 3600, and needs detached` |

## 3. Delivery

A final, non-cancelled script is delivered once, as a user message that begins:

```text
[Background work finished]
d_k3x9a1b2  succeeded in 41s  — check both vendor pages and run the tests
result: {...}
stdout: ...
```

A failed script gives its exception and traceback; a timed-out one says so; an interrupted one says the server restarted. Files the script produced are session files, named in the message. The result is cut to 32 KiB with a marker if longer.

**When**: at the start of the agent's next inner round if a turn is running (never into a running script); otherwise as a turn of its own that wakes the conversation. The same text either way. Scripts that are final together arrive together.

## 4. Tools

All three are in category `agents`, code-mode only, and are not available to dreams.

| Tool | Input | Result |
|---|---|---|
| `list_detached_scripts` | `state?` (`running` \| `done` \| `stopped` \| `all`, default `all`) | `{ "scripts": [ {id, intent, state, elapsed_secs, delivered} ] }`, oldest first, this conversation only |
| `get_detached_script` | `id` | `{ id, intent, state, started_at, elapsed_secs, timeout_secs, parent_id, delivered, tool_calls: [{name, ok, duration_ms}], outcome? }` (`outcome` once final); an unknown or other conversation's id is `{ "status": "not_found" }` |
| `cancel_detached_script` | `id` | `{ "status": "cancelled" }`, `{ "status": "already_finished", "state": … }` or `{ "status": "not_found" }`; stops the script and what it is waiting on and cancels scripts launched from it; nothing is delivered |

Refusals are `status` results, never errors (the convention of the scratchpad and time tools).

`tool_calls` here is deliberately the slim form (`name`, `ok`, `duration_ms`): the agent wrote the script and has its arguments in its own history, and the slim form keeps the reply small. The stored row and the HTTP detail keep the fuller `ToolInvocationRecord` (with `seq` and redacted argument previews) that the panel draws.

Only the agent cancels in this feature; cancelling from the WebUI is deferred (contracts/http-and-frames.md, "Deferred").

## 5. Retired names

`background_subagents`, `list_background_jobs` and `cancel_background_job` are in `RETIRED_TOOL_NAMES`; calling one raises:

> `background_subagents was replaced: run execute_python with detached=true and call parallel_subagents (or any tool) inside it; see list_detached_scripts, get_detached_script and cancel_detached_script.`
