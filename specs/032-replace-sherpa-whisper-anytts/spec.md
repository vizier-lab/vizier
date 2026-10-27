# Feature Specification: Replace sherpa-onnx with whisper-rs and any-tts

**Feature Branch**: `032-replace-sherpa-whisper-anytts`

**Created**: 2026-10-27

**Status**: Draft

**Input**: User description: "let's replace sherpa onnx with whisper-rs and any-tts"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Local speech-to-text keeps working after the swap (Priority: P1)

A user who runs an agent with `stt` enabled and `provider: whisper` (no cloud credentials) can still transcribe audio files and use `audio_chat` exactly as before. The downloaded model, the resulting transcription quality, and the first-run download progress in the WebUI setup board are all preserved in spirit (a different model artifact is downloaded once).

**Why this priority**: Local STT is the only capability that has no cloud fallback for air-gapped users; breaking it blocks offline usage entirely.

**Independent Test**: Enable `stt` with `provider: whisper`, send an audio file, and confirm a transcript is returned; verify the setup board reports download progress on first run.

**Acceptance Scenarios**:

1. **Given** an agent with `stt.provider = whisper` and no model cached, **When** the first transcription is requested, **Then** the model downloads with progress reported and transcription succeeds.
2. **Given** a cached model, **When** an audio file is transcribed, **Then** the result matches the previous sherpa-onnx Whisper output within normal model tolerance.

---

### User Story 2 - Local text-to-speech keeps working after the swap (Priority: P1)

A user who runs an agent with `tts` enabled and `provider: kokoro` can still generate speech audio into session files, with the same voice IDs and comparable quality, and no espeak-ng system dependency.

**Why this priority**: Local TTS is the offline counterpart to cloud TTS; it must not regress for users who rely on it without credentials.

**Independent Test**: Enable `tts` with `provider: kokoro`, call `tts_generate`, and confirm a playable WAV is produced for a known voice.

**Acceptance Scenarios**:

1. **Given** an agent with `tts.provider = kokoro` and no model cached, **When** speech is generated, **Then** the model downloads with progress and a valid WAV is returned.
2. **Given** a known voice ID (e.g. `af`), **When** speech is generated, **Then** audio is produced without requiring espeak-ng on the host.

---

### User Story 3 - Single self-contained binary with no ONNX version conflicts (Priority: P2)

A developer building vizier from source (or a release consumer) gets a binary that no longer bundles or links sherpa-onnx's static ONNX Runtime, eliminating the version clash with fastembed and removing the sherpa prebuilt-lib re-download step from release builds.

**Why this priority**: This is the underlying motivation — sherpa-onnx's bundled runtime blocks newer ONNX models and complicates the build.

**Independent Test**: `cargo build --release` succeeds without sherpa-onnx-sys, and the release workflow no longer references sherpa prebuilt libs.

**Acceptance Scenarios**:

1. **Given** the new dependency set, **When** a release build runs, **Then** no sherpa-onnx prebuilt download step is required.
2. **Given** fastembed's `ort` dependency, **When** the binary links, **Then** there is no second, conflicting ONNX Runtime in the link graph.

---

### Edge Cases

- What happens when a user has a stale sherpa-onnx model cache from a previous version? The old cache directory is ignored; the new runtime downloads its own artifact into a new path. No crash, no migration of bytes.
- How does the system handle a model download interrupted mid-flight? Existing retry/progress logic is reused; partial files are not treated as complete.
- What about the `large-v3` default STT model name? It becomes a GGML artifact name; the provider default is updated accordingly.
- What about voice IDs that existed only in sherpa's Kokoro (e.g. multi-lang v1.1 voice list)? The new phonemizer/backend may expose a different voice set; documented defaults are updated and unknown voices fall back to voice 0 with a warning, as today.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST provide local speech-to-text via `whisper-rs` (whisper.cpp) instead of sherpa-onnx, behind the existing `VizierSttModel` trait, with no change to the `stt` tool's external behavior.
- **FR-002**: System MUST provide local text-to-speech via `any-tts` (Candle/Kokoro) instead of sherpa-onnx, behind the existing `VizierTtsModel` trait, with no change to the `tts_generate` tool's external behavior.
- **FR-003**: System MUST remove `sherpa-onnx` and `sherpa-onnx-sys` from `Cargo.toml` and all build/release references.
- **FR-004**: System MUST download and cache the new model artifacts through the existing SetupBoard progress reporting, into provider-specific cache directories.
- **FR-005**: System MUST not require espeak-ng (system package or data directory) on the host for either capability.
- **FR-006**: System MUST preserve the `whisper` STT provider and `kokoro` TTS provider entries in the agent schema, updating default model names to the new artifacts.
- **FR-007**: System MUST keep all cloud STT/TTS providers (OpenAI, OpenRouter, Groq, ElevenLabs, Mistral, Gemini, HuggingFace, xAI, Hyperbolic) unchanged.
- **FR-008**: System MUST update documentation that references sherpa-onnx model paths and the local provider descriptions.
- **FR-009**: System MUST make the local providers the defaults: `SttProvider` defaults to `Whisper` and `TtsProvider` defaults to `Kokoro`, so enabling `stt`/`tts` without an explicit provider needs no cloud credentials. Tool-enablement booleans (`stt`/`tts`) remain `false` by default.

### Key Entities

- **WhisperSttModel**: Local STT implementation backed by whisper-rs; downloads a GGML Whisper model; transcribes WAV/PCM audio to text.
- **KokoroTtsModel**: Local TTS implementation backed by any-tts; downloads Kokoro weights; generates WAV audio for a voice ID.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A release build completes without any sherpa-onnx prebuilt download step.
- **SC-002**: Local STT and TTS work offline with no cloud credentials and no espeak-ng installed, verified by transcription and speech generation returning valid output.
- **SC-003**: The link graph contains a single ONNX Runtime (fastembed's) or none, never two conflicting ones.
- **SC-004**: First-run model download reports progress in the WebUI setup board for both capabilities, as before.

## Assumptions

- `whisper-rs` and `any-tts` versions chosen are compatible with the Rust edition and toolchain already used by vizier (edition 2024).
- Model quality for the new artifacts is comparable enough that existing tests/prompts do not need gold-standard updates.
- The `whisper` and `kokoro` provider names are retained for backward compatibility with saved agent configs, even though the underlying runtime changes.
- The default provider for STT/TTS becomes the local option (`whisper`/`kokoro`); cloud providers remain selectable. Tool-enablement flags stay off by default, so model downloads only happen when a user opts in.
- Cloud providers are the recommended path for users who need the highest quality or specific voices; local providers remain best-effort/offline options.