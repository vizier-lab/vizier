# Implementation Plan: Live Voice Conversation

**Branch**: `055-live-voice` | **Date**: 2026-11-08 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/055-live-voice/spec.md` (draft resumed and clarified
2026-11-08). This plan is written by `@system-analyst` on the routing decision of `@bro-pm` for
JOB: SPEC-055; the usual split would put `plan.md` with `@mamang-lead-be`. Nothing here is
implemented yet: spec and plan only.

## Summary

Add a **Live Voice** capability to a conversation: one control starts a session, the microphone stays
open, each finished utterance is transcribed and submitted as an ordinary message, the agent answers,
and the answer is spoken in segments while the rest of it is still being synthesised. Two
interruptions: the person speaking over the agent (barge-in, detected on the client) and a dedicated
interrupt control. The feature is separate from the existing speech tools and is off unless pressed.

The technical shape is fixed by what the repository already is:

* Audio travels over the **existing per-agent WebSocket** as JSON frames with base64 PCM16, 100 ms
  per chunk. No WebRTC, no second socket, no new HTTP endpoint.
* Voice activity detection runs **in the browser**, on the AudioWorklet capture path that already
  exists, using an energy gate with an adaptive noise floor. No model, no new dependency.
* Speech goes through the **existing recogniser and voice** (`VizierStt`, `VizierTts`). Both are
  whole-buffer, so a reply is **segmented** and each segment synthesised in turn.
* Interruption is **server-authoritative**: a generation counter per session; every audio frame
  carries its generation, and an interruption bumps it so in-flight synthesis is abandoned and late
  frames are dropped by the client.

## Technical Context

**Language/Version**: Rust (workspace, edition 2021, toolchain as pinned in `rust-toolchain`), TypeScript + React 19 (webui), Tauri 2 (desktop shell).

**Primary Dependencies**: no new mandatory dependency.
Rust: `tokio` (already, with `sync` for `watch`), `serde`, `base64` 0.23, `hound` 3.5 (already, behind
`provider-media`) for the WAV header, `any-tts`/`symphonia` untouched.
Web: `AudioContext`, `AudioWorklet` (already used by `webui/public/vizier-capture-worklet.js`), WebSocket (already open per topic).

**Storage**: none. Audio is never written to disk (FR-016); only transcript and reply text, through the existing conversation storage.

**Testing**: `cargo test` per crate and per feature flag (`vizier-core` with and without `provider-live-voice`), `cargo test -p vizier-http` for the frame contract, `vitest` for the client state machine, VAD and segmenter (`webui/app/lib/liveVoice/*.test.ts`), Playwright for the two interruption paths with a fake provider, and dummyplug steps in [quickstart.md](./quickstart.md) for the agent-observable half. Speech providers are faked in every automated test; one live-provider check is marked as such.

**Target Platform**: the chat interface only — the WebUI in a browser and the desktop app's WebView (FR-035, Clarifications C1). Discord and Telegram are out of scope.

**Performance Goals**: end of the person's speech to the first audible word of the answer, with a hosted recogniser and voice: p90 <= 3.5 s and p99 <= 5 s is the release gate, 2.0 s at p50 is the stretch target (SC-001 as restated in the Lead's review; the table below shows why 2 s p90 was not reachable); barge-in silences the audio on the client in <= 50 ms and server-side in <= 300 ms (SC-002); the interrupt control silences it in <= 200 ms (SC-003).

**Constraints**: the variable-size bus envelope is capped at 1 MiB (`crates/vizier-schema/src/envelope.rs:11`), so audio is chunked; frames are JSON text (binary frames are ignored today, `crates/vizier-http/src/api/v1/agents/channel.rs:818`); the turn protocol delivers finished messages, not token deltas (`crates/vizier-core/src/llm/sse.rs:1-3`); the desktop app's CSP is `default-src 'self'` with no wasm allowance (`app/src-tauri/tauri.conf.json:14`), which rules out a wasm VAD worker as shipped.

**Scale/Scope**: one session per conversation (FR-005); one cargo feature flag; one new frame family
(top-level key `live_voice`); two new client controls; roughly 6 new modules, no schema migration.

## Constitution Check

*GATE: must pass before Phase A. Re-check after Phase E.*

| Gate (constitution) | How this plan passes it |
|---|---|
| **I. Lean by default** | No new mandatory dependency in Rust or in the web bundle. The one seam (a trait between the session and its speech) exists because FR-036a requires a second implementation to be possible later; the session itself is a concrete type. The existing worklet, player and recogniser are reused rather than duplicated. |
| **II. DRY via trait seams** | Speech enters through `VizierStt`/`VizierTts` behind one `LiveVoiceEngines` trait, never through `match` over providers. Recognition and synthesis stay one implementation per provider, as they are today. |
| **III. Optional capability behind a flag** | `provider-live-voice` in `vizier-core` (`default = []`, added to `full`). With the flag off, no frame is ever emitted, the capability reads false, the control is hidden, and the existing speech tools are unchanged (FR-040, FR-041). Adding a `SessionFrame` variant is additive: the client's `classifyFrame` returns `null` for an unknown top-level key (`webui/app/api/frames.ts:607-630`), so an older client ignores the new frames instead of failing. |
| **Design before UI code** | The control layout, states and both interruption paths are drawn in [diagrams.md](./diagrams.md) (wireframe + Mermaid) before any `.tsx` is written. |
| **Verification per flag and crate** | Every phase below names the job that proves it: `vizier-core` on/off the flag, `vizier-http`, `webui` vitest, Playwright, and the documented full matrix on develop/master/release. |

No violations, so the complexity table at the end stays empty.

## Project Structure

### Documentation (this feature)

```text
specs/055-live-voice/
├── spec.md                     # The clarified specification
├── plan.md                     # This file
├── research.md                 # Phase 0: findings, decisions, dependency report
├── data-model.md               # Phase 1: entities, states, frame payloads
├── diagrams.md                 # Architecture, both interruption sequences, wireframe
├── quickstart.md               # Phase 1: how to run and verify it by hand
├── contracts/
│   └── live-voice-frames.md    # Phase 1: the wire contract
└── tasks.md                    # Phase 2 (speckit_tasks, @mamang-lead-be) — not written here
```

### Source Code (repository root)

```text
crates/vizier-core/src/
├── live_voice/
│   ├── mod.rs              # feature gate, capability, re-exports
│   ├── session.rs          # one session: state machine, generation counter, registry
│   ├── engines.rs          # LiveVoiceEngines trait; cascade impl over VizierStt/VizierTts
│   ├── utterance.rs        # PCM buffer -> WAV -> transcript (FR-010, FR-011)
│   └── segmenter.rs        # reply text -> speakable segments (FR-013, FR-014)
├── schema/agent.rs         # + live_voice: ToolConfig<LiveVoiceSettings>  (additive, defaulted)
└── transport/mod.rs        # + SessionFrame::LiveVoice(LiveVoiceFrame)

crates/vizier-http/src/
├── api/v1/agents/channel.rs  # inbound `live_voice_*` frames -> session; existing auth reused
└── topic.rs                  # audio to the owning connection; control frames keep the session broadcast

webui/app/
├── lib/liveVoice/
│   ├── vad.ts              # energy VAD on the worklet tap, adaptive floor, hangover, guard
│   ├── capture.ts          # getUserMedia with echoCancellation, 100 ms PCM16 chunks
│   ├── session.ts          # client state machine + frame codec (pure, unit-tested)
│   ├── player.ts           # AudioContext queue, gapless segments, immediate stop()
│   └── segmenter.test.ts   # mirrors the server segmenter's expectations
└── screens/chat/
    ├── LiveVoiceButton.tsx # the one control (off/on), next to the mic and the spoken toggle
    └── LiveVoiceBar.tsx    # state, level meter, the interrupt control, the reason when ended
```

**Structure Decision**: the engine stays in `vizier-core` (it must run without the web layer, per spec
037's core extraction); the socket side stays in `vizier-http` where the existing auth, topic routing
and client limits already live; everything audible stays in `webui/app`, because only the browser can
reach the microphone and the speakers. The desktop app inherits the webui with no separate code path.

## Phase 0 — Research (done; see research.md)

The decisions that shape the rest of the plan, each with the evidence it rests on:

| # | Decision | Evidence |
|---|---|---|
| R1 | Reuse the topic WebSocket, JSON + base64 PCM16, 100 ms chunks | `channel.rs:799-820` (write half is JSON text; binary is dropped), `envelope.rs:11` (1 MiB) |
| R2 | Client-side energy VAD, no model | CSP `default-src 'self'`, no wasm allowance (`tauri.conf.json:14`); worklet already shipped |
| R3 | Server-authoritative interruption with a generation counter | inbound frames arrive over the same socket, so a stale synth task must be told to stop by the server, not trusted to notice |
| R4 | One utterance per recognition call (batch providers) | `stt/mod.rs:10-18` — `transcribe(audio, filename, language)`, whole clip |
| R5 | Reply spoken in segments, synthesised one ahead | `tts/mod.rs:13-15` — `generate_speech(text, voice, speed) -> Vec<u8>`, whole buffer |
| R6 | Echo cancelled by the browser, plus a playback guard | `Composer.tsx:295-297` currently asks `audio: true`; constraints are ours to set |
| R7 | Interruption reuses the existing abort path | `AgentsClient::abort(agent_id, session)` (`crates/vizier-core/src/agents_client.rs:205-207`, impl `:298-312`), which sends `VizierRequestContent::Command("abort")` — the same thing `Composer.tsx:38`'s `/abort` sends. There is no `AgentCommand::Abort`; `agents_client.rs:415` is a test fake (`review.md` D12) |
| R8 | No new dependency is mandatory | audit in research.md §Dependency report |

## Phase A — Capability and configuration (no audio yet)

**Goal**: the control can exist, be off, and prove it changes nothing.

1. `vizier-core`: new feature `provider-live-voice`; `live_voice` module behind it; `full` includes it.
2. `schema/agent.rs`: `live_voice: ToolConfig<LiveVoiceSettings>` — `enabled` plus optional
   `stt`/`tts` overrides that default to the agent's existing speech settings. Both defaulted, so a
   stored agent file without the block still loads.
3. Agent detail response gains `live_voice: bool` beside the existing `stt`/`tts` booleans
   (`webui/app/routes/chat.tsx:104-119` already reads those two).
4. **Exit criteria**: `cargo test -p vizier-core --features provider-live-voice` and without it;
   `cargo test -p vizier-http`; a webui build where the control is absent because the flag is off.

## Phase B — Server session and recognition

**Goal**: press the control, speak, and see a message appear; nothing is spoken yet.

1. `session.rs`: registry keyed by (agent, conversation), one session each (FR-005); states from
   FR-006; end reasons; idle timer (FR-029); a `tokio::sync::watch<u64>` generation.
2. Inbound frames `live_voice_start`, `live_voice_audio`, `live_voice_utterance_end`,
   `live_voice_stop` accepted on the existing socket, authorised exactly like a chat message.
3. `utterance.rs`: accumulate PCM16 for the open utterance, cap at 60 s (FR-010's default), build a
   WAV header with `hound`, call `VizierStt::transcribe`, emit `live_voice_transcript`, submit the
   transcript as a normal request through the same path a typed message takes (FR-007, FR-011).
4. **Exit criteria**: `cargo test -p vizier-http` with a faked recogniser: an utterance produces
   exactly one request, whose text is the transcript; a flag-off build rejects the frames as unknown
   without disturbing the socket.

## Phase C — Progressive speech out

**Goal**: the answer is heard, in order, and can be abandoned mid-sentence.

1. `segmenter.rs`: reply text -> speakable segments at sentence and clause boundaries, skipping code
   fences, tables and raw URLs with a short spoken notice (FR-014).
2. `engines.rs`: `LiveVoiceEngines` trait; the cascade implementation synthesises segment *n+1* while
   *n* is being sent, converts to PCM16 16 kHz mono, and emits `live_voice_speech` frames carrying
   `generation`, `seq`, `final`.
3. `transport/mod.rs`: `SessionFrame::LiveVoice(LiveVoiceFrame)` carries the control frames (`state`,
   `transcript`, `error`, `stall`) and rides the session broadcast like the rest; **audio**
   (`live_voice_speech`) is written to the socket that owns the session
   (`ClientConnection::send`, `crates/vizier-core/src/agents/client/connection.rs`) and never enters
   the broadcast, so it can be dropped without ever dropping a reply's text (`review.md` D9). One
   socket, one auth, one queue that already exists.
   **Where the drop actually happens**: not in `channel.rs` (there is no room there — its outbound
   queue is `flume::unbounded()`, `channel.rs:374`, and its writer is `mpsc(64)`, `:780`), but in the
   socket's own outbound loop, `TopicOutbound::run`: it sees the depth (`self.out.len()`) and skips a
   `speech` frame above `LIVE_VOICE_SOCKET_BACKLOG = 16`, reporting one `stall` frame per episode. No
   new channel and no `TopicConnection::open` signature change (`architecture.md` §11, `tasks.md`
   T020–T022).
4. Honest limit: a single long message is segmented only after its text exists (Clarifications,
   constraint note), so latency is "first segment", not "first token".
5. **Exit criteria**: `cargo test -p vizier-core --features provider-live-voice` — a three-segment
   reply yields three ordered frames whose concatenated PCM equals the faked synthesiser's output; a
   first frame is emitted before the last segment is requested.

## Phase D — Client capture, VAD and barge-in

**Goal**: the person speaks, the agent stops.

1. `capture.ts`: `getUserMedia({audio: {echoCancellation: true, noiseSuppression: true, autoGainControl: true}})`,
   the existing worklet as the tap, downmix to 16 kHz mono PCM16, 100 ms chunks (FR-026, C5).
2. `vad.ts`: energy gate over a 20 ms window with an adaptive noise floor; start threshold, stop
   hangover 800 ms (FR-010), and a **playback guard**: while the reply is audible, the gate is raised
   and ignores the first 150 ms after each segment starts, so the session does not interrupt itself
   (FR-026).
3. On speech start while audio is playing: stop the player locally (<= 50 ms, SC-002) and send
   `live_voice_speech_start`; the server bumps the generation, abandons synthesis and cancels the
   turn (FR-019, FR-027). **Onset guard, and what it does not delay**: `player.stop()` runs the
   moment the gate opens, so SC-002's 50 ms is measured from the first voiced sample and never waits
   for the guard; the guard (~250 ms of voiced energy) only decides when the *server* is told, which is
   what SC-002's 300 ms is measured from. A burst shorter than the guard resumes the segments already
   queued instead of leaving the reply half-spoken (FR-022) and submits nothing (FR-024), and the gate
   during `waiting` (nothing audible) is the same one used during playback — a cough or a door must not
   cancel a turn the person never heard (FR-026, SC-008; risk row below).
4. On utterance end: send `live_voice_utterance_end`; a burst shorter than a word stops the audio and
   submits nothing (Story 2, scenario 5; FR-024).
5. **Exit criteria**: `vitest` over `vad.ts` with recorded fixtures — speech over a playback sample
   triggers, playback alone at 80% volume does not; `session.ts` never emits `start` twice.

## Phase E — Client playback, interrupt control and UI

**Goal**: both interruption paths are usable, and the state is visible.

1. `player.ts`: one `AudioContext` resumed on the control press, an ordered queue of
   `AudioBufferSourceNode`s scheduled back to back, `stop()` that closes every source at once and
   drains the queue, and stale-generation frames dropped on arrival (FR-022).
2. `LiveVoiceButton.tsx` + `LiveVoiceBar.tsx` as drawn in [diagrams.md](./diagrams.md): one control
   to start and stop, the interrupt control present for the whole session, the state text from FR-006,
   the microphone-open indicator including when the window is unfocused (FR-038).
3. Typing during a session stops the audio like the interrupt control (FR-025).
4. Ends: press again, leave the page, permission withdrawn, conversation or agent gone, idle, reload
   (FR-028, FR-034) — every one releases the microphone and leaves the text (SC-007).
5. **Exit criteria**: Playwright with a faked provider — both interruption paths silence the audio and
   the session stays live; closing the tab releases the track; a second tab is refused (FR-005).

## Phase F — Hardening and documentation

1. Reconnect within 30 s keeps the session and does not duplicate the message in flight (FR-033).
2. Failure surfaces: recogniser empty (FR-031), voice provider failing mid-reply (FR-032), no device
   or permission refused (FR-030), lost device, screen lock on mobile — each a reason within 5 s
   (SC-009).
3. Docs: start and end, what audio goes to which provider, what is stored, the two interruptions, the
   limits (FR-039); one page under `docs/`, plus the build-flavour note for the new flag.
4. **Exit criteria**: the full job matrix for the crates and flags touched is green; the docs page is
   linked from the speech configuration page.

## Latency budget (assumption, to be measured in Phase F)

| Stage | Budget (p90) | Where it comes from |
|---|---|---|
| End of speech -> utterance closed | 800 ms | FR-010 default hangover |
| Utterance -> request on the wire | 120 ms | local WAV header + frame |
| Recognition | 900 ms hosted / up to 2500 ms local model | provider RTT; a local `whisper` model is CPU-bound |
| Turn start | 100 ms | existing scheduling |
| First `Message` frame | the model's own latency, not ours | unchanged by this feature |
| First segment synthesised | 600 ms hosted | one short segment, not the whole reply |
| First audio frame -> sound | 50 ms | `AudioContext` already resumed |
| **The model produces the whole message** | **not budgeted: the model's own latency** | the turn protocol has no token deltas (`llm/sse.rs:1-3`), so nothing can be spoken before the message exists; this is the one stage this feature cannot shorten |
| **End of speech -> first word of the answer** | **floor 2.57 s + the model's latency** | 800 + 120 + 900 + 100 + 600 + 50 ms; SC-001's release gate (p90 3.5 s) leaves ~0.9 s of model time, and its 2 s stretch target needs a model that answers in ~0 ms, which is why it is a stretch target and not a gate |

Local-only recognisers and local voices (whisper + Kokoro) are expected to miss SC-001; the
specification's success criterion is therefore measured with the agent's hosted provider, which is
recorded as open question Q3.

## Fallbacks and degradation

| Failure | Behaviour |
|---|---|
| Server without the feature or with the flag off | capability reads false, the control is not rendered, nothing else changes (FR-041) |
| `AudioWorklet` unavailable (old WebView) | fall back to a `ScriptProcessorNode` tap if present; otherwise the control is offered but reports "handsfree unavailable" and works as press-to-talk: press to open the microphone, press to send (the interrupt control keeps working) |
| Person declines the microphone | the session fails with a reason and is retryable without reloading (FR-030) |
| Browser does not honour echo cancellation | the playback guard is raised (longer ignore window, higher threshold); if self-interruption is still detected the session says so once and suggests headphones, instead of looping |
| Voice provider fails mid-reply | the rest of the reply still appears as text, the failure is reported, the session listens again (FR-032) |
| WebSocket drops | state reads reconnecting and audio stops within 2 s; the same session resumes within 30 s or ends with the reason (FR-033). Resuming re-binds the audio lane to the **new** socket handle (`TopicConnection::open` makes one per socket, `topic.rs:144`), otherwise the session would be live and mute (`review.md` D13) |
| Agent restarts, conversation deleted | session ends, in-flight reply finishes as text and is never spoken (FR-028, FR-034) |
| Slow client (frames pile up) | audio is dropped and text never is: the socket's own queue is checked before an audio frame is written (`LIVE_VOICE_SOCKET_BACKLOG = 16` frames), the drop is counted, and one `live_voice_error { kind: "stall" }` follows when the backlog clears; the session is not ended. The socket queue is `flume::unbounded()` (`channel.rs:374`), so this check is the only memory bound there — it is enforced, not assumed (`review.md` D9) |

## Risks

| Risk | Mitigation |
|---|---|
| Barge-in misfires on the device's own audio | browser AEC + raised gate during playback + SC-008's 80%-volume test as a release gate |
| Batch recognition makes the turn feel slow | measure per provider; keep the local model out of the default path for live voice |
| Segment boundaries heard as gaps | synthesise one segment ahead and schedule buffers back to back on one `AudioContext` clock |
| Frame traffic grows with the bus copy | 100 ms chunks (`~4.3 KB` base64) per direction, one session per conversation |
| A forgotten session listens on | idle stop at 3 min with a visible reason (FR-029) and a microphone indicator (FR-038) |
| Noise cancels a turn nobody has heard yet | the 250 ms onset guard above, and a higher gate while the state is `waiting`; a burst shorter than the guard only pauses the local playback and resumes it, so a misfire costs a stutter, not a cancelled turn. If it still misfires the fix is a stricter guard, not a spec change |
| Audio in the session-event ring would evict text (1024 slots shared by the whole process, `transport/mod.rs:29`) | audio frames bypass `publish_session_event` entirely while control frames keep using it (a handful per turn); the ring, its capacity and every existing consumer stay exactly as they are (`review.md` D9) |
| Desktop CSP blocks a later wasm VAD | the optional upgrade needs a CSP change and self-hosted assets; recorded in research.md so nobody adds it silently |

## Out of scope

Token-level speech (needs a streaming turn protocol); speech-to-speech realtime providers (FR-036a
leaves the seam); Discord and Telegram voice (FR-035); wake words and always-listening; storing audio;
multi-person capture; editing a sent transcript.

## Definition of Done

1. Spec, plan, research, data model, contracts, diagrams and quickstart present under `specs/055-live-voice/`.
2. `tasks.md` generated by `speckit_tasks` (@mamang-lead-be) from this plan, then implementation by `@sepuh-be` per task.
3. Verification by `@teteh-qa` with `speckit_analyze` + `speckit_checklist`.
4. Both interruption paths demonstrated with the faked provider and once by hand (quickstart).
5. No new mandatory dependency; `cargo test` green with the flag on and off.

