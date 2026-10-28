# Implementation Plan: Replace sherpa-onnx with whisper-rs and any-tts

**Branch**: `032-replace-sherpa-whisper-anytts` | **Date**: 2026-10-27 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/032-replace-sherpa-whisper-anytts/spec.md`

## Summary

Replace the two local media runtimes (Whisper STT in `src/stt/whisper.rs` and Kokoro TTS in `src/tts/kokoro.rs`) that currently depend on `sherpa-onnx`/`sherpa-onnx-sys` with `whisper-rs` (whisper.cpp, GGML) for STT and `any-tts` (Candle/Kokoro) for TTS. Both new crates are pure Rust with no espeak-ng and no bundled ONNX Runtime, eliminating the version clash with fastembed and the sherpa prebuilt-lib release step. The `VizierSttModel`/`VizierTtsModel` traits, agent schema provider names (`whisper`/`kokoro`), download/progress machinery, and all cloud providers remain unchanged.

## Technical Context

**Language/Version**: Rust, edition 2024 (per `Cargo.toml`)
**Primary Dependencies**: `whisper-rs` (+ `whisper-rs-sys`), `any-tts` (Candle), replacing `sherpa-onnx`/`sherpa-onnx-sys`; `hound`, `symphonia`, `reqwest`, `tar`, `bzip2`, `fastembed` (unchanged)
**Storage**: SQLite (unchanged); model artifacts cached under `<workspace>/.runtime/stt/whisper/` and `<workspace>/.runtime/tts/kokoro/`
**Testing**: `cargo test` / `cargo clippy`; dummyplug e2e (STT/TTS tools are agent-observable) per constitution
**Target Platform**: Linux/macOS/Windows, x86_64 + aarch64 (cross builds in `Cross.toml`)
**Project Type**: single binary (CLI + HTTP + channels)
**Performance Goals**: comparable transcription/synthesis latency to sherpa-onnx baseline; no regression in first-run download UX
**Constraints**: single self-contained binary; no espeak-ng system dependency; no second ONNX Runtime in link graph; offline-capable local providers
**Scale/Scope**: two implementation files rewritten; two Cargo deps swapped; release workflow sherpa steps removed; docs updated

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. Lean by Default**: Swapping one runtime for another with concrete offline-STT/TTS need; removing sherpa deps reduces weight. ✓
- **II. DRY via Trait-Based Extensibility**: Both impls behind existing `VizierSttModel`/`VizierTtsModel` traits; no new type-branching. ✓
- **III. Self-Contained, Zero-Dependency Runtime**: New crates are pure Rust/static; model downloads remain opt-in behind `stt`/`tts` flags. ✓
- **IV. Portability by Default**: whisper-rs-sys compiles whisper.cpp (may need C++ toolchain at build); must verify `cross build` targets and add pre-build steps if needed. ⚠ (tracked in research)
- **V. Unified Errors & Observability**: All fallible code returns `crate::Result`; logging via `tracing`. ✓
- **Distribution constraints**: no new external service; single binary; dependency weight reviewed (whisper-rs + any-tts vs sherpa). ✓
- **e2e gate**: STT/TTS tools are agent-observable → dummyplug quickstart steps required.

**Result**: PASS (portability caveat tracked, not blocking).

## Project Structure

### Documentation (this feature)

```text
specs/032-replace-sherpa-whisper-anytts/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
└── contracts/
```

### Source Code (repository root)

```text
src/
├── stt/
│   ├── mod.rs            # unchanged (trait + dispatch)
│   └── whisper.rs        # REWRITE: whisper-rs backend
├── tts/
│   ├── mod.rs            # unchanged (trait + dispatch)
│   └── kokoro.rs         # REWRITE: any-tts backend
Cargo.toml                # swap sherpa-onnx -> whisper-rs, any-tts
.github/workflows/release.yml  # remove sherpa prebuilt steps
docs/src/configuration/tools-embedding.md  # update local provider descriptions
```

**Structure Decision**: Single project (existing layout). Changes are localized to two implementation files plus dependency/build/docs edits; no new modules.

## Complexity Tracking

> No constitution violations requiring justification. The portability caveat (whisper-rs-sys C++ build) is handled in research/quickstart, not a complexity exception.