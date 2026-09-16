# Contract: Tool Definitions (what the model sees)

These are the `ToolDefinition`s (`rig_core::completion::ToolDefinition { name, description, parameters }`) returned by `VizierTools::tools()` in each exposure mode. Names are stable identifiers; descriptions are rendered from templates with the agent's live limits interpolated (FR-008).

## Exposure table

| `ToolExposure` | `tools()` returns |
|---|---|
| `Direct` (sandbox off) | all regular tools — **unchanged** |
| `SandboxAdditive` (sandbox on, code mode off) | all regular tools **+** `execute_python` |
| `CodeModeExclusive` (both on) | `think`, `execute_python`, `list_tool_functions`, `describe_tool_function` — **nothing else** |

`call()` in `CodeModeExclusive` rejects any other name with:
`"<name> is not exposed directly while code mode is on; call it from a script via execute_python (see list_tool_functions)"`.

`dream_tools()` / `dream_call()` are unchanged in every mode (FR-007).

## `execute_python`

**Input schema**

```json
{ "type": "object", "required": ["code"],
  "properties": { "code": { "type": "string", "description": "Python source to run. The value of the last expression is returned as `result`." } } }
```

**Output**: `ExecutionReport` (data-model.md), delivered as `VizierResponseContent::ToolResponse { response: <report JSON> }` with any attachments produced by nested tools attached to the same `VizierResponse` (FR-018).

**Description (sandbox only — code mode off)** — template, `{…}` interpolated:

```text
Run a Python script in an isolated sandbox and get back the value of its last expression
(`result`) plus anything it printed (`stdout`). Use it for exact computation: arithmetic,
date/time math, parsing, sorting, de-duplication, regex, JSON transformation, small algorithms.

The sandbox has NO filesystem, network, environment or OS access, and this agent's tools are
NOT callable from scripts (calling one raises NameError). Each run is stateless.

Supported: functions, closures, lambdas, simple classes (no inheritance), dataclasses,
comprehensions, try/except/finally, with, f-strings, and these modules:
json, math, datetime, re, collections, itertools, functools, dataclasses, typing, base64, copy,
random, unicodedata.
Not supported: generators/yield, match, del, inheritance, @property/@classmethod, user-defined
exception classes, eval/exec, third-party packages, time, hashlib, io, socket, subprocess.

Limits: {tools_timeout} wall-clock (the same tool timeout as every other tool), recursion depth 1000.
Exceeding a limit ends the run with an error naming the limit.
Return plain data (str, int, float, bool, None, list, dict); other objects cannot be returned.
Keep results small: everything you return or print is delivered to you verbatim.
On error you get the exception and a traceback with line numbers — fix the script and re-run.
```

**Description (code mode on)** — same as above with the second paragraph replaced by:

```text
The sandbox has NO filesystem, network, environment or OS access. This agent's tools ARE callable
from the script as plain functions with keyword arguments, e.g.
    hits = memory_read(query="rust releases", limit=5)
    page = fetch_webpage(url=hits[0]["url"])
Call `list_tool_functions` to see every available function and `describe_tool_function` for a
function's parameters, return shape and example — or call `list_tools()` / `describe_tool("name")`
from inside the script. A tool error is raised as an exception you may catch; an uncaught one ends
the run. The whole script, including every tool call it makes, must finish within {tools_timeout}.
Each run is stateless; loop and aggregate inside one script and return only what you need.
```

## `list_tool_functions` (code mode only)

**Input**: `{ "type": "object", "properties": {} }` (no arguments)

**Output**

```json
{ "count": 23,
  "functions": [ { "function": "memory_read", "summary": "Search this agent's memory…" }, … ] }
```

Sorted by `function`. Reflects the agent's *current* tool configuration on every call (FR-012) — it is computed from `ToolRouter::definitions()` at call time, not cached at agent start (MCP servers may add tools after connect).

**Description**: `List every function this agent can call from an execute_python script, with a one-line summary each. Use describe_tool_function for parameters and examples.`

## `describe_tool_function` (code mode only)

**Input**

```json
{ "type": "object", "required": ["name"],
  "properties": { "name": { "type": "string", "description": "Function name as shown by list_tool_functions" } } }
```

**Output (found)**: `ToolFunctionDoc` (data-model.md):

```json
{ "function": "memory_read", "tool": "memory_read",
  "summary": "Search this agent's memory…", "description": "…full…",
  "parameters": [ { "name": "query", "type": "string", "required": true, "description": "…" },
                  { "name": "limit", "type": "integer", "required": false, "description": "…" } ],
  "returns": "{ results: [{ slug, title, snippet, score }] }",
  "example": "result = memory_read(query=\"…\", limit=5)" }
```

**Output (not found)** — not an error (FR-014):

```json
{ "available": false, "name": "memory_reed",
  "message": "No function named 'memory_reed' is available to this agent.",
  "did_you_mean": ["memory_read", "memory_detail"] }
```

`did_you_mean` = up to 3 names by smallest edit distance ≤ 3 (or prefix match), may be empty.

**Description**: `Show the parameters, return shape and a usage example for one function callable from execute_python.`

## `think` (unchanged)

Kept in `CodeModeExclusive` as the single reasoning-only housekeeping tool.

## System prompt briefing — `SANDBOX.md` (`system_prompt/sandbox.rs`)

`sandbox_md(exposure, tools_timeout) -> Option<String>`; pushed by `prepare_system_prompts` immediately after `BOOT.md` when the sandbox is on. `{timeout}` is the agent's `tools.timeout` rendered like BOOT.md renders times.

**`SandboxAdditive` (sandbox on, code mode off)**

```text
# SANDBOX.md - Python Sandbox

You can run Python with the `execute_python` tool. Use it whenever an answer depends on exact
computation — arithmetic, dates, parsing, sorting, regex, JSON reshaping — instead of working it
out in your head. The value of the script's last expression comes back as `result`; `print()`
output comes back as `stdout`.

The sandbox is isolated: no files, network, environment or OS, and your other tools are NOT
callable from inside a script — call them directly as usual. A script must finish within
{timeout}. If a script fails you receive the exception and a traceback; fix it and run again.
```

**`CodeModeExclusive` (both on)**

```text
# SANDBOX.md - Code Mode

Code mode is on: your tools are not offered to you directly. You reach every one of them by
writing a Python script for `execute_python`, where each tool is a plain function taking
keyword arguments and returning plain data.

- Not sure what exists? Call `list_tool_functions` first, then `describe_tool_function` for
  parameters and an example. Inside a script the same information is available from
  `list_tools()` and `describe_tool("name")`.
- Prefer ONE script that loops, filters and aggregates over many small round-trips; only the
  script's final `result` (and `stdout`) enters your context.
- A tool error is raised inside the script as `RuntimeError("<tool>: ...")` — catch it if partial
  results are acceptable; uncaught, it ends the run and you get the traceback.
- The whole script, including every tool call it makes, must finish within {timeout}.
- Return only what you need. Everything you return or print is delivered to you verbatim.
- `think` is still available directly for reasoning.
```

Not injected for `Direct` (sandbox off). The static BOOT.md directive 7 remains unchanged.
