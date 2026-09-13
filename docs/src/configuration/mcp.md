# 2.9 MCP Servers

[MCP (Model Context Protocol)](https://modelcontextprotocol.io/) lets an agent call tools exposed by external servers. In Vizier, MCP servers are configured **per agent** — each agent spins up its own client connections when it starts. There is no global MCP registry.

## Configuration

Agent settings → Tools → MCP Servers in the WebUI, or the `tools.mcp_servers` map in `POST|PUT /api/v1/agents/{id}`:

```json
{
  "tools": {
    "mcp_servers": {
      "filesystem": {
        "host": "local",
        "command": "npx",
        "args": ["-y", "@modelcontextprotocol/server-filesystem", "/home/me/docs"],
        "env": { "LOG_LEVEL": "info" }
      },
      "remote": {
        "host": "http",
        "uri": "https://mcp.example.com/mcp"
      }
    }
  }
}
```

The map key is the server name you choose; it becomes part of every tool name.

### `host: local` — stdio subprocess

| Field | Required | Description |
|-------|----------|-------------|
| `command` | yes | Executable to spawn |
| `args` | yes | Arguments (may be `[]`) |
| `env` | no | Extra environment variables for the subprocess |

The process is started with the agent and communicates over stdin/stdout.

### `host: http` — Streamable HTTP

| Field | Required | Description |
|-------|----------|-------------|
| `uri` | yes | MCP endpoint URL |

Uses the MCP *Streamable HTTP* transport (`rmcp`). Authentication headers are not currently configurable — use a server that doesn't need them or put a proxy in front.

## Tool naming

Every tool the server advertises is registered on the agent as:

```
mcp_<server_name>__<tool_name>
```

| Server | Tool | Registered as |
|--------|------|---------------|
| `filesystem` | `read_file` | `mcp_filesystem__read_file` |
| `remote` | `search` | `mcp_remote__search` |

Tool input schemas are passed through from the server. Calls are subject to the agent's `tools.timeout`.

## Lifecycle

- Clients are created in `VizierTools::new()` when the agent process starts; a server that fails to start is logged and skipped (the agent still runs without its tools).
- Updating the agent restarts the process and therefore all MCP connections.
- Deleting the agent tears them down.

## Example: GitHub MCP server

```json
{
  "tools": {
    "mcp_servers": {
      "github": {
        "host": "local",
        "command": "npx",
        "args": ["-y", "@modelcontextprotocol/server-github"],
        "env": { "GITHUB_PERSONAL_ACCESS_TOKEN": "ghp_…" }
      }
    }
  }
}
```

The agent then sees `mcp_github__create_issue`, `mcp_github__search_repositories`, and so on.

## Migration note

Older versions declared servers globally under `tools.mcp_servers` in `.vizier.yaml` and referenced them by name from each agent. That YAML key is now ignored; a one-time startup migration copied any global definitions onto the agents that referenced them.
