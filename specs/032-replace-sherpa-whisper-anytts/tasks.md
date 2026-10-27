# Tasks: Replace sherpa-onnx with whisper-rs and any-tts

**Input**: Design documents from `/specs/032-replace-sherpa-whisper-anytts/`
**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, quickstart.md

**Tests**: No test tasks generated (spec does not request TDD; existing `cargo test`/dummyplug e2e cover validation).

**Organization**: Tasks grouped by user story. US1 = local STT, US2 = local TTS, US3 = self-contained build.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3)

## Path Conventions

- Single project: `src/`, `webui/`, `docs/` at repository root

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Swap dependencies and remove sherpa build references.

- [X] T001 Update `Cargo.toml`: remove `sherpa-onnx`/`sherpa-onnx-sys`; add `whisper-rs` (+ `whisper-rs-sys`) and `any-tts`; keep `hound`, `symphonia`, `fastembed`.
- [X] T002 [P] Update `.github/workflows/release.yml`: remove the sherpa-onnx prebuilt re-download / clean steps.
- [X] T003 [P] Update `docs/src/configuration/tools-embedding.md`: replace "runs locally via sherpa-onnx" with the new runtimes and update default model names.

**Checkpoint**: Dependency set changed; build will be restored in US1/US2.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: None blocking beyond Phase 1; trait interfaces and provider enums are already in place.

*(No foundational tasks — the `VizierSttModel`/`VizierTtsModel` traits and `SttProvider`/`TtsProvider` enums already exist.)*

**Checkpoint**: Ready to implement user stories.

---

## Phase 3: User Story 1 - Local speech-to-text keeps working (Priority: P1) 🎯 MVP

**Goal**: Provide local Whisper STT via whisper-rs with no sherpa-onnx, no espeak-ng, no ONNX conflict.

**Independent Test**: Enable `stt` with `provider: whisper`, transcribe an audio file, get a transcript; first run downloads the GGML model with progress.

### Implementation for User Story 1

- [X] T004 [US1] Rewrite `src/stt/whisper.rs` to use `whisper-rs`: download the GGML Whisper model into `<workspace>/.runtime/stt/whisper/`, report progress via `SetupBoard`, load the recognizer, transcribe WAV/PCM audio to text. Keep the `VizierSttModel` impl signature.
- [X] T005 [P] [US1] Update the STT default model string in `src/schema/agent.rs` (`SttProvider::Whisper.default_model()`) to the new GGML artifact name.
- [X] T006 [P] [US1] Update `webui/app/lib/agentForm.ts`: `STT_DEFAULT_MODELS.whisper` and the `emptyForm`/`fromDetail` defaults to the new artifact name and `whisper` provider.
- [X] T007 [US1] Run `cargo build` to confirm the STT rewrite compiles; fix any errors.

**Checkpoint**: Local STT works offline end to end.

---

## Phase 4: User Story 2 - Local text-to-speech keeps working (Priority: P1)

**Goal**: Provide local Kokoro TTS via any-tts with no sherpa-onnx, no espeak-ng.

**Independent Test**: Enable `tts` with `provider: kokoro`, generate speech for voice `af`, get a valid WAV; no espeak-ng on host.

### Implementation for User Story 2

- [X] T008 [US2] Rewrite `src/tts/kokoro.rs` to use `any-tts` (Candle/Kokoro): download weights into `<workspace>/.runtime/tts/kokoro/`, report progress via `SetupBoard`, generate WAV for a voice ID. Keep the `VizierTtsModel` impl signature; fall back to voice 0 with a warning for unknown IDs.
- [X] T009 [P] [US2] Update the TTS default model string in `src/schema/agent.rs` (`TtsProvider::Kokoro.default_model()`) to the new artifact name.
- [X] T010 [P] [US2] Update `webui/app/lib/agentForm.ts`: `TTS_DEFAULT_MODELS.kokoro` and the `emptyForm`/`fromDetail` defaults to the new artifact name and `kokoro` provider.
- [X] T011 [US2] Run `cargo build` to confirm the TTS rewrite compiles; fix any errors.

**Checkpoint**: Local TTS works offline end to end.

---

## Phase 5: User Story 3 - Single self-contained binary (Priority: P2)

**Goal**: Verify no sherpa-onnx remains and the build is self-contained.

**Independent Test**: `cargo build --release` succeeds without sherpa prebuilt steps; `Cargo.lock` has no sherpa-onnx; cross-build passes.

### Implementation for User Story 3

- [X] T012 [US3] Verify `Cargo.lock` contains no `sherpa-onnx`/`sherpa-onnx-sys` entries; run `cargo build --release`.
- [X] T013 [P] [US3] Run `cargo clippy` and `cargo test`; fix warnings/errors.
- [X] T014 [P] [US3] Run `cd webui && npm run typecheck`.
- [ ] T015 [US3] Cross-build check: run `cross build` for an aarch64 target; add compiler pre-build steps to `Cross.toml` if whisper-rs-sys needs them. **Pre-build steps for cmake + the cross C++ toolchain were added to `Cross.toml`, but `cross` is not installed and Docker is unavailable in this environment, so the build itself was not run.**
- [X] T016 [US3] Grep for remaining `sherpa_onnx`/`sherpa-onnx` references in `src/` and remove any stale ones.

**Checkpoint**: Clean, self-contained release build.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Validation against quickstart and constitution e2e gate.

- [X] T017 Run the dummyplug STT steps from `quickstart.md` against a running binary. (Audio generated locally by TTS, transcribed back to the exact spoken sentence; a restart re-transcribed it from the cached model in 484 ms with no download.)
- [X] T018 Run the dummyplug TTS steps from `quickstart.md` against a running binary. (A 183,644-byte RIFF/WAVE, 24 kHz mono 16-bit, 3.83 s file; a post-restart run took 3.7 s from the cached model.)
- [X] T019 Verify first-run download progress appears in the WebUI setup board for both capabilities.
- [X] T020 Verify TTS succeeds with no `espeak-ng` installed on the host.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies.
- **Phase 3 (US1)**: Depends on Phase 1.
- **Phase 4 (US2)**: Depends on Phase 1; can run in parallel with US1 (different files).
- **Phase 5 (US3)**: Depends on US1 and US2 completing.
- **Phase 6 (Polish)**: Depends on US3.

### Parallel Opportunities

- T002, T003 (Phase 1) parallel.
- T005, T006 (US1) parallel; T009, T010 (US2) parallel.
- T013, T014 (US3) parallel.
- US1 and US2 can be implemented in parallel by different developers.

## Implementation Strategy

### MVP First (User Story 1)

1. Complete Phase 1 (Setup).
2. Complete US1 (local STT).
3. **STOP and VALIDATE**: transcribe audio via dummyplug.
4. Proceed to US2, then US3, then Polish.

### Incremental Delivery

1. Setup → STT working (MVP).
2. Add TTS → both local capabilities working.
3. Verify clean self-contained build + cross-compile.
4. Run dummyplug e2e + setup-board + no-espeak checks.