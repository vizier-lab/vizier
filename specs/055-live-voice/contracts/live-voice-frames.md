# Contract: Live Voice frames

**Feature**: `specs/055-live-voice` | **Date**: 2026-11-08 | **Surface**: the existing topic WebSocket

All frames travel on the socket the conversation already uses
(`/api/v1/agents/{agent_id}/channel`, `crates/vizier-http/src/api/v1/agents/channel.rs`), as JSON
text. No new endpoint, no new authentication, no binary frames (they are dropped today,
`channel.rs:818`).

**Compatibility rule**: every server frame carries a distinct top-level key, `live_voice`, so a client
that does not know it ignores it (`webui/app/api/frames.ts:607-630`). With `provider-live-voice` off,
no `live_voice` frame is ever emitted. Client frames carry `live_voice_*` action names; a server
without the feature ignores them (its inbound handler drops unknown keys) and the client never sends
them, because the capability reads false.

## Common fields

| Field | Type | Notes |
|---|---|---|
| `generation` | u64 | per session; a frame from a lower generation is stale and is dropped |
| `utterance_id` | string | correlates one utterance |
| `sample_rate` | u32 | always `16000` in this release |
| `format` | string | always `"pcm16"` in this release |
| `seq` | u32 | order within its stream |

## Client to server

### `live_voice_start`

```json
{ "live_voice_start": { "session": "…", "language": "id" } }
```

Refused with `live_voice_error` when the agent has no usable recogniser or voice (FR-004), or when
another person holds the session (FR-005). Idempotent while the caller's own session is live.

### `live_voice_audio`

```json
{ "live_voice_audio": { "utterance_id": "u-42", "seq": 7, "sample_rate": 16000, "format": "pcm16", "base64": "…" } }
```

100 ms of PCM16 mono (3 200 bytes, 4 268 base64 characters). The server drops chunks that would take
one utterance past `max_utterance_seconds`, closes it, and says so once.

### `live_voice_utterance_end`

```json
{ "live_voice_utterance_end": { "utterance_id": "u-42" } }
```

Closes the utterance and starts recognition. Exactly one of: a transcript submitted as a message, a
`too_short` outcome, or an `unintelligible` error.

### `live_voice_speech_start`

```json
{ "live_voice_speech_start": { "utterance_id": "u-42" } }
```

Sent the moment the client's VAD registers the person speaking — **before** any audio, because this
is the barge-in signal (FR-019). Bumps the generation, abandons synthesis, cancels the running turn.
Also opens the utterance named in it.

### `live_voice_interrupt`

```json
{ "live_voice_interrupt": { "reason": "button" } }
```

`reason`: `button` | `typed` | `barge_in`. Same effect as `speech_start` on the audio and the turn,
but opens no utterance: the session stays live and listening (FR-020, FR-021, FR-025).

### `live_voice_stop`

```json
{ "live_voice_stop": {} }
```

Ends the session, releases everything, reason `pressed_again` (FR-028).

## Server to client

### `live_voice_state`

```json
{ "live_voice": { "type": "state", "state": "interrupted", "generation": 8, "reason": null } }
```

The FR-006 state word. `reason` is present only for `ended` / `failed` / `reconnecting`; it is one of
`pressed_again | left_conversation | page_closed | permission_withdrawn | conversation_deleted |
agent_changed | idle | connection_lost | server_restart | no_device | no_recognizer | no_voice |
provider_failed | unintelligible`.

### `live_voice_transcript`

```json
{ "live_voice": { "type": "transcript", "utterance_id": "u-42", "text": "no, the other file", "submitted": true } }
```

`submitted: false` with empty `text` is the too-short outcome (FR-024): the audio stopped and nothing
was sent.

### `live_voice_speech`

```json
{ "live_voice": { "type": "speech", "generation": 8, "seq": 3, "sample_rate": 16000, "format": "pcm16", "base64": "…", "final": false } }
```

One segment. `final: true` marks the last of the reply; the client plays the queue and returns the
state to `listening` (FR-015).

### `live_voice_error`

```json
{ "live_voice": { "type": "error", "kind": "provider_failed", "message": "the voice provider refused the segment" } }
```

`kind` mirrors the `reason` list above. The session's own state is not changed by this frame alone —
a `live_voice_state` always follows, so the client has one source of truth (FR-032, SC-009).

## Limits and budgets

| Limit | Value | Where |
|---|---|---|
| Audio chunk | 100 ms, 3.2 KB raw / ~4.3 KB base64 | this contract |
| One utterance | 60 s (default) | FR-010 |
| One frame | far below the 1 MiB bus envelope (`envelope.rs:11`) | measured at 4.3 KB |
| Backlog to one socket | a few seconds of audio, then frames are dropped and a stall is reported | plan, fallbacks |
| Sessions per conversation | 1 | FR-005 |
| Sessions per agent | unbounded in this release; the deployment's socket limits apply | open question Q4 |

## What is deliberately *not* in the contract

No binary frames, no per-frame acknowledgement (the socket's ordering is enough), no audio
retransmission (a lost chunk is a syllable, not a state), and no transcript editing. Adding a
realtime speech-to-speech pipeline later would add new frame names, not change these.


## Review delta (2026-11-08, `@mamang-lead-be`)

**C-1 — every server frame carries a `type` discriminator.** As first written, the six server frames
shared the single top-level key `live_voice` and were distinguished only by which fields they happened
to contain (`state` vs `utterance_id`+`text` vs `base64`), which a client cannot dispatch on safely
(`review.md` D3). The type in code is `#[serde(tag = "type", rename_all = "snake_case")]`:

| `type` | Variant | Extra fields |
|---|---|---|
| `state` | state word of FR-006 | `state`, `generation`, `reason` |
| `transcript` | one utterance's outcome | `utterance_id`, `text`, `submitted` |
| `speech` | one synthesised segment | `generation`, `seq`, `sample_rate`, `format`, `base64`, `final` |
| `error` | a provider or session failure | `kind`, `message` |

Client frames are unchanged: their top-level keys (`live_voice_start`, `live_voice_audio`,
`live_voice_utterance_end`, `live_voice_speech_start`, `live_voice_interrupt`, `live_voice_stop`) are
already distinct. Nothing else in this contract changed; the sizes, limits and "deliberately not in
the contract" list stand.
