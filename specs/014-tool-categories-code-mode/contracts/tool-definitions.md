# Contract: Direct tools, the catalogue, and naming

**Feature**: `014-tool-categories-code-mode` | FR-001, FR-005–FR-009, FR-013–FR-015, FR-018, FR-019

## 1. Tools offered to the model

On every agent turn, the model is offered exactly these four tools, sorted by name:

| Tool | Change |
|------|--------|
| `describe_tool_function` | none |
| `execute_python` | its description always uses the "tools are callable" variant; the variant for when they are not is removed |
| `list_tool_functions` | gains an optional `category` parameter (§2) |
| `think` | none (now registered in the direct set, not the router) |

A direct call to any other name returns:

```text
<name> is not called directly; call it from an execute_python script (see list_tool_functions)
```

The dream cycle is unchanged: it is still offered `DREAM_TOOL_NAMES` directly, with `WRITE_CORE` and `READ_CORE` in that list replaced by `write_core` and `read_core`.

## 2. `list_tool_functions`

**Input**

```json
{ "category": "github" }
```

`category` is optional (a string or omitted). There is no wildcard value.

**Output: no category given**

```json
{
  "count": 42,
  "functions": [
    { "name": "memory_search", "summary": "Search memory for passages….", "category": "memory" },
    { "name": "github_create_issue", "summary": "Create an issue", "category": "github" }
  ]
}
```

The list is sorted by category (in system-prompt order) and then by `name`.

**Output: a known category**

```json
{
  "category": "github",
  "count": 2,
  "functions": [
    { "name": "github_create_issue", "summary": "Create an issue" },
    { "name": "github_search_issues", "summary": "Search issues" }
  ]
}
```

If the category belongs to a configured MCP server that currently lists no tools:

```json
{ "category": "github", "count": 0, "functions": [],
  "message": "No functions from the github MCP server right now; it may be unreachable. Try again later." }
```

**Output: an unknown category** (FR-009)

```json
{
  "available": false,
  "category": "schedule",
  "message": "No category named 'schedule'.",
  "did_you_mean": ["scheduling"]
}
```

`did_you_mean` uses the existing `docs::did_you_mean` (up to 3 names, edit distance ≤ 3 or a prefix match) over the agent's category names.

## 3. `describe_tool_function`

Input and output are unchanged, with one addition: the `ToolFunctionDoc` carries `"category": "<name>"`. Old names (§5) return the existing `not_found` shape.

## 4. In-script helpers

| Call | Returns |
|------|---------|
| `list_tools()` | the same `functions` list as §2 with no category given; each dict has `name`, `summary`, `category` |
| `list_tools(category="github")` | the `functions` list for that category; each dict has `name`, `summary`, `category` |
| `list_tools(category="nope")` | raises `ValueError("no category 'nope'; did you mean: …")` |
| `describe_tool("name")` | unchanged, plus `category` |

`list_tool_functions` / `describe_tool_function` keep working inside scripts as aliases of the same two helpers.

## 5. Names

**Native tools**: unchanged, except `READ_CORE` → `read_core` and `WRITE_CORE` → `write_core` (FR-018).

**MCP tools**: `python_identifier("<server>_<tool>")`, where `<server>` is the agent config's server key and `<tool>` is the server's own tool name (FR-013).

**Collisions** (FR-014): native names are reserved first. MCP tools are then named in `(server, tool)` order, and a name that is already taken gets `_2`, `_3`, … appended.

| Agent has | Catalogue names |
|-----------|-----------------|
| server `github`, tool `create_issue` | `github_create_issue` |
| native `memory_search`, plus server `memory` with tool `search` | `memory_search` (native), `memory_search_2` (MCP) |
| server `a` with tool `b_c`, plus server `a_b` with tool `c` | `a_b_c` (server `a`), `a_b_c_2` (server `a_b`) |
| server `my-srv`, tool `do-thing` | `my_srv_do_thing` |

**Old names** (FR-015, FR-019): `mcp_<server>__<tool>`, `READ_CORE` and `WRITE_CORE` are not supported. They are not listed, not described and not dispatched; in a script they raise `NameError` like any unknown function.

## 6. Categories

The built-in categories, their descriptions and their tools are as listed in `data-model.md` (ToolCategory). An MCP category is named after its server key, or `<key>_mcp` if the key equals a built-in category name. Its description is "Tools from the `<key>` MCP server".
