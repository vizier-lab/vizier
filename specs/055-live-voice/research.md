# Phase 0 Research: Live Voice Conversation

**Feature**: `specs/055-live-voice` | **Date**: 2026-11-08 | **Author**: `@system-analyst`

Everything below was checked against the working tree at
`/home/muruyung/ngoding/pribadi/vizier` on branch `055-live-voice`. Where a claim comes from the
project's own source, the file and line are given. Where a claim is a judgement rather than a
measurement, it says so. Nothing here was measured in a browser: no profiling run was made, so every
latency figure in `plan.md` is a budget, not a result.

## A. What the repository already does (verified)

| Fact | Evidence | Why it matters |
|---|---|---|
| Recognition is a whole clip in, one string out | `crates/vizier-core/src/stt/mod.rs:10-18` — `async fn transcribe(&self, audio: &[u8], filename: &str, language: Option<&str>) -> Result<String>` | No streaming recogniser exists, so the cascade recognises one utterance per call |
| Synthesis is a whole string in, one buffer out | `crates/vizier-core/src/tts/mod.rs:13-15` — `async fn generate_speech(&self, text: &str, voice: &str, speed: f32) -> Result<Vec<u8>>` | A reply must be cut into segments to be heard before it ends |
| Every TTS provider returns the full body | `llm/media/openai_shaped.rs:122-128` (`mp3_to_wav(&response.body)`), `tts/elevenlabs.rs:25`, `tts/kokoro.rs` | "Streaming TTS" would mean a new provider shape; the plan does without it |
| The LLM layer has no streaming API at all | `crates/vizier-core/src/llm/sse.rs:1-3` — *"like rig we read the full body and parse it once (research R6): no streaming API"* | Token-by-token speech is impossible without a separate feature |
| A turn's text arrives as one frame | `crates/vizier-core/src/schema/response.rs:129-132` — `Message { content, stats }`, emitted at `agents/agent/mod.rs:1098` | Confirms the same limit from the protocol side |
| Speech is per agent, behind a tool switch | `crates/vizier-core/src/schema/agent.rs:196-201` — `tts: ToolConfig<TtsToolSettings>`, `stt: ToolConfig<SttToolSettings>`; `ToolConfig { enabled, settings }` at `:206-209` | Live Voice needs its own switch, not theirs (FR-003) |
| Speech providers are chosen per agent, with credentials resolved per provider | `schema/agent.rs:349-357` (`TtsToolSettings`), `:406-412` (`SttToolSettings`) | Reuse rather than a second configuration surface |
| The feature-switch pattern exists | `crates/vizier-core/Cargo.toml` features: `provider-media = ["dep:whisper-rs", "dep:any-tts", "dep:symphonia", "dep:symphonia-adapter-opus-rs", "dep:hound"]`, and `full` | The new flag follows the same shape |
| The WebSocket writes JSON text and ignores binary | `crates/vizier-http/src/api/v1/agents/channel.rs:804` (outbound `Message::Text`), `:818` — `Some(Ok(Message::Pong(_))) \| Some(Ok(Message::Binary(_))) => {}` | Audio in binary frames would be a protocol change; JSON + base64 is not |
| The bus envelope is capped at 1 MiB | `crates/vizier-schema/src/envelope.rs:11` — `pub const MAX_PAYLOAD: usize = 1_048_576` | 100 ms of PCM16 at 16 kHz is 3.2 KB (4.3 KB base64): a 240x margin |
| Outbound frames are one enum | `crates/vizier-core/src/transport/mod.rs:57-72` — `enum SessionFrame { Response, Job, Reactions, ClientToolCall, ... }` | One variant carries all live voice traffic |
| Clients already ignore unknown frames | `webui/app/api/frames.ts:607-630` — `classifyFrame` returns `null` when no known top-level key matches | An older client sees no change (supports FR-040) |
| The recording path captures PCM through a worklet | `webui/app/lib/wavRecorder.ts:1-16`, `:14` — `WORKLET_URL = '/vizier-capture-worklet.js'` | The capture tap for live voice already exists and is CSP-safe |
| The microphone is requested without constraints | `webui/app/screens/chat/Composer.tsx:295-297` — `getUserMedia({ audio: true })` | Echo cancellation has to be asked for explicitly (FR-026) |
| Playback is a plain `<audio>` element today | `webui/app/screens/chat/ReplyMedia.tsx` (`AudioPlayer`, attachment URLs) | Not reusable for gapless segments: needs an `AudioContext` queue |
| There is a stop control and an abort command | `webui/app/screens/chat/Composer.tsx:38` — `{ name: 'abort', description: 'Stop what the agent is doing' }`; `crates/vizier-core/src/agents_client.rs:415` — `Abort { agent_id, session }` | FR-027 (interrupt cancels the turn) rides on an existing path |
| The composer already distinguishes spoken messages | `webui/app/lib/frames.ts:36` — `spoken?: boolean` on a queued message; `webui/app/routes/chat.tsx:559` passes it | FR-011's "marked as spoken" has a place to go |
| The desktop app's CSP forbids wasm in workers | `app/src-tauri/tauri.conf.json:14` — `default-src 'self' ipc: …`, no `script-src`/`worker-src` extension | A wasm VAD (`@ricky0123/vad-web`) cannot ship as-is; see the dependency report |

## B. Decisions

### R1 — Transport: the existing topic WebSocket, JSON, base64 PCM16, 100 ms chunks

One socket per topic already exists, is authenticated, has ping/pong, an idle timeout and an
outbound queue with a size cap (`channel.rs:442`, `MAX_RESULT_BYTES`). Audio is just more frames on
it. Chunk size is a trade: 100 ms is small enough that a barge-in has little audio left to discard
and large enough that the per-frame JSON overhead (~40 bytes) stays under 1% of the payload.

*Rejected*: WebRTC (needs signalling, a TURN server for restrictive networks, and
`livekit-client` at 12.3 MiB unpacked with ten transitive dependencies, for a two-party stream that
crosses one hop). *Rejected*: binary WebSocket frames (they are dropped today, so every existing
client and proxy path would need to change; the 4.3 KB chunks do not need the saving).

### R2 — Voice activity detection: energy gate on the client, no model

A model-based VAD (Silero through `@ricky0123/vad-web`) is more robust in noise, but it needs a wasm
worker, an onnx runtime and a ~2 MB model, and the desktop CSP (`default-src 'self'`, no
`wasm-unsafe-eval`, no `worker-src`) blocks it as shipped. An energy gate with an adaptive noise
floor and an 800 ms hangover is a few hundred lines, runs on the audio thread, and is adequate for
the room this feature is used in. It is the mandatory path; the model is the optional upgrade, and
the VAD sits behind a small interface so the upgrade does not touch the session.

*Consequence*: the VAD is the weakest link in a noisy room, which is exactly why FR-020's interrupt
control is a first-class path and not a fallback.

### R3 — Interruption is server-authoritative, with a generation counter

The client cannot be trusted to notice that its own synthesis should stop: the audio may already be
queued in the socket, and a second tab, a reconnected socket or a slow network all make "who is
speaking now" a server question. Every session holds a generation; every audio frame carries it; an
interruption increments it. The synthesis task checks the generation between segments and exits, the
client drops any frame whose generation is stale, and the end-of-turn bookkeeping marks the reply
interrupted. The cost is one `tokio::sync::watch<u64>` per session — no new dependency
(no `tokio-util` in the workspace, so no `CancellationToken`).

### R4 — Recognition: one utterance, one call

The cascade recognises the closed utterance, not a stream. This adds the hangover (800 ms) plus one
provider round trip to every turn; it also means a mis-recognition costs one message, not a whole
session. Where a local whisper model is configured the call is CPU-bound and slower — which is why
`plan.md`'s latency budget separates hosted from local and why the success criterion is measured on a
hosted provider.

### R5 — Speech: segment the reply, synthesise one ahead

The reply's text is cut at sentence and clause boundaries, each segment is synthesised, converted to
PCM16 16 kHz mono, and scheduled. Synthesising segment *n+1* while *n* plays removes the gap between
segments; the first segment is short by construction (the segmenter prefers the first sentence
boundary after ~80 characters), which is what keeps time-to-first-word inside SC-001. Code fences,
tables and raw URLs are dropped from the spoken form with one short notice (FR-014).

*Rejected*: synthesising the whole reply once (a 300-word answer would be silent for seconds and
could not be interrupted before its audio existed). *Rejected*: a streaming TTS provider (a new
per-provider streaming shape, and per-minute billing for audio we already pay for per character).

### R6 — Echo: the browser cancels it, with a playback guard

`getUserMedia({audio: {echoCancellation: true, noiseSuppression: true, autoGainControl: true}})` is
the platform's AEC and is free. It is best-effort, so the VAD raises its threshold while the reply is
audible and ignores the first 150 ms after a segment starts. SC-008 (80% volume, no headphones, ten
minutes, no self-interruption) is the release gate for this decision; if it fails on a target browser
the answer is a stronger guard, not a new dependency.

### R7 — The interrupt reuses the existing abort path

`Command::Abort` already cancels a running turn and the UI already shows "Stopped by you"
(`MessageList.tsx:292`). FR-027 therefore adds no new cancellation machinery: the session calls the
same path and marks the reply interrupted. The only new ordering rule is "stop the audio first, then
cancel", so no further words are heard while the turn unwinds.

### R8 — One seam, and only one

`LiveVoiceEngines` is the single trait: `transcribe(utterance)`, `speak(segment) -> PCM`. The cascade
implements it today; a realtime speech-to-speech pipeline would implement it later (FR-036a). Nothing
else in this feature gets an abstraction — the session, the segmenter and the VAD are concrete, per
Constitution Principle I.

### R9 — Compatible by construction

The new frames carry a distinct top-level key (`live_voice`), so the existing client's
`classifyFrame` returns `null` and ignores them (`frames.ts:607-630`); the new `SessionFrame` variant
is additive; the new agent setting is `#[serde(default)]`. With the flag off, no frame is emitted and
the capability reads false.

## C. Dependency report

Fetched from `registry.npmjs.org` on 2026-11-08 (raw `curl` of `/latest`). Sizes are the registry's
`dist.unpackedSize` — the installed-on-disk size, not the bundle's contribution, which is measured at
install time.

### Web (npm)

| Package | Version | Unpacked | Licence | Why | Cost / effect | Verdict |
|---|---|---|---|---|---|---|
| *none* | — | 0 | — | The mandatory path uses `AudioContext`, `AudioWorklet` (already shipped), `WebSocket` (already open) and a hand-written energy VAD | No bundle growth, no new supply-chain surface | **Required: none** |
| `@ricky0123/vad-web` | 0.0.31 | 6,755,237 B (~6.4 MiB) | ISC | Model-based VAD (Silero) instead of the energy gate: better in a noisy room, fewer false barge-ins | Pulls `onnxruntime-web`; needs `script-src 'self' 'wasm-unsafe-eval'` and `worker-src 'self'` in `tauri.conf.json`, assets self-hosted from `webui/public/`; adds a wasm worker on the audio path | **Optional** (upgrade path, off by default) |
| `onnxruntime-web` | 1.31.1 | 150,622,959 B (~143.6 MiB) | MIT | Transitive dependency of `vad-web` only | Ships several backends; only the wasm build is bundled, so the real bundle delta must be measured — the registry size overstates it by an order of magnitude | **Optional, transitive** (never direct) |
| `livekit-client` | 2.23.0 | 12,946,924 B (~12.3 MiB) | Apache-2.0 | Would give WebRTC transport | Ten transitive dependencies (`jose`, `protobufjs`-class, `webrtc-adapter`, …), a signalling service and a TURN server for NAT traversal | **Rejected** |

### Rust crates

| Crate | Version | Already in the workspace | Why | Verdict |
|---|---|---|---|---|
| `tokio` | 1.53.3 resolved (`Cargo.lock:9741-9742`; the manifest's floor is `1.49.0`, `Cargo.toml:45`) | yes (`sync` feature) | `watch` for the generation counter, `time` for the idle stop and the hangover | already present |
| `serde` / `serde_json` | workspace | yes | Frame encoding | already present |
| `base64` | 0.23 | yes | PCM chunk encoding on the frame | already present |
| `hound` | 3.5 | yes, behind `provider-media` | WAV header for the utterance handed to `transcribe` | already present (or a 44-byte header by hand, if the flag must not imply `provider-media`) |
| `tokio-util` | — | **no** | `CancellationToken` would be the obvious tool | **not added**: `tokio::sync::watch<u64>` does the job with a dependency already in the tree |

### Models and assets

| Asset | Size | Where | Needed | Verdict |
|---|---|---|---|---|
| Silero VAD ONNX (bundled inside `@ricky0123/vad-web`) | ~2 MB, not verified individually | would be copied to `webui/public/` | only on the optional wasm VAD path | **Optional** |
| Existing local models (whisper-rs, Kokoro via `any-tts`) | unchanged | `provider-media` | already there | no change |

### Net effect

**Zero new mandatory dependencies in Rust and in the web bundle.** The optional wasm VAD adds
~2 MB of assets and one CSP relaxation; that cost is why it is not the default.

## D. What is *not* verified

* Latency numbers: budgets from provider round-trip reasoning, not measurements.
* Safari and mobile-WebView behaviour: `AudioWorklet` and background suspension are assumed per
  platform documentation, not tested here.
* The real bundle delta of the optional wasm VAD (needs an install and a build).
* Whether `any-tts`'s local Kokoro model is fast enough for segment-sized synthesis (unmeasured; the
  plan keeps it out of the default live-voice path).
* The exact behaviour of the desktop WebView's AEC (platform-provided, unmeasured; SC-008 decides).

