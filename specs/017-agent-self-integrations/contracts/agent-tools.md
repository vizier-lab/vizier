# Contract: Agent self-management tools

These tools exist on an agent only while at least one grant is on. They sit in category `integrations` and are reached from code mode (`list_tool_functions {category:"integrations"}`, then called from `execute_python`). They are not in `DREAM_TOOL_NAMES`.

Every call does the following, in order:

1. Re-reads the agent's grants. If the call's family (from `kind`, or from the integration's family) isn't granted, it returns the error `"managing <family> is not enabled for this agent; ask someone who manages its integrations to turn it on"` and changes nothing (FR-008).
2. Resolves `id` among the agent's **own, non-legacy** integrations. Anything else returns `"no integration '<id>'"` (FR-009, FR-023).
3. Runs the shared `integrations::service` call with `Actor::Agent`, so validation, conflicts and secret handling are identical to the HTTP path (FR-011).

Errors come back as tool errors (catchable in scripts) carrying the same `code` the HTTP API uses (`name_in_use_on_agent`, `platform_in_use_on_agent`, `token_in_use`, `addon_installed`, `invalid`). Settings are always returned redacted (FR-014).

## `list_integration_kinds`

Input: `{ "family"?: "mcp" | "addon" | "channel" }`. Only granted families are listed.

Output:

```json
{ "kinds": [ { "kind": "mcp", "family": "mcp", "name": "MCP server",
  "description": "…", "installed": null,
  "fields": [ {"key":"transport","type":"select","options":["local","http"],"default":"local"},
              {"key":"command","type":"string"}, {"key":"env","type":"secret_map"} ] } ] }
```

For an addon, `installed` is `true` or `false`; for other families it is `null`.

## `list_integrations`

Input: `{ "family"?: … }`.

Output:

```json
{ "integrations": [ { "id": "…", "kind": "mcp", "family": "mcp", "name": "github",
  "enabled": true, "status": {"state":"running","tools":14}, "missing": [],
  "settings": {"transport":"http","uri":"https://…"}, "tool_category": "github",
  "creator": {"user":"alice"} } ] }
```

`tool_category` is the category name an MCP server's or addon's tools are listed under (`list_tool_functions {category}` lists them; that is how the agent "sees the tools a server offers", FR-007). It is `null` for channels.

## `create_integration`

Input: `{ "kind": "mcp", "name": "github", "settings": {…}, "enabled"?: true }`.

Output: the new integration as `list_integrations` shows it, plus `"note": "available from your next turn"`.

## `update_integration`

Input: `{ "id": "…", "name"?: "…", "settings"?: {…}, "enabled"?: bool }`. `settings` holds only the keys to change: they are laid over the stored settings (top-level keys; a nested value such as `env` is replaced whole), then through `merge_secrets`, so a redacted value sent back unchanged is kept.

Output: the integration, plus `note`, plus `warning` when R7 applies.

## `delete_integration`

Input: `{ "id": "…" }`.

Output: `{ "deleted": "<id>", "warning"?: "…" }`.

## Descriptions

Each tool's description is fixed when the tools are composed and names the granted families, e.g. *"Create one of your own integrations (allowed: MCP servers, addons). A local MCP server runs its command on the vizier host."* For the channel family, the description of `update_integration` and `delete_integration` also warns that disabling or deleting the connection you are talking on stops your messages there.
