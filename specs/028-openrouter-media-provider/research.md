# Research: OpenRouter Media Provider

**Feature**: `028-openrouter-media-provider` | **Researched**: 2026-10-24/25 | **Plan**: [plan.md](./plan.md)

All provider facts below were checked against OpenRouter's public API and documentation on **2026-10-24**. Live model lists change; the seed and the documented defaults must be re-checked before release (see R8).

## R1 — One credential, resolved through the existing media endpoint

**Decision**: OpenRouter media uses the same `ProviderId::OPENROUTER` credential as chat and embeddings. The factories call the existing `llm::media::endpoint(deps, &ProviderId::OPENROUTER, model)`, which reads the saved provider row first and then the catalogue/env variables (`OPENROUTER_API_KEY`). No new setting, secret or per-capability key is added.

**Rationale**: FR-002 and SC-003 require exactly one credential. `endpoint()` already produces the right base URL (`https://openrouter.ai/api/v1`) and a bearer `StaticKey`; using it keeps credential precedence, redaction and restart-on-change behavior identical to every other media provider. The provider page already counts media usage (`providers/usage.rs`), and `restart_using` already restarts agents whose media provider uses the changed credential, so FR-012 needs no code.

**Alternatives rejected**:
- A separate media key or account: violates the one-credential promise and duplicates secret handling.
- Reading `OPENROUTER_API_KEY` directly in the factories: bypasses saved Settings credentials and their restart behavior.
- A new `OpenRouterEndpoint`: duplicates auth, base URL and error handling for no gain.

**Error path**: a missing key already yields `no API key found for provider "openrouter": add one in Settings › Providers or set OPENROUTER_API_KEY`, which satisfies FR-009 without a new message.

## R2 — Defaults: keep two, replace the retired TTS model

**Decision**:

| Capability | Default model | Default voice | Live on 2026-10-24? |
|---|---|---|---|
| Speech (`tts`) | `hexgrad/kokoro-82m` | `af_alloy` | yes (36 speech models) |
| Transcription (`stt`) | `openai/whisper-large-v3` | — | yes (28 transcription models) |
| Image (`image_gen`) | `bytedance-seed/seedream-4.5` | — | yes (61 image models) |

The previous TTS default `openai/gpt-4o-mini-tts-2025-12-15` is **not in the live speech list** (OpenRouter still shows it in an old docs example, which is why the spec's assumption was wrong). `alloy` is an OpenAI voice and is not published for Kokoro; the matching voice is `af_alloy`. `TtsProvider::default_voice()` must therefore stop sharing one value between OpenAI and OpenRouter.

**Rationale**: Kokoro is open-weight, present in the live list, cheap, and publishes a stable `af_*`/`am_*` voice naming scheme including `af_alloy`. It supports MP3 through OpenRouter (only HeyGen Voice and Gemini TTS are PCM-only; MiniMax is MP3-only), so the existing MP3→WAV path works unchanged. Keeping Whisper Large V3 and Seedream 4.5 avoids churn where the current defaults are still valid.

**Alternatives rejected**:
- `google/gemini-3.8-flash-tts`: PCM-only, so it would require a new PCM→WAV path and a different default voice (`Kore`).
- `microsoft/mai-voice-2.1-flash`: good quality but only four voices in the 2.0 line and provider-specific Azure voice ids with a model suffix; a larger behavioral change for no requirement.
- `elevenlabs/eleven-multilingual-v2`: stable but materially more expensive; no reason to make the default the priciest option.
- Keeping the retired OpenAI TTS id: fails the first acceptance scenario with an unknown-model error.

**Change surface**: `TtsProvider::default_model()`, `TtsProvider::default_voice()`, `tts/mod.rs`, `webui/app/lib/agentForm.ts` (`TTS_DEFAULT_MODELS`, `TTS_DEFAULT_VOICES`), `docs/src/configuration/tools-embedding.md`, and the spec Assumptions. The seed still contains every live model, so the old id disappears naturally after the next `just catalogue-update`.

**Fallback rule**: if a chosen default is retired after this plan, the implementation substitutes the nearest stable model of the same modality and updates model, voice, docs, seed and tests in the same change. Runtime never substitutes silently. If a live check shows Kokoro rejecting `response_format: mp3` (the docs say only HeyGen/Gemini are PCM-only, so this is not expected), either add a PCM→WAV path or choose the next MP3-capable speech model with a published voice, and update the default pair in the same change.

## R3 — STT: reuse the OpenAI multipart client

**Decision**: Add `Openrouter` to `SttProvider`; the existing `(_, Some(id)) => Arc::new(OpenAiStt(...))` arm handles it. Do not add a new client.

**Evidence**: OpenRouter documents `POST /api/v1/audio/transcriptions` with either base64 JSON (`input_audio`) or OpenAI-style multipart (`file`, `model`, optional `language`, `response_format`). Multipart uploads are capped at 25 MB, the same as OpenAI. The existing client sends exactly `model`, `file` (with filename) and optional `language`, and parses `text` from the JSON response; that is compatible.

**Rationale**: Multipart is one request, no base64 expansion, no new encoder, no MIME table. The existing filename already carries the extension, and OpenRouter accepts the common formats. A provider rejection (oversized, unsupported format, empty audio, unknown model) comes back as a JSON error body and is already turned into a `VizierError` by `error_in_body`.

**Alternatives rejected**:
- Base64 JSON: more code (encode, format detection) and 33% larger payloads, with no capability gained.
- A dedicated `OpenRouterStt`: duplicates an identical request.
- Pre-validating the 25 MB cap: the limit is provider-side and may change; surfacing the provider's message is the existing contract for every other STT provider.

**Note**: `language` remains a short code (`en`, `ja`); omission means auto-detect. An unsupported code is a provider error, not a guessed language.

## R4 — Image: one new endpoint variant, `size`, and the returned media type

**Decision**: Replace `OpenAiImages.sizing` with `ImageApi::{Generations(ImageSizing), OpenRouter}` (set by the factory). `OpenRouter`:
- posts to `/images` (base already ends in `/api/v1`);
- sends `{model, prompt}` plus `size` exactly as supplied by the call or the agent's configured fallback, only when present (no `response_format`, no `parse_size` rewrite);
- decodes `data[0].b64_json` and reads `data[0].media_type`; falls back to `image/png` only when the field is absent; for a `url` reply, keeps the existing Content-Type logic;
- returns `(bytes, mime)` to the tool, which chooses the default extension from the MIME.

`Generations(ImageSizing::Size)` preserves OpenAI's `size`/`response_format` behavior and `Generations(ImageSizing::AspectRatio(..))` preserves xAI's.

**Evidence**: OpenRouter's image API is `POST /api/v1/images`; `size` is a documented universal convenience that accepts explicit pixels (`"2048x2048"`) and is normalized per provider. `media_type` is present whenever identifiable, including non-PNG JPEG/WebP/SVG from Recraft vector models. The image-specific model list (`GET /api/v1/images/models`) reports `supported_parameters` with `resolution`/`aspect_ratio` enums; the general `/models?output_modalities=image` is thinner.

**Rationale**: This is the smallest change that satisfies FR-005/FR-006 without hardcoding `image/png` (the current client does) and without branching on a provider id in `openai_shaped.rs`. Sending `size` only when requested avoids imposing the client's 256×256 default, which many image models reject. `response_format: b64_json` is an OpenAI generations field, not an OpenRouter `/images` field; omitting it is required.

**Alternatives rejected**:
- Reusing `/images/generations`: 404 at OpenRouter.
- Always sending `size: "256x256"`: not a supported tier and likely rejected; also overrides provider defaults.
- Mapping `size` to `aspect_ratio` ourselves: loses exact dimensions and duplicates OpenRouter's normalization.
- A separate `OpenRouterImages` struct: duplicates decoding, URL download and error handling; the module's stated rule is a factory-set field, not a provider check.
- Hardcoding PNG: violates FR-006 and mislabels JPEG/WebP/SVG.

**Tool change**: `image_generate` currently defaults the filename to `img_{uuid}.png` regardless of the returned MIME. Change it to pick `.png`, `.jpg`, `.webp` or `.svg` from the MIME when no filename was supplied; an explicit filename is respected. This also fixes the existing URL-response path for OpenAI/xAI.

## R5 — TTS: keep MP3, omit the default speed

**Decision**: Keep the existing OpenRouter wiring (`OpenAiTts` with `response_format: Some("mp3")`) and the MP3→WAV conversion. Change `OpenAiTts` to omit the `speed` field when it is exactly `1.0`; send it when the person configured a non-default speed.

**Evidence**: OpenRouter's speech endpoint is OpenAI-compatible; `response_format` defaults to `pcm`, and only `mp3` is decodable by `mp3_to_wav`. HeyGen Voice and Gemini TTS are PCM-only, MiniMax is MP3-only. `speed` is honored by models that support it; others ignore it or return 400 for a **non-default** value.

**Rationale**: Kokoro supports MP3, so no new audio decoder is needed. Omitting the field at 1.0 means an unset speed never becomes a rejected parameter on a model without speed control, while an explicit value still reaches models that support it. The default voice is `af_alloy`, which is in Kokoro's published voice list. The feature passes a configured non-default speed through; OpenRouter documents that models without speed control ignore it or return 400, so the guaranteed playable path is the default speed and a non-default speed is verified only on a model that documents support (otherwise the provider's explanation is the expected result, per FR-010).

**Alternatives rejected**:
- Switching to PCM: requires a PCM header/rate path and makes the default model choice smaller.
- Sending `speed: 1.0` unconditionally: risks a 400 on models that reject the field, even when the person never set it.
- Dropping speed support for OpenRouter: loses a documented override for models that do support it.

## R6 — Discovery sources: the public model lists, not models.dev

**Decision**: Fetch:
- speech: `GET https://openrouter.ai/api/v1/models?output_modalities=speech`
- transcription: `GET https://openrouter.ai/api/v1/models?output_modalities=transcription`
- image: `GET https://openrouter.ai/api/v1/images/models`

All three are public on 2026-10-24 (no Authorization header needed). If a response is 401/403, retry once with the saved/env OpenRouter key; if no key exists, record the failure and keep the seed/cache. Never use `/models/user` (it requires a key and filters by privacy/guardrail settings, which would hide models a person could still select).

**Rationale**: `models.dev` (the source of the chat/embedding catalogue) does not carry OpenRouter's TTS/STT media entries with the fields needed here, and its image entries lack `supported_parameters`. The provider's own lists are authoritative, capability-specific and free. The image-specific endpoint is richer than the general `output_modalities=image` list.

**Alternatives rejected**:
- Bundling OpenRouter media models into `api.json`: mixes two sources and two refresh policies; models.dev lacks the data.
- Refreshing all three lists on every capability read: wasted requests; per-capability storage/refresh is simple and the API returns one capability at a time.
- Making the list fetch require a key: prevents configuring an agent before connecting the provider, and FR-020/SC-010 want the seed usable offline.
- Scraping the models page: brittle and unnecessary.

**Parsing**:
- speech/transcription: `data[]` with `id`, `name`, optional `supported_voices`.
- image: `data[]` with `id`, `name`, `supported_parameters`; keep `aspect_ratio` enum values and `resolution` enum values when present. `size` is accepted by the router as a convenience even when absent from a model's native parameters, so it is not used to gate the picker.
- Drop entries without a non-empty id; keep the provider order; preserve `name` or fall back to the id.

## R7 — Refresh policy: the existing catalogue policy, per capability

**Decision**: Reuse `catalogue.refresh` (enabled) and `catalogue.ttl` (default 24 h). Add no new config keys. Per capability:
- a read handler calls `maybe_refresh`: if refresh is on, the list is older than the TTL, no failure happened in the last hour and no check is running, spawn one background check; the read is served immediately from memory;
- a manual `POST .../refresh` ignores TTL/backoff, waits for an in-flight check (up to 30 s) or starts one, and returns the result;
- a failed check records `last_error`/`failed_at` (memory and SQLite), keeps the previous list, and suppresses checks for 1 h;
- a successful check writes the snapshot, clears the failure and swaps the in-memory list;
- startup and every non-person path never call the refresh.

**Rationale**: FR-018/FR-019 and SC-011 require the exact same visibility and no background traffic. Reusing the existing config keeps the operational surface unchanged. Per-capability TTLs avoid one bad image response blocking speech.

**Alternatives rejected**:
- A separate `catalogue.media.*` config: more knobs for no request, and operators would have to learn a second policy.
- ETag/conditional requests: the three lists are ~125 KB uncompressed and fetched at most once a day; the extra column/304 path is not justified. (A full GET is also simpler to validate.)
- Refresh on startup: violates SC-011 and slows a headless boot.
- A blocking refresh on the first read: makes the picker wait on the network and breaks the offline-first seed promise.

**Time handling**: normalize `Utc::now()` to millisecond precision before storing/writing, as the existing catalogue does, so memory and SQLite agree.

## R8 — Seed and maintenance

**Decision**: Commit `assets/catalogue/openrouter_media.json`:

```json
{
  "schema": 1,
  "fetched_at": "2026-10-24T00:00:00Z",
  "speech": [{ "id": "...", "name": "...", "voices": ["..."] }],
  "transcription": [{ "id": "...", "name": "..." }],
  "image": [{ "id": "...", "name": "...", "aspect_ratios": ["..."], "resolutions": ["..."] }]
}
```

`build.rs` validates it as JSON and declares `rerun-if-changed`; `catalogue/seed.rs` embeds it with `include_str!` (small, no gzip). The release/`just catalogue-update` script fetches and normalizes the three lists, preserves the committed file on failure (`--keep-on-failure` release behavior), and stamps `fetched_at` in UTC.

**Startup choice**: per capability, use the cached row when its schema matches and its `fetched_at` is newer than the seed's; otherwise the seed. A corrupt/unreadable cache logs and falls back to the seed. Never network.

**Rationale**: FR-020 requires choices on a fresh/offline install; FR-019 requires persistence across restarts. Tying the seed to the existing update recipe keeps it from silently aging. A single `fetched_at` for all three capabilities is enough because the script writes them together.

**Alternatives rejected**:
- Fetching the seed at build time: network-dependent builds and non-reproducible binaries.
- Hand-maintaining the seed: guaranteed drift.
- Gzipping the seed: ~50 KB raw is not worth a second embedded format.
- Using the cache whenever present regardless of age: an old cache could override a newer shipped seed.

## R9 — Storage: one concern, one table

**Decision**: Add `MediaCatalogueStorage` with three methods and a `media_catalogue_snapshot` table keyed by capability. Rows store gzip JSON, schema version, `fetched_at`, `checked_at`, `last_error`, `failed_at`. `SqliteStorage` is the only implementation; `VizierStorage` forwards.

**Rationale**: This mirrors `CatalogueCacheStorage` (`catalogue_snapshot`) and `storage::catalogue` value types without overloading the single-row chat catalogue with three more rows and a different source. It keeps the storage boundary (a backend owns bytes) and the existing migration-free startup. The `schema` column lets a future payload change invalidate old rows; gzip keeps the DB small.

**Alternatives rejected**:
- Reusing `catalogue_snapshot` with a `kind` column: changes the chat catalogue's primary key and every query for a separate concern.
- A generic key/value cache: loses the typed timestamps/failure fields and the schema check.
- In-memory only: violates persistence across restarts (FR-019) and offline after a refresh.
- Filesystem JSON: the runtime is SQLite-only; adding a second persistence mechanism is not lean.

**Failure restore**: unlike the chat catalogue, load the stored `last_error`/`failed_at` into the in-memory refresh state at startup, so the UI's non-blocking notice survives a restart (the spec's snapshot entity records the last failure).

## R10 — HTTP API: two catalogue endpoints

**Decision**:
- `GET /api/v1/catalogue/media-models?capability=speech|transcription|image` — auth required (same as the rest of `/catalogue`), triggers the lazy check, returns the list + status. Missing/invalid capability → 400.
- `POST /api/v1/catalogue/media-models/{capability}/refresh` — auth + `settings:providers` (same as `POST /catalogue/refresh`); 200 with the listing, 409 when disabled, 502 with the reason on failure.

Response:

```json
{
  "capability": "speech",
  "source": "bundled",
  "fetched_at": "2026-10-24T00:00:00Z",
  "checked_at": null,
  "default_voice": "af_alloy",
  "refresh": { "enabled": true, "ttl": "24h", "in_progress": false, "last_error": null, "failed_at": null },
  "models": [
    { "id": "hexgrad/kokoro-82m", "name": "hexgrad: Kokoro 82M", "default": true, "listed": true,
      "voices": ["af_alloy", "..."], "aspect_ratios": [], "resolutions": [] }
  ]
}
```

The documented default is marked `default: true`; if it is absent from the fetched/seed list, it is prepended with `listed: false` and its name set to the id, so the effective value is never invisible and no silent substitution happens. Capability-specific fields are empty arrays rather than omitted, keeping the type stable for the WebUI.

**Rationale**: FR-025 asks for the same API surface the WebUI uses; living under `/catalogue` matches the existing auth/settings split and OpenAPI conventions. One read per capability keeps the picker simple and avoids returning ~125 KB to render one list. The status block reuses the existing `RefreshStatus` shape and vocabulary.

**Alternatives rejected**:
- One endpoint returning all three lists: wastes a refresh of image models when editing TTS, and makes per-capability failures/TTLs awkward.
- A provider-scoped path (`/catalogue/providers/openrouter/media-models`): the feature is OpenRouter-only, but the capability is the useful key and the seed/cache are not provider tables.
- Returning 202 for a background refresh: the UI would need polling; serving the current list with `in_progress` matches the existing catalogue UX.
- Exposing typed model ids as 404: the picker accepts typed ids, so the API only lists; validation stays at the provider.

## R11 — WebUI: a small `ModelSelect`-shaped picker

**Decision**:
- One hook `useMediaModels(capability, enabled)` fetches once per capability when the OpenRouter provider is selected; it returns `{ data, loading, error, refresh }`.
- `MediaModelSelect`: design `Picker` with the default in its own first section, then the capability list; search filters name and id; a typed row offers `Use “<query>”`; selecting sets the stored model id; the trigger shows `Name · id`, or `Default · <name>` when empty. A status line shows `bundled`/`cached`, age and a refresh button for `settings:providers`; failures are a small non-blocking notice and the typed row still works.
- `MediaVoiceSelect` for TTS: same picker over the selected speech model's `voices`; a typed row; uses the response's `default_voice` once loaded (the local map before that); if the model publishes none, the previous free-text field behavior (placeholder = default voice).
- `ToolsFields` uses these only when the provider is `openrouter`; every other provider keeps the existing `TextField`.
- `agentForm.ts`: add `openrouter` to `STT_PROVIDERS`/`IMAGE_GEN_PROVIDERS`; add `TTS_DEFAULT_MODELS`; set `TTS_DEFAULT_VOICES.openrouter = af_alloy`; placeholders show the documented default for all providers.

**Rationale**: FR-016/FR-021/FR-022 and SC-009 are exactly the `ModelSelect` interaction already proven in the app (searchable, typed fallback, default). Reusing `Picker` and the trigger pattern avoids a new combobox or a design change (the repo's design-first rule would otherwise apply). Keeping the form's value independent of the fetched list satisfies FR-023/FR-024: a refresh replaces only the options. Fetching starts on the first open of the model or voice picker (or an explicit refresh), once per capability, so a form render alone makes no request and the network path is always traceable to a person opening the list (FR-018/SC-011). The documented voice belongs to the documented model: when a person chooses a different speech model, the voice picker shows that model's voices and the UI should prompt a choice rather than pretend `af_alloy` is valid for every model.

**Alternatives rejected**:
- A native `<datalist>`: no readable grouping, no keyboard list, inconsistent with the app.
- A plain `<Select>`: not searchable for ~60 long ids.
- A second catalogue screen: configuration belongs in the agent form.
- Fetching inside both model and voice components: duplicate requests; the hook lives at the `ToolsFields` level.
- Disabling Save on list failure: violates FR-024.

## R12 — Errors, accounting and observability

**Decision**: Provider errors flow through the existing `VizierError` paths (`error_in_body`, `http::send*`) and include OpenRouter's message. Refresh errors are `tracing::warn!`/`info!` and the API's `last_error`; they never panic or block a save. Media calls remain outside usage accounting (they bypass `VizierModel`); no cost field is added. No content is logged.

**Rationale**: FR-010 and SC-007 require the provider's explanation and an agent that stays available; the existing tool-call boundary already provides that. Usage accounting is explicitly out of scope. The refresh is a person-facing background operation, so logs plus the status block are enough; no span is needed (and no new content attribute is introduced).

**Missing vs rejected credential**: a *missing* credential is detected at build (the factory resolves it), so the supervisor records a failed agent with the Settings hint, as the spec asks. A *rejected* credential (a non-empty but invalid key) is only knowable by calling the provider; it surfaces as the provider's 401/403 explanation on that call and the agent remains healthy (FR-010). We deliberately do not add a build-time validation ping: it would be an automatic network request at agent build, unlike every other media provider, and the spec's FR-010 treats provider-side failures as tool errors.

**Alternatives rejected**:
- Retrying media calls in the client: the model/turn layer already retries transport failures where appropriate; adding a second retry policy for non-idempotent generation risks duplicate billing.
- Recording media usage: out of scope and the cost shapes vary per model.
- Silent fallback to another model: violates the spec's edge case.

## R13 — Verification strategy

**Decision**: Unit tests cover the pure seams; dummyplug covers the agent-observable path offline; live-key steps are clearly marked because real transcription/speech/image output cannot be deterministic.

- Pure Rust: capability parse, seed parse, list parse/validate, default injection, image path/body/media-type, speech body speed, MIME→extension, enum defaults/credentials, storage round-trip.
- WebUI: `agentForm` default maps (every provider has a model/voice default), picker sections/filter/typed/default/voice helpers, typecheck.
- Dummyplug e2e: no key, seeded lists, default marked, typed id accepted, save/reopen, missing-key tool error, agent still healthy. Refresh with network on (public lists) and with network off (seed + notice).
- Live key (marked): `stt_transcribe` on a short clip, `tts_generate` with empty and explicit voice/model/speed, `image_generate` with empty and explicit model/size; verify MIME/extension and playability.
- Gates: `cargo clippy`, `cargo test`, `npm test`, `npm run typecheck`.

**Rationale**: The constitution's dummyplug gate is about agent-observable behavior; the discovery/picker and the missing-credential error are fully testable offline. Real provider output is inherently live, so marking those steps separately keeps the mandatory gate honest and runnable in CI without a key.

**Alternatives rejected**:
- A mock OpenRouter server: would require making the media base URL configurable (a new test-only config surface) and would not test the real response quirks (`media_type`, model lists).
- Only live tests: fail the offline/CI gate and the seed promise.
