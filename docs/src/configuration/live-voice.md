# 2.23 Live Voice

Live voice is a spoken conversation with an agent, held open: you talk, the agent answers aloud, and
you can talk over it to cut it short. It is a capability of its own, separate from the `stt` and `tts`
tools — those transcribe a file and synthesise a file; live voice runs the whole loop.

There is no switch for it. An agent offers live voice whenever it can both hear and speak: its
**Speech-to-Text** and **Text-to-Speech** are both turned on (the agent's Models page). Turn either off
and the voice icon goes from the composer.

## Using it

The call uses the agent's own Speech-to-Text and Text-to-Speech providers — see
[Tools & Embedding](./tools-embedding.md) for the provider tables. Credentials are the ones already
saved for those providers; live voice asks for none of its own.

Through `/api/v1/agents` an agent may also carry a `tools.live_voice` block, for the call's timings and,
optionally, a recogniser or voice that differs from the tools'. Its `enabled` flag is no longer read.

```json
{
  "tools": {
    "live_voice": {
      "settings": {
        "end_of_utterance_ms": 1400,
        "stt": { "provider": "whisper", "model": "ggml-large-v3", "language": "en" },
        "tts": { "provider": "openai", "voice": "nova" }
      }
    }
  }
}
```

| Setting | Default | What it does |
|---|---:|---|
| `idle_stop_seconds` | `180` | A session with nothing said for this long ends itself |
| `max_utterance_seconds` | `60` | One utterance is capped here; the server enforces it |
| `end_of_utterance_ms` | `1400` | How long a silence closes what you were saying (100–2000) |

## The call

With the message box empty, the composer's Send button shows a **voice icon**: press it to start a
call (typing anything turns it back into Send). The call is a **pill in the composer's status strip**,
ahead of the counts, and the composer itself never moves:

| The pill says | What is happening |
|---|---|
| `Connecting` | The microphone is being opened; the browser may ask for permission now |
| `Listening` | Open and waiting for you — also while the agent works on what you said; the conversation's activity trail shows that work, as for any message |
| `Speaking` | The reply is being read aloud; speak to cut it short |
| `Interrupted` | You cut the reply short; the call is still listening |
| `Reconnecting` | The connection dropped; it stays open for 30 s before giving up |
| `Call ended` / `Call failed` | Over, with a short reason, **Start again** and a close |

Beside the word: a level meter, the time (`0:42`) and a stop button. On a phone the meter gives way so
the strip's counts still fit.

## How speech is heard

The browser listens with a loudness gate over a slowly learned room level, every 20 ms. About a
quarter of a second of voice opens what you are saying, so a click, a cough or a chair does not;
the moments before it are kept and sent with it. A pause of `end_of_utterance_ms` (1.4 s) ends it.
While the agent is speaking you interrupt it by talking over it for about half a second; anything
shorter is ignored and the reply keeps playing. It cannot tell voices apart: a second person in the
room is heard like you.

## Starting, moving and ending

- **Start** — press the voice icon in the composer (the message box must be empty). The pill appears and the call begins.
- **Move** — starting a call in another conversation, or with another agent, does not fail and does not
  open a second one: the call **moves** there, and the conversation it left says why.
- **End** — the stop button in the pill, or closing the tab. Both release the microphone.
- **End by itself** — silence for `idle_stop_seconds`, the agent or conversation being deleted, the
  microphone taken away by the browser or the operating system, or the connection staying down past
  30 s. Every one of those leaves a readable reason in the pill.

There is **at most one live session per person**, across every conversation and every agent.

## Interrupting

**By speaking.** Talk over the agent and it stops — not when you start talking, but when you stop:
the reply keeps playing while you speak, and once `end_of_utterance_ms` of silence has passed the audio
stops and the running turn is cancelled. That is deliberate; cutting the agent off at your first
syllable would make a cough stop an answer.

**There is no interrupt button in this release.** It is deferred, and the pill does not show a control
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

A key made with `capabilities: ["live_voice"]` acts as its owner, but it reaches **one place**: the
conversation socket, `/api/v1/agents/{agent}/channel/{channel}/topic/{topic}/chat`, and there it may
send live voice frames only. Every other route answers `403`, and a typed message on the socket is
refused with `live_voice_only`, so it cannot read settings, memory or files, or type to the agent. It
reaches the agents its owner can reach. A key made **without** `capabilities` is what the API keys page
has always made: it reaches everything its owner can. An empty or unknown capability is refused when
the key is made. Hand the scoped one to an outside system; never the other.

The protocol a client implements — the frames, the lifecycle, the audio description and the reason
words — is `specs/055-live-voice/contracts/live-voice-frames.md`. The audio is base64 PCM16, 16 kHz,
mono, in 100 ms chunks; a client that declares anything else is refused rather than converted.

## What is stored

- **Your words** become a voice message in the conversation, as one recorded with the mic would:
  the recording appears as soon as you stop speaking, with *Transcribing* where *Show transcript*
  will be, and the transcript fills in when recognition finishes. An utterance that was too short or
  could not be understood is never sent, and its bubble goes.
  If the agent is still answering something else in that conversation (a typed message, say), your
  words wait for their own turn rather than joining the running one, so their answer is spoken.
- **The reply** is stored as an ordinary agent message, and reads like a spoken reply: while it is
  being said it is already a player waiting for its recording, with the text behind *Show transcript*;
  once spoken the player plays what you heard (up to where you cut in, if you interrupted).
- Nothing is said that the thread does not show: there are no filler words while the agent thinks.
- **Audio is stored.** Each utterance and each reply heard in full is saved with the conversation, like
  any voice message, and goes when the conversation is deleted.
- Ending a session for any reason never deletes, rewrites or reorders the conversation's text. A reply
  still being written when you end finishes as **text** and is not spoken.

## Limits

| Limit | Value |
|---|---|
| Silence before a session ends itself | `idle_stop_seconds`, default 180 s |
| One utterance | `max_utterance_seconds`, default 60 s |
| Silence that closes an utterance | `end_of_utterance_ms`, default 1.4 s |
| A dropped connection may stay open | 30 s |
| Sessions at once | One per person, across every conversation and agent |
| Total session length | No cap; the idle stop is what ends a quiet call |

## When it will not start

| Reason in the pill | What to fix |
|---|---|
| `no recogniser set up` | Add `stt_settings` (or a `live_voice` `stt` override) |
| `no voice set up` | Add `tts_settings` (or a `live_voice` `tts` override) |
| `microphone is blocked` | The browser refused the microphone; allow it and press *Start again* |
| `no microphone found` | No input device, or the browser has none to offer |
| `microphone was taken away` | Permission withdrawn mid-call; grant it and press *Start again* |
| `needs 16 kHz mono audio` | A client sent audio in a format this build does not carry |

## In the composer

While the message box is empty, Send shows the voice icon: press it to start a call. Typing anything
turns it back into Send. A call is a pill in the composer's status strip (state, level, time, stop);
what the agent is doing meanwhile shows in the conversation's activity trail like any turn. The
dictation mic and the spoken-reply toggle stay as they were.

## Measuring it

The client logs one line per turn when the recorder is switched on:

```text
live_voice_metric turn=3 end_of_utterance_to_audible_ms=1420.5 end_of_utterance_to_silence_ms=12.3
```

The clock starts when your utterance closes on the client and stops at the first sample that is
actually audible, so it measures what you hear rather than what the network did. The recorder only
records: there is no threshold that fails a build. The marks, the ring size and what is not measured
yet are written down in `specs/055-live-voice/latency.md` in the repository.

