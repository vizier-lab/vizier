# 2.23 Custom Tools

An agent can save a Python script that composes its tools as a **custom tool**: a named, versioned tool of its own that any later script calls like a built-in one. A workflow the agent keeps repeating, such as "pull last week's notes, summarise them, write a digest", becomes one call.

Custom tools belong to exactly one agent. They are not shared or exported, and one agent cannot see another's.

## Skill or custom tool?

Use a **custom tool** for a workflow whose steps are fixed and only the inputs change. Use a **skill** when the steps need judgment. A one-off needs neither.

## Writing one

Custom tools are made from `execute_python`, with the authoring tools in the `custom_tools` category:

```python
create_custom_tool(
    name="weekly_digest",
    description="Summarise a topic's notes from the last week into the digests bundle",
    category="reports",
    category_description="Pull and send the weekly status reports",
    input_schema={
        "type": "object",
        "properties": {
            "topic": {"type": "string"},
            "limit": {"type": "integer", "default": 10, "maximum": 50},
        },
        "required": ["topic"],
    },
    script="""
notes = memory_search(query=input["topic"], limit=input["limit"])
titles = [p["title"] for p in notes["passages"]]
memory_write(bundle="digests", path=input["topic"], content="\\n".join(titles))
{"count": len(titles), "titles": titles}
""",
    sample_input={"topic": "releases"},
)
```

- The validated arguments arrive as the dict `input`, with `default`s filled in.
- The value of the script's last expression is the result.
- The script can call every tool an `execute_python` script can, including other custom tools.

From the next script on, the agent calls it by its name, `weekly_digest(topic="releases")`, in this conversation and every other.

A custom tool has no prefix, so its name cannot be one any of the agent's tools already has (a built-in, addon, MCP, client or project tool), a Python builtin, a sandbox tool or a retired tool name: saving such a name is refused with `name_taken`. If a tool attached later (say, a new MCP server) takes the name of an existing custom tool, the attached tool keeps it. The custom tool is then not offered and shows *needs attention: its name is taken* until it is renamed.

### What is checked when it is saved

Every problem comes back at once as `{ "status": "invalid", "problems": [...] }`, and nothing is saved until all of them are fixed:

| Code | Meaning |
|---|---|
| `name_invalid`, `name_taken` | Names are lower-case letters, digits and `_`, start with a letter, and are not the name of another custom tool or any other tool the agent has |
| `category_description_required` | A new category needs a one-line description |
| `schema_not_object`, `schema_keyword_unsupported` | The input schema is an object schema built from `type`, `properties`, `required`, `additionalProperties`, `items`, `enum`, `const`, `description`, `default`, `title`, `minimum`, `maximum`, `minLength`, `maxLength`, `minItems` and `maxItems` |
| `script_syntax` | The script doesn't parse (with its line and column) |
| `unknown_tool` | The script calls a function the agent doesn't have (with the nearest names) |
| `cycle` | Custom tools would call each other in a loop (`a → b → a`) |
| `sample_invalid` | `sample_input` doesn't match the schema |

With `sample_input`, the tool runs once before it is saved. The run is real, so its tool calls take effect; a failed run is `sample_failed` and nothing is stored.

### Calling one

A call checks the arguments against the schema first. A mismatch fails with a message such as `weekly_digest: invalid input: $.limit: expected integer, got string`, and nothing runs. The script then runs as part of the calling turn: the same tools, the same people to ask (or nobody, in an unattended turn), and the same time limit as the script that called it.

A failure names the tool, the step that failed and the calls made before it:

```text
weekly_digest failed at step 2 (memory_write): bundle 'digests' is reserved
calls made: 1 memory_search ok, 2 memory_write failed
```

The caller's report records one call, `weekly_digest`, with the calls its script made nested under it.

## Categories

The system prompt lists one line per category that has at least one enabled custom tool, after the built-in, addon, MCP and client categories. `list_tool_functions(category="reports")` lists that category's tools.

- The first save into a new category creates it, and needs a `category_description`.
- A tool saved with no category goes into `custom`.
- A category named after a built-in, addon or MCP category gets a `_custom` suffix (`memory_custom`).
- `update_custom_tool_category` renames or redescribes a category. Tools keep their `` names when their category is renamed.
- A category with no tools left is deleted.

The category lines are read when a turn starts, so a change shows from the agent's next turn and the prompt stays the same within a turn.

## Versions

Every change is a new version: `update_custom_tool`, a move to another category, and `rollback_custom_tool`, which saves an old version again as a new one. `custom_tool_history` lists versions and events, and `diff_custom_tool` shows line diffs of the description, input schema and script. Pass `expected_version` to be told `out_of_date` instead of overwriting a change you haven't seen.

## In the WebUI

In the WebUI, custom tools are called **functions**. People who can view an agent see its **Functions** page (beside Skills), grouped by category. Each tool shows its version, author, last change, whether it is model-backed, and *needs attention* when it calls a tool the agent no longer has.

People who can edit the agent can also:

- switch a tool off or on (a tool a person switched off stays off until a person switches it on);
- edit it, which saves a new version authored by them;
- roll it back or delete it (the deletion stays in the events);
- rename, redescribe or move between categories.

A person's edit runs no sample. Called tools are checked against the agent's published tool list; when that list is unavailable, the save goes through with a `tools_unchecked` warning.

The HTTP API is under `/api/v1/agents/{id}/custom-tools` and `/api/v1/agents/{id}/custom-tool-categories/{name}`. There is no create or run route: agents create custom tools, people inspect and govern them.

## Dreams

The dream cycle looks for **reusable workflows**: sequences of calls the agent ran more than once. Consolidation turns each one into a custom tool (fixed steps) or a skill (steps that need judgment), checking `list_custom_tools` and `list_skills` first so it updates instead of duplicating.

A dream may create, update, roll back and recategorise custom tools. It never deletes, switches off or calls one, and never runs a sample; its versions are authored by the cycle.

## Not in this version

No limits are set on the number or size of custom tools, beyond the sandbox's 64 KiB script cap. There are no security checks beyond those of any script (a custom tool may call any tool the agent has, including ones that wait on a person or end the turn); these come in a later pass.
