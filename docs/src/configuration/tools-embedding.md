# 2.5 Tools & Embedding

## `tools`

Global tool settings in `.vizier.yaml`:

```yaml
tools:
  brave_search:
    api_key: "${BRAVE_API_KEY}"
    safesearch: true
  mcp_servers:
    my_server:
      host: local
      command: "python"
      args: ["/path/to/server.py"]
```

> **Note:** MCP servers and shell config are auto-migrated to storage on first run, then managed via WebUI (Settings > MCP Servers, Settings > Shell). Brave Search API key can be set globally in YAML or per-agent in the WebUI.

### Tool Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `brave_search.api_key` | string | `"${BRAVE_API_KEY}"` | Brave Search API key (global default) |
| `brave_search.safesearch` | bool | `true` | Enable safe search filtering |
| `mcp_servers` | map | `{}` | MCP server definitions (see [MCP Servers](./mcp.md)) |

## `embedding`

Configure embedding models for vector memory:

```yaml
embedding:
  type: local
  model: all_mini_lml6_v2
```

> **Note:** Embedding config is **not** migrated to storage — it remains file-based and is read once at startup.

### Local Models

Set `type: local` and choose from 29+ local models (via fastembed):

| Model | Size | Use Case |
|-------|------|----------|
| `all_mini_lml6_v2` | ~22MB | Fast, good quality (default) |
| `all_mini_lml12_v2` | ~33MB | Better quality, slower |
| `bge_large_env15` | ~1.3GB | Best quality |
| `nomic_embed_text_v15` | ~540MB | Good balance |
| `mxbai_embed_large_v1` | ~1.3GB | Large model |
| `multilingual_e5_large` | ~1.3GB | Multilingual support |

### Cloud Providers

```yaml
embedding:
  type: openrouter
  model: "openai/text-embedding-3-small"
```

Supported cloud providers: `openrouter`, `ollama`, `openai`, `gemini`

### Per-Agent Tool Settings

Most tool settings are configured per-agent via the WebUI or API. See [Agent Configuration](./agents.md) for the full list of per-agent tool options including:

- `shell_access` — shell command execution
- `programmatic_sandbox` — Python sandbox for tool calls
- `brave_search` — per-agent API key and settings
- `fetch` — web page fetching
- `http_client` — arbitrary HTTP requests
- `vector_memory` — vector-based memory access
- `discord` / `telegram` — channel-specific tools
- `notify_primary_user` — notification tools
