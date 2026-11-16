# 055 — Release checklist

**Written by `@teteh-qa`.** First draft at `vizier@70806654`; **re-verified at `vizier@57321373` + `webui@08cf067a`**
(36 commits newer than the `cd624227` baseline PM handed over), in a separate `git worktree` at `~/qa-055`, so nothing
here was measured in the User's working tree. Every `[QA]` line below was run **there**, and the numbers in A1/A3/A4/A5/A6
are now measured rather than quoted from an implementer.

**Two evidence classes, used on every line:**

- **[QA]** — I opened the file or ran the command myself today. Line numbers are mine.
- **[lap]** — reported by the implementer in this topic and **not** re-measured by me. A release runner must
  re-run it; a self-report is not a gate.

**The honest header:** the e2e specs in `webui/e2e/live-voice-*.spec.ts` are **written but never run** — there
is no server config in this tree (`dev.vizier.yaml` absent [QA]; `dummyplug` lives in
`crates/vizier-http/src/test_support.rs` and in tests [QA]). Each spec carries a `// NOT RUN in this
environment` header on purpose. **No line of this document was proved by real audio.**

---

## 0. Prerequisites — the two things that make a correct binary look broken

**0.1 `live_voice` is off per agent by default, and one switch turns it on.** [QA]

- `crates/vizier-core/src/schema/agent.rs:240-241` — `stt: None`, `tts: None`; and
  `crates/vizier-core/src/service/agents.rs:509` — `live_voice: …enabled && has_recogniser && has_voice`.
  Three conditions, all false on a fresh agent.
- **CORRECTED 2026-11-10:** when this checklist was first written T041 did not exist (both files read 0).
  It landed as `c683f077` (pin `cd624227`): the agent page now has a `Live voice` switch
  (`webui/app/screens/agents/ModelsFields.tsx:205-223`) that borrows the two providers above it. **Enabling
  it is a click**, and the config write below is only the fallback for an API-only or headless setup.
- The fallback, when there is no UI: `PUT /api/v1/agents/{agent_id}`
  (`crates/vizier-http/src/api/v1/agents/mod.rs:106`), whose handler takes a full `CreateAgentRequest`
  (`mod.rs:318-324`), with the speech halves and the block below:

```yaml
tools:
  stt: { enabled: true, settings: { provider: <SttProvider>, model: <id>, language: <code> } }
  tts: { enabled: true, settings: { provider: <TtsProvider>, voice: <id> } }
  live_voice:
    enabled: true
    settings: { idle_stop_seconds: 180, max_utterance_seconds: 60, end_of_utterance_ms: 800 }
```

- **What failing looks like:** no pill at all, or a greyed pill. A greyed pill is **not** a bug — it is
  FR-004 (`webui/app/screens/chat/LiveVoiceRow.tsx:196-214`) — **but its tooltip names the missing half only when a
  caller supplies it, and today no caller does** *(opening corrected 2026-11-16, `@teteh-qa`, at `44bf167f` +
  `webui@0d5255f`: the sentence before this correction claimed the tooltip names the missing half while item (a)
  below said it never does — the two halves of one paragraph contradicted each other, and (a) was right)*.
  Two rough edges [QA], **re-measured 2026-11-16** (the `70806654` line numbers this paragraph used to carry were stale
  by 16 Nov): (a) `app/routes/chat.tsx:544` passes `liveVoiceEnabled` but **never**
  `liveVoiceMissing`, although `Composer.tsx:71,114,487` accepts and forwards it - so the tooltip is always the generic
  *"live voice is not configured for this agent"* and never names the missing half (this is analysis finding **M1**, and
  it is **open**, not closed - see the dated correction row in `analysis.md` §6); (b) the greyed control is a `<span>`
  (`LiveVoiceRow.tsx:204-212`; the words live in `title` `:205` and `sr-only` `:211`), not a button, so pressing it cannot
  open settings: `onOpenSettings` reaches `LiveVoiceControl.tsx:31,67,133` but `Composer.tsx` never supplies it, and the
  one branch that would use it (`LiveVoiceControl.tsx:131-135`) is unreachable because the only controls that call
  `start` are rendered only when `capability` is true. **Neither is a release blocker** - the generic FR-004 sentence is
  shown; what is missing is *which half* and *where to set it*. Do not report it as "the pill is dead" without this line.

**0.2 A real recogniser and a real voice.** `SttProvider` / `TtsProvider` need saved provider credentials
(`crates/vizier-core/src/schema/agent.rs:398-408` and `:335-343`). **Do not** write credentials into this file
and do not open `app-config.json` / `.env`; that choice is the User's and is still open.

**0.3 The release build.** `just release` = `cargo build --release -p vizier` (`Justfile:98-99`) [QA], and
`crates/vizier/Cargo.toml:30-32` pulls `vizier-core` with `["full"]` and `vizier-http` with **no features**
(the `["live-voice"]` there was removed with the flag, C17, 2026-11-16) [QA] — the live voice code is in the
binary because `vizier-core` and `vizier-http` **always** compile it now, not because a feature is switched on.
If you still find `["live-voice"]` there, the C17 change did not land in this tree. `crates/vizier-http/build.rs:15-33` runs `npm run build` in `webui/`
when `node_modules` exists and embeds `webui/build/client` [QA], so `just build` is not a prerequisite — but
`node_modules` is (without it, build.rs panics). Expect ≈23 min, ≈10 GB peak RSS, ≈17 GB `target` [lap, Lead].

  *(Corrected 2026-11-16, `@teteh-qa`: the `live-voice` half of that line stops being true the moment the C17 change
  lands — the feature is being **deleted, not renamed** (`specs/_antrean.md` section "ACC: lepas feature gate live voice").
  After it, this line reads `vizier-core` `["full"]` only, and A3 must not point at a flag that no longer exists. As of
  `44bf167f` the change sits uncommitted in the working tree, so it is a plan, not a fact about this tree.)*

---

## A. Provable without a server, a provider or a microphone

Run these first: a red here invalidates section B.

| # | Command (exact) | Expected | How you know it failed |
|---|---|---|---|
| A1 | `cd webui && npx vitest run app/lib/liveVoice app/screens/chat` | **12 files, 145 passed \| 1 skipped, EXIT=0** (measured) | non-zero exit, or a red naming `LiveVoiceRow`, `Composer` or `session` |
| A2 | `cd webui && npx tsc --noEmit` | **`EXIT=0`, zero `error TS` lines — measured by Lead 2026-11-16 at `webui@0d5255f`/superproject `7739b006`**, replacing this cell's earlier `EXIT=2` with one `TS7016` `rehype-katex` (`app/screens/shared/Markdown.tsx:4`). **Why it moved, as far as artefacts prove:** `webui/package.json` has pinned `rehype-katex` **7.0.1** since `9777b57` (unchanged — no commit touched the pin), and 7.0.1 **ships its own `index.d.ts`**; the installed copy (`webui/node_modules/rehype-katex`, mtime **2026-11-16 09:18:31**, after today's `npm install`) declares `export { default } from "./lib/index.js"`. A package with its own types cannot produce `TS7016`. The install that produced the old red is **gone** and no npm log survives for it, so the old version cannot be read back: the consistent reading is **a stale `node_modules`, not a repo state** — which means the old `EXIT=2` was a snapshot of this machine, and this cell should never have been a gate on anybody else's tree. Re-measure it; do not copy the old red forward. | any `error TS` line at all - the baseline is now zero. If a `TS7016` for `rehype-katex` returns, check `node_modules/rehype-katex/index.d.ts` exists and that `package.json` still pins 7.0.1 **before** touching types or imports |
| A3 | `env -u PYTHONHOME -u PYTHONPATH cargo test -p vizier-core --lib live_voice::` | **56 passed, 0 failed, EXIT=0, `running 56 tests`, 2.81s** — **measured by @teteh-qa at `c71f0e64`** (2026-11-16, `CARGO_TARGET_DIR=/home/muruyung/vz-gate-target`, `1966 filtered out`). `grep -cE '^[[:space:]]*#\[(tokio::)?test'` over `crates/vizier-core/src/live_voice/` also gives **56** (11 framing + 7 segmenter + 33 session + 5 utterance). Earlier readings of the same module: **counted at `58942ccd`; identical at `5c84baae`** — 56 `#[test]` / `#[tokio::test…]` under `crates/vizier-core/src/live_voice/`. The **49** this row carried an hour earlier was the same tree read with an anchored pattern that misses `#[tokio::test(start_paused = true)]` — 7 of the 56 are written that way; the **48** before that was measured *with* the flag in the older snapshot in this file's header, `57321373`) | `error: package 'vizier-core' does not contain this feature: provider-live-voice` means the C17 change has **not** landed yet - a red for the gate, not a skip, so do not reach for the old flag. A count below 56 means a test stopped compiling; name it, never edit the number to match. **The `::` in `live_voice::` is load-bearing**: without it the filter also matches a test whose *name* contains `live_voice` but which lives outside the module (`crates/vizier-core/src/schema/agent.rs:612`, `live_voice_is_off_and_defaulted_until_it_is_configured`), and the run prints **62** against this cell's 56 — a red on correct code. **62, not 57, measured at `c71f0e64`**: the 56 in the module plus six outside it — `schema::agent::media_provider_tests::live_voice_is_off_and_defaulted_until_it_is_configured` (a *test-name* match) and the five in `service::agents::live_voice_input_tests::…` (a *module-path* match). The earlier **57** was this tree before those five moved into `live_voice_input_tests`; a figure in this cell that nobody re-measured on the tree in front of them is a guess, so re-run it instead of trusting the number here. `--features` is now always an error here, and `--workspace` never enters this file. **This cell is occasionally false, and the tree says so (16 Nov 2026):** `@teteh-qa` saw **1 of 11** runs fail — `55 passed, 1 failed`, `live_voice::session::filler_tests::a_dropped_filler_never_costs_the_reply`; Lead then ran it **21 times** at `vizier@39d16fed` (`CARGO_TARGET_DIR=/home/muruyung/vz-gate-target`: 1 warm-up block of 56, **10** runs of that test alone, **10** runs of the full 56-test block) and saw **0 failures**. So a re-run of this exact line can come back green while the race is real, and a red on this row is **not** evidence that T113 or C17 broke something — it is **`GI-08`** (`specs/_antrean.md`) until someone makes the test deterministic. The suspicion, not yet proven: it races on the real clock — it sleeps **500 ms** (`session.rs:1998`) to free the lane while the filler fires at **400 ms** (`:1997`), so a loaded scheduler can free the lane *before* the filler tries and let the filler through, failing `all(\|len\| *len > 8)` (`:2007`). |
| A4 | `env -u PYTHONHOME -u PYTHONPATH cargo test -p vizier-http` | **every target `test result: ok`, 0 failed, EXIT=0**, and the four `live_voice_*` binaries each `running N tests` with **7 / 8 / 11 / 6** (**32** in all) - never `running 0 tests`. **Measured by @teteh-qa at `25689640`** (agents_seam and the full crate, 2026-11-16) **and re-measured in full at HEAD `39d16fed`**, whose `crates/` is byte-identical to `03ce4589` (`git diff 03ce4589 HEAD -- crates/` = **0 lines**; `03ce4589` carries the cosmetic `a06afd13` rename + rustfmt, no behaviour change). Both runs used `CARGO_TARGET_DIR=/tmp/vz-qa-t113`. The `c71f0e64` reading of this cell was **109 passed / 108 functions** before T113: **12 `test result` blocks, EXIT=0**, all `ok`, none `#[ignore]`d — lib **58** · agents_seam **12** (was 6) · conformance **1** · desktop_principal **2** · detached_routes **1** · inbox_stream **7** · barge_in **11** · delivery **6** · frames **8** · session **7** · topic **1** · Doc-tests **1**. That is **114 test functions** (58 in `src/`, 56 in `tests/`) **+ 1 doc-test = 115 `passed` in all**, and the two numbers are not the same figure: sum the `passed` column and you get **115**; count `#[test]`/`#[tokio::test]` in the crate and you get **114**. **This row is the full `cargo test -p vizier-http`, with no `--lib` and no `--tests`**, and that is where the twelfth block comes from. Two neighbouring invocations, both measured at `39d16fed` so nobody has to guess: `--tests` gives **11 blocks / 114 passed** (the doc-test is what the twelfth block adds), and `--lib` gives **1 block / 58 passed** — it runs the lib unit tests only, not the integration targets, so it is *not* the way to check this cell's 114. The frame count **8** is unchanged because the tautological test is **replaced** by a real one, `a_start_is_ignored_while_the_agent_switch_is_off`, so 32 stays 32. The "100 passed, 11 blocks" this row used to carry was measured in the older snapshot `57321373`, before the doc-test existed. **T113 has landed (`25689640`) and this cell was raised in the same round, as it required.** The four `live_voice_*` binaries are **untouched by T113**: they stay `7 / 8 / 11 / 6`, so their **32** does not move — the rise from 108 to 114 functions is entirely `agents_seam` (6 → 12), and a reader comparing this cell with an older copy should not read the total change as a change in the live-voice test count | a `live_voice_*` binary at `running 0 tests` = a file-level `#![cfg(feature = "live-voice")]` has come back (those lines were removed at `c71f0e64`); `does not contain this feature: live-voice` = the crate's `[features]` block survived; a total below **114** (**115** with the doc-test) names a test that stopped running, never the cell |
| A5 | `cd webui && npx playwright test --list e2e/live-voice-row.spec.ts e2e/live-voice-lifecycle.spec.ts e2e/live-voice-failures.spec.ts` | **16 tests in 3 files** (8 per project x 2 projects), exit 0 (measured 2026-11-10) | parse error. **A list is not a pass** — it only proves the selectors are syntactically valid |
| A6 | `node design/scripts/lint-tokens.mjs webui/app` from the repo root (**not** `app`: the script resolves against the cwd, and `app` silently lints nothing and exits 0) | **12 violations, `EXIT=1` — measured by Lead 2026-11-16** at `webui@0d5255f`, replacing the 2026-11-10 reading of **10**. Breakdown: `SideCard.tsx` x2 (`:8` gradient, `:16` `p-18`) · `WorkspaceHeader.tsx` x2 (`:101` `py-3`, `:105` `-ml-5`) · `WorkspaceSidebar.tsx` **x6** (all on `:151`: `pr-60` x3, `pr-36` x3 — one line, six matches) · `routes/chat.tsx` **x2** (`:508` `pr-[calc(24px+var(--scrollbar))]` and `pr-[calc(12px+var(--scrollbar))]`). **Why 10 -> 12:** the two `chat.tsx` matches came in with the master merge (`webui@bd39cdb` *"fix(chat): line the composer up with the message column"*, reached here via the 16 Nov merge `webui@0d5255f`), not from 055. **Zero** hits on `LiveVoiceRow`/`liveVoice` — verified by `grep -ciE 'LiveVoiceRow\|liveVoice'` over the script's output = **0**. So this is a pre-existing debt that got a name, not a 055 regression | **any** violation whose path is `LiveVoiceRow` or `liveVoice` — a raw hex or a raw spacing step got in. Count is **12** now: a 13th means someone added one, and a lower count means someone fixed one without saying so - re-measure, name the file |
| A7 | `grep -n 'Interrupt' webui/app/screens/chat/LiveVoiceRow.tsx webui/app/screens/chat/LiveVoiceControl.tsx` | **`LiveVoiceRow.tsx:74` only** (re-measured 2026-11-16 at `webui@0d5255f`; the `:48` this row carried came from the older tree) - the state word `'Interrupted'` inside `rowWord()`; `LiveVoiceControl.tsx` **0** | any hit that renders a **button or label** named Interrupt, or any hit outside `rowWord()`. A bare `grep -c` reports `1` here and is a false alarm - read the line, not the count |
| A8 | `grep -n 'MANUAL_SPEECH_SURFACES' webui/app/screens/chat/Composer.tsx` | **3 hits**: the constant `false` at `:59`, and its two uses `:440` (mic) and `:494` (spoken-reply toggle) — re-measured 2026-11-16 at `webui@0d5255f` | a `true` default: the retired surfaces (FR-053/T107) reappear |
| A9 | read `rowWord()` in `webui/app/screens/chat/LiveVoiceRow.tsx:62-84` (re-measured 2026-11-16 at `webui@0d5255f`; the `:37-58` this row carried was the older tree) | `Connecting…`, `Thinking`, `Listening`, `Speaking`, `Interrupted`, `Reconnecting`, `Ended`, `Failed` | a missing word, or `Thinking` and `Reconnecting` sharing one |

**Why A3/A4 changed (2026-11-16, C17 — the flags are deleted, not renamed).** Three consequences a runner will hit,
all of them arithmetic, none of them a guess:

1. **`--features provider-live-voice` and `--features live-voice` now fail hard** (`does not contain this feature`).
   The old commands did not weaken, they stopped existing.
2. **The four `live_voice_*` files no longer compile to empty binaries.** Each *used to* begin with
   `#![cfg(feature = "live-voice")]` (those lines were removed at `c71f0e64`), so every earlier no-flag run showed
   them at `running 0 tests` and their tests were invisible. Counting `^\s*#\[(tokio::)?test` over them gives **7 / 8 / 11 / 6 = 32**. The tautological
   `the_flag_off_build_has_no_live_voice_surface` goes and **a real test replaces it**
   (`a_start_is_ignored_while_the_agent_switch_is_off`, `#[tokio::test(flavor = "multi_thread")]`), so the count
   **stays 32** — the shape changes, not the total. **Count without a `\]` anchor**: seven tests in `vizier-core` and
   several in this crate are written `#[tokio::test(start_paused = true)]` or `(flavor = "multi_thread")`, and an
   anchored pattern reads them as absent — the same slip produced this row's first "31" and A3's first "49". The
   brief's "15 tests in 4 files" reproduces under no counting rule I tried: `#[tokio::test]` alone gives **16**
   (3 / 0 / 9 / 4), plain `#[test]` alone gives **16** (4 / 8 / 2 / 2), and both together give **32** — 15 is not a
   rounding of anything.
3. **So the rise against the old *no-flag* total is +32, not +14.** A no-flag run used to skip these four files
   whole, running 76 of the crate's 108 tests; it now runs all 108. The removal brief expected +14 and that number is
   wrong; do not hunt a phantom count to reach it. The *with-flag* total does not move either way: the test added is
   exactly the test removed.

Both `Expected` cells in A3/A4 were **derived from the source at `5c84baae`** until 2026-11-16; they are now
**measured by @teteh-qa at `c71f0e64`**, the commit that removed the flags, run once on a clean tree with
`CARGO_TARGET_DIR=/home/muruyung/vz-gate-target`. A3 was run on `c71f0e64` itself; A4 was re-run on
`301e23a6`, where `crates/` is byte-identical to `c71f0e64` (`git diff --stat c71f0e64 HEAD -- crates/` = **0
lines**), so the figures belong to the gate commit and not to a later tree. The label rule is unchanged for
every other `[QA]` line: nothing
counted from source may sit in an `Expected` cell without saying it is derived.

---

## B. Needs the running binary — server, provider, microphone

Start the release binary, open a chat with an agent configured per §0.1, then walk these in order.

1. **Nothing appears until you press.** The composer shows one pill `Live voice` (106×24), no badge, no pulse.
   *Broke if:* a row, waveform or state word is visible with no session running.
2. **Press the pill.** It stays in place; its frame becomes `Connecting…` while the browser asks for the
   microphone; then the state word takes the pill's slot: `Listening`.
   *Broke if:* a second row grows above the composer (that was B2, not chosen), or the composer jumps.
3. **The microphone is readable for the whole session.** `mic on` is written beside the glyph, including
   while the window is unfocused. *Broke if:* the word vanishes at some width, or becomes a bare icon.
4. **Thinking vs reconnecting, three layers.** Ask something slow: `Thinking` + emerald + three dots. Drop the
   network for a second: `Reconnecting` + amber + a still bar. *Broke if:* same word, same colour or same
   marker — FR-042 needs all three to differ.
5. **Barge-in.** While the agent speaks, start talking: it **keeps talking** (C8 — that is correct). Stop
   talking: within ≈300 ms the audio stops and your words become the next message. *Broke if:* audio stops the
   moment you start (the old onset rule), or never stops.
6. **An ending carries a reason, and it stays.** Say nothing for 3 minutes (`idle_stop_seconds: 180`): the row
   shows `Ended` + a reason + `mic off` + `Start again`, and the OS microphone indicator goes off within 2 s.
   *Broke if:* the row silently returns to the pill, or the microphone stays on.
7. **End is the only control.** One action ends the call; no `Interrupt` anywhere. *Broke if:* a second button
   with the same effect, or an interrupt control (deferred, C7).
8. **A failure is retryable without a reload.** Revoke the microphone permission, press the pill: `Failed` +
   a readable reason; press `Try again` → `Connecting…` → `Listening`. *Broke if:* a reload is needed, or the
   reason is a code word such as `no_device`.
9. **Audio the server cannot carry is refused, and named.** With a WS client, send `live_voice_start`
   declaring `sample_rate: 44100`: one `live_voice_error` whose text names **16000 vs 44100**, and **no
   session is registered**. Then send one 44.1 kHz frame mid-session: refused, and the session **stays live**
   (`305fc838`, T102) [lap]. *Broke if:* the session starts anyway, or the mid-session refusal ends the call.
10. **One session per person.** Start a call in conversation A, then press the pill in B: A ends with `moved`,
    exactly one microphone is open, and nothing from A's reply is spoken into B (FR-050/051/052).
    *Status:* the client half is in; the `moved` path is **C6d and still open** — do not report this as
    passing before it lands.
11. **Latency is recorded, not judged.** Every turn logs one line
    `live_voice_metric turn=… end_of_utterance_to_audible_ms=… end_of_utterance_to_silence_ms=…`
    (`webui/app/lib/liveVoice/metrics.ts:79-89`, `console.info` at `:83` [QA]). Collect ≥50 turns before anyone quotes a percentile.
    *Broke if:* the line is absent — the recorder is off by default (one boolean per turn, no allocation).

---

## C. Cannot be proved on this machine at all — say it, do not paper over it

| What | Why not | Consequence for the claim |
|---|---|---|
| All three `live-voice-*.spec.ts` | no server config in the tree; `dummyplug` is test-only | never write "e2e green" |
| SC-008 — no self-interruption at 80 % speaker volume, 10 minutes | needs the physical device, a quiet room, time | the echo claim rests on a manual run, not a test |
| SC-004 soak (15 turns / 10 min) and SC-005 first use ≤20 s | no harness, no provider | unmeasured |
| SC-012 third-party client | that client does not exist (T106 open) | the public contract has no conformance proof |
| SC-013 key scope | C6b is in flight as this is filed; no HTTP-level test | **the security gate is open** |
| FR-031 unintelligible, server side | `TopicConnection` needs `HTTPState` + `Principal` + storage; no harness [lap] | in the code, untested |
| T077 server half (`tracing` spans) | not written; the latency work is client-only [lap] | no server-side breakdown of the numbers |
| Echo and reconnection under real packet loss | needs a real network and real audio | not provable here |

---

## C2. Two defects I found while verifying (not on the list above)

**1. The published contract has no word for a reason the server sends.**
`crates/vizier-http/src/topic.rs:451` and `:563` send `kind: EndReason::UnsupportedAudio`, i.e. the wire
word `unsupported_audio` (`crates/vizier-core/src/live_voice/frame.rs:119`). The contract's reason
vocabulary (`contracts/live-voice-frames.md:145-147` and again `:339-341`) lists **17** words and omits it;
`grep -c unsupported_audio` on the contract file = **0**. Line `:342` then promises the opposite of what the
code does: *"a client MUST NOT receive a reason it has no word for."* A third-party client built from the
published document will meet an unnamed reason on its first bad `live_voice_start`. Owner is the contract's,
not mine.

**2. `tasks.md` undercounts and one of its ids points at a path that never existed.**
111 task ids (T001-T111, T111 is the `[PROVISIONAL]` reply-segmentation task), **65 checked / 46 open**;
`[BACKLOG]` = **9**. T095/T096/T097 and T110 are checked now, so the earlier "4 landed but unticked" gap is
closed - but the ids are **111, not 110**. Several bodies still name `webui/app/screens/livevoice/...`, a
directory that does not exist; the files live in `screens/chat/`.

**Fixed since my first draft, verified:** SC-001 no longer calls itself a release gate (`spec.md:627` now
reads *"is the figure this document records"*), so the contradiction in section D below is history, not a
live defect. The greyed pill is still a `<span>` and `liveVoiceMissing` is still not passed from
`app/routes/chat.tsx:544` (re-measured 2026-11-16 at `webui@0d5255f`; the `:558` this line used to carry came from an
older tree, and the file lives in `app/routes/`, not `app/screens/chat/`), so **both** FR-004 halves - *which half* and
*where to set it* - remain unshown - cosmetic, and I say so rather than
call it broken.

## D. The thresholds — recorded today, **not** a gate

SC-001 (`p90 ≤ 3.5 s`, `p99 ≤ 5 s`) and SC-002 (`300 ms` in ≥95 %) are **not** pass/fail in this release.
Decision of 2026-11-09 19:16, approved by the User: there is **no baseline** — zero samples from a real
provider — and a gate without a baseline only manufactures a false green. Collect ≥50 real turns, then it
becomes a gate.

- Recording side: `metrics.ts` keeps a ring of 200 samples and computes p50/p90/p99
  (`liveVoiceReport()`), with **no assertion** that fails on a slow turn [QA].
- **Contradiction, flagged not obeyed:** `spec.md:546` still reads *"is the release gate"*. The sentence
  disagrees with the approved decision; its owner is `@system-analyst`. I did not edit it.

## E. Two known reds — debt, not a 055 regression

`app/screens/shared/Markdown.test.tsx` → `draws inline math` and `draws display math` fail (KaTeX renders as
plain text; `tsc` reports the same missing types). Evidence it is not ours [QA]: `git log -1` on
`Markdown.test.tsx` → `f6f1b1a`, **2026-11-05**; on `Markdown.tsx` → `a505a26`, **2026-11-07** — both before
this feature branch existed. **Do not fix on this branch**; the release must not carry an unrequested
markdown change.

## F. What this document does not cover

The row's behaviour under long messages and scrolling, right-to-left text, the desktop Tauri shell (only the
browser path was reasoned about), and anything about speech **quality** — that belongs to the provider, per
the spec's own assumptions. I read documents and code; I ran no browser session.

---

**Most likely way the User's own test fails, in one line:** the binary is right and the pill is greyed — the agent's
`Live voice` switch is off, or one of the two providers behind it is unconfigured (§0.1). Since `c683f077` that is
a click, not a config write, so the release note should say **where the switch is**, not only how to start the binary.

