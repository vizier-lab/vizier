# Quickstart: Python Sandbox & Code Mode

**Feature**: `007-code-mode-python-sandbox` | **Date**: 2026-09-16

## Build

```sh
just install        # once — webui/node_modules must exist for build.rs
cargo build         # pulls monty 0.0.23 (+ ~20 s cold compile), no system packages needed
cargo test          # sandbox unit tests run without an LLM or a running server
cargo clippy
cd webui && npm run typecheck
```

Cross-compilation check (constitution gate — pure-Rust dependency, expected to pass with no `Cross.toml` change):

```sh
cross build --release --target x86_64-unknown-linux-musl
cross build --release --target aarch64-unknown-linux-gnu
```

## End-to-end checks with a dummyplug agent (constitution gate)

Every step below runs against the real binary with an agent on the offline `dummyplug` provider (`specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`): no keys, no network, deterministic. Dummyplug's `tools` reply shows exactly the tool list the model would get, and sending `{"tool": "execute_python", "arguments": {"code": "…"}}` runs a script through the real agent loop, hooks, session history and WebSocket. The reply is `**Tool result** (`execute_python`):` followed by the `ExecutionReport` JSON, or the turn error for a timeout.

### 0. Setup

```sh
env -u OPENAI_API_KEY -u ANTHROPIC_API_KEY just run
```

Create **three** dummyplug agents (WebUI **Agents → New**, provider `dummyplug (testing)`, or the HTTP API with `"provider": "dummyplug", "model": "dummyplug"`), each with `tools.timeout = "5s"`:

| Agent | `tools.python` |
|---|---|
| `dp-off` | *(omitted)* |
| `dp-sandbox` | `{"enabled": true}` |
| `dp-code` | `{"enabled": true, "code_mode": true}` |

In the WebUI a JSON request can be pasted in chat as is; over the API, send it as the message text.

### 1. Exposure per mode (FR-003, FR-004, FR-012, US3-S6, US4-S1–S3, SC-007, SC-008)

| Agent | Send | Expect |
|---|---|---|
| `dp-off` | `tools` | the same list as before this feature; no `execute_python`, no docs tools |
| `dp-sandbox` | `tools` | the `dp-off` list **plus** `execute_python`; no `list_tool_functions` / `describe_tool_function` |
| `dp-code` | `tools` | exactly 4 lines: `think`, `execute_python`, `list_tool_functions`, `describe_tool_function` |
| `dp-code` | `{"tool": "memory_read", "arguments": {"query": "x"}}` | ``Unknown tool `memory_read` `` — nothing runs (the hidden tool is not offered; `call()`'s own refusal is unit-tested) |
| `dp-code` | `execute_python` | the sample request; the description is the *code mode on* variant with the timeout set to `5s` |

### 2. Sandbox only — pure computation (US1, FR-009/010/019)

On `dp-sandbox`, send each `code` as `{"tool": "execute_python", "arguments": {"code": "<script>"}}`:

| Script | Expect in the report |
|---|---|
| `a, b = 0, 1\nfor _ in range(40): a, b = b, a + b\na` | `ok: true`, `result: 102334155`, `tool_calls: []` |
| `print("hi")\nx = 1` | `ok: true`, `stdout: "hi\n"`, `result: null` |
| `import datetime\n(datetime.date(2026, 9, 16) - datetime.date(2026, 1, 1)).days` | `result: 258` |
| `y = 1` then, as a second request, `y` | second run: `NameError` (runs are stateless, FR-009) |
| `def f(:` | `ok: false`, `error.kind: "script"`, `SyntaxError` with a line number |
| `memory_read(query="x")` | `error.kind: "script"`, `NameError` (tool access off, FR-019) |
| `execute_python(code="1")` | `error.kind: "script"`, message mentions nested execution (FR-011) |
| `class A: pass\nA()` | `error.kind: "script"`, "cannot return … return plain data" |
| any script over 64 KiB (e.g. `"#" * 70000` pasted as the code) | `error.kind: "limit"`, `error.limit: "script_size"` |

After these, the agent's regular tools still run directly: send `READ_CORE` then paste the sample back and get the CORE document.

### 3. Safety and limits (FR-020–FR-023, SC-005, SC-006)

On `dp-sandbox`:

| Script | Expect |
|---|---|
| `while True: pass` | after ~5 s the turn ends with `Tool 'execute_python' timed out after 5s`; the next message (`tools`) is answered normally |
| `"a" * (10**10)` | `error.limit: "memory"` straight away (single-allocation guard); process RSS does not jump |
| `def f(n): return f(n + 1)\nf(0)` | `error.limit: "recursion"` |
| `open("/etc/passwd").read()` | `error.kind: "script"`, "not available in the sandbox" |
| `import os\nos.environ` / `os.getenv("HOME")` | `error.kind: "script"`, "not available in the sandbox" |
| `import socket` | `ModuleNotFoundError` |
| `while True: pass` sent from **two** chats with `dp-sandbox` at once, while a third chat with `dp-off` sends `tools` | both scripts time out independently; `dp-off` answers immediately (FR-022/023) |

**Memory is released per run (FR-021a, SC-013)**: note the process RSS (`ps -o rss= -p $(pgrep -f "vizier run")`), then send `x = "a" * (200 * 1024 * 1024)\nlen(x)` three times. RSS after the third run is no higher than after the first (±10%).

**Overhead (SC-004)** — release build only (`cargo run --release -- run …`): send `1 + 1` five times; every report's `duration_ms` is ≤ 50.

### 4. Code mode — nested tool calls (US2, FR-015–FR-018, FR-025)

On `dp-code`:

| Script | Expect |
|---|---|
| `memory_write(title="qs-1", content="alpha")\nmemory_write(title="qs-2", content="beta")\nmemory_list()` | `ok: true`; `tool_calls` has 3 records in order (`seq` 1–3, `ok: true`, `duration_ms`); the result lists both memories; the WebUI shows `🐍 Running Python`, then the three nested `tool_choice` events, then the execution report; both memories appear in the Memory view |
| `READ_CORE()` | `ok: true`; the result is the CORE text (upper-case tool names are called as is) |
| `try:\n    memory_read(limit=3)\nexcept RuntimeError as e:\n    str(e)` | `ok: true`; the result starts with `memory_read:` (missing required `query`); `tool_calls[0].ok == false` (caught tool error, FR-017). *(`memory_detail` on a missing path is not an error — it returns `"Memory not found"`.)* |
| `memory_read(limit=3)` | `ok: false`, `error.kind: "tool"`, message names `memory_read` |
| `memory_read("x", "y", "z", "w")` | `TypeError … takes keyword arguments only; see describe_tool('memory_read')` |
| `n = 0\nwhile True:\n    memory_list()\n    n += 1` | runs past 1000 calls (no round-trip cap) and the turn ends at the 5 s timeout like any tool; the next message is answered normally |
| `execute_python(code="1")` | refused: nested execution error |

Then `GET /api/v1/…/sessions/{id}/history` for that chat: each run is **one** `ToolCall { name: "execute_python" }` + **one** `ToolResult` whose content is the report. None of the nested calls appear as top-level entries (SC-009 via the embedded records).

**Attachments (FR-018)**: if the agent has `tts` or `image_gen` enabled with a working provider, call `tts_generate(...)` / `image_generate(...)` from a script and confirm the file reaches the chat exactly as for a direct call. *(Needs provider keys; skip offline.)*

### 5. Code mode — discovery (US3, FR-012–FR-014)

On `dp-code`:

| Send | Expect |
|---|---|
| `{"tool": "list_tool_functions", "arguments": {}}` | `count` and a list sorted by `function`, including `memory_read`, `READ_CORE`, `think` and any `mcp_*` tools (sanitised names); no `execute_python`, and no tool the agent doesn't have enabled |
| `{"tool": "describe_tool_function", "arguments": {"name": "memory_read"}}` | parameters with types and required flags, `returns`, and an `example` line |
| `{"tool": "describe_tool_function", "arguments": {"name": "memory_reed"}}` | `available: false`, `did_you_mean` contains `memory_read` — a normal result, not a turn error |
| `execute_python` with `[d["function"] for d in list_tools()][:5]` | same names as the catalogue; no invocation records (docs lookups are not tool calls) |
| `execute_python` with `describe_tool("nope")` | the `available: False` dict; the run succeeds |
| enable `fetch` on `dp-code`, save, then `list_tool_functions` again | `fetch` now appears (US3-S5) |

With an MCP server configured on `dp-code`: its tools appear in the catalogue, and calling one from a script dispatches to the server (US2-S6).

### 6. Switches and validation (US4, FR-001/002/006)

| Action | Expect |
|---|---|
| `PUT` `dp-sandbox` with `"python": {"enabled": false, "code_mode": true}` | `400 {"status": 400, "message": "tools.python.code_mode requires tools.python.enabled"}`; the agent is unchanged |
| `PUT` `dp-code` with `"python": {"enabled": false}` | 200; `tools` on `dp-code` now lists the regular tools and no sandbox tools |
| WebUI: open `dp-code`, turn **Python sandbox** off | **Code mode** switches off in the same change and is disabled; the warning callout appears only while code mode is on |
| an agent record saved before this feature (restart on an existing data dir) | `GET` shows `"python": {"enabled": false, "code_mode": false}`; `tools` unchanged |
| set `dp-sandbox`'s `tools.timeout` to `2s`, send `while True: pass` | timed out after 2 s (US4-S6) |
| trigger a dream on `dp-code` | dream completes; its tool list is unchanged (no `execute_python`, FR-007) |

### 7. Live provider only (behaviour that depends on the model)

These need a real model, because they measure whether the **model** writes good scripts. They are outside the dummyplug gate:

- `dp-sandbox` on a live provider: *"What is the 40th Fibonacci number, and how many weekdays are there between 2026-01-01 and 2026-09-16?"* → one `execute_python` call, correct answer (SC-001).
- `dp-code` on a live provider with web search + fetch: *"Search the web for the three latest Rust releases and give me a one-line summary of each release page."* → one `execute_python` run with ≥4 nested invocations, and the page bodies do not appear as separate `ToolResult`s (SC-002/003); the model looks up docs before its first call (SC-011) and fixes a failing script within one extra turn (SC-010).

## What a script can do (cheat sheet the agent also receives)

```python
import json, math, re, datetime
from collections import Counter

# sandbox only: pure computation; the LAST EXPRESSION is the result
words = re.findall(r"\w+", "the quick brown the lazy the")
Counter(words).most_common(1)          # → result: [["the", 3]]

# code mode: tools are plain functions, keyword args, plain data in/out
hits = memory_read(query="rust releases")
pages = []
for h in hits["results"][:3]:
    try:
        pages.append(fetch_webpage(url=h["url"])["content"][:2000])
    except RuntimeError as e:          # a tool error is catchable
        pages.append(f"failed: {e}")
{"count": len(pages), "pages": pages}  # ← returned to the model; nothing else enters its context

describe_tool("fetch_webpage")         # same info as describe_tool_function
```

Not available (all raise): `open()`, `os.environ`, `import socket/subprocess/time/hashlib`, `yield`, `match`, class inheritance, `@property`, `eval`. Calling a tool while code mode is off → `NameError`.

## Limits — what they really mean

| Setting | Enforced as | Notes |
|---|---|---|
| agent `tools.timeout` | the agent loop's existing per-tool wall-clock **and** Monty's CPU clock set to the same value + 500 ms grace | One setting, already there. The whole script — including every nested tool call — must finish within it; raise the agent's tool timeout for heavy code-mode scripts. The CPU clock only exists so an orphaned `while True: pass` stops on its own. |
| memory | **no ceiling in v1** | Everything a script allocates is released when the run ends (verified — repeated 200 MB runs don't grow the process), so nothing accumulates. A run's *peak* is bounded only by the timeout. A single allocation over 1 GiB is rejected up front by a fixed guard. |
| tool calls per script | **no cap in v1** | Bounded by the timeout. |
| output | **no truncation in v1** | Whatever the script prints/returns reaches the model; the engine's 10 MiB print buffer is the only hard cap. |
| recursion (1000) | Monty | CPython's default. |

## Files to look at

- `src/sandbox/` — engine: `runtime.rs` (the resume loop), `convert.rs`, `docs.rs`
- `src/agents/tools/python/` — the three tools + the bridge to `ToolRouter`
- `src/agents/tools/mod.rs` — `ToolRouter`, `ToolExposure`, gating in `tools()` / `call()`
- `specs/007-code-mode-python-sandbox/contracts/` — exact model-facing, script-facing, HTTP and WebUI contracts
