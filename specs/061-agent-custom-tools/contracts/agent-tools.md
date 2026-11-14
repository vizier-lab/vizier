# Contract: agent-facing tools

All tools are reached from `execute_python` scripts (code mode). Authoring tools are native tools
in category `custom_tools` (`ToolCategory::CustomTools`, appended 22nd). Refusals and misses are
`status` results, never `Err`, like the scratchpad and secrets tools.

## 1. Writing a custom tool's script

```python
# `input` is the validated arguments (a dict, defaults filled in).
notes = memory_search(query=input["topic"], limit=input.get("limit", 10))
titles = [p["title"] for p in notes["passages"]]
memory_write(bundle="digests", path=input["topic"], content="\n".join(titles))
{"count": len(titles), "titles": titles}     # last expression = the return value
```

Everything available to an `execute_python` script is available here, under the caller's turn.
`print` output is discarded unless the tool fails (then it is in the failure).

## 2. Authoring tools

| Tool | Input | Success result | In dreams |
|---|---|---|---|
| `create_custom_tool` | `name`, `description`, `input_schema`, `script`, `category?`, `category_description?`, `output_description?`, `sample_input?`, `reason?` | `{status: "created", tool: {id, name, call_as, category, version: 1}, category_created?: bool, category_renamed_to?: str, sample_run?: {ok, result, tool_calls, took_effect: true}}` | ✅ (no sample run) |
| `update_custom_tool` | `name`, any of the create fields plus `new_name?`, `expected_version?`, `reason?` | `{status: "updated", tool, version}` | ✅ |
| `list_custom_tools` | `category?`, `include_disabled?` (default true) | `{tools: [{name, call_as, category, version, enabled, disabled_by?, needs_attention?, model_backed, author, updated_at, last_called_at}], categories: [{name, description, tools}]}` | ✅ |
| `get_custom_tool` | `name`, `version?` | full definition of that version + `calls`, `callers`, `needs_attention`, `model_backed` | ✅ |
| `custom_tool_history` | `name` | `{versions: [{version, author, trigger, dream_cycle_id?, reason?, restored_from?, at}], events: [...]}` | ✅ |
| `diff_custom_tool` | `name`, `from`, `to?` (default current) | `{description: RevisionDiff, input_schema: RevisionDiff, script: RevisionDiff, category?: {from, to}}` | ✅ |
| `rollback_custom_tool` | `name`, `to_version`, `reason?` | `{status: "rolled_back", version, restored_from}` (validated as an update) | ✅ |
| `delete_custom_tool` | `name` | `{status: "deleted"}` | ❌ |
| `set_custom_tool_enabled` | `name`, `enabled` | `{status: "enabled" \| "disabled"}` | ❌ |
| `update_custom_tool_category` | `name`, `description?`, `new_name?` | `{status: "updated", category, renamed_to?}` | ✅ |

Refusal shape (every problem at once):

```json
{ "status": "invalid",
  "problems": [
    { "code": "unknown_tool", "message": "memory_serch is not a tool you have", "did_you_mean": ["memory_search"] },
    { "code": "script_syntax", "line": 4, "column": 12, "message": "expected ')'" },
    { "code": "schema_keyword_unsupported", "path": "$.properties.when", "keyword": "format" }
  ] }
```

Other statuses: `not_found`, `out_of_date` (with `current_version`), `disabled_by_person`,
`has_callers` (with `callers`), `sample_failed` (with the report), `cycle` is a `problems` entry.

`create_custom_tool`'s description carries the one-line skills/custom-tools rule (FR-029):
"Use a custom tool for a workflow whose steps are fixed and only the inputs change; use a skill
when the steps need judgment."

## 3. Calling a custom tool

From any script: `custom_<name>(**arguments)`, like any tool. The bridge:

1. resolves `custom_<name>` from the turn's catalogue (enabled tools only);
2. loads the current version; disabled/deleted since the listing → `RuntimeError("custom_x: disabled by <who>")` / `NameError`-like unknown;
3. validates the arguments (R6); a mismatch → `RuntimeError("custom_x: invalid input: $.limit: expected integer, got string; …")` and nothing runs;
4. refuses a name already on the call chain → `RuntimeError("custom_a: cycle a → b → a")`;
5. runs the script with `input` bound, under the caller's tool timeout and deadline;
6. returns the result value, or raises `RuntimeError` whose message is the failure summary:

```text
custom_weekly_report_pull failed at step 3 (memory_write): bundle 'digests' is reserved
calls made: 1 memory_search ok, 2 memory_read ok, 3 memory_write failed
```

The caller's report records one invocation `custom_<name>` with `calls: [...]` holding the nested
records (FR-028).

## 4. Discovery additions (existing tools)

- **`SANDBOX.md` category list**: one line per agent-owned category with ≥ 1 enabled tool, after
  built-in, addon, MCP and client lines: `- reports: Pull and send the weekly status report`. The
  `custom_tools` authoring category is a built-in line.
- **`list_tool_functions(category="reports")`**: the category's tools, as for any category.
- **`list_tool_functions()`** (all): custom entries carry `"custom": true`.
- **`describe_tool_function("custom_weekly_report_pull")`**: the usual doc plus
  `{"custom": {"version": 3, "author": "you" | "a dream (2026-11-14)" | "<person>", "model_backed": false, "needs_attention": null | {"missing": ["stripe_list_charges"]}}}`.
- Disabled tools appear in none of these.
