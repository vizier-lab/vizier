# Voice, Images & Embeddings

Everything on this page is set **per agent**, in the WebUI under the agent's **Config → Models** tab (or the `tools`, `embedding` and `indexer` fields of `PUT /api/v1/agents/{id}`). There is no global setting. Which tools an agent has is not configured here: tools that need setup (Shell, HTTP, Brave Search, Office, MCP servers) are **addons** added from the agent's integrations pages, and everything else an agent can do needs no configuration.

## What you can set

| Setting | Default | What it does |
|---------|---------|--------------|
| **Tool timeout** (`tools.timeout`) | `30m` | How long one tool call may run (`30s`, `5m`, …) |
| **Voice replies** (`tts`) | off | The agent can speak: text becomes an audio file in the conversation |
| **Voice input** (`stt`) | off | The agent can listen: audio becomes text. Also used for [live voice](./live-voice.md) and voice messages |
| **Image reading** (`read_image`) | off | A vision model describes images instead of the main model seeing them directly |
| **Image generation** (`image_gen`) | off | The agent can make images, optionally editing ones you sent |
| **Embedding model** (`embedding`) | local `all_mini_lml6_v2` | Powers semantic [memory](./memory.md) search and skill suggestions |

Each feature has a switch and its own provider and model. In the API the switch and its settings are separate fields:

```json
{
  "tools": {
    "timeout": "30m",
    "tts": true,
    "tts_settings": { "provider": "openai", "model": "tts-1", "voice": "alloy", "speed": 1.0 },
    "stt": true,
    "stt_settings": { "provider": "whisper", "language": "en" },
    "read_image": true,
    "read_image_settings": { "provider": "openai", "model": "gpt-4o-mini" },
    "image_gen": false
  }
}
```

Cloud providers use the credentials you saved under **Settings → Providers** (or the provider's environment variable); there is no separate key per feature.

## TTS providers (`tts_settings.provider`)

| Provider | Default model | Default voice | Credential |
|----------|---------------|---------------|------------|
| `kokoro` (default) | `Kokoro-82M` | `af` | none (local — runs in-process via any-tts/Candle; weights downloaded into `<workspace>/.runtime/tts/kokoro/`) |
| `openrouter` | `hexgrad/kokoro-82m` | `af_alloy` | `openrouter` key |
| `elevenlabs` | `eleven_multilingual_v2` | `pqHfZKP75CvOlQylNhV4` | `elevenlabs` key |
| `xai` | `grok-2-tts` | `default` | `xai` key |
| `hyperbolic` | `Melo-TTS` | `default` | `hyperbolic` key |
| `openai` | `tts-1` | `alloy` | `openai` provider key |

`model`, `voice`, `speed` are optional overrides. OpenRouter returns MP3, which Vizier converts to WAV; a non-default `speed` is passed through, so a speech model without speed control may reject it (the provider's explanation is surfaced and the agent stays available). OpenRouter's default pair is `hexgrad/kokoro-82m` with its published voice `af_alloy`; if you pick a different speech model, choose one of the voices **it** publishes.

## STT providers (`stt_settings.provider`)

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

## Image generation providers (`image_gen_settings.provider`)

| Provider | Default model | Credential |
|----------|---------------|------------|
| `openai` (default) | `dall-e-3` | `openai` key |
| `openrouter` | `bytedance-seed/seedream-4.5` | `openrouter` key |
| `xai` | `grok-2-image-1212` | `xai` key |
| `huggingface` | `stabilityai/stable-diffusion-xl-base-1.0` | `huggingface` key |
| `hyperbolic` | `SDXL1.0-base` | `hyperbolic` key |
| `bedrock` | `amazon.nova-canvas-v1:0` | `amazon-bedrock` connection (access key pair + region) |
| `venice` | `venice-sd35` | `venice` key |
| `together` | `black-forest-labs/FLUX.1-schnell` | `togetherai` key |

`size` is optional. OpenRouter receives it as its `size` convenience (explicit pixels such as `1792x1024`, or a tier such as `2K`) and normalizes it per model; leave it empty to let the provider choose. Bedrock receives explicit `WxH` pixels (default `1024x1024`; Nova Canvas accepts 320–4096 per side, multiples of 16, aspect 1:4–4:1, under 4,194,304 pixels; Titan Image Generator V2 follows its own size table) and surfaces the provider's refusal for an unsupported size. Venice's safety filter is off: every Venice request sends `safe_mode: false` (or `moderation: "low"` on the OpenAI-compatible endpoint, which has no `safe_mode`), so images are not blurred. For a Venice edit, `size` is a `resolution` tier (`1K`, `2K`, `4K`) or an aspect ratio (`16:9`); pixels are ignored and the shape follows the input image. For a text-only Venice request, `size` comes from its own list (`256x256`, `512x512`, `1024x1024`, `1536x1024`, `1024x1536`, `1792x1024`, `1024x1792`) and uses 1024x1024 when it is empty. Together receives `WxH` as pixel `width`/`height` (default 1024x1024); a size that is not `WxH` is left to Together's default. The saved file keeps the media type the provider actually returned (PNG, JPEG, WebP or SVG) and its default filename extension follows that type.

`reference_images` is optional: a list of session image filenames that turns the call into an image-to-image edit or style transfer. An image the user sent, one from `read_image_file`, or a previous `image_generate` result all work, and order is meaningful (the first image is the primary subject). Support by provider:

| Provider | Reference images | Notes |
|----------|------------------|-------|
| `openrouter` | up to the model's limit | Sent as `input_references`; models that do not accept them return their own error |
| `openai` | up to 16 | Sent to `/images/edits`, which only `gpt-image-*` models serve; `dall-e-3` cannot edit |
| `xai` | one | Sent to `/images/edits`; the output keeps the source image's aspect ratio |
| `bedrock` | up to 5 | Sent as an `IMAGE_VARIATION` task; PNG and JPEG only |
| `venice` | up to the model's limit (16 at most) | With an edit model (`…-edit`, such as `nano-banana-2-edit` or `seedream-v5-5-edit`): one image goes to `/image/edit`, several to `/image/multi-edit` with the first as the base. With any other model the images become `style_references` on `/image/generate`, which only `krea-v2-large`, `krea-v2-medium`, `luma-uni-1` and `luma-uni-1-max` accept; the others return Venice's error |
| `huggingface`, `hyperbolic`, `together` | none | The call fails with a clear error rather than ignoring the images |

A reference whose stored media type is not `image/*` is refused before any provider call.

## Choosing OpenRouter media models

When a capability's provider is `openrouter`, the WebUI model field is a searchable list of that capability's current models, with the documented default marked first. The list comes from the embedded seed on a fresh or offline install and is refreshed lazily from OpenRouter's public lists when a person opens it (never at startup, on agent build, or during a turn; the same `catalogue.refresh` / `catalogue.ttl` policy as chat models applies, with a manual refresh for people who manage providers). A typed model id is always accepted, so a missing or unreachable list never blocks saving. For text-to-speech, the voice field offers the selected model's published voices and still accepts a typed voice.

## `read_image_settings`

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

> **Vector dimensions.** A `vec0` table fixes the width of the vectors it holds, so the workspace keeps **one vector table per size**: the first model's table keeps the name `document_index` (an existing install keeps its vectors), and each other width gets `document_index_d<size>`. Agents whose embedding models return the same size share a table; agents with different sizes use different tables, so they no longer block each other. When an agent's model changes to one of another width, its stored passage coordinates are dropped on its next start and its corpus is re-indexed into the new table in the background — semantic search for that agent is empty until that pass finishes. The old table keeps the rows of the agents still on its size. Embeddings are generated when a memory is written or imported (not re-generated on boot), apart from that one automatic re-index.

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

All supported models return 1024-dimensional vectors, so they share one index table. There is no dimension setting. A model id outside these families fails the agent's build, naming the model and the supported families, before any request.

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

