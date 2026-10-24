# Tasks: OpenRouter Media Provider

**Input**: Design documents from `/specs/028-openrouter-media-provider/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/), [quickstart.md](./quickstart.md)

**Tests**: The plan requires unit coverage for the pure seams and the constitution requires the dummyplug e2e gate. Tests are written alongside implementation; no separate TDD-first phase was requested.

**Organization**: Tasks are grouped by user story so each capability can be implemented, tested and demonstrated independently. The shared provider plumbing lives in Phase 2 because the image factory must stay exhaustive and the WebUI provider lists must agree with the Rust enums.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: Which user story this task belongs to (`US1`–`US4`)
- Every task names its exact file path(s)

## Path Conventions

- Rust backend: `src/`
- WebUI submodule: `webui/app/`
- Design submodule: `design/` (no change required; the existing `Picker` is reused)
- Seed/tooling: `assets/catalogue/`, `scripts/`, `build.rs`
- Feature docs: `specs/028-openrouter-media-provider/`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Establish a clean baseline and re-confirm the provider facts the defaults depend on.

- [X] T001 Capture the baseline at the repository root: run `cargo test`, `cargo clippy`, `cd webui && npm test`, and `cd webui && npm run typecheck`; confirm `webui/node_modules`, `design/`, and `webui/` are installed. Record any pre-existing failures; make no code changes. Files: `Cargo.toml`, `webui/package.json`.
- [X] T002 [P] Re-check the three public OpenRouter lists and the three default ids (`hexgrad/kokoro-82m` with `af_alloy`, `openai/whisper-large-v3`, `bytedance-seed/seedream-4.5`) against `research.md` R2/R8. If a default is retired, apply the R2 substitution rule and update `specs/028-openrouter-media-provider/research.md`, `spec.md`, and the plan defaults before implementation. Files: `specs/028-openrouter-media-provider/research.md`, `specs/028-openrouter-media-provider/spec.md`.

**Checkpoint**: Baseline recorded; live defaults confirmed or the docs updated.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Add the shared provider values and the OpenRouter image client so every capability is selectable and every factory match stays exhaustive. No discovery or picker work yet.

**⚠️ CRITICAL**: Complete T003–T005 before starting a user story. T003 adds the `ImageGenProvider::Openrouter` variant, so T004 must land with it to keep `src/image_generation/mod.rs` compiling.

- [X] T003 [P] Add `Openrouter` to `SttProvider` and `ImageGenProvider` in `src/schema/agent.rs`, with `credentials() = Some(ProviderId::OPENROUTER)` and defaults `openai/whisper-large-v3` (STT) and `bytedance-seed/seedream-4.5` (image). Add `TtsProvider::default_model()` with `openai` `tts-1`, `openrouter` `hexgrad/kokoro-82m`, `elevenlabs` `eleven_multilingual_v2`, `xai` `grok-2-tts`, `hyperbolic` `Melo-TTS`, `kokoro` `kokoro-en-v0_19`. Do **not** change `default_voice()` or the TTS factory yet; add unit tests for the new enum values and defaults.
- [X] T004 [P] Add `ImageApi::{Generations(ImageSizing), OpenRouter}` in `src/llm/media/openai_shaped.rs` (replacing the separate `sizing` field). The `OpenRouter` arm posts `/images`, sends the per-request `size` or the agent's configured fallback only when present, omits `response_format`, decodes `data[0].b64_json`, and reads `data[0].media_type` with an `image/png` fallback. Wire `ImageGenProvider::Openrouter` in `src/image_generation/mod.rs` and update the constructor call sites in `src/llm/media/golden_tests.rs` to `ImageApi::Generations(ImageSizing::Size)` / `Generations(ImageSizing::AspectRatio(..))`. Add unit tests for the path/body (size present and absent) and `media_type` decoding. Files: `src/llm/media/openai_shaped.rs`, `src/image_generation/mod.rs`, `src/llm/media/golden_tests.rs`.
- [X] T005 [P] Add `openrouter` to `STT_PROVIDERS`/`STT_DEFAULT_MODELS` (`openai/whisper-large-v3`) and `IMAGE_GEN_PROVIDERS`/`IMAGE_GEN_DEFAULT_MODELS` (`bytedance-seed/seedream-4.5`) in `webui/app/lib/agentForm.ts`. Extend `webui/app/lib/agentForm.test.ts` so every provider in each list has a model default and a `provider: 'openrouter'` STT/image setting round-trips with null overrides. Do not add the TTS model/voice defaults yet; US3 owns those.

**Checkpoint**: `cargo build` succeeds; OpenRouter is selectable for STT and images; the image client is wired; existing TTS behavior is unchanged.

---

## Phase 3: User Story 1 - Voice messages are transcribed through OpenRouter (Priority: P1) 🎯 MVP

**Goal**: OpenRouter is a complete speech-to-text provider: one saved credential, a documented default, optional model/language, the transcription tool and the channel voice-message path.

**Independent Test**: With only an OpenRouter credential saved, enable STT with provider `openrouter` and no model; transcribe a short clip through `stt_transcribe` and through a voice message; both produce readable text. Repeat with an explicit model and language.

### Implementation for User Story 1

- [X] T006 [P] [US1] Add an explicit `(SttProvider::Openrouter, Some(id))` arm in `src/stt/mod.rs` that builds `OpenAiStt` over the existing multipart `/audio/transcriptions` path (optional `language`, no 25 MB pre-flight), so the OpenRouter case does not rely on the catch-all. Add a focused test/pin that an empty model resolves to `openai/whisper-large-v3` and that `language` is passed through. Files: `src/stt/mod.rs`.
- [X] T007 [US1] Validate US1 with the free-text fields (the picker arrives in US4): quickstart §1/§5 and the LIVE §6 STT steps. Confirm the transcript comes back through `stt_transcribe`, a channel voice message reaches the agent, an explicit model/language works, and a missing credential fails the agent build with the `openrouter` Settings hint. Files: `specs/028-openrouter-media-provider/quickstart.md`, `src/stt/mod.rs`.

**Checkpoint**: US1 is fully functional and independently testable; the agent remains healthy after provider errors.

---

## Phase 4: User Story 2 - Images are generated through OpenRouter (Priority: P1)

**Goal**: OpenRouter image generation produces a valid artifact with the provider's actual media type, a matching default extension, and a per-request or configured size.

**Independent Test**: With only an OpenRouter credential saved, enable image generation with provider `openrouter` and no model; ask for an image; a valid file appears in the session files and opens. Repeat with an explicit model and a per-request `size`.

### Implementation for User Story 2

- [X] T008 [P] [US2] In `src/agents/tools/image_gen/mod.rs`, choose the default filename extension from the returned MIME: `image/png` → `.png`, `image/jpeg`/`image/jpg` → `.jpg`, `image/webp` → `.webp`, `image/svg+xml` → `.svg`, unknown → `.png`; keep an explicit `filename` untouched. Keep the existing `args.size.or(default_size)` precedence (a per-call size overrides the configured default) and update the tool description to say the default extension follows the returned format and an unset size means the provider default. Add a unit test for the MIME→extension mapping. Files: `src/agents/tools/image_gen/mod.rs`.
- [X] T009 [US2] Validate US2 with the free-text fields: quickstart §1 and the LIVE §6 image steps. Generate with empty overrides, with an explicit model, and with a per-request `size` (`1792x1024`); confirm the returned MIME and extension agree, the file opens, `send_attachment` delivers it, and a non-PNG response (use a Recraft vector model) keeps `image/svg+xml`. Files: `specs/028-openrouter-media-provider/quickstart.md`, `src/agents/tools/image_gen/mod.rs`.

**Checkpoint**: US2 is fully functional and independently testable; existing OpenAI/xAI image behavior is unchanged.

---

## Phase 5: User Story 3 - Replies are spoken through OpenRouter (Priority: P2)

**Goal**: OpenRouter text-to-speech uses a live default model and matching voice, produces MP3-derived WAV audio, and passes explicit voice/model/speed without breaking models that lack speed control.

**Independent Test**: With only an OpenRouter credential saved, enable TTS with provider `openrouter` and empty overrides; generate a short sentence through `tts_generate`; the audio plays. Repeat with an explicit voice and model; a non-default speed either plays or surfaces the provider's explanation.

### Implementation for User Story 3

- [X] T010 [P] [US3] Change `TtsProvider::default_voice()` so OpenAI keeps `alloy` and OpenRouter uses `af_alloy`, and change `src/tts/mod.rs` to resolve the model through `TtsProvider::default_model()`; keep the OpenRouter arm on `OpenAiTts` with `response_format: Some("mp3")`. Add a test/pin for the resolved model and voice pair. Files: `src/schema/agent.rs`, `src/tts/mod.rs`.
- [X] T011 [P] [US3] Add `TTS_DEFAULT_MODELS` for all six providers (OpenRouter `hexgrad/kokoro-82m`) and set `TTS_DEFAULT_VOICES.openrouter = 'af_alloy'` in `webui/app/lib/agentForm.ts`; use the model map for the TTS model placeholder. Extend `webui/app/lib/agentForm.test.ts` so every TTS provider has a model and voice default. Files: `webui/app/lib/agentForm.ts`, `webui/app/lib/agentForm.test.ts`.
- [X] T012 [P] [US3] In `src/llm/media/openai_shaped.rs`, omit the `speed` field when it is exactly `1.0` and send it for non-default values; add a unit test that the speech body omits `speed` at `1.0` and includes `1.25`. This file is shared with T004, so apply it after T004. Files: `src/llm/media/openai_shaped.rs`.
- [X] T013 [US3] Validate US3: quickstart §1 and the LIVE §6 TTS steps. Empty overrides must produce playable audio with `hexgrad/kokoro-82m`/`af_alloy`; an explicit voice/model must work; a non-default speed either plays or surfaces the provider's explanation (FR-010) while the agent stays healthy. Files: `specs/028-openrouter-media-provider/quickstart.md`, `src/tts/mod.rs`.

**Checkpoint**: US3 is fully functional and independently testable; OpenAI TTS still uses `alloy`.

---

## Phase 6: User Story 4 - OpenRouter media models are discoverable and selectable (Priority: P2)

**Goal**: Per-capability model lists for speech, transcription and images, with readable names, published voices/sizing, a marked default, a typed-id fallback, persistent offline caching and a manual refresh.

**Independent Test**: In the WebUI, select OpenRouter for each capability; the picker lists the capability's models with the default marked; choose a non-default model, save and reopen; type an unlisted id and confirm it persists; disconnect the network, reopen the editor, and confirm the last-known/seed list still appears.

### Domain and persistence

- [X] T014 [P] [US4] Create `src/catalogue/media.rs` with `MediaCapability` (`speech`, `transcription`, `image`) and `MediaModel { id, name, voices, aspect_ratios, resolutions }`. Implement `MediaCapability`'s `source_url`, `default_model` (delegate to the matching provider enum's `Openrouter` value from T003), `default_voice` (`Some(TtsProvider::Openrouter.default_voice())` for speech, `None` otherwise) and `parse`. Parse `GET /models?output_modalities=speech|transcription` and `GET /images/models`. Enforce the data-model rules verbatim: ids required, non-empty after trim and unique (first duplicate wins); `name` falls back to the id; arrays keep strings only; at most 2,000 entries; reject an empty or malformed list. Add unit tests for all three payloads, duplicates, empty/missing fields and voice/aspect-ratio/resolution extraction. Files: `src/catalogue/media.rs`.
- [X] T015 [P] [US4] Add `MediaCatalogueSnapshotRow` and `MediaCatalogueStorage` with **three** methods (`get_media_snapshot`, `put_media_snapshot`, `record_media_failure`) in `src/storage/media_catalogue.rs`; implement it for `SqliteStorage` in `src/storage/sqlite/media_catalogue.rs`; add the `media_catalogue_snapshot` DDL to `src/storage/sqlite/mod.rs` (`capability` PK, `schema`, `fetched_at`, `checked_at`, `last_error`, `failed_at`, gzip `data`); add the supertrait and forwarding methods in `src/storage/mod.rs`. Timestamps are Unix milliseconds; `put` clears the failure in one transaction; `record_failure` leaves data/checked_at untouched. Add round-trip/failure tests. Files: `src/storage/media_catalogue.rs`, `src/storage/sqlite/media_catalogue.rs`, `src/storage/sqlite/mod.rs`, `src/storage/mod.rs`.
- [X] T016 [US4] Implement `MediaCatalogue` in `src/catalogue/media.rs`: per-capability lists and refresh state, `maybe_refresh`, `refresh_now`, `status` and `load_cache`. Reuse `catalogue.refresh`/`catalogue.ttl`, apply the 1-hour failure backoff, serve immediately from memory, keep the previous list on failure, restore the stored last failure at startup, use the seed when the cache is absent/older/corrupt, and never fetch at startup. Fetch anonymously first; on 401/403 retry once with the saved/env OpenRouter key, and record the failure when neither works. Hold the catalogue in `Catalogue` (`src/catalogue/mod.rs`) and call `catalogue.load_media_cache(&storage)` in `src/dependencies.rs` next to `cache::newest`. Depends on T014/T015. Files: `src/catalogue/media.rs`, `src/catalogue/mod.rs`, `src/dependencies.rs`.
- [X] T017 [P] [US4] Extend `scripts/catalogue_update.py` to fetch the three OpenRouter lists, normalize them to the seed shape (name fallback, dedupe, voices/aspect ratios/resolutions), reject an empty list, write `assets/catalogue/openrouter_media.json` atomically with `schema: 1` and a UTC `fetched_at`, and honor `--keep-on-failure` by keeping the committed file. Update the module docstring and printed summary. Files: `scripts/catalogue_update.py`, `assets/catalogue/openrouter_media.json`.
- [X] T018 [US4] Embed the media seed generated by T017: `src/catalogue/seed.rs` reads `assets/catalogue/openrouter_media.json` with `include_str!` and parses it; `build.rs` validates the JSON and adds `rerun-if-changed`; the committed asset has `schema: 1`, a UTC `fetched_at`, and the three lists (including the documented defaults and published voices/sizing). A broken seed logs and yields empty lists, not a crash. Depends on T014/T017; can run parallel with T016 after T017. Files: `src/catalogue/seed.rs`, `build.rs`, `assets/catalogue/openrouter_media.json`.

### API

- [X] T019 [US4] Add the HTTP API in `src/channels/http/api/v1/catalogue.rs` and `src/channels/http/mod.rs`: `GET /api/v1/catalogue/media-models?capability=…` (auth, `maybe_refresh`, returns source/fetched_at/checked_at/default_voice/refresh plus models with `default` and `listed`; inject a missing default with `listed: false`; 400 on a bad/missing capability) and `POST /api/v1/catalogue/media-models/{capability}/refresh` (auth + `settings:providers`; 200, 400, 409 when disabled, 502 on failure; the previous list stays in use). Register the paths/schemas in `ApiDoc` and add a test for default injection and capability parsing. Depends on T016. Files: `src/channels/http/api/v1/catalogue.rs`, `src/channels/http/mod.rs`.
- [X] T020 [US4] Add `MediaCapability`, `MediaModelSummary`, `MediaModels` and `refresh` status types plus `mediaModels(capability)` / `refreshMediaModels(capability)` calls in `webui/app/api/endpoints/catalogue.ts`; regenerate `webui/app/api/schema.d.ts` from the server's OpenAPI document. Depends on T019. Files: `webui/app/api/endpoints/catalogue.ts`, `webui/app/api/schema.d.ts`.

### WebUI

- [X] T021 [US4] Add `useMediaModels(capability)` in `webui/app/screens/agents/useMediaModels.ts`: fetch once on the first open of the model or voice picker (not on form render), keep the current data when a refresh fails, and expose `{ data, loading, error, ensure, refresh }`. It must never mutate the form's selected model/voice. Depends on T020. Files: `webui/app/screens/agents/useMediaModels.ts`.
- [X] T022 [P] [US4] Add `MediaModelSelect` in `webui/app/screens/agents/MediaModelSelect.tsx`: design `Picker`; default first with a badge; name/id search; a typed `Use “<query>”` row; trigger shows `Name · id` or `Default · <name>`; a source/age line and non-blocking error; a Refresh button only with `settings:providers`. Use the API's `default`/`listed` flags and never clear the selected value when the list changes. Depends on T021. Files: `webui/app/screens/agents/MediaModelSelect.tsx`.
- [X] T023 [P] [US4] Add `MediaVoiceSelect` in `webui/app/screens/agents/MediaVoiceSelect.tsx`: a picker over the selected speech model's `voices`, a typed voice row, the response's `default_voice` when loaded (local map before that), and the existing free-text/placeholder behavior when the model publishes no voices. Depends on T021. Files: `webui/app/screens/agents/MediaVoiceSelect.tsx`.
- [X] T024 [US4] Wire `MediaModelSelect`/`MediaVoiceSelect` into `webui/app/screens/agents/ToolsFields.tsx` only when the provider is `openrouter`; keep the existing `TextField` for every other provider, keep the image **Size** field free text, and keep the per-request size override. Depends on T022/T023. Files: `webui/app/screens/agents/ToolsFields.tsx`.
- [X] T025 [P] [US4] Add pure helper coverage in `webui/app/lib/mediaModels.ts` and `webui/app/lib/mediaModels.test.ts` for the default-first sections, name/id filtering, typed-id row, voice sections and `default_voice` fallback. Depends on T022/T023. Files: `webui/app/lib/mediaModels.ts`, `webui/app/lib/mediaModels.test.ts`.
- [X] T026 [US4] Validate US4 with `quickstart.md` §1–§4: the seed lists all three capabilities with the defaults marked; a non-default model persists across save/reopen; a typed id persists exactly; a refresh becomes `cached` with a new `fetched_at`; a restart serves the cache; a failed refresh keeps the last list and shows a non-blocking notice; a form render makes no request. Files: `specs/028-openrouter-media-provider/quickstart.md`.

**Checkpoint**: US4 is fully functional; the picker works offline from the seed/cache and never blocks saving.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Documentation, gates, and cross-story verification.

- [X] T027 [P] Update `docs/src/configuration/tools-embedding.md`: TTS default model/voice for OpenRouter (`hexgrad/kokoro-82m`/`af_alloy`), STT OpenRouter (`openai/whisper-large-v3`), image OpenRouter (`bytedance-seed/seedream-4.5`), the shared `openrouter` credential, the picker/typed-id behavior, and the note that a non-default speech model needs one of its published voices. Files: `docs/src/configuration/tools-embedding.md`.
- [X] T028 Run `cargo clippy` at the repository root; fix every warning introduced by the feature. Files: `Cargo.toml`, `src/`.
- [X] T029 Run `cargo test` at the repository root; fix every failure. Files: `Cargo.toml`, `src/`.
- [X] T030 Run `cd webui && npm test`; fix every failure. Files: `webui/package.json`, `webui/app/`.
- [X] T031 Run `cd webui && npm run typecheck`; fix every failure. Files: `webui/package.json`, `webui/app/`.
- [X] T032 Run the constitution's dummyplug e2e gate: `quickstart.md` §1–§5 on a running binary, including the missing-credential failed-health check and the offline seed. Record the results. Files: `specs/028-openrouter-media-provider/quickstart.md`.
- [X] T033 Confirm SC-011: with `VIZIER_CATALOGUE_REFRESH=false` and a dummyplug agent, startup, agent build and a turn make no media-list request; the seed serves. Files: `src/catalogue/media.rs`, `src/dependencies.rs`, `specs/028-openrouter-media-provider/quickstart.md`.
- [X] T034 Regression check: an existing OpenAI/Gemini/Whisper media agent still builds and behaves unchanged; OpenAI TTS keeps `alloy`; `GET /api/v1/catalogue/providers?kind=media` includes `openrouter`. Files: `src/tts/mod.rs`, `src/stt/mod.rs`, `src/image_generation/mod.rs`.
- [X] T035 Build the deliverable: `cargo build` (or `just build`) to confirm `build.rs` embeds the media seed and the WebUI build succeeds with the new picker. Files: `build.rs`, `webui/package.json`.
- [X] T036 Walk spec `SC-001`–`SC-013` and record evidence for each; fix any gap before declaring the feature complete. Files: `specs/028-openrouter-media-provider/spec.md`, `specs/028-openrouter-media-provider/quickstart.md`.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: Starts immediately.
- **Foundational (Phase 2)**: Depends on Setup. **Blocks every user story.** T003 and T004 must land together because the new image enum makes the image factory match exhaustive only once T004 adds the arm.
- **US1 (Phase 3)**: Depends on Foundational. No dependency on US2–US4.
- **US2 (Phase 4)**: Depends on Foundational. The image client is already wired; this phase adds the tool's artifact naming. Can run in parallel with US1 and US3 except for its shared use of `openai_shaped.rs` history (T004 is already complete).
- **US3 (Phase 5)**: Depends on Foundational. Can run in parallel with US1/US2; T012 edits `openai_shaped.rs`, so apply it after T004 and do not overlap it with another edit to that file.
- **US4 (Phase 6)**: Depends on Foundational. Its speech defaults use `TtsProvider::default_model()` from T003 and `TtsProvider::default_voice()` after US3 T010; implement US3 before the US4 speech validation if both are in flight.
- **Polish (Phase 7)**: Depends on all selected stories.

### User Story Dependencies

- **US1 (P1)**: Independent after Foundational.
- **US2 (P1)**: Independent after Foundational; it also benefits from US1's shared provider plumbing but does not require US1 code.
- **US3 (P2)**: Independent after Foundational; the default pair is the only cross-story value, and US4 reads it.
- **US4 (P2)**: Independent after Foundational in code; the speech row's default voice is the value US3 finalises, so run US3 T010 before US4 validation.

### Within Each User Story

- US1: T006 → T007.
- US2: T008 → T009.
- US3: T010/T011/T012 → T013.
- US4: T014/T015/T017 → T016/T018 → T019 → T020 → T021 → T022/T023 → T024/T025 → T026.

### Parallel Opportunities

- Setup: T001 and T002.
- Foundational: T003, T004 and T005 (different files; T003+T004 must be integrated together).
- After Foundational, these can start together: T006 (STT), T008 (image tool), T010 (TTS defaults/factory), T011 (TTS WebUI maps), T012 (speech body) and US4's T014/T015/T017.
- US4: T014/T015/T017 in parallel; T016 after T014/T015; T018 after T014/T017; T019 after T016; T020 after T019; T021 after T020; T022/T023 in parallel; T024/T025 after T022/T023.
- Polish: T027 can run while T028–T031 are run; the gates T032–T036 are sequential.

---

## Parallel Example: Foundational and Early Stories

```bash
# Foundational, different files (integrate T003 + T004 before building):
Task: "T003 Add OpenRouter provider enums/defaults and TtsProvider::default_model() in src/schema/agent.rs"
Task: "T004 Add ImageApi::OpenRouter and wire the image factory in src/llm/media/openai_shaped.rs + src/image_generation/mod.rs"
Task: "T005 Add OpenRouter to the STT/image WebUI provider lists in webui/app/lib/agentForm.ts"

# After foundational, independent capability work:
Task: "T006 Add the explicit OpenRouter STT arm in src/stt/mod.rs"
Task: "T008 Default image filename extension from MIME in src/agents/tools/image_gen/mod.rs"
Task: "T010 Switch OpenRouter TTS to kokoro-82m/af_alloy in src/schema/agent.rs + src/tts/mod.rs"
Task: "T011 Add TTS_DEFAULT_MODELS and the af_alloy voice map in webui/app/lib/agentForm.ts"
Task: "T012 Omit the default speed in src/llm/media/openai_shaped.rs"
```

## Parallel Example: Discovery Foundations

```bash
Task: "T014 Add MediaCapability/MediaModel parsers in src/catalogue/media.rs"
Task: "T015 Add MediaCatalogueStorage + media_catalogue_snapshot in src/storage/"
Task: "T017 Extend scripts/catalogue_update.py for the three OpenRouter lists"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Setup (T001–T002).
2. Complete Foundational (T003–T005).
3. Complete US1 (T006–T007).
4. **STOP and VALIDATE**: transcribe through `stt_transcribe` and a voice message; confirm the missing-credential error.
5. Deploy/demo the STT increment.

### Incremental Delivery

1. Setup + Foundational → provider plumbing ready.
2. US1 → STT MVP (P1).
3. US2 → image generation (P1).
4. US3 → TTS defaults/speed (P2).
5. US4 → discovery/picker (P2).
6. Polish → docs, gates, e2e, regression.

Each increment adds one capability without breaking the previous ones.

### Parallel Team Strategy

With multiple developers after Foundational:

- **Developer A**: US1 then US2 (or one each; both are P1).
- **Developer B**: US3, then the US4 API/storage half.
- **Developer C**: US4 domain/seed/script, then the WebUI picker half.
- Coordinate T012 (`openai_shaped.rs`) with whoever owns US2/T004; it is the only shared file after Foundational.
- Do not parallelise two edits to `src/schema/agent.rs`, `webui/app/lib/agentForm.ts` or `src/llm/media/openai_shaped.rs`.

---

## Notes

- `[P]` tasks touch different files and can be started together; the shared-file caveats above take precedence.
- Tests are included alongside implementation, not as a separate TDD phase.
- Each story ends at a checkpoint that can be validated without the later stories.
- The discovery cache is separate from `catalogue_snapshot`; do not mix the two.
- A failed refresh must never clear a form's selected value or block a save/media call.
- Live-key checks in quickstart §6 are marked **LIVE** and are not part of the offline CI gate.
