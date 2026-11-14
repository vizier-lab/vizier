# Quickstart: verify 060 end to end

Offline, on a `dummyplug` agent (no keys). Run the server with `just run`, create an agent on the
`dummyplug` provider in the WebUI, and open a chat with it. Every message below is sent in that
one chat, in order. Each `{…}` message is the dummyplug §3 JSON request. Its reply is the §4
tool-result echo.

Each script below returns 76,000 characters, about 19,000 estimated tokens.

## 1. Below the mark nothing changes

| Send | Expect |
|---|---|
| `{"tool":"execute_python","arguments":{"code":"'a'*76000"}}` | echo of the report |
| `{"tool":"execute_python","arguments":{"code":"'b'*76000"}}` | echo |
| `{"tool":"execute_python","arguments":{"code":"'c'*76000"}}` | echo |
| `pruned` | `(nothing pruned)`: the three results are about 57,000 tokens, not over the mark |

## 2. Passing the mark prunes the oldest down to the low mark

| Send | Expect |
|---|---|
| `{"tool":"execute_python","arguments":{"code":"'d'*76000"}}` | echo |
| `pruned` | `3 pruned:` with the results of the `a`, `b` and `c` scripts, each about `74.2 KB`. The `d` result is whole (19,000 tokens is under the low mark). Note the first `<record>` as **A** |
| `pruned` | the same three lines: pruning is one-way, and the reply is stable across turns |

## 3. Small outputs are never pruned

| Send | Expect |
|---|---|
| `{"tool":"execute_python","arguments":{"code":"1"}}` | echo |
| `pruned` | still the same three lines |

## 4. Recall

| Send | Expect |
|---|---|
| `{"tool":"execute_python","arguments":{"code":"r = recall_history(id='A')\n(r['status'], len(r['output']['result']))"}}` (put **A** in) | the report's result is `["found", 76000]` |
| `{"tool":"execute_python","arguments":{"code":"[recall_history(id=i)['status'] for i in ['A', 'A', 'no-such-id']]"}}` | `["found", "found", "not_found"]`, with `ok: true` (a miss does not fail the script) |
| In a **second** chat with the same agent, send the first recall script again | `not_found`: recall reads only its own conversation |

## 5. People still see everything

Open the first chat in the WebUI and reload. Every tool step in the trail shows its full output
(76,000 characters for `a`–`d`). Stored history is unchanged.

## 6. Category and reserved name

| Send | Expect |
|---|---|
| `{"tool":"list_tool_functions","arguments":{"category":"history"}}` | lists `recall_history` |
| `{"tool":"list_tool_functions","arguments":{}}` | the category list includes `history` with its description |

Adding an MCP server keyed `history` to the agent lists its tools under `history_mcp`.

## 7. Telemetry (optional)

Run `just jaeger` and `just run-otel`, repeat step 2, and open the `invoke_agent` span of the
second `pruned` turn: `vizier.prune.outputs = 3`, and `vizier.prune.tokens_removed` is about
57,000.

## 8. Unit tests

```sh
cargo test -p vizier-core prune
cargo test -p vizier-core recall_history
cargo test -p vizier-core every_tool_is_in_its_category
```

**Live-provider step (marked: depends on real model output)**: in a project chat on a real model,
ask for a change that reads many files. Afterwards, compare the turn's `usage_event` input tokens
with the same request on `master` (SC-001), and check whether the agent called `recall_history`
on its own.
