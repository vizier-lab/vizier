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

## Try it — sandbox only

1. `just run`, open the WebUI, edit an agent → **Tools → Python** → turn on **Python sandbox**, save.
2. Ask the agent: *"What is the 40th Fibonacci number, and how many weekdays are there between 2026-01-01 and 2026-09-16?"*
3. Expect **one** `🐍 Running Python` event, a collapsed `✅ Python finished` report, and a correct answer. The agent's other tools still work directly (ask it to search or read memory).

Equivalent API call:

```sh
curl -X PUT localhost:8080/api/v1/agents/$AGENT -H "Authorization: Bearer $JWT" -H 'content-type: application/json' \
  -d '{"...existing fields...", "tools": {"python": {"enabled": true}}}'
```

## Try it — code mode

1. Same agent → turn on **Code mode (programmatic tool calling)** (only enabled once the sandbox is on), read the warning, save.
2. Ask: *"Search the web for the three latest Rust releases and give me a one-line summary of each release page."*
3. Expect: `📖 Listing tool functions` (maybe), `📖 Describing …` (maybe), then `🐍 Running Python` followed by nested `🔍`/`🔧` events for each search/fetch the script makes, then one report listing every tool call with timings, then the answer. The model's tool list for this agent is now exactly `think`, `execute_python`, `list_tool_functions`, `describe_tool_function` — verify by asking it to call `memory_read` directly: it will be told to use `execute_python`.

Validation check:

```sh
curl -X PUT … -d '{"tools": {"python": {"enabled": false, "code_mode": true}}}'
# → 400 {"error": "tools.python.code_mode requires tools.python.enabled"}
```

## What a script can do (cheat sheet the agent also receives)

```python
import json, math, re, datetime
from collections import Counter

# sandbox only: pure computation; the LAST EXPRESSION is the result
words = re.findall(r"\w+", "the quick brown the lazy the")
Counter(words).most_common(1)          # → result: [["the", 3]]

# code mode: tools are plain functions, keyword args, plain data in/out
hits = memory_read(query="rust releases", limit=5)
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
| agent `tools.timeout` | the agent loop's existing per-tool wall-clock **and** Monty's CPU clock set to the same value | One setting, already there. The whole script — including every nested tool call — must finish within it; raise the agent's tool timeout for heavy code-mode scripts. The CPU clock only exists so an orphaned `while True: pass` stops on its own. |
| memory | **no ceiling in v1** | Everything a script allocates is released when the run ends (verified — repeated 200 MB runs don't grow the process), so nothing accumulates. A run's *peak* is bounded only by the timeout. A single allocation over 1 GiB is rejected up front by a fixed guard. |
| tool calls per script | **no cap in v1** | Bounded by the timeout. |
| output | **no truncation in v1** | Whatever the script prints/returns reaches the model; the engine's 10 MiB print buffer is the only hard cap. |
| recursion (1000) | Monty | CPython's default. |

## Verifying the safety properties manually (FR-020/022/023)

With the sandbox on, ask the agent to run each of these and confirm the outcome:

| Script | Expected |
|---|---|
| `while True: pass` | the turn fails with `Tool 'execute_python' timed out` after the agent's tool timeout, exactly like a hung tool; the agent replies normally on the next message |
| `"a" * (10**10)` | `❌ … (memory)` — the single-allocation guard, without touching 10 GB |
| `x = []\nwhile True: x.append("a" * 10000)` | times out like the above; RSS climbs during the run and is reusable afterwards (no ceiling in v1 — keep the tool timeout modest on small hosts) |
| `def f(n): return f(n + 1)\nf(0)` | `❌ … (recursion)` |
| `open("/etc/passwd").read()` | script error: OS access not available |
| `import socket` | `ModuleNotFoundError` |
| two chats with the same agent running `while True: pass` concurrently | both time out independently; other agents unaffected |

## Files to look at

- `src/sandbox/` — engine: `runtime.rs` (the resume loop), `convert.rs`, `docs.rs`
- `src/agents/tools/python/` — the three tools + the bridge to `ToolRouter`
- `src/agents/tools/mod.rs` — `ToolRouter`, `ToolExposure`, gating in `tools()` / `call()`
- `specs/007-code-mode-python-sandbox/contracts/` — exact model-facing, script-facing, HTTP and WebUI contracts
