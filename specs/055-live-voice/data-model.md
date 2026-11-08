# Phase 1 Data Model: Live Voice Conversation

**Feature**: `specs/055-live-voice` | **Date**: 2026-11-08

Two kinds of thing: what the server holds while a session lives (nothing on disk), and what the two
sides say to each other. The wire forms are in
[contracts/live-voice-frames.md](./contracts/live-voice-frames.md).

## 1. LiveVoiceSession (server, in memory only)

One per (agent, conversation), owned by the person who started it (FR-005). Never persisted: a
restart ends every session (FR-028) and loses nothing but the session.

| Field | Type | Notes |
|---|---|---|
| `agent_id` | String | |
| `session` | VizierSession | the conversation, existing type |
| `owner` | Principal | who may end it; everyone else is told it is active (FR-005) |
| `state` | SessionState | see §2 |
| `generation` | `tokio::sync::watch<u64>` | bumped by every interruption (R3); carried on every audio frame |
| `utterance` | Option<Utterance> | the one being captured |
| `language` | Option<String> | from the agent's recogniser settings (FR-037) |
| `started_at` | Timestamp | |
| `last_speech_at` | Timestamp | drives the idle stop (FR-029) |
| `end_reason` | Option<EndReason> | set once, then the session is gone |
| `backlog` | usize | frames queued to a slow socket, dropped beyond the cap (plan, fallbacks) |

`EndReason` = `pressed_again | left_conversation | page_closed | permission_withdrawn |
conversation_deleted | agent_changed | idle | connection_lost | server_restart`.

Not stored, by decision: audio (FR-016). A session is readable afterwards only as its text.

## 2. SessionState

Mirrors FR-006 one for one, so the client never invents a state:

```text
off -> connecting -> listening -> waiting -> speaking
                                     ^          |
                                     |          v
                                  listening <- interrupted
any state -> reconnecting -> ended | failed
```

`interrupted` is transient and never ends a session. Every terminal state carries a reason, shown
until the person dismisses it (SC-009: within 5 s of the failure).

## 3. Utterance

| Field | Type | Notes |
|---|---|---|
| `id` | String | correlates `speech_start`, `audio`, `utterance_end`, transcript |
| `pcm` | `Vec<i16>` (16 kHz mono) | accumulated; capped at 60 s (FR-010) |
| `started_at` | Timestamp | |
| `last_voiced_at` | Timestamp | client-side hangover decides the end; the server enforces the cap |
| `cancelled` | bool | set when the generation moves on |

Lifecycle: opened by `live_voice_speech_start` (or by the first `audio` chunk if a `speech_start` was
missed), closed by `live_voice_utterance_end` or the 60 s cap. One open utterance at a time; a second
`speech_start` while one is open ends the first.

Outcome, exactly one: **message** (transcript submitted, FR-011), **too short** (audio stopped, no
message, FR-024), **unintelligible** (told, no message, FR-031), **cancelled** (session ended).

## 4. SpokenSegment

| Field | Type | Notes |
|---|---|---|
| `generation` | u64 | stale segments are dropped by both sides (FR-022) |
| `seq` | u32 | order within the reply |
| `pcm` | base64 PCM16 16 kHz mono | one segment, ~80–400 characters of speech |
| `final` | bool | the reply's last segment |

Produced by the segmenter from the reply text. The segmenter skips fenced code, tables and raw URLs
and emits one short notice instead (FR-014); it never splits inside a word and prefers the first
sentence end after ~80 characters, which is what keeps time-to-first-word inside the budget.

## 5. LiveVoiceSettings (per agent, additive, defaulted)

```jsonc
"live_voice": {
  "enabled": false,          // its own switch; independent of stt.enabled / tts.enabled (FR-003)
  "settings": {
    "stt": null,             // null => use this agent's stt.settings, whatever its tool flag says
    "tts": null,             // null => use this agent's tts.settings
    "idle_stop_seconds": 180,      // FR-029 default
    "max_utterance_seconds": 60,   // FR-010 default
    "end_of_utterance_ms": 800     // FR-010 default
  }
}
```

`enabled: false` and the two `null`s mean a stored agent file without this block behaves exactly as
it does today (FR-040). The three numbers are the spec's defaults, exposed so the plan's budgets can
be tuned without a rebuild.

## 6. What each existing entity gains

| Entity | Change | Constraint |
|---|---|---|
| Request / message | a request may be voice-originated; the reply that answers it may be spoken | additive flags only; history, search and deletion unchanged (FR-016) |
| Reply | may be marked *interrupted*, with how much was spoken | text stays complete-as-produced; nothing is rewritten (FR-023) |
| Agent detail | gains `live_voice: bool` beside `stt` / `tts` | additive; older clients ignore it |
| `SessionFrame` | gains `LiveVoice(LiveVoiceFrame)` | additive; unknown top-level keys are ignored by existing clients (`frames.ts:607-630`) |

