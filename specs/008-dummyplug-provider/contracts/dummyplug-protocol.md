# Contract: Dummyplug Chat Protocol

**Feature**: `008-dummyplug-provider`

This is the user-facing contract of a dummyplug agent. It applies to every channel (WebUI, HTTP REST/WS, Discord, Telegram), because it lives entirely in the model layer. "Message" below means the user's text after the channel's framing is removed (see research R4).

## 1. `tools`: list tools

**Input**: the message is `tools`, compared case-insensitively after trimming.

**Output** (Markdown text):

```text
**Available tools** (N):

- `read_core` — Read this agent's CORE document.
- `write_memory` — …
- `mcp_fs__read_file` — …

Send a tool name to get a sample request.
```

- There is one line per `ToolDefinition`, in the order the agent provides them. That order is the default toolset, then the user toolset, then MCP.
- Only the first line of each description is shown, truncated to 120 characters.
- With zero tools, the reply is `This agent has no tools available.`

## 2. `<tool name>`: sample request

**Input**: the message exactly equals a tool's name (case-sensitive, trimmed).

**Output**:

````text
**`write_memory`** — <full description>

Required:
- `title`: Title of the memory
- `content`
Optional:
- `tags`: Tags used to group related memories

```json
{
  "tool": "write_memory",
  "arguments": {
    "content": "content",
    "tags": ["tags"],
    "title": "title"
  }
}
```

Send the JSON back (edit the values first) to run the tool.
````

- Each argument is listed on its own line as `` `name`: description ``, with required arguments in the order the schema's `required` list gives and optional ones alphabetically. Keys in the JSON block are alphabetical, because JSON objects in Vizier don't keep declared key order. The description is the property's `description` from the schema, or the `description` of the definition a `$ref` points to. It is collapsed to a single line, with inner whitespace and newlines folded to single spaces. When no description exists, only `` `name` `` is shown (see `content` above).
- Nested object properties are not listed separately. Their descriptions are only shown for top-level arguments.
- A `Required:` or `Optional:` heading is omitted when its group is empty. A tool with no parameters gets no argument list and `"arguments": {}`.
- The JSON block must be accepted unchanged by §3 (FR-005 / SC-003).

## 3. `{ … }`: run a tool

**Input**: the message, after one optional surrounding code fence is stripped, starts with `{`.

**Shape**:

```json
{ "tool": "<tool name>", "arguments": { } }
```

- `tool` (string, required)
- `arguments` (object, optional, defaults to `{}`)
- Any other keys are ignored.

**Outcomes**:

| Case | Reply |
|------|-------|
| Valid and known tool | The agent executes the tool through the normal tool path. The tool call and the tool response appear in session history and the UI. The final reply follows §4. |
| Invalid JSON | `Could not parse tool request: <parser error>` plus the expected shape |
| Wrong shape | `Invalid tool request: <reason>` plus the expected shape |
| Unknown tool | ``Unknown tool `<name>`. Send `tools` to list available tools.`` |

No tool runs in any of the error cases.

## 4. Tool result echo

After a tool from §3 returns, the final reply of the turn is:

```text
**Tool result** (`<tool name>`):
<tool output or error text>
```

Dummyplug never starts another tool call in the same turn.

## 5. Anything else: lorem ipsum

This covers every other input, including scheduled task prompts, dream prompts, handover/summary prompts, and image-description prompts.

**Output**: 1–3 paragraphs of random lorem ipsum. It is never empty and differs between calls.

## Provider API

| Endpoint | Behavior for `dummyplug` |
|----------|--------------------------|
| `PUT /api/v1/providers/dummyplug` | `400`, `"dummyplug requires no configuration"` |
| `GET /api/v1/providers/dummyplug` | `404` (no entry is ever stored) |
| `GET /api/v1/providers` | Not listed |
| Agent create/update with `provider: "dummyplug"` | Accepted. No provider entry is required. |
