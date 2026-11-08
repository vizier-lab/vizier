# Specification Analysis Report — SPEC-055 Live Voice

**Command**: `speckit-analyze` (read-only) · **Analyst**: `@teteh-qa` · **Date**: 2026-11-08
**Feature**: `specs/055-live-voice` · **Branch**: `055-live-voice`

## 0. Snapshot this analysis is valid for (read first)

The working tree was **dirty when this analysis ran**: five artifacts were rewritten at 15:25 by
`@mamang-lead-be` (D9 + instrumentation) *after* `@bro-pm`'s brief and *after* my previous status
message. So that the result is reproducible, here is the exact tree analysed:

```
HEAD ea21843f   git status --porcelain:
 M specs/055-live-voice/architecture.md
 M specs/055-live-voice/contracts/live-voice-frames.md
 M specs/055-live-voice/plan.md
 M specs/055-live-voice/review.md
 M specs/055-live-voice/tasks.md

sha256 (first 12): spec 9af33f90332e · plan e523604b54e5 · tasks 15721b790123
                   architecture 5a239a1c3b6f · review 9de2996a57b5 · contracts 44f9b1b14ace
                   data-model 27b09d68a253 · quickstart fce840644067 · research 42c98e48bd61
Counts: 42 FR (FR-001..FR-041 + FR-036a) · 9 SC · 79 tasks (T001..T079) · 21 [P]
```

**Correction of my own earlier report.** My 15:22 message said the audio path was *unpatched*
(`plan.md:98` "like any other frame", zero `broadcast` hits). That was true of the tree I read at
15:20. In the current tree it **is** patched: `plan.md:98`, `plan.md:171`, `plan.md:257-269`,
`contracts` §Delivery path, `architecture.md` §11 + I9, and tasks `T020`/`T021`/`T040` all now carry
the per-connection decision, and the backlog bound `LIVE_VOICE_SOCKET_BACKLOG = 16` exists in
`plan.md:257`. Analysis below is against the patched tree, not the one I quoted before.

## 1. Specification Analysis Report

| ID | Category | Severity | Location(s) | Summary | Recommendation |
|----|----------|----------|-------------|---------|----------------|
| C1 | Constitution | **CRITICAL** | `quickstart.md` (whole file), `plan.md:41`, `tasks.md:187` | The constitution requires that "Each feature's `quickstart.md` MUST script its checks as dummyplug steps: the message to send and the expected reply, stored state, or history entry" (`.specify/memory/constitution.md:201-209`). `quickstart.md` has **zero** `dummyplug` mentions (9 by-hand steps only); `plan.md:41` claims these steps exist, and `T077` says "run the dummyplug gate" with nothing scripted to run. | Add a dummyplug step block to `quickstart.md` (message in / expected reply / expected history entry for the agent-observable half) before `speckit-implement`; mark the live-provider audio step as such. Owner: `@mamang-lead-be`. |
| H1 | Inconsistency (code) | **HIGH** | `tasks.md:112` (T044), `plan.md:107` (R7), `research.md` | `T044` instructs `transport.send_agent_command(AgentCommand::Abort { .. })` at `agents_client.rs:415`. **That symbol does not exist**: `AgentCommand` is at `crates/vizier-core/src/schema/commands.rs:142` and has variants `Create/Update/Delete/HealthCheck/AgentHealth/RefreshIntegrations/IntegrationStatus/Restart` — no `Abort`; and `agents_client.rs:415` is `FakeCall::Abort`, a test fake. The real abort is `AgentsClient::abort(agent_id, session)` (`agents_client.rs:206-207`, impl `:298-305`) which sends `VizierRequestContent::Command("abort")` — i.e. the same `/abort` the Composer already offers (`Composer.tsx:38`). | Rewrite T044 to call `AgentsClient::abort` (or submit `Command("abort")` exactly as the typed `/abort` does) and fix R7's citation. As written, US2 fails to compile on its first implementation task. |
| H2 | Inconsistency | **HIGH** | `spec.md:220` (FR-023) vs `spec.md:224` (FR-027) | FR-023 (clarified) says the abort path keeps **no partial prose** ("not partial prose"); FR-027 says an interruption leaves "text produced so far stays in the conversation marked interrupted". Two requirements give two answers for the same event. | Restate FR-027 to defer to FR-023's two cases (speaking → whole message kept; generating → abort, mark + activity trail). Owner: spec author, not QA. |
| H3 | Inconsistency | **HIGH** | `spec.md:55` (US1 narrative), `spec.md:150` (flow node) vs `spec.md:38-45, 206` | The story promises "The answer starts being spoken **while it is still being written** into the conversation" and the flow node says "Reply spoken while it is written", while FR-012 and the constraint note say the turn delivers a **finished** message and speech starts per segment. US1's independent test cannot be written to satisfy both. | Reword spec.md:55 and the `speak` node to "the reply is spoken in segments once the turn's message exists"; keep FR-012 as the authority. |
| H4 | Coverage / Measurability | **HIGH** | `spec.md:266-268` (SC-001..003), `plan.md:45,228`, `tasks.md:183` (T073), `quickstart.md:41` | The three latency gates have **no defined measurement points**. T073 instruments server stages only (end of utterance, recognition, turn start, first segment, first frame); "first audible word" is a client event with no mark, and "silences within 200/300 ms" has no start point either. `quickstart.md:41` asks a human to "hear its first words within SC-001's gate: p90 under 3.5 s" — p90 is not observable by hand. Without this, SC-001/002/003 are numbers, not gates. | Add a client mark task (`performance.now()` on speech-end / audio-start / `player.stop()`) and name the two endpoints of each gate in `plan.md`; have T073 record both sides in `latency.md`. Requirement unchanged. |
| H5 | Coverage | **HIGH** | `spec.md:241` (FR-037), `tasks.md` (none) | `grep -ni language tasks.md` returns **nothing**: FR-037 (recognition follows the conversation's configured language) has no task and no test, though T027 builds the recogniser call where the language argument is passed. | Add the language argument + a test to T027 (or a new task): a session with a configured non-default language passes that language to `VizierStt::transcribe`. |
| H6 | Inconsistency (numeric) | **HIGH** | `plan.md:45` / `spec.md:267` (SC-002) vs `plan.md:191` (Phase D.3, onset guard) | SC-002 allows **300 ms** server-side for a barge-in to take effect, while the plan delays the barge-in signal by **~250 ms** of voiced energy. That leaves ~50 ms for network + queue + abort, against a socket path that also carries 10 audio frames/s. The gate is decided by the guard, not by the pipeline. | Either state SC-002's server budget as "≤ 300 ms after the onset guard", or reduce the guard to ~150 ms with the same false-positive argument re-checked. Pick one and make spec and plan agree. |
| H7 | Underspecification | **HIGH** | `spec.md:168-170` (edge case: second person / television) | The edge case requires that "only speech the session **can attribute to the person**" is submitted, but no FR, plan phase or task provides speaker attribution — the chosen VAD is an energy gate over one microphone with no speaker identity. The requirement promises a capability the architecture cannot deliver. | Either drop the attribution promise (restate as "background speech may be transcribed; the person corrects it by speaking again") or open it as a separate spec. Do not leave it as an acceptance-level expectation for US2. |
| M1 | Coverage | MEDIUM | `spec.md:196` (FR-004), `tasks.md:52` (T011) | FR-004 requires the control to "name what is missing and how to set it"; T011 only computes the boolean (flag on + usable recogniser + voice). No task renders the missing-capability message. | Extend T036 to render the FR-004 message (which of recogniser/voice is missing, where to set it). |
| M2 | Coverage | MEDIUM | `spec.md:199` (FR-008), `spec.md:210` (FR-016) | FR-008 (session uses the agent's own model and speech config; typed turns unaffected) and FR-016 (raw audio never stored) have no task and no test asserting them. FR-016 is an invariant that is cheap to falsify and expensive to miss. | Add one test task: after a session, no audio blob exists in the session/work dir and the stored message carries text only; and a session does not mutate the agent's model/voice config. |
| M3 | Coverage | MEDIUM | `spec.md:237` (FR-035) | "Chat interface only; Discord/Telegram must behave exactly as today" has no task and no test that the control is absent there. | Add to T067/T014 an assertion that no `live_voice` frame is produced for a channel session and the capability reads false there. |
| M4 | Coverage | MEDIUM | `spec.md:269-271` (SC-004, SC-005, SC-006), `tasks.md` | Three success criteria have **zero** task references: SC-004 (10-minute / ≥15-turn soak, ≥90% hands-free), SC-005 (talking within 20 s on first use), SC-006 (existing speech unchanged "verified by the existing checks"). SC-006 is partly implied by T074's matrix but never named. | Add a soak/e2e step (SC-004), a first-use scripted check (SC-005), and name the existing speech checks in T074 (SC-006). |
| M5 | Inconsistency (open question) | MEDIUM | `spec.md:298` (Q4), `review.md:164-170`, PM's D8 | The spec's **Q4 is "a hard cap on concurrent live sessions per agent or per server"**, default "no extra cap". PM's D8 asks about a **cap on session duration**, which is a different question and appears nowhere in the spec. As written, D8 cannot be answered "from the spec default". | Relabel: Q4 = concurrency (unchanged); if a duration cap is wanted it is a new requirement (and a new task on the idle path). Ask the User with the distinction spelled out. |
| M6 | Inconsistency (counts) | MEDIUM | `tasks.md:290`, `review.md:174-180` | Stale counts inside the artifacts: `tasks.md:290` says "Phase 2 (now 13 tasks, ending at T019)" while Phase 2 is **15** tasks ending at **T021**; `review.md` §6 C-2 says "the list is 77 tasks" while its own D9 row says "77 → 79". Anyone pinning a baseline by those numbers picks the wrong one. | Fix both sentences; keep one canonical statement of `tasks.md` = 79 tasks, and record path + commit in the handoff (PM asked for this explicitly). |
| M7 | Inconsistency | MEDIUM | `quickstart.md:46` | Step 4 still says the interrupted reply is "readable as far as it got", the phrasing FR-023 was clarified to remove (abort keeps no partial prose). | Reword step 4 to FR-023's two cases. |
| M8 | Inconsistency | MEDIUM | `data-model.md:102` | "text stays complete-as-produced" repeats the pre-clarification wording of FR-023, contradicting FR-023's abort case. | Reword to match FR-023. |
| M9 | Terminology | MEDIUM | `tasks.md:58` (T017), `data-model.md:101`, `spec.md:211` (FR-011) | The "came by speech" marker is named three ways: `spoken: bool` (T017), "additive flags only" (data-model), "marked as spoken" (FR-011). No artifact states the field name and its default together. | Name `VizierRequest.spoken: bool` (default `false`) in `data-model.md` so the contract and the task agree. |
| M10 | Constitution (process) | MEDIUM | `constitution.md:213-222`, `diagrams.md:151`, `tasks.md:93` (T036) | The mockup gate ("MUST be shown as a mockup and reviewed before any UI code") is met in form — `diagrams.md` §5 has an ASCII wireframe — but no task or record says the wireframe was **reviewed** before T036/T037 write UI. | Record the wireframe review in `review.md` (or a task note) before US1's UI tasks start. |
| M11 | Ambiguity | MEDIUM | `spec.md:170,223` (short utterance) vs `plan.md:191` (onset guard) / `tasks.md:113` (T045) | The edge case requires a very short utterance ("yes", "no", "stop") to be recognised and submitted, and US2 scenario 5 requires a too-short burst to stop the audio and send nothing; the onset guard needs ~250 ms of voiced energy before *any* barge-in is signalled. Which rule wins for a 200 ms "no"? Not stated. | State the precedence explicitly (e.g. guard decides barge-in; a burst under the guard stops audio locally and submits nothing) and check it against FR-024. |
| L1 | Duplication (historical) | LOW | `spec.md:319` vs `spec.md:332` (Delta Log) | Two Delta Log entries describe FR-023 differently ("readable as far as it was produced" → later "keeps no partial prose"). History is fine, but the log has no "superseded by" marker. | Mark line 319 superseded by line 332. |
| L2 | Traceability | LOW | `tasks.md` | FR-012, FR-015, FR-018, FR-034 are covered only implicitly — no task carries their ID. Nothing is missing in substance; the link is just not greppable. | Add the IDs to T030/T031 (FR-012), T034 (FR-015), T052 (FR-018), T060 (FR-034). |
| L3 | Process | LOW | this file | This analysis ran on a **dirty** tree (5 modified files, see §0). If those edits continue, this report's IDs and line numbers stop matching. | Commit the D9/instrumentation delta, then re-run `speckit-analyze` so the report has a commit to point at. |

Nothing above changes a User requirement. C1 and H1 are the two that block `speckit-implement`;
H2/H3/H6/H7 are requirement text that only the spec author may change.

## 2. Coverage Summary

### Requirements → tasks (explicit ID references in `tasks.md`)

| Requirement | Has task? | Task IDs / note |
|---|---|---|
| FR-001, FR-002, FR-003 | ✅ | T036 |
| FR-004 | ⚠️ partial | T011 (boolean only; the missing-capability message is M1) |
| FR-005 | ✅ | T025, T059, T026 |
| FR-006 | ✅ | T034, T037 |
| FR-007 | ✅ | T028 |
| FR-008 | ❌ no reference | implied by T025/T011 (M2) |
| FR-009, FR-010 | ✅ | T033 |
| FR-011 | ✅ | T017, T018, T028 |
| FR-012 | ⚠️ implicit | T030, T031 (L2) |
| FR-013, FR-014 | ✅ | T023, T030 |
| FR-015 | ⚠️ implicit | T034 (L2) |
| FR-016 | ❌ no reference | nothing asserts audio is not stored (M2) |
| FR-017 | ✅ | T018 (+T017) |
| FR-018 | ⚠️ implicit | T052 (L2) |
| FR-019, FR-022, FR-024, FR-025, FR-026 | ✅ | T042–T048, T051 |
| FR-020, FR-021 | ✅ | T049, T050, T053 |
| FR-023 | ✅ (used, not cited) | T044 via `review.md` D1 |
| FR-027 | ✅ (used, not cited) | T044 — with the wrong API (H1) |
| FR-028, FR-029, FR-030 | ✅ | T057–T060, T068 |
| FR-031, FR-032, FR-033 | ✅ | T070, T067, T065 |
| FR-034 | ⚠️ implicit | T060, T057 (L2) |
| FR-035 | ❌ no reference | M3 |
| FR-036, FR-036a | ✅ | T029 |
| FR-037 | ❌ **none** | H5 |
| FR-038, FR-039, FR-040, FR-041 | ✅ | T037, T071, T008/T014, T014 |

| Success criterion | Has task? | Note |
|---|---|---|
| SC-001 | ❌ | only T073 (server-side spans), no client endpoint (H4) |
| SC-002 | ⚠️ | named in T042; the 300 ms budget contradicts the 250 ms guard (H6) |
| SC-003 | ⚠️ | named in T051; no measurement point (H4) |
| SC-004 | ❌ | M4 |
| SC-005 | ❌ | M4 |
| SC-006 | ❌ | implied by T074, never named (M4) |
| SC-007 | ✅ | T056, T062 |
| SC-008 | ✅ | T041, T045 |
| SC-009 | ✅ | T064, T066 |

### Unmapped tasks

None that are wholly unmapped: every task names at least one FR/SC or a plan phase step. T005, T006,
T016, T072, T075, T078, T079 are process/foundation tasks with no requirement — expected.

### Constitution Alignment

| Principle | Status |
|---|---|
| I. Lean by default | ✅ no new mandatory dependency (verified: `base64` in `Cargo.lock`, worklet shipped) |
| II. DRY via trait seams | ✅ one `LiveVoiceEngines` trait (T029) |
| III. Optional capability behind a flag | ✅ `provider-live-voice`; ⚠️ make explicit that `live_voice: bool` exists in **both** builds reading `false` when off, or the flag changes the API shape (T011 currently reads either way) |
| Quality gate: `quickstart.md` MUST script dummyplug steps | ❌ **C1** |
| Quality gate: design/mockup before UI code | ⚠️ wireframe exists (`diagrams.md:151`), review not recorded (M10) |
| Quality gate: e2e against `dummyplug` before done | ⚠️ T077 exists, but nothing to run (C1) |

### Metrics

- Total requirements: **42 FR** + **9 SC** + 5 user stories
- Total tasks: **79** (21 parallel)
- FR with an explicit task reference: **32/42**; FR with no coverage at all: **3** (FR-016, FR-037, FR-035)
- SC with a task: **5/9**; SC with no measurement definition: **SC-001, SC-002, SC-003**
- Findings: **22** (1 CRITICAL, 7 HIGH, 11 MEDIUM, 3 LOW) — overflow: none
- Ambiguity / duplication count: 1 ambiguity (M11), 1 duplication (L1)

## 3. Next Actions

1. **Before `speckit-implement`**: fix **C1** (dummyplug steps in `quickstart.md`) and **H1**
   (`AgentCommand::Abort` → `AgentsClient::abort`). Both are in `@mamang-lead-be`'s files.
2. **Spec owner only** (not QA): H2, H3, H7 are requirement text; H6 needs a number agreed between
   spec and plan.
3. **Before US1's UI tasks**: H4's client marks, so the first demo can actually report a latency.
4. **Housekeeping**: M6 (stale counts) and commit the dirty tree (L3), then re-run `speckit-analyze`
   for a report pinned to a commit.
5. Checklist for the same tree: `specs/055-live-voice/checklists/live-voice.md`.

**Remediation offered, not applied** — I can write concrete edits for C1, H1 and M6 on request;
I do not edit spec/plan/tasks myself.

## 4. Re-check at the moment this file was written (15:4x)

The tree moved again **while this analysis was being written** — `@mamang-lead-be` committed the D9
delta and two follow-ups (`dd27fb86`, `c0bcc202`, `fc6c1192`, HEAD `fc6c1192`) and is still editing
4 files. So every finding was re-checked against that newer tree; five of the twenty-two are already
fixed, fourteen are still open. Use this table, not the severity column alone, to decide what is
left:

| ID | Status at HEAD `fc6c1192` + dirty tree | Evidence |
|---|---|---|
| C1 | **OPEN** | `grep -c dummyplug specs/055-live-voice/quickstart.md` → `0` |
| H1 | **FIXED** | T044 now calls `AgentsClient::abort(agent_id, session)` (`agents_client.rs:205-207`) and names the impl — the non-existent `AgentCommand::Abort` is gone |
| H2 | **OPEN** | `spec.md` unchanged (not in `git status`): FR-023 vs FR-027 still disagree |
| H3 | **OPEN** | `spec.md:55` and `:150` still say "spoken while it is written" |
| H4 | **FIXED** | new T073 defines the three gates (client `performance.now()` marks + server spans) and `quickstart.md` gained step 10 that reads them |
| H5 | **FIXED** | T029 now passes the conversation's configured language to `VizierStt::transcribe` |
| H6 | **OPEN** | `plan.md` still holds the ~250 ms onset guard against SC-002's 300 ms (`grep -c '250 ms plan.md'` → 2) |
| H7 | **OPEN** | `spec.md:168-170` still promises speaker attribution |
| M1 | **OPEN** | no task carries the FR-004 "what is missing" message (`grep -ci 'what is missing' tasks.md` → 0) |
| M2 | **OPEN** | `grep -c FR-016 tasks.md` → 0 |
| M3 | **OPEN** | `grep -c FR-035 tasks.md` → 0 |
| M4 | **OPEN** | `grep -coE 'SC-00[456]' tasks.md` → 0 |
| M5 | **OPEN** | `spec.md` Q4 still means *concurrency*; D8 asks about *duration* |
| M6 | **FIXED** | the "now 13 tasks, ending at T019" line is gone |
| M7 | **OPEN** | `quickstart.md` step 4 still says "readable as far as it got" (the file is being edited right now, so re-read it last) |
| M8 | **OPEN** | `data-model.md:102` still "complete-as-produced" |
| M9 | **FIXED** | `data-model.md:101` now names `VizierRequest.spoken: bool`, `#[serde(default)] = false` |
| M10 | **OPEN** | no wireframe-review record anywhere (`grep -ni wireframe tasks.md review.md` → nothing) |
| M11 | **OPEN** | no stated precedence between the onset guard and a very short utterance |

**What this means for the gate**: `speckit-implement` is blocked by exactly one thing now — **C1**
(dummyplug steps). H2/H3/H7 are spec text only the author may change; H6 is a number to agree
between spec and plan. The rest are MEDIUM/LOW and do not block a first task.

Task total moved 77 → 79 → **80** during this analysis (`T073` inserted). Any baseline quoted to
`@sepuh-be` must name the commit, not the count.

## 5. Final status, and how to re-read this file later

The tree moved **twice more while §4 was being written**: `spec.md` is now modified in the working
tree and the two spec-text findings I listed as open are being fixed as this file lands. §4's table is
therefore a snapshot with a shelf life of minutes. What is true as of the last command I ran:

| ID | Status | Evidence |
|---|---|---|
| H2 | **FIXED (uncommitted)** | `spec.md` FR-027 now defers to FR-023's two cases ("what the interrupted reply leaves in the conversation is exactly FR-023's two cases and nothing else") |
| H3 | **FIXED (uncommitted)** | `spec.md:55` → "spoken in segments as soon as the turn's message exists"; flow node → "Reply spoken in segments once its message exists" |
| C1 | **OPEN** | `grep -c dummyplug specs/055-live-voice/quickstart.md` → 0 |
| H6, H7, M1–M5, M7, M8, M10, M11 | **OPEN** at last check | commands in §4 |

Everything else in §1 stands; nothing was weakened, and no requirement was changed by me. Two
consequences worth keeping:

1. **The report is only as good as its commit.** `analysis.md` was written against `ea21843f` plus a
   dirty tree; by the time it was saved the branch was at `fc6c1192` plus a dirtier one. When the
   delta is committed, re-run `speckit-analyze` and this file is regenerated — do not quote its
   counts from memory.
2. **C1 is the only thing standing between this spec and `speckit-implement`.** It is one block of
   scripted steps in `quickstart.md`, and it is a constitution MUST, not a preference.

**Re-run** (read-only, anyone can):

```bash
cd ~/ngoding/pribadi/vizier
grep -c dummyplug specs/055-live-voice/quickstart.md          # C1: must be > 0
grep -n 'AgentsClient::abort' specs/055-live-voice/tasks.md   # H1: must hit T044
grep -c 'SC-00[456]' specs/055-live-voice/tasks.md            # M4: must be > 0
grep -n 'FR-016\\|FR-035' specs/055-live-voice/tasks.md        # M2/M3: must hit
```

---

## 6. Re-run at `HEAD 7454df5f` — `@mamang-lead-be` (`speckit-analyze`, 2026-11-08 15:5x)

Ran after `@bro-pm` authorised it, on a tree that is **not** the one §0 analysed. Prerequisite check
(`.specify/scripts/bash/check-prerequisites.sh --json --require-tasks --include-tasks`) returns
`FEATURE_DIR=specs/055-live-voice`, `AVAILABLE_DOCS=[research.md, data-model.md, contracts/, quickstart.md, tasks.md]`.

**Snapshot.** `HEAD 7454df5f` plus the edits of this pass (`tasks.md` T009/T010/T045/T080-T082).
Artifacts that moved since §0: `spec.md` (FR-027 restated, story 1 and the flow node reworded),
`quickstart.md` (a `dummyplug` section added), `plan.md` (D9 delivery path, `LIVE_VOICE_SOCKET_BACKLOG`),
`contracts` (§Delivery path), `architecture.md` (§11 + the honest note on `Lagged`), `tasks.md`
(D9/D10/D11/D12/D13, 80 → 83 tasks).

### 6.1 Findings of §1, re-tested one by one

| ID | §1 severity | Now | Evidence in the current tree |
|---|---|---|---|
| C1 (no `dummyplug` steps in `quickstart.md`) | CRITICAL | **closed** | `quickstart.md:77-95` — "The `dummyplug` half (agent-observable, no keys, no microphone)": setup, message, expected reply and the stored `Request` with `spoken: true`; the audio half is marked as needing a real provider |
| H1 (`AgentCommand::Abort` does not exist) | HIGH | **closed** | `tasks.md` T044 now calls `AgentsClient::abort(agent_id, session)` (`agents_client.rs:205-207`, impl `:298-312`); `AgentCommand` at `schema/commands.rs:142` confirmed to have no `Abort` variant; `agents_client.rs:415` is `FakeCall::Abort` |
| H2 (FR-023 vs FR-027) | HIGH | **closed** | `spec.md:224` — FR-027 now defers: "what the interrupted reply leaves in the conversation is exactly FR-023's two cases and nothing else" |
| H3 (story 1 promises speech "while it is still being written") | HIGH | **closed** | `spec.md:55` — "The answer is spoken in segments as soon as the turn's message exists (FR-012, FR-013 …)" |

No finding of §1 was dismissed; three were fixed in the artifacts, one by editing the tasks.

### 6.2 New findings of this pass

| ID | Severity | Location | Finding | Disposition |
|---|---|---|---|---|
| X1 | **HIGH** | `tasks.md` T009/T010 vs T020, `plan.md:96` | The two halves of the delivery decision were written in different passes and contradicted each other: T009/T010 put **all** `LiveVoiceFrame`s on `SessionFrame` → the broadcast arm, while T020 puts **audio** on the owner's `ClientConnection`. Implemented literally, T009/T010 would re-create D9 (audio on `broadcast`), which is the bug this feature just paid for. | **Fixed**: T009 now adds the variant "for the control types only" and requires the publish helper to take a control-only enum; T010 states the arm is reached through `events.recv()` and adds a test that publishing `speech` is impossible by construction |
| X2 | MEDIUM | `tasks.md` T045 | Editing artifact: the sentence ended "… (`review.md` D13) signal is sent", two stray words from an earlier draft. Harmless to a person, confusing to whoever implements it. | **Fixed**: T045 rewritten in one sentence, and its measurement convention recorded |
| X3 | MEDIUM | `spec.md:263` (SC-002) vs `tasks.md` T045/T073 | SC-002 says speaking over the agent silences it "within 300 ms". The onset guard added in the review (250 ms of voiced energy before `live_voice_speech_start`) now consumes almost the whole server-side leg: 250 ms of guard plus the abort leaves ~50 ms of headroom. The perceptible part (client `player.stop()`) is still <= 50 ms, which is what a person experiences, but the two legs of SC-002 are no longer the same measurement and the spec does not say which one it means. | **Recorded** as the measurement convention in T045/T073 (server leg counts from the guard firing). **Not** changed in the spec: tightening or moving that number is the spec owner's call, and it belongs with RK-1's answer |
| M1 | MEDIUM | `spec.md:216` (FR-016) | "Raw audio MUST NOT be stored" is a testable requirement that **no task asserted**: nothing would fail if the implementation saved the utterance as an attachment (the existing `AudioChat` path does exactly that). | **Fixed**: T080 asserts it end to end (buffer dropped, no `SessionFile`, stored conversation holds text only) |
| M2 | MEDIUM | `spec.md:253` (FR-035) | "offered on the chat interface only … MUST NOT be offered on Discord or Telegram" had no task: the risk is not that a bot gains a button, it is that a `live_voice` frame leaks into readers that cannot render it. | **Fixed**: T081 |
| M3 | MEDIUM | `spec.md:265` (SC-004) | "a 10-minute session of at least 15 turns … no lost turns in 100%" is the criterion that catches the whole class of lost-turn bugs, and nothing measured it: the phase 3-7 e2e checks are single-turn. | **Fixed**: T082 (soak) |
| L1 | LOW | `tasks.md` throughout | 9 FRs (FR-008, FR-012, FR-015, FR-018, FR-023, FR-027, FR-034, FR-035) and 4 SCs (SC-004…SC-007) are satisfied by tasks that never cite the ID. Coverage is real but untraceable: `speckit-checklist` cannot answer "where is FR-018 handled" without reading all 83 tasks. | **Not fixed on purpose**: adding IDs to task text is churn for a reviewer's convenience; the mapping is written here instead (§6.3) |
| L2 | LOW | `spec.md:267` (SC-005) | "a person who has never used the feature is talking within 20 seconds" is a usability measurement with no instrument and no owner. | **Accepted as manual**: it belongs in the `quickstart.md` by-hand pass (T077) and cannot be automated honestly |

### 6.3 Coverage, counted

42 FRs (FR-001…FR-041 + FR-036a), 9 SCs, **83 tasks** (T001…T083), 21 `[P]`, 49 story-tagged:
US1 19 · US2 8 · US3 7 · US4 8 · US5 7.

FRs cited by at least one task: 33 of 42. The nine that are covered without being cited:

| FR | Where it is actually implemented |
|---|---|
| FR-008 | T025 (engines built from the agent's own speech settings), T027 (submit through the ordinary request path) |
| FR-012 | T031 (speech frames), T036/T037 (text already in the conversation before it is spoken) |
| FR-015 | T037 (`live_voice_state` returns to `listening`), T048 |
| FR-018 | T052 (typing stops the audio), T054 (a typed message is answered and spoken) |
| FR-023 | T044 (mark only when the `Message` had arrived), T047 (e2e keeps the mark) |
| FR-027 | T044 (abort after the audio stops) |
| FR-034 | T062 (text kept after every ending), T067 |
| FR-035 | T081 (added in this pass) |
| SC-006 | T075 (flag matrix), T078 (dummyplug) |
| SC-007 | T057 (one release path), T062 (track `ended`) |

### 6.4 What still needs an owner other than me

1. **SC-002 / X3** — the spec should say which leg of "within 300 ms" it means. Owner: spec author.
2. **FR-013 / RK-1** — "live" means "as fast as the model's whole answer". Accepted as a risk, open with the User.
3. **D8 / Q4** — no session cap in the spec, a cap in `@bro-pm`'s summary. Open with the User.
4. **SC-005 / L2** — manual by nature; no instrument proposed, and none is worth building.

**Verdict of this pass**: the artifact set is internally consistent enough to implement. The one
finding that could have cost a rewrite was X1, and it is closed in `tasks.md`.
