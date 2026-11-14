# Quickstart: verify 061 end to end

Offline, on a `dummyplug` agent (no keys). Run the server with `just run`, create an agent on the
`dummyplug` provider in the WebUI, and open a chat with it. Send the messages in order in one chat
unless a step says otherwise. Each `{…}` message is the dummyplug §3 JSON request; its reply is the
§4 tool-result echo. `S(code)` below means
`{"tool":"execute_python","arguments":{"intent":"test","code":"<code>"}}` with the code
JSON-escaped.

Steps 1–7 should also be automated as `cargo test -p vizier-core --test dummyplug_e2e custom_tools`
(the same harness 060 uses).

## 0. Before any custom tool

| Send | Expect |
|---|---|
| `context` | the echoed `SANDBOX.md` category list has `custom_tools` and no custom category lines |
| `{"tool":"list_tool_functions","arguments":{"category":"custom_tools"}}` | the ten authoring tools |

## 1. Create (Story 1)

| Send | Expect |
|---|---|
| `S(create_custom_tool(name="echo_sum", description="Add two numbers", category="maths", category_description="Small arithmetic helpers", input_schema={"type":"object","properties":{"a":{"type":"integer"},"b":{"type":"integer"}},"required":["a","b"]}, script="input['a'] + input['b']"))` | `status: "created"`, `call_as: "echo_sum"`, `version: 1`, `category_created: true` |
| `S(create_custom_tool(name="bad", description="x", input_schema={"type":"string"}, script="memory_serch(query=1"))` | `status: "invalid"` with three problems: `schema_not_object`, `script_syntax` (line/column), and — once the syntax is fixed — `unknown_tool` with `did_you_mean: ["memory_search"]` |
| `S(create_custom_tool(name="echo_sum", description="dup", input_schema={"type":"object"}, script="1"))` | `invalid`, `name_taken` |
| `S(create_custom_tool(name="current_time", description="x", input_schema={"type":"object"}, script="1"))` | `invalid`, `name_taken` naming the built-in `time` category as its owner |
| `S(create_custom_tool(name="nocat", description="x", input_schema={"type":"object"}, script="1", category="newcat"))` | `invalid`, `category_description_required` |
| `S(create_custom_tool(name="loose", description="x", input_schema={"type":"object"}, script="42"))` | created in category `custom` |
| `S(create_custom_tool(name="with_sample", description="x", input_schema={"type":"object","properties":{"n":{"type":"integer"}}}, script="input['n'] * 2", sample_input={"n": 3}))` | created; `sample_run: {ok: true, result: 6, took_effect: true}` |
| `S(create_custom_tool(name="memory", description="x", input_schema={"type":"object"}, script="1", category="memory", category_description="clash"))` | created; `category_renamed_to: "memory_custom"` |

## 2. Call in the same turn and later (Story 2, SC-001)

| Send | Expect |
|---|---|
| `S(echo_sum(a=2, b=3))` | result `5`; the report's tool call `echo_sum` has `calls: []` |
| `S(echo_sum(a="2", b=3))` | script fails with `RuntimeError` `echo_sum: invalid input: $.a: expected integer, got string`; nothing ran |
| `S(create_custom_tool(name="fails", description="x", input_schema={"type":"object"}, script="current_time()\nmemory_read(bundle='nope', path='nope')['missing']"))` then `S(fails())` | failure names `fails`, the failing step and lists `current_time ok` before it |
| `context` | category lines now include `maths: Small arithmetic helpers`, `memory_custom: clash` and `custom: …`, after the MCP/client lines |
| `{"tool":"list_tool_functions","arguments":{"category":"maths"}}` | `echo_sum` |
| `{"tool":"describe_tool_function","arguments":{"name":"echo_sum"}}` | usual doc + `custom: {version: 1, author: "you", model_backed: false, needs_attention: null}` |
| In a **second chat**, `S(echo_sum(a=1, b=1))` | `2` |

## 3. Runs inside the calling turn (Story 3)

| Send | Expect |
|---|---|
| `S(create_custom_tool(name="ask", description="x", input_schema={"type":"object"}, script="ask_question(questions=[{'kind':'confirm','text':'ok?'}])"))` | created (no security refusal in this version) |
| Schedule a one-time task in a minute whose prompt is the JSON `S(ask())`; read the task run | the inner `ask_question` gets the same "not offered here" answer a direct call gets in an unattended turn |
| `S(create_custom_tool(name="slow", description="x", input_schema={"type":"object"}, script="while True:\n  pass"))` then `S(slow())` | ends at the caller's time limit with a timeout naming `slow` |

## 4. Versions, diff, rollback (Story 4)

| Send | Expect |
|---|---|
| `S(update_custom_tool(name="echo_sum", script="input['a'] + input['b'] + 0", reason="tidy"))` | `version: 2` |
| `S(update_custom_tool(name="echo_sum", description="Add two integers"))` | `version: 3` |
| `S(diff_custom_tool(name="echo_sum", **{"from": 1, "to": 3}))` | hunks for description and script |
| `S(rollback_custom_tool(name="echo_sum", to_version=1))` | `version: 4`, `restored_from: 1` |
| `S(update_custom_tool(name="echo_sum", script="input[", expected_version=4))` | `invalid`; `S(get_custom_tool(name="echo_sum"))` still shows version 4 |
| `S(update_custom_tool(name="echo_sum", script="1", expected_version=2))` | `out_of_date`, `current_version: 4` |

## 5. Composition and cycles (Story 6)

| Send | Expect |
|---|---|
| `S(create_custom_tool(name="double_sum", description="x", input_schema={"type":"object","properties":{"a":{"type":"integer"},"b":{"type":"integer"}}}, script="echo_sum(a=input['a'], b=input['b']) * 2"))` then `S(double_sum(a=1, b=2))` | `6`; the record's `calls` contains `echo_sum` |
| `S(update_custom_tool(name="echo_sum", script="double_sum(a=1, b=1)"))` | `invalid`, `cycle: echo_sum → double_sum → echo_sum` |
| `S(update_custom_tool(name="echo_sum", new_name="add"))` | `has_callers: ["double_sum"]` |

## 6. Categories (Story 8)

| Send | Expect |
|---|---|
| `S(update_custom_tool_category(name="maths", description="Arithmetic"))` | updated; next turn's `context` shows the new line; this turn's does not change |
| `S(update_custom_tool_category(name="maths", new_name="arith"))` | renamed; `echo_sum` is still callable by the same name and lists under `arith` |
| `S(update_custom_tool(name="loose", category="arith"))` | moved, as a new version; `custom` line disappears from the next turn's `context` |
| Two turns with `S(create_custom_tool(... category="arith" ...))` between them | `context` before and after are identical (SC-010) |

## 7. People (Story 5) — WebUI, as a person who can edit the agent

1. Open the agent's **Custom tools** page: groups `arith`, `memory_custom`, … each with its
   description; rows show version, author, time, switch.
2. Switch `with_sample` off. In chat: `S(with_sample(n=1))` → unknown; `S(set_custom_tool_enabled(name="with_sample", enabled=True))` → `disabled_by_person`.
3. Open `echo_sum`, edit the script, save → version 5 authored by you; `S(echo_sum(a=1,b=1))` runs it. Edit again from a second tab holding version 4 → "changed underneath you", reload offered.
4. Rename `arith` from its heading; move `ask` into it from its editor (new category asks for a description).
5. Delete `double_sum` (confirm). `S(list_custom_tools())` no longer lists it; the events list keeps the deletion.
6. Sign in as a person who can only view the agent: no switches, editor or category edit.

## 8. Dream (Story 7)

**Mechanics (dummyplug)**: trigger a dream (`POST /agents/{id}/dream` or the WebUI button) on an
agent whose dummyplug dream script calls `create_custom_tool` directly (no `execute_python`), then:
the tool exists, its version 1 author is "a dream (<cycle id>)", `sample_run` is absent, and a
dream call of `delete_custom_tool` or `echo_sum` is refused as not in the dream toolset.

**Judgment (live provider, marked)**: seed two conversations that run the same three-call workflow
with different inputs and one that needs judgment, run a dream, and check the extraction lists the
workflows, consolidation makes one custom tool and one skill, and the audit counts them; run the
dream again and check no duplicate appears (SC-008).

## 9. Needs attention

Attach an MCP server, create a tool that calls one of its tools, detach the server: the tool shows
*needs attention* naming the missing tool in `describe_tool_function`, `list_custom_tools` and the
WebUI, and a call fails at that step with the usual unknown-function answer.
