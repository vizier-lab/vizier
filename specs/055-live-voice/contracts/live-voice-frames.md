# Contract: Live Voice frames

**Feature**: `specs/055-live-voice` | **Date**: 2026-11-08 | **Surface**: the existing topic WebSocket

All frames travel on the socket the conversation already uses
(`/api/v1/agents/{agent_id}/channel`, `crates/vizier-http/src/api/v1/agents/channel.rs`), as JSON
text. No new endpoint, no new authentication, no binary frames (they are dropped today,
`channel.rs:818`).

**The carrier is decided: `ws-json-pcm16`** — base64 PCM16, 100 ms chunks, on this socket (Q11, closed
by the User 2026-11-09 16:35). **`media-track` stays in this contract as the reserved value only**: a
client MUST NOT send it, and a server MUST refuse it if it does, because none of the setup, decoding or
relaying for it exists. Every *frame* below other than the audio ones is
unaffected by that choice, and the transport-agnostic obligations a third party owes us are collected in
two sections at the end. **Two rules hold under either carrier and are not provisional:** an audio
description this document publishes **MUST** be accepted, and audio in a description it does not publish
**MUST** be refused — never guessed at, never silently converted.

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

### `live_voice_audio` — **the shipped carrier (Q11, closed 2026-11-09 16:35)**

> **This is the shipped frame.** Q11 was closed on the socket by the User on 2026-11-09 16:35, so this
> frame is what a client sends — not a candidate. It is the path that was already implemented and tested
> (46 client tests, 3 server suites). `media-track` survives in this contract as a **reserved** value: a
> client must not send it and a server must refuse it, because none of the setup, decoding or relaying
> for it exists.

```json
{ "live_voice_audio": { "utterance_id": "u-42", "seq": 7, "sample_rate": 16000, "format": "pcm16", "base64": "..." } }
```

100 ms of PCM16 mono (3 200 bytes, 4 268 base64 characters). The server drops chunks that would take
one utterance past `max_utterance_seconds`, closes it, and says so once.

**If Q11 chooses a media transport, a server MUST refuse this frame rather than mix two audio paths in
one session.** Two sources for one utterance have no correct ordering, and `utterance_id` cannot tell
them apart. That refusal is part of every candidate, not of one.

The side of this contract a stranger implements — the lifecycle, the audio description, the key and
the closed surfaces — is collected in **one** section near the end (§ The public audio contract: what a
third-party client implements), because it is the part that must make sense without reading the rest.

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

Sent the moment the client's VAD registers the person speaking — **before** any audio — and it now
does exactly one thing: it **opens the utterance** named in it. It does **not** interrupt.

The interruption moved to the end of the utterance (spec C8, 2026-11-08 21:19): `live_voice_utterance_end`
bumps the generation, abandons synthesis and cancels the running turn. Between the two frames the agent
**keeps speaking**, which is the behaviour the User asked for; the client stops its own playback when it
sends `utterance_end`, so the person hears silence as soon as they stop talking.

### `live_voice_interrupt`

```json
{ "live_voice_interrupt": { "reason": "barge_in" } }
```

`reason`: `typed` | `barge_in`. It stops the audio and cancels the turn without opening an utterance: the
session stays live and listening (FR-027). `typed` is the value a typed message would send, and it is
**deferred with FR-025** — this release's client sends only `barge_in`.

**`button` is deferred to the backlog (spec C7, 2026-11-08 21:19).** There is no interrupt control in
this release, so a client MUST NOT send it. It is **not shown in any example above**, deliberately: a
third-party client copies examples, and an example is the strongest documentation there is. A server that
receives it anyway MUST treat it exactly like `barge_in` rather than fail, so the backlog item needs no
protocol change when it lands.

**`typed` exists only where the conversation is reachable during a session** — spec open question Q7's
default says it is yes — a row above the composer covers nothing (C11, FR-049) — and a call is moved
rather than duplicated (C10). The reason stays in the contract so the backlog item needs
no protocol change when it returns — but **`typed` is not sent by this release's client**: FR-025 is
deferred with the interrupt control (C7, 2026-11-08 21:19), and although the two client helpers for it
exist (`Session.onTyped`, `webui/app/lib/liveVoice/session.ts:239`, and `forwardLiveVoiceTyped`,
`webui/app/lib/liveVoice/control.ts:56`), **nothing calls them** — the composer never reaches the
session, so a keystroke stops nothing today.

**A typed message has no frame here.** When FR-025 returns, typing during a session sends
`live_voice_interrupt { reason: "typed" }` (the audio stops) and then the text as an ordinary chat
request on the same socket, exactly as it would with no session at all — the missing piece is the call
site, not the frame. Its reply is text and is never spoken (FR-018 withdrawn,
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
an indicator in the row — never as the same thing as `reconnecting`. `reason` is present only for
`ended` / `failed` / `reconnecting`; it is one of
`pressed_again | left_conversation | page_closed | permission_withdrawn | conversation_deleted |
agent_changed | idle | connection_lost | server_restart | no_device | no_recognizer | no_voice |
provider_failed | unintelligible | moved | key_revoked | key_expired`.

**`key_revoked` and `key_expired` are new (C15, 2026-11-09 16:35).** A third-party client reaches this
feature with a scoped API key (FR-061), and that key can be taken away while a call is running:
revocation or expiry ends the session through the ordinary release path, with the reason the client
shows, rather than being noticed at the next connection (FR-063).

**`moved` is new (spec C10, 2026-11-09).** There is exactly one live session per person, so a
`live_voice_start` that arrives for a *different* conversation or agent is not a second session: the
server ends the old one with `reason: "moved"` and starts the new one. The old client shows that reason
until its row goes; nothing is written into the old conversation's history (open question Q10's
default), and the old session's running turn is cancelled on the normal abort path rather than spoken
into the new call (FR-052). A `live_voice_start` for the **same** conversation is still refused when
another connection holds it (open question Q2's default).

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

`kind` mirrors the `reason` list above, plus **two kinds that are not session reasons**, because a
refusal that happens *before a session exists* has no session to end and cannot carry a `reason`:

| `kind` | When | What the client shows |
|---|---|---|
| `forbidden` | the caller may not use this feature at all: missing, revoked, expired or under-scoped key, or no right to chat in this conversation (FR-064) | nothing about the conversation — the refusal must not disclose whether it, the agent or a session exists |
| `session_active` | this conversation already has a call, held by another connection of the same owner (FR-005, Q2's default) | that the call is already open here, and that it can be ended from the connection holding it |

**One term, one meaning.** `session_active` is the **only** answer for a second connection on a
conversation that already has a call, and it is **never** the answer to a start elsewhere — that
**moves** the call (FR-050, FR-051). A server MUST NOT answer a start elsewhere with `session_active`,
and MUST NOT answer a same-conversation start with anything else. When a session does exist, the frame's
usual rule holds: the session's own state is not changed by this frame alone — a `live_voice_state`
always follows, so the client has one source of truth (FR-032, SC-009).

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

> **This is the delivery path that ships (Q11, closed 2026-11-09 16:35).** Audio frames travel on a
> per-connection lane with a bounded queue and a `stall` frame, because they are droppable; control
> frames keep the session broadcast, because they are not. Both halves are now the product, not a
> choice between halves.

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
| Audio chunk | 100 ms, 3.2 KB raw / ~4.3 KB base64 | this contract, `ws-json-pcm16` |
| One utterance | 60 s (default) | FR-010 |
| One frame | far below the 1 MiB bus envelope (`envelope.rs:11`) | measured at 4.3 KB |
| Backlog to one socket | 16 audio frames (~1.6 s), then audio frames are dropped and one stall is reported | this contract, Delivery path |
| Sessions in total, per person | **1**, across every conversation and agent (a start elsewhere moves it, reason `moved`) | FR-005, FR-050, C10 |
| Sessions per agent | unbounded in this release; the deployment's socket limits apply | open question Q4 |

## What is deliberately *not* in the contract

No binary frames, no per-frame acknowledgement (the socket's ordering is enough), no audio
retransmission (a lost chunk is a syllable, not a state), and no transcript editing. Adding a
realtime speech-to-speech pipeline later would add new frame names, not change these.

**C-12 — `live_voice_text` (D17).** ~~The contract had no way to say "the person typed this while the
session is live", so FR-018's "answered **and** spoken" had no path.~~ **Superseded 2026-11-08 by the
User's decision:** a typed message is text only, FR-018 is withdrawn, and the frame is removed with it.
Kept as history so nobody re-adds it from an old branch.

## The public audio contract: what a third-party client implements

This section is written **for a client we have never seen** (FR-058) and it is deliberately the only
place where a third party's obligations are stated together, because the parts of this feature that a
stranger must implement are: a key, a lifecycle, an audio description, and a list of surfaces that are
ours. Everything else is our implementation.

**The lifecycle, the description and the key are settled** — Q11 closed on the socket, so what a client
builds against is what ships, not a shape to be confirmed later. The carrier column reads
`ws-json-pcm16` everywhere, because that is the decision (Q11); `media-track` survives in this contract
as a reserved value so that a future change is a value change rather than a rewrite — and a server must
refuse it rather than half-honour it.

### The lifecycle, as a client sees it

| Step | Client sends | Server answers | Carrier |
|---|---|---|---|
| begin | `live_voice_start` — conversation, agent, and the audio description it will send | `state: connecting`, then `listening`; or `error` with a reason | socket |
| send sound | audio in the description it declared | nothing per frame (the socket's order is enough) | `ws-json-pcm16` — `live_voice_audio`, 100 ms chunks. `media-track` is reserved and refused |
| close an utterance | `live_voice_utterance_end` — `utterance_id` | `live_voice_transcript`, or an error (`too_short`, `unintelligible`) | socket |
| receive the reply | — | the reply **text** in the conversation, and audio per segment | text: socket, always; sound: the same carrier the client sends on — one carrier per session, never mixed |
| interrupt by speaking | **nothing extra** — the end of an utterance while a reply is audible | `state: interrupted`, a new `generation`, the rest of the reply discarded | socket |
| end | `live_voice_stop` | `state: ended` with a reason | socket |
| move | `live_voice_start` for another conversation or agent | the old session `ended` with reason `moved`, the new one starting | socket |

**The reason vocabulary is part of the contract**, because a third-party client has to show something
truthful: `pressed_again | left_conversation | page_closed | permission_withdrawn |
conversation_deleted | agent_changed | idle | connection_lost | server_restart | no_device |
no_recognizer | no_voice | provider_failed | unintelligible | moved | key_revoked | key_expired`. These strings are stable; a
client MUST NOT receive a reason it has no word for.

### The audio description (FR-057)

A client declares what it will send before it sends it. A server **refuses** a description it cannot
accept, with a reason from the list above — it never silently downgrades or converts.

| Field | Today | Reserved |
|---|---|---|
| `format` | `pcm16` | `opus` — **reserved, refused today** |
| `encoding` | `base64` inside JSON | `media-track` — **reserved, refused today** |
| `sample_rate` | `16000` | by negotiation |
| `channels` | `1` | `1` (a second channel is not a feature, it is a bug with more bytes) |
| `frame_ms` | `100` | by negotiation |

**Two rules that make this a contract rather than a suggestion.** A client that declares the published
description MUST be accepted, whichever browser or library produced it (FR-057). And a client MUST NOT
send audio in a description it did not declare: that is refused, not guessed at, because the server
cannot tell an undeclared format from a corrupt stream.

### Authentication — two ways in, one authorisation outcome

| Client | How it gets in | What it may do |
|---|---|---|
| **this WebUI** | the person's own session, as every other screen does | live voice in the conversations that person can already open — no wider than the UI already is |
| **any other client** | an **API key scoped to live voice** (FR-061) | start, speak, hear, interrupt, end and move a live session for the person the key belongs to — **and nothing else** |

**The scope is the security boundary, not a detail of the key.** A key issued for live voice MUST be
refused by every route outside live voice: another conversation, another agent's history, settings,
memory, files, jobs, provider credentials. It MUST NOT inherit the owner's role or permissions — a key
that resolves to whatever the person can do is a password, and handing one to an outside system is how a
partnership becomes an incident. FR-062; measured by SC-013 against an explicit list of routes.

**The lifecycle a client may meet** (FR-063):

| Event | What the server does | What the client sees |
|---|---|---|
| key expired | the next request is refused; a **live** session ends | `ended` with reason `key_expired` |
| key revoked | as above, within 1 s of the revocation | `ended` with reason `key_revoked` |
| key rotated (new key, old one retired) | the old key stops working; a session opened with it ends | reason `key_revoked` on the old session |
| key scoped without live voice | refused before any state exists | `error` with `kind: "forbidden"` |

**Refused before anything is created** (FR-064). A missing, revoked, expired or under-scoped key is
answered before a session, an utterance or a turn exists — and the refusal MUST NOT disclose whether the
conversation, the agent or a live session exists, because a caller who may not use the feature may not
enumerate with it either.

**What the key never carries**: the person's password, their WebUI token, an agent's provider
credential, or a second long-lived secret minted for the media path. One key, one scope, one session at a
time (FR-005 applies to a key exactly as it applies to a person: same owner, same single call).



### What is ours and stays ours (FR-059)

| Ours | Why a third party must not need it |
|---|---|
| the session registry (`LiveVoiceSessions`, `live_voice/session.rs:430`) | it is how **we** enforce one call per person (FR-005); publishing it would make our data structure contractual |
| the segmenter and its chunking | a client that reimplemented it would drift from the text the conversation stores (FR-055) |
| how the generation number is produced | the **number** travels in every frame — a client needs it to recognise a stale frame, and needs nothing about how it is maintained. The guarantee is public; the mechanism is not |
| provider credentials, model choice, voice selection | ours, and a client supplies its own if it runs its own |

**What a client also does not need, stated plainly so nobody looks for it:** our React components, our
client state machine, our `capture.ts`, our tests. If a document and a socket are not enough to build a
working client, this section is incomplete and that is a defect in the contract, not in the reader.

## The audio contract as a public surface (C14)

Everything above was written for one client: ours. This section is the part that has to survive a
client we have never seen (FR-058) — this is the **standalone** part the User asked for, and it is the
section a third party reads first.

**What a third-party client needs, in the order it will need it:**

1. **A socket, and a **key**.** An **API key scoped to live voice** (FR-061) — not the person's WebUI
   session, not an agent's provider credential, and emphatically not a key that carries the owner's
   whole account (FR-062). Its lifecycle is part of the contract: create, expire, rotate, revoke, and a
   running session ending with `key_revoked` / `key_expired` when one of those happens (FR-063).
2. **The identity of the call.** Which conversation, which session — and the guarantee that audio
   belonging to one cannot join another (FR-056). Without this a client cannot safely reconnect,
   because it cannot tell *my call* from *a call*.
3. **The audio it may send.** Sample rate, channels, frame duration, and the fact that these are
   required rather than observed (FR-057). This is the single most useful line in the whole contract
   for anyone not using a browser.
4. **The lifecycle.** Start, speak, hear, interrupt by speaking, move, end, and what a reconnect means
   (FR-033, FR-050). Expressed as steps and states, not as one client's internal events.
5. **The words for failure.** `live_voice_error`'s `kind` values, so the client can say something true
   instead of "something went wrong".
6. **Nothing else.** The segmenter, the generation counter, the registry and the provider credentials
   are ours (FR-059). A client that needs to know them is a client we failed to specify.

**The three surfaces that stay ours on purpose**, and why the line is drawn there:

| Internal | Why it is not public |
|---|---|
| the segmenter and its chunking | a client that reimplemented it would drift from the text the conversation stores (FR-055) |
| the generation counter | it is how *we* make an interruption safe; a client only needs the guarantee, not the mechanism |
| the session registry | it is how *we* enforce one call per person; publishing it would make an implementation detail contractual |

**What publishing this costs.** Six surfaces become promises: how a call is set up, identity, the key,
rule, the audio description, the lifecycle and the failure words. None of them can be renamed quietly
once a third party depends on them. That is the price of the User's request, and the mitigation is the
one FR-058 already names — **our own WebUI is a client of this contract too**, so breaking it breaks us
first. Its one difference is the door it comes in by (a person's session rather than a scoped key,
Authentication above); every frame past that door is the same, which is what keeps the contract honest. If that price is not worth paying, the honest move is to withdraw C14 explicitly rather than
publish and drift.

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

Client frames keep their top-level keys (`live_voice_start`, `live_voice_audio`,
`live_voice_utterance_end`, `live_voice_speech_start`, `live_voice_interrupt`, `live_voice_stop`); what
changed on 2026-11-08 21:19 is *when* two of them are sent and what one of them does — see
`live_voice_speech_start` above (opens an utterance, no longer interrupts) and `live_voice_interrupt`
(`button` deferred). The row of spec C11 needs **no new frame**: it is a surface over the same
session, and its state word comes from `live_voice_state`. Nothing else in this contract changed; the sizes, limits and "deliberately not in
the contract" list stand.
