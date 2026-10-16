# Contract: Integrations HTTP API

**Feature**: `016-integrations-section`. Everything here lives under `/api/v1`, uses the existing JWT auth, and returns the existing error shape.

Secrets in responses are always redacted (`"••••a91c"`), see data-model §4.

## Agent-scoped (what the WebUI pages use)

`{agent_id}` must be an agent the caller can view (`user_can_view_agent`), or the request is `404`.

### `GET /agents/{agent_id}/integrations?family=channel|addon|mcp`

This returns what the page for that agent shows. `items` lists every integration of that family that the caller can see, marked as attached or not:

- everything, for all-scope managers;
- their own plus `shared` ones, for owned-scope users;
- only the attached ones, for read-only viewers.

```json
{
  "family": "mcp",
  "can_manage": true,
  "items": [
    {
      "id": "6f1c…", "kind": "mcp", "name": "github", "enabled": true,
      "scope": "shared", "created_by": null,
      "settings": { "transport": "local", "command": "npx", "args": ["-y","@modelcontextprotocol/server-github"], "env": { "GITHUB_TOKEN": "••••a91c" } },
      "attached": true,
      "also_used_by": [{ "agent_id": "atlas", "name": "Atlas" }],
      "status": { "state": "running", "tools": 26 },
      "can_edit": false, "can_attach": true
    }
  ]
}
```

- **`also_used_by`** lists the other agents the integration is attached to. All-scope managers see every one. Everyone else sees only the agents they can view, plus `"others": N` for the rest.
- **`status`** is this agent's view of the integration: the reader for channels, its own connection for MCP, the addon state for addons. It is `{"state":"unknown"}` while the agent isn't running.

### `PUT /agents/{agent_id}/integrations/{integration_id}/attachment`

Body `{ "attached": true | false }`.
- Allowed when `can_attach` holds: all-scope, or owned-scope on an agent the caller owns and an integration that is theirs or `shared`.
- **Channel family**: a connection belongs to the agent it was created on. Detaching it, or attaching it to another agent, is refused with `409 channel_belongs_to_agent`; it leaves its agent only by `DELETE /integrations/{id}`.
- **Addon, `attached: true`** replaces any other configuration of the same addon kind on this agent (one per agent); the response's `affected_agents` covers both.
- **MCP, `attached: true`** is refused with `409 name_in_use_on_agent` when another attached MCP server on this agent has the same `name`.
- **Addon, `attached: true`, settings incomplete** is refused with `409 {"error":"incomplete","missing":["api_key"]}`. The UI then shows the settings form first (spec FR-011a).
- **Response:** the updated item, plus `"affected_agents": [...]`.

### `POST /agents/{agent_id}/integrations`

Creates an integration and attaches it to `{agent_id}` in the same transaction.

Body: `{ "kind": "mcp", "name": "linear", "settings": {...}, "enabled": true }`.

The caller needs either integration permission, and owned-scope callers must own the agent. `scope` is set from the caller's permissions (research R6).

| Status | When |
|---|---|
| `201` | Returns the item |
| `409 name_in_use_on_agent` | `mcp` only: this agent already has an MCP server with that `name` attached (tool names would collide); the body names it |
| `409 platform_in_use_on_agent` | channel only: this agent already has a connection for that platform |
| `409 token_in_use` | The bot token is already used by another connection (FR-009) |
| `422` | Settings fail the kind's `validate` |

## Integration-level (edit and delete, independent of the page's agent)

### `GET /integrations/{id}`

Returns the item, without the agent-relative fields. It needs see-access through any agent, or ownership, or all-scope.

### `PUT /integrations/{id}`

Body: any of `name`, `settings`, `enabled`.
- Needs `can_edit`: all-scope, or `created_by == caller`, or, for a channel connection, permission to manage its agent's integrations.
- Secrets that are left out, or sent back redacted, are kept (`merge_secrets`).
- The response lists `affected_agents`. The UI shows these in the shared-edit warning **before** saving, using `also_used_by` from the GET.

### `DELETE /integrations/{id}`

Needs `can_edit`. With `?confirm=true` it deletes the integration and returns `affected_agents`. Without it, it returns `409 {"error":"attached","agents":[...]}` when the integration is attached anywhere (spec edge case: confirm first).

## Catalogue

### `GET /integrations/kinds?family=addon|channel|mcp`

The built-in kinds' `listing()`, read from the `IntegrationKinds` registry; there is no network call. The Addons page builds its tiles from this.

```json
[{
  "id": "brave_search", "family": "addon", "name": "Brave Search",
  "description": "Search the web", "icon": "brave_search",
  "fields": [
    { "key": "api_key", "label": "API key", "type": "secret", "required": true },
    { "key": "safesearch", "label": "Safe search", "type": "bool", "default": true }
  ]
}]
```

Icons are embedded assets served from `/api/v1/integrations/kinds/{id}/icon`, the same way catalogue logos are embedded, so the binary stays self-contained.

## Changed endpoints

- **`POST /agents` and `PUT /agents/{id}`** return `422 {"error":"moved_to_integrations","fields":["discord_token", ...]}` when the body carries any of these fields:
  - `discord_token`, `telegram_token`;
  - `tools.discord`, `tools.telegram`, `tools.brave_search`, `tools.mcp_servers`.

  `[**breaking**]` (FR-021). The responses no longer contain those fields.
- **`GET /agents/{id}`** no longer returns them.
- **`GET /agents/health`** is unchanged in shape. `channels` now comes from `ChannelConnections`, filtered to the agent.
- **Role endpoints:** the permission list loses `agents:mcp_config` and gains the two integration permissions.

## Side effects (all write endpoints)

After the transaction commits, the handler publishes `IntegrationChanged { integration_id, family, agents: affected_agents }` (see [runtime.md](runtime.md)). The HTTP response does not wait for agents to refresh or readers to reconnect. Status catches up in a later `GET`.
