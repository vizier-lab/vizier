# Quickstart: Code Mode by Default, Tool Categories, Simpler MCP Tool Names

**Feature**: `014-tool-categories-code-mode` | **Date**: 2026-10-14

## Build and static checks

```sh
just install
cargo build
cargo test          # includes the category-coverage, catalogue-naming and old-names-unsupported unit tests
cargo clippy
cd webui && npm run typecheck
```

## End-to-end checks with dummyplug agents (constitution gate)

Every step runs against the real binary with agents on the offline `dummyplug` provider (`specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`), so there are no keys and no network. The MCP servers are `fixtures/echo_mcp.py`, which needs only `python3`. Each echoes `"<tool>: <text>"`, so the reply shows which tool a call reached.

Send the JSON requests as chat messages. A script request has this shape:

```json
{"tool": "execute_python", "arguments": {"intent": "check", "code": "<script>"}}
```

### 0. Setup

**An agent saved before the upgrade (FR-003)**: on `master`, run `just run` and create the dummyplug agent `dp-old` with `"tools": {"python": {"enabled": false, "code_mode": false}}`. Then run `just shutdown` and switch to this branch.

On this branch, start the server:

```sh
env -u OPENAI_API_KEY -u ANTHROPIC_API_KEY just run
```

Then create two more dummyplug agents (`"provider": "dummyplug", "model": "dummyplug"`, `tools.timeout = "5s"`):

| Agent | Extra config |
|-------|--------------|
| `dp-new` | none. Over the HTTP API, also send `"tools": {"python": {"enabled": true, "code_mode": true}, …}` to check the key is ignored (FR-004) |
| `dp-mcp` | `mcp_servers` as below, with `F=$PWD/specs/014-tool-categories-code-mode/fixtures/echo_mcp.py` |

```json
{
  "github": { "host": "local", "command": "python3", "args": ["-I", "F", "create_issue", "search_issues"] },
  "memory": { "host": "local", "command": "python3", "args": ["-I", "F", "search"] },
  "a":      { "host": "local", "command": "python3", "args": ["-I", "F", "b_c"] },
  "a_b":    { "host": "local", "command": "python3", "args": ["-I", "F", "c"] }
}
```

Give the servers a few seconds to connect in the background. The `memory` category steps below assume the agents have memory set up (an embedding config). Without it, the `memory` category is absent, which is FR-011 working as intended.

### 1. Every agent is in code mode (US1, FR-001–FR-004)

| Agent | Send | Expect |
|-------|------|--------|
| `dp-old` | `tools` | exactly 4 lines: `describe_tool_function`, `execute_python`, `list_tool_functions`, `think` |
| `dp-new` | `tools` | the same 4 lines |
| `dp-new` | `{"tool": "memory_search", "arguments": {"query": "x"}}` | ``Unknown tool `memory_search` ``: nothing runs |
| `dp-old` | script `memory_list(limit=1)` | `ok: true`, and `tool_calls[0].name` is `memory_list` |
| (API) | `GET /api/v1/agents/dp-new` | there is no `tools.python` key in the response |
| (WebUI) | open the agent settings for `dp-new` | there are no Python sandbox or code-mode switches |

### 2. Discovery by category (US2, FR-008, FR-009)

On `dp-mcp`:

| Send | Expect |
|------|--------|
| `{"tool": "list_tool_functions", "arguments": {"category": "scheduling"}}` | `category: "scheduling"`, 7 functions, each with `name` and `summary` only |
| `{"tool": "list_tool_functions", "arguments": {}}` | every function, each with `category`. No `think`, `execute_python` or docs tools. Built-in categories come first, in system-prompt order |
| `{"tool": "list_tool_functions", "arguments": {"category": "schedule"}}` | `available: false`, `did_you_mean: ["scheduling"]`, and the turn continues normally |
| script `[f["name"] for f in list_tools(category="identity")]` | `result: ["read_core", "write_core"]` |
| script `try:\n  list_tools(category="nope")\nexcept ValueError as e:\n  r = str(e)\nr` | `ok: true`, and `result` names `nope` |
| `{"tool": "describe_tool_function", "arguments": {"name": "github_create_issue"}}` | a doc with `category: "github"` and a `text` parameter |

### 3. MCP names, categories and collisions (US3, FR-007, FR-013–FR-015)

On `dp-mcp`:

| Send | Expect |
|------|--------|
| `{"tool": "list_tool_functions", "arguments": {"category": "github"}}` | `github_create_issue`, `github_search_issues` |
| `{"tool": "list_tool_functions", "arguments": {"category": "memory"}}` | only the built-in memory tools, with `memory_search` among them |
| `{"tool": "list_tool_functions", "arguments": {"category": "memory_mcp"}}` | `memory_search_2` |
| `{"tool": "list_tool_functions", "arguments": {"category": "a"}}` / `"a_b"` | `a_b_c` / `a_b_c_2` |
| script `github_create_issue(text="hi")` | `result` is `"create_issue: hi"` |
| script `memory_search_2(text="x")` | `"search: x"`, reached on the MCP server, not built-in memory |
| script `[a_b_c(text="1"), a_b_c_2(text="2")]` | `["b_c: 1", "c: 2"]` |
| script `mcp_github__create_issue(text="old")` | `ok: false`, with a `NameError` (old names are not supported) |
| `list_tool_functions` with no category | no name starting with `mcp_` |

**Server down**: stop `dp-mcp`'s `github` server by temporarily changing its command to `false` in the agent's config and saving. Then:
- `list_tool_functions` with `category: "github"` returns `count: 0` and the "may be unreachable" message;
- `github` is still in the system prompt (§4).

Restore the config afterwards.

### 4. System prompt (FR-010–FR-012)

Live-provider step, or the server's debug log of the outgoing prompt (`RUST_LOG=vizier=debug`): the dummyplug protocol has no way to echo the system prompt. Check `dp-mcp`'s prompt for:
- a `# SANDBOX.md - Tools` section;
- category lines in this order: `memory`, `identity`, `scheduling`, `agents`, `skills`, `files`, `webui`, then `a`, `a_b`, `github`, `memory_mcp`;
- no `web`, `audio`, `media`, `discord`, `telegram` or `shell` line, because those tools are off;
- no counts.

Send two more turns and confirm the section is byte-identical. Then do the same with a server down: the section must not change.

### 5. CORE rename (FR-018, FR-019)

| Agent | Send | Expect |
|-------|------|--------|
| `dp-new` | script `read_core()` | `ok: true`, and the result is the CORE document |
| `dp-new` | script `READ_CORE()` | `ok: false`, with a `NameError` |
| `dp-new` | `{"tool": "list_tool_functions", "arguments": {"category": "identity"}}` | `read_core`, `write_core`, and no upper-case names |
| (WebUI) | open an old conversation with a `WRITE_CORE` or `mcp_x__y` tool call | it loads, and the call shows as "🔧 Using WRITE_CORE" / "🔧 Using mcp_x__y" with its arguments |
| (WebUI) | open the agent's CORE page | the text names `write_core` |

### 6. Dream cycle unaffected

Trigger a dream for `dp-new` (WebUI **Dreams → Run now**). The dream's tools include `write_core`/`read_core` and no sandbox tools, and the run completes.

### 7. Context saved (SC-001)

Compare `dp-old`'s `tools` reply on `master` with its reply on this branch, and add up the tool-definition JSON size of each with `RUST_LOG=vizier=debug`. The size on this branch must be at most 20% of the size on `master`.
