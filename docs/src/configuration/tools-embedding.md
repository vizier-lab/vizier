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
| `parallel_subagents` | Run several sub-prompts in parallel and wait for their results in the current turn |
| `background_subagents` | Run several sub-prompts in parallel in the background; results arrive later as a background report |
| `create_skill`, `update_skill`, `delete_skill`, `list_skills`, `get_skill_details`, `use_skill`, `read_skill_resource`, `execute_skill_resource` | Skills — see [Skills](./skills.md) |
| `list_session_files`, `read_document_file`, `read_image_file` | Session files — transient uploaded/attached files; documents (txt/md/json/yaml/csv/html/pdf/docx/xlsx…) are extracted to text |
| `promote_artifact`, `update_artifact`, `list_artifacts`, `load_artifact_as_session_file`, `send_attachment` | Permanent artifacts — copy a session file in, replace its content, list them, load one back as a session file, or send one with a reply |
| `attach_artifact_to_memory`, `detach_attachment_from_memory` | Attach or remove one artifact on a memory (memory_write never changes attachments) |
| `write_tmp`, `append_tmp`, `read_tmp` | Scratchpad — temporary notes private to the conversation, see [below](#scratchpad) |
| `webui_send_message`, `webui_list_topics` | Push messages into WebUI topics |
| `memory_*` (8 tools) | Present whenever `embedding` + `indexer` are set (the API default) — see [Memory](./memory.md) |

### Scratchpad

The `scratchpad` category holds named plain-text files private to one conversation, for working notes, plans and output built up across tool calls:

- `write_tmp(name, content)` creates a file or replaces its content.
- `append_tmp(name, content)` adds to the end exactly as given (no separator), creating the file if absent.
- `read_tmp(name)` returns the content, or `not_found` with the names that do exist.

**Lifetime.** A conversation's scratch files last while it has a turn running or queued, so back-to-back messages share them. They are deleted once it goes idle, on `/abort`, when the agent stops with nothing queued, when the conversation or agent is deleted, and on server restart. Use memory for anything that must last.

**Limits.** Names are 1–128 characters of letters, digits, `-`, `_` and `.`. Each file holds up to 1 MiB, and a conversation up to 64 files. A write over a limit is refused and changes nothing; refusals and missing files come back as results with a `status`, never as errors.

Files live in the `scratch_file` SQLite table and are never written to memory, history or usage. The dream cycle gets the three tools too; each dream stage has its own scratchpad.

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
    "stt_settings": { "provider": "whisper", "model": "ggml-large-v3", "language": "en" },
    "read_image": false,
    "read_image_settings": { "provider": "openai", "model": "gpt-4o-mini" },
    "image_gen": false,
    "image_gen_settings": { "provider": "openai", "model": "dall-e-3", "size": "1024x1024" },
    "python": { "enabled": true, "code_mode": false }
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
| `python` | `{enabled, code_mode}` | both `false` | `execute_python` (+ `list_tool_functions`, `describe_tool_function` in code mode) | See [Python sandbox & code mode](#python-sandbox--code-mode). `code_mode: true` without `enabled: true` is rejected with 400 |

In the WebUI the boolean and its `_settings` are one form section; in the API they're separate fields (`brave_search: true` + `brave_search_settings: {...}`).

### TTS providers (`tts_settings.provider`)

| Provider | Default model | Default voice | Credential |
|----------|---------------|---------------|------------|
| `kokoro` (default) | `Kokoro-82M` | `af` | none (local — runs in-process via any-tts/Candle; weights downloaded into `<workspace>/.runtime/tts/kokoro/`) |
| `openrouter` | `hexgrad/kokoro-82m` | `af_alloy` | `openrouter` key |
| `elevenlabs` | `eleven_multilingual_v2` | `pqHfZKP75CvOlQylNhV4` | `elevenlabs` key |
| `xai` | `grok-2-tts` | `default` | `xai` key |
| `hyperbolic` | `Melo-TTS` | `default` | `hyperbolic` key |
| `openai` | `tts-1` | `alloy` | `openai` provider key |

`model`, `voice`, `speed` are optional overrides. OpenRouter returns MP3, which Vizier converts to WAV; a non-default `speed` is passed through, so a speech model without speed control may reject it (the provider's explanation is surfaced and the agent stays available). OpenRouter's default pair is `hexgrad/kokoro-82m` with its published voice `af_alloy`; if you pick a different speech model, choose one of the voices **it** publishes.

### STT providers (`stt_settings.provider`)

| Provider | Default model | Credential |
|----------|---------------|------------|
| `openai` | `whisper-1` | `openai` key |
| `openrouter` | `openai/whisper-large-v3` | `openrouter` key (the same one chat and embeddings use) |
| `elevenlabs` | `scribe_v1` | `elevenlabs` key |
| `groq` | `whisper-large-v3` | `groq` key |
| `mistral` | `voxtral-mini-2507` | `mistral` key |
| `huggingface` | `openai/whisper-large-v3` | `huggingface` key |
| `gemini` | `gemini-1.5-flash` | `google` key |
| `whisper` (default) | `ggml-small` | none — runs locally via whisper-rs; GGML model downloaded into `<workspace>/.runtime/stt/whisper/` |

`language` is optional (auto-detect when unset). OpenRouter caps multipart uploads at 25 MB; an oversized clip comes back as the provider's error.

### Image generation providers (`image_gen_settings.provider`)

| Provider | Default model | Credential |
|----------|---------------|------------|
| `openai` (default) | `dall-e-3` | `openai` key |
| `openrouter` | `bytedance-seed/seedream-4.5` | `openrouter` key |
| `xai` | `grok-2-image-1212` | `xai` key |
| `huggingface` | `stabilityai/stable-diffusion-xl-base-1.0` | `huggingface` key |
| `hyperbolic` | `SDXL1.0-base` | `hyperbolic` key |
| `bedrock` | `amazon.nova-canvas-v1:0` | `amazon-bedrock` connection (access key pair + region) |

`size` is optional. OpenRouter receives it as its `size` convenience (explicit pixels such as `1792x1024`, or a tier such as `2K`) and normalizes it per model; leave it empty to let the provider choose. Bedrock receives explicit `WxH` pixels (default `1024x1024`; Nova Canvas accepts 320–4096 per side, multiples of 16, aspect 1:4–4:1, under 4,194,304 pixels; Titan Image Generator V2 follows its own size table) and surfaces the provider's refusal for an unsupported size. The saved file keeps the media type the provider actually returned (PNG, JPEG, WebP or SVG) and its default filename extension follows that type.

### Choosing OpenRouter media models

When a capability's provider is `openrouter`, the WebUI model field is a searchable list of that capability's current models, with the documented default marked first. The list comes from the embedded seed on a fresh or offline install and is refreshed lazily from OpenRouter's public lists when a person opens it (never at startup, on agent build, or during a turn; the same `catalogue.refresh` / `catalogue.ttl` policy as chat models applies, with a manual refresh for people who manage providers). A typed model id is always accepted, so a missing or unreachable list never blocks saving. For text-to-speech, the voice field offers the selected model's published voices and still accepts a typed voice.

### `read_image_settings`

`provider` is any chat [provider variant](./providers.md) with a vision-capable `model`. If `read_image` is enabled but provider/model are missing, the tool falls back to raw-image injection with a warning.

## Embedding (`embedding`)

Each agent has its own embedding model, used for semantic memory search and skill recommendation.

```json
{
  "embedding": {
    "provider": "local",
    "model": "all_mini_lml6_v2"
  },
  "indexer": { "kind": "sqlite" }
}
```

Defaults when omitted on create: `local` / `all_mini_lml6_v2` and `sqlite` indexer. If either `embedding` or `indexer` is `null`, the agent has **no** vector index — memory tools and skill recommendation are disabled.

> **Vector dimension caveat.** Embeddings are generated when a memory is written or imported (not re-generated on boot). The `document_index` vector table in the workspace database is created once, with the dimension of the first embedding model that touches it, and is shared by every agent in the workspace. When an agent is built with a model whose vectors have a different size, the build is refused with the index's dimension, the model's dimension and the reindex instruction, and no wrong-sized vectors are written. Keep all agents in a workspace on same-dimension embedding models, or rebuild the index when you switch.

### Embedding providers

| `provider` | `model` example | Notes |
|------------|-----------------|-------|
| `local` (default) | `all_mini_lml6_v2` | Runs in-process via [fastembed](https://github.com/Anush008/fastembed-rs); model files download to the workspace on first use |
| `ollama` | `nomic-embed-text` | Uses the `ollama` provider's `base_url` |
| `openai` | `text-embedding-3-small` | |
| `gemini` | `text-embedding-004` | Uses the `google` key |
| `openrouter` | `openai/text-embedding-3-small` | |
| `voyageai` | `voyage-3` | |
| `mistral` | `mistral-embed` | |
| `together` | `togethercomputer/m2-bert-80M-8k-retrieval` | Uses the `togetherai` key |
| `cohere` | `embed-english-v3.0` | |
| `copilot` | `text-embedding-3-small` | |
| `bedrock` | `amazon.titan-embed-text-v2:0` (default when empty) | Uses the `amazon-bedrock` connection; see below |

Cloud providers use the credentials of the matching provider in Settings → Providers, or its environment variable (see [Providers](./providers.md#connecting-a-provider)). There is no per-agent embedding key; an `api_key`/`base_url` saved on an older agent is ignored.

### Amazon Bedrock

Bedrock embeddings and image generation reuse the one `amazon-bedrock` connection (access key pair and region) described in [Providers](./providers.md#amazon-bedrock). Leave the model empty to use the documented default; a typed id is stored and sent unchanged. A cross-region inference profile prefix (`us.`, `eu.`, `apac.`, `global.`) is accepted and ignored for family detection.

**Embeddings:**

| Family | Model ids | Request | Dimension |
|---|---|---|---|
| Amazon Titan Text Embeddings V2 (default) | `amazon.titan-embed-text-v2:0` | one text per call, `normalize: true` | 1024 |
| Amazon Titan Text Embeddings V1 | `amazon.titan-embed-text-v1` | one text per call | 1024 |
| Cohere Embed v3 | `cohere.embed-english-v3`, `cohere.embed-multilingual-v3` | batches of up to 96 texts, `input_type: search_document` | 1024 |

All supported models return 1024-dimensional vectors, so they share one index. There is no dimension setting. A model id outside these families fails the agent's build, naming the model and the supported families, before any request. A model whose vectors differ from the existing index is refused with the reindex instruction above.

**Image generation:**

| Family | Model ids | Default size |
|---|---|---|
| Amazon Nova Canvas (default) | `amazon.nova-canvas-v1:0` | `1024x1024` |
| Amazon Titan Image Generator V2 | `amazon.titan-image-generator-v2:0` | `1024x1024` |

The tool asks for one image per call. A blocked prompt or a response with no image fails the tool call with the provider's message and saves no file. The media type is read from the bytes (PNG, JPEG or WebP), so the filename extension matches what was returned.

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

## Python sandbox & code mode

Two per-agent switches under `tools.python`, both off by default:

- **`enabled`: the sandbox.** Adds `execute_python`, which runs a Python script in an in-process [monty](https://github.com/pydantic/monty) interpreter. There is no filesystem, network, environment or OS access, and nothing to install. Each run is stateless. The value of the script's last expression comes back as `result`, and `print()` output comes back as `stdout`. The agent's other tools are still called directly as usual.
- **`code_mode`: programmatic tool calling** (requires `enabled`). The agent's tools become plain functions inside scripts (`hits = memory_read(query="…")`) and are hidden from the model's direct tool list. The model then sees only `think`, `execute_python`, `list_tool_functions` and `describe_tool_function`. Nested calls go through the same dispatch, hooks and per-tool timeout as direct calls, so code mode adds no capability. Only the script's final `result`/`stdout` enters the model's context.

Every run returns an execution report: `{ ok, result, stdout, error: {kind, message, traceback, limit}, tool_calls: [...], duration_ms }`. The report is stored as the `execute_python` tool result in session history and streamed live to the WebUI.

**Limits.** The agent's `tools.timeout` is the only bound. It covers the whole script, including every tool call it makes. Other fixed limits: recursion depth 1000, script size 64 KiB, any single allocation over 1 GiB rejected, and `print()` output up to 10 MiB. Deliberately **not** in this version: a cumulative memory ceiling (memory is released when each run ends, and a run's peak is bounded only by the timeout), a cap on tool calls per script, and output truncation.

The sandbox tools are never part of the dream cycle's toolset.

## Dream-cycle tool subset

During a [dream cycle](./agents.md#dream-cycle) the agent runs with a restricted toolset: the 8 `memory_*` tools, `READ_CORE`/`WRITE_CORE`, the 4 scheduler tools, `create_skill`/`update_skill`/`list_skills`/`get_skill_details`/`use_skill`, `write_tmp`/`append_tmp`/`read_tmp`, plus `read_dream_journal`/`write_dream_journal`.
