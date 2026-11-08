# Quickstart: Live Voice Conversation

**Feature**: `specs/055-live-voice` | **Date**: 2026-11-08

This is the by-hand check for after `tasks.md` is implemented (nothing here exists yet). It is
written so that the person who runs it does not have to read the plan.

## Before

* An agent with a **hosted** recogniser and voice configured (`stt.settings`, `tts.settings`) and
  their provider credentials working. Live Voice reads those settings whether or not the
  `stt`/`tts` *tools* are enabled — that is FR-003, and step 6 checks it.
* A laptop with a microphone and speakers, and a quiet room for the first pass. Headphones for the
  second.
* `provider-live-voice` in the build. In the workspace it is part of `full`, which is what
  `crates/vizier/Cargo.toml:30` asks `vizier-core` for, so the normal build has it. To prove the
  other side of FR-041:

```bash
cargo build -p vizier --no-default-features --features ""      # or without `full`
cargo test -p vizier-core --features provider-live-voice
cargo test -p vizier-core                                      # without it: everything else must pass
```

## Run it

```bash
just run                                   # cargo run -- run --config dev.vizier.yaml
cd webui && npm run dev                    # the WebUI, in a browser
```

Open a conversation whose agent has speech configured. The **Live Voice** control sits apart from the
microphone and the "ask for a spoken reply" toggle, and is off.

## Steps

1. **It starts.** Press Live Voice; the browser asks for the microphone; the bar shows `connecting`
   then `listening`, and the state word is visible without a reload (FR-006, FR-038).
2. **It answers.** Ask a short question and stop speaking. Within ~800 ms the utterance closes, the
   question appears as your message marked spoken, the reply appears, and you hear its first words
   within SC-001's gate: p90 under 3.5 s with a hosted recogniser and voice (2 s at p50 is the
   stretch target, not the gate — the budget's floor is 2.57 s before the model answers).
3. **It follows up.** Ask a second question without touching anything (Story 1, scenario 3).
4. **Barge-in.** Ask something that produces a long answer. While it is speaking, say "no, the other
   one". The voice stops before your words are transcribed (FR-019), your correction becomes the next
   message, and the interrupted reply is in the conversation marked interrupted — with its whole text
   when it had already been spoken, or with the interrupted mark and the activity trail when the
   interruption landed while the model was still working (FR-023, FR-027).
5. **Interrupt control.** Ask for another long answer and press Interrupt. The audio stops, the
   session stays `listening`, and you can type a correction: it is answered **as text**, with nothing
   spoken for it, while the session stays live (FR-020, FR-021, FR-025; FR-018 withdrawn).
6. **Separation.** Turn the agent's STT and TTS tool switches off in its settings and start Live
   Voice again: it still works (FR-003). Turn them on: the mic and the spoken-reply toggle still work
   as they did (FR-040).
7. **It lets go.** Close the tab mid-answer. The operating system's microphone indicator goes out
   within 2 s and the reply's text is kept (SC-007). Reload: the control is off (FR-001).
8. **It stops itself.** Start a session, stay silent for the idle period (3 min by default). It ends
   with a visible reason (FR-029).
9. **Two tabs.** Start a session in one tab and open the same conversation in another: the second is
   told a session is active and cannot start one (FR-005).

10. **Measure the three gates.** SC-001, SC-002 and SC-003 are only gates if they are measured the
    same way every time (T073 fixes the marks). With a hosted provider, three turns are enough for a
    smoke measurement; a release gate wants twenty:
    * **SC-001** — the clock starts when the utterance closes on the client (the 800 ms hangover
      fires) and stops at the first `AudioBufferSourceNode.start()` of the reply. Read it from the
      `live_voice_metric` lines the client logs, or from the browser's Performance panel.
    * **SC-002** — speak over the agent and read the same log: utterance-close to `player.stop()`
      returning must be <= 50 ms on the client, and the server span must show the `Abort` delivered
      within 300 ms.
    * **SC-003** — press Interrupt and read the click-to-silence time; it must be <= 200 ms in every
      attempt, which is why `stop()` is synchronous.
    Record the numbers in the pull request next to the run's provider and machine. A local whisper
    model is expected to miss SC-001; that is open question Q3, not a bug.


## The `dummyplug` half (agent-observable, no keys, no microphone)

The constitution asks every feature's quickstart to script its agent-observable checks as `dummyplug`
steps (`specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`). Live voice's *audio* half
cannot run on dummyplug — the offline provider has no recogniser and no voice — so the split is
explicit: the three steps below are the agent-observable half and run with no keys; the steps above are
the audio half, and only their step 2 and step 10 need a live provider (marked *live provider* where
they appear).

**Setup**: `just run`; one agent `voice` on provider `dummyplug`; a conversation with it. Nothing here
needs a microphone, a key, or the desktop app.

**The frame shape is the real one.** A request on the conversation socket is the whole `VizierRequest`
(`crates/vizier-core/src/schema/request.rs:282-350`): `content` is an externally tagged enum
(`VizierRequestContent`, `#[serde(rename_all = "snake_case")]`, `:40-67`), so it is
`{"chat": "…"}` — not `{"content": "chat", "message": "…"}`. The repository's own test sends exactly
this shape (`crates/vizier-http/src/topic.rs:48-53`):

```json
{"timestamp": "2026-11-08T15:00:00+07:00", "user": "owner",
 "content": {"chat": "tools"}, "metadata": {}}
```

`timestamp` must be present but is replaced on receipt (`topic.rs:36-39`); `user` is the sender's name;
`metadata` may be empty. `spoken` is the field T017 adds, `#[serde(default)]`, so a frame without it
reads `false`. `curl` cannot send this — it is a WebSocket text frame on
`/api/v1/agents/{agent_id}/channel/{channel_id}/{topic_id}`. Today that means the WebUI's own socket,
or `scripts/dummyplug-vizier-e2e.mjs` as a template; the scripted version of these three steps
(`scripts/live-voice-e2e.mjs`) is **written by T081**, the task that runs this gate, so it does not exist
yet; until it does, the three steps below are run by hand over the WebUI socket.

1. **A spoken message reaches the agent as speech.** Send
   `{"timestamp":"2026-11-08T15:00:00+07:00","user":"owner","content":{"chat":"tools"},"spoken":true,"metadata":{}}`.
   **Expect** a `dummyplug` reply, one stored `Request` whose `spoken` is `true`, and the prompt the
   agent received containing the listening note (FR-017): visible in the session's history entry and,
   with `RUST_LOG=vizier_core::agents=debug`, in the turn's prompt trace.
2. **A typed message does not.** Send the same frame without `spoken` (or with `"spoken": false`).
   **Expect** the same reply, a stored `Request` with `spoken: false`, and **no** listening note — the
   flag is per turn, never global (FR-017, FR-040).
3. **The interruption path the session reuses works.** While a turn is running, send
   `{"timestamp":"2026-11-08T15:00:01+07:00","user":"owner","content":{"command":"abort"},"metadata":{}}`
   — the same content `/abort` sends (`crates/vizier-core/src/agents_client.rs:298-312`).
   **Expect** the turn ending as `abort`, the conversation showing "Stopped by you", and no partial
   reply text invented (FR-027, FR-023's abort case) — exactly what T044 calls.

What these three steps **do not** cover, and where it is covered instead: voice activity detection,
segmentation and playback, and the interruption by speech — there is **no interrupt button** in this
release (spec C7), so there is one path to test, not two. Those are `crates/vizier-http/tests/live_voice_*.rs`
and `webui/e2e/live-voice-*.spec.ts` with a faked recogniser and voice, plus the steps above by hand.
A scripted browser run of the interruption path against a live provider stays a manual gate, and it is
marked as such.

## The automated half

| What | Command | What it proves |
|---|---|---|
| Server frames | `cargo test -p vizier-http` | a session accepts the frames, one utterance produces one request, an interrupt bumps the generation and cancels, flag-off rejects them harmlessly |
| Engine | `cargo test -p vizier-core --features provider-live-voice` | segments are ordered and complete, the first frame is emitted before the last segment is requested, an interruption stops the stream |
| Client | `cd webui && npm test -- liveVoice` | the VAD fires on speech over playback and not on playback alone; stale generations are dropped; `stop()` drains the queue |
| End to end | `cd webui && npm run e2e` | the interruption path and the call screen, with a faked provider |

Speech providers are faked in every automated test. One test is marked as needing a live provider;
run it with credentials before calling the feature done.

## What to look at when it goes wrong

* **No sound but a transcript:** the voice provider failed — the reply should still be text and a
  reason should be visible (FR-032).
* **It interrupts itself:** echo cancellation was not honoured. Speak at 80% volume with no
  headphones — that is SC-008, and failing it is a hard failure, not a tuning issue.
* **Long silence before the first word:** the recogniser is slow (a local whisper model will be).
  Check which provider the agent uses before blaming the pipeline.
* **The microphone stays on:** a bug, and the worst kind — check the idle stop and the page-closed
  path first (FR-028, FR-029).

