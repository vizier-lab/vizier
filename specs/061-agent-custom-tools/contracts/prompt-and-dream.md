# Contract: prompt and dream changes

## 1. `SANDBOX.md` (system prompt)

`sandbox_md(categories, ..)` is unchanged; its input list gains, at the end, the agent's custom
categories from `ToolContext.custom_categories` (snapshotted at turn start):

```text
## Tool categories

- memory: Search, read, write and organise long-term memory
…
- custom_tools: Make, change and organise your own custom tools
- github: Tools from the github MCP server
- reports: Pull and send the weekly status report
- custom: Tools you made that have no category yet
```

Order: built-ins (declaration order, `custom_tools` last of them), addons, MCP servers, client
categories, then custom categories by name with `custom` last. Rules:

- An agent with no enabled custom tools gets no custom lines: its prompt is byte-identical to
  today's except the `custom_tools` built-in line, which — like every built-in — appears only when
  its tools are offered. **SC-007 therefore needs `custom_tools` to be offered only once the agent
  has at least one custom tool or the agent can create them**; since agents can always create, the
  line is present for every agent from this release on. That is one deliberate, one-time prompt
  change on upgrade; SC-007 is read as "no further change from custom tools until one exists"
  (tests pin both).
- Within a turn the list never changes (snapshot). Across turns it changes only when a category is
  added, renamed, redescribed, removed, or its last enabled tool is disabled/enabled (SC-010).
  Tests: two turns with a tool saved into an existing category in between produce identical
  `SANDBOX.md`; a new category between them changes exactly one line.

## 2. Dream extraction (`EXTRACTION_PROMPT`)

New section, after `### Learnings`:

```text
### Reusable Workflows
- Sequences of tool calls you ran more than once, or that the person asked you to repeat.
- For each: the steps in order, the inputs that changed between runs, whether any step needed
  your judgment (and which), how many times you saw it, and the script if one ran.
```

## 3. Dream consolidation (`CONSOLIDATION_PROMPT_TEMPLATE`)

Step 4 becomes:

```text
4. **Create or update skills and custom tools** — For every reusable workflow in the reports:
   - steps fixed, only the inputs vary → a custom tool (`create_custom_tool`; `input` holds the
     arguments, the last expression is the result);
   - steps need judgment → a skill (`create_skill`);
   - a one-off → nothing.
   First check `list_custom_tools` and `list_skills`; update an existing one rather than create a
   near-duplicate. Put a new custom tool in a fitting existing category before making a new one,
   and if categories hold only one or two related tools, merge them
   (`update_custom_tool` with another `category`, `update_custom_tool_category`).
```

The audit step adds: `N skills and M custom tools created or updated; reusable workflows left
alone and why`, and the final report lists the custom tools by name.

## 4. Dream toolset

`DREAM_TOOL_NAMES` adds `list_custom_tools`, `get_custom_tool`, `custom_tool_history`,
`diff_custom_tool`, `create_custom_tool`, `update_custom_tool`, `rollback_custom_tool`,
`update_custom_tool_category`. Never `delete_custom_tool`, `set_custom_tool_enabled`, or any
custom tool. `dream_call` sets `ToolContext.script_router` to the agent's full router with
`dream = true`: saves are validated against the full toolset, the sample run is skipped, and the
version records the cycle id from the request metadata.
