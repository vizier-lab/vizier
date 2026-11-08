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
   within about two seconds (SC-001).
3. **It follows up.** Ask a second question without touching anything (Story 1, scenario 3).
4. **Barge-in.** Ask something that produces a long answer. While it is speaking, say "no, the other
   one". The voice stops before your words are transcribed (FR-019), your correction becomes the next
   message, and the interrupted reply is in the conversation marked interrupted, readable as far as
   it got (FR-023).
5. **Interrupt control.** Ask for another long answer and press Interrupt. The audio stops, the
   session stays `listening`, and you can type a correction that is answered and spoken (FR-020,
   FR-021, FR-025).
6. **Separation.** Turn the agent's STT and TTS tool switches off in its settings and start Live
   Voice again: it still works (FR-003). Turn them on: the mic and the spoken-reply toggle still work
   as they did (FR-040).
7. **It lets go.** Close the tab mid-answer. The operating system's microphone indicator goes out
   within 2 s and the reply's text is kept (SC-007). Reload: the control is off (FR-001).
8. **It stops itself.** Start a session, stay silent for the idle period (3 min by default). It ends
   with a visible reason (FR-029).
9. **Two tabs.** Start a session in one tab and open the same conversation in another: the second is
   told a session is active and cannot start one (FR-005).

## The automated half

| What | Command | What it proves |
|---|---|---|
| Server frames | `cargo test -p vizier-http` | a session accepts the frames, one utterance produces one request, an interrupt bumps the generation and cancels, flag-off rejects them harmlessly |
| Engine | `cargo test -p vizier-core --features provider-live-voice` | segments are ordered and complete, the first frame is emitted before the last segment is requested, an interruption stops the stream |
| Client | `cd webui && npm test -- liveVoice` | the VAD fires on speech over playback and not on playback alone; stale generations are dropped; `stop()` drains the queue |
| End to end | `cd webui && npm run e2e` | both interruption paths with a faked provider |

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

