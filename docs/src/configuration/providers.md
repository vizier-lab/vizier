# 2.3 Providers

A provider is an LLM backend. Providers are shared across all agents; each agent picks one `provider` + `model` (and optionally a different `dream_provider`/`dream_model` for its dream cycle).

## Seeding from `.vizier.yaml`

```yaml
vizier:
  providers:
    ollama:
      base_url: "http://localhost:11434"
    openrouter:
      api_key: "${OPENROUTER_API_KEY}"
    anthropic:
      api_key: "${ANTHROPIC_API_KEY}"
```

Seed values are copied into the providers table on first run. After that they're managed at runtime:

- **WebUI**: Settings → Providers (requires the `settings:providers` permission)
- **API**: `GET /api/v1/providers`, `GET|PUT|DELETE /api/v1/providers/{variant}`

The `PUT` body accepts `api_key`, `base_url`, `enabled`, `access_token`, `account_id`, `endpoint` — whichever apply to the variant. `GET` responses never return the key itself, only `has_api_key`.

With no config file, `ollama` and `llama_cpp` are seeded at their default localhost URLs.

## Credential resolution

When an agent (or an embedding/TTS/STT/image tool) needs credentials for a provider, Vizier resolves them at request time in this order:

1. The provider's entry in the **providers table** (WebUI / API / migrated YAML), if it has a non-empty key or URL
2. The **environment variable** for that provider (column "Env var fallback" below)
3. For `ollama` / `llama_cpp` only: the built-in default localhost URL

So you don't strictly need a config file or a WebUI entry — exporting `OPENAI_API_KEY` in the process environment is enough for `openai` to work.

## Supported providers

| Variant | Config fields | Env var fallback | Notes |
|---------|---------------|------------------|-------|
| `ollama` | `base_url` | `OLLAMA_BASE_URL` | Local [Ollama](https://ollama.com), default `http://localhost:11434` |
| `llama_cpp` | `base_url` | `LLAMA_CPP_BASE_URL` | Local [llama.cpp](https://github.com/ggml-org/llama.cpp) server, default `http://localhost:8080` |
| `openai` | `api_key` | `OPENAI_API_KEY` | |
| `anthropic` | `api_key` | `ANTHROPIC_API_KEY` | The YAML *seed placeholder* written by `onboard` is `${ANTROPHIC_API_KEY}` (sic); the runtime fallback is `ANTHROPIC_API_KEY`. |
| `gemini` | `api_key` | `GEMINI_API_KEY` | Google Gemini |
| `deepseek` | `api_key` | `DEEPSEEK_API_KEY` | |
| `openrouter` | `api_key` | `OPENROUTER_API_KEY` | Hundreds of models behind one key |
| `mimo` | `api_key` | `XIAOMI_MIMO_API_KEY` | Xiaomi MiMo |
| `groq` | `api_key` | `GROQ_API_KEY` | |
| `mistral` | `api_key` | `MISTRAL_API_KEY` | |
| `xai` | `api_key` | `XAI_API_KEY` | Grok |
| `perplexity` | `api_key` | `PERPLEXITY_API_KEY` | |
| `moonshot` | `api_key`, `base_url?` | `MOONSHOT_API_KEY` | Kimi |
| `zai` | `api_key`, `base_url?` | `ZAI_API_KEY` | Z.ai / GLM |
| `minimax` | `api_key`, `base_url?` | `MINIMAX_API_KEY` | |
| `together` | `api_key` | `TOGETHER_API_KEY` | |
| `cohere` | `api_key` | `COHERE_API_KEY` | |
| `huggingface` | `api_key` | `HUGGINGFACE_API_KEY` | Inference API |
| `hyperbolic` | `api_key` | `HYPERBOLIC_API_KEY` | |
| `voyageai` | `api_key` | `VOYAGE_API_KEY` | Primarily an embedding provider |
| `galadriel` | `api_key` | `GALADRIEL_API_KEY` | |
| `mira` | `api_key` | `MIRA_API_KEY` | |
| `chatgpt` | `access_token`, `account_id`, `base_url?` | `CHATGPT_ACCESS_TOKEN`, `CHATGPT_ACCOUNT_ID`, `CHATGPT_API_BASE` | ChatGPT OAuth session (Codex-style), not an API key |
| `copilot` | `api_key` | `COPILOT_API_KEY` | GitHub Copilot |
| `azure` | `endpoint`, `api_key` | `AZURE_ENDPOINT`, `AZURE_API_KEY` | Azure OpenAI |
| `custom` | `api_key`, `base_url` | `CUSTOM_BASE_URL`, `CUSTOM_API_KEY` | Any OpenAI-compatible chat-completions endpoint |
| `opencode_zen` | `api_key`, `base_url?` | `OPENCODE_ZEN_API_KEY` | [OpenCode Zen](https://opencode.ai/docs/zen/) gateway, default `https://opencode.ai/zen/v1`. Chat-Completions-compatible models only. |
| `opencode_go` | `api_key`, `base_url?` | `OPENCODE_GO_API_KEY` | [OpenCode Go](https://opencode.ai/docs/go/) gateway, default `https://opencode.ai/zen/go/v1`. Same scope as `opencode_zen`. |
| `elevenlabs` | `api_key` | `ELEVENLABS_API_KEY` | TTS/STT only — not a chat provider |

`base_url?` means optional; omit it to use the provider's public endpoint.

In YAML you can put any literal or any `${VAR}` in these fields; the placeholder names above are just what the built-in defaults reference.

## Context window detection

Each agent's model context window is auto-detected from a built-in registry (`src/agents/agent/model/registry.rs`) or, for Ollama, queried from the server. Set the agent's `context_window` field to override. The context window drives automatic [checkpoints](./agents.md#checkpoints).

## Full seed example

```yaml
vizier:
  providers:
    ollama:
      base_url: "http://localhost:11434"
    llama_cpp:
      base_url: "http://localhost:8080"
    openai:
      api_key: "${OPENAI_API_KEY}"
    anthropic:
      api_key: "${ANTHROPIC_API_KEY}"
    gemini:
      api_key: "${GEMINI_API_KEY}"
    deepseek:
      api_key: "${DEEPSEEK_API_KEY}"
    openrouter:
      api_key: "${OPENROUTER_API_KEY}"
    groq:
      api_key: "${GROQ_API_KEY}"
    moonshot:
      api_key: "${MOONSHOT_API_KEY}"
      base_url: null
    azure:
      endpoint: "${AZURE_ENDPOINT}"
      api_key: "${AZURE_API_KEY}"
    custom:
      api_key: "${CUSTOM_API_KEY}"
      base_url: "https://my-gateway.example.com/v1"
    chatgpt:
      access_token: "${CHATGPT_ACCESS_TOKEN}"
      account_id: "${CHATGPT_ACCOUNT_ID}"
    elevenlabs:
      api_key: "${ELEVENLABS_API_KEY}"
```

## Related per-agent providers

Embedding, TTS, STT, and image generation each have their own provider enums configured **per agent** (they reuse the API keys stored here where applicable). See [Tools & Embedding](./tools-embedding.md).
