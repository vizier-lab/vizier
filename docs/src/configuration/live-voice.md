# 2.23 Live Voice

Live voice is a spoken conversation with an agent, held open: you talk, the agent answers aloud, and
you can talk over it to cut it short. It is a capability of its own, separate from the `stt` and `tts`
tools — those transcribe a file and synthesise a file; live voice runs the whole loop.

It is **off by default** for every agent, and every conversation, until you turn it on.

## Turning it on

Live voice is a tool block on the agent, like `tts` and `stt`, and it is created at runtime through the
WebUI or `/api/v1/agents` — never in `.vizier.yaml`.

```json
{
  "tools": {
    "live_voice": {
      "enabled": true,
      "settings": {
        "idle_stop_seconds": 180,
        "max_utterance_seconds": 60,
        "end_of_utterance_ms": 800
      }
    }
  }
}
```

`settings.stt` and `settings.tts` are **optional overrides**. Left out — which is the default — live
voice uses the agent's own `stt_settings` and `tts_settings`, *whatever their `stt`/`tts` tool flags
say*: those flags gate the tools, not the session. Set them only when the conversation should use a
different recogniser or voice from the tools:

```json
{
  "tools": {
    "live_voice": {
      "enabled": true,
      "settings": {
        "stt": { "provider": "whisper", "model": "ggml-large-v3", "language": "en" },
        "tts": { "provider": "openai", "voice": "nova" }
      }
    }
  }
}
```

The agent needs **a usable recogniser and a usable voice** for a session to start at all — see
[Tools & Embedding](./tools-embedding.md) for the provider tables. Credentials are the ones already
saved for those providers; live voice asks for none of its own and stores none.

| Setting | Default | What it does |
|---|---:|---|
| `idle_stop_seconds` | `180` | A session with nothing said for this long ends itself |
| `max_utterance_seconds` | `60` | One utterance is capped here; the server enforces it |
| `end_of_utterance_ms` | `800` | How long a silence closes what you were saying |

## The row

Live voice lives in **one row inside the composer's meta line** — the line that already carries the
context ring and the model name. It adds **no height**: the composer is the same 156 px with or without
a call, and nothing moves when a session starts or ends. There is no floating panel, no separate rail,
and nothing to drag.

Before a session the row holds one control: a pill reading **`Live voice`**. Press it and the same line
becomes the call:

| The row says | What is happening |
|---|---|
| `Connecting…` | The microphone is being opened; the browser may ask for permission now |
| `Listening` | Open and waiting for you |
| `Thinking` | Your words are being turned into a turn and the agent is working — **not** a dropped call |
| `Speaking` | The reply is being read aloud |
| `Interrupted` | You cut the reply short; the session is still live and still listening |
| `Reconnecting` | The connection dropped; it stays open for 30 s before giving up |
| `Ended` | Over, with the reason underneath |
| `Failed` | Could not run, with the reason underneath |

Beside the state word the row always shows the microphone: `mic on` or `mic off`. It is never the
element that disappears, and it stays true when the window is not focused. There is a timer (`0:42`),
a small waveform, and one action: **End**. After `Ended` or `Failed` that action becomes **`Start
again`** / **`Try again`**, and the reason stays until you start again or type.

When the window is narrow the row gives things up in a fixed order: the **model name** shrinks and
then goes, then the **waveform** shortens and goes, then the **context ring** goes. The state word, the
microphone, the timer and the end action never shrink, never wrap and never turn into an ellipsis, down
to a 240 px floor.

## Starting, moving and ending

- **Start** — press `Live voice` in the composer. The row appears and the session begins.
- **Move** — starting a call in another conversation, or with another agent, does not fail and does not
  open a second one: the call **moves** there, and the conversation it left says why.
- **End** — the `End` action in the row, or closing the tab. Both release the microphone.
- **End by itself** — silence for `idle_stop_seconds`, the agent or conversation being deleted, the
  microphone taken away by the browser or the operating system, or the connection staying down past
  30 s. Every one of those leaves a readable reason in the row.

There is **at most one live session per person**, across every conversation and every agent.

## Interrupting

**By speaking.** Talk over the agent and it stops — not when you start talking, but when you stop:
the reply keeps playing while you speak, and once `end_of_utterance_ms` of silence has passed the audio
stops and the running turn is cancelled. That is deliberate; cutting the agent off at your first
syllable would make a cough stop an answer.

**There is no interrupt button in this release.** It is deferred, and the row does not show a control
that does not work. Where speaking over the agent does not work — a loud room, a poor microphone, a
quiet voice — the only way to stop a reply is `End`.

## What audio goes where

| Leg | Where it goes |
|---|---|
| Your microphone | The recogniser configured on the agent (`stt_settings`, or the `live_voice` override) |
| The reply's text | The voice configured on the agent (`tts_settings`, or the `live_voice` override) |
| The audio itself | The conversation's own WebSocket, as base64 PCM16 — 16 kHz, mono, 100 ms per chunk |

The sound travels on the connection the conversation already has: no second connection, no relay
server to run, no separate media port to open. Nothing is sent anywhere the agent's providers were not
already sending. Echo cancellation, noise suppression and automatic gain control are asked of the
browser's microphone, so a reply heard through speakers does not come back as a new utterance.

## From another client

Live voice is not tied to this WebUI. A system of your own can hold a session against the same server,
with a key that can do **only** this:

```http
POST /api/v1/auth/api-keys
Authorization: Bearer <jwt>
Content-Type: application/json

{ "name": "call-centre", "capabilities": ["live_voice"] }
```

A key made with `capabilities: ["live_voice"]` can open a live voice session and **nothing else** — it
does not inherit the owner's role or permissions, so it cannot read another conversation, another
agent's history, settings, memory or files. A key made **without** `capabilities` is what the API keys
page has always made: it carries its owner's full permissions. Hand the scoped one to an outside
system; never the other.

The protocol a client implements — the frames, the lifecycle, the audio description and the reason
words — is `specs/055-live-voice/contracts/live-voice-frames.md`. The audio is base64 PCM16, 16 kHz,
mono, in 100 ms chunks; a client that declares anything else is refused rather than converted.

## What is stored

- **Your words** become an ordinary message in the conversation, marked as spoken.
- **The reply** is stored as an ordinary agent message, whether it finished or was interrupted. An
  interrupted reply stays visible, marked as interrupted, so you can read what you did not hear.
- **No audio is stored.** Neither your microphone nor the reply is written to disk.
- Ending a session for any reason never deletes, rewrites or reorders the conversation's text. A reply
  still being written when you end finishes as **text** and is not spoken.

## Limits

| Limit | Value |
|---|---|
| Silence before a session ends itself | `idle_stop_seconds`, default 180 s |
| One utterance | `max_utterance_seconds`, default 60 s |
| Silence that closes an utterance | `end_of_utterance_ms`, default 800 ms |
| A dropped connection may stay open | 30 s |
| Sessions at once | One per person, across every conversation and agent |
| Total session length | No cap; the idle stop is what ends a quiet call |

## When it will not start

| Reason in the row | What to fix |
|---|---|
| `a recogniser is not configured for this agent` | Add `stt_settings` (or a `live_voice` `stt` override) |
| `a voice is not configured for this agent` | Add `tts_settings` (or a `live_voice` `tts` override) |
| `no microphone could be opened` | No input device, or the browser has none to offer |
| `the microphone was taken away` | Permission withdrawn mid-session; grant it and press `Try again` |
| `this release carries 16 kHz mono audio` | A client sent audio in a format this build does not carry |

## The old speech buttons

The composer's record control and spoken-reply toggle are **no longer shown**. Live voice replaces
them on screen. Their code, providers and settings remain in the build — a different surface may use
them later — so an agent that had them configured keeps that configuration untouched.

## Measuring it

The client logs one line per turn when the recorder is switched on:

```text
live_voice_metric turn=3 end_of_utterance_to_audible_ms=1420.5 end_of_utterance_to_silence_ms=12.3
```

The clock starts when your utterance closes on the client and stops at the first sample that is
actually audible, so it measures what you hear rather than what the network did. The recorder only
records: there is no threshold that fails a build. The marks, the ring size and what is not measured
yet are written down in `specs/055-live-voice/latency.md` in the repository.

