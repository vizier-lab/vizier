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

**A typed message has no frame here.** Typing during a session sends `live_voice_interrupt { reason:
"typed" }` (the audio stops, FR-025) and then the text as an ordinary chat request on the same socket,
exactly as it would with no session at all. Its reply is text and is never spoken (FR-018 withdrawn,
2026-11-08). The earlier `live_voice_text` frame existed only to route that reply through the session's
speaker; it is gone with the requirement.

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

The FR-006 state word. `waiting` is the one that carries FR-042: it is sent as soon as the turn is
accepted, before any audio exists, and the client must render it as **the agent is thinking** — text or
an animation in the bar — never as the same thing as `reconnecting`. `reason` is present only for
`ended` / `failed` / `reconnecting`; it is one of
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

### `live_voice` `stall`

```json
{ "live_voice": { "type": "stall", "dropped": 23 } }
```

The one report of a socket that fell behind: `dropped` is how many `live_voice_speech` frames were
discarded in that episode (contract §Delivery path). It is emitted **once** per episode by the
socket's outbound loop, carries no `generation`, and changes no state: the session is still live and
the next segment plays whenever the queue drains. It is a control frame, so it rides the session
broadcast like `state` and `transcript` — a person whose audio stalled hears about it on the same
channel that tells them the session is live.

## Delivery path (not the session-event broadcast)

**Audio** frames (`live_voice_speech`) do **not** travel through `publish_session_event` and the
session-event broadcast: they are written to the one socket that owns the session, through
`ClientConnection::send` (`crates/vizier-core/src/agents/client/connection.rs`). **Control** frames
(`state`, `transcript`, `error`, `stall`) keep the broadcast they already use — a handful per turn, and
it is how a second tab on the conversation is told a session is active (FR-005) without a per-socket
replay log. Three reasons the audio had to move:

| Why | What it means on the wire |
|---|---|
| the broadcast drops **all or nothing** per subscriber (`topic.rs:609-613`, `Lagged`) | a lagging socket would lose reply text because of audio; on a per-connection queue audio alone can be dropped |
| the broadcast ring holds 1024 events for the whole process (`transport/mod.rs:29`) | 10 audio frames/s at 4.3 KB would evict text from every behind subscriber's window and cost ~4.4 MB |
| a session belongs to one person (FR-005) | the broadcast is filtered **per session**, not per socket (`crates/vizier-http/src/topic.rs:533` — `ev.session == self.session`), so a second tab on the same conversation was receiving the audio of a session it may not have; per-connection delivery closes that leak, while the control frames above remain what tells that tab a session is active |

Rules:

* **Audio may be dropped; control may not.** `live_voice_speech` is written only while the socket's
  own queue is below `LIVE_VOICE_SOCKET_BACKLOG` (16 frames, about 1.6 s of audio); beyond it the
  frame is dropped and counted. `live_voice_state`, `live_voice_transcript` and `live_voice_error`
  are always written.
* **One counter, one reporter.** The socket's outbound loop performs the drop, so it owns the count:
  `backlog` rises on each discarded `speech` frame, resets when the queue falls back below the gate,
  and is reported **once per episode** as one `stall` frame (`{ "type": "stall", "dropped": N }`). The
  session keeps no second counter; a `false` from `ClientConnection::send` while the connection is
  open (its 64-slot queue full) is a backstop that is logged, never shown as a stall — a healthy pair
  drains that queue at flume speed.
* **The gate sits in the outbound loop, not in `channel.rs`.** No new channel, no
  `TopicConnection::open` signature change: `self.out.len()` is `flume`'s own `Sender::len`
  (`crates/vizier-http/src/api/v1/agents/channel.rs:374` creates an unbounded queue, `topic.rs`'s
  `TopicOutbound::run` is the only place that can see its depth). A stall never ends the session.
* **A closed socket ends the session; a full queue does not.** `ClientConnection::send` is
  `try_send(..).is_ok()`, so it returns `false` **both** when the queue is full (`Full`, capacity 64,
  `crates/vizier-http/src/topic.rs:141`) and when the connection has closed (`Closed`). Only the second
  is an ending: the session ends with reason `page_closed` when `!connection.is_open()` (FR-028), and a
  `false` while the connection is still open is a dropped audio frame, counted, nothing more. Reading
  `false` as "closed" would end live sessions on a busy socket — the exact opposite of dropping audio
  (contract delta C-7, `review.md` D10).
* **Control frames still ride the session broadcast**, and that is deliberate: `live_voice_state`,
  `live_voice_transcript` and `live_voice_error` are a handful per turn, so the 1024-slot ring
  (`crates/vizier-core/src/transport/mod.rs:29`) is not reachable by them, and a socket that reconnects
  inside FR-033's 30 s gets the current state without the session holding a per-socket replay log.
  Audio is the volume; audio is the only thing moved off it.

* **The lane follows the socket.** Every socket gets a fresh connection handle
  (`crates/vizier-http/src/topic.rs:144`), so after a drop and resume the session must adopt the new
  one: `live_voice_start` from the owner while their session is live is **idempotent** and re-binds
  the audio lane to the caller's current socket (FR-033). The client sends it again on reconnect; it
  does not resend the utterance in flight. A session whose owner does not return inside 30 s ends with
  `connection_lost`.
* **Client → server is unaffected**: inbound audio still arrives on the topic socket like every other
  client frame, because there is only one socket for it to arrive on.

## Limits and budgets

| Limit | Value | Where |
|---|---|---|
| Audio chunk | 100 ms, 3.2 KB raw / ~4.3 KB base64 | this contract |
| One utterance | 60 s (default) | FR-010 |
| One frame | far below the 1 MiB bus envelope (`envelope.rs:11`) | measured at 4.3 KB |
| Backlog to one socket | 16 audio frames (~1.6 s), then audio frames are dropped and one stall is reported | this contract, Delivery path |
| Sessions per conversation | 1 | FR-005 |
| Sessions per agent | unbounded in this release; the deployment's socket limits apply | open question Q4 |

## What is deliberately *not* in the contract

No binary frames, no per-frame acknowledgement (the socket's ordering is enough), no audio
retransmission (a lost chunk is a syllable, not a state), and no transcript editing. Adding a
realtime speech-to-speech pipeline later would add new frame names, not change these.



**C-12 — `live_voice_text` (D17).** ~~The contract had no way to say "the person typed this while the
session is live", so FR-018's "answered **and** spoken" had no path.~~ **Superseded 2026-11-08 by the
User's decision:** a typed message is text only, FR-018 is withdrawn, and the frame is removed with it.
Kept as history so nobody re-adds it from an old branch.


## Review delta (2026-11-08, `@mamang-lead-be`)

**C-7 — full is not closed (D10).** `ClientConnection::send` collapses two cases into one
`false`; the contract now separates them, because the queue is 64 slots (not a socket timeout) and a
busy socket must lose audio, not its session.

**C-6 — delivery path (D9).** The contract now says *how* server frames reach the client, not only
what they contain: **audio** goes per connection and may be dropped; **control** keeps the session
broadcast and is never dropped, which is also how a second tab learns a session is active (FR-005).
Before this, the document implied the broadcast forwarded all of live voice like any other frame,
which would have let audio evict reply text from a lagging subscriber — and sent one person's audio
to every socket on the conversation.

**C-1 — every server frame carries a `type` discriminator.** As first written, the six server frames
shared the single top-level key `live_voice` and were distinguished only by which fields they happened
to contain (`state` vs `utterance_id`+`text` vs `base64`), which a client cannot dispatch on safely
(`review.md` D3). The type in code is `#[serde(tag = "type", rename_all = "snake_case")]`:

| `type` | Variant | Extra fields |
|---|---|---|
| `state` | state word of FR-006 | `state`, `generation`, `reason` |
| `transcript` | one utterance's outcome | `utterance_id`, `text`, `submitted` |
| `speech` | one synthesised segment | `generation`, `seq`, `sample_rate`, `format`, `base64`, `final` |
| `stall` | the socket fell behind and audio was dropped (control lane) | `dropped` |
| `error` | a provider or session failure | `kind`, `message` |

Client frames are unchanged: their top-level keys (`live_voice_start`, `live_voice_audio`,
`live_voice_utterance_end`, `live_voice_speech_start`, `live_voice_interrupt`, `live_voice_stop`) are
already distinct. Nothing else in this contract changed; the sizes, limits and "deliberately not in
the contract" list stand.
