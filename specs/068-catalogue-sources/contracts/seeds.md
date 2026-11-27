# Contract: seeds and source parsing

Every source ships a seed in `crates/vizier-core/assets/catalogue/`, written by
`scripts/catalogue_update.py` (`just catalogue-update`; the release job runs it with
`--keep-on-failure`). A refresh fetches the same URLs and keeps the same fields. The seeds and the
cached rows hold the **trimmed upstream JSON**, and normalization runs in Rust on load, so one
parser serves both.

## models.dev — `api.json` (unchanged)

As spec 015. A model's `kind` comes from `modalities.output` (`text` → `Text`, `image` → `Image`,
`audio` → `Speech`). If there are none, `kind` is unknown.

## OpenRouter — `openrouter_models.json` (new) and `openrouter_media.json` (schema 2)

- `openrouter_models.json`: `GET https://openrouter.ai/api/v1/models`. It keeps, per entry: `id`,
  `name`, `context_length`, `architecture.{input_modalities,output_modalities}`, `pricing`,
  `top_provider.max_completion_tokens`, `supported_parameters`, `expiration_date`.
- `pricing` values are **strings of USD per unit token**. Normalized as `Cost { input: prompt×1e6,
  output: completion×1e6, cache_read: input_cache_read×1e6, cache_write: input_cache_write×1e6 }`.
  `audio`, `image`, `web_search`, `internal_reasoning` and `overrides` are dropped in this feature.
  An entry whose `prompt` and `completion` are both `"0"` is priced at 0, which is a real
  published price, not "unknown". A missing or unparseable `pricing` means no price.
- Facts: `tool_call = supported_parameters ∋ "tools"`, `reasoning = ∋ "reasoning"`, `input`/`output`
  modalities copied, `limit.context = context_length`, `limit.output =
  top_provider.max_completion_tokens`, `status = Deprecated` when `expiration_date` has passed.
- `openrouter_media.json` schema 2: each speech, transcription and image model gains `price` (a
  normalized `Price`). Speech and transcription prices come from the same `pricing` object. Image
  prices come from `GET /api/v1/images/models/{id}/endpoints`, taking the endpoint with the lowest
  `output_image` rate. `billable: input_text` → `input`, `output_image` → `output` (per 1M when
  `unit: "token"`), and `unit: "image"` → `per_image.default`. The seed script fetches those
  endpoints 4 at a time.

## Venice — `venice.json` (new)

- `GET https://api.venice.ai/api/v1/models?type=text|image|tts`, stored as `{schema: 1, fetched_at,
  text: [...], image: [...], tts: [...]}`. It keeps `id`, `type`, `context_length`,
  `model_spec.{name, availableContextTokens, maxCompletionTokens, capabilities, pricing,
  deprecation}`.
- Units, from Venice's pricing page and checked against `venice-sd35` = $0.01 per image:
  - text: `pricing.{input,output,cache_input,cache_write}.usd` are USD per 1M tokens → `Cost`.
    `extended` is dropped.
  - image: `pricing.generation.usd` → `per_image.default`. `pricing.resolutions.{1K,2K,4K}.usd` →
    `variants`, and a `quality` level → variant `{res}:{quality}`. With no `generation`, the lowest
    tier is the default. `upscale` is dropped.
  - tts: `pricing.input.usd` → `per_million_characters`.
- Facts (text): `tool_call = capabilities.supportsFunctionCalling`, `reasoning =
  supportsReasoning`, image input from `supportsVision`, `limit.context = availableContextTokens`
  or `context_length`, `limit.output = maxCompletionTokens`.

## Curated — `assets/media-prices.json` (format unchanged)

As spec 065 `contracts/media-prices.md`. Additionally, `kind: "text"` entries are allowed,
priced with `tokens`. Each figure still names `source` and `as_of`.

## Build

`build.rs` checks the syntax of `openrouter_models.json` and `venice.json`, as it does for
`openrouter_media.json` (`cargo:rerun-if-changed` each). Only `api.json` is gzipped, since the new
files are small after trimming. A unit test per source parses its committed seed, asserts a
non-zero model count, and asserts that one known model's price is correct.
