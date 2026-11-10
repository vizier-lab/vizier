# Review: SPEC-055 Live Voice — verification and gate decisions

**Reviewer**: `@mamang-lead-be` | **Date**: 2026-11-08 | **Branch**: `055-live-voice`
**Reviewing**: `spec.md` (327 lines at `4c38fd73`, 335 now), `plan.md` (276 → 281), `research.md` (165),
`data-model.md` (105), `contracts/live-voice-frames.md` (138 → 187 after this review and D9),
`diagrams.md` (179), `quickstart.md` (82 → 83), `architecture.md` (338), `tasks.md` (290 → 292,
77 → 85 tasks). Four of them moved **during** this review — see the note below §1, and D9 for the
last one.

Everything below was run against the working tree, not accepted from the author's report. Commands are
reproducible; where a claim failed, the raw output is quoted.

## 1. State on disk (LANGKAH 1)

| Claim | Check | Result |
|---|---|---|
| branch `055-live-voice` exists and is checked out | `git rev-parse --abbrev-ref HEAD` | `055-live-voice` ✅ |
| 7 files in `specs/055-live-voice/` (incl. `contracts/`) | `ls -la` | 6 `.md` + `contracts/live-voice-frames.md` ✅ |
| `spec.md` 327 lines at the time of the report | `wc -l` | `327` then, `335` after the amendments of §3 |
| commit `4c38fd73` was the base, local only | `git log --oneline` | `4c38fd73` then `2dfc92c9`, `6357b0c5 docs: add tasks`, `e5ac773a docs(055): correct two citations…` ✅ |
| working tree clean | `git status --porcelain` | empty ✅ |
| `tasks.md` absent at the start of this job | `ls` | absent then; `6357b0c5` (15:15) is **this agent's own previous turn**, not a second writer — one file, one line of history, no parallel copy (§6, C-2) |

## 2. Code claims re-checked (LANGKAH 2, first half)

| # | Claim in `research.md`/`plan.md` | Verified at | Verdict |
|---|---|---|---|
| V1 | binary WS frames are ignored | `crates/vizier-http/src/api/v1/agents/channel.rs:818` — `Some(Ok(Message::Pong(_))) \| Some(Ok(Message::Binary(_))) => {}` | ✅ true |
| V2 | outbound is JSON text | same file `:804` — `Message::Text(frame.to_string().into())` | ✅ true |
| V3 | bus envelope 1 MiB | `crates/vizier-schema/src/envelope.rs:11` | ✅ true |
| V4 | STT is whole-clip | `crates/vizier-core/src/stt/mod.rs:13-19` — `transcribe(audio, filename, language)` | ✅ true |
| V5 | TTS is whole-buffer | `crates/vizier-core/src/tts/mod.rs:13-15` — `generate_speech(text, voice, speed) -> Vec<u8>` | ✅ true |
| V6 | no token streaming | `crates/vizier-core/src/llm/sse.rs:1-3` — *"no streaming API"* | ✅ true |
| V7 | speech settings per agent, `ToolConfig{enabled, settings}` | `crates/vizier-core/src/schema/agent.rs:196-201`, `:207-211` | ✅ true (range is 199-204 for stt/tts, 207-211 for `ToolConfig`) |
| V8 | desktop CSP forbids wasm | `app/src-tauri/tauri.conf.json:14` — `default-src 'self' …`, no `script-src`/`worker-src`/`'wasm-unsafe-eval'` | ✅ true |
| V9 | old clients ignore unknown frames | `webui/app/api/frames.ts:606-630` — `classifyFrame` ends with `return null` | ✅ true (line range is 606-630, not 619-621) |
| V10 | abort is an existing path | `crates/vizier-core/src/agents_client.rs:415` — `Abort { agent_id, session }`; the `/abort` command becomes `VizierRequestContent::Command("abort")` at `agents_client.rs:305` | ✅ true |
| V11 | capture worklet already ships from our origin | `webui/public/vizier-capture-worklet.js`, `webui/app/lib/wavRecorder.ts:13-14` | ✅ true |
| V12 | a client queue already has a `spoken` flag | `webui/app/lib/frames.ts:36` | ✅ but **client-only** — see D2 |
| V13 | `base64` present | `Cargo.lock` — `0.23.1` (also `0.13.1`, `0.22.1` for other crates) | ✅ true |
| V14 | `tokio` version stated | `research.md` §C says `1.49.0`; `Cargo.lock` has `1.53.3` | ⚠️ stale number, harmless |
| V15 | "no new mandatory dependency" | `tokio::sync::watch`, `serde`, `base64`, `AudioContext`, `AudioWorklet`, `WebSocket` all present; no wasm needed on the default path | ✅ holds |

## 3. Discrepancies that change the work (must be folded into `tasks.md`)

**D1 — FR-023 as written cannot be implemented.** `Abort` is documented as a turn that produced
*nothing*: `crates/vizier-core/src/schema/response.rs:156-166` — *"`Abort` and `Empty` are turns that
produced nothing"*, and `ends_turn()` treats them as terminal. The LLM layer is non-streaming
(`sse.rs:1-3`), so at the moment of an abort there is **no partial reply text** anywhere to keep.
There are only two reachable cases, and the spec's wording ("readable as far as it was produced",
"text produced so far stays") describes a third that does not exist:

* **case 1 — the reply `Message` already arrived** (that is what was being spoken): its full text is
  already saved; keep it and mark it interrupted. FR-023 fine.
* **case 2 — the turn is still generating** (nothing shown yet): abort ends it with `Abort`, nothing
  to mark. The UI must say "Stopped by you" (the existing behaviour, `MessageList.tsx`) and the
  session must not claim a partial reply exists.

**Status: already closed in the spec.** `spec.md:220` now states exactly this split — FR-023 was
clarified in the review delta committed on this branch while this review was being written — and
**T042** is worded to implement it. Nothing here is left for the User to answer; the finding is kept
because it is the reason T042 says *first the audio, then the abort, and no invented partial*.

**Note on the files moving under this review**: `spec.md`, `plan.md` and `quickstart.md` were amended
on this branch at 15:14-15:16 (FR-012 reworded, FR-023 clarified, SC-001 restated to p90 3.5 s / p99 5 s
with 2 s p50 as a stretch target, a 250 ms onset guard added to Phase D, and the 2.57 s latency floor
written into the budget). This review was read against `4c38fd73`; where a number here differs from the
spec, **the spec wins** and this file has been aligned.

**D9 — the audio path (raised by `@system-analyst` after this review was written; my call).**
`SessionEvent` travels a `broadcast::channel(1024)` shared by the process
(`crates/vizier-core/src/transport/mod.rs:29,270`), and a subscriber that falls behind receives
`Lagged` and loses **every** frame in between (`crates/vizier-http/src/topic.rs:609-613`). Putting
10 audio frames/s into that ring would let audio evict a reply's text, and the plan's promised
fallback ("drop audio frames beyond a small backlog") is unimplementable there, because a broadcast
drops all-or-nothing per subscriber.

*Mechanism check, corrected:* the report said a slow socket lags "far more quickly". It does not,
strictly: every arm of the outbound match is synchronous and `self.out` is `flume::unbounded()`
(`channel.rs:374`), so a slow network grows the flume queue instead of starving the subscriber. The
decision still holds for three grounds that do hold — all-or-nothing loss, ring eviction cost
(~4.4 MB, text pushed out of the replay window), and a socket queue that this feature would otherwise
feed at 43 KB/s with no ceiling at all.

*Decision:* live voice frames bypass the broadcast and are written to the owner's socket through
`ClientConnection::send` — a path the repo already uses for client tool calls, and whose own module
doc gives the same reason. Audio is dropped above `LIVE_VOICE_SOCKET_BACKLOG = 16` at the one place
that can see the real queue (`TopicOutbound::run`), control frames never are, and a closed connection
ends the session with `page_closed`. Applied to `plan.md` (Phase C, fallbacks, risks),
`contracts/live-voice-frames.md` (§Delivery path, C-6), `architecture.md` (§11, invariant I9) and
`tasks.md` (T020, T021; the old backlog task is now T040 and no longer invents a second queue).

**D2 — "marked as spoken" has no server-side field.** FR-011/FR-017 need the stored message to carry
"came by speech"; today `spoken?: boolean` exists only in the WebUI queue (`frames.ts:36`) and
`grep -rn "spoken" --include=*.rs crates/` returns **nothing**. `AudioChat(att, Some(text))` is the
existing voice path, but it stores the audio attachment (`request.rs:52`,
`agents/agent/mod.rs:876`), which FR-016 forbids. So: one additive field on `VizierRequest`
(`#[serde(default)] spoken: bool`) + the note for the agent injected in the block that already does
voice preprocessing (`agents/agent/mod.rs:712-750`). Added to `tasks.md` as T0xx (foundational).

**D3 — the frame contract has no discriminator.** `contracts/live-voice-frames.md` has six server
frames all under the single key `live_voice` distinguished only by which fields they happen to carry
(`state` vs `utterance_id`+`text` vs `base64`). A client cannot dispatch on that safely. Fix:
`LiveVoiceFrame` is `#[serde(tag = "type", rename_all = "snake_case")]`, i.e.
`{"live_voice": {"type": "state", …}}`. Applied to the contract in this review (see §6, delta C-1).

**D4 — outbound frames are a hand-written `match`, not automatic.** `plan.md` Phase C step 3 says the
topic "forwards it like any other frame". In fact every variant is mapped by hand:
`crates/vizier-http/src/topic.rs:535-600` (`SessionFrame::Response` → `serde_json::to_value`,
`Title` → `json!({"session_title": …})`, …). Adding `LiveVoice` therefore needs an explicit arm plus
a feature-gated existence, and `vizier-http` must compile with `vizier-core`'s flag **off**. This is
a task, not a freebie.

**D5 — inbound cost per audio frame.** `topic.rs:216-218` serialises every inbound frame
(`let text_str = frame.to_string();`) before dispatching. At 10 audio frames/s × 4.3 KB that is a
needless copy on the hot path; the `live_voice_*` branch must be taken **before** that line (or the
line moved), otherwise every session copies its own audio twice. Task, with a note.

**D6 — session registry must outlive one socket.** FR-005 (second tab refused / told) cannot be
satisfied by state on `TopicConnection` (`topic.rs:67-85`), which is per-socket. The registry goes
beside `questions: Arc<vizier_core::questions::Questions>` in the shared service layer and is handed to
the connection the same way. Architecture decision, recorded in `architecture.md` §3.

**D7 — `hound` must not become a coupling.** WAV header is 44 bytes; `hound` sits behind
`provider-media` (`plan.md` says so itself). Emitting the header by hand keeps `provider-live-voice`
independent of `provider-media`, which is what FR-041 wants. Fix in tasks.

**D10 — `ClientConnection::send` returns `false` for two different events.** It is
`try_send(frame).is_ok()` (`crates/vizier-core/src/agents/client/connection.rs`) and the queue is
`mpsc::channel::<serde_json::Value>(64)` (`crates/vizier-http/src/topic.rs:141`): `false` means
**full** or **closed**, and only the second ends a session. The D9 patch as first written read
`false` as "closed", which would end live sessions on a busy socket — the behaviour D9 exists to
prevent. Fixed in the contract (C-7) and in T020.

**D13 — the delivery lane, after `@sepuh-be` looked for the task that makes it real (15:25).** His
review of the plan found two things worth acting on and one claim that does not hold. All three were
re-checked in the tree:

* **Right, and now recorded where the code is:** the socket's `out` is `flume::unbounded()`
  (`crates/vizier-http/src/api/v1/agents/channel.rs:374`) and the writer is `mpsc::channel::<Message>(64)`
  (`:780`); a stalled writer makes `handle_socket` wait inside its `write_tx.send(..)` arm (`:790-825`),
  so `out_rx` stops draining and grows without limit. That chain is why the gate belongs in
  `TopicOutbound::run` on `self.out.len()`, and `architecture.md` §11 now spells it out link by link.
* **Wrong, and corrected:** he read the stall as also starving `self.events.recv()` and therefore
  producing `Lagged`. It does not: `events.recv()` lives in `TopicOutbound::run`, spawned as its own
  task (`channel.rs:787`), which keeps draining the broadcast into the flume. A slow socket costs
  memory, not broadcast lag; `Lagged` stays a risk of the *control* ring for other reasons.
* **A real hole, now a task:** `TopicConnection::open` builds a **new** `ClientConnection` per socket
  (`topic.rs:144`). Since the session holds the owner's `Arc`, a reconnect inside FR-033's window
  would leave the session alive and **mute**. New task **T067** re-binds the lane on an idempotent
  `live_voice_start` from the owner, and ends with `connection_lost` if the owner never returns.
* **His proposal, decided against:** a second `flume::bounded(20)` audio lane threaded through
  `TopicConnection::open` (two call sites: `channel.rs:375`, `embedded.rs:56`) plus a priority arm in
  `handle_socket`. It would work, but it adds a second queue to keep in sync with the first, changes a
  signature used by the desktop IPC path, and can put audio ahead of the `live_voice_state` that must
  arrive before the speech it describes. `out.len()` measures the same depth at the same moment.
  No task touches `channel.rs` **by decision**, not by omission.
* **Already covered, not re-opened:** his question whether T010 narrows — it was already written to
  carry control types only, with `speech` impossible to publish through `publish_session_event` by
  construction (T009/T010).

**D10b — the mockup gate is satisfied, and here is the record.** The constitution asks that a new
control be shown as a mockup before any UI code; `diagrams.md` §5 has the wireframe (one on/off control
apart from the mic, one always-visible interrupt control, the state bar), and it is **approved by this
review** — T036/T037 may write `.tsx` against it. Any change to that layout comes back here first.

**D11 — SC-001/002/003 had no measurement points** (raised by `@teteh-qa`). `plan.md` says the
budget is "measured in Phase F", but nothing said where the clock starts and stops, so "p90 <= 3.5 s"
was an aspiration rather than a gate. T073 fixes the three marks and `quickstart.md` step 10 is the
procedure; T074 keeps the server-side spans.

**D12 — `AgentCommand::Abort` does not exist** (raised by `@teteh-qa` H1, and she is right).
`crates/vizier-core/src/schema/commands.rs:142` lists `Create`/`Update`/`Delete`/`HealthCheck`/
`AgentHealth`/`RefreshIntegrations`/`IntegrationStatus`/`Restart`; `agents_client.rs:415` is
`FakeCall::Abort`, a test fake. The real call is `AgentsClient::abort(agent_id, session)`
(`agents_client.rs:205-207`, impl `:298-312`), which sends `VizierRequestContent::Command("abort")` —
the same thing `/abort` does. T044 was rewritten; as written it would not have compiled.

**D15 — the 250 ms onset guard ate SC-002's 300 ms budget** (raised as H6/M11). *(Numbered D15 here:
D13 in this file is the delivery lane, raised by `@sepuh-be` at 15:25; the colliding heading is
`analysis.md` M12's finding, fixed without changing what either says.)* The guard is now split
from the local stop: `player.stop()` runs the moment the gate opens (SC-002's 50 ms, measured from the
first voiced sample), and the ~250 ms guard only delays *telling the server*. A burst shorter than the
guard resumes playback from the segments already queued instead of leaving the reply half-spoken
(FR-022) and submits nothing (FR-024). SC-002's server-side 300 ms is measured from the guard firing —
stated in the plan and in T045.

**D14 — FR-037 (configured language) had no task** (raised as H5). T030 now passes
`stt.settings.language` into `VizierStt::transcribe`, the same value the auto-transcription path uses
(`crates/vizier-core/src/agents/agent/mod.rs:721`), with a test.

**D8 — Q4 in `@bro-pm`'s summary and in the spec disagree.** The summary says the default is "ada cap
(angka di spec)"; `spec.md` Q4 says **no extra cap** (deployment socket limits apply), with 3 min
idle + 60 s utterance as the only limits. Taking the spec's default (no new cap) — it is the one with
the reasoning attached, and adding a cap later is one config value.

**D16 — where the lane drops, and who counts (the ambiguity D9 left).** *(D15 above took the number
this finding would have had; `D14` is the language finding T030 cites, so the lane decision is D16.)*
D9 moved audio to a
per-connection queue but left two counter-owners standing: T020 (the session counting a `try_send`
failure) and T021 (the outbound loop counting a depth drop). Two counters for one behaviour is how a
stall gets reported twice, or never. One rule now: the **outbound loop** owns `backlog`, because that
is where the drop happens and the only place that can see the socket's real depth; the session's
queue-full is a logged backstop (a healthy pair never fills 64 slots) and never becomes a user-facing
frame. It is reported as one control frame per episode, `{ "type": "stall", "dropped": N }`.

*Consequence, stated so nobody looks for it:* **no task touches `channel.rs` or `embedded.rs`, and
`TopicConnection::open` keeps its signature.** `@sepuh-be`'s bounded-lane proposal
(`flume::bounded(20)` + a priority arm in `handle_socket`) is recorded as considered and rejected in
`architecture.md` §11: a second queue whose depth must be kept in sync with the first, two call sites
changed (including desktop IPC), and a priority arm that can put audio ahead of the `live_voice_state`
that has to reach the client *before* the speech it describes.

## 4. The three checks the brief named (LANGKAH 2, second half)

**4.1 "Separate feature, own button, does not ride the existing STT/TTS switch" — PASS.** FR-001/003
are unambiguous, `data-model.md` §5 gives it its own `ToolConfig`-shaped block with `enabled: false`,
and its own flag `provider-live-voice`. Reusing the agent's *settings* (provider, voice, language) is
not reuse of the *switch*: `stt.enabled`/`tts.enabled` are never read by the live path, and
`quickstart.md` step 6 tests exactly that (tools off → live voice still works). One nit only:
`LiveVoiceSettings.stt/tts = null` means "use the agent's speech settings" — that must resolve the
**settings**, ignoring `enabled`. Written into `architecture.md` §10 as an invariant so nobody
"helpfully" checks the tool flag.

**4.2 Both interruption paths are QA-testable — PASS, with one added observable.** Each has
acceptance scenarios (US2 1-5, US3 1-4) and its own frame (`live_voice_speech_start` vs
`live_voice_interrupt {reason}`), so QA can drive both without audio. Missing piece: "the audio stops
**before** their words are transcribed" (FR-019) is not observable from the outside. Add the
observed-at timestamps to the client state machine test: `player.stop()` is called on the same tick
the VAD fires, and `live_voice_speech_start` is sent immediately after — no waiting for
`utterance_end`. In `tasks.md`: an assertion, not a new requirement.

**4.3 FR-013 "per segment" still means a live conversation — accepted, with a risk raised.** With a
non-streaming model the first audible word waits for the whole reply text, not for the first sentence.
For an ordinary spoken answer (2-4 sentences) that is fine. For a long answer (a 1500-word report), the
person hears nothing until the model finishes: the feature still feels live in the short-answer case
and does not in the long-answer case. **Risk raised to `@bro-pm`, requirement untouched:**

* **RK-1**: live voice is only as live as the model's whole-response latency. Mitigations that do **not**
  change the requirement: (a) instruct the agent, per FR-017, to answer briefly when spoken to; (b)
  show "agent is thinking" text immediately so the pause is explained; (c) token-level speech stays a
  separate feature (FR-013 already excludes it). If the User wants real sub-second first-word, that is a
  streaming turn protocol — a different spec.

## 5. Decisions raised to the User (not taken here)

D8 is answered on the implementation side (below) so QA can write the test; RK-1 stays a requirement
question for the User. Neither blocks a task: the answer changes wording and a config value, not
structure.

| # | Decision | Status |
|---|---|---|
| D8 | PM's summary asked about a **duration** cap; the spec's **Q4** is a cap on **concurrent sessions** — two different questions (raised as M5) | **answered for the implementation.** No concurrency cap and no duration cap are introduced: the only limits are the spec's idle stop (3 min, FR-029) and utterance cap (60 s, FR-010), which are `quickstart.md` steps 8-9 and T057/T059. A duration cap would be a **new requirement**, not a default; a concurrency cap is one config value. If the User wants either, it is a one-line default and needs no restructure |
| RK-1 | Accept "live = model latency" for long answers? | **open — raised to the User via `@bro-pm` (15:23); no default taken.** The plan's FR-017 note (answer for listening) is a mitigation, not an answer |

## 6. Delta applied to the reviewed artifacts (mine, dated)

Every change made **in this review**, with the finding that forced it. Nothing here was made by
rewriting the analyst's text wholesale: each row names the file and what it now says.

| # | Change | Files | Finding |
|---|---|---|---|
| C-1 | every server frame carries an explicit `"type"` inside `live_voice` | `contracts` | D3 — six frames under one key, no discriminator |
| C-2 | tasks: T017/T018/T019 added, seven made specific, IDs renumbered | `tasks.md` | D1, D2, D5, D6, D7 |
| C-6 | the contract states the delivery path, not only the payload | `contracts` | D9 — audio on the broadcast cannot be dropped per kind |
| C-7 | `Full` and `Closed` separated; only `Closed` ends a session | `contracts`, `tasks.md` T020 | D10 |
| C-8 | control frames stay on the broadcast, audio does not; one counter owned by the outbound loop | `contracts`, `data-model.md`, `architecture.md` §11, `tasks.md` T021/T041 | D15, D16 |
| C-9 | the reconnect re-binds the session to the new connection | `architecture.md` §11, `tasks.md` T067 | D13 |
| C-10 | spec: FR-012, FR-023, FR-027, SC-001 restated; FR-027 now defers to FR-023 | `spec.md` | D1, plus the analyst's own SC-001 error |
| C-11 | spec: SC-002 names where its clock starts and stops; the third-party edge case stops promising attribution | `spec.md` | H6, H7 (`@teteh-qa`) |
| C-12 | `quickstart.md` gains a scripted `dummyplug` half and a measurement step | `quickstart.md` | C1 — the constitution's e2e gate had nothing to run |
| C-13 | tasks: T038/T039 now render the terminal states with their reason and no interrupt control, move the elapsed counter out of the announced text, and require the design system instead of a stylesheet | `tasks.md` | Lead's review, 2026-11-08 — `LiveVoiceBar.tsx:46-47` drops `ended` and `failed` while `session.ts:314-316` and `:324` already hold the reason: FR-006, FR-029, FR-030 |
| C-14 | tasks: T038 keeps the state words English, and `start()` clears the previous session's notice | `tasks.md` | same review — one Indonesian word in a bar of eight English ones widened it 2.3× in the designer's render; `session.ts:112-114` never resets `view.notice`, so an old reason can surface inside a healthy session |
| C-15 | **C9 (2026-11-09 06:52): the surface is a floating panel, not a screen and not a route.** US6, FR-043…FR-046 rewritten, **FR-048** (drag, clamp, resize, position) and **FR-049** (the conversation behind stays readable and usable) new, SC-010 reworded, wireframe replaced; `Tasks` target is `LiveVoicePanel.tsx`. C-13 and C-14 above were written against `LiveVoiceBar.tsx` and are **superseded, not withdrawn** — the bar is archived, its two behaviours (terminal reasons with no interrupt control, English state words, cleared notice) still bind the panel | `spec.md`, `tasks.md`, `architecture.md` §1, §8, `@teteh-desener`'s render | the User's decision, and the full screen could not promise FR-049: a screen cannot leave the conversation readable behind it **Superseded by C11 (2026-11-09 16:07), and this row is now history, not a promise:** the floating panel was cancelled — the surface is one row inside the composer's own meta line (surface B1-1), 24 px, 0 px added. FR-048 (drag) and Q9 (remembered position) retire with it (`spec.md:106`, `spec.md:980`); FR-049 now constrains the row (`spec.md:570`); what survives from the panel work is the order in which the row gives up its own detail, which `spec.md:203-210` carries and T094 tests. |
| C-16 | **C10 (2026-11-09 06:52): one live session per person, and it moves.** FR-005 rewritten, **FR-050**, **FR-051**, **FR-052** new, SC-011 new, Q4 and Q8 closed, Q2 narrowed to "second tab on the same conversation is refused", contract gains `ended{reason:"moved"}` and the "1 session in total" limit row; `architecture.md` §3 (two indexes, per-process honest limit) and a new §13 (the move: generation → abort → `ended{moved}` → new session, transcript stays) | `spec.md`, `tasks.md`, `contracts/live-voice-frames.md`, `architecture.md` §3, §10 I10, §13 | the User's decision. It is the one place the feature gets *less* permissive on purpose; the refusal that remains is the second tab, where the microphone is already in that conversation |

**Canonical baseline for anything quoting counts**: `specs/055-live-voice/tasks.md` at the commit
being read — **`cf663414`, the commit this paragraph was last corrected at (2026-11-10)** — carrying
**111 tasks (T001–T111)**, **64 `[x]` / 47 `[ ]`**, **7 `[BACKLOG]`**, **one superseded (T093)**, counted with
`grep -cE '^- \[[ x]\] T[0-9]{3}'` (the id sits outside the bold so one pattern counts all of them).
**T111 is `[PROVISIONAL]`**: it carries Level 1 of the latency work, whose direction the User approved
on 2026-11-10 and whose start is not scheduled — do not count it as committed work.

The history of that number, so an older figure can be dated instead of argued about: 88 → 92 at
`c760cdb3` with C8–C12 (the surface became a surface of its own — FR-044/FR-045 —, the composer bar
was archived, the interrupt control went to the backlog as T051–T058, and barge-in moved from the
start of speech to the end of the utterance), then **92 → 97 at `dbc50b37`** with C15–C16 (the
floating panel with FR-048/FR-049, and the move with FR-050/FR-051/FR-052 + SC-011), then **97 → 98**
(design tokens), then **98 → 108** at 2026-11-09 16:07 with the spec's C11–C14 (a row instead of a
panel, the retirement of a surface, the media contract), then **108 → 110** at 16:35 with the ninth
spec delta (the socket question closed; a scoped key for outsiders), then **110 → 111** with the
provisional T111. Any other number in an older message or report is stale, including the "97" this
paragraph itself used to carry, and the "85"/"86" before that.

---

## 7. Verdict

**Spec, plan, architecture, contract and tasks are implementable, and the analyzable objections are
closed.** Every "requirement" objection that turned out to be an impossible promise (FR-012, FR-023,
FR-027, SC-001, the third-party edge case) is corrected in the spec with a delta entry; every
code-shaped finding (D2, D4–D16) is a task or an invariant. C1 — the one finding that actually blocked
`speckit-implement` — is closed by `quickstart.md`'s `dummyplug` half.

What is **not** closed and cannot be closed here: RK-1 (long answers are silent until the model
finishes) is a requirement question for the User, and the live-provider audio pass stays manual.

Implementation belongs to `@sepuh-be`; verification of the result belongs to `@teteh-qa`.

---

## 8. What is still not covered (verified 2026-11-10 at `cf663414` + Sepuh's uncommitted T110 r2)

Written because a green gate is not the same as a closed gap. Every line below was checked with a
`grep`/`sed` against the tree named in the heading, and each one names what would close it.

**1. N2 — `idle_stop_seconds` is declared, defaulted, tested, and read by nothing.** The field is at
`crates/vizier-core/src/schema/agent.rs:230` with a default of `180` at `:242`; it is asserted in that
file's tests (`:616`, `:623-627`) and in `crates/vizier-core/src/service/agents.rs:1253`. No
production path reads it: a person who starts a session and walks away leaves the microphone open for
as long as the window is open. Closed by FR-029 (Opsi A, the server ends an idle session) — the same
work that closes gap 2.

**2. FR-029 / Opsi A — the server never ends a session on its own.** `last_speech_at` has **0 hits**
in `crates/` and `webui/app` (working tree and `cf663414`), and `EndReason::Idle` is produced only by
a test fixture (`crates/vizier-core/src/live_voice/frame.rs:420`) — the enum variant exists and
nothing in production emits it. **T026 was ticked while its own text promised exactly
`last_speech_at` + an idle timer**; it is unchecked again as of 2026-11-10, reason on
`specs/055-live-voice/tasks.md:138`.

**3. G3 fast-path — accepted deviation, not switchable per request.** The gate is the first condition
of the branch, `if !req.spoken { return false; }` (`crates/vizier-core/src/agents/agent/mod.rs:2372`),
and `VizierRequest` carries no `without_tools` field: the deviation was taken deliberately so the
public contract does not grow, and its cost is that a client cannot turn the fast path off for one
request. Its only observable is the counter `FAST_PATH_RETRIES` (`mod.rs:2353`, read at `:2360`,
incremented at `:1293`) — a number in a log, not in a response.

**4. The waveform is drawn but never moves.** `level` is initialised to `0`
(`webui/app/lib/liveVoice/session.ts:115`) and only ever re-set to `0` (`:402`); nothing assigns it
from an analyser or from the microphone. This is the designer's "visible but not moving, because
`loud = 0`" seen from the client side. The rest of QA's V1–V11 findings sit behind it; this is the
one the User will notice first.

**5. N1's late-device guard closes a microphone that was legitimately opening** — found by the
User's first manual test at 11:06 on the 10:32 binary, and reproduced by reading three lines.
`start()` sends `startFrame` and *then* awaits the device; the server answers a Start frame by
publishing `listening` at once (`crates/vizier-http/src/topic.rs:551` at `30500bf5`), and the client
accepts that frame out of `connecting` (`webui/app/lib/liveVoice/session.ts:98` at `08cf067`). The
guard N1 added at `:211` then reads `this.currentState !== 'connecting'` → true, so it stops the
capture and returns **before** `view.micOpen` (`:217`) and the render tick (`:220`). What a person
sees is exactly the screenshot: the word `Listening`, the chip `mic off`, the timer at `0:00`, no
audio ever sent, and N1's own deadline unable to fire because `abandonConnect` returns unless the
state is still `connecting`. Before N1 the same race was harmless, and it is not a stale build —
the minified guard is inside the binary that served the page
(`if(this.currentState!=="connecting"||this.released){i.stop();return}`, byte offset `19073262` of
`target/release/vizier`, md5 `65fe9ce727f272d0214487f2a88c2056`). Closed by ignoring a `listening`
frame while the client is `connecting` with no open capture, narrowing the guard to
`released`/`ended`/`failed`, and a test that delivers that frame mid-await — the 145 tests never did,
which is why the gate was green.

**Checked and NOT a gap** (so the same worry is not raised twice): `key_lapsed` is present at
`cf663414` — definition `crates/vizier-http/src/topic.rs:67`, call at `:415`, five test assertions
`:209-213`. It was reported as "zero hits" against an older tree; that report is stale.
