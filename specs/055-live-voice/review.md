# Review: SPEC-055 Live Voice — verification and gate decisions

**Reviewer**: `@mamang-lead-be` | **Date**: 2026-11-08 | **Branch**: `055-live-voice`
**Reviewing**: `spec.md` (327 lines), `plan.md` (276), `research.md` (165), `data-model.md` (105),
`contracts/live-voice-frames.md` (138), `diagrams.md` (179), `quickstart.md` (82) — commit `4c38fd73`.

Everything below was run against the working tree, not accepted from the author's report. Commands are
reproducible; where a claim failed, the raw output is quoted.

## 1. State on disk (LANGKAH 1)

| Claim | Check | Result |
|---|---|---|
| branch `055-live-voice` exists and is checked out | `git rev-parse --abbrev-ref HEAD` | `055-live-voice` ✅ |
| 7 files in `specs/055-live-voice/` (incl. `contracts/`) | `ls -la` | 6 `.md` + `contracts/live-voice-frames.md` ✅ |
| `spec.md` 327 lines | `wc -l` | `327` ✅ |
| commit `4c38fd73`, local only | `git log --oneline -5` | `4c38fd73 chore: save progress before planning` ✅ |
| working tree clean | `git status --porcelain` | empty ✅ |
| `tasks.md` absent | `ls` | absent ✅ (generated in this job, §4) |

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

*Recommendation*: split FR-023 into `a`/`b` along those lines. **Not changed unilaterally** — it is a
requirement wording change, so it is raised to `@bro-pm` (see §5, needs a yes/no). `tasks.md` is
written so case 2 is handled correctly either way.

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
`{"live_voice": {"type": "state", …}}`. Applied to the contract in this review (see §6, delta R-1).

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

**D8 — Q4 in `@bro-pm`'s summary and in the spec disagree.** The summary says the default is "ada cap
(angka di spec)"; `spec.md` Q4 says **no extra cap** (deployment socket limits apply), with 3 min
idle + 60 s utterance as the only limits. Taking the spec's default (no new cap) — it is the one with
the reasoning attached, and adding a cap later is one config value.

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

* R-1: live voice is only as live as the model's whole-response latency. Mitigations that do **not**
  change the requirement: (a) instruct the agent, per FR-017, to answer briefly when spoken to; (b)
  show "agent is thinking" text immediately so the pause is explained; (c) token-level speech stays a
  separate feature (FR-013 already excludes it). If the User wants real sub-second first-word, that is a
  streaming turn protocol — a different spec.

## 5. Decisions needed from `@bro-pm` / the User (nothing blocks implementation)

| # | Decision | Default used by `tasks.md` |
|---|---|---|
| D1 | FR-023 split into "reply already arrived → keep + mark interrupted" and "turn aborted before any text → nothing to mark, UI says stopped" | implemented as the split; spec wording left as authored pending a yes |
| D8 | Q4 cap: spec says none, summary says cap | no new cap (3 min idle + 60 s utterance only) |
| R-1 | Accept "live = model latency" for long answers? | accepted; FR-017 note asks for spoken-length answers |

## 6. Delta applied to the reviewed artifacts (mine, dated)

* **R-1 (contract)** — `contracts/live-voice-frames.md`: every server frame now carries an explicit
  `"type"` inside `live_voice` (D3). Client frames already have distinct `live_voice_*` keys.
* No other file of the analyst's was edited. Findings that need code-shaped answers (D1, D2, D4-D7)
  live in `architecture.md` and `tasks.md`.

## 7. Verdict

**Spec and plan are implementable, with D2/D4/D5/D6/D7 folded into the task list.** No requirement was
loosened. The two interruptions converge on one mechanism (generation counter), the separation from
STT/TTS is real, and the transport decision survives contact with the code. Proceeding to
`architecture.md` and `tasks.md`; implementation stays with `@sepuh-be`.

