# Tasks: Live Voice Conversation

**Input**: Design documents from `/specs/055-live-voice/`

**Prerequisites**: `plan.md`, `spec.md`, `research.md`, `data-model.md`, `contracts/live-voice-frames.md`, `quickstart.md`

**Tests**: required by the plan (its Testing section names the crate, flag, vitest and Playwright jobs),
so test tasks are included and come before their implementation in every story.

**Organization**: phases follow the five user stories in `spec.md`; the plan's phases A–F are the source
for the technical tasks. All code is written by `@sepuh-be`; this file is the contract it works from.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: US1–US5, mapping to `spec.md`
- every task names the file it touches

## Path Conventions

Repository root `/home/muruyung/ngoding/pribadi/vizier`: Rust crates under `crates/`, the WebUI under
`webui/app`, the desktop shell under `app/`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: the flag, the module skeleton, the empty client folder — no behaviour yet.

- [ ] T001 Add the `provider-live-voice` feature to `crates/vizier-core/Cargo.toml` (`default = []`, added to `full`); confirm `crates/vizier/Cargo.toml` already reaches it through `full`, and add a comment beside the feature saying what it gates
- [ ] T002 [P] Create `crates/vizier-core/src/live_voice/mod.rs` with the `#[cfg(feature = "provider-live-voice")]` gate, the `mod session; mod engines; mod utterance; mod segmenter;` declarations and the re-exports; register the module in `crates/vizier-core/src/lib.rs`
- [ ] T003 [P] Create `webui/app/lib/liveVoice/frames.ts` with the wire types of `contracts/live-voice-frames.md` (client→server and server→client) plus pure `encode`/`decode` helpers, no side effects
- [ ] T004 [P] Create `webui/app/lib/liveVoice/__fixtures__/README.md` listing the two clips the VAD tests need (a person speaking over a playback sample, and the playback sample alone at 80% volume) and how they were produced
- [ ] T005 [P] Create the empty test files so the suites exist before the code: `crates/vizier-http/tests/live_voice_frames.rs`, `webui/app/lib/liveVoice/session.test.ts`, `webui/app/lib/liveVoice/vad.test.ts`
- [ ] T006 Record the baseline before touching anything: `cargo check -p vizier-core`, `cargo check -p vizier-core --features provider-live-voice`, `cargo check -p vizier-http`, `cd webui && npm run build` — paste the four exit codes into the pull request

**Checkpoint**: the flag exists and changes nothing; every baseline is green.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: the setting, the frame family, the capability, and the client's ability to see it. Nothing
listens or speaks yet, and no user story can start until this phase is done.

**⚠️ CRITICAL**: no user story work begins before T021.

- [ ] T007 Add `LiveVoiceSettings` (`stt: Option<…>`, `tts: Option<…>`, `idle_stop_seconds`, `max_utterance_seconds`, `end_of_utterance_ms`, all defaulted) and `live_voice: ToolConfig<LiveVoiceSettings>` in `crates/vizier-core/src/schema/agent.rs`
- [ ] T008 Serde test in `crates/vizier-core/src/schema/agent.rs`: an agent JSON **without** the block loads and reads `enabled: false`; one **with** a partial block loads with the documented defaults (FR-040)
- [ ] T009 Add `LiveVoiceFrame` and `SessionFrame::LiveVoice(LiveVoiceFrame)` in `crates/vizier-core/src/transport/mod.rs`, matching the shapes in `contracts/live-voice-frames.md`; the server frame is `#[serde(tag = "type", rename_all = "snake_case")]` (`state` / `transcript` / `speech` / `error`), per the contract's review delta C-1
- [ ] T010 Map the new variant to a top-level `live_voice` JSON object in the outbound match of `crates/vizier-http/src/topic.rs:535-600` — every variant is mapped by hand there, so this is an explicit arm, not free serialisation (`review.md` D4); the match is exhaustive, so the compiler is the check
- [ ] T011 Add `live_voice: bool` beside `tts`/`stt` in the agent detail type at `crates/vizier-core/src/service/agents.rs:69-71`; it reads true only when the flag is on **and** the agent has a usable recogniser and voice (FR-004)
- [ ] T012 [P] Read `detail.live_voice` in `webui/app/routes/chat.tsx:104-119` beside the existing `stt`/`tts` and pass it to the composer's control state
- [ ] T013 [P] Add the `live_voice` case to `classifyFrame` in `webui/app/api/frames.ts:597-630` (a new `kind: 'live_voice'`) plus a case in `webui/app/api/frames.test.ts` proving an older payload still returns `null`
- [ ] T014 Contract test in `crates/vizier-http/tests/live_voice_frames.rs`: with the flag **off** a `live_voice_start` is ignored without closing the socket and no `live_voice` frame is ever emitted (FR-041)
- [ ] T015 Guard the inbound path in `crates/vizier-http/src/topic.rs`: `live_voice_audio` larger than one chunk (~6 KB of base64) or a chunk rate above the contract's cap is refused with `live_voice_error`, not buffered
- [ ] T016 [P] Regenerate the typed API schema (`webui/app/api/schema.d.ts`) with the project's existing generator script so `live_voice: bool` is in the generated types
- [ ] T017 Add `#[serde(default)] spoken: bool` to `VizierRequest` in `crates/vizier-core/src/schema/request.rs`, with a serde test proving a stored request from before this feature loads as `false` — the one field FR-011's "marked as spoken" and FR-017's indication both need (there is none today: `grep -rn "spoken" crates/` is empty; `review.md` D2)
- [ ] T018 [P] Use `spoken` on the turn: append the "this came by speech, answer for listening" indication in the block that already preprocesses voice input, `crates/vizier-core/src/agents/agent/mod.rs:712-750`, gated on `spoken` so no typed turn changes (FR-017); render the spoken mark on the stored request in history (server side `SessionHistoryContent::Request`, client side the FR-011/Q5 badge)
- [ ] T019 Take the `live_voice_*` branch in `crates/vizier-http/src/topic.rs:209` `handle_inbound` **before** the `let text_str = frame.to_string();` copy at `:216`, so a session does not serialise its own audio twice on the way in (4.3 KB x 10/s per session); add a test that one audio chunk is dispatched without being stringified (`review.md` D5)
- [ ] T020 Carry live voice frames to **one** socket, never the session broadcast, in `crates/vizier-core/src/live_voice/registry.rs` and `.../session.rs`: the session holds the owner's `Arc<ClientConnection>` (`crates/vizier-core/src/agents/client/connection.rs`) and writes through `ClientConnection::send`; `false` from a closed connection ends the session with reason `page_closed` (FR-028), `false` from a full queue drops that one audio frame and counts it (`review.md` D9)
- [ ] T021 Bound the socket, not the broadcast, in the `client_rx` arm of `TopicOutbound::run` (`crates/vizier-http/src/topic.rs:519-525`): before writing an audio frame, check the socket queue depth (`self.out.len()`) and skip the frame above `LIVE_VOICE_SOCKET_BACKLOG = 16`, writing exactly one `live_voice_error { kind: "stall" }` when the backlog clears; `live_voice_state` / `live_voice_transcript` / `live_voice_error` are never skipped. Test in `crates/vizier-http/tests/live_voice_delivery.rs` that a stalled socket loses audio and never a `SessionFrame::Response` (contract §Delivery path, `review.md` D9)

**Checkpoint**: the capability reads true/false correctly, the frames round-trip, and a flag-off build is indistinguishable from today.

---

## Phase 3: User Story 1 — A conversation without typing (Priority: P1) 🎯 MVP

**Goal**: press the control, speak, see the message, hear the answer, ask a follow-up without pressing.

**Independent Test**: `quickstart.md` steps 1–3 with a hosted recogniser and voice: both questions and both answers land in the conversation as text.

### Tests for User Story 1

- [ ] T022 [P] [US1] Session test in `crates/vizier-http/tests/live_voice_session.rs` with a faked recogniser: `start` → three `audio` chunks → `utterance_end` produces exactly one request whose text is the transcript
- [ ] T023 [P] [US1] Segmenter tests in `crates/vizier-core/src/live_voice/segmenter.rs`: splits at sentence and clause boundaries, never inside a word, skips fenced code, tables and raw URLs with one notice (FR-014)
- [ ] T024 [P] [US1] Client state machine test in `webui/app/lib/liveVoice/session.test.ts`: `off → connecting → listening → waiting → speaking → listening`, and `start` is never sent twice

### Implementation for User Story 1

- [ ] T025 [US1] `crates/vizier-core/src/live_voice/session.rs` plus `crates/vizier-core/src/live_voice/registry.rs`: one `Arc<LiveVoiceSessions>` per process, held beside `Arc<vizier_core::questions::Questions>` and handed to `TopicConnection` the same way (`crates/vizier-http/src/topic.rs:79-81`) — per-socket state cannot enforce FR-005 across two tabs (`review.md` D6); key (agent, conversation), the FR-006 states, owner principal, `tokio::sync::watch<u64>` generation, `last_speech_at` and the idle timer
- [ ] T026 [US1] Accept `live_voice_start`, `live_voice_audio`, `live_voice_utterance_end` and `live_voice_stop` in `crates/vizier-http/src/topic.rs`, authorised exactly like a chat message on the same socket (FR-005)
- [ ] T027 [US1] `crates/vizier-core/src/live_voice/utterance.rs`: accumulate PCM16, cap at `max_utterance_seconds`, write the 44-byte WAV header **by hand** (not `hound`, which lives behind `provider-media` and would couple the new flag to it — `review.md` D7), decide `too_short`
- [ ] T028 [US1] Submit the transcript with `transport.send_request(session, req, Some(response_tx))` (`crates/vizier-core/src/transport/mod.rs:467`) from `crates/vizier-core/src/live_voice/session.rs`, carrying its own `ResponseSender` so the reply read is provably this turn's (FR-007), with the `spoken` field from T017 (FR-011, FR-017)
- [ ] T029 [US1] `crates/vizier-core/src/live_voice/engines.rs`: the single `LiveVoiceEngines` trait plus its cascade implementation over `VizierStt`/`VizierTts` (FR-036, FR-036a)
- [ ] T030 [US1] `crates/vizier-core/src/live_voice/segmenter.rs`: reply text → speakable segments, first boundary preferred after ~80 characters (FR-013)
- [ ] T031 [US1] Emit `live_voice_speech` frames from `crates/vizier-core/src/live_voice/session.rs` with `generation`, `seq`, `final`, converting the provider's audio to PCM16 16 kHz mono, synthesising segment *n+1* while *n* is sent
- [ ] T032 [US1] `webui/app/lib/liveVoice/capture.ts`: `getUserMedia` with `echoCancellation`, `noiseSuppression`, `autoGainControl`; reuse `/vizier-capture-worklet.js`; downmix and resample to 16 kHz mono; 100 ms chunks (C5)
- [ ] T033 [US1] `webui/app/lib/liveVoice/vad.ts`: energy gate over a 20 ms window, adaptive noise floor, 800 ms end-of-utterance hangover (FR-009, FR-010)
- [ ] T034 [US1] `webui/app/lib/liveVoice/session.ts`: the client loop — start/stop, chunked send, frame handling, state transitions, `waiting` while the turn runs
- [ ] T035 [US1] `webui/app/lib/liveVoice/player.ts`: one `AudioContext`, an ordered queue of `AudioBufferSourceNode`s scheduled back to back, `stop()` that closes every source and drains the queue (FR-022 groundwork)
- [ ] T036 [US1] `webui/app/screens/chat/LiveVoiceButton.tsx`: the single off/on control, separate from the mic and the spoken-reply toggle, rendered only when the capability is true, off on every visit (FR-001, FR-002, FR-003)
- [ ] T037 [US1] `webui/app/screens/chat/LiveVoiceBar.tsx`: the FR-006 state word, the input level, the microphone-open indicator including an unfocused window (FR-038)
- [ ] T038 [US1] Mount both controls from `webui/app/screens/chat/Composer.tsx` next to the existing mic and spoken-reply controls, without changing either
- [ ] T039 [US1] Expose the per-agent `live_voice` switch in `webui/app/lib/agentForm.ts` and its field in `webui/app/screens/agents/ModelsFields.tsx`, defaulting off
- [ ] T040 [US1] Keep the session's own accounting honest: surface `backlog`/drop counters as one `live_voice_error { kind: "stall" }` per episode from `crates/vizier-core/src/live_voice/session.rs`, matching the bound T021 enforces at the socket (no second, independent queue)

**Checkpoint**: US1 works on its own and `quickstart.md` steps 1–3 pass.

---

## Phase 4: User Story 2 — Stopping the agent by speaking (Priority: P1)

**Goal**: the person talks over the agent and it stops, before their words are transcribed.

**Independent Test**: a long answer, spoken over mid-sentence, silences the audio; the interrupting words become a message; the interrupted reply is marked interrupted.

- [ ] T041 [P] [US2] VAD tests in `webui/app/lib/liveVoice/vad.test.ts` with the T004 fixtures: speech over playback triggers, playback alone at 80% volume does not (FR-026, SC-008)
- [ ] T042 [P] [US2] Server test in `crates/vizier-http/tests/live_voice_barge_in.rs`: `live_voice_speech_start` bumps the generation, the running turn takes the existing abort path, a synthesised segment from the old generation is dropped
- [ ] T043 [US2] Client barge-in in `webui/app/lib/liveVoice/session.ts`: on speech start while the reply is audible call `player.stop()` (<= 50 ms) and send `live_voice_speech_start` (FR-019)
- [ ] T044 [US2] Server barge-in in `crates/vizier-core/src/live_voice/session.rs`: bump the generation, abandon synthesis, then cancel the turn with `transport.send_agent_command(AgentCommand::Abort { .. })` (`crates/vizier-core/src/agents_client.rs:415`) — audio first, abort second; mark the reply interrupted **only when its `Message` had already arrived**, because an abort before any text is an `Abort` turn that produced nothing (`crates/vizier-core/src/schema/response.rs:156-166`) and the UI then shows the existing "Stopped by you" instead of an invented partial (`review.md` D1)
- [ ] T045 [US2] Playback guard and onset guard in `webui/app/lib/liveVoice/vad.ts`: raised gate while the reply is audible, the first 150 ms after each segment ignored, and ~250 ms of voiced energy before a barge-in signal is sent
- [ ] T046 [US2] Too-short burst in `webui/app/lib/liveVoice/session.ts` and `crates/vizier-core/src/live_voice/utterance.rs`: the audio stops, nothing is submitted, the session listens again (FR-024)
- [ ] T047 [US2] End-to-end check in `webui/e2e/live-voice-barge-in.spec.ts` with a faked provider: audio stops, the correction is submitted, the interrupted reply keeps its mark
- [ ] T048 [US2] Confirm stale-generation frames are dropped by the client on arrival in `webui/app/lib/liveVoice/player.ts`, with a unit test in `webui/app/lib/liveVoice/session.test.ts`

**Checkpoint**: US1 and US2 both work; SC-002 and SC-008 are measurable.

---

## Phase 5: User Story 3 — Stopping the agent with a control (Priority: P1)

**Goal**: one button stops the voice at any moment and never ends the session.

**Independent Test**: with the agent speaking, press interrupt; the audio stops, the session stays live, a typed correction is answered and spoken.

- [ ] T049 [P] [US3] Server test in `crates/vizier-http/tests/live_voice_interrupt.rs`: the interrupt frame bumps the generation, submits nothing, and leaves the session live (FR-020, FR-021)
- [ ] T050 [US3] Add the always-visible interrupt control to `webui/app/screens/chat/LiveVoiceBar.tsx` (present for the whole session, distinct from the on/off control)
- [ ] T051 [US3] Implement the interrupt path in `webui/app/lib/liveVoice/session.ts` and `player.ts`: `player.stop()`, send `live_voice_interrupt` with `reason: "button"`, move `interrupted → listening` (SC-003)
- [ ] T052 [US3] Typing during playback stops the audio in `webui/app/screens/chat/Composer.tsx`'s send path, sending `reason: "typed"` (FR-025)
- [ ] T053 [US3] Interrupt while nothing is being spoken is a no-op on state in `crates/vizier-core/src/live_voice/session.rs` (FR-021)
- [ ] T054 [US3] End-to-end check in `webui/e2e/live-voice-interrupt.spec.ts`: one press silences, the session stays live, a typed message is answered and spoken
- [ ] T055 [US3] In `webui/app/screens/chat/LiveVoiceBar.tsx`, show the interrupt control disabled with a reason only when the capability is false; never hide it inside a live session (FR-020)

**Checkpoint**: both interruption paths usable; US3 does not depend on US2.

---

## Phase 6: User Story 4 — A session never keeps the microphone (Priority: P2)

**Goal**: every ending releases the microphone and leaves the text.

**Independent Test**: close the tab mid-answer; the OS microphone indicator goes out within 2 s and the reply text is kept; a silent session ends after the idle period with a reason.

- [ ] T056 [P] [US4] Tests in `webui/app/lib/liveVoice/session.test.ts` and `crates/vizier-http/tests/live_voice_session.rs`: page close releases tracks, idle stop ends with a reason, a second start on the same conversation is refused
- [ ] T057 [US4] One release path in `webui/app/lib/liveVoice/session.ts` — stop every track, close the `AudioContext`, drop the queue — called by every terminal transition
- [ ] T058 [US4] Wire `pagehide`/unmount handling in `webui/app/routes/chat.tsx` and `webui/app/screens/chat/Composer.tsx` so leaving the conversation ends the session (FR-028)
- [ ] T059 [US4] Refuse a second session on the same conversation in `crates/vizier-core/src/live_voice/session.rs` with `live_voice_error` reason `session_active`, and show it in `webui/app/screens/chat/LiveVoiceBar.tsx` (FR-005)
- [ ] T060 [US4] Idle stop and owner-gone ends in `crates/vizier-core/src/live_voice/session.rs`: idle timer (FR-029), agent reconfigured/deleted, conversation deleted, server restart
- [ ] T061 [US4] Ending leaves the control off and remembers nothing between visits in `webui/app/screens/chat/LiveVoiceButton.tsx` (no storage)
- [ ] T062 [US4] End-to-end check in `webui/e2e/live-voice-lifecycle.spec.ts`: close mid-answer, track `readyState` becomes `ended`, text kept, control off on reload
- [ ] T063 [US4] Mobile/desktop suspension (screen lock, WebView background) ends the session with a reason rather than staying "live" in `webui/app/lib/liveVoice/session.ts`

**Checkpoint**: SC-007 holds; no code path can leave listening.

---

## Phase 7: User Story 5 — A broken session explains itself (Priority: P2)

**Goal**: every failure is a visible reason within 5 s, never a session that looks live.

**Independent Test**: cut the connection mid-answer; the state reads reconnecting within 2 s and the audio stops; restore within 30 s and the same session continues without a duplicate message.

- [ ] T064 [P] [US5] Tests in `crates/vizier-http/tests/live_voice_failures.rs`: empty recognition, voice provider failing mid-reply, socket drop then reconnect inside the window, no device (FR-030–FR-033)
- [ ] T065 [US5] Reconnect in `webui/app/lib/liveVoice/session.ts` and `crates/vizier-core/src/live_voice/session.rs`: resume the same session within 30 s, never re-submit a message already in flight
- [ ] T066 [US5] Failure surfaces in `webui/app/screens/chat/LiveVoiceBar.tsx`: one reason line within 5 s and the microphone released (SC-009)
- [ ] T067 [US5] Voice provider failure mid-reply in `crates/vizier-core/src/live_voice/engines.rs`: the rest of the reply still appears as text, the failure is reported, the session listens again (FR-032)
- [ ] T068 [US5] No device and refused permission in `webui/app/lib/liveVoice/capture.ts`: fail with a reason, retryable without a reload (FR-030)
- [ ] T069 [US5] End-to-end check in `webui/e2e/live-voice-failures.spec.ts` using route interception to drop and restore the socket
- [ ] T070 [US5] Unintelligible transcript in `crates/vizier-core/src/live_voice/utterance.rs`: tell the person, send nothing, listen again (FR-031)

**Checkpoint**: every failure path in SC-009 is demonstrated.

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: the documentation, the measurement, and the gates the constitution asks for.

- [ ] T071 [P] Write the user documentation page `docs/src/configuration/live-voice.md` (start/end, what audio goes to which provider, what is stored, both interruptions, the limits) and add it to `docs/src/SUMMARY.md` (FR-039)
- [ ] T072 [P] Add the build-flavour note for `provider-live-voice` beside the other optional features in `docs/src/deployment/` (whichever page lists the `full` build)
- [ ] T073 Instrument the stages in `crates/vizier-core/src/live_voice/session.rs` (end of utterance, recognition, turn start, first segment, first frame) with `tracing` spans and record p50/p90/p99 in `specs/055-live-voice/latency.md`
- [ ] T074 Run the flag matrix and record it: `cargo test -p vizier-core`, `cargo test -p vizier-core --features provider-live-voice`, `cargo test -p vizier-http`, `cd webui && npm test`, `cd webui && npm run e2e`
- [ ] T075 Run `cargo clippy --all-targets` for both flag states and `cd webui && npm run lint`; fix everything new
- [ ] T076 Walk `quickstart.md` by hand with a hosted provider and record the result of each of its nine steps in the pull request
- [ ] T077 Run the `dummyplug` agent-observable e2e gate from the constitution against a running binary (message in, history out) with the speech providers faked
- [ ] T078 Remove dead code and leftover TODOs from the new modules (`crates/vizier-core/src/live_voice/*`, `webui/app/lib/liveVoice/*`) before the review gate
- [ ] T079 Hand the branch to `@teteh-qa` for `speckit-analyze` and `speckit-checklist`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies
- **Foundational (Phase 2)**: needs Setup; **blocks every story**
- **US1 (Phase 3)**: needs Foundational
- **US2 (Phase 4)**: needs Foundational; its barge-in is only observable once US1 can speak, so in practice after US1
- **US3 (Phase 5)**: needs Foundational and US1's player; independent of US2
- **US4 (Phase 6)**: needs US1 (a session to release)
- **US5 (Phase 7)**: needs US1 and US4 (an ending to report)
- **Polish (Phase 8)**: after the stories that are in scope

### User Story Dependencies

- **US1** — no story dependency. MVP.
- **US2** — depends on US1's playback path; the interruption logic itself is independent of US3.
- **US3** — depends on US1's playback path; independent of US2 (each proves a different interruption).
- **US4** — depends on US1.
- **US5** — depends on US1 and US4.

### Within Each User Story

Tests first and failing, then the server side, then the client side, then the end-to-end check.

### Parallel Opportunities

- T002–T005 (module skeleton, frame types, fixtures, empty tests) run together.
- T012 and T013 (client capability read, client frame classification) run together after T011.
- In every story, the `[P]` test tasks run together before the implementation tasks.
- US2 and US3 can be built by two people once US1 lands.

---

## Parallel Example: User Story 1

```bash
# the three test suites first
Task: "crates/vizier-http/tests/live_voice_session.rs — one utterance, one request"   # T022
Task: "crates/vizier-core/src/live_voice/segmenter.rs — segment boundaries"            # T023
Task: "webui/app/lib/liveVoice/session.test.ts — state machine"                        # T024

# then the server modules and the client modules side by side
Task: "crates/vizier-core/src/live_voice/{session,utterance,engines,segmenter}.rs"     # T025–T031
Task: "webui/app/lib/liveVoice/{capture,vad,session,player}.ts"                        # T032–T035
```

## Implementation Strategy

### MVP first (User Story 1 only)

1. Phase 1 and Phase 2 to completion (the flag, the frames, the capability).
2. Phase 3 to completion.
3. Stop and validate with `quickstart.md` steps 1–3, then demo.

### Incremental delivery

US1 → demo (talk and hear) → US2 (barge-in) → US3 (interrupt control) → US4 (microphone discipline) →
US5 (failures) → Polish. Every story leaves the previous one working, and a flag-off build stays
untouched throughout.

## Notes

- [P] means different files and no dependency on an unfinished task.
- Commit in conventional format; `just commit "<msg>"` commits submodules and workspace together.
- No new mandatory dependency is allowed by this plan (research §C). If a task seems to need one, stop
  and raise it with `@mamang-lead-be` instead of adding it.
- Do not implement a token-level speech path, a WebRTC transport, or speech-to-speech providers: they
  are out of scope in `plan.md`.

---

## Delta of this review (2026-11-08, `@mamang-lead-be`)

This file was drafted on this branch earlier the same day (in the working tree at 15:15, committed as
`6357b0c5 docs: add tasks`) and is **verified and amended here**, not rewritten: all 74 original tasks
were checked against the code and kept, three were added, seven were made more specific, and every ID
from the old `T017` upward moved by three to keep the sequence in execution order.

**Why this delta exists**: `review.md` re-checked every citation in `research.md`/`plan.md` against the
tree. Six findings change what a task must say; `architecture.md` is the shape they take.

| Finding | Task touched | What changed |
|---|---|---|
| **D1** — an abort before any reply text is an `Abort` turn that produced nothing (`schema/response.rs:156-166`), so "readable as far as it was produced" has no text to read | T044 | interruption marks the reply only when its `Message` had already arrived; otherwise "Stopped by you", no invented partial |
| **D2** — nothing server-side records that a message came by speech (`grep -rn "spoken" crates/` is empty) | **new T017, T018**; T028 | `#[serde(default)] spoken: bool` + the FR-017 note in `agents/agent/mod.rs:712-750`; the submission task now names the field |
| **D3** — the frame contract had no discriminator inside `live_voice` (contract delta C-1) | T009 | `#[serde(tag = "type", rename_all = "snake_case")]` |
| **D4** — outbound frames are a hand-written `match` (`topic.rs:535-600`), not free serialisation | T010 | the task names the match and its line range |
| **D5** — `handle_inbound` stringifies every inbound frame before dispatch (`topic.rs:216`) | **new T019** | the `live_voice_*` branch must be taken before that copy |
| **D6** — the registry cannot live on `TopicConnection` (per socket) and still refuse a second tab | T025 | one process-wide registry handed over like `Arc<Questions>` (`topic.rs:79-81`) |
| **D7** — `hound` sits behind `provider-media`; using it would couple the new flag | T027 | 44-byte WAV header by hand |

**Also recorded, no task changed**: the author's claim "no new mandatory dependency" holds
(`Cargo.lock`: `base64 0.23.1`, `tokio` `sync` feature, `serde`, `AudioContext`/`AudioWorklet` already
shipped); `tokio` is `1.53.3`, not the `1.49.0` written in `research.md` §C; `classifyFrame` lives at
`webui/app/api/frames.ts:606-630`, not `:619-621` (already corrected in the analyst's `e5ac773a`).

**Execution order is unchanged**: Phase 1 → Phase 2 (now 13 tasks, ending at T019) → US1..US5 →
Polish. `@sepuh-be` implements; `@teteh-qa` verifies with `speckit-analyze` + `speckit-checklist`
after T079.
