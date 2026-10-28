# Data Model: Replace sherpa-onnx with whisper-rs and any-tts

No new persisted entities. The change is confined to runtime implementation and dependency metadata. Existing entities affected:

## AgentConfig (unchanged shape)

- `tools.stt.provider = whisper` and `tools.tts.provider = kokoro` remain valid enum values.
- `tools.stt.model` default changes from `large-v3` (sherpa artifact name) to the GGML Whisper artifact name (e.g. `ggml-large-v3`).
- `tools.tts.model` default changes from `kokoro-en-v0_19` (sherpa archive name) to the any-tts/Kokoro artifact name.
- `tools.tts.voice` default remains `af`; unknown voice IDs fall back to 0 with a warning.

## Cached model artifacts (filesystem, per agent workspace)

- `.runtime/stt/whisper/<artifact>/` — GGML Whisper model files.
- `.runtime/tts/kokoro/<artifact>/` — Kokoro weights + voices.
- Old `.runtime/stt/whisper/sherpa-onnx-whisper-*` and `.runtime/tts/kokoro/kokoro-*-v*` directories are left in place but ignored by the new code.

## No schema/storage changes

- `VizierSttModel` / `VizierTtsModel` traits unchanged.
- `SttProvider` / `TtsProvider` enums unchanged (variant names retained for backward compat).
- No SQLite migration required.