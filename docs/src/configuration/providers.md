# 2.3 Providers

## `providers`

Configure AI model providers. At least one provider must be configured:

```yaml
providers:
  openrouter:
    api_key: "your-api-key"
  
  deepseek:
    api_key: "your-api-key"
  
  ollama:
    base_url: "http://localhost:11434"  # Default Ollama URL
    
  llama_cpp:
    base_url: "http://localhost:8080"  # Default llama.cpp URL
```

> **Note:** Provider configurations are auto-migrated to storage on first run. After migration, providers are managed via the WebUI (Settings > Providers) or HTTP API (`/api/v1/providers`).

## Supported Providers

| Provider | Config | Description |
|----------|--------|-------------|
| `ollama` | `base_url` | Local Ollama instance (default: `http://localhost:11434`) |
| `openrouter` | `api_key` | [OpenRouter.ai](https://openrouter.ai) - Access 200+ models with a single API key |
| `deepseek` | `api_key` | [DeepSeek](https://deepseek.com) - High-quality Chinese and English models |
| `anthropic` | `api_key` | [Anthropic Claude](https://anthropic.com) models |
| `openai` | `api_key`, `base_url` | OpenAI models (custom base_url for compatibility with OpenAI-compatible APIs) |
| `gemini` | `api_key` | [Google Gemini](https://ai.google.dev) models |
| `llama.cpp` | `base_url` | Local Llama.cpp instance (default: `http://localhost:8080`) |
| `mimo` | `api_key` | [Xiaomi MiMo](https://mimo.xiaomi.com) models |

## Example Configuration

```yaml
providers:
  ollama:
    base_url: "http://localhost:11434"
  
  openrouter:
    api_key: "${OPENROUTER_API_KEY}"
  
  deepseek:
    api_key: "${DEEPSEEK_API_KEY}"
  
  anthropic:
    api_key: "${ANTHROPIC_API_KEY}"
  
  openai:
    api_key: "${OPENAI_API_KEY}"
    base_url: null  # Optional: for OpenAI-compatible APIs
  
  gemini:
    api_key: "${GEMINI_API_KEY}"
    
  llama_cpp:
    base_url: "http://localhost:8080"

  mimo:
    api_key: "${XIAOMI_MIMO_API_KEY}"
```

## Managing Providers at Runtime

After the initial seed config is migrated, add or update providers via:

- **WebUI**: Settings > Providers
- **API**: `PUT /api/v1/providers/{variant}` with the provider config

The API supports the same provider variants as the YAML config.
