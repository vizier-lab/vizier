# 055 — Release checklist

**Written by `@teteh-qa` at `70806654`** (parent; `design 021ab46`, `webui c1dabc6`), 2026-11-09, and
**re-verified at `70806654`**: every `[QA]` line below was re-read against this tree, not carried over.
Since the first draft, T102 landed `unsupported_audio` (`de006e3e`, pin `2569aa34`); C6b (API key scope) sits
**uncommitted in the working copy - 12 files `M`** and is therefore in no gate below.
Scope: what a person running the release binary must actually check, and what **cannot** be checked on this
machine. This is a test plan, not a claim that anything below has passed.

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
  FR-004 (`webui/app/screens/chat/LiveVoiceRow.tsx:169-183`) and its tooltip names the missing half.
  Two rough edges [QA], both re-checked at `70806654`: (a) `chat.tsx:546` passes `liveVoiceEnabled` but **never**
  `liveVoiceMissing`, although `Composer.tsx:71,102,472` accepts and forwards it - so the tooltip is always the generic
  *"live voice is not configured for this agent"* and never names the missing half; (b) the greyed control is a `<span>`
  (`LiveVoiceRow.tsx:169-189`), not a button, so pressing it cannot open settings: `onOpenSettings` reaches
  `LiveVoiceControl.tsx:26,75` but `Composer.tsx` never supplies it. **Neither is a release blocker** - FR-004's text is
  shown, only the *where to set it* half is missing. Do not report it as "the pill is dead" without this line.

**0.2 A real recogniser and a real voice.** `SttProvider` / `TtsProvider` need saved provider credentials
(`crates/vizier-core/src/schema/agent.rs:398-408` and `:335-343`). **Do not** write credentials into this file
and do not open `app-config.json` / `.env`; that choice is the User's and is still open.

**0.3 The release build.** `just release` = `cargo build --release -p vizier` (`Justfile:98-99`) [QA], and
`crates/vizier/Cargo.toml:30-32` pulls `vizier-core` with `["full"]` and `vizier-http` with `["live-voice"]`
[QA] — the feature **is** in the binary. `crates/vizier-http/build.rs:15-33` runs `npm run build` in `webui/`
when `node_modules` exists and embeds `webui/build/client` [QA], so `just build` is not a prerequisite — but
`node_modules` is (without it, build.rs panics). Expect ≈23 min, ≈10 GB peak RSS, ≈17 GB `target` [lap, Lead].

---

## A. Provable without a server, a provider or a microphone

Run these first: a red here invalidates section B.

| # | Command (exact) | Expected | How you know it failed |
|---|---|---|---|
| A1 | `cd webui && npx vitest run app/lib/liveVoice app/screens/chat` | green, 1 skipped | non-zero exit, or a red naming `LiveVoiceRow`, `Composer` or `session` |
| A2 | `cd webui && npx tsc --noEmit` | `EXIT=2` with **one** error: `TS7016` `rehype-katex` (`app/screens/shared/Markdown.tsx:4`) | any *other* TS error is new |
| A3 | `env -u PYTHONHOME -u PYTHONPATH cargo test -p vizier-core --features provider-live-voice --lib live_voice` | 23 passed [lap] | any failure. Crate + feature minimum; never `--workspace` |
| A4 | `env -u PYTHONHOME -u PYTHONPATH cargo test -p vizier-http --features live-voice` | 11 suites ok [lap] | any failure |
| A5 | `cd webui && npx playwright test --list e2e/live-voice-row.spec.ts e2e/live-voice-lifecycle.spec.ts e2e/live-voice-failures.spec.ts` | prints a list, exit 0 | parse error. **A list is not a pass** — it only proves the selectors are syntactically valid |
| A6 | `node design/scripts/lint-tokens.mjs app` | 15 violations, all pre-existing (`WorkspaceSidebar`, `SideCard`, `WorkspaceHeader`) [lap] | **any** violation whose path is `LiveVoiceRow` or `liveVoice` — a raw hex or a raw spacing step got in |
| A7 | `grep -n 'Interrupt' webui/app/screens/chat/LiveVoiceRow.tsx webui/app/screens/chat/LiveVoiceControl.tsx` | **`LiveVoiceRow.tsx:48` only** - the state word `'Interrupted'` inside `rowWord()`; `LiveVoiceControl.tsx` **0** | any hit that renders a **button or label** named Interrupt, or any hit outside `rowWord()`. A bare `grep -c` reports `1` here and is a false alarm - read the line, not the count |
| A8 | `grep -n 'MANUAL_SPEECH_SURFACES' webui/app/screens/chat/Composer.tsx` | one constant = `false`, plus its two uses (mic, spoken-reply toggle) | a `true` default: the retired surfaces (FR-053/T107) reappear |
| A9 | read `rowWord()` in `webui/app/screens/chat/LiveVoiceRow.tsx:37-58` | `Connecting…`, `Thinking`, `Listening`, `Speaking`, `Interrupted`, `Reconnecting`, `Ended`, `Failed` | a missing word, or `Thinking` and `Reconnecting` sharing one |

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

