# FR-029 / option A — how to verify it without a microphone

**Owner: `@teteh-qa`. Status: PLAN, nothing run.** User chose **A** (server closes an idle call) on
2026-11-10 10:20. This file exists because two of the five conditions are invisible from the outside:
you cannot tell "the timer was not reset on this path" by watching the bar, and you cannot tell "the
timer kept counting during a turn" until an answer gets cut in half. Both need a clock we control.

```
baseline:  parent af9954bc · webui 510ace8 (+ session.ts dirty, N1 in flight) · design 021ab46
           session.rs dirty: T110 r2, `end_one` — not committed
evidence:  [QA] = I read the line myself today · [design] = stated by the implementer, not re-measured
ran:       nothing. No cargo, no server, no microphone. This is the plan, not a result.
```

---

## 0. The one design decision that decides whether any of this is verifiable

An idle timer of 180 s implemented on the **real** clock cannot be tested: every case costs three
minutes of wall time, so in practice nobody runs more than one, and "the timer resets on every path"
stays a claim.

**The repo already has the pattern, with a passing test.** `crates/vizier-core/src/channels/side_chat.rs`
ends idle exchanges the same way A has to:

| What it does | Where |
|---|---|
| a lifetime on the stored object | `Lifetime { idle: Duration }`, `:239` |
| one pass over everything that is past it | `fn sweep(now: tokio::time::Instant)`, `:390` |
| a task that calls it on an interval | `fn spawn_sweeper(every, on_end)`, `:438` |
| a test that moves the clock instead of waiting | `tokio::time::advance(...)`, `:735-755` |

So the requirement I would put in the brief, before a line of A is written:

> **The deadline must be compared against `tokio::time::Instant`, never `std::time::Instant`, and the
> sweeper must take `now` as an argument** (`sweep(now)`), the way `SideChats` does. Not a
> `tokio::time::sleep(idle)` buried inside a per-session task — that sleeps but cannot be steered.

With that shape, every case below is a `#[tokio::test(start_paused = true)]` of a few milliseconds.
Without it, this document degrades to "wait three minutes, nine times", and will not be executed.

## 1. A should walk through the door that already exists

`session.rs:215` `end(reason)` is **idempotent** (`stopped.swap(true)`) and publishes `ended{reason}`.
`session.rs:738-756` `end_one(conversation, session, reason)` **[design — uncommitted, T110 r2]** adds the
two things an ending by force needs: `abort_turn(BargeIn)` first (or the agent keeps writing an answer
for a call that is over) and `remove` from the registry.

**A = one sweeper + `end_one(..., EndReason::Idle)`.** No new ending path, no second publish, no extra
`abort`. Anything else re-opens the bug `end_one` was written to close, and we have already paid for one
lesson about a second release path (C2).

**That answers the ordering question Lead had to decide:** A depends on `end_one` landing, so **T110 r2
commits first**. Building A on `end(reason)` alone would ship an idle ending that leaves the agent
writing into a conversation nobody is in.

---

## 2. The five conditions, each with the thing that proves it

| # | Condition | Command | Expected | How you know it failed | Mic? |
|---|---|---|---|---|---|
| 1 | `idle_stop_seconds` has a production reader | `grep -rn 'idle_stop_seconds' crates/ --include=*.rs` | at least one hit **outside** `schema/agent.rs` and `service/agents.rs` — i.e. the sweeper/registry reads it | every hit is still a field, a default, or an assertion. Today: 5 hits, all that | no |
| 2 | the deadline is reset on **every** path that means activity | `grep -n 'fn touch\|last_active\|last_speech_at' crates/vizier-core/src/live_voice/session.rs` | one function, called from each path in §3 — and a test per path | a path in §3 with no call site; or the reset written inline in five places (one will be missed) | no |
| 3 | `ended{idle}` really reaches the client, and the mic closes | unit test reading the lane: `(len, seq, final)` helper at `session.rs:1401`, or `state` frames | a `state` frame with `state: "ended"`, `reason: "idle"` | only `EndReason::Idle` exists in `as_str` (`frame.rs:111`) and in a fixture (`:420`) — that is where we are today | no |
| 4 | the clock does not run while the agent is working | paused-time test: `idle = 300 ms`, fake voice `delay = 700 ms`, `advance(3 s)` | **no** `ended` while busy; exactly one after the answer | an `ended` mid-turn, or a second `ended` after it | no |
| 5 | N1: `connecting` has an exit | `grep -n 'CONNECTING_DEADLINE_MS\|abandonConnect' webui/app/lib/liveVoice/session.ts` + `grep -n 'connectDeadline' webui/app/lib/liveVoice/session.test.ts` | both non-empty | code without a test — **which is where we are right now** (§5) | no |

All five are answerable without a microphone. That is deliberate: the microphone is only needed to
confirm the *reason a person sees*, which item 3 already proves at the frame level.

---

## 3. Condition 2 is the one that breaks silently — so here is the enumeration

A missed reset has no error. It shows up as a session ending while something was happening. The set of
"something is happening" events in `session.rs`, and the symptom of forgetting each one:

| Event | Where | If the reset is missing |
|---|---|---|
| session starts | `start` / `LiveVoiceSessions::start` `:611` | `last_active` is unset → the first sweep ends a call that just began |
| audio chunk arrives | `push_audio` `:309` | a person talking for a long time is **cut mid-sentence**, with `mic on` on screen — the worst of the five |
| utterance opens | `open_utterance` `:300` | same as above, one window earlier |
| utterance closes | `close_utterance` `:337` | the transcript is being recognised when the sweep fires; `ended` and a submitted message race |
| turn accepted | `run_turn` `:512` (`busy = true`) | the agent working through tools is cut mid-turn — this is condition 4 seen from the reset side |
| a segment is sent | `speak_reply` `:437` → `send_speech` | a long answer stops mid-sentence, with the bar still showing `Speaking` |

**One function, six call sites, one test each.** If the implementer prefers a single choke point
(`publish()` at `:180` sees every control frame), say so explicitly — but then say what it misses:
`send_speech` does not go through `publish` (it writes the per-connection lane), so audio-out would not
count as activity. That trade-off has to be written down, not discovered later.

**Two more edges, both cheap to state now:**

- **`idle_stop_seconds: 0`.** Today 180 is the default and nothing validates. With a reader in place, `0`
  means "end immediately" and `5` means "end while someone is reading the answer". The client clamps its
  hangover to 100..2000 (`vad.ts:24-36`) while the server sends `end_of_utterance_ms` raw — that mismatch
  is already a finding of mine (V1). **Do not repeat it here**: either clamp on the server or refuse the
  value with a named message. Decide, do not inherit.
- **After `stopped`.** The sweeper must skip sessions whose `is_stopped()` is true (`:169`) and must not
  keep them in the map (`remove` is already called by `end_one`), otherwise every tick re-publishes.

---

## 4. Condition 4 in full — the test that proves the clock pauses

Not "does the timer exist", but "does it count the wrong thing":

```
given:  settings.idle_stop_seconds = 0.3 s   (a test value, not the 180 s default)
        a voice engine whose speak() takes 0.7 s per segment
        a reply of two segments  (the existing helper at :1401 reads the lane)

when:   run_turn() is awaited under a paused clock, advancing to 0.4 s, 0.8 s, 1.4 s

then:   no `ended` frame at 0.4 or 0.8 — the turn is busy
        1.4 s = 0.7 s after the last segment is *sent*, so `ended{idle}` appears exactly once
        and the reason is `idle`, not `pressed_again` (set_state defaults to PressedAgain, `:187-196`)
```

Two ways to fail it, both real: a timer that counts from **turn start** cuts a slow answer; a timer that
resets on turn start but not on the last segment ends the call the instant the reply finishes, which is
the one moment the person is most likely still listening.

The `set_state` detail at `:187-196` is worth a line of its own: it stamps `Ended` with
`PressedAgain` when called directly. So A **must** call `end(Idle)` (`:215`), not `set_state(Ended)`.

---

## 5. N1 is no longer "no door" — it is "door, no test"

Updated against the tree at `af9954bc`, not against my morning note:

- `CONNECTING_DEADLINE_MS = 30_000` (`session.ts:57`), the timer armed at `:199-200`, cleared at `:213`,
  and `abandonConnect` at `:498-502`: sends `stopFrame()` then fails.
- `grep -n 'connectDeadline' webui/app/lib/liveVoice/session.test.ts` → **0 hits.** So the door exists and
  nothing proves it opens.
- It is **uncommitted** (`webui/app/lib/liveVoice/session.ts` is `M`).

So condition 5 is now two things, not one: **commit it, and test it** — a paused/faked clock test that
asserts the session leaves `connecting` on its own, sends exactly one `stop` frame, and does not leave
`micOpen` true. And one wording note for whoever owns it: a person who ignored the permission prompt
reads *"no microphone"* (`abandonConnect` fails with `no_device`), which is not what happened — the
reason vocabulary has `permission_withdrawn` and neither says "you did not answer". That is a copy
decision, not a blocker, but it is the sentence the person sees.

---

## 6. If you want this greppable in production, add two lines — not a dashboard

The four numbers Lead has to collect (turn duration, filler, `FAST_PATH_RETRIES`, `too_short`) come from
existing lines. The idle timer has none, so a real-world "it ended too early" report would be
unanswerable. Two `tracing::debug!` calls close that:

```
per reset:  tracing::debug!(trigger = "push_audio" | "open_utterance" | ... , "live voice idle clock reset");
per sweep:  tracing::debug!(idle_for_ms = …, sessions = …, "live voice idle sweep");
```

`trigger=` is the whole point: it makes §3 a `grep -o 'trigger="[a-z_]*"' | sort | uniq -c`, so a missing
path shows as a zero count instead of a rumour. Two lines, no new metric, no dashboard.

---

## 7. What this document does not cover

| Item | Why | Who |
|---|---|---|
| the real 180 s on the wall clock | covered by the paused-clock test; a wall-clock rerun proves nothing extra | nobody, unless a bug appears |
| the microphone actually closing in a browser | needs a browser; **the client path already exists** — `applyState` releases on `ended`/`failed` (`session.ts:519-522`), so A needs **zero** client change | Lead, via one e2e when a server exists |
| a third-party client seeing `ended{idle}` | it does not exist yet (SC-012, T106) | later |
| the reason a person reads | `humanReason('idle')` already returns a sentence (`session.ts:554`) | nothing |
| whether 3 minutes is the right number | product decision, already defaulted to 180 in `agent.rs:242` | User, if he ever wants it |
---

## 8. Reading conditions 2 and 4 from the log, not from the microphone

Both are silent failures: a missing reset has no error, and a clock that runs during a turn only shows
up as an answer cut in half. So they need a line in the log, written to a shape that a script can decide
on. This section is that shape — **it is part of the brief, not a follow-up**, because a line added
afterwards is a line nobody knows how to read.

### 8.1 Three lines, exact tokens

```rust
// one per reset, at the place that resets. `trigger` is the whole point: it makes a missing call
// site show up as a zero count instead of a rumour.
tracing::debug!(trigger, idle_for_ms, session = %topic, "lv_idle reset");
// the sweep wanted to end the call and refused because a turn was running
tracing::debug!(idle_for_ms, busy = true, session = %topic, "lv_idle held");
// the sweep ended it
tracing::debug!(idle_for_ms, busy, session = %topic, "lv_idle end");
```

`trigger` takes the six names of §3 — `start`, `push_audio`, `open_utterance`, `close_utterance`,
`turn_start`, `segment_sent` — so §3 stops being prose and becomes six counters. **If the implementer
prefers one choke point instead of six call sites, that is allowed, but then the six names must still be
emitted** and the reason the choke point covers all six written down; a choke point at `publish()`
(`session.rs:180`) would *not* cover `segment_sent`, because `send_speech` writes the per-connection lane
and never calls `publish`.

### 8.2 They will actually appear — checked, not assumed

`crates/vizier/src/main.rs:71-72` sets the default filter to `EnvFilter::new("vizier=debug")`, and that
prefix matches `vizier_core::…` too. Evidence from today's run, not from reading the docs:
`grep -c 'LLM completion received' /home/muruyung/server-055test.log` → **91**, and every one of them
carries the target `vizier_core::agents::agent`. `session.rs` already emits three `tracing::debug!` of
its own (`:274`, `:397`, `:718`), so the pattern is in the file being edited.

One practical trap: the log file carries ANSI colours (`\x1b[2m…`), so a naive `grep '^2026'` or a field
parser sees escape bytes inside the fields. The reader below strips them; anyone copying a command by hand
must too.

### 8.3 The run itself: a low limit, one ordinary turn

Waiting 180 s per case is what makes this unverifiable in practice. For the measurement run only:

```yaml
tools: { live_voice: { settings: { idle_stop_seconds: 20 } } }   # test instance, not the default
```

The default stays 180 (`schema/agent.rs:242`); say in the report that 20 was used, so the log is not
mistaken for a 3-minute run. Then **one ordinary spoken turn** is enough: `start` fires on connect,
`push_audio` and `open_utterance` on the first timestamp of speech, `close_utterance` at the hangover,
`turn_start` when the agent is given the transcript, and `segment_sent` when the first audio goes out.
Six call sites, one turn — but only if the person then stays quiet for 20 s, which is the point.

### 8.4 The reader

```
/home/muruyung/.qa055/read-idle.sh /home/muruyung/server-055test.log 20000
```

Three sections: resets by trigger (condition 2), one verdict per ending (condition 4), and the held
sweeps that prove the guard was reached rather than merely written.

**It is tested on fixtures, because a parser that has never failed proves nothing.** On
`~/.qa055/fixture-idle-ok.log` (six triggers, one ending 22 s after the last reset) it prints six rows
and `t-1 OK idle_for_ms=20500 since_last_reset=22000`. On `~/.qa055/fixture-idle-bad.log` — five triggers,
no `segment_sent`, and an ending 2 s after the last reset while `busy=true` — it prints five rows and

```
t-2  FAIL  idle_for_ms=20000  since_last_reset=2000  ended while busy only 2000 ms after the last reset (limit 20000)
```

Three findings in one line, and each maps to a real defect: the missing trigger is a path that does not
reset, the short gap is the clock not being reset by the last segment, and `busy=true` is a live answer
being cut. On a log with no `lv_idle` lines it exits **3** with *"either the build predates the change, or
`RUST_LOG` hid debug"* — never "all clear".

### 8.5 What the log cannot say, and where that is covered instead

| Question | Why the log cannot answer it | Where it is answered |
|---|---|---|
| did the microphone actually close? | release is client-side | the existing client path: `applyState` releases on `ended`/`failed` (`session.ts:519-522`) |
| did the person read a sentence, not a code? | rendering | `humanReason('idle')` already returns one (`session.ts:554`) |
| was the ending *timely*, to the second? | the sweep has an interval; the log shows the sweep, not the wish | state the interval next to the number; do not claim second-precision |
| does a third-party client also stop? | it does not exist | SC-012 / T106, later |

**One number I will not accept without it:** *"the timer resets on every path"* is not a claim until
section 1 shows six rows. Five rows is a finding, and the name of the missing one is the bug.
