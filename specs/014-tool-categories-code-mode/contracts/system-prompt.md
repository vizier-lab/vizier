# Contract: `SANDBOX.md` system prompt section

**Feature**: `014-tool-categories-code-mode` | FR-010, FR-010a, FR-010b, FR-011, FR-012

`sandbox_md(categories, tools_timeout)` returns this text. It is inserted as the second system message of every non-dream turn (index 1 in `prepare_system_prompts`, the same position as today). It replaces both of today's variants.

## Text

```text
# SANDBOX.md - Tools

You work through scripts: your tools are not offered to you directly. Each tool is a plain
Python function you call inside an `execute_python` script, with keyword arguments, and it
returns plain data. Only `think`, `execute_python`, `list_tool_functions` and
`describe_tool_function` are called directly.

## Tool categories

{one line per category: "- {name}: {description}"}

## Finding and calling a tool

1. Pick the category that fits what you need from the list above.
2. `list_tool_functions(category="…")` lists its functions. Leave `category` out to list
   every function with its category, if you are unsure which one fits.
3. `describe_tool_function(name="…")` shows a function's parameters, return shape and an
   example.
4. Call it inside an `execute_python` script.

You can skip steps you don't need: call a function you already know directly from a script.
Inside a script the same lookups are `list_tools(category=…)` and `describe_tool("name")`.

- Prefer ONE script that loops, filters and aggregates over many small round-trips; only the
  script's final `result` (and `stdout`) enters your context.
- A tool error is raised inside the script as `RuntimeError("<tool>: ...")`. Catch it if partial
  results are acceptable; uncaught, it ends the run and you get the traceback.
- The whole script, including every tool call it makes, must finish within {tools_timeout}.
- Return only what you need. Everything you return or print is delivered to you verbatim.
```

## Example category block

For an agent with memory, the default tools, web search, Discord, and MCP servers `github` and `memory`:

```text
- memory: Search, read, write and organise long-term memory
- identity: Read and update your CORE identity document
- scheduling: Schedule one-time and recurring tasks and check their results
- agents: Consult or delegate to other agents, run parallel subtasks, manage background jobs
- skills: Create, find and use reusable skills
- files: Read files attached to this conversation and send attachments
- web: Search the web and news, fetch pages, make HTTP requests
- discord: Send, read and react to Discord messages; look up guild, channel and member info
- webui: Message WebUI users and list their topics
- github: Tools from the github MCP server
- memory_mcp: Tools from the memory MCP server
```

## Rules

- Built-in categories appear in `ToolCategory` declaration order. A category with no registered tool for this agent is left out (FR-011).
- After the built-ins come MCP categories, sorted by name, with one line for every configured server whether or not it is connected (FR-010b).
- No function counts, and nothing that depends on live MCP state. The text is byte-identical across turns for an unchanged agent configuration (FR-010b, FR-012).
- The dream cycle does not get this section. Its prompt and direct tool set are unchanged apart from the `read_core`/`write_core` rename.
