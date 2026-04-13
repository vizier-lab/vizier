# 2.7 Agent Configuration

Agents are defined in Markdown files with YAML frontmatter. Create files like `vizier.agent.md`:

```markdown
---
name: "Vizier"
description: "Your personal AI assistant"
provider: openrouter
model: "anthropic/claude-3.5-sonnet"
session_memory:
  max_capacity: 50
thinking_depth: 10
prompt_timeout: "5m"
session_timeout: "30m"
tools:
  timeout: "1m"
  shell_access: false
  brave_search:
    enabled: true
    programmatic_tool_call: false
  vector_memory:
    enabled: true
    programmatic_tool_call: true
  discord:
    enabled: true
    programmatic_tool_call: false
silent_read_initiative_chance: 0.1
show_thinking: true
include_documents:
  - "docs/**/*.md"
---

You are Vizier, a helpful AI assistant. You serve as the right hand of your user.

[Rest of your system prompt here...]
```

## Agent Configuration Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | string | required | Display name for the agent |
| `description` | string | `null` | Brief description of the agent's purpose |
| `provider` | enum | required | AI provider: `openrouter`, `deepseek`, `ollama`, `anthropic`, `openai`, `gemini` |
| `model` | string | required | Model identifier (provider-specific) |
| `session_memory.max_capacity` | number | required | Max messages in short-term memory |
| `thinking_depth` | number | required | How many previous messages to include in context |
| `prompt_timeout` | duration | `"5m"` | Tool execution timeout |
| `session_timeout` | duration | `"30m"` | Session TTL before automatic cleanup |
| `silent_read_initiative_chance` | float | `0.0` | Probability (0-1) of agent initiating conversation |
| `show_thinking` | boolean | `null` | Whether to show agent's thinking process |
| `show_tool_calls` | boolean | `null` | Whether to show tool call debug info |
| `heartbeat_interval` | duration | `"30m"` | Interval for agent heartbeat/initiation |
| `include_documents` | array | `null` | Glob patterns for additional context files |

## Agent Tools Configuration

Each tool can be configured with:

```yaml
tools:
  timeout: "1m"                         # Global tool execution timeout
  shell_access: false                   # Enable shell command execution
  mcp_servers: []                       # List of MCP server names to use
  brave_search:
    enabled: false
    programmatic_tool_call: false       # Allow tools to invoke search
  vector_memory:
    enabled: true
    programmatic_tool_call: true        # Allow tools to access memory
  discord:
    enabled: false                      # Enable Discord-specific actions
    programmatic_tool_call: false
  telegram:
    enabled: false                      # Enable Telegram-specific actions
    programmatic_tool_call: false
  notify_primary_user:
    enabled: true                       # Enable notification to primary user
    programmatic_tool_call: false
```

### Tool Options

| Tool | `enabled` | `programmatic_tool_call` | Note |
|------|-----------|-------------------------|------|
| `timeout` | N/A | N/A | Global tool execution timeout |
| `shell_access` | N/A (use `true`/`false`) | N/A | Subject to global `dangerously_enable_cli_access` |
| `mcp_servers` | N/A | N/A | List of MCP server names from `.vizier.yaml` |
| `brave_search` | Enable web search | Allow tools to invoke search | Requires `BRAVE_API_KEY` |
| `vector_memory` | Enable memory | Allow tools to use memory | Requires embedding config |
| `discord` | Enable Discord actions | Allow tools to use Discord | Requires Discord token in `.vizier.yaml` |
| `telegram` | Enable Telegram actions | Allow tools to use Telegram | Requires Telegram token in `.vizier.yaml` |
| `notify_primary_user` | Enable notifications | Allow tools to send notifications | Sends via Discord DM, Telegram DM, or WebUI |

## Complete Example

**`assistant.agent.md`:**
```markdown
---
name: "Assistant"
description: "A helpful coding assistant"
provider: openrouter
model: "anthropic/claude-3.5-sonnet"
session_memory:
  max_capacity: 100
thinking_depth: 20
prompt_timeout: 5m
session_timeout: 1h
tools:
  timeout: 1m
  shell_access: false
  mcp_servers: []                       # Add MCP server names here
  brave_search:
    enabled: true
    programmatic_tool_call: true
  vector_memory:
    enabled: true
    programmatic_tool_call: false
  discord:
    enabled: true
    programmatic_tool_call: false
  telegram:
    enabled: false
    programmatic_tool_call: false
  notify_primary_user:
    enabled: true
    programmatic_tool_call: false
show_thinking: true
show_tool_calls: null
heartbeat_interval: 30m
include_documents:
  - "docs/**/*.md"
---

You are a helpful coding assistant specialized in Rust.
Help the user write clean, efficient code.
```
