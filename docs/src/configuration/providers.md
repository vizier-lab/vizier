# 2.3 Providers

A provider is an LLM backend. Providers are shared across all agents; each agent picks one `provider` + `model` (and optionally a different `dream_provider`/`dream_model` for its dream cycle, and a vision model for read-image).

## The model catalogue

Vizier's list of providers and models comes from the [models.dev](https://models.dev) catalogue: over 200 providers and thousands of models, each with its context window, output limit, capabilities (tool calling, image input, reasoning) and price. Any catalogue provider that speaks a protocol Vizier supports (OpenAI chat or Responses, Anthropic, Gemini, Ollama) can be used — adding a provider needs no code change.

- **Offline by default**: a snapshot of the catalogue is built into the binary, so a fresh install works without network access.
- **Kept current lazily**: when a person opens the provider pickers or Settings → Providers, Vizier checks models.dev in the background (at most once per `catalogue.ttl`, with a conditional request) and keeps the result in SQLite. Agents, scheduled tasks and dreams never trigger a check, and startup makes no request. Settings → Providers shows the catalogue's date and has a **Refresh now** button.
- **Turn it off** with `catalogue.refresh: false` (or `VIZIER_CATALOGUE_REFRESH=false`); the built-in snapshot is then used until the next upgrade.

```yaml
vizier:
  catalogue:
    refresh: true   # default
    ttl: 24h        # default; how often a person's visit may trigger a check
```

## Connecting a provider

- **WebUI**: Settings → Providers (requires the `settings:providers` permission). Search for a provider, enter its key, **Test**, **Save**. Agent editors can also connect a provider inline from the agent form's model picker.
- **Environment variables**: the catalogue's variable for the provider (e.g. `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `GROQ_API_KEY`) is picked up automatically. Settings → Providers lists these as *From env NAME*.
- **API**: `GET /api/v1/providers` (connected providers, with where their key comes from and which agents use them), `PUT|DELETE /api/v1/providers/{id}`, `POST /api/v1/providers/{id}/test`. The `PUT` body accepts only the fields the provider uses (`api_key`, and for some `endpoint`, `base_url`, `access_token`, `account_id`); responses never return a secret.

Credentials are resolved in this order: the saved entry, then the catalogue's environment variable, then the environment variable Vizier read before the catalogue existed (see the table below). Saving or removing a provider's credentials restarts the agents that use it, so the change takes effect immediately. Saved API keys and access tokens are encrypted at rest; the workspace key is separate from the database and must be backed up — see [Encryption at Rest](./encryption.md).

A cloud provider's address always comes from the catalogue; to point at another address (a proxy, a self-hosted server) add a **custom provider**.

> **YAML `providers` is gone.** Earlier versions seeded providers from a `providers:` section in `.vizier.yaml`. It is now ignored; Vizier logs a warning naming the environment variable to set instead. Credentials saved by earlier versions were migrated automatically on upgrade.

## Custom providers

For a self-hosted model (vLLM, LiteLLM, LM Studio, a llama.cpp server) or a gateway, add a custom provider in Settings → Providers → **+ Custom provider**:

1. **Connection**: a name, an id (agents refer to the provider by it; it can't change once saved), the protocol (**OpenAI-compatible** or **Anthropic**), the base URL, an optional API key and optional extra headers. **Test connection** checks it before saving.
2. **Models**: **Fetch from server** lists the server's models; edit each row's context window, output limit and capabilities. Anything left blank is filled from the catalogue when a model with the same id is known there (marked ⓒ). Rows you edit are kept when you fetch again.

API: `GET|POST /api/v1/providers/custom`, `PUT|DELETE /api/v1/providers/custom/{id}`, `POST /api/v1/providers/custom/{id}/fetch-models`, `POST /api/v1/providers/custom/test`. Keys and header values are write-only.

## Built-in providers

| Id | Credentials | Env vars | Notes |
|----|-------------|----------|-------|
| `ollama` | `base_url` | `OLLAMA_BASE_URL` | Local [Ollama](https://ollama.com), default `http://localhost:11434`; its models are listed live |
| `llama-cpp` | `base_url` | `LLAMA_CPP_BASE_URL` | Local [llama.cpp](https://github.com/ggml-org/llama.cpp) server, default `http://localhost:8080` |
| `chatgpt` | `access_token`, `account_id` | `CHATGPT_ACCESS_TOKEN`, `CHATGPT_ACCOUNT_ID`, `CHATGPT_API_BASE` | ChatGPT subscription session (Codex-style), not an API key |
| `elevenlabs` | `api_key` | `ELEVENLABS_API_KEY` | TTS/STT only |
| `voyageai` | `api_key` | `VOYAGE_API_KEY` | Embeddings only |
| `hyperbolic` | `api_key` | `HYPERBOLIC_API_KEY` | TTS and image generation |
| `dummyplug` | `base_url` | `DUMMYPLUG_URL` | Local test server; default `http://127.0.0.1:9998/v1` |

Every other provider comes from the catalogue.

## Ids from earlier versions

Provider ids now follow the catalogue. The old names are still accepted everywhere (API, agent configs) and are normalised on read, and the environment variables Vizier used before are still honoured:

| Old name | Id now | Old env vars still read |
|----------|--------|-------------------------|
| `gemini` | `google` | `GEMINI_API_KEY` |
| `mimo` | `xiaomi` | `XIAOMI_MIMO_API_KEY` |
| `moonshot` | `moonshotai` (or `moonshotai-cn` for `api.moonshot.cn`) | `MOONSHOT_API_KEY` |
| `zai` | `zai` (or `zhipuai` for `open.bigmodel.cn`) | `ZAI_API_KEY` |
| `minimax` | `minimax` (or `minimax-cn`) | `MINIMAX_API_KEY` |
| `together` | `togetherai` | `TOGETHER_API_KEY` |
| `copilot` | `github-copilot` | `COPILOT_API_KEY` |
| `opencode_zen` | `opencode` | `OPENCODE_ZEN_API_KEY` |
| `opencode_go` | `opencode-go` | `OPENCODE_GO_API_KEY` |
| `llama_cpp` | `llama-cpp` | `LLAMA_CPP_BASE_URL` |
| `azure` | `azure` | `AZURE_API_KEY`, `AZURE_ENDPOINT` |
| `custom` | the custom provider `custom` | `CUSTOM_BASE_URL`, `CUSTOM_API_KEY` |
| `galadriel`, `mira` | **removed** | — |

On upgrade, saved credentials and agents are rewritten to the new ids once. An agent on a removed provider shows `provider "galadriel" is not available; pick another provider` until it's moved.

## Dummyplug (testing)

`dummyplug` is a local Node.js OpenAI-compatible chat server for exercising agents, channels, sessions and tools without a live model. Start it with `just dummyplug` (requires Node.js 22), then set an agent's provider to `dummyplug` (shown as **dummyplug (testing)**) and any model name. Its default URL is `http://127.0.0.1:9998/v1`; override it with `DUMMYPLUG_URL` or save a `base_url` through Settings → Providers. It is optional: Vizier and other providers work when the server is stopped, while dummyplug turns report a connection error.

The separate server reads the latest user message and replies by these rules, first match wins:

| Send | Reply |
|------|-------|
| `tools` (any case) | A list of every tool available to the agent, including MCP tools (named `<server>_<tool>`) |
| An exact tool name, e.g. `memory_write` | The tool's description, its required/optional arguments, and a sample request in a `json` code block |
| `{"tool": "<name>", "arguments": { ... }}` (optionally fenced) | Runs the tool through the normal tool path (hooks, history, UI), then replies with a **Tool result** (`<name>`) block holding its output. Malformed JSON or an unknown tool gets an error reply and runs nothing. |
| Anything else | 1–3 paragraphs of random lorem ipsum |

Example:

````text
> read_core
**`read_core`** — read your CORE document

```json
{
  "tool": "read_core",
  "arguments": {}
}
```

> {"tool": "read_core", "arguments": {}}
**Tool result** (`read_core`):
"# CORE\n\n## Operating Framework ..."
````

Scheduled tasks, dream cycles and other non-interactive prompts fall into the last rule, so they complete normally. Token usage is always reported as zero.

## Model limits and capabilities

An agent's context window and output limit come from the catalogue entry for its model; Settings shows them on the model card, and `vizier agent ps` shows the effective context window and where it came from. The agent's `context_window` and `max_tokens` fields (under **Advanced** in the WebUI) override them. For a model the catalogue doesn't list, Vizier asks the provider's model listing (OpenRouter, Ollama) and then falls back to detection by model name. The context window drives automatic [checkpoints](./agents.md#checkpoints).

Capabilities are used too: images are sent straight to a model that can see them (read-image needs no separate vision model), and are replaced by a short note for one that can't. Saving an agent on a model without tool calling, or with read-image on a model that can't see images, returns a warning (never an error).

## Related per-agent providers

Embedding, TTS, STT, and image generation each have their own provider choice configured **per agent**; they reuse the credentials of the matching provider here (e.g. Gemini embeddings use `google`'s key). See [Tools & Embedding](./tools-embedding.md).
