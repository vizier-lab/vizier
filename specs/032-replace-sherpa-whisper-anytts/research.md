# Research: Replace sherpa-onnx with whisper-rs and any-tts

## Decisions

### D1: STT runtime — whisper-rs (whisper.cpp)

- **Decision**: Use `whisper-rs` (bindings to whisper.cpp) for local STT.
- **Rationale**: Mature, widely used, no ONNX dependency (GGML), so it cannot clash with fastembed's `ort`. Whisper.cpp supports the models vizier already ships (large-v3 etc.) as GGML `.bin` files. Streaming and VAD are available if needed later. The `VizierSttModel` trait is unchanged.
- **Alternatives considered**: Parakeet via `ort` directly (reimplements mel/tokenizer glue), transcribe-rs (extra abstraction), whisperfile (subprocess). Rejected for complexity or runtime weight.
- **Build note**: `whisper-rs-sys` compiles whisper.cpp from source by default; requires a C/C++ toolchain at build time. For cross targets in `Cross.toml`, add the compiler to `pre-build` if missing (verify before merge).

### D2: TTS runtime — any-tts (Candle/Kokoro)

- **Decision**: Use `any-tts` for local TTS, specifically its Kokoro backend.
- **Rationale**: Pure Rust + Candle, no ONNX, no espeak-ng (ships an in-tree pure-Rust phonemizer). Kokoro is the model vizier already ships, so voice IDs and quality stay comparable. Removes the espeak-ng data directory shipping entirely.
- **Alternatives considered**: piper-rs (broad voice catalog but still espeak-ng-based and re-enters `ort` version alignment), pocket-tts (optional espeak-ng OOD fallback, different voice model), kokoroxide (espeak-ng required). Rejected to honor the "no espeak-ng" and "no ONNX conflict" goals.
- **Caveat**: any-tts is newer; pin a specific version and verify CPU performance on the release targets. Voice set may differ from sherpa's multi-lang v1.1 list — unknown voices fall back to 0 with a warning (existing behavior).

### D3: Model artifacts and cache paths

- **Decision**: Download GGML Whisper `.bin` files and Kokoro weights (`.safetensors`/`.onnx`+`voices.bin` per any-tts's format) into new cache directories under `<workspace>/.runtime/stt/whisper/` and `<workspace>/.runtime/tts/kokoro/`. Old sherpa cache dirs are left untouched and ignored.
- **Rationale**: Keeps the SetupBoard download/progress flow and per-agent cache layout; avoids byte-migrating incompatible artifacts.

### D4: Provider schema

- **Decision**: Keep `SttProvider::Whisper` and `TtsProvider::Kokoro` enum variants and their names for backward compat with saved agent configs; update `default_model()` strings to the new artifact names.
- **Rationale**: Saved agents deserialize by variant name; renaming would break existing configs. Only the default model string changes.

### D5: Removals

- **Decision**: Remove `sherpa-onnx` and `sherpa-onnx-sys` from `Cargo.toml`; remove the sherpa prebuilt re-download steps from `.github/workflows/release.yml`; delete `src/stt/whisper.rs` sherpa code and `src/tts/kokoro.rs` sherpa code (rewritten, not patched).
- **Rationale**: Principle I (lean) — no dead dependency or dead build steps. Update `docs/src/configuration/tools-embedding.md` local provider descriptions.

### D6: espeak-ng

- **Decision**: No espeak-ng system package or data directory is required after the swap; any-tts's in-tree phonemizer replaces it.
- **Rationale**: Satisfies FR-005 and Principle III (no hidden runtime dependency).