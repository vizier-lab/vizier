# Implementation Plan: OpenRouter Media Provider

**Branch**: `028-openrouter-media-provider` | **Date**: 2026-10-25 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/028-openrouter-media-provider/spec.md`

## Summary

Make OpenRouter a first-class media provider for **speech-to-text**, **text-to-speech** (already wired, verified and corrected here) and **image generation**, and give its media models the catalogue treatment chat models already have: per-capability, searchable, persisted model lists with readable names, published voices/sizing, a documented default, and a typed-id fallback.

Five decisions carry the plan (details in [research.md](./research.md)):

1. **Reuse the OpenAI-shaped media clients wherever OpenRouter is OpenAI-compatible** (R3, R5). STT is the existing multipart `OpenAiStt` against `/audio/transcriptions`; TTS is the existing `OpenAiTts` against `/audio/speech` with `response_format: mp3`. Only the image endpoint differs materially: OpenRouter uses `POST /images` (not `/images/generations`), the universal `size` convenience field, and returns `b64_json` plus an optional `media_type`. That difference becomes a factory-set `ImageApi` variant, never a provider check inside the client.
2. **One credential, verified defaults** (R1, R2). The saved OpenRouter provider credential (or `OPENROUTER_API_KEY`) serves chat, embeddings and all three media capabilities. The live lists were checked on 2026-10-24: the old TTS default `openai/gpt-4o-mini-tts-2025-12-15` is **retired**, so TTS moves to `hexgrad/kokoro-82m` with voice `af_alloy`; STT keeps `openai/whisper-large-v3` and images keep `bytedance-seed/seedream-4.5`, both present.
3. **Media discovery is a second, per-capability catalogue inside the existing `Catalogue` handle** (R6–R9). It has an embedded seed, a SQLite snapshot per capability, a lazy TTL refresh with a 1 h failure backoff, and a manual refresh. It reuses `catalogue.refresh`/`catalogue.ttl`; there is no new config surface and no network at startup, agent build, turn, scheduler or dream time. Refreshes are public (no key needed) with a key fallback only if OpenRouter ever starts requiring one.
4. **The storage change is one concern with one implementation** (R9): `MediaCatalogueStorage` (three methods) and a `media_catalogue_snapshot` table keyed by capability, gzip-JSON payload, schema version, fetch/check timestamps and last failure — mirroring `CatalogueCacheStorage` rather than generalising it.
5. **The WebUI picker is shaped like `ModelSelect`, only smaller** (R11): a searchable `Picker` with the default marked, a typed-id row for anything missing, an optional voice picker fed by the selected model's published voices, and a refresh action for people with `settings:providers`. Other media providers keep their free-text fields; OpenRouter is added to the three provider drop-downs and the existing Settings › Providers media filter already includes it.

## Technical Context

**Language/Version**: Rust edition 2024 (single `vizier` binary, tokio multi-thread); TypeScript 5.9 / React 19 / React Router 7 (SPA) / Tailwind v4 in `webui/`

**Primary Dependencies**: No new Rust crate. Reuse `reqwest`, `serde`/`serde_json`, `chrono`, `rusqlite`, `axum`, `utoipa`, `tracing`, `parking_lot`, `flate2`. Reuse the existing `@vizier/design` `Picker`, `TextField`, `Button`, `Select`, `ToggleSection`; no WebUI dependency is added.

**Storage**: Embedded SQLite. New table `media_catalogue_snapshot` (`capability` primary key, `schema`, `fetched_at`, `checked_at`, `last_error`, `failed_at`, gzip `data`). One embedded seed at `assets/catalogue/openrouter_media.json`. Agent configs need no migration: the three provider enums gain a variant, and existing values keep their meaning.

**Testing**: `cargo test` (capability parsing, seed parsing, per-capability payload parsing/validation, image request body/path, `media_type` → extension, speech body speed omission, defaults and credentials, storage round-trip); `cargo clippy`; `cd webui && npm test` (section/filter/default/typed helpers, `agentForm` defaults/serialization) and `npm run typecheck`; end-to-end: dummyplug steps in [quickstart.md](./quickstart.md) §1–§5 (offline discovery, missing-credential error, seeded picker) plus clearly marked live-key steps §6 for real transcription, speech and images.

**Target Platform**: Linux, macOS and Windows via the single binary; modern evergreen browsers for the WebUI. No OS-specific code.

**Project Type**: Web application inside the existing single-binary Rust service (Rust backend + `webui` submodule on the `design` submodule).

**Performance Goals**: model-list reads are served from memory (target < 50 ms p95, no network wait); a refresh fetches three small public JSON documents (~125 KB uncompressed total on 2026-10-24) in the background under a 30 s timeout; picker search is client-side and instant for ~60 rows.

**Constraints**: zero network at startup, agent build, turn, scheduled task or dream (Constitution III and SC-011); offline installs must work from the seed; the list fetch must not require a credential; STT multipart is capped by OpenRouter at 25 MB (provider error surfaced, not pre-flighted); media calls stay outside usage accounting; one credential; no new config keys; image size is passed as OpenRouter's `size` convenience and normalized upstream; TTS must request `mp3` because `mp3_to_wav` only decodes MP3; a non-default TTS speed is passed through and may be rejected by a model without speed control (FR-010), so the guaranteed default path leaves speed at 1.0.

**Scale/Scope**: one provider, three capabilities, roughly 36 speech / 28 transcription / 61 image models at planning time; two HTTP endpoints; one storage table; one catalogue module; one WebUI picker/hook plus two field swaps per capability; documentation and seed refresh.

## Constitution Check

*GATE: passes before Phase 0 research; re-checked after design below.*

| Principle | Assessment |
|---|---|
| **I. Lean by default** | PASS. No new dependency. The heavy lifting reuses existing clients (`OpenAiStt`, `OpenAiTts`, `OpenAiImages`), the existing `Catalogue` handle, the existing refresh policy, the existing SQLite/cache shape and the existing design `Picker`. The new code is one domain module (`catalogue/media.rs`), one storage concern/trait/table, two handlers and one WebUI picker/hook. No plugin framework, no per-provider trait hierarchy, no discovery for the other providers. |
| **II. DRY via traits** | PASS. `MediaCatalogueStorage` is a new storage concern implemented once by `SqliteStorage` and forwarded once by `VizierStorage`, following the established pattern. Capability differences live in one `MediaCapability` enum (`source_url`, `default_model`, `default_voice`, payload parser), not in handlers. The OpenRouter image difference is a field (`ImageApi`) set by the factory; `openai_shaped.rs` keeps its "no provider checks here" rule. The WebUI picker is one component parameterised by capability. |
| **III. Self-contained runtime** | PASS. The seed ships in the binary; a fresh or offline install lists models without a request. Startup only reads SQLite and the seed. The only network path is a person-triggered read/refresh, reached from the catalogue handlers, never from agents, the scheduler or dreams. Provider calls themselves are the requested feature. |
| **IV. Portability** | PASS. No OS/arch code, no native dependency, no filesystem assumptions beyond the existing workspace. |
| **V. Unified errors & observability** | PASS. Rust paths return `crate::Result`/`anyhow::Result` per existing storage convention; refresh failures are `tracing::warn!` with the URL and reason and never panic; a media call surfaces the provider explanation in `VizierError`. No `unwrap` outside tests. Focused spans are not needed for a list fetch; media calls already run inside `execute_tool`. |
| **Workflow gates** | PASS. `cargo clippy`, `cargo test`, `npm test`, `npm run typecheck`, and the dummyplug e2e gate (quickstart §1–§5) are in the task list; live-key steps are marked. Docs update is required by FR-014. No migration/legacy shim is added, so no #82 entry is needed. Conventional commits; the TTS default change is called out in the commit body. |

**Post-design re-check**: no violations. Three watch items: (a) the seed and the live list can disagree — the default is injected into the response when missing and marked `listed: false`, never silently substituted; (b) the TTS default changed, so docs, WebUI placeholders and the server default must move together; (c) a refresh must never clear an open form's selection — the picker only replaces its list, and the typed/selected id is owned by the form.

## Project Structure

### Documentation (this feature)

```text
specs/028-openrouter-media-provider/
├── plan.md                         # this file
├── research.md                     # R1–R13 decisions, live-check results
├── data-model.md                   # capabilities, listings, snapshots, defaults, transitions
├── quickstart.md                   # offline dummyplug e2e + live-key checks
├── contracts/
│   ├── media-catalogue-api.md      # GET/POST endpoints, auth, payload, status codes
│   ├── media-catalogue-storage.md  # trait, table, load/refresh/failure semantics
│   └── openrouter-media.md         # provider request/response shapes, defaults, seed schema
├── checklists/requirements.md
└── tasks.md                        # /speckit-tasks (not created here)
```

### Source Code (repository root)

```text
src/
├── schema/agent.rs                         # + Openrouter in SttProvider/ImageGenProvider;
│                                           #   TtsProvider::default_model/default_voice;
│                                           #   defaults + credentials; tests
├── tts/mod.rs                              # use provider.default_model(); keep OpenAiTts(mp3)
├── stt/mod.rs                              # Openrouter reaches the existing OpenAiStt arm
├── image_generation/mod.rs                 # Openrouter -> OpenAiImages with ImageApi::OpenRouter
├── llm/media/
│   ├── openai_shaped.rs                    # ImageApi { Generations(ImageSizing), OpenRouter };
│   │                                       #   path/body; read media_type; omit speed at 1.0
│   └── golden_tests.rs                     # constructor call sites use ImageApi::Generations
├── agents/tools/image_gen/mod.rs           # default filename extension follows the returned MIME
├── catalogue/
│   ├── mod.rs                              # Catalogue holds MediaCatalogue; load_media_cache()
│   ├── media.rs                            # MediaCapability, MediaModel, parse/validate,
│   │                                       #   seed load, per-capability cache + refresh
│   └── seed.rs                             # embed and parse openrouter_media.json
├── storage/
│   ├── media_catalogue.rs                  # MediaCatalogueSnapshotRow + MediaCatalogueStorage
│   ├── mod.rs                              # trait in VizierStorageProvider + forwarding impl
│   └── sqlite/
│       ├── mod.rs                          # media_catalogue_snapshot DDL in init_schema
│       └── media_catalogue.rs              # get/put/failure
├── dependencies.rs                         # load cached media lists next to the catalogue cache
└── channels/http/
    ├── mod.rs                              # OpenAPI paths for the two media endpoints
    └── api/v1/catalogue.rs                 # GET media-models + POST .../{capability}/refresh

assets/catalogue/openrouter_media.json      # committed seed (generated)
scripts/catalogue_update.py                 # also fetch/normalize the three OpenRouter lists
build.rs                                    # rerun/validate the media seed

webui/app/
├── api/endpoints/catalogue.ts              # MediaCapability/MediaModels types + calls
├── api/schema.d.ts                         # regenerated from OpenAPI
├── screens/agents/
│   ├── useMediaModels.ts                   # fetch once per capability on first picker open
│   ├── MediaModelSelect.tsx                # Picker, default mark, typed row, refresh notice
│   ├── MediaVoiceSelect.tsx                # Picker over selected model's voices + typed row
│   └── ToolsFields.tsx                     # OpenRouter paths use the pickers; others unchanged
└── lib/agentForm.ts                        # + openrouter in STT/IMAGE lists, TTS model/voice defaults
                                            #   + agentForm.test.ts pins the default maps

docs/src/configuration/tools-embedding.md   # provider tables: OpenRouter defaults for all three
```

**Structure Decision**: web application inside the existing single binary, following the patterns already in place. OpenRouter discovery is not a new subsystem: it hangs off `Catalogue`, the one handle every provider/model read already uses, so `HTTPState` and `VizierDependencies` keep their shape. Media transport stays in `src/llm/media/openai_shaped.rs`; the factory chooses the endpoint variant. The WebUI adds a small hook/component pair rather than a second catalogue screen. `design/` needs no change because `Picker` already covers search, windowing, keyboard navigation and a trigger.

## Phases

### Phase 1 — Provider plumbing (Rust)
1. Add `Openrouter` to `SttProvider` and `ImageGenProvider`; add their `credentials()` and `default_model()` values. Add `TtsProvider::default_model()` and move the TTS factory off its inline match. Split `default_voice()` so OpenAI keeps `alloy` and OpenRouter uses `af_alloy`.
2. STT: the existing `(_, Some(id)) => OpenAiStt` arm now covers OpenRouter; pin the default and credential in unit tests.
3. Image: change `OpenAiImages` to carry `ImageApi::{Generations(ImageSizing), OpenRouter}` (replacing the separate `sizing` field); OpenRouter posts `/images`, sends the per-request `size` (falling back to the agent's configured default when the call omits it) as-is only when present (no `response_format`, no `parse_size` fallback), and reads `data[0].media_type` before falling back to `image/png`. Add helper tests for path/body/decode; update the `OpenAiImages` constructor sites and golden tests (`ImageApi::Generations(ImageSizing::Size)` / `Generations(ImageSizing::AspectRatio(..))`).
4. Image artifact naming: choose the default filename extension from the returned MIME (`.png`, `.jpg`, `.webp`, `.svg`); keep an explicit filename untouched. Update the tool description (default extension follows the returned format; an unset size means the provider default on OpenRouter).
5. TTS: keep `response_format: mp3`; omit `speed` when it is 1.0 so models without speed control do not reject an unset field; keep explicit non-default speeds.

### Phase 2 — Media catalogue domain (Rust)
1. `catalogue/media.rs`: `MediaCapability` (`speech`/`transcription`/`image`) with `source_url`, `default_model`, `default_voice`, `parse`; `MediaModel { id, name, voices, aspect_ratios, resolutions }`.
2. Parsers for the two `GET /models?output_modalities=…` shapes and `GET /images/models`; validate non-empty/unique ids and keep provider order. Reject an empty or malformed refresh without touching the current list.
3. `MediaCatalogue`: seed per capability, `list(capability)`, `swap`, per-capability `RefreshState`, `maybe_refresh`, `refresh_now`, `status`, `load_cache`; no network outside `check`.
4. Seed loading: parse and validate `assets/catalogue/openrouter_media.json`; a broken seed logs and yields an empty list (typed fallback still works). Compare seed/cache `fetched_at` per capability.

### Phase 3 — Storage (Rust)
1. `storage/media_catalogue.rs`: row + `MediaCatalogueStorage` (`get_media_snapshot`, `put_media_snapshot`, `record_media_failure`) keyed by capability string.
2. SQLite DDL in `init_schema`; gzip payload using the existing `catalogue::cache::{gzip, gunzip}`; transaction per write.
3. Add the trait to `VizierStorageProvider` and forward all three methods from `VizierStorage`; call `catalogue.load_media_cache(&storage)` in `dependencies.rs` after the storage exists and next to `cache::newest` (never network).

### Phase 4 — HTTP API (Rust)
1. `GET /api/v1/catalogue/media-models?capability=speech|transcription|image`: auth, `maybe_refresh`, return source/fetch/check/refresh status plus models and the documented default voice (speech only); mark the documented default model and inject it with `listed: false` if absent. Invalid/missing capability → 400.
2. `POST /api/v1/catalogue/media-models/{capability}/refresh`: auth + `settings:providers`; waits for or starts the check; 200 with the listing, 409 when `catalogue.refresh` is false, 502 with the failure reason; previous list stays in use.
3. Register both paths in the `ApiDoc` list and their schemas in `components`; regenerate `webui/app/api/schema.d.ts` during implementation.

### Phase 5 — WebUI
1. Extend `catalogue.ts` types/calls; add `useMediaModels(capability)` that fetches once per capability on the first open of the model or voice picker (or an explicit refresh) and exposes `{ data, loading, error, ensure, refresh }`. A form render alone makes no request; the trigger's default label comes from the local defaults map until the list is loaded.
2. `MediaModelSelect`: design `Picker`; sections put the default first then the capability list; filter by name/id; a typed row (`Use “…”`) when the query/new id is absent; trigger shows the selected name/id or `Default · <name>`; a status line shows source/age and a non-blocking error; a Refresh button appears only with `settings:providers`. Never mutate the model when the list changes.
3. `MediaVoiceSelect`: same pattern over the selected speech model's `voices`; typed fallback; uses the response's `default_voice` when loaded (local `TTS_DEFAULT_VOICES` before that); empty voices fall back to a text field/placeholder with the default voice.
4. `ToolsFields`: render the pickers only when provider is `openrouter`; keep the existing `TextField` for every other provider. Add `openrouter` to `STT_PROVIDERS`/`IMAGE_GEN_PROVIDERS`, add `TTS_DEFAULT_MODELS`, set the OpenRouter defaults, and update `TTS_DEFAULT_VOICES.openrouter = af_alloy`.
5. Tests: `agentForm` default maps cover every listed provider; picker helper tests (filter, default section, typed row, voices); typecheck.

### Phase 6 — Seed, tooling, docs
1. Extend `scripts/catalogue_update.py` with the three OpenRouter URLs, normalization to `MediaModel`, a non-empty/shape check, and `--keep-on-failure` behavior; write `assets/catalogue/openrouter_media.json` with `schema`, `fetched_at`, and per-capability arrays.
2. `build.rs` adds `rerun-if-changed` and JSON validation for the media seed (no gzip needed).
3. Update `docs/src/configuration/tools-embedding.md` provider tables: TTS default model/voice, STT OpenRouter, image OpenRouter, shared credential, and a sentence on the picker/typed fallback plus choosing a voice for a non-default speech model.
4. Reconcile the spec's Assumptions with the verified TTS default (this plan's date).

### Phase 7 — Verification
1. `cargo test`, `cargo clippy`, `cd webui && npm test && npm run typecheck`.
2. Dummyplug e2e: offline seeded lists, default marked, typed fallback, save/reopen, missing-credential error; §6 live-key checks for real artifacts.
3. Confirm no network on startup/agent build/turn (logs or a no-network run); confirm the seed still serves.
4. Regression: an existing OpenAI/Gemini/Whisper media agent behaves unchanged; the app still builds with the embedded WebUI.

## Complexity Tracking

No constitution violations. No new dependency, subsystem, service or migration is introduced.
