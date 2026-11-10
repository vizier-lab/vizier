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

