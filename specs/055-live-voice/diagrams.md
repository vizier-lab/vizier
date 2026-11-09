# Diagrams: Live Voice Conversation

**Feature**: `specs/055-live-voice` | **Date**: 2026-11-08

Three diagrams the specification and the plan refer to: the component architecture, the two
interruption sequences, and the session's states. The wireframe at the end is the "design before UI
code" gate (constitution, Development Workflow) — it is deliberately crude; it fixes placement and
state visibility, not styling.

## 1. Component architecture

```mermaid
flowchart LR
    subgraph Client["Browser or desktop WebView"]
        MIC["getUserMedia<br/>echoCancellation on"]
        WORKLET["Capture worklet<br/>PCM16 16 kHz mono"]
        VAD["Energy VAD<br/>adaptive floor, 800 ms hangover"]
        CAPTURE["capture.ts<br/>the sound's client end"]
        SESSION["session.ts<br/>state machine + generation"]
        PLAYER["player.ts<br/>AudioContext queue"]
        SPEAKERS["Speakers"]
        UI["LiveVoiceButton<br/>LiveVoiceRow (the composer's meta line) + End"]
    end
    subgraph Server["Vizier server"]
        SOCKET["Topic WebSocket<br/>channel.rs"]
        LIVE["live_voice/session.rs<br/>registry + generation"]
        UTTER["utterance.rs<br/>PCM to WAV"]
        ENGINES["engines.rs<br/>LiveVoiceEngines"]
        SEGMENT["segmenter.rs<br/>reply text to segments"]
        STT["VizierStt<br/>batch transcribe"]
        TTS["VizierTts<br/>batch synthesis"]
        TURN["Agent turn<br/>existing pipeline"]
    end
    MIC --> WORKLET --> VAD --> CAPTURE --> SESSION
    UI --> SESSION
    SESSION -->|"the sound"| AUDIO["Audio path<br/>socket today — Q11 decides"]
    AUDIO -->|"live_voice_audio, utterance_end"| SOCKET
    SOCKET --> LIVE --> UTTER --> STT
    UTTER -->|"transcript as a normal request"| TURN
    TURN -->|"Message frame"| SEGMENT --> ENGINES --> TTS
    ENGINES -->|"live_voice_speech frames"| SOCKET
    SOCKET -->|"the sound (Q11)"| SESSION
    SESSION --> PLAYER --> SPEAKERS
    VAD -->|"speech_start while the reply is audible"| SESSION
    SESSION -->|"live_voice_utterance_end when they stop"| SOCKET
```

Reading it: **the box marked "Audio path" is the only part Q11 can move.** Today the sound leaves as
100 ms base64 chunks on the topic socket; if the Lead's numbers favour a media transport, that one box
becomes a peer connection and nothing else in this picture changes — which is the whole point of
writing FR-054 as a shape rather than as plumbing. The transcript enters the ordinary turn path (so tools, memory, permissions and accounting are the same), and the reply's text is
segmented on the server before any synthesis. The generation counter, held in `session.rs`, is what
makes an interruption — now a single path, the end of the person's utterance (spec C8) — safe against
segments already in flight.

## 2. Sequence — barge-in (the person finishes speaking; spec C8)

```mermaid
sequenceDiagram
    autonumber
    actor P as Person
    participant C as Client
    participant S as Topic socket
    participant L as Live voice session
    participant T as Agent turn
    participant V as Voice provider

    Note over L: generation 7, segment 3 of 9 playing
    V-->>L: PCM for segment 3
    L->>S: live_voice_speech gen 7 seq 3
    S->>C: live_voice_speech gen 7 seq 3
    C->>C: schedule segment 3 on the AudioContext
    P->>C: starts speaking, "no, the other file"
    C->>S: live_voice_speech_start utterance 42
    Note over C,L: nothing stops: the agent keeps talking while the person talks (C8)
    V-->>L: PCM for segment 4
    L->>S: live_voice_speech gen 7 seq 4
    S->>C: live_voice_speech gen 7 seq 4
    Note over C: the reply is audible under the person's voice, and the detector still has to hear them finish (FR-047)
    P->>C: stops speaking
    Note over C: 800 ms of quiet on the microphone: the hangover fires
    C->>C: player.stop() in the same tick
    C->>S: live_voice_utterance_end utterance 42
    S->>L: utterance_end
    L->>L: generation becomes 8, synthesis abandoned
    L->>T: abort the running turn
    T-->>L: turn ends, text so far saved, marked interrupted
    L-->>S: live_voice_state interrupted
    S-->>C: live_voice_state interrupted
    V-->>L: PCM for segment 5, stale gen 7
    L->>L: dropped, generation is 8
    C->>C: any late gen 7 frame is dropped too
    L->>L: recognise utterance 42
    L->>T: new request, transcript as its content
    Note over C: state returns to waiting for the agent
```

The two places the audio dies, and they are now both at the **end** of the person's speech: on the
client in the tick the hangover fires (that is what the person experiences; SC-002's 300 ms is a
ceiling, not a target, measured from that tick), and on the server by generation, which is what stops
the next segment from ever being sent. Before that tick the audio deliberately plays on — that is C8,
not a bug.

## 3. Sequence — the interrupt control (deferred to the backlog, spec C7)

```mermaid
sequenceDiagram
    autonumber
    actor P as Person
    participant B as Interrupt control
    participant C as Client
    participant S as Topic socket
    participant L as Live voice session
    participant T as Agent turn

    Note over C: state speaking, generation 4
    P->>B: press interrupt
    B->>C: interrupt()
    C->>C: player.stop(), queued segments discarded
    C->>S: live_voice_interrupt reason button
    S->>L: interrupt
    L->>L: generation becomes 5
    alt a turn is still running
        L->>T: abort on the existing stop path
        T-->>L: partial text saved, marked interrupted
    else nothing was being spoken
        L->>L: state unchanged, session stays live
    end
    L-->>S: live_voice_state interrupted
    S-->>C: live_voice_state interrupted
    Note over C: microphone stays open, no restart, no new press
```

**None of this ships in this release.** There is no interrupt control (spec C7): the sequence is kept so
the backlog item does not have to rediscover it. When it lands, the button goes on the live row, and
the difference from barge-in stays as described: the control submits nothing, so the session returns to
`listening` and waits — which is why it is worth having in a loud room or on a bad microphone, where
speaking over the agent does not work.

## 4. Session states

```mermaid
stateDiagram-v2
    [*] --> off
    off --> connecting: press the control
    connecting --> listening: microphone open, engines ready
    connecting --> failed: no device, permission refused, engine missing
    listening --> waiting: utterance submitted
    waiting --> speaking: first segment ready
    waiting --> listening: turn ended with nothing to say
    speaking --> listening: reply ends
    speaking --> interrupted: the person finishes speaking (C8)
    interrupted --> listening: nothing was said to keep
    interrupted --> waiting: the interrupting words became a message
    listening --> ended: idle, page closed, agent gone
    speaking --> ended: page closed, connection lost
    failed --> off: reason shown
    ended --> off: reason shown
    off --> [*]
```

Every arrow that leaves `listening` or `speaking` for `ended` releases the microphone (FR-028,
FR-034, SC-007). `interrupted` is a state, not an end: the session is still live and still listening.

## 5. Wireframe — the live row (design gate, spec C11)

The surface has changed five times and each change is archived rather than deleted: a **bar in the
composer** (`20d97316`), a **full screen** (C6), a **draggable floating panel** (C9), a **rail above the
composer** (B2, drawn and not chosen), and now **the composer's own meta line** (C11 as amended 17:31,
surface **B1-1**). Only the last is a design gate; the others are history that explains why each of the
row's rules exists.

```text
IDLE — today's composer, unchanged. The only addition is the pill.
+---------------------------------------------------------------+
| Deploy notes                                        [agent ▾] |
+---------------------------------------------------------------+
|   you   what is the disk usage on the build host?             |
|   agent It is 61 percent used, and the largest ...            |
|                                                               |
|   ... the conversation, full width, nothing covered           |
|                                                               |
+---------------------------------------------------------------+
| [ type a message...                              ] [attach]   |
| (18px) [mic] Live voice   opencode-go/deepseek-v4.1-flash     |  <- meta line, 24 px
+---------------------------------------------------------------+

A SESSION — the same line, taken over. Nothing moves; 0 px added.
+---------------------------------------------------------------+
| [ type a message...                              ] [attach]   |
| speaking  ·  mic on  ~~~~~~~~~~  0:24  [ End ]  ...flash      |  <- the live row
+---------------------------------------------------------------+
```


States the row must be able to show (FR-006), with the thinking state visibly different from the
reconnecting one (FR-042):

```text
connecting      Connecting…            ● mic open
listening       Listening              ● mic open
waiting         Thinking · 3 dots      ● mic open     <- FR-042, never confused with the line below
speaking        Speaking               ● mic open
interrupted     Interrupted            ● mic open
reconnecting    Reconnecting…          ● mic open     <- "gone", not "thinking"
ended           Ended · idle for 3 minutes              <- reason stays, no timer, no action but End
failed          Failed · no voice configured
```

* The row **is** the composer's meta line — 24 px, **0 px added** to a 156 px composer — and covers
  nothing (FR-043, FR-049). There is no drag handle: there is no position (FR-048 superseded).
* **What it gives up when space is short**, in this order and no other (FR-049): the **model chip**
  shrinks to 96 px and then goes (cut to its last segment), the **waveform** 39 → 12 bars → gone, the
  **context ring** goes whole. **Never** the microphone indicator (FR-038), the state word (FR-042), the
  elapsed time or the End action — all `nowrap`, and no ellipsis ever in the state word. Floor **240 px**;
  a row that wraps to two lines has become a panel again — the failure mode to watch in review.
* **The idle line is not the session line.** Before a session the meta line carries the context ring and
  the model chip, and the only addition is one pill: `[mic] Live voice`, labelled rather than a bare
  microphone glyph, because a bare glyph in that slot reads as the old record-once-and-send control.
  After `ended`/`failed` the line goes back to exactly that.
* There is **one action on the row: End** (FR-045), and no second control that means the same thing.
  No interrupt button ships in this release (spec C7); the interruption is done by speaking, so the row
  does not hint at a control that does not exist.
* `ended` and `failed` keep their reason in the row with no five-second timer and no action but End —
  the commonest ending is the idle stop, when the person has already walked away.
* **One call, one row.** Starting a call in another conversation moves it there: this row shows the
  reason `moved` and disappears, the other conversation's row appears, and the running turn is
  cancelled rather than spoken into the new call (FR-050, FR-051, FR-052, SC-011).
* The composer's own recording control and spoken-reply toggle **retire** with C12 (FR-053); during
  the overlap they may still be drawn, but they are no longer the design's subject.


