# Contract: OpenRouter Media

**Feature**: `028-openrouter-media-provider` | **Plan**: [../plan.md](../plan.md)

How Vizier calls OpenRouter for speech, transcription and images, and how the model lists/seed are obtained. Base URL: `https://openrouter.ai/api/v1` (the existing `media_base_url("openrouter")`). All requests use the one saved `openrouter` credential as a bearer token unless stated otherwise.

## 1. Speech (text-to-speech)

**Endpoint**: `POST {base}/audio/speech`

**Request** (JSON):

```json
{
  "model": "hexgrad/kokoro-82m",
  "input": "Hello from Vizier.",
  "voice": "af_alloy",
  "speed": 1.0,
  "response_format": "mp3"
}
```

- `model`, `input`, `voice` are required by Vizier (the default voice is always supplied).
- `response_format` is always `mp3` because `mp3_to_wav` only decodes MP3. OpenRouter defaults to `pcm`.
- `speed` is omitted when it is exactly `1.0`; a configured non-default value is sent. Providers without speed control ignore it or reject only non-default values (OpenRouter docs).
- No `instructions`, `input_references` or multi-speaker input in this feature.

**Response**: raw audio bytes, `Content-Type: audio/mpeg`. `OpenAiTts` converts them to WAV for session files.

**Errors**: JSON (or text) error body; `http::send` + `dialect::error_in_body` surface the provider explanation as `VizierError`. Unknown voice/model, unsupported speed and insufficient credit follow the same path.

**Default**: model `hexgrad/kokoro-82m`, voice `af_alloy`. The previous default `openai/gpt-4o-mini-tts-2025-12-15` is retired from the live speech list (2026-10-24) and must not be used.

**Known formats**: HeyGen Voice and Gemini TTS return only `pcm`; MiniMax returns only `mp3`. Kokoro accepts `mp3`. If a future default is PCM-only, the implementation must add a PCM→WAV path before switching.

## 2. Transcription (speech-to-text)

**Endpoint**: `POST {base}/audio/transcriptions`

**Request**: OpenAI-style `multipart/form-data`:

| Part | Required | Value |
|---|---|---|
| `file` | yes | audio bytes with the session filename (extension carries the format) |
| `model` | yes | e.g. `openai/whisper-large-v3` |
| `language` | no | ISO-639-1 code (`en`, `ja`, …); omitted = auto-detect |
| `response_format` | no | omitted = `json` |

The existing `OpenAiStt` sends exactly the first three. Multipart is documented as OpenAI-compatible. The base64 JSON form (`input_audio`) is not used.

**Limits**: 25 MB per multipart upload. Vizier does not pre-check; an oversized clip returns the provider's error.

**Response**: JSON `{ "text": "...", "usage": {...}? }`. `OpenAiStt` returns `text`. An empty transcript is returned as an empty string, not converted to an error.

**Errors**: unsupported format, empty/corrupt audio, oversized clip, unknown model, rate limit and credit failures surface the provider message.

**Default**: `openai/whisper-large-v3`.

## 3. Image generation

**Endpoint**: `POST {base}/images` (not `/images/generations`).

**Request** (JSON):

```json
{
  "model": "bytedance-seed/seedream-4.5",
  "prompt": "a red panda astronaut, studio lighting",
  "size": "1024x1024"
}
```

- `size` is optional. It comes from the `image_generate` call's `size` argument; when the call omits it, the agent's configured `image_gen_settings.size` is the fallback, and when both are empty it is omitted so OpenRouter chooses. The string is passed through as configured: explicit pixels (`WxH`) or a tier (`2K`); OpenRouter normalizes it per provider. Do **not** rewrite a malformed value to `256x256` (let OpenRouter explain it).
- Do **not** send `response_format`; it is an OpenAI `/images/generations` field and is not part of the OpenRouter `/images` schema.
- Single prompt, single result; no reference images, streaming, `n` or provider routing.

**Response**:

```json
{
  "created": 1748372400,
  "data": [
    { "b64_json": "<base64>", "media_type": "image/png" }
  ],
  "usage": { "prompt_tokens": 0, "completion_tokens": 4175, "total_tokens": 4175, "cost": 0.04 }
}
```

- Decode `data[0].b64_json`.
- Read `data[0].media_type` when present; it is present whenever identifiable, including `image/png`, and may be `image/jpeg`, `image/webp` or `image/svg+xml`.
- Fall back to `image/png` only when `media_type` is absent.
- If `data[0].url` is returned (not expected for OpenRouter but supported by the shared client), download it and use its `Content-Type`.
- The tool stores the returned MIME in the session file and chooses the default filename extension from it:
  - `image/png` → `.png`
  - `image/jpeg` / `image/jpg` → `.jpg`
  - `image/webp` → `.webp`
  - `image/svg+xml` → `.svg`
  - unknown → `.png`
- An explicit tool `filename` is respected as-is.

**Errors**: unknown model, unsupported size, rejected prompt, credit/rate/timeout errors surface the provider message. A missing `data[0]` is a `VizierError("image generation: missing image data")`.

**Default**: `bytedance-seed/seedream-4.5` (live; supports 1K/2K/4K and aspect ratios).

## 4. Model discovery

All three endpoints are public on 2026-10-24; no credential is sent on the first attempt. If the response is `401`/`403`, retry once with the saved/env OpenRouter key; if none is available, record the failure and keep the seed/cache. Timeout: 30 s.

### Speech

`GET {base}/models?output_modalities=speech`

```json
{
  "data": [
    {
      "id": "hexgrad/kokoro-82m",
      "name": "hexgrad: Kokoro 82M",
      "supported_voices": ["af_alloy", "af_aoede", "..."],
      "architecture": { "output_modalities": ["speech"] }
    }
  ]
}
```

Map to `MediaModel { id, name, voices, aspect_ratios: [], resolutions: [] }`.

### Transcription

`GET {base}/models?output_modalities=transcription`

Same shape; `supported_voices` is normally `null`. Map to `MediaModel` with empty arrays.

### Image

`GET {base}/images/models`

```json
{
  "data": [
    {
      "id": "bytedance-seed/seedream-4.5",
      "name": "ByteDance Seed: Seedream 4.5",
      "supported_parameters": {
        "resolution": { "type": "enum", "values": ["1K", "2K", "4K"] },
        "aspect_ratio": { "type": "enum", "values": ["1:1", "16:9", "..."] }
      }
    }
  ]
}
```

Map to `MediaModel { id, name, voices: [], aspect_ratios, resolutions }`:

- `aspect_ratios` from `supported_parameters.aspect_ratio.values` when `type == "enum"`.
- `resolutions` from `supported_parameters.resolution.values` when `type == "enum"`.
- Ignore other parameters (`n`, `seed`, `input_references`, `size`); `size` is a router convenience and is not used to gate the picker.
- The general `/models?output_modalities=image` list is not used: it lacks the per-model parameter descriptors.

### Parsing/validation

- `data` must be an array; non-object entries are dropped.
- `id` must be a non-empty string after trim; duplicates: first wins.
- `name` falls back to `id`.
- Arrays keep strings only; non-strings are dropped; duplicates removed; order preserved.
- At least one valid entry is required; otherwise the check fails and the previous list stays.
- At most 2,000 entries per capability (defensive cap).

## 5. Seed

**File**: `assets/catalogue/openrouter_media.json`

```json
{
  "schema": 1,
  "fetched_at": "2026-10-24T00:00:00Z",
  "speech": [{ "id": "...", "name": "...", "voices": ["..."] }],
  "transcription": [{ "id": "...", "name": "..." }],
  "image": [{ "id": "...", "name": "...", "aspect_ratios": ["..."], "resolutions": ["..."] }]
}
```

- `schema` is `1`.
- `fetched_at` is RFC 3339 UTC.
- Missing arrays default to empty; missing `name`/arrays are tolerated.
- `build.rs` validates JSON syntax and registers `rerun-if-changed`; `src/catalogue/seed.rs` embeds it with `include_str!`.
- The seed must contain the documented default for each capability, even if a later live refresh retires it (the API also injects a missing default).

**Update recipe**: `just catalogue-update` → `python3 -I scripts/catalogue_update.py`.

Extend the script to:

1. Fetch the three URLs above with the existing `USER_AGENT`/`TIMEOUT` helpers.
2. Normalize each to the seed shape (dedupe ids, fall back names, sort? keep provider order).
3. Validate each list is non-empty (otherwise treat as failure).
4. Write `openrouter_media.json` atomically (write temp, replace) with `schema: 1` and a UTC `fetched_at` without microseconds.
5. Under `--keep-on-failure`, warn and keep the committed file on any failure; otherwise exit non-zero.
6. Update the module docstring (it currently mentions only models.dev) and the printed summary.

## 6. Credential and error contract

- One `ProviderId::OPENROUTER` credential is used by all three capabilities and chat/embeddings.
- A missing credential must produce an error naming OpenRouter and the place to add it (the existing `media::api_key` message already does: `no API key found for provider "openrouter": add one in Settings › Providers or set OPENROUTER_API_KEY`).
- Provider-side failures must be returned as tool errors with the provider's explanation; they must not abort the agent, the turn's other tool results, or scheduling.
- Media calls are not recorded in usage accounting.
- No model fallback is attempted on a provider error.

## 7. Live-check record (2026-10-24)

| Check | Result |
|---|---|
| `GET /models?output_modalities=speech` | 36 models; Kokoro present with 54 voices including `af_alloy`; old `openai/gpt-4o-mini-tts-2025-12-15` absent |
| `GET /models?output_modalities=transcription` | 28 models; `openai/whisper-large-v3` present |
| `GET /images/models` | 61 models; `bytedance-seed/seedream-4.5` present with `resolution` 1K/2K/4K and aspect ratios |
| Anonymous access | 200 for all three |
| Total raw size | ~125 KB (49,903 + 27,070 + 47,730 bytes) |

Re-run the three list checks (and the default presence checks) before release; if a default is gone, apply the R2 substitution rule in the same change.
