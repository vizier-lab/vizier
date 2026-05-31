# 2.7 Agent Configuration

> **Note:** Agents are now fully managed at runtime via the WebUI or HTTP API. The `.agent.md` file format is legacy and no longer used for agent configuration.

## Creating Agents

### Via WebUI

1. Open the WebUI at `http://localhost:9999`
2. Navigate to Agents
3. Click "Create Agent"
4. Configure name, provider, model, system prompt, and tools
5. Save — the agent starts automatically

### Via API

```sh
POST /api/v1/agents
```

See [REST API](../api-integration/rest-api.md) for details.

## Agent Configuration Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | string | required | Display name for the agent |
| `description` | string | `null` | Brief description of the agent's purpose |
| `avatar_url` | string | `null` | Custom avatar URL for the agent |
| `provider` | enum | required | AI provider: `ollama`, `openrouter`, `deepseek`, `anthropic`, `openai`, `gemini`, `mimo`, `llama_cpp` |
| `model` | string | required | Model identifier (provider-specific) |
| `system_prompt` | string | `null` | Full markdown system prompt (edited via WebUI) |
| `session_memory.max_capacity` | number | required | Max messages in short-term memory |
| `thinking_depth` | number | required | How many previous messages to include in context |
| `max_tokens` | number | `null` | Maximum tokens for model response |
| `prompt_timeout` | duration | `"5m"` | Tool execution timeout |
| `heartbeat_interval` | duration | `"30m"` | Interval for agent heartbeat/initiation |
| `dream_interval` | duration | `"1d"` | Interval for agent dream/review cycle |
| `silent_read_initiative_chance` | float | `0.0` | Probability (0-1) of agent initiating conversation |
| `show_thinking` | boolean | `null` | Whether to show agent's thinking process |
| `show_tool_calls` | boolean | `null` | Whether to show tool call debug info |
| `include_documents` | array | `null` | Glob patterns for additional context files |
| `discord_token` | string | `null` | Per-agent Discord bot token |
| `telegram_token` | string | `null` | Per-agent Telegram bot token |

## Agent Tools Configuration

Each tool can be configured per-agent:

| Tool | `enabled` | `programmatic_tool_call` | Note |
|------|-----------|-------------------------|------|
| `shell_access` | N/A (use `true`/`false`) | N/A | Subject to global shell config |
| `timeout` | N/A | N/A | Global tool execution timeout |
| `programmatic_sandbox` | N/A (use `true`/`false`) | N/A | Wrap tools in Python sandbox |
| `mcp_servers` | N/A | N/A | List of MCP server names from global config |
| `brave_search` | Enable web search | Allow tools to invoke search | Requires Brave Search API key |
| `vector_memory` | Enable memory | Allow tools to use memory | Requires embedding config |
| `fetch` | Enable web fetch | Allow tools to fetch webpages | Converts HTML to markdown |
| `http_client` | Enable HTTP client | Allow tools to make HTTP requests | Arbitrary REST API calls |
| `discord` | Enable Discord actions | Allow tools to use Discord | Requires Discord token |
| `telegram` | Enable Telegram actions | Allow tools to use Telegram | Requires Telegram token |
| `notify_primary_user` | Enable notifications | Allow tools to send notifications | Sends via Discord DM, Telegram DM, or WebUI |

## Agent Documents

Each agent has three markdown documents managed via the WebUI:

| Document | Purpose |
|----------|---------|
| `AGENT.md` | Core behavior instructions and system prompt |
| `IDENTITY.md` | Identity and personality definition |
| `HEARTBEAT.md` | Periodic heartbeat context |

These are edited through the WebUI's markdown editor and stored as files on disk at `{workspace}/agents/{agent_id}/`.

## Complete Example

**Via API:**
```json
{
  "name": "Assistant",
  "description": "A helpful coding assistant",
  "provider": "openrouter",
  "model": "anthropic/claude-3.5-sonnet",
  "session_memory_capacity": 100,
  "thinking_depth": 20,
  "prompt_timeout": "5m",
  "heartbeat_interval": "30m",
  "dream_interval": "1d",
  "tools": {
    "timeout": "1m",
    "shell_access": false,
    "programmatic_sandbox": false,
    "mcp_servers": [],
    "brave_search": { "enabled": true },
    "vector_memory": { "enabled": true },
    "fetch": { "enabled": true },
    "http_client": { "enabled": false },
    "discord": { "enabled": false },
    "telegram": { "enabled": false },
    "notify_primary_user": { "enabled": true }
  },
  "show_thinking": true,
  "system_prompt": "You are a helpful coding assistant specialized in Rust."
}
```
