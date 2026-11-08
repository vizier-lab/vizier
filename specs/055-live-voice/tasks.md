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

**⚠️ CRITICAL**: no user story work begins before T022.

**Deliberately untouched by this phase**: `crates/vizier-http/src/api/v1/agents/channel.rs` and
`crates/vizier-http/src/embedded.rs`. No new queue, no `TopicConnection::open` signature change and no
second call site: the drop point is T021's gate inside `TopicOutbound::run`, which already sees the
socket's real depth (`self.out.len()`, `flume`'s `Sender::len`). The rejected alternative and its
reasons are in `architecture.md` §11 — do not build both.

- [ ] T007 Add `LiveVoiceSettings` (`stt: Option<…>`, `tts: Option<…>`, `idle_stop_seconds`, `max_utterance_seconds`, `end_of_utterance_ms`, all defaulted) and `live_voice: ToolConfig<LiveVoiceSettings>` in `crates/vizier-core/src/schema/agent.rs`
- [ ] T008 Serde test in `crates/vizier-core/src/schema/agent.rs`: an agent JSON **without** the block loads and reads `enabled: false`; one **with** a partial block loads with the documented defaults (FR-040)
- [ ] T009 Add the wire types in `crates/vizier-core/src/live_voice/frame.rs`, `#[serde(tag = "type", rename_all = "snake_case")]` (`state` / `transcript` / `speech` / `error` / `stall`) per contract delta C-1, **split in two so the compiler holds the lane**: `LiveVoiceControl` (`state`, `transcript`, `error`, `stall`) and `LiveVoiceSpeech`. `SessionFrame::LiveVoice(LiveVoiceControl)` in `crates/vizier-core/src/transport/mod.rs` therefore *cannot* carry audio — publishing a speech frame as a session event is a type error, not a review rule (`review.md` D9), and the control-only publish helper of T020 needs no runtime check
- [ ] T010 Map `SessionFrame::LiveVoice` to a top-level `live_voice` object in the outbound match of `crates/vizier-http/src/topic.rs:535-600` — every variant is mapped by hand there, so this is an explicit arm, not free serialisation (`review.md` D4); the match is exhaustive, so the compiler is the check. This arm is reached through `events.recv()`, the broadcast, and carries **control only** (`state`, `transcript`, `error`, `stall`): `live_voice_speech` **never** passes here, it is written per connection by T020 (contract §Delivery path), and T009's type split makes sending it here a compile error rather than a review rule. Write the arm as `serde_json::to_value(&control)` wrapped in the `live_voice` key — the same serialiser the speech lane uses — not a hand-built `json!(...)`, so the two lanes cannot drift in field names (`analysis.md` M15). Test in `crates/vizier-http/tests/live_voice_delivery.rs`: a control frame published as a session event reaches every socket on that conversation, and no `live_voice_speech` frame is ever observable on `session_events`
- [ ] T011 Add `live_voice: bool` beside `tts`/`stt` in the agent detail type at `crates/vizier-core/src/service/agents.rs:69-71`; it reads true only when the flag is on **and** the agent has a usable recogniser and voice (FR-004)
- [ ] T012 [P] Read `detail.live_voice` in `webui/app/routes/chat.tsx:104-119` beside the existing `stt`/`tts` and pass it to the composer's control state
- [ ] T013 [P] Add the `live_voice` case to `classifyFrame` in `webui/app/api/frames.ts:597-630` (a new `kind: 'live_voice'`) plus a case in `webui/app/api/frames.test.ts` proving an older payload still returns `null`
- [ ] T014 Contract test in `crates/vizier-http/tests/live_voice_frames.rs`: with the flag **off** a `live_voice_start` is ignored without closing the socket and no `live_voice` frame is ever emitted (FR-041)
- [ ] T015 Guard the inbound path in `crates/vizier-http/src/topic.rs`: `live_voice_audio` larger than one chunk (~6 KB of base64) or a chunk rate above the contract's cap is refused with `live_voice_error`, not buffered
- [ ] T016 [P] Regenerate the typed API schema (`webui/app/api/schema.d.ts`) with the project's existing generator script so `live_voice: bool` is in the generated types
- [ ] T017 Add `#[serde(default)] spoken: bool` to `VizierRequest` in `crates/vizier-core/src/schema/request.rs`, with a serde test proving a stored request from before this feature loads as `false` — the one field FR-011's "marked as spoken" and FR-017's indication both need (there is none today: `grep -rn "spoken" crates/` is empty; `review.md` D2)
- [ ] T018 [P] Use `spoken` on the turn: append the "this came by speech, answer for listening" indication in the block that already preprocesses voice input, `crates/vizier-core/src/agents/agent/mod.rs:712-750`, gated on `spoken` so no typed turn changes (FR-017); render the spoken mark on the stored request in history (server side `SessionHistoryContent::Request`, client side the FR-011/Q5 badge)
- [ ] T019 Take the `live_voice_*` branch in `crates/vizier-http/src/topic.rs:209` `handle_inbound` **before** the `let text_str = frame.to_string();` copy at `:216`, so a session does not serialise its own audio twice on the way in (4.3 KB x 10/s per session); add a test that one audio chunk is dispatched without being stringified (`review.md` D5)
- [ ] T020 Carry live voice frames to **one** socket, never the session broadcast, in `crates/vizier-core/src/live_voice/registry.rs` and `.../session.rs`: the session holds the owner's `Arc<ClientConnection>` (`crates/vizier-core/src/agents/client/connection.rs`) and writes through `ClientConnection::send`. **`false` is two events, not one** (`try_send` returns `Full` and `Closed` alike; the queue is 64, `crates/vizier-http/src/topic.rs:141`): end the session with reason `page_closed` only when `!connection.is_open()` (FR-028); while the connection is open a `false` is one dropped audio frame, counted. Test both cases in `crates/vizier-http/tests/live_voice_delivery.rs` (`review.md` D9, D10)
- [ ] T021 Bound the socket, not the broadcast, in the `client_rx` arm of `TopicOutbound::run` (`crates/vizier-http/src/topic.rs:519-525`): before writing an audio frame, check the socket queue depth (`self.out.len()`) and skip the frame above `LIVE_VOICE_SOCKET_BACKLOG = 16`, emitting exactly one `stall` control frame (`{ "type": "stall", "dropped": N }`, contract §`live_voice` `stall`) when the backlog clears; `live_voice_state` / `live_voice_transcript` / `live_voice_error` are never skipped. Test in `crates/vizier-http/tests/live_voice_delivery.rs` that a stalled socket loses audio and never a `SessionFrame::Response` (contract §Delivery path, `review.md` D9)
- [ ] T022 Say what the `Lagged` arm means now, in `crates/vizier-http/src/topic.rs:609-613`: it drops **control and text** frames only, because `live_voice_speech` never enters this ring (T020). Keep the arm's behaviour (a lagging subscriber cannot be repaired mid-burst) and make the rule explicit in the code comment and in a test in `crates/vizier-http/tests/live_voice_delivery.rs`: push 2 000 audio frames' worth of speech through the per-connection lane while a `SessionFrame::Response` is pending on the broadcast, and assert the `Response` still arrives **and** the subscriber is never `Lagged`. This is the arm that must never be the reason a reply disappears (`review.md` D9)

**Checkpoint**: the capability reads true/false correctly, the frames round-trip, and a flag-off build is indistinguishable from today.

---

## Phase 3: User Story 1 — A conversation without typing (Priority: P1) 🎯 MVP

**Goal**: press the control, speak, see the message, hear the answer, ask a follow-up without pressing.

**Independent Test**: `quickstart.md` steps 1–3 with a hosted recogniser and voice: both questions and both answers land in the conversation as text.

### Tests for User Story 1

- [ ] T023 [P] [US1] Session test in `crates/vizier-http/tests/live_voice_session.rs` with a faked recogniser: `start` → three `audio` chunks → `utterance_end` produces exactly one request whose text is the transcript
- [ ] T024 [P] [US1] Segmenter tests in `crates/vizier-core/src/live_voice/segmenter.rs`: splits at sentence and clause boundaries, never inside a word, skips fenced code, tables and raw URLs with one notice (FR-014)
- [ ] T025 [P] [US1] Client state machine test in `webui/app/lib/liveVoice/session.test.ts`: `off → connecting → listening → waiting → speaking → listening`, and `start` is never sent twice

### Implementation for User Story 1

- [ ] T026 [US1] `crates/vizier-core/src/live_voice/session.rs` plus `crates/vizier-core/src/live_voice/registry.rs`: one `Arc<LiveVoiceSessions>` per process, held beside `Arc<vizier_core::questions::Questions>` and handed to `TopicConnection` the same way (`crates/vizier-http/src/topic.rs:79-81`) — per-socket state cannot enforce FR-005 across two tabs (`review.md` D6); key (agent, conversation), the FR-006 states, owner principal, `tokio::sync::watch<u64>` generation, `last_speech_at` and the idle timer
- [ ] T027 [US1] Accept `live_voice_start`, `live_voice_audio`, `live_voice_utterance_end` and `live_voice_stop` in `crates/vizier-http/src/topic.rs`, authorised exactly like a chat message on the same socket (FR-005)
- [ ] T028 [US1] `crates/vizier-core/src/live_voice/utterance.rs`: accumulate PCM16, cap at `max_utterance_seconds`, write the 44-byte WAV header **by hand** (not `hound`, which lives behind `provider-media` and would couple the new flag to it — `review.md` D7), decide `too_short`
- [ ] T029 [US1] Submit the transcript with `transport.send_request(session, req, Some(response_tx))` (`crates/vizier-core/src/transport/mod.rs:467`) from `crates/vizier-core/src/live_voice/session.rs`, carrying its own `ResponseSender` so the reply read is provably this turn's (FR-007), with the `spoken` field from T017 (FR-011, FR-017)
- [ ] T030 [US1] `crates/vizier-core/src/live_voice/engines.rs`: the single `LiveVoiceEngines` trait plus its cascade implementation over `VizierStt`/`VizierTts` (FR-036, FR-036a). The recognition call passes the conversation's configured language to `VizierStt::transcribe(audio, filename, language)` — the same `stt.settings.language` the auto-transcription path uses (`crates/vizier-core/src/agents/agent/mod.rs:721`); test a non-default language reaching the call (FR-037, `review.md` D14)
- [ ] T031 [US1] `crates/vizier-core/src/live_voice/segmenter.rs`: reply text → speakable segments, first boundary preferred after ~80 characters (FR-013)
- [ ] T032 [US1] Emit `live_voice_speech` frames from `crates/vizier-core/src/live_voice/session.rs` with `generation`, `seq`, `final`, converting the provider's audio to PCM16 16 kHz mono, synthesising segment *n+1* while *n* is sent
- [ ] T033 [US1] `webui/app/lib/liveVoice/capture.ts`: `getUserMedia` with `echoCancellation`, `noiseSuppression`, `autoGainControl`; reuse `/vizier-capture-worklet.js`; downmix and resample to 16 kHz mono; 100 ms chunks (C5)
- [ ] T034 [US1] `webui/app/lib/liveVoice/vad.ts`: energy gate over a 20 ms window, adaptive noise floor, 800 ms end-of-utterance hangover (FR-009, FR-010)
- [ ] T035 [US1] `webui/app/lib/liveVoice/session.ts`: the client loop — start/stop, chunked send, frame handling, state transitions, `waiting` while the turn runs
- [ ] T036 [US1] `webui/app/lib/liveVoice/player.ts`: one `AudioContext`, an ordered queue of `AudioBufferSourceNode`s scheduled back to back, `stop()` that closes every source and drains the queue (FR-022 groundwork)
- [ ] T037 [US1] `webui/app/screens/chat/LiveVoiceButton.tsx`: the single entry control, separate from the mic and the spoken-reply toggle, rendered only when the capability is true, off on every visit, and showing that pressing it **opens a call** (FR-001, FR-002). It is the only thing the conversation gains (C6)
- [ ] T038 [US1] `webui/app/screens/livevoice/LiveVoiceCall.tsx`: **the call screen** — in place of the conversation view, showing the agent, the FR-006 state word, the FR-042 thinking indicator, the microphone-open indicator including an unfocused window (FR-038), and the end control of FR-045 (FR-043, FR-044). The archived bar amendment (`20d97316`) is not a specification for this screen: its pixel floor, narrowing order and placement do **not** carry over | **C6**
- [ ] T039 [US1] Test the FR-042 indicator on the call screen in `webui/app/lib/liveVoice/session.test.ts` and `webui/e2e/live-voice-thinking.spec.ts`: a turn that produces no audio for 4 s shows the thinking state within 100 ms of the request being accepted, its wording differs from `reconnecting`, and it clears at the first segment / `interrupted` / `ended` / `failed` (FR-042)
- [ ] T040 [US1] Mount the entry control from `webui/app/screens/chat/Composer.tsx` next to the existing mic and spoken-reply controls, without changing either, and navigate to the call screen on press; returning from the screen shows the same conversation, not a reload (FR-043, FR-046)
- [ ] T041 [US1] Expose the per-agent `live_voice` switch in `webui/app/lib/agentForm.ts` and its field in `webui/app/screens/agents/ModelsFields.tsx`, defaulting off
- [ ] T042 [US1] Keep exactly **one** counter: `backlog` is incremented and reset by T021's gate in `crates/vizier-http/src/topic.rs` and reported there as one `stall` control frame per episode (via `publish_session_event`), never by the session. The session's `ClientConnection::send` returning `false` while the connection is open is a **backstop** — `tracing::warn!` with a metric, no user-facing frame — and a test in `crates/vizier-http/tests/live_voice_delivery.rs` asserts the 64-slot queue never fills when the outbound loop runs (a healthy pair cannot produce a `Full`)

**Checkpoint**: US1 works on its own and `quickstart.md` steps 1–3 pass.

---

## Phase 4: User Story 2 — Stopping the agent by speaking (Priority: P1)

**Goal**: the person talks over the agent and it stops, before their words are transcribed.

**Independent Test**: a long answer, spoken over mid-sentence, silences the audio; the interrupting words become a message; the interrupted reply is marked interrupted.

- [ ] T043 [P] [US2] VAD tests in `webui/app/lib/liveVoice/vad.test.ts` with the T004 fixtures: speech over playback triggers, playback alone at 80% volume does not (FR-026, SC-008)
- [ ] T044 [P] [US2] Server test in `crates/vizier-http/tests/live_voice_barge_in.rs`, with the trigger C8 asks for: `live_voice_utterance_end` (not `speech_start`) bumps the generation, the running turn takes the existing abort path, a synthesised segment from the old generation is dropped, and an interruption that arrives while the reply was audible still lands — the test sends audio while a `live_voice_speech` frame is in flight
- [ ] T045 [US2] Client barge-in in `webui/app/lib/liveVoice/session.ts`: the reply **keeps playing while the person speaks** (C8); when the VAD closes the utterance, call `player.stop()` and send `live_voice_utterance_end` in the same tick, so audio stops at the end of their speech and not before it (FR-019). `live_voice_speech_start` stays what it is — the signal that opens an utterance — and no longer interrupts anything
- [ ] T046 [US2] Server barge-in in `crates/vizier-core/src/live_voice/session.rs`: bump the generation, abandon synthesis, then cancel the turn through **`AgentsClient::abort(agent_id, session)`** (`crates/vizier-core/src/agents_client.rs:205-207`, impl `:298-312`) — the same `VizierRequestContent::Command("abort")` the Composer's `/abort` already sends. There is **no** `AgentCommand::Abort` (`crates/vizier-core/src/schema/commands.rs:142` lists `Create`/`Update`/`Delete`/`HealthCheck`/`AgentHealth`/`RefreshIntegrations`/`IntegrationStatus`/`Restart`); `agents_client.rs:415` is `FakeCall::Abort`, a test fake (`review.md` D12, raised by `@teteh-qa` H1) — audio first, abort second; mark the reply interrupted **only when its `Message` had already arrived**, because an abort before any text is an `Abort` turn that produced nothing (`crates/vizier-core/src/schema/response.rs:156-166`) and the UI then shows the existing "Stopped by you" instead of an invented partial (`review.md` D1)
- [ ] T047 [US2] Playback guard and reply ducking in `webui/app/lib/liveVoice/vad.ts` + `session.ts`: a raised gate while the reply is audible and the first 150 ms after each segment ignored, so the session does not hear itself (FR-026); and, because the agent now talks **through** the person's speech (C8), the reply's contribution to the microphone must be reduced once the person is detected speaking — the mechanism is `getUserMedia` echo cancellation plus, where that is not enough, lowering the playback gain rather than the audible level the person hears (FR-047). **The ~250 ms onset guard no longer applies**: with the trigger at the end of the utterance it has nothing to gate, and it is recorded here as replaced rather than merely unused (`review.md` D15, superseded by C8)
- [ ] T048 [US2] Too-short burst in `webui/app/lib/liveVoice/session.ts` and `crates/vizier-core/src/live_voice/utterance.rs`: under C8 the old rule would cancel a reply for a cough, so the precedence is fixed the other way here and must be testable. **A sub-word burst neither cancels the reply nor becomes a message** (FR-024, Story 2 scenario 6): the length that decides is the one that makes a barge-in, measured as voiced energy, never a character count; a transcript that is empty after recognition is `unintelligible` (FR-031), not `too_short` (`analysis.md` M11, re-derived for C8)
- [ ] T049 [US2] End-to-end check in `webui/e2e/live-voice-barge-in.spec.ts` with a faked provider: audio stops, the correction is submitted, the interrupted reply keeps its mark
- [ ] T050 [US2] Confirm stale-generation frames are dropped by the client on arrival in `webui/app/lib/liveVoice/player.ts`, with a unit test in `webui/app/lib/liveVoice/session.test.ts`

**Checkpoint**: US1 and US2 both work; SC-002 and SC-008 are measurable.

---

## Phase 5: User Story 3 — Stopping the agent with a control — **[BACKLOG, deferred 2026-11-08]**

**Goal**: one button stops the voice at any moment and never ends the session.

> **Not in this release (User decision 2026-11-08 21:19, spec C7).** T051–T057 are kept, marked
> `[BACKLOG]`, and are **not** part of any checkpoint, gate or definition of done for this feature.
> Do not implement them, do not test them, and do not let US1/US2 depend on them. The reason they are
> kept rather than deleted is that the requirement is real and will come back: without a control,
> a person in a noisy room or with a quiet voice cannot stop a reply at all.

**Independent Test**: with the agent speaking, press interrupt; the audio stops, the session stays live, and a typed correction is answered **as text** with no audio scheduled for it.

- [ ] **T051 [BACKLOG]** [P] [US3] Server test in `crates/vizier-http/tests/live_voice_interrupt.rs`: the interrupt frame bumps the generation, submits nothing, and leaves the session live (FR-020, FR-021)
- [ ] **T052 [BACKLOG]** [US3] Add an always-visible interrupt control to the **call screen** (`webui/app/screens/livevoice/LiveVoiceCall.tsx`, FR-044's surface), distinct from the end control of FR-045. **When this backlog item lands, the control belongs on the call screen; the composer bar it was written for is archived (C6)**
- [ ] **T053 [BACKLOG]** [US3] Implement the interrupt path in `webui/app/lib/liveVoice/session.ts` and `player.ts`: `player.stop()`, send `live_voice_interrupt` with `reason: "button"`, move `interrupted → listening` (SC-003)
- [ ] **T054 [BACKLOG]** [US3] Typing during a session in `webui/app/screens/chat/Composer.tsx`: the send path first sends `live_voice_interrupt { reason: "typed" }` so the audio stops like the button (FR-025), then sends the text as an **ordinary chat request on the same socket** — not through the session, and the session gains no frame for typed text. Its reply is text only (FR-018 withdrawn, 2026-11-08). With no session live the composer behaves exactly as today (FR-040)
- [ ] **T055 [BACKLOG]** [US3] Interrupt while nothing is being spoken is a no-op on state in `crates/vizier-core/src/live_voice/session.rs` (FR-021)
- [ ] **T056 [BACKLOG]** [US3] End-to-end check in `webui/e2e/live-voice-interrupt.spec.ts`: one press silences the agent, the session stays live and listening, and a typed message is answered **as text with no audio scheduled** — assert the `AudioBufferSourceNode` count stays zero after the typed turn (FR-025, FR-018 withdrawn)
- [ ] **T057 [BACKLOG]** [US3] Keep the interrupt control reachable without hiding it inside a live session — disabled with a reason only when the capability is false (FR-020). **Placed on the call screen, not on the archived bar (C6)**

**Checkpoint**: not part of this release. The only interruption path in scope is US2's (speaking). US3 is deferred (C7) and its tasks must not be implemented or tested now.

---

## Phase 6: User Story 4 — A session never keeps the microphone (Priority: P2)

**Goal**: every ending releases the microphone and leaves the text.

**Independent Test**: close the tab mid-answer; the OS microphone indicator goes out within 2 s and the reply text is kept; a silent session ends after the idle period with a reason.

- [ ] T058 [P] [US4] Tests in `webui/app/lib/liveVoice/session.test.ts` and `crates/vizier-http/tests/live_voice_session.rs`: page close releases tracks, idle stop ends with a reason, a second start on the same conversation is refused
- [ ] T059 [US4] One release path in `webui/app/lib/liveVoice/session.ts` — stop every track, close the `AudioContext`, drop the queue — called by every terminal transition
- [ ] T060 [US4] Wire `pagehide`/unmount handling in `webui/app/routes/chat.tsx` and `webui/app/screens/chat/Composer.tsx` so leaving the conversation ends the session (FR-028)
- [ ] T061 [US4] Refuse a second session on the same conversation in `crates/vizier-core/src/live_voice/session.rs` with `live_voice_error` reason `session_active`, and show it in `webui/app/screens/chat/LiveVoiceBar.tsx` (FR-005)
- [ ] T062 [US4] Idle stop and owner-gone ends in `crates/vizier-core/src/live_voice/session.rs`: idle timer (FR-029), agent reconfigured/deleted, conversation deleted, server restart
- [ ] T063 [US4] Ending leaves the control off and remembers nothing between visits in `webui/app/screens/chat/LiveVoiceButton.tsx` (no storage)
- [ ] T064 [US4] End-to-end check in `webui/e2e/live-voice-lifecycle.spec.ts`: close mid-answer, track `readyState` becomes `ended`, text kept, control off on reload
- [ ] T065 [US4] Mobile/desktop suspension (screen lock, WebView background) ends the session with a reason rather than staying "live" in `webui/app/lib/liveVoice/session.ts`

**Checkpoint**: SC-007 holds; no code path can leave listening.

---

## Phase 7: User Story 5 — A broken session explains itself (Priority: P2)

**Goal**: every failure is a visible reason within 5 s, never a session that looks live.

**Independent Test**: cut the connection mid-answer; the state reads reconnecting within 2 s and the audio stops; restore within 30 s and the same session continues without a duplicate message.

- [ ] T066 [P] [US5] Tests in `crates/vizier-http/tests/live_voice_failures.rs`: empty recognition, voice provider failing mid-reply, socket drop then reconnect inside the window, no device (FR-030–FR-033)
- [ ] T067 [US5] Reconnect in `webui/app/lib/liveVoice/session.ts` and `crates/vizier-core/src/live_voice/session.rs`: resume the same session within 30 s, never re-submit a message already in flight
- [ ] T068 [US5] Re-bind the audio lane on reconnect in `crates/vizier-core/src/live_voice/session.rs` and `.../registry.rs`: `TopicConnection::open` builds a **new** `ClientConnection` per socket (`crates/vizier-http/src/topic.rs:144`), so a session that kept the old `Arc` would stay live and go mute. A `live_voice_start` from the owner while their session is live is idempotent and adopts the caller's current connection; a session whose owner never comes back inside FR-033's 30 s ends with `connection_lost`. Test in `crates/vizier-http/tests/live_voice_failures.rs`: drop the socket, reconnect, the next `live_voice_speech` reaches the **new** connection (`review.md` D13)
- [ ] T069 [US5] Failure surfaces in `webui/app/screens/chat/LiveVoiceBar.tsx`: one reason line within 5 s and the microphone released (SC-009)
- [ ] T070 [US5] Voice provider failure mid-reply in `crates/vizier-core/src/live_voice/engines.rs`: the rest of the reply still appears as text, the failure is reported, the session listens again (FR-032)
- [ ] T071 [US5] No device and refused permission in `webui/app/lib/liveVoice/capture.ts`: fail with a reason, retryable without a reload (FR-030)
- [ ] T072 [US5] End-to-end check in `webui/e2e/live-voice-failures.spec.ts` using route interception to drop and restore the socket
- [ ] T073 [US5] Unintelligible transcript in `crates/vizier-core/src/live_voice/utterance.rs`: tell the person, send nothing, listen again (FR-031)

**Checkpoint**: every failure path in SC-009 is demonstrated.

---

## Phase 8: User Story 6 — The call has a screen of its own (Priority: P1, C6)

**Goal**: pressing the control opens a call, not a strip of chrome: the agent, its state, the
thinking indicator, the open microphone and a way out — and the conversation unchanged underneath.

**Independent Test**: from a conversation, press the control; the call screen opens in place of the
conversation view; ask something and watch the thinking state; end the call; the conversation is back
with both turns and the control off.

- [ ] T089 [P] [US6] Navigation for the call screen in `webui/app/routes/chat.tsx`: entering it replaces the conversation view (a view over the conversation, not a route of its own — open question Q8's default), and ending the call returns to the same conversation without a reload (FR-043, FR-046)
- [ ] T090 [US6] Release on leaving in `webui/app/screens/livevoice/LiveVoiceCall.tsx` and `webui/app/lib/liveVoice/session.ts`: leaving the screen by any means runs the one release path of T059 — every track stopped, `AudioContext` closed, queue dropped (FR-028, FR-045)
- [ ] T091 [P] [US6] Assert FR-046 in `webui/e2e/live-voice-screen.spec.ts`: a session with three turns, opened and left, leaves the conversation's messages, order and timestamps identical to a session that was never opened
- [ ] T092 [US6] Script SC-010 in `webui/e2e/live-voice-screen.spec.ts`: pressing the control shows an interactive screen within 500 ms on the first attempt, on a fresh profile, with microphone permission granted at the system prompt (SC-005's clock stops here, not at the first answer)

**Checkpoint**: a person can hold a conversation without ever seeing the chat layout, and can leave it without losing a word of it.

---

## Phase 9: Polish & Cross-Cutting Concerns

**Purpose**: the documentation, the measurement, and the gates the constitution asks for.

- [ ] T074 [P] Write the user documentation page `docs/src/configuration/live-voice.md` (how to start and end a call, the call screen, what audio goes to which provider, what is stored, how to interrupt by speaking **and that there is no interrupt button in this release** — spec C7 — plus the limits) and add it to `docs/src/SUMMARY.md` (FR-039)
- [ ] T075 [P] Add the build-flavour note for `provider-live-voice` beside the other optional features in `docs/src/deployment/` (whichever page lists the `full` build)
- [ ] T076 Define the three measured gates before anything is measured, in `webui/app/lib/liveVoice/session.ts` (marks) and `crates/vizier-core/src/live_voice/session.rs` (spans): **SC-001** = `performance.now()` at the client's end-of-utterance (hangover fired) to the **first sample that is actually audible**, computed as the `AudioBufferSourceNode.start()` timestamp plus the `AudioContext.baseLatency + outputLatency` (or the scheduled `when`, if the first segment was queued ahead) — never the frame's arrival at the socket, and never a server span, or the gate measures a different event than "first audible word"; **SC-002** = the same end-of-utterance mark to `player.stop()` returning (client) and to `Abort` reaching the agent (server span); **SC-003** = the interrupt click handler entry to `player.stop()` returning. Emit one `live_voice_metric` line per turn with the stage durations, and write the procedure into `specs/055-live-voice/latency.md` — without fixed marks the numbers in the plan are not a gate (`review.md` D11)
- [ ] T077 Instrument the stages in `crates/vizier-core/src/live_voice/session.rs` (end of utterance, recognition, turn start, first segment, first frame) with `tracing` spans and record p50/p90/p99 in `specs/055-live-voice/latency.md`
- [ ] T078 Run the flag matrix and record it: `cargo test -p vizier-core`, `cargo test -p vizier-core --features provider-live-voice`, `cargo test -p vizier-http`, `cd webui && npm test`, `cd webui && npm run e2e`. **SC-006 lives here**: the flag-off build must pass the *existing* speech checks unchanged — the suites that already cover the mic/`stt_transcribe`, the spoken-reply/`tts_generate`, auto-transcription of an uploaded clip, and `ReplyMedia` playback — so the row records those four by name, not just "everything is green" (`analysis.md` M4)
- [ ] T079 Run `cargo clippy --all-targets` for both flag states and `cd webui && npm run lint`; fix everything new
- [ ] T080 Walk `quickstart.md` by hand with a hosted provider and record the result of every step in the pull request
- [ ] T081 Run the `dummyplug` agent-observable e2e gate from the constitution against a running binary (message in, history out) with the speech providers faked
- [ ] T082 Remove dead code and leftover TODOs from the new modules (`crates/vizier-core/src/live_voice/*`, `webui/app/lib/liveVoice/*`) before the review gate
- [ ] T083 [P] Assert FR-016 in `crates/vizier-core/src/live_voice/utterance.rs`: the raw audio never reaches storage — the PCM buffer is dropped once the recogniser returns, no `SessionFile`/attachment is created for it, and a test drives one full turn then reads the conversation's stored requests and files to prove only text is there (FR-016; no task asserted this before, `analysis.md` M1)
- [ ] T084 [P] Assert FR-035 in `crates/vizier-http/tests/live_voice_frames.rs`: the Discord and Telegram paths never emit a `live_voice` frame and never gain a control, because the feature is offered on the chat surface only — a session started from the WebUI leaves those readers exactly as they are today (`analysis.md` M2)
- [ ] T085 [P] Soak the loop in `webui/e2e/live-voice-soak.spec.ts`: 10 minutes, 15 turns, no interaction other than speaking, every spoken message present in the conversation — SC-004's "no lost turns in 100%" is the one criterion nothing in the plan measured (`analysis.md` M3)
- [ ] T086 [P] Script SC-005 in `webui/e2e/live-voice-first-use.spec.ts`: a session on a fresh browser profile, no prior knowledge of the feature — the clock runs from the first paint of the chat screen to the first audible reply, and the test fails above 20 s including the microphone permission prompt. The permission dialog is stubbed (`grantPermissions`) so the measurement is the product's, not the browser's dialog (`analysis.md` M4)
- [ ] T087 [P] Assert FR-008 in `crates/vizier-core/src/live_voice/session.rs` and `crates/vizier-http/tests/live_voice_session.rs`: a session reads the agent's configured model and speech settings and writes none of them — after a full turn the agent config is byte-identical, and a typed turn started in the same conversation uses the same model the voice turn did (`analysis.md` M2)
- [ ] T088 Hand the branch to `@teteh-qa` for `speckit-analyze` and `speckit-checklist`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies
- **Foundational (Phase 2)**: needs Setup; **blocks every story**
- **US1 (Phase 3)**: needs Foundational
- **US2 (Phase 4)**: needs Foundational; its barge-in is only observable once US1 can speak, so in practice after US1
- **US3 (Phase 5)**: **backlog — not built, not gated (C7)**
- **US4 (Phase 6)**: needs US1 (a session to release)
- **US5 (Phase 7)**: needs US1 and US4 (an ending to report)
- **US6 (Phase 8)**: needs US1's client session and T037's entry control; independent of US2
- **Polish (Phase 9)**: after the stories that are in scope

### User Story Dependencies

- **US1** — no story dependency. MVP.
- **US2** — depends on US1's playback path; the interruption logic itself is independent of US3.
- **US3** — **deferred to the backlog (C7)**; its acceptance criteria are not part of this release.
- **US6** — depends on US1 (a session to put on a screen); independent of US2, and its screen is where US2's interruption is observed by the person.
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
Task: "crates/vizier-http/tests/live_voice_session.rs — one utterance, one request"   # T023
Task: "crates/vizier-core/src/live_voice/segmenter.rs — segment boundaries"            # T024
Task: "webui/app/lib/liveVoice/session.test.ts — state machine"                        # T025

# then the server modules and the client modules side by side
Task: "crates/vizier-core/src/live_voice/{session,utterance,engines,segmenter}.rs"     # T026–T032
Task: "webui/app/lib/liveVoice/{capture,vad,session,player}.ts"                        # T033–T036
```

## Implementation Strategy

### MVP first (User Story 1 only)

1. Phase 1 and Phase 2 to completion (the flag, the frames, the capability).
2. Phase 3 to completion.
3. Stop and validate with `quickstart.md` steps 1–3, then demo. The demo is the **call screen** (US6), not a composer bar.

### Incremental delivery

US1 → demo (talk and hear) → **US6 (the call screen)** → US2 (barge-in, now triggered by the end of the
person's speech) → US4 (microphone discipline) → US5 (failures) → Polish. **US3 is not in this sequence:
it is backlog (C7).** Every story leaves the previous one working, and a flag-off build stays
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

This file was written on this branch earlier the same day (`6357b0c5 docs: add tasks`, 74 tasks;
`f2756b30 docs: add tasks`, 77) — **both by `@mamang-lead-be`**, which closes the ownership question
raised at 15:25 — and is **verified and amended here three times**, not rewritten: all 74 original
tasks were checked against the code and kept; **six** were added (T017, T018, T019 for the findings
below; T020, T021, T022 for the delivery path), eight were made more specific, and the ID sequence was
renumbered three times (old `T017` and up moved by three, old `T038`/`T040` and up by two, old `T022`
and up by one, and again by two for SC-005/FR-008) so the list stays in execution order. Total: 88 tasks,
T001–T088 (`analysis.md` M12 caught the
stale 84 here and the stale phase count below).

**Why this delta exists**: `review.md` re-checked every citation in `research.md`/`plan.md` against the
tree. Seven findings change what a task must say; `architecture.md` is the shape they take.

**Delta 2026-11-08 21:19 (User: surface, scope, trigger — spec C6/C7/C8).** 88 → **92** tasks
(T001–T092), no ID renumbered: four were **appended** for the call screen (T089–T092) and the seven US3
tasks are kept, tagged `[BACKLOG]`, and excluded from every checkpoint. Changed in place: **T037**
(entry control opens a call, not a toggle), **T038** (the composer bar became the call screen;
the `20d97316` bar amendment is archived, so its 210 px floor and narrowing order are **not**
requirements), **T039**, **T040**, **T044** and **T045** (the interruption trigger moved from
`speech_start` to `utterance_end`), **T047** (the onset guard is replaced, and reply ducking is
added), **T048** (a sub-word burst now cancels nothing), **T074** (the docs page must say there is no
interrupt button). The phase list grew one story: US6 is Phase 8 and Polish moved to Phase 9.

| Finding | Task touched | What changed |
|---|---|---|
| **D1** — an abort before any reply text is an `Abort` turn that produced nothing (`schema/response.rs:156-166`), so "readable as far as it was produced" has no text to read | T045 | interruption marks the reply only when its `Message` had already arrived; otherwise "Stopped by you", no invented partial |
| **D2** — nothing server-side records that a message came by speech (`grep -rn "spoken" crates/` is empty) | **new T017, T018**; T029 | `#[serde(default)] spoken: bool` + the FR-017 note in `agents/agent/mod.rs:712-750`; the submission task now names the field |
| **D3** — the frame contract had no discriminator inside `live_voice` (contract delta C-1) | T009 | `#[serde(tag = "type", rename_all = "snake_case")]` |
| **D4** — outbound frames are a hand-written `match` (`topic.rs:535-600`), not free serialisation | T010 | the task names the match and its line range |
| **D5** — `handle_inbound` stringifies every inbound frame before dispatch (`topic.rs:216`) | **new T019** | the `live_voice_*` branch must be taken before that copy |
| **D6** — the registry cannot live on `TopicConnection` (per socket) and still refuse a second tab | T026 | one process-wide registry handed over like `Arc<Questions>` (`topic.rs:79-81`) |
| **D7** — `hound` sits behind `provider-media`; using it would couple the new flag | T028 | 44-byte WAV header by hand |
| **D9** — audio on the session-event broadcast: `Lagged` drops all-or-nothing (`topic.rs:609-613`), the ring is 1024 slots for the whole process (`transport/mod.rs:29`), and the socket itself is an unbounded `flume` (`channel.rs:374`), so "drop only audio" was unimplementable and the real memory bound was missing | **new T020, T021**; the old backlog task is now T041 | frames go to the owner's socket through `ClientConnection::send`, never `publish_session_event`; audio is dropped above `LIVE_VOICE_SOCKET_BACKLOG = 16` at the one place that can see the queue, control frames never are; a closed socket ends the session with `page_closed`; **T022** makes the `Lagged` arm state its own rule (it can drop control and text, never audio, because audio is not in the ring) and proves a 2 000-frame audio burst cannot make a subscriber lag. `plan.md` Phase C/fallbacks/risks, the contract (`§Delivery path`, C-6) and `architecture.md` (§11, I9) carry the same decision |
| **D16** — where the audio lane drops and who counts | T021, T041 | one counter (`backlog`), owned by the outbound loop that performs the drop, reported as one `stall` control frame per episode; the session's queue-full is a logged backstop; `channel.rs`/`embedded.rs` untouched, no `open()` signature change (`architecture.md` §11) |

| ~~**D17** — FR-018 "answered and spoken" had no mechanism~~ **superseded 2026-11-08 by the User's decision** | T053, T056 (T054 deleted) | FR-018 is withdrawn: a typed message is text only. The `live_voice_text` frame, its contract entry (C-12), its task and its assertion are gone; typing still stops the audio (FR-025) and still works as ordinary chat (FR-040) |

**Also recorded, no task changed**: the author's claim "no new mandatory dependency" holds
(`Cargo.lock`: `base64 0.23.1`, `tokio` `sync` feature, `serde`, `AudioContext`/`AudioWorklet` already
shipped); `tokio` is `1.53.3`, not the `1.49.0` written in `research.md` §C; `classifyFrame` lives at
`webui/app/api/frames.ts:606-630`, not `:619-621` (already corrected in the analyst's `e5ac773a`).

**Execution order is unchanged**: Phase 1 → Phase 2 (16 tasks, T007–T022) → US1..US5 →
Polish. `@sepuh-be` implements; `@teteh-qa` verifies with `speckit-analyze` + `speckit-checklist`
after T088.
