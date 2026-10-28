# Quickstart: Replace sherpa-onnx with whisper-rs and any-tts

## Prerequisites

- `cargo build --release` succeeds with the new dependency set (no sherpa-onnx-sys).
- A dummyplug agent is configured (see `specs/008-dummyplug-provider`).

## Build verification

1. `cargo build --release` — must succeed without sherpa prebuilt download.
2. `cargo clippy` and `cargo test` — must pass.
3. Confirm `Cargo.lock` no longer contains `sherpa-onnx` / `sherpa-onnx-sys`.
4. Confirm `.github/workflows/release.yml` no longer references sherpa prebuilt libs.

## Dummyplug e2e — STT

1. Start vizier with a dummyplug agent that has `stt` enabled (`provider: whisper`).
2. Send `tools` → expect `stt_transcribe` listed.
3. Send `stt_transcribe` → send a JSON request with a small WAV audio payload → expect a transcript string returned (no error).
4. Restart vizier; repeat transcription with cached model → expect no download and a transcript returned.

## Dummyplug e2e — TTS

1. Start vizier with a dummyplug agent that has `tts` enabled (`provider: kokoro`, `voice: af`).
2. Send `tools` → expect `tts_generate` listed.
3. Send `tts_generate` → send a JSON request with text → expect a session file (WAV) produced.
4. Verify no espeak-ng is installed on the host (e.g., `which espeak-ng` returns nothing) and TTS still succeeds.

## Manual verification

- First-run download: delete `.runtime/stt/whisper/` and `.runtime/tts/kokoro/`, trigger STT/TTS, confirm WebUI setup board shows download progress for both.
- Cross-build: run `cross build` for at least one aarch64 target to confirm whisper-rs-sys compiles (add pre-build compiler steps to `Cross.toml` if needed).