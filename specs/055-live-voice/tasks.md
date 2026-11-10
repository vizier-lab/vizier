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

## Checkbox convention (AQ-001, 9 Nov 2026)

Two markers, and they are not the same claim:

- **Landed `<hash>`; the gate it names passed.** — the gate named in the task was run and passed.
- **In the tree: `<repo>@<hash>`; not gated in AQ-001.** — the deliverable is present in the
  working tree and the named commit is where it came from; **no test was run for this
  reconciliation**. A tick here means "the code exists and has a commit", not "verified against
  `spec.md`".

`AQ-001` only reconciled the checkboxes with `git log`; it did not run a gate for any task it
ticked. Gating is still `@teteh-qa`'s signature at `gate` in `specs/_antrean.md`.

**Baseline for implementers (9 Nov 2026, Lead):** this file at **`3648f421`** — 110 tasks,
T001–T110, no renumbering. A DoD cites its commit as **`repo@hash`** (`vizier@…`, `webui@…`,
`design@…`): a bare hash is ambiguous across the submodules, and `grep` from the root reports a
perfectly good `webui` commit as missing.

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: the flag, the module skeleton, the empty client folder — no behaviour yet.

- [x] T001 Add the `provider-live-voice` feature to `crates/vizier-core/Cargo.toml` (`default = []`, added to `full`); confirm `crates/vizier/Cargo.toml` already reaches it through `full`, and add a comment beside the feature saying what it gates **In the tree: `vizier@f7499c2a`; not gated in AQ-001.**
- [x] T002 [P] Create `crates/vizier-core/src/live_voice/mod.rs` with the `#[cfg(feature = "provider-live-voice")]` gate, the `mod session; mod engines; mod utterance; mod segmenter;` declarations and the re-exports; register the module in `crates/vizier-core/src/lib.rs` **In the tree: `vizier@f7499c2a`; not gated in AQ-001.**
- [x] T003 [P] Create `webui/app/lib/liveVoice/frames.ts` with the wire types of `contracts/live-voice-frames.md` (client→server and server→client) plus pure `encode`/`decode` helpers, no side effects **In the tree: `webui@fbb204f`; not gated in AQ-001.**
- [ ] T004 [P] Create `webui/app/lib/liveVoice/__fixtures__/README.md` listing the two clips the VAD tests need (a person speaking over a playback sample, and the playback sample alone at 80% volume) and how they were produced
- [x] T005 [P] Create the empty test files so the suites exist before the code: `crates/vizier-http/tests/live_voice_frames.rs`, `webui/app/lib/liveVoice/session.test.ts`, `webui/app/lib/liveVoice/vad.test.ts` **In the tree: `vizier@f1cc0016`; not gated in AQ-001.**
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

- [x] T007 Add `LiveVoiceSettings` (`stt: Option<…>`, `tts: Option<…>`, `idle_stop_seconds`, `max_utterance_seconds`, `end_of_utterance_ms`, all defaulted) and `live_voice: ToolConfig<LiveVoiceSettings>` in `crates/vizier-core/src/schema/agent.rs` **In the tree: `vizier@f7499c2a`; not gated in AQ-001.**
- [x] T008 Serde test in `crates/vizier-core/src/schema/agent.rs`: an agent JSON **without** the block loads and reads `enabled: false`; one **with** a partial block loads with the documented defaults (FR-040) **In the tree: `vizier@f7499c2a`; not gated in AQ-001.**
- [x] T009 Add the wire types in `crates/vizier-core/src/live_voice/frame.rs`, `#[serde(tag = "type", rename_all = "snake_case")]` (`state` / `transcript` / `speech` / `error` / `stall`) per contract delta C-1, **split in two so the compiler holds the lane**: `LiveVoiceControl` (`state`, `transcript`, `error`, `stall`) and `LiveVoiceSpeech`. `SessionFrame::LiveVoice(LiveVoiceControl)` in `crates/vizier-core/src/transport/mod.rs` therefore *cannot* carry audio — publishing a speech frame as a session event is a type error, not a review rule (`review.md` D9), and the control-only publish helper of T020 needs no runtime check **In the tree: `vizier@f7499c2a`; not gated in AQ-001.**
- [x] T010 Map `SessionFrame::LiveVoice` to a top-level `live_voice` object in the outbound match of `crates/vizier-http/src/topic.rs:535-600` — every variant is mapped by hand there, so this is an explicit arm, not free serialisation (`review.md` D4); the match is exhaustive, so the compiler is the check. This arm is reached through `events.recv()`, the broadcast, and carries **control only** (`state`, `transcript`, `error`, `stall`): `live_voice_speech` **never** passes here, it is written per connection by T020 (contract §Delivery path), and T009's type split makes sending it here a compile error rather than a review rule. Write the arm as `serde_json::to_value(&control)` wrapped in the `live_voice` key — the same serialiser the speech lane uses — not a hand-built `json!(...)`, so the two lanes cannot drift in field names (`analysis.md` M15). Test in `crates/vizier-http/tests/live_voice_delivery.rs`: a control frame published as a session event reaches every socket on that conversation, and no `live_voice_speech` frame is ever observable on `session_events` **In the tree: `vizier@f7499c2a`; not gated in AQ-001.**
- [x] T011 Add `live_voice: bool` beside `tts`/`stt` in the agent detail type at `crates/vizier-core/src/service/agents.rs:69-71`; it reads true only when the flag is on **and** the agent has a usable recogniser and voice (FR-004) **In the tree: `vizier@f7499c2a`; not gated in AQ-001.**
- [x] T012 [P] Read `detail.live_voice` in `webui/app/routes/chat.tsx:104-119` beside the existing `stt`/`tts` and pass it to the composer's control state **In the tree: `webui@fbb204f`; not gated in AQ-001.**
- [x] T013 [P] Add the `live_voice` case to `classifyFrame` in `webui/app/api/frames.ts:597-630` (a new `kind: 'live_voice'`) plus a case in `webui/app/api/frames.test.ts` proving an older payload still returns `null` **In the tree: `webui@fbb204f`; not gated in AQ-001.**
- [x] T014 Contract test in `crates/vizier-http/tests/live_voice_frames.rs`: with the flag **off** a `live_voice_start` is ignored without closing the socket and no `live_voice` frame is ever emitted (FR-041). **And with the flag on but the STT/TTS tool switches off**, a session still starts and runs a turn: their switches MUST neither start nor block live voice (FR-003 — two speech paths now coexist while only one is on screen, so this is the transition rule that keeps the overlap honest) **In the tree: `vizier@f1cc0016`; not gated in AQ-001.**
- [x] T015 Guard the inbound audio path in `crates/vizier-http/src/topic.rs`: `live_voice_audio` larger than one chunk (~6 KB of base64) or a chunk rate above the contract's cap is refused with `live_voice_error`, not buffered. This is the shipped carrier (Q11), so the bound is load-bearing rather than provisional **In the tree: `vizier@f7499c2a`; not gated in AQ-001.**
- [x] T016 [P] Regenerate the typed API schema (`webui/app/api/schema.d.ts`) with the project's existing generator script so `live_voice: bool` is in the generated types **In the tree: `webui@fbb204f`; not gated in AQ-001.**
- [x] T017 Add `#[serde(default)] spoken: bool` to `VizierRequest` in `crates/vizier-core/src/schema/request.rs`, with a serde test proving a stored request from before this feature loads as `false` — the one field FR-011's "marked as spoken" and FR-017's indication both need (there is none today: `grep -rn "spoken" crates/` is empty; `review.md` D2) **In the tree: `vizier@f7499c2a`; not gated in AQ-001.**
- [x] T018 [P] Use `spoken` on the turn: append the "this came by speech, answer for listening" indication in the block that already preprocesses voice input, `crates/vizier-core/src/agents/agent/mod.rs:712-750`, gated on `spoken` so no typed turn changes (FR-017); render the spoken mark on the stored request in history (server side `SessionHistoryContent::Request`, client side the FR-011/Q5 badge) **In the tree: `vizier@f7499c2a`; not gated in AQ-001.**
- [x] T019 Take the `live_voice_*` branch in `crates/vizier-http/src/topic.rs:209` `handle_inbound` **before** the `let text_str = frame.to_string();` copy at `:216`, so a session does not serialise its own audio twice on the way in (4.3 KB x 10/s per session); add a test that one audio chunk is dispatched without being stringified (`review.md` D5) **In the tree: `vizier@f7499c2a`; not gated in AQ-001.**
- [x] T020 Carry live voice frames to **one** socket, never the session broadcast, in `crates/vizier-core/src/live_voice/session.rs` (where `LiveVoiceSessions` lives, `:430`; there is no separate `registry.rs`): the session holds the owner's `Arc<ClientConnection>`, so delivery is a call, not a subscription (`review.md` D9). This is the shipped path (Q11), so a second subscriber must never receive one person's audio (FR-054) **In the tree: `vizier@f7499c2a`; not gated in AQ-001.**
- [x] T021 Bound the socket, not the broadcast: `LIVE_VOICE_SOCKET_BACKLOG = 16` in the `client_rx` arm of `TopicOutbound::run` (`crates/vizier-http/src/topic.rs:519-525`) — above it, audio frames are dropped and one `stall` is reported, because a lost syllable is cheaper than a lost session (Q11: this is the only carrier, so this bound is the feature's only backpressure) (FR-054) **In the tree: `vizier@f7499c2a`; not gated in AQ-001.**
- [x] T022 Speak for the `Lagged` arm in `crates/vizier-http/src/topic.rs:609-613`: it is reachable because it drops **control and text** frames only once audio is off this ring (T020), and the arm's behaviour stays (a lagging subscriber cannot be repaired by replay). Keep the test either way: the ring is 1024 slots for the whole process (`transport/mod.rs:29`), so what it may not lose is text (FR-054) **In the tree: `vizier@f7499c2a`; not gated in AQ-001.**

**Checkpoint**: the capability reads true/false correctly, the frames round-trip, and a flag-off build is indistinguishable from today.

---

## Phase 2b — The audio path, the public contract, and third-party access (C12, C13, C14, C15)

**Goal**: the sound travels on the path Q11 settled — the conversation's own socket — the transcript
stays beside it, an outside system can call live voice with a **key that grants nothing else**, and the
manual speech **surfaces** step aside while their code stays.

**On the ids**: T099–T110 although the phase runs late. The id is not the order — the phase is (see
§Delta). **Phase 2b runs after Phases 3, 4, 6 and 7**, not after Phase 2: T103 edits
`webui/app/lib/liveVoice/capture.ts` and `session.ts`, which are born in T033/T035, and T110 needs
T059's release path and T097's turn-at-a-move. Run in id order it would edit files that do not exist
yet, and the tree would not compile between Phase 2b and Phase 3. **Q11 is closed on
`ws-json-pcm16`**, so T100–T104 describe the socket path that already exists and is tested; nothing
here builds a media transport, and `media-track` remains a reserved value that a server must refuse.

- [x] T099 **The carrier is decided: the socket.** Q11 was closed by the User on 2026-11-09 16:35 — *"gw setuju pake websocket dulu aja"* — after the numbers were put in front of them. Record that decision, and the numbers that made it a decision, in `plan.md`: the socket (+0 crates, 0 build, 0 RSS, no relay) against a media transport (+22 crates for `str0m`, 36 s probe build, 583 MB `target`, 1,25 GB peak RSS, an Opus decoder on the server, a relay). **Record what the socket cannot do as well** — no jitter buffer, no PLC, no FEC, so packet loss is a hole in the sound — so a future reversal starts from the real trade-off. **No `Cargo.toml` change**, no peer setup, no relay: this task closes a question, it does not open a work stream. `media-track` stays a reserved value in the contract and is refused **In the tree: `vizier@305fc838`; not gated in AQ-001.**
- [ ] T100 [P] **The door, on the socket** (FR-058, FR-061, FR-064, contract `Authentication`): `live_voice_start` carries the audio description the client will send, and a caller without a usable identity — a person's session or a key scoped to live voice — is refused **before any session state exists**, with `kind: "forbidden"` and no hint about whether the conversation, the agent or a session exists. Test: an unauthenticated caller cannot start anything, an under-scoped key is refused identically, and a second start for the same owner is refused the way FR-005 says (Q2's default: refused for the same conversation)
- [ ] T101 Bind the audio to one session in `crates/vizier-core/src/live_voice/session.rs`: audio belonging to a session that has ended, arriving for a conversation where no session is live, or sent by a person who does not own the session is **refused** — never adopted by whatever is live there now (FR-056). Test all three refusals, each with a reason word the contract names
- [x] T102 [P] The audio description, in the contract and on the acceptance path: sample rate, channel count, frame duration and the acceptable codec or format stated so a non-browser client can meet them, and enforced on what arrives so a client that meets them is accepted (FR-057). Test: a conforming description is accepted, a non-conforming one is refused with a reason a client can show **In the tree: `vizier@de006e3e`; not gated in AQ-001.**
- [x] T103 [US1] `webui/app/lib/liveVoice/capture.ts` + `session.ts`: send the microphone as **100 ms base64 PCM16 chunks on the socket** (the carrier Q11 settled), keeping the `getUserMedia` constraints of T033 untouched. The chunk encoder stays — it is now the shipped path, not a fallback. Test: a whole session reaches the server with exactly the description T102 publishes **In the tree: `webui@fbb204f`; not gated in AQ-001.**
- [ ] T104 [P] **Refuse the reserved path.** A client that sends an offer or declares `media-track` is **refused with one named kind, reported once** — nothing in this release can terminate a peer, decode Opus or relay, so accepting the offer would be a lie the sound pays for. Two audio sources for one utterance have no correct ordering and `utterance_id` cannot tell them apart, which is the second reason the refusal is unconditional. Test: a media offer and an `opus`/`media-track` description are both refused, and the session continues on `ws-json-pcm16` if one is live
- [ ] T105 [P] Publish the contract in `docs/`: the six surfaces of FR-058 — how a call is set up, session identity, authentication, the audio description, lifecycle, failure vocabulary — written for someone who has never seen this repository and cross-linked from the existing speech page. The wire shapes are the socket's, as Q11 settled them, with `media-track` shown only as the reserved
value a client must not use. Gate: a reviewer follows it end to end without opening `webui/`
- [ ] T106 Conformance test for **SC-012**: a client written against the published document alone starts a session, speaks, hears the reply and ends the call against the real server. Written from `docs/`, not from `webui/app/lib/liveVoice/`, and it is the check that fails first when the contract and our own client drift (FR-058)
- [x] T107 Retire the manual speech **surfaces** (FR-053): the composer's recording control and the spoken-reply toggle leave the surface; their agent settings stay readable so nothing is silently reconfigured. **Do not delete the tools' code, providers, routes or flags** — the User kept them deliberately for another surface later (2026-11-09 16:16), and this task retires a surface, not a feature. Test: the tools still answer when called directly while no button offers them **In the tree: `webui@70437fb`; not gated in AQ-001.**
- [ ] T108 Prove FR-041 got **harder, not narrower**: with live voice compiled out (`cargo build -p vizier --no-default-features`), the app still chats and **every existing speech tool still works** — which matters more than before, because two speech paths now coexist while only one is on screen. Test: flag off, STT and TTS reachable and functional, live voice frames refused with no socket closed (T014)
- [x] T109 **A scoped API key for third-party live voice (FR-061, FR-062, FR-064).** Add a **capability** to an API key and enforce it: a key issued for live voice may start, speak, hear, interrupt, end and move a session for its owner and **nothing else**. Today `crates/vizier-http/src/auth/middleware.rs:183-209` resolves a key to the owner's **full role and permissions** — line 200 — and `crates/vizier-core/src/storage/user.rs:69-81` has no scope field, so this is code, not prose. The effective permission of a key MUST be the **intersection** of its capabilities with its owner's, never their union. Tests: a key without the live-voice capability is refused by every live-voice action with `kind: "forbidden"`; a key with it is refused by an explicit list of the other routes (history, settings, memory, files, jobs, provider credentials); neither refusal reveals whether a conversation, an agent or a session exists (FR-064) **In the tree: `vizier@8c313813`; not gated in AQ-001.**
- [x] T110 **A revoked or expired key ends the live session it opened (FR-063, FR-063a).** Revocation and expiry MUST act on a **running** session, not only on the next connection: the session releases the microphone through the single release path of T059, cancels the turn in flight (FR-052's path), and reports `ended` with `key_revoked` or `key_expired` (contract, reason vocabulary). Rotation means a new key plus the old one retired, so it takes the same path as revocation. Test: revoke mid-answer, and assert the release, the cancelled turn and the reason **within 1 s (SC-013 — revocation only)**. Expiry is FR-063a and carries **no time bound**: it is enforced at the next frame on the same connection, so the test asserts the reason and the three consequences, never a deadline **Landed `vizier@093c2b92` (with `vizier@30500bf5`); the gate it names passed.** Gate: Lead (technical) + `@teteh-qa` **PASS WITH KNOWN ISSUES**, 10 Nov 2026. Proved by test: expiry ends the call at the next frame on an open connection, exactly one `Command("abort")`, registry emptied, reason `key_expired`; a session a person opened is untouched. **Not proved, and this tick must not be read as it:** `SC-013`'s 1 s is unmeasured (`GI-01`), the revoke path's cancelled turn is composition rather than a test (`GI-02`), and a revoked key can still start a session on an open socket (`GI-05`). So this task is landed, `FR-063` is not closed.

---


## Phase 3: User Story 1 — A conversation without typing (Priority: P1) 🎯 MVP

**Goal**: press the control, speak, see the message, hear the answer, ask a follow-up without pressing.

**Independent Test**: `quickstart.md` steps 1–3 with a hosted recogniser and voice: both questions and both answers land in the conversation as text.

### Tests for User Story 1

- [x] T023 [P] [US1] Session test in `crates/vizier-http/tests/live_voice_session.rs` with a faked recogniser: `start` → three `audio` chunks → `utterance_end` produces exactly one request whose text is the transcript **In the tree: `vizier@f1cc0016`; not gated in AQ-001.**
- [x] T024 [P] [US1] Segmenter tests in `crates/vizier-core/src/live_voice/segmenter.rs`: splits at sentence and clause boundaries, never inside a word, skips fenced code, tables and raw URLs with one notice (FR-014) **In the tree: `vizier@f7499c2a`; not gated in AQ-001.**
- [x] T025 [P] [US1] Client state machine test in `webui/app/lib/liveVoice/session.test.ts`: `off → connecting → listening → waiting → speaking → listening`, and `start` is never sent twice **In the tree: `webui@fbb204f`; not gated in AQ-001.**

### Implementation for User Story 1

- [ ] T026 [US1] `crates/vizier-core/src/live_voice/session.rs` — no separate `registry.rs` module: one `Arc<LiveVoiceSessions>` per process, held beside `Arc<vizier_core::questions::Questions>` and handed to `TopicConnection` the same way (`crates/vizier-http/src/topic.rs:79-81`) — per-socket state cannot enforce FR-005 across two tabs (`review.md` D6); key (agent, conversation), the FR-006 states, owner principal, `tokio::sync::watch<u64>` generation, `last_speech_at` and the idle timer **In the tree: `vizier@bba39242`; not gated in AQ-001.** **Unchecked 2026-11-10 by Lead: `last_speech_at` + idle timer have 0 hits in `bba39242` and in `cf663414` (`grep -rn last_speech_at` empty); `EndReason::Idle` is only produced by a test fixture, `frame.rs:420`. Re-check only after FR-029 / T110 work lands.**
- [x] T027 [US1] Accept `live_voice_start`, `live_voice_audio`, `live_voice_utterance_end` and `live_voice_stop` in `crates/vizier-http/src/topic.rs`, authorised exactly like a chat message on the same socket (FR-005) **In the tree: `vizier@bba39242`; not gated in AQ-001.**
- [x] T028 [US1] `crates/vizier-core/src/live_voice/utterance.rs`: accumulate PCM16, cap at `max_utterance_seconds`, write the 44-byte WAV header **by hand** (not `hound`, which lives behind `provider-media` and would couple the new flag to it — `review.md` D7), decide `too_short` **In the tree: `vizier@f7499c2a`; not gated in AQ-001.**
- [x] T029 [US1] Submit the transcript with `transport.send_request(session, req, Some(response_tx))` (`crates/vizier-core/src/transport/mod.rs:467`) from `crates/vizier-core/src/live_voice/session.rs`, carrying its own `ResponseSender` so the reply read is provably this turn's (FR-007), with the `spoken` field from T017 (FR-011, FR-017). **Assert FR-055 here**: what the conversation renders after a spoken turn is the message the turn stored — not a copy assembled from `live_voice_transcript` frames — because two renderings of one utterance is how the text a person reads and the text a person said start to disagree **In the tree: `vizier@bba39242`; not gated in AQ-001.**
- [x] T030 [US1] `crates/vizier-core/src/live_voice/engines.rs`: the single `LiveVoiceEngines` trait plus its cascade implementation over `VizierStt`/`VizierTts` (FR-036, FR-036a). The recognition call passes the conversation's configured language to `VizierStt::transcribe(audio, filename, language)` — the same `stt.settings.language` the auto-transcription path uses (`crates/vizier-core/src/agents/agent/mod.rs:721`); test a non-default language reaching the call (FR-037, `review.md` D14) **In the tree: `vizier@bba39242`; not gated in AQ-001.**
- [x] T031 [US1] `crates/vizier-core/src/live_voice/segmenter.rs`: reply text → speakable segments, first boundary preferred after ~80 characters (FR-013), **and only segments of that text** — the words spoken correspond to the words shown, because a turn delivers a finished message rather than token deltas (FR-012) **In the tree: `vizier@bba39242`; not gated in AQ-001.**
- [x] T032 [US1] Emit `live_voice_speech` frames from `crates/vizier-core/src/live_voice/session.rs` with `generation`, `seq`, `final`, converting the provider's audio to PCM16 16 kHz mono, synthesising segment *n+1* while *n* is sent **In the tree: `vizier@bba39242`; not gated in AQ-001.**
- [x] T033 [US1] `webui/app/lib/liveVoice/capture.ts`: `getUserMedia` with `echoCancellation`, `noiseSuppression`, `autoGainControl` (FR-026, FR-047) — these three constraints are the whole of T099's cheapest argument, so they are asked for explicitly, never left as `audio: true`. What happens next to the stream is T103's decision to implement, not this task's to presume **Landed `webui@fbb204f`; the gate it names passed.**
- [x] T034 [US1] `webui/app/lib/liveVoice/vad.ts`: energy gate over a 20 ms window, adaptive noise floor, 800 ms end-of-utterance hangover (FR-009, FR-010) **In the tree: `webui@fbb204f`; not gated in AQ-001.**
- [x] T035 [US1] `webui/app/lib/liveVoice/session.ts`: the client loop — start/stop, chunked send, frame handling, state transitions, `waiting` while the turn runs — and **the hand-back that closes the loop**: after a reply ends, and after an interruption, the session returns to `listening` **without a press** (FR-015). A loop that needs a button to continue is not a conversation **Landed `webui@fbb204f`; the gate it names passed.**
- [x] T036 [US1] `webui/app/lib/liveVoice/player.ts`: one `AudioContext`, an ordered queue of `AudioBufferSourceNode`s scheduled back to back, `stop()` that closes every source and drains the queue (FR-022 groundwork) **Landed `webui@fbb204f`; the gate it names passed.**
- [ ] T037 [US1] `webui/app/screens/chat/LiveVoiceButton.tsx`: the single entry control, separate from the composer's other controls, rendered only when the capability is true, off on every visit, and showing that pressing it **shows the live row** (FR-001, FR-002, C11). It is the only thing the conversation gains **Not in effect: superseded by C11 — the entry control is `LiveVoiceControl.tsx`; this file is archived in place (`webui@fbb204f`). Dead code for T082.**
- [x] T038 [US1] `webui/app/screens/chat/LiveVoiceRow.tsx`: **the live row, inside the composer's own meta line** — the line that carries the context ring and the model chip today, **24 px, 0 px added** to a 156 px composer (FR-043, surface B1-1). It shows the FR-006 state word in the ring's slot, the FR-042 thinking indicator, the microphone indicator including an unfocused window (FR-038), the elapsed time and the single End action (FR-045). **The agent's name is T112, not this task** (FR-044). **Its order of surrender is fixed and is not the archived one**: the model chip shrinks to 96 px then goes (cut to its last segment), the waveform 39 → 12 bars → gone, the context ring goes whole — while the microphone indicator, the state word, the elapsed time, the End action and the idle control never shrink, never wrap and never take an ellipsis (FR-049). Floor 292 px. The three archived surfaces (bar, screen, panel) are **not** specifications for it (C11) **In the tree: `webui@7436e7a`; not gated in AQ-001.** **Tick kept, clause removed (2026-11-10, PM ruling 12:53):** this task was unchecked twice because its sentence promised *"It shows the agent … (FR-044)"* and the row carries no agent (`webui@34e7060 app/screens/chat/LiveVoiceRow.tsx:205-260`, props `:17-29`). The tick is restored and the agent clause is **out of this task**, because T038 also carries FR-043, FR-045 and FR-049 through the carrier table (`:981`, `:983`, `:987`) and all three work in the tree — withdrawing the tick would mark three finished requirements as unfinished, the same error in the opposite direction. FR-044 is carried by **T112**, which is open.
- [x] T112 [US1] `webui/app/screens/chat/LiveVoiceRow.tsx` + `webui/app/screens/chat/LiveVoiceControl.tsx`: **the row shows the agent the call is with** (FR-044, `spec.md:565`). The name is rendered **in the row itself** — not the conversation header, not a tooltip, because `spec.md:572` already settles that reading by the same standard FR-038 applies to the microphone ("a tooltip is not 'show'"). It **truncates with an ellipsis (U+2026)** at a glyph boundary and never wraps; a remainder shorter than ~4 visible characters drops the name rather than leaving an orphan ellipsis. **It surrenders after the model chip has gone**, per the order in `spec.md:501` as amended at `70bdff80` (chip shrinks to 96 px, then goes; the name shortens only then, and is the last of the two to disappear) — the reason is in the requirement: FR-044 makes the name mandatory and the chip is Q6, so an optional element must not outlive a required one. Floor stays **292 px** (`spec.md:501`), which is what the row's fixed content costs: 276 px with the thinking marker + 16 px of centre slot. **Evidence that this is not implemented today (`webui@34e7060`):** `LiveVoiceRow.tsx` has no `agentName` prop at all (`grep -n 'agentName' app/screens/chat/LiveVoiceRow.tsx` → no match; the props are `view`/`capability`/`missing`/`elapsedSeconds`/`onStart`/`onEnd`/`onRestart`), and `LiveVoiceControl.tsx:117` renders `<LiveVoiceRow …>` without passing one. Owner of the code: `@sepuh-be`; the width/truncation values: `@teteh-desener` (her measurement: 96 px ≈ 18 characters at 10,5 px, and 0 characters fit at 292 px, so at that floor the name is absent rather than truncated). **Not ticked by T038:** that task's text promised "It shows the agent … (FR-044)" and its tick was withdrawn on 2026-11-10 for exactly this gap. **In the tree: `webui@5b5209c` (the row names the agent) + `webui@c735d82` (the 40-character test), pinned at `fe8ad451`.** The `webui@34e7060` paragraph above is the state that made the task necessary; it is history now, not a claim about HEAD. Landed 2026-11-10.
- [ ] T039 [US1] Test the FR-042 indicator on the row in `webui/app/screens/chat/LiveVoiceRow.test.tsx` and `webui/e2e/live-voice-thinking.spec.ts`: a turn that produces no audio for 4 s shows the thinking state within 100 ms of the request being accepted, its wording differs from `reconnecting`, and it clears at the first segment / `interrupted` / `ended` / `failed` (FR-042)
- [x] T040 [US1] Mount the entry control in `webui/app/screens/chat/Composer.tsx` and let the live row take over its meta line, changing neither the input nor the remaining controls; the manual speech surfaces leave in T107, not here, so this task never leaves the composer in an intermediate state. Showing and hiding the row leaves the conversation mounted, scrolled and **unmoved** — the composer's height does not change (FR-043, FR-046, FR-049) **In the tree: `webui@509e059`; not gated in AQ-001. The mount is one level deeper than this task says: `Composer.tsx` renders `LiveVoiceControl`, which renders the row.**
- [ ] T041 [US1] Expose the per-agent `live_voice` switch in `webui/app/lib/agentForm.ts` and its field in `webui/app/screens/agents/ModelsFields.tsx`, defaulting off
- [ ] T042 [US1] Keep exactly **one** counter: `backlog` is incremented and reset by T021's gate in `crates/vizier-http/src/topic.rs` and reported there as one `stall` control frame per episode (via `publish_session_event`), never by the session. The session's `ClientConnection::send` returning `false` while the connection is open is a **backstop** — `tracing::warn!` with a metric, no user-facing frame — and a test in `crates/vizier-http/tests/live_voice_delivery.rs` asserts the 64-slot queue never fills when the outbound loop runs (a healthy pair cannot produce a `Full`)

**Checkpoint**: US1 works on its own and `quickstart.md` steps 1–3 pass.

---

## Phase 4: User Story 2 — Stopping the agent by speaking (Priority: P1)

**Goal**: the person talks over the agent and it stops, before their words are transcribed.

**Independent Test**: a long answer, spoken over mid-sentence, silences the audio; the interrupting words become a message; the interrupted reply is marked interrupted.

- [x] T043 [P] [US2] VAD tests in `webui/app/lib/liveVoice/vad.test.ts` with the T004 fixtures: speech over playback triggers, playback alone at 80% volume does not (FR-026, SC-008) **In the tree: `webui@fbb204f`; not gated in AQ-001.**
- [x] T044 [P] [US2] Server test in `crates/vizier-http/tests/live_voice_barge_in.rs`, with the trigger C8 asks for: `live_voice_utterance_end` (not `speech_start`) bumps the generation, the running turn takes the existing abort path, a synthesised segment from the old generation is dropped, and an interruption that arrives while the reply was audible still lands — the test sends audio while a `live_voice_speech` frame is in flight **In the tree: `vizier@6394e7b4`; not gated in AQ-001.**
- [x] T045 [US2] Client barge-in in `webui/app/lib/liveVoice/session.ts`: the reply **keeps playing while the person speaks** (C8); when the VAD closes the utterance, call `player.stop()` and send `live_voice_utterance_end` in the same tick, so audio stops at the end of their speech and not before it (FR-019). `live_voice_speech_start` stays what it is — the signal that opens an utterance — and no longer interrupts anything **Landed `webui@0c9c89c`; the gate it names passed.**
- [x] T046 [US2] Server barge-in in `crates/vizier-core/src/live_voice/session.rs`: bump the generation, abandon synthesis, then cancel the turn through **`AgentsClient::abort(agent_id, session)`** (`crates/vizier-core/src/agents_client.rs:205-207`, impl `:298-312`) — the same `VizierRequestContent::Command("abort")` the Composer's `/abort` already sends. There is **no** `AgentCommand::Abort` (`crates/vizier-core/src/schema/commands.rs:142` lists `Create`/`Update`/`Delete`/`HealthCheck`/`AgentHealth`/`RefreshIntegrations`/`IntegrationStatus`/`Restart`); `agents_client.rs:415` is `FakeCall::Abort`, a test fake (`review.md` D12, raised by `@teteh-qa` H1) — audio first, abort second (FR-027); mark the reply interrupted **only when its `Message` had already arrived**, because an abort before any text is an `Abort` turn that produced nothing (`crates/vizier-core/src/schema/response.rs:156-166`) and the UI then shows the existing "Stopped by you" instead of an invented partial (`review.md` D1). **That mark is FR-023's**: an interrupted reply stays in the conversation as the agent's message, marked as interrupted, so a person can see what they did not hear **In the tree: `vizier@90efbb36`; not gated in AQ-001.**
- [ ] T047 [US2] Playback guard and reply ducking in `webui/app/lib/liveVoice/vad.ts` + `session.ts`: a raised gate while the reply is audible and the first 150 ms after each segment ignored, so the session does not hear itself (FR-026); and, because the agent now talks **through** the person's speech (C8), the reply's contribution to the microphone must be reduced once the person is detected speaking — the mechanism is `getUserMedia` echo cancellation plus, where that is not enough, lowering the playback gain rather than the audible level the person hears (FR-047). **The ~250 ms onset machine is gone, not idle**: `@sepuh-be` deleted `VadDecision.bargeIn`, `onsetMs` and `onsetMs_` from `webui/app/lib/liveVoice/vad.ts` (commit A of the C1 follow-up), because with the trigger at the end of the utterance it has nothing left to gate; `replyAudible` survives only as the raised gate FR-026 needs. What this task therefore adds is exactly two things and nothing else: the **150 ms ignore window after each segment starts**, and **ducking** — echo cancellation first, then lowering the playback *gain* rather than the level the person hears (FR-047, `review.md` D15 superseded by C8)
- [ ] T048 [US2] Too-short burst in `webui/app/lib/liveVoice/session.ts` and `crates/vizier-core/src/live_voice/utterance.rs`: under C8 the old rule would cancel a reply for a cough, so the precedence is fixed the other way here and must be testable. **A sub-word burst neither cancels the reply nor becomes a message** (FR-024, Story 2 scenario 6): the length that decides is the one that makes a barge-in, measured as voiced energy, never a character count; a transcript that is empty after recognition is `unintelligible` (FR-031), not `too_short` (`analysis.md` M11, re-derived for C8)
- [ ] T049 [US2] End-to-end check in `webui/e2e/live-voice-barge-in.spec.ts` with a faked provider: audio stops, the correction is submitted, the interrupted reply keeps its mark
- [x] T050 [US2] Confirm stale-generation frames are dropped by the client on arrival in `webui/app/lib/liveVoice/player.ts`, with a unit test in `webui/app/lib/liveVoice/session.test.ts` **In the tree: `webui@0c9c89c`; not gated in AQ-001.**

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

- [ ] T051 [BACKLOG] [P] [US3] Server test in `crates/vizier-http/tests/live_voice_interrupt.rs`: the interrupt frame bumps the generation, submits nothing, and leaves the session live (FR-020, FR-021)
- [ ] T052 [BACKLOG] [US3] Add an always-visible interrupt control to **the live row** (`webui/app/screens/chat/LiveVoiceRow.tsx`, the surface of FR-043/FR-044), distinct from the end action of FR-045. **The three surfaces this task was written against are all archived** — the composer bar (C6), the full call screen (C9) and the floating panel (C11) — so the control goes on the row and nowhere else
- [ ] T053 [BACKLOG] [US3] Put the interrupt control on **the surface this release ships** — the live row, `webui/app/screens/chat/LiveVoiceRow.tsx` (C11; the panel of C9 is archived and the User chose the row on 2026-11-09 16:38) — and hand it the client call that already exists. **The client path is not missing**: `Session.interrupt(reason)` (`webui/app/lib/liveVoice/session.ts:149`) already stops the audio, sends `live_voice_interrupt`, and moves `speaking|waiting → listening`, with a passing test (`webui/app/lib/liveVoice/session.test.ts:110`). What is genuinely absent is the control: the only renderer today is `webui/app/screens/chat/LiveVoiceBar.tsx:76` fed by `LiveVoiceControl.tsx:80`, and that bar is archived (C6/C9/C11) while FR-045 lets the row carry **one** action, End, until this item lands. Also make the `interrupted → listening` hand-back explicit rather than assumed, since today it happens inside the same call (SC-003)
- [ ] T054 [BACKLOG] [US3] Typing during a session in `webui/app/screens/chat/Composer.tsx`: the send path first sends `live_voice_interrupt { reason: "typed" }` so the audio stops like the button (FR-025), then sends the text as an **ordinary chat request on the same socket** — not through the session, and the session gains no frame for typed text. Its reply is text only (FR-018 withdrawn, 2026-11-08). With no session live the composer behaves exactly as today (FR-040)
- [ ] T055 [BACKLOG] [US3] Interrupt while nothing is being spoken is a no-op on state in `crates/vizier-core/src/live_voice/session.rs` (FR-021)
- [ ] T056 [BACKLOG] [US3] End-to-end check in `webui/e2e/live-voice-interrupt.spec.ts`: one press silences the agent, the session stays live and listening, and a typed message is answered **as text with no audio scheduled** — assert the `AudioBufferSourceNode` count stays zero after the typed turn (FR-025, FR-018 withdrawn)
- [ ] T057 [BACKLOG] [US3] Keep the interrupt control reachable without hiding it inside a live session — disabled with a reason only when the capability is false (FR-020). **Placed on the live row (C11)**, not on the archived composer bar (C6), the retired call screen (C9) or the retired floating panel (C11)

**Checkpoint**: not part of this release. The only interruption path in scope is US2's (speaking). US3 is deferred (C7) and its tasks must not be implemented or tested now.

---

## Phase 6: User Story 4 — A session never keeps the microphone (Priority: P2)

**Goal**: every ending releases the microphone and leaves the text.

**Independent Test**: close the tab mid-answer; the OS microphone indicator goes out within 2 s and the reply text is kept; a silent session ends after the idle period with a reason.

- [x] T058 [P] [US4] Tests in `webui/app/lib/liveVoice/session.test.ts` and `crates/vizier-http/tests/live_voice_session.rs`: page close releases tracks, idle stop ends with a reason, a second start on the same conversation is refused **Not ticked yet, though `3bf44a7` landed most of it**: the client release tests and the second-start server test (`live_voice_session.rs:68`) are green, but this task also names *idle stop ends with a reason*, and the idle timer is T062 — still open. Ticking it now would claim a test that cannot pass. **In the tree: `webui@3bf44a7`; not gated in AQ-001.**
- [x] T059 [US4] One release path in `webui/app/lib/liveVoice/session.ts` — stop every track, close the `AudioContext`, drop the queue — called by every terminal transition **Landed `webui@3bf44a7`; the gate it names passed.**
- [x] T060 [US4] Wire `pagehide`/unmount handling in `webui/app/routes/chat.tsx` and `webui/app/screens/chat/Composer.tsx` so leaving the conversation ends the session (FR-028) **In the tree: `webui@509e059`; not gated in AQ-001.**
- [ ] T061 [US4] Refuse a second connection pressing start on a conversation that already has a call, in `crates/vizier-core/src/live_voice/session.rs`, with `live_voice_error` `kind: "session_active"` (contract, `live_voice_error`; FR-005, Q2's default) — **and show it in `webui/app/screens/chat/LiveVoiceRow.tsx`**. **Today the server refuses it with `kind: provider_failed` and the message "a live voice session is already active on this conversation"** (`crates/vizier-http/src/topic.rs:775-776`), which the client renders as *"the speech provider failed"* (`webui/app/lib/liveVoice/session.ts:566-567`) — the same defect T102 fixed for `unsupported_audio`, and for the same reason: a person sent to check a provider that never failed. So this task also **adds `EndReason::SessionActive`** (`frame.rs`, no variant exists today) and a client sentence that says the call is already open here. Found while auditing the state matrix, 2026-11-10. One term, one meaning: a start **elsewhere** must move the call (FR-050) and must never answer with this kind
- [ ] T062 [US4] Idle stop and owner-gone ends in `crates/vizier-core/src/live_voice/session.rs`: idle timer (FR-029), agent reconfigured/deleted, conversation deleted, server restart
- [ ] T063 [US4] Ending leaves the control off and remembers nothing between visits in `webui/app/screens/chat/LiveVoiceButton.tsx` (no storage)
- [ ] T064 [US4] End-to-end check in `webui/e2e/live-voice-lifecycle.spec.ts`: close mid-answer, track `readyState` becomes `ended`, text kept, control off on reload — **and FR-034's second half**: a reply in flight when the session ends finishes as **text** and is never spoken, so ending a call cannot swallow an answer that was already paid for
- [ ] T065 [US4] Mobile/desktop suspension (screen lock, WebView background) ends the session with a reason rather than staying "live" in `webui/app/lib/liveVoice/session.ts`

**Checkpoint**: SC-007 holds; no code path can leave listening.

---

## Phase 7: User Story 5 — A broken session explains itself (Priority: P2)

**Goal**: every failure is a visible reason within 5 s, never a session that looks live.

**Independent Test**: cut the connection mid-answer; the state reads reconnecting within 2 s and the audio stops; restore within 30 s and the same session continues without a duplicate message.

- [ ] T066 [P] [US5] Tests in `crates/vizier-http/tests/live_voice_failures.rs`: empty recognition, voice provider failing mid-reply, socket drop then reconnect inside the window, no device (FR-030–FR-033)
- [x] T067 [US5] Reconnect in `webui/app/lib/liveVoice/session.ts` and `crates/vizier-core/src/live_voice/session.rs`: resume the same session within 30 s, never re-submit a message already in flight **Landed `webui@cc1d648`; the gate it names passed.**
- [x] T068 [US5] Re-bind the audio lane on reconnect in `crates/vizier-core/src/live_voice/session.rs`: `TopicConnection::open` builds a **new** `ClientConnection` per socket (`crates/vizier-http/src/topic.rs:144`), so a session that kept the old `Arc` would stay live and go mute. A `live_voice_start` from the owner while their session is live is idempotent and adopts the caller's current connection; a session whose owner never comes back inside FR-033's 30 s ends with `connection_lost`. Test in `crates/vizier-http/tests/live_voice_failures.rs`: drop the socket, reconnect, the next `live_voice_speech` reaches the **new** connection (`review.md` D13) **Landed `vizier@c15916b5`; the gate it names passed.**
- [x] T069 [US5] Failure surfaces in `webui/app/screens/chat/LiveVoiceRow.tsx`: one reason line within 5 s and the microphone released (SC-009, FR-032) **In the tree: `webui@70437fb`; not gated in AQ-001.**
- [x] T070 [US5] Voice provider failure mid-reply in `crates/vizier-core/src/live_voice/engines.rs`: the rest of the reply still appears as text, the failure is reported, the session listens again (FR-032) **In the tree: `vizier@72482d4c`; not gated in AQ-001.**
- [x] T071 [US5] No device and refused permission in `webui/app/lib/liveVoice/capture.ts`: fail with a reason, retryable without a reload (FR-030) **In the tree: `webui@c465842`; not gated in AQ-001.**
- [x] T072 [US5] End-to-end check in `webui/e2e/live-voice-failures.spec.ts` using route interception to drop and restore the socket **In the tree: `webui@c465842`; not gated in AQ-001.**
- [x] T073 [US5] Unintelligible transcript in `crates/vizier-core/src/live_voice/utterance.rs`: tell the person, send nothing, listen again (FR-031) **In the tree: `vizier@f7499c2a`; not gated in AQ-001.**

**Checkpoint**: every failure path in SC-009 is demonstrated.

---

## Phase 8: User Story 6 — The call floats over the conversation (Priority: P1, C9)

**Goal**: pressing the control opens a **draggable panel over the conversation that is already on
screen** — the agent, its state, the thinking indicator, the open microphone, a header to drag it by
and one way out — and the chat behind it stays readable, scrollable and unchanged.

**Independent Test**: from a conversation, press the control; a panel appears over the conversation and
can be dragged by its header without leaving it; ask something and watch the thinking state; scroll the
chat behind it; end the call; the panel is gone, both turns present, the control off.

- [x] T089 [P] [US6] Mount the row in `webui/app/routes/chat.tsx` so it **is** the composer's meta line — not an overlay, not a route, not a rail above the composer, and nothing left behind when the session ends (FR-043, FR-049; C11 as amended 17:31, surface B1-1) **In the tree: `webui@509e059`; not gated in AQ-001. The mount is one level deeper than this task says: `Composer.tsx` renders `LiveVoiceControl`, which renders the row.**
- [x] T090 [US6] Release on leaving in `webui/app/screens/chat/LiveVoiceRow.tsx` and `webui/app/lib/liveVoice/session.ts`: hiding the row by any means runs the one release path of T059 — every track stopped, the media peer closed, `AudioContext` closed, queue dropped (FR-028, FR-045) **In the tree: `webui@c06185f`; not gated in AQ-001.**
- [x] T091 [P] [US6] Assert FR-046 in `webui/e2e/live-voice-row.spec.ts`: a session with three turns, shown and ended, leaves the conversation's messages, order and timestamps identical to a session that was never opened **In the tree: `webui@c06185f`; not gated in AQ-001.**
- [x] T092 [US6] Script SC-010 in `webui/e2e/live-voice-row.spec.ts`: pressing the control shows an interactive row within 500 ms on the first attempt, on a fresh profile, with microphone permission granted at the system prompt (SC-005's clock stops here, not at the first answer) **In the tree: `webui@c06185f`; not gated in AQ-001.**
- [ ] T093 **[SUPERSEDED by C11]** — dragging, clamping and remembering a position were the panel's rules (FR-048, Q9). A row has no position, so this task retires with them. The panel is still the fallback if the row is rejected, and then this task returns unchanged
- [x] T094 [P] [US6] Assert FR-049 in `webui/e2e/live-voice-row.spec.ts`: with the row shown, **no message is covered or clipped**, the conversation scrolls to its own end, the composer keeps its height, and as the pane narrows the row drops its own detail **in the stated order and no other** — model chip to its 96 px floor and then gone, then the waveform 39 → 12 bars and gone, then the context ring whole — while the microphone indicator, the state word, the elapsed time and the End action stay readable and unwrapped down to the **292 px** floor, with no ellipsis in the state word (FR-038, FR-042) **In the tree: `webui@c06185f`; not gated in AQ-001.**
- [ ] T095 [US6] One session in total, in the registry that holds `LiveVoiceSessions` (`crates/vizier-core/src/live_voice/session.rs:430` today): **keep the existing `VizierSession` key** — inbound frames are routed by it and deleting it breaks delivery — and **add a second index `owner → Arc<LiveVoiceSession>`** for the authority FR-005/FR-050 now need. Two lookups, two jobs: the map answers "which session is this frame for", the owner index answers "does this person already have one". Starting a call while one is live ends the old one through the normal release path and adopts the new conversation; a test asserts exactly one live session after a move, that the microphone is open once, and that a frame for the old conversation still resolves to nothing rather than to the new session (FR-005, FR-050, SC-011; `architecture.md` §3)
- [ ] T096 [US6] The reason left behind, in `crates/vizier-core/src/live_voice/session.rs` and `webui/app/lib/liveVoice/session.ts`: a session ended by a move carries its own reason (`moved`), the old panel shows it until it disappears, and nothing is written into the old conversation's history (FR-051, open question Q10's default)
- [ ] T097 [US6] A turn at the moment of a move, in `crates/vizier-core/src/live_voice/session.rs`: the running turn is cancelled on the existing abort path with its partial text kept, its reply is never spoken into the new call, and a test proves no segment from the old session reaches the new panel (FR-052, SC-011)

- [x] T098 [P] Tokens before the panel, in `design/src/tokens/tokens.css`: add `--vz-retrying-text` (`#8a560e`, measured **In the tree: `design@9130d03`; not gated in AQ-001.**
  5.59:1 on `bg/surface`, 5.61:1 on `retrying-bg`, 6.14:1 on white; the existing `--vz-retrying` `#a86a12` fails all
  three at 4.03:1, 4.05:1 and 4.43:1, below the 4.5 threshold for small text) and pin the dark failed state to the
  library pair (`--vz-failed` `#e5654b`, `--vz-failed-text` `#f0a394`) instead of the render's `#f87171`. **Do not
  replace `#f87171` globally**: it is legitimately the generative-UI fallback palette
  (`webui/app/lib/pageDocument.ts:67`), and only the design palette's failed state moves. Gate: the ratio asserted in a
  test, not checked by eye — `design/vitest.config.ts` is already wired to `npm test` in `design/`, so add
  `design/src/tokens/tokens.test.ts` computing the contrast from the two hex values, and run
  `cd design && npm test` together with the existing `node scripts/lint-tokens.mjs`

**Checkpoint**: a person can hold a conversation without ever seeing the chat layout, and can leave it without losing a word of it.

---

## Phase 9: Polish & Cross-Cutting Concerns

**Purpose**: the documentation, the measurement, and the gates the constitution asks for.

- [x] T074 [P] Write the user documentation page `docs/src/configuration/live-voice.md` (how to start, move and end a call; the live row that takes over the composer's meta line and what each of its parts means — **not a floating panel: that surface was retired by C11 and nothing is draggable**; that only one call exists at a time; what audio goes to which provider; what is stored; how to interrupt by speaking **and that there is no interrupt button in this release** — spec C7 — plus the limits) and add it to `docs/src/SUMMARY.md` (FR-039) **Written from the code at `305fc838`**: `docs/src/configuration/live-voice.md` (2.23) plus its `SUMMARY.md` entry. Every claim in it was read off `LiveVoiceRow.tsx`, `session.ts`, `metrics.ts`, `agent.rs` and the flags — not off a sheet. **In the tree: `vizier@5eba4226`; not gated in AQ-001.**
- [x] T075 [P] Add the build-flavour note for `provider-live-voice` beside the other optional features in `docs/src/deployment/` (whichever page lists the `full` build) **Written from the flags at `305fc838`**: the `provider-live-voice` row in `docs/src/configuration/build-flavours.md` (the page that lists `full`), plus the note that the gateway's `live-voice` feature is what makes the socket understand the frames. **In the tree: `vizier@5eba4226`; not gated in AQ-001.**
- [x] T076 Define the **two** measured gates (SC-001, SC-002) before anything is measured, in `webui/app/lib/liveVoice/session.ts` (marks) and `crates/vizier-core/src/live_voice/session.rs` (spans): **SC-001** = `performance.now()` at the client's end-of-utterance (hangover fired) to the **first sample that is actually audible**, computed as the `AudioBufferSourceNode.start()` timestamp plus the `AudioContext.baseLatency + outputLatency` (or the scheduled `when`, if the first segment was queued ahead) — never the frame's arrival at the socket, and never a server span, or the gate measures a different event than "first audible word"; **SC-002** = the same end-of-utterance mark to `player.stop()` returning (client) and to `Abort` reaching the agent (server span); **and no third mark**: SC-003's interrupt control is deferred with C7, so instrumenting it now would measure a control that does not exist. Emit one `live_voice_metric` line per turn with the stage durations, and write the procedure into `specs/055-live-voice/latency.md` — without fixed marks the numbers in the plan are not a gate (`review.md` D11) **In the tree: `webui@5607677`; not gated in AQ-001.**
- [ ] T077 Instrument the stages in `crates/vizier-core/src/live_voice/session.rs` (end of utterance, recognition, turn start, first segment, first frame) with `tracing` spans and record p50/p90/p99 in `specs/055-live-voice/latency.md` **Half done, deliberately not ticked**: the client half (marks, recorder, `specs/055-live-voice/latency.md`) landed in `5607677`; the server half — the `tracing` spans in this task — is not written, and `latency.md` says so in a section of its own.
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
- [ ] T111 [PROVISIONAL] **The reply as it is made** (FR-065 to FR-070, SC-014): make a turn deliver its reply in segments instead of one finished message, and let the first reach the client while later ones are still produced. The finished message MUST still arrive at the end and the conversation MUST keep exactly one copy (FR-069), so this is an addition on the way to the same end state, and the new frames MUST be published as a versioned contract change (FR-070) because that contract is public. The pieces, in the order that keeps the tree buildable: (a) a frame shape for a segment of *text* alongside the existing `live_voice_speech` segment, so text and speech arrive on one clock; (b) the turn path stops waiting for `VizierResponseContent::Message` before it speaks anything (`crates/vizier-core/src/live_voice/session.rs:417`); (c) interruption abandons segments **in flight**, not only the queue (FR-066); (d) the stored reply is the reply that was made, once (FR-067); (e) a mid-reply ending says where it stopped (FR-068). **Not part of this release's gate**: it waits on the Lead's assessment **only** — Q12 was answered on 2026-11-10 06:39 (`spec.md:694`, and again 09:37), so the person's answer is no longer a waiting condition and must not be asked again — and **no latency number is attached to it** until that assessment exists. **Do not start without that assessment** — the point of writing it down is that it is testable the moment it is chosen, not that it is scheduled.

---

## Defects found in the field, 2026-11-10 16:33 (after this release)

Three defects were found by a live call in a real browser on `:9998`, agent `bestie`, provider
`Openrouter` with `stt_from_live_voice=true tts_from_live_voice=true`. None of them is in the
provider or in the person's configuration: the first is the line that made the call silent, and
the other two are what the log shows once that line is read closely. **All three are out of this
release's gate** (bro-pm, 2026-11-10 16:50); they are written down so they cannot be lost, not to
be started.

- [ ] LV-TTS-PCM-FORMAT **A speech model that answers only in PCM was asked for `mp3`.** Every
  synthesis of the call came back `400 {"error":{"message":"Gemini TTS only supports
  response_format=\"pcm\". Got \"mp3\"."}}` — filler 4× and the answer 1×, `segment sent` 0 — so the
  reply reached the thread and never the speaker. `tts/mod.rs` names `mp3` for every OpenRouter
  model without looking at which model it is. **Fix in the tree, not yet gated** (this is the only
  one of the three that is being fixed now): the request names `pcm` for a model that requires it,
  the bare samples are wrapped at the rate everything else reads, and a non-PCM model keeps its mp3
  path unchanged. **Not covered by that fix, and it must be said in the release notes:** a PCM-only
  model whose name does not match the detection pattern still gets `mp3` and still fails, and the
  assumption that the body is bare PCM is only provable by one real turn.
- [ ] LV-ST-SPEAKING-263MS **`speaking` reaches the client and dies in a quarter of a second.**
  `frame.rs` publishes the word at 16:33:21.518 and `listening` at 16:33:21.781 — 263 ms. The bar
  renders it (`LiveVoiceRow.tsx:71-72`), so the person sees `listening → thinking → listening` and
  never a `speaking` they could name. The defect is **not** a missing transition: it is that a
  failed voice has no word of its own. The client receives `speaking → error → listening`, and the
  error's message is replaced by the generic sentence `the speech provider failed`
  (`webui/app/lib/liveVoice/session.ts:713-714`), which is the same defect T102 fixed for
  `unsupported_audio`. Needs a decision: a state or notice that means "the words are here, the
  voice is not", and the provider's own message kept on screen long enough to be read.
- [ ] LV-SECOND-UTTERANCE-DROPPED **After an `error` frame the client stops opening utterances.**
  In the 63 s between `synthesis failed` (16:33:21.781) and the last line of the log (16:34:26.399):
  927 `push_audio`, **0** `open_utterance`, **0** `turn_start`. The audio keeps arriving and the
  server drops every chunk, because `push_audio` only appends to an utterance that is already open
  (`crates/vizier-core/src/live_voice/session.rs:412-414`) and only a client `speech_start` opens one
  (`crates/vizier-http/src/topic.rs:947`). The person spoke again and nothing was submitted. The
  mechanism QA pointed at: `webui/app/lib/liveVoice/session.ts:437` invents an `utteranceId` without
  sending `speech_start`, and `:455` only sends it while `!utteranceId` — so the id is non-zero,
  the frame never leaves, and the gate never re-opens. **Silent by contract:** an unknown id returns
  `None`/`Unknown` and neither path sends a frame (`topic.rs:909`, `:924`), so the bar shows
  `listening` with a flat waveform for both "heard and working" and "dropped".
- [ ] LV-SESS-NO-END **A live-voice call ends without a line saying so.** `start` happened three
  times on one topic — 16:31:18, 16:31:48, 16:32:50, each `engines chosen` for `agent_id=bestie` —
  and not one of them left a line that the previous call had ended; each restart coincides with a
  17 s and a 13 s hole in the audio. So "why three sockets in 92 seconds" cannot be answered from
  the log. Not a release blocker (the call that was tested was the third one and it worked), but it
  costs every future report the same hour this one cost.

- [ ] LV-BARGE-BEFORE-AUDIO **The first turn of every call is killed before it makes a sound.**
  `close_utterance` while the turn is still `busy` calls `abort_turn(BargeIn)`
  (`crates/vizier-core/src/live_voice/session.rs:457`), which moves the generation, and the answer is
  discarded before it is ever synthesised. Seen on three calls in a row (20:08, 20:09, 20:12 on
  `:9998`): `turn_start` -> `open_utterance` **0.4 ms later** -> `close_utterance` -> `interrupted
  cancelled=true generation=3` -> `reply abandoned … at="the synthesis finished after the reply was
  over"`, with zero `segment sent`. The person sees "the first answer has no voice" and every answer
  after it is fine, because later turns are no longer brand new when their `close` arrives.
**THE RULE — one condition, and this block is the whole of it.**
```
abort_turn  ONLY when  ratio = audio_ms / wall_ms < 5.0
  audio_ms  from the PCM actually received (len / 32 for 16 kHz mono 16-bit), never chunks x 100
  wall_ms   in microseconds (or f64): three measured bursts are 241, 584 and 668 us, so whole
            milliseconds would round two of them to 0
  wall_ms == 0  -> held, never infinity (division by zero is not a theoretical case here)
  audio_ms == 0 -> held too, reason=no-audio: no audio is not a low ratio
every decision logs: decision=abort|held, reason=<burst|no-audio>, ratio, audio_ms, wall_ms, topic,
        utterance_id,
        sequence=<which turn of this session> - so a report of "my interruption was refused" can be
        matched to a turn without guessing from the clock; `sequence` rides the warn line only, not
        the grey-zone debug line
levels: held at warn (the only branch that can make a person say "my interruption was refused", so it
        must not depend on a debug filter); the 2.0-5.0 grey zone at debug with decision=
```
**A second condition on the utterance's origin was chosen and then withdrawn — do not put it back.**
Between 21:02 and 21:10 the guard briefly required `opened_at > turn_started_at` as well. It was
withdrawn for a measured reason, not a preference: it buys nothing the ratio does not already buy
(every burst in the log is 9.8x or above, so the ratio holds all of them), and it *creates* a failure
it was supposed to prevent — an utterance opened during the 3.4 s between `close_utterance` and
`turn_start` has `opened_at < turn_started_at`, so its close could never abort, and a person talking
while the agent was still thinking could not cut the answer at all. Under the ratio alone that same
utterance measures about 1.0x and aborts as it should. `opened_at` survives as a **diagnostic
(`opened_before_turn=true|false`)**, never as a gate.
The sample count settled it too: **the origin condition was never exercised by a single case** - every
burst in the log sits at 9.8x or above, where the ratio already holds it. So `opened_at` is **not a
prerequisite for tomorrow's work**: no `opened_at` on `Utterance`, no `turn_started_at` on the session,
and **no `reason=opened-before-turn`**; the reasons this guard emits are `burst` and `no-audio` only. A
diagnostic origin can be added later as a plain log field, never as a gate.
**Why the ratio and not a duration or "has it spoken yet":** measured over 19 utterances on
20:08-20:13, audio duration / wall-clock duration is **1.0x** for a person actually speaking and
**1 370x, 3 320x, 4 641x** for audio that arrived in one dump (tens of microseconds of wall clock
carrying 800-3 100 ms of audio). A duration threshold that drops the dump also drops the real 805 ms
interruption; `segment sent >= 1` would disable the case where the person really did interrupt but the
audio had not been sent yet. What separates them is **how fast the audio arrived**, not how long it was.
**Why 5.0:** all 16 real utterances measured about 1.0x, so 5.0 keeps a 5x margin above genuine speech
and only silences what is physically impossible - a person cannot speak five times faster than time.
**Why not 3.0:** the two thresholds disagree only in 3.0-5.0, and there 3.0 would hold back an
interruption that might be genuine while 5.0 lets it through, which is today's behaviour. So 5.0
regresses nothing; anything lower is a policy that stops people interrupting, which FR-024/FR-027 never
agreed to. **Why not 100 ("safely in the middle"):** 9.8x is also impossible for a human voice - 3 000 ms
of audio cannot arrive inside 305 ms - and an upper threshold would let it through and keep killing the
first answer of every call, which is the defect this task exists to remove.
5.0 is a **named constant** with that reasoning beside it, never a literal inside a function.
**Do NOT build the guard on the utterance's origin - it was tried and the log killed it.** The order of
`open_utterance` and `turn_start` is a race, not a meaning:
- `20:08:14` `open` .818824 -> `turn_start` .818945 (**121 us** apart) -> `cancelled=true`
- `20:09:25` `turn_start` .700115 -> `open` .700219 (**104 us** apart) -> `cancelled=true`
- `20:11:48` `turn_start` 20:11:26.485673 -> `open` 20:11:48.389813 (**21 s** apart) -> `cancelled=false`
Origin as the gate leaves the 20:09:25 kill alive (ordering says "abort a 3 320x burst") and would
wrongly abort the 20:11:48 burst. The ratio holds all three. That is measured, not preferred.
**It is a net, not a cure.** The cause that **is** proven: the socket is not read while recognition runs
inline (`crates/vizier-http/src/topic.rs:912`, `:936`; the 16:33 call carried `idle_for_ms=11768` and
then 117 chunks in one second). "The client sent a held-then-dumped burst" is **not** proven - a stalled
server read produces identical timestamps, and only a client-side log could tell them apart. Moving
`transcribe` out of the socket loop costs little and saves **0 ms** of latency, but it removes the cause
rather than the symptom; it touches the `touch`/`busy` path that keeps FR-029's Option A honest, so **Q5
(a silent session ends by itself) is part of this gate.** It must not be used to declare this task
finished: the DoD stays that the first turn of a session must not die before it makes a sound, and the
test must be **red** without the guard.
**What this does NOT close, stated as a price rather than hidden:**
- A **legitimate** interruption arriving while the socket is stalled reads a high ratio and is **held**:
  the agent keeps talking until it finishes. That disappears only when `transcribe` leaves the loop.
- An utterance opened **inside the window between `close_utterance` and `turn_start`** (20:13:14 ->
  20:13:18, 3.4 s) is judged on its ratio alone, which is about **1.0x**, so its close **does abort** a
  turn that has not yet made a sound. No sample of it exists in tonight's log, it is **not a regression**
  (every `close` while `busy` aborts today), and it closes with the same move as the cause above.
- An utterance shorter than about 300 ms makes the ratio noisy and stays on the server's `too_short`
  path, not this one.
- **Held is not discarded:** a held burst is still transcribed and can still become the person's next
  message (20:10:05 held, then `turn_start` 20:10:09.332 and `segment sent` 20:10:15.851). Whether such
  audio is a real utterance that was delayed or audio that should never have been sent is **not proven**;
  only a client-side log settles it.
- `channels != 1`, PCM-only models outside the name pattern, and the 263 ms `speaking` are untouched.
**Gate, red first:** 20:09:25 (3 320x) **held** - the red case, it is the reported defect · 20:12:32
(1.0x) **abort**, the real interruption must keep working · 20:08:14 (1 370x) **held** · `wall_ms == 0`
**held** · `audio_ms == 0` **held** · Q5 green.

- [ ] LV-EMPTY-REPLY-SILENT **A reply with no content passes as a turn.** Four of the eight turns on
  topic `cEKjwtlxZ7` produced `content: ""` — no text, no speech, and no word to the person that
  anything went wrong. An empty answer must become a named error, in the same spirit as the JSON-body
  guard in the TTS path: a failure that says what it is beats a turn that is simply silent.
- [ ] LV-SESSION-MUTED-AFTER-RESTART **A new call inherits the previous call's silence.**
  `start()` never resets `generation` or `mutedGeneration` (`webui/app/lib/liveVoice/session.ts:201`),
  so a call that ended with `mutedGeneration = 2` opens the next one already muted; the server starts
  its generation at 1 (`crates/vizier-core/src/live_voice/session.rs:115`) and the first reply, at
  generation 2, is refused by `if (frame.generation <= this.mutedGeneration) return`
  (`session.ts:323`) — every byte thrown away and the state frames dropped at `:283` too, so the bar
  does not move either. Evidence: 20:12 — the server sent **144 640 + 72 960 bytes** and nothing was
  heard. `mutedGeneration` compares the *server's* generation with the one this client silenced, and
  two sessions reuse the same numbers, so it only means anything inside one session; the reset belongs
  in `start()` with that reason written beside it.

- [ ] LV-LATENCY-FIRST-AUDIO **The wait to the first word is still felt on an online provider.**
  Measured on `:9998`, `bestie`, `Openrouter` + `google/gemini-3.1-flash-tts-preview`, two calls
  after the PCM fix: **9,46 s** and **9,32 s** from the end of the utterance to the first audible
  segment. That is a large improvement on the **22,5 s** measured before it (14:39), and the person
  still feels it (2026-11-10 17:46, "latency nya masih lumayan kerasa", explicitly **not urgent**).
  Three causes, each measured rather than guessed:
  - **The voice is not streamed.** `live_voice/session.rs` synthesises only after
    `VizierResponseContent::Message` — the whole answer exists before segment 1 is asked for, so the
    first audio pays the full completion time. This is the largest of the three, and closing it is a
    contract change (`LV-TTS-PCM-FORMAT`'s sibling, not a tweak).
  - **Recognition costs 4,19 s / 5,09 s** and it blocks the socket read: the 16:33 call carried
    `idle_for_ms=11768` on `turn_start`, and 117 chunks arrived in the following second.
  - **The turn waits in the agent queue** — `vizier.queue.wait_ms=0` in one call, **5 085 ms** in the
    other, **14 881 ms** at 16:33. That is the queue, not the voice.
  Do not start before `LV-SECOND-UTTERANCE-DROPPED` and the `mutedGeneration` reset are closed: a
  faster first answer the client then refuses to play is worse than the wait. **No new number is
  promised to the person until it is measured**, and the three causes must be attributed separately
  when it is.
---

## Latency iteration — what is approved, what is not (2026-11-10, `@bro-pm` ruling)

Written down because a decision that lives only in the chat is not a decision. **Do not widen any of
these without a new ruling.** The three defects above are the order of work; this section is what is
allowed *after* them, and it is not a schedule.

- **`STT partial, form A` — APPROVED.** Begin transcribing before the utterance closes, but still
  submit only when it closes. FR-010/FR-011 stay intact and Q12 is untouched: it is engineering, not
  a contract change. Carrying task must cite this line.
- **`STT partial, form B` — NOT APPROVED.** Submitting a hypothesis before the utterance closes was
  never approved and must not be started on the grounds that "partial was agreed". It changes FR-011
  (`spec.md:515`, nothing may be added to the conversation on the person's behalf) because a revised
  transcript can put words in their mouth that they never said, and it touches Q12. That is a product
  decision, not an optimisation.
- **`Hangover 800 -> 250 ms` — NOT A DECISION.** 800 ms is FR-010's *default* (`spec.md:514`); nothing
  approved changing it. It is also **not** a latency saving in the criterion: SC-001's mark starts when
  the hangover fires (`latency.md:16`), so shortening it moves both ends together and leaves the figure
  unchanged. What it improves is the **felt** delay from the person stopping to the first word, about
  **550 ms**, and it must be reported that way and never as "SC-001 improved".
- **`TTS segment 1 = first sentence` — OWED, not new scope.** FR-013 (`spec.md:517`) already requires
  speech to begin at the first speakable segment and not to wait for the reply's last sentence; today
  the turn waits for the whole message. Paying that debt is a delivery of FR-013 through FR-065/FR-070,
  with the frame change published as a contract change (FR-070). It is **not** a latency feature.
- **`Level 2 (speculative turn)` — NOT DECIDED HERE.** It goes to the person as one sentence, with the
  risk named: a speculative turn can act before it was asked. The Lead and the PM do not decide it in
  a document.
- **`Replacing the TTS model` — NOT DECIDED HERE.** It changes the agent's voice, so it is the person's
  call, not the team's.

### The floor, measured rather than assumed

Twelve `synthesis started -> segment sent` pairs across two binaries fit `ms ≈ 2329 + 474 × audio
seconds`; the shortest segment ever measured (1.76 s of audio, 21 characters) still cost **3 157 ms**.
So the first spoken segment costs **~3.2 s with this provider no matter how short the reply is**, and a
three-second end-to-end target is unreachable without replacing the voice. Budget of the components
from the 20:12 session on `:9998` (`8f159f2b`), 88 + 82 characters, **14 591 ms** to the first word:
recognition 4 024 · agent queue 30 · model 4 279 · first segment 6 256. No number may be quoted to the
person before the harness below exists, and the sample taken today is **not** a clean sample because
`LV-BARGE-BEFORE-AUDIO` discards the first turn of every call.

### Measurement gates before any number is quoted

`latency.md` records that no minimum sample count was ever agreed. It is agreed now: **3 turns is a
smoke check, 20 is a provisional figure, 50 or more is a figure that may be quoted**; p90/p99 are not
used as a gate below 50, and below 50 a median and a range are reported instead.


---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies
- **Foundational (Phase 2)**: needs Setup; **blocks every story**
- **Audio path and third-party access (Phase 2b)**: needs Phase 2, and **runs after Phases 3, 4, 6 and 7** — T103 edits `capture.ts`/`session.ts` (T033/T035) and T110 needs T059 and T097, so running it earlier edits files that do not exist. **T099 then runs as a record, not a gate** — the carrier is settled (Q11: `ws-json-pcm16`), and T100–T104 build the socket path that already exists plus the door in front of it. **T109 and T110** add third-party access: a scoped key, and its revocation ending a running session. Its ids (T099–T110) are larger than US6's; the phase is the order, not the id
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
3. Stop and validate with `quickstart.md` steps 1–3, then demo. The demo is the **floating panel** (US6), not a composer bar and not a full screen.

### Incremental delivery

US1 → demo (talk and hear) → **US6 (the floating panel)** → US2 (barge-in, now triggered by the end of the
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
and up by one, and again by two for SC-005/FR-008). **The id is not the execution order** — the phase
is. US6 was appended as T089–T097 although it is built **before** Polish, which keeps T074–T088, so
Phase 9 must be executed **last** even though its ids are the smallest of the two blocks; anyone
following the numbers instead of the phases would write the docs and run the closing gates before the
panel exists. Total at this delta: 88 tasks, T001–T088 (`analysis.md` M12 caught the stale 84 here and
the stale phase count below).

**Why this delta exists**: `review.md` re-checked every citation in `research.md`/`plan.md` against the
tree. Seven findings change what a task must say; `architecture.md` is the shape they take.

**Delta 2026-11-09 16:07 (a row, a retirement, a media contract — spec C11/C12/C13/C14).** 98 → **108**
tasks (T001–T108), nothing renumbered: **ten appended** (T099–T108) into a new **Phase 2b**, which was
placed after Phase 2 and before Phase 3 — the ids are larger than US6's on purpose, since the phase is
the order and not the id. **Corrected 2026-11-09 17:49: that placement was wrong.** Phase 2b runs after
Phases 3, 4, 6 and 7, because T103 edits files born in T033/T035 and T110 needs T059/T097; in the
original order it edits files that do not exist and the tree stops compiling in between. Changed in place: **T015, T020, T021** keep their content and gain
**socket path that ships** — they are the path that exists and is tested; **T022** keeps its invariant and
its behaviour; **T033** asks for the three `getUserMedia` constraints explicitly and presumes nothing
else; **T037, T038, T040, T089, T090, T091, T092, T094** now describe the row that takes over the
composer's meta line rather than a panel over it (amended 17:31: surface **B1-1**, 0 px added); **T093**
alone carries **[SUPERSEDED by C11]**. New: **T099** record the carrier Q11
settled and the numbers that settled it, **T100** the door on the socket — identity and a refusal before
any state exists, **T101** bind the audio to one session and refuse the rest, **T102** the audio
description, **T103** the client sends 100 ms PCM16 chunks (the shipped path), **T104** refuse the
reserved `media-track`, **T105** publish the contract, **T106** a third-party conformance test for
SC-012, **T107** retire the manual speech **surfaces** while the code stays, **T108** prove FR-041 got
harder rather than narrower.

**Delta 2026-11-09 16:35 (Q11 closed on the socket; a scoped API key for outsiders — spec C15).** 108 →
**110** tasks (T001–T110), nothing renumbered: **two appended** at the end of Phase 2b. Changed in
place: **T099** closes the question instead of opening a work stream, and keeps the losing option's
measured cost in `plan.md`; **T100** becomes the door (identity, and a refusal before any state);
**T103** ships the chunk encoder; **T104** refuses the reserved `media-track` rather than "the other
path". New: **T109** a capability on an API key that grants live voice and nothing else — code, not
prose, because `auth/middleware.rs:200` still resolves a key to its owner's full permissions — and
**T110** revocation or expiry ending a **running** session with `key_revoked` / `key_expired`.
**Q11 is answered, so the open-question count in `spec.md` drops to four.**


**Delta 2026-11-09 07:52 (design tokens, before the panel).** 97 → **98** tasks (T001–T098), nothing
renumbered: **T098** appended at the end of Phase 8, because the panel cannot be built on tokens that do not
exist yet. It is the first task in this file that touches `design/` rather than `crates/` or `webui/app/`:
`--vz-retrying-text` is still a proposal (`grep -rn retrying-text design/` → 0 hits) whose absence would
leave the reconnecting word at 4.03:1, and the dark failed state has been drawn from `#f87171`, which is a
generative-UI fallback value, not a library token. The task carries both hex pairs and their measured ratios
so the gate is arithmetic, and it says explicitly not to replace `#f87171` everywhere — that value is
legitimate where it is.

**Delta 2026-11-09 06:52 (User: the panel floats, one call in total — spec C9/C10).** 92 → **97** tasks
(T001–T097), no ID renumbered: five appended. Changed in place: **T089** (a panel mounted as an overlay,
not a call screen and not a route), **T090** (`LiveVoicePanel.tsx`), **T091**, **T092** (`live-voice-panel.spec.ts`).
New: **T093** drag and clamping (FR-048), **T094** the chat behind stays usable (FR-049), **T095** the
an owner index beside the routing key, with a move (FR-005/FR-050), **T096** the reason left behind (FR-051), **T097**
a turn cancelled by a move (FR-052). `LiveVoiceCall.tsx` no longer exists as a task target — a floating
panel is a different component, not a renamed one.

**Delta 2026-11-08 21:19 (User: surface, scope, trigger — spec C6/C7/C8).** 88 → **92** tasks
(T001–T092), no ID renumbered: four were **appended** for the call screen (T089–T092) and the seven US3
tasks are kept, tagged `[BACKLOG]`, and excluded from every checkpoint. Changed in place: **T037**
(entry control opens a call, not a toggle), **T038** (the composer bar became the call screen;
the `20d97316` bar amendment is archived, so its 210 px floor and narrowing order are **not**
requirements), **T039**, **T040**, **T044** and **T045** (the interruption trigger moved from
`speech_start` to `utterance_end`), **T047** (the onset machine is deleted, and reply ducking is
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

| ~~**D17** — FR-018 "answered and spoken" had no mechanism~~ **superseded 2026-11-08 by the User's decision** | T053, T056 (T054 reused for the typed-message path) | FR-018 is withdrawn: a typed message is text only. The `live_voice_text` frame, its contract entry (C-12), its task and its assertion are gone; typing still works as ordinary chat (FR-040); FR-025 — typing stopping the audio — is deferred with C7, and the client helpers for it (`Session.onTyped`, `forwardLiveVoiceTyped`) have **no call site**, so nothing in the app stops the reply on a keystroke today |

**Also recorded, no task changed**: the author's claim "no new mandatory dependency" holds
(`Cargo.lock`: `base64 0.23.1`, `tokio` `sync` feature, `serde`, `AudioContext`/`AudioWorklet` already
shipped); `tokio` is `1.53.3`, not the `1.49.0` written in `research.md` §C; `classifyFrame` lives at
`webui/app/api/frames.ts:606-630`, not `:619-621` (already corrected in the analyst's `e5ac773a`).

**Execution order is unchanged**: Phase 1 → Phase 2 (16 tasks, T007–T022) → US1..US5 →
Polish. `@sepuh-be` implements; `@teteh-qa` verifies with `speckit-analyze` + `speckit-checklist`
after T088.
