# Latency: how it is measured, and what is not

Companion to `specs/055-live-voice/spec.md` (SC-001, SC-002) and `tasks.md` (T076, T077). This page
records **what is instrumented today**, **how to read it**, and — in a section of its own — **what is
not instrumented yet**. It is written after the code, not before it, so that nothing here describes a
measurement that does not exist.

## The two marks, and why they are where they are

Both criteria are measured from the same event: **the person's utterance closing on the client**, which
is the 800 ms hangover firing in `webui/app/lib/liveVoice/vad.ts`. Not the arrival of a frame from the
server — that would measure the network's answer rather than what the person hears.

| Criterion | Clock starts | Clock stops | Where |
|---|---|---|---|
| **SC-001** — end of utterance → first audible word | The hangover fires, before the audio is stopped (`session.ts:377`) | The first sample is **actually audible** | `session.ts:401-403`, using `player.ts:93-94` |
| **SC-002** (client leg) — end of utterance → silence | The same instant | `stopAudio()` returns, in the same tick (`session.ts:380`) | `session.ts:380` |

The second half of SC-001 is not a stopwatch reading and cannot be taken in `session.ts`. `player.ts`
knows three things the session does not: when the buffer was scheduled to start (`startAt`, the value
handed to `source.start()`), and the audio context's own `baseLatency` and `outputLatency`. It returns
`(startAt − now) × 1000 + (baseLatency + outputLatency) × 1000`; the session adds that to its own
elapsed time. That is why a reply whose first segment was queued ahead reports a smaller number than
one synthesised on the spot, and why the number is honest about a scheduled start instead of pretending
the audio began when the frame arrived.

## Reading it

The recorder is **off by default and costs nothing when off**: one boolean read per turn, no object
allocated, nothing logged. Switched on, it prints one line per turn:

```text
live_voice_metric turn=3 end_of_utterance_to_audible_ms=1420.5 end_of_utterance_to_silence_ms=12.3
```

The name `live_voice_metric` and both field names are the contract `quickstart.md` tells a measurement
run to read. The last **200** turns are kept (`LIVE_VOICE_RING`), which is a ring, not a database: a
smoke run needs three turns, a release run wants twenty, and 200 is more than either.

`liveVoiceReport()` returns `{ n, firstAudible: {p50, p90, p99}, silence: {…} }`. Percentiles use
**linear interpolation**, not nearest-rank: with three turns in a smoke run the difference between
those two methods is the difference between a number and a cliff, and a mean would hide the tail this
exists to expose.

To measure: switch the recorder on (`setLiveVoiceMetrics(true)`), hold a conversation, then read the
report or the console. Nothing else has to be enabled.

## These are not gates

**No assertion anywhere fails a test because a percentile is over a threshold.** That is a decision, not
an omission: there is no agreed minimum sample count and no baseline from a real provider yet, so a
gate today would either pass by luck or fail without meaning. A recorder that fails builds is a worse
recorder.

The thresholds the criteria name, quoted here so whoever turns them into a gate has them to hand:

| Criterion | Threshold as written |
|---|---|
| SC-001 | p90 ≤ 3.5 s, p99 ≤ 5 s |
| SC-002 | ≤ 300 ms in at least 95% of turns |

Turning them into a gate needs two things first: a minimum sample count, and samples from a real
provider rather than a fake. Both are open.

## What is not instrumented yet

**The server half of SC-002.** The criterion is "the `Abort` reaches the agent within 300 ms". The
client measures how long *stopping the audio* took; it cannot see the server. `tasks.md` T077 also asks
for `tracing` spans in `crates/vizier-core/src/live_voice/session.rs` at end of utterance,
recognition, turn start, first segment and first frame, and that is **not built**. Until it is, the
server-side stages are unmeasured and no number here should be read as covering them.

**Anything end to end against a real recogniser and voice.** Every figure produced so far comes from
unit tests with injected clocks. No measurement run has been made against a hosted provider or a local
model, because this environment has neither. SC-003's interrupt-button timing is out of scope entirely:
that control is deferred (C7) and is not measured in this release.

## Where the code is

| File | What it holds |
|---|---|
| `webui/app/lib/liveVoice/metrics.ts` | The recorder: ring, percentiles, report, the log line, the on/off switch |
| `webui/app/lib/liveVoice/session.ts` | The two marks, and the per-turn sample |
| `webui/app/lib/liveVoice/player.ts` | When a sample is actually audible, and the context's own latency |

