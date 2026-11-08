# Specification Analysis Report — SPEC-055 Live Voice

**Command**: `speckit-analyze` (read-only) · **Analyst**: `@teteh-qa` · **Date**: 2026-11-08
**Feature**: `specs/055-live-voice` · **Branch**: `055-live-voice`
**Supersedes**: every earlier draft of this file. Findings that closed in between are kept in §3 with
their evidence — including the ones closed by `@mamang-lead-be`'s patches rather than by me.

## 0. Snapshot this analysis is valid for

```
HEAD  f29f3f0d29547e049b5c0406a91e4f1e2796a67e
sha256[:12]  spec 0ce867f17859 · plan a8ac991ebdbb · tasks 38e94f67a9f5 · quickstart f0b118b04f93
lines        spec 344 · plan 288 · tasks 299 · quickstart 131
counts       42 FR (FR-001..FR-041 + FR-036a) · 9 SC · 5 user stories · 85 tasks (T001..T085) · 24 [P]
note         task ids below were shifted by one at T067+ when T067 (re-bind the audio lane on reconnect)
             was inserted; every reference in this file and in `checklists/` was renumbered with it
```

**Read this before quoting a task number anywhere.** The branch moved four times while this analysis
ran (`7454df5f` → `801239b5` → `f29f3f0d`), and `f29f3f0d` **inserted T022**, renumbering everything
from the old T022 upward. Task IDs in this file are the `f29f3f0d` IDs. Any baseline handed to
`@sepuh-be` must name the **commit**, not a count or an ID range; the counts in the artifacts
themselves have already drifted once (§1 M12).

## 1. Findings open at this commit

| ID | Category | Severity | Location(s) | Summary | Recommendation |
|----|----------|----------|-------------|---------|----------------|
| H7 | Underspecification | **HIGH** | `spec.md:168-170` (edge case: second person / television) | The edge case requires that "only speech the session **can attribute to the person**" is submitted. Nothing in plan, contract or tasks provides speaker attribution: the detector is an energy gate on one microphone (T034, T046). The requirement promises a capability the architecture does not have, in the list a reviewer reads as acceptance-level. | Owner is the spec author: restate as "background speech may be transcribed; the person corrects it by speaking again", or open speaker attribution as its own spec. |
| H9 | Coverage / Inconsistency | **HIGH** | `spec.md:212` (FR-018), `architecture.md:277-278` (I8), `tasks.md:134` (T055), `tasks.md:132` (T053) | FR-018 and I8 require that a message typed during a session is "answered **and spoken**", and T055's independent test asserts it. No task implements it, and the mechanism does not exist: the session speaks only replies arriving on **its own** `ResponseSender` (`architecture.md:103-105`, T029), while a typed message is submitted by the Composer on a different path. An earlier draft counted FR-018 as "covered by T052 + T054" (now T053 + T055) — a stop-the-audio task and a test are not an implementation. | Name the mechanism and add its task: the Composer sets `expect_audio_reply: true` (`schema/request.rs:297`, already exists) while a session is live and the server routes that reply's audio to the session; or the session re-submits the typed text as its own turn. Until then, remove "and spoken" from T055. |
| M1 | Coverage | MEDIUM | `spec.md:196` (FR-004), `tasks.md:52` (T011) | FR-004 requires the control to "name what is missing and how to set it". T011 computes the boolean; `grep -ci 'what is missing' tasks.md` → `0`. Nothing renders the refusal. | Extend T037/T038 with the message and a test (which of recogniser/voice, and where to set it). |
| M11 | Ambiguity | MEDIUM | `spec.md:170,223` (very short utterance), `tasks.md:114` (T046), `tasks.md:115` (T047) | The edge case wants a very short word ("yes", "no", "stop") **submitted**; T046 says a burst shorter than the ~250 ms guard resumes playback and submits nothing. No unit is defined for "a word", so the same 200 ms "no" is a message under one sentence and noise under the other. | State the floor in the guard's units (e.g. "below the guard is noise; a recognised word above it is a message") and check it against FR-024. |
| M12 | Inconsistency (counts) | MEDIUM | `tasks.md:188` (T079), `tasks.md:299`, `quickstart.md:35-76` | Two counts inside the artifacts are already stale. T079 says "each of its **nine** steps" while `quickstart.md` has **ten** (step 10 is the measurement step added with T075); and `tasks.md:299` says "Phase 2 (now 15 tasks, T007–T021)" while Phase 2 is **16** tasks ending at **T022** after this commit. The handoff document misreports the run it describes. | T079 → "every step"; the phase sentence → "16 tasks, T007–T022". Prefer wording that cannot drift over exact counts. |
| M14 | Ambiguity (measurement) | MEDIUM | `spec.md:263` (SC-002) vs `tasks.md:114` (T046), `tasks.md:184` (T075) | SC-002 says speaking over the agent silences it "within 300 ms" without saying which leg: the audible stop on the client (≤50 ms) or the abort reaching the agent (250 ms guard + ~50 ms). T046/T075 now record the convention (server leg counts from the guard firing), so the plan is measurable — the requirement still is not, and a reader of the spec alone gets a stricter promise than the system makes. | Spec owner adds one clause: "audible silence is measured on the client from the first voiced sample; server-side cancellation is bounded separately at 300 ms from the guard". |
| M5 | Inconsistency (open question) | MEDIUM | `spec.md:298` (Q4) vs PM's D8 | The spec's Q4 is a cap on **concurrent** sessions (default: none, deployment socket limits apply). D8 asks about a cap on **session duration**, which the spec never mentions, so D8 cannot be answered "from the spec default" — yet it is being put to the User that way. | Keep Q4 as concurrency; a duration cap is a new requirement plus a task on the idle path (FR-029). Put the distinction in front of the User, not the default. |
| M15 | Consistency (design) | MEDIUM | `tasks.md:50-51` (T009/T010) vs `tasks.md:61` (T020) | The route is now split correctly (control via the broadcast arm, audio via the connection lane), but it leaves **two serialisers for one wire shape**: T010 maps `SessionFrame::LiveVoice` to a `live_voice` JSON object by hand, while T020 hands a `LiveVoiceFrame` to `ClientConnection::send`, which serialises it independently. Two encoders of one type drift, and the client cannot tell which route a frame took. | One `LiveVoiceFrame::to_json()` used by both paths, plus a test that the two routes produce identical JSON for the control types. |
| M16 | Terminology | MEDIUM | `tasks.md:50` (T009) vs `contracts/live-voice-frames.md:140-141`, `tasks.md:98` (T041) | T009 lists `stall` as a frame **type**; the contract and T041 say the notice is `live_voice_error { kind: "stall" }` — an error frame carrying a kind, not a type. A client classifier written from one document mishandles the other. | Keep `error` + `kind: "stall"` (contract, T041) and drop `stall` from T009's type list. |
| L2 | Measurability | LOW | `spec.md:267` (SC-005) | "Talking within 20 seconds, without reading documentation" has no instrument and no owner; it is a usability measurement, not a gate. | Accept as manual (T079's by-hand pass) and say so in the spec, or drop it from the gate list. |
| L4 | Traceability | LOW | `tasks.md` | 9 FRs (FR-008, FR-012, FR-015, FR-018, FR-023, FR-027, FR-034, FR-035) and SC-006 are satisfied without citing the ID, so "where is FR-x handled" needs a full read. Coverage is real for all but FR-018 (H9). | Optional: add the IDs where §2 already knows the mapping. Not worth churn on its own. |

Nothing above changes a User requirement. H7, M14 and M5 are spec text (owner: the spec author);
H9, M1, M11, M12, M15, M16 are `@mamang-lead-be`'s, each a one-to-few-line edit.

## 2. Coverage summary

### Requirements → tasks

| Requirement | Has task? | Note |
|---|---|---|
| FR-001..FR-004 | ✅ | T037; FR-004 only as a boolean (M1) |
| FR-005..FR-007 | ✅ | T026/T060/T027, T038/T035, T029 |
| FR-008 | ⚠️ implicit | T026 built from the agent's own speech settings, T029 the ordinary request path |
| FR-009..FR-011 | ✅ | T034, T017/T018/T029 |
| FR-012, FR-015 | ⚠️ implicit | T032/T037/T038; T038/T049 |
| FR-013, FR-014, FR-017 | ✅ | T024/T031, T018 |
| FR-016 | ✅ | **T082** — no stored audio, asserted end to end |
| FR-018 | ❌ **none** | H9 — I8 promises it, T053 only stops audio, T055 asserts it |
| FR-019, FR-022, FR-024..FR-026 | ✅ | T043–T049, T052, T053 |
| FR-020, FR-021 | ✅ | T050, T051, T054 |
| FR-023, FR-027, FR-034 | ⚠️ implicit | T045, T048; T045; T063/T069 |
| FR-028..FR-030 | ✅ | T058–T061, T070 |
| FR-031..FR-033 | ✅ | T072, T069, T066 |
| FR-035 | ✅ | **T083** — channel neutrality as a failing test |
| FR-036, FR-036a, FR-037 | ✅ | T030 (cascade trait, language argument) |
| FR-038..FR-041 | ✅ | T038, T073, T008/T014, T001/T014 |

| Success criterion | Has task? | Note |
|---|---|---|
| SC-001, SC-002, SC-003 | ✅ | T075 marks + T076 spans + quickstart step 10; SC-002's leg still ambiguous (M14) |
| SC-004 | ✅ | **T084** soak (10 min, 15 turns, no lost turns) |
| SC-005 | ⚠️ manual | L2 — by-hand pass in T079 |
| SC-006 | ⚠️ implicit | T077 flag matrix + T080 dummyplug; not named in the criterion |
| SC-007..SC-009 | ✅ | T057/T063, T042/T046, T065/T068 |

### Unmapped / process tasks

T005, T006, T016, T074, T077, T078, T081, T085 carry no requirement by design (scaffolding, build
matrix, docs, cleanup, handoff). No task is orphaned.

### Constitution alignment (`.specify/memory/constitution.md`)

| Principle / gate | Status |
|---|---|
| I. Lean by default | ✅ no new mandatory dependency: `base64`, `tokio::sync`, `serde` already locked; the worklet ships (`webui/public/vizier-capture-worklet.js`) |
| II. DRY via trait seams | ✅ one `LiveVoiceEngines` trait (T030), no type-tag branching in the task list |
| Optional capability behind a flag, API shape unchanged | ✅ `provider-live-voice`, `default = []` (T001); `live_voice: bool` additive in both builds (T011) |
| dummyplug e2e + scripted `quickstart.md` | ✅ three scripted steps (`quickstart.md:77-108`), run by T080 |
| Design before UI code | ✅ wireframe `diagrams.md` §5, record `review.md:135-139`. Caveat: the approval is the author's own; acceptable for the gate, worth one look by the PM at the first demo |
| Manual run for runtime-affecting changes | ⚠️ not applicable yet (no code); T079/T080 carry it later |

### Metrics

- Total requirements: **42 FR** + **9 SC** + 5 user stories
- Total tasks: **84** (24 `[P]`): Phase 1 6 · Phase 2 16 · US1 19 · US2 8 · US3 7 · US4 8 · US5 7 · Polish 13
- FR cited by ID: **33/42**; FR with no implementation at all: **1** (FR-018)
- SC with a task: **7/9** explicit + 1 manual (SC-005) + 1 implicit (SC-006)
- Findings open: **10** (2 HIGH, 7 MEDIUM, 1 LOW) — overflow: none
- Closed since the first draft: **16** (§3)

## 3. Closed since the first draft — with evidence, so it is not re-litigated

| ID | Was | Now | Evidence at `f29f3f0d` |
|---|---|---|---|
| C1 | `quickstart.md` had no dummyplug steps (constitution MUST) | **closed** | `grep -c dummyplug quickstart.md` → 6; §"The `dummyplug` half"; T080 runs it |
| H1 | T044 called the non-existent `AgentCommand::Abort` | **closed** | T045 names `AgentsClient::abort` (`agents_client.rs:207`, impl `:298-312`); `AgentCommand` (`schema/commands.rs:142`) has no `Abort`; `agents_client.rs:415` is `FakeCall::Abort` |
| H2 | FR-023 vs FR-027 disagreed | **closed** | `spec.md:224` defers to "exactly FR-023's two cases and nothing else" |
| H3 | US1 promised speech "while it is still being written" | **closed** | `spec.md:55` and the flow node: "spoken in segments once the turn's message exists" |
| H4 | SC-001/002/003 had no measurement points | **closed** | T075 (client marks + server spans), T076 (p50/p90/p99 in `latency.md`), quickstart step 10 |
| H5 | FR-037 had no task | **closed** | T030 passes the conversation's language to `VizierStt::transcribe` (`stt/mod.rs:77-84`) |
| H6 | SC-002's 300 ms vs the 250 ms guard (arithmetic) | **closed** | `plan.md:192-194`: `player.stop()` is immediate on the client; the guard delays only the server. Which leg the *spec* means is M14 |
| H8 / X1 | T009/T010 put **all** live-voice frames on the broadcast while T020 put audio on the connection lane | **closed** | T009 adds the variant "for the control types only" plus a publish helper that cannot take a `speech` frame; T010 states its arm carries control only. Verified in code: the arm sits inside `events.recv()` (`topic.rs:531`) and `SessionFrame` is matched exhaustively (`transport/mod.rs:58`). Residual seam: M15 |
| H10 (new, closed same day) | The `Lagged` arm had no owner after D9 | **closed** | **T022** states the arm now drops control/text only (audio never enters the ring) and tests a 2 000-frame audio burst against a pending `Response` |
| X2 | T045 ended with two stray words ("signal is sent") | **closed** | T046 rewritten, measurement convention recorded |
| M2 | FR-016 had no test | **closed** | **T082** |
| M3 | FR-035 had no channel test | **closed** | **T083** |
| M7 | quickstart step 4 used pre-clarification FR-023 wording | **closed** | `quickstart.md:44-48` spells out both cases |
| M8 | `data-model.md` said "complete-as-produced" | **closed** | no hit |
| M9 | The spoken marker had three names | **closed** | `data-model.md:101`: `VizierRequest.spoken: bool`, `#[serde(default)] = false` |
| M10 | No record that the mockup was reviewed | **closed (caveat)** | `review.md:135-139` D10b; see the constitution row |
| L1 | Two Delta Log entries described FR-023 differently | **closed** | `spec.md:319` struck through, marked "superseded below" |

The audio-path finding from my 15:22 message (`broadcast` + `Lagged` could evict reply text) is closed
and verified in code, not only in prose: `SESSION_EVENTS_CAPACITY: usize = 1024`
(`transport/mod.rs:29`), the `Lagged` arm at `topic.rs:609-613`, the rule at
`contracts/live-voice-frames.md:122-140`, `ClientConnection::send` (`connection.rs:82`; queue 64 at
`topic.rs:141`) with `LIVE_VOICE_SOCKET_BACKLOG = 16`, and now T022 for the arm itself.

## 4. Next actions

1. **Before `speckit-implement`**: **H9** — FR-018 either gets a mechanism plus a task, or T055 loses
   its "and spoken" half. It is the only finding that leaves an acceptance test unachievable.
2. **`@mamang-lead-be`, one edit each, non-blocking**: M15 (one serialiser), M16 (`stall` as a kind),
   M12 (two stale counts), M1 (FR-004 message), M11 (short-utterance floor).
3. **Spec author only**: H7 (attribution), M14 (which leg of SC-002), M5 (Q4 concurrency vs D8
   duration — two different questions for the User).
4. **Accept or drop**: L2 (SC-005 is manual by nature), L4 (traceability churn).
5. Checklist for this same commit: `specs/055-live-voice/checklists/live-voice.md` (CHK001–CHK058 plus
   this pass's CHK059–CHK064).
6. **Freeze the tree for the next pass.** It moved four times during this one; a report is only as good
   as the commit it names.
7. Implementation still needs the User's explicit go. Nothing here grants it.

**Remediation offered, not applied** — I can write the concrete diff for H9, M12, M15, M16, M1 and M11
on request. I do not edit `spec.md`, `plan.md`, `tasks.md` or the contracts myself.

## 5. How to re-run every claim in this file

```bash
cd ~/ngoding/pribadi/vizier
git rev-parse HEAD                                          # f29f3f0d...
grep -c '^- \[ \] T' specs/055-live-voice/tasks.md          # 84
grep -c '\[P\]' specs/055-live-voice/tasks.md               # 24
grep -ci 'FR-018' specs/055-live-voice/tasks.md             # H9: 0
grep -ci 'what is missing' specs/055-live-voice/tasks.md    # M1: 0
grep -n 'nine steps' specs/055-live-voice/tasks.md          # M12: T079
grep -n 'Phase 2 (now' specs/055-live-voice/tasks.md        # M12: says 15, is 16
grep -n 'attribute' specs/055-live-voice/spec.md            # H7: the edge case itself
grep -n 'stall' specs/055-live-voice/contracts/live-voice-frames.md   # M16: kind, not type
grep -n 'SessionFrame::LiveVoice' specs/055-live-voice/tasks.md       # M15: T009/T010
```

