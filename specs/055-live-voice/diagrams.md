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
        CAPTURE["capture.ts<br/>100 ms chunks"]
        SESSION["session.ts<br/>state machine + generation"]
        PLAYER["player.ts<br/>AudioContext queue"]
        SPEAKERS["Speakers"]
        UI["LiveVoiceButton<br/>LiveVoiceBar + interrupt"]
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
    SESSION -->|"live_voice_audio, utterance_end"| SOCKET
    SOCKET --> LIVE --> UTTER --> STT
    UTTER -->|"transcript as a normal request"| TURN
    TURN -->|"Message frame"| SEGMENT --> ENGINES --> TTS
    ENGINES -->|"live_voice_speech frames"| SOCKET
    SOCKET --> SESSION --> PLAYER --> SPEAKERS
    VAD -->|"speech_start while the reply is audible"| SESSION
    SESSION -->|"live_voice_interrupt"| SOCKET
```

Reading it: the person's audio never leaves the client un-chunked, the transcript enters the ordinary
turn path (so tools, memory, permissions and accounting are the same), and the reply's text is
segmented on the server before any synthesis. The generation counter, held in `session.rs`, is what
makes the two interruption paths converge on one mechanism.

## 2. Sequence — barge-in (the person speaks over the agent)

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
    C->>C: player.stop() in under 50 ms
    C->>S: live_voice_speech_start utterance 42
    S->>L: speech_start
    L->>L: generation becomes 8, synthesis abandoned
    L->>T: abort the running turn
    T-->>L: turn ends, text so far saved, marked interrupted
    L-->>S: live_voice_state interrupted
    S-->>C: live_voice_state interrupted
    V-->>L: PCM for segment 4, stale gen 7
    L->>L: dropped, generation is 8
    C->>C: any late gen 7 frame is dropped too
    P->>C: stops speaking
    C->>S: live_voice_utterance_end utterance 42
    S->>L: utterance_end
    L->>L: recognise utterance 42
    L->>T: new request, transcript as its content
    Note over C: state returns to waiting for the agent
```

The two places the audio dies: on the client immediately (that is what the person experiences,
SC-002's 300 ms is a ceiling, not a target), and on the server by generation, which is what stops the
next segment from ever being sent.

## 3. Sequence — the interrupt control

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

The difference from barge-in is only who started it and what follows: the control submits nothing, so
the session returns to `listening` and waits, which is why it works in a loud room or on a bad
microphone where speaking over the agent does not.

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
    speaking --> interrupted: person speaks or presses interrupt
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

## 5. Wireframe — the controls (design gate)

```text
+----------------------------------------------------------------------+
| Conversation: Deploy notes                              [agent ▾]     |
+----------------------------------------------------------------------+
|                                                                      |
|  ... the conversation, ordinary messages, voice turns read the same  |
|      [ you · spoken ]  what is the disk usage on the build host?     |
|      [ agent ]  It is 61 percent used, and the largest ...           |
|                (marked: interrupted at 14:32)                        |
|                                                                      |
+----------------------------------------------------------------------+
| LIVE VOICE  ● listening        [ Interrupt ]           (bar, only     |
|                                                         while live)   |
+----------------------------------------------------------------------+
| [ type a message...                                     ] [mic][tts] |
|                                   [ ⏻ Live Voice ]  <- the one control|
+----------------------------------------------------------------------+
```

* The **Live Voice** control sits apart from the microphone and the "ask for a spoken reply" toggle,
  because it is a different capability (FR-001, FR-003). Off by default, on every visit.
* The bar above the composer exists **only** while a session lives: it carries the state word from
  FR-006, the always-visible **Interrupt** control (FR-020), and the microphone-open indicator
  (FR-038). When the session ends it is replaced by one line of reason.
* The interrupt control is not a second Live Voice control: pressing it never ends the session
  (FR-020, FR-021).

