# 2.5 Tools & Embedding

Everything on this page is configured **per agent** — in the WebUI agent settings, or through the `tools`, `embedding`, and `indexer` fields of `POST|PUT /api/v1/agents/{id}`. There is no global tool configuration.

## Always-on tools

Every agent gets these regardless of configuration:

| Tool | Description |
|------|-------------|
| `think` | Scratchpad for reasoning |
| `READ_CORE` / `WRITE_CORE` | Read / overwrite the agent's `CORE.md` (see [Agents](./agents.md#coremd)) |
| `schedule_one_time_task`, `schedule_cron_task`, `list_task`, `get_task_detail`, `delete_task` | Scheduler — tasks run as the agent in a dedicated `Task` session |
| `consult_agent` | Ask another agent and wait for its reply |
| `delegate_agent` | Hand a task to another agent |
| `paralel_subtasks` | Run several sub-prompts in parallel (sub-agents of the same agent) |
| `create_skill`, `update_skill`, `delete_skill`, `list_skills`, `get_skill_details`, `use_skill`, `read_skill_resource`, `execute_skill_resource` | Skills — see [Skills](./skills.md) |
| `list_session_files`, `read_document_file`, `read_image_file`, `send_attachment` | Session files — uploaded/attached files; documents (txt/md/json/yaml/csv/html/pdf/docx/xlsx…) are extracted to text |
| `webui_send_message`, `webui_list_topics` | Push messages into WebUI topics |
| `memory_*` (8 tools) | Present whenever `embedding` + `indexer` are set (the API default) — see [Memory](./memory.md) |

## Configurable tools (`tools`)

The `tools` object in the agent API:

```json
{
  "tools": {
    "timeout": "30m",
    "shell": { "environment": "local", "path": "/home/me/project" },
    "brave_search": true,
    "brave_search_settings": { "api_key": "BSA…", "safesearch": true },
    "fetch": true,
    "http_client": true,
    "discord": false,
    "telegram": false,
    "mcp_servers": {
      "filesystem": { "host": "local", "command": "npx", "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"] }
    },
    "tts": false,
    "tts_settings": { "provider": "openai", "model": "tts-1", "voice": "alloy", "speed": 1.0 },
    "stt": false,
    "stt_settings": { "provider": "whisper", "model": "large-v3", "language": "en" },
    "read_image": false,
    "read_image_settings": { "provider": "openai", "model": "gpt-4o-mini" },
    "image_gen": false,
    "image_gen_settings": { "provider": "openai", "model": "dall-e-3", "size": "1024x1024" }
  }
}
```

| Field | Type | Default | Tools it enables | Notes |
|-------|------|---------|------------------|-------|
| `timeout` | duration | `30m` | — | Per tool-call timeout (`"30s"`, `"5m"`, …) |
| `shell` | object \| null | `null` | `shell_exec` | Local or Docker shell — see [Storage & Shell](./storage-shell.md#shell) |
| `brave_search` + `brave_search_settings` | bool + `{api_key, safesearch}` | `false` | `web_search`, `news_search` | **Both** `api_key` and `safesearch` must be set or the tools are not registered |
| `fetch` | bool | `false` | `fetch` | Fetch a URL → markdown |
| `http_client` | bool | `false` | `http_client` | Arbitrary HTTP requests (GET/POST/PUT/DELETE/PATCH/HEAD/OPTIONS) with custom headers |
| `discord` | bool | `false` | `discord_*` | Requires `discord_token` on the agent |
| `telegram` | bool | `false` | `telegram_*` | Requires `telegram_token` |
| `mcp_servers` | map | `{}` | `mcp_<server>__<tool>` | See [MCP Servers](./mcp.md) |
| `tts` + `tts_settings` | bool + settings | `false` | `tts_generate` | Text → speech file in session files |
| `stt` + `stt_settings` | bool + settings | `false` | `stt_transcribe` | Audio file → text; also used to auto-transcribe `audio_chat` requests |
| `read_image` + `read_image_settings` | bool + settings | `false` | (changes `read_image_file`) | When enabled with a vision `provider` + `model`, `read_image_file` returns a text description instead of injecting the raw image |
| `image_gen` + `image_gen_settings` | bool + settings | `false` | `image_generate` | Prompt → image file in session files |

In the WebUI the boolean and its `_settings` are one form section; in the API they're separate fields (`brave_search: true` + `brave_search_settings: {...}`).

### TTS providers (`tts_settings.provider`)

| Provider | Default voice | Credential |
|----------|---------------|------------|
| `openai` (default) | `alloy` | `openai` provider key |
| `openrouter` | `alloy` | `openrouter` key |
| `elevenlabs` | `pqHfZKP75CvOlQylNhV4` | `elevenlabs` key |
| `xai` | `default` | `xai` key |
| `hyperbolic` | `default` | `hyperbolic` key |
| `kokoro` | `af` | none (local) |

`model`, `voice`, `speed` are optional overrides.

### STT providers (`stt_settings.provider`)

| Provider | Default model | Credential |
|----------|---------------|------------|
| `openai` (default) | `whisper-1` | `openai` key |
| `elevenlabs` | `scribe_v1` | `elevenlabs` key |
| `groq` | `whisper-large-v3` | `groq` key |
| `mistral` | `voxtral-mini-2507` | `mistral` key |
| `huggingface` | `openai/whisper-large-v3` | `huggingface` key |
| `gemini` | `gemini-1.5-flash` | `gemini` key |
| `whisper` | `large-v3` | none — runs locally via sherpa-onnx; model downloaded into `<workspace>/.runtime/stt/whisper/` |

`language` is optional (auto-detect when unset).

### Image generation providers (`image_gen_settings.provider`)

| Provider | Default model |
|----------|---------------|
| `openai` (default) | `dall-e-3` |
| `xai` | `grok-2-image-1212` |
| `huggingface` | `stabilityai/stable-diffusion-xl-base-1.0` |
| `hyperbolic` | `SDXL1.0-base` |

### `read_image_settings`

`provider` is any chat [provider variant](./providers.md) with a vision-capable `model`. If `read_image` is enabled but provider/model are missing, the tool falls back to raw-image injection with a warning.

## Embedding (`embedding`)

Each agent has its own embedding model, used for semantic memory search and skill recommendation.

```json
{
  "embedding": {
    "provider": "local",
    "model": "all_mini_lml6_v2",
    "api_key": null,
    "base_url": null
  },
  "indexer": { "kind": "sqlite" }
}
```

Defaults when omitted on create: `local` / `all_mini_lml6_v2` and `sqlite` indexer. If either `embedding` or `indexer` is `null`, the agent has **no** vector index — memory tools and skill recommendation are disabled.

> **Vector dimension caveat.** Embeddings are generated when a memory is written or imported (not re-generated on boot). The `document_index` vector table in the workspace database is created once, with the dimension of the first embedding model that touches it, and is shared by every agent in the workspace. Agents using a model with a different dimension will fail to index (the error is logged and the write to markdown still succeeds, but semantic search won't find it). Keep all agents in a workspace on same-dimension embedding models, or reset the index if you switch.

### Embedding providers

| `provider` | `model` example | Notes |
|------------|-----------------|-------|
| `local` (default) | `all_mini_lml6_v2` | Runs in-process via [fastembed](https://github.com/Anush008/fastembed-rs); model files download to the workspace on first use |
| `ollama` | `nomic-embed-text` | Uses the `ollama` provider's `base_url` (override with `base_url`) |
| `openai` | `text-embedding-3-small` | |
| `gemini` | `text-embedding-004` | |
| `openrouter` | `openai/text-embedding-3-small` | |
| `voyageai` | `voyage-3` | |
| `mistral` | `mistral-embed` | |
| `together` | `togethercomputer/m2-bert-80M-8k-retrieval` | |
| `cohere` | `embed-english-v3.0` | |
| `copilot` | `text-embedding-3-small` | |

Cloud providers resolve credentials from the providers table → env var (see [Providers](./providers.md#credential-resolution)); `api_key` on the embedding config is an optional per-agent override.

### Local models

`GET /api/v1/embedding-models/local` lists them at runtime. Sizes are approximate.

| Model | Profile |
|-------|---------|
| `all_mini_lml6_v2` / `all_mini_lml6_v2q` | lightweight (default) |
| `all_mini_lml12_v2` / `all_mini_lml12_v2q` | performance / balanced |
| `bge_small_env15` / `bge_small_env15q` | lightweight |
| `bge_base_env15` / `bge_base_env15q` | balanced |
| `bge_large_env15` / `bge_large_env15q` | performance / balanced |
| `bge_small_zh_v15`, `bge_large_zh_v15` | Chinese |
| `nomic_embed_text_v1`, `nomic_embed_text_v15`, `nomic_embed_text_v15q` | balanced |
| `paraphrase_ml_mini_lml12_v2` (+`q`), `paraphrase_ml_mpnet_base_v2` | multilingual |
| `multilingual_e5_small` / `_base` / `_large` | multilingual |
| `mxbai_embed_large_v1` / `mxbai_embed_large_v1q` | performance / balanced |
| `gte_base_env15` (+`q`), `gte_large_env15` (+`q`) | balanced / performance |
| `modernbert_embed_large` | performance |
| `clip_vit_b32` | image/text |
| `jina_embeddings_v2_base_code` | code |

`q` suffix = quantized (smaller, faster, slightly lower quality).

## Indexer (`indexer`)

| `kind` | Description |
|--------|-------------|
| `sqlite` (default, only option) | Vector index in the workspace SQLite database via `sqlite-vec` |

## Dream-cycle tool subset

During a [dream cycle](./agents.md#dream-cycle) the agent runs with a restricted toolset: the 8 `memory_*` tools, `READ_CORE`/`WRITE_CORE`, the 4 scheduler tools, `create_skill`/`update_skill`/`list_skills`/`get_skill_details`/`use_skill`, plus `read_dream_journal`/`write_dream_journal`.
