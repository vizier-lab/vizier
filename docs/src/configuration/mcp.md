# 2.9 MCP Servers

## Overview

[MCP (Model Context Protocol)](https://modelcontextprotocol.io/) allows Vizier to connect to external tools and services through a standardized protocol. MCP servers can provide additional capabilities beyond the built-in tools.

## Configuration

MCP servers are configured in `.vizier.yaml` under `tools.mcp_servers`:

```yaml
tools:
  mcp_servers:
    my_server:
      host: local
      command: "python"
      args: ["/path/to/server.py"]
      env:
        KEY: "value"
```

## Connection Types

### Local Process

Run a local MCP server as a subprocess:

```yaml
tools:
  mcp_servers:
    my_server:
      host: local
      command: "python"                    # Executable
      args: ["/path/to/server.py"]        # Arguments
      env:                                  # Optional environment variables
        API_KEY: "${MY_API_KEY}"
```

### HTTP Endpoint

Connect to a remote MCP server via HTTP:

```yaml
tools:
  mcp_servers:
    remote_server:
      host: http
      uri: "https://example.com/mcp"
```

## Using MCP Servers in Agents

Reference MCP servers in your agent configuration:

```yaml
# in your *.agent.md file
tools:
  mcp_servers:
    - my_server        # Name from .vizier.yaml config
    - remote_server
```

Tools from MCP servers are prefixed with `mcp_<server_name>__` when called:

| Tool Name | MCP Server | Full Tool Name |
|-----------|------------|----------------|
| `tools_list` | `my_server` | `mcp_my_server__tools_list` |
| `search` | `remote_server` | `mcp_remote_server__search` |

## Example: Brave Search MCP Server

A local MCP server implementation:

```yaml
tools:
  mcp_servers:
    brave_search:
      host: local
      command: "npx"
      args: ["-y", "@modelcontextprotocol/server-brave-search"]
      env:
        BRAVE_API_KEY: "${BRAVE_API_KEY}"
```

Then in your agent:

```yaml
tools:
  mcp_servers:
    - brave_search
```

## Requirements

- MCP server must implement the [MCP protocol](https://modelcontextprotocol.io/specification)
- Local servers require the command to be executable
- HTTP servers must expose the MCP endpoint