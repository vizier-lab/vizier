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

## Supported Providers

| Provider | Config | Description |
|----------|--------|-------------|
| `openrouter` | `api_key` | [OpenRouter.ai](https://openrouter.ai) - Access 200+ models with a single API key |
| `deepseek` | `api_key` | [DeepSeek](https://deepseek.com) - High-quality Chinese and English models |
| `ollama` | `base_url` | Local Ollama instance (default: `http://localhost:11434`) |
| `anthropic` | `api_key` | [Anthropic Claude](https://anthropic.com) models |
| `openai` | `api_key`, `base_url` | OpenAI models (custom base_url for compatibility with OpenAI-compatible APIs) |
| `gemini` | `api_key` | [Google Gemini](https://ai.google.dev) models |
| `llama.cpp` | `base_url` | Local Llama.cpp instance (default: `http://localhost:8080`) |

## Example Configuration

```yaml
providers:
  openrouter:
    api_key: "${OPENROUTER_API_KEY}"
  
  deepseek:
    api_key: "${DEEPSEEK_API_KEY}"
  
  ollama:
    base_url: "http://localhost:11434"
  
  anthropic:
    api_key: "${ANTHROPIC_API_KEY}"
  
  openai:
    api_key: "${OPENAI_API_KEY}"
    base_url: null  # Optional: for OpenAI-compatible APIs
  
  gemini:
    api_key: "${GEMINI_API_KEY}"
    
  llama_cpp:
    base_url: "http://localhost:8080"
```
