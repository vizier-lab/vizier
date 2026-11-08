# Requirements Quality Checklist — Live Voice (SPEC-055)

**Feature**: `specs/055-live-voice` · **Created**: 2026-11-08
**Audience**: reviewer (PR gate) · **Depth**: standard · **Author**: `@teteh-qa`
**Focus**: (1) the two interruption paths, (2) measurability of the latency/behaviour gates
**Snapshot**: tree at `HEAD ea21843f` + 5 uncommitted edits; hashes in `analysis.md` §0.

Each item tests the **requirements writing**, not the implementation. An item is answered by
pointing at the sentence in `spec.md` / `plan.md` / `tasks.md` that settles it; if no sentence
exists, the item fails and belongs to the spec or plan owner.

## Requirement Completeness

- [ ] CHK001 - Is the button that starts the session fully specified — label, placement, and the rule that it is visually separate from the existing mic and spoken-reply controls? [Completeness, Spec §FR-001/FR-002]
- [ ] CHK002 - Is every state the control must show enumerated with one term each (off, connecting, listening, waiting, speaking, interrupted, reconnecting, ended, failed)? [Completeness, Spec §FR-006]
- [ ] CHK003 - Is the "marked as spoken" marker on a submitted message defined as a named, persisted field with a default for messages created before the feature? [Completeness, Gap, Spec §FR-011/FR-017]
- [ ] CHK004 - Are the two interruption paths each given their own trigger, their own state transition and their own effect on the running turn? [Completeness, Spec §FR-019/FR-020/FR-027]
- [ ] CHK005 - Is what the interface shows while a session ends specified per end reason, not only "a reason"? [Completeness, Spec §FR-028/FR-029/FR-030]
- [ ] CHK006 - Is the microphone-release obligation stated for every terminal path, including reload, process restart, and device removal? [Completeness, Spec §FR-028/FR-034, Edge Cases]

## Requirement Clarity

- [ ] CHK007 - Is "at once" quantified for each interruption path with a threshold and the moment it is measured from? [Clarity, Spec §FR-019/FR-020/SC-002/SC-003]
- [ ] CHK008 - Is "the first speakable segment" defined by an objective boundary rule rather than left to interpretation? [Clarity, Spec §FR-013, Plan Phase C]
- [ ] CHK009 - Is "too short to be a message" quantified (a duration, an energy floor, or a recogniser result) rather than described qualitatively? [Ambiguity, Spec §FR-024/Edge Cases]
- [ ] CHK010 - Is "the person's voice interrupts it" distinguished from "background speech" by a rule an implementer can apply, given that no speaker identity is available? [Ambiguity, Spec §FR-026/Edge Cases]
- [ ] CHK011 - Is "answer in a form that suits listening" (§FR-017) given a checkable expectation, or is it explicitly left to the model? [Clarity, Spec §FR-017]

## Requirement Consistency

- [ ] CHK012 - Do FR-023 and FR-027 give the same account of what survives an interruption? [Conflict, Spec §FR-023/FR-027]
- [ ] CHK013 - Does the User Story 1 narrative agree with FR-012 about speech starting after the turn's message exists, not while it is being generated? [Conflict, Spec §US1 vs §FR-012]
- [ ] CHK014 - Do the interaction-flow diagram's edge labels match the FR wording they summarise? [Consistency, Spec §Interaction Flow]
- [ ] CHK015 - Does every artifact use one name for the "came by speech" field (`spoken` / "marked as spoken" / "additive flags")? [Consistency, Consistency, Spec §FR-011, Data-model §4]
- [ ] CHK016 - Are the session end reasons in the spec the same set the contract's `live_voice_error` / state frame can carry? [Consistency, Spec §FR-028, Contracts]
- [ ] CHK017 - Is the open question about caps unambiguous between "cap on concurrent sessions" and "cap on session duration"? [Conflict, Spec §Q4]

## Acceptance Criteria Quality

- [ ] CHK018 - Does SC-001 name both endpoints of the measurement (end of the person's speech, first audible word) and where each is observed? [Measurability, Spec §SC-001]
- [ ] CHK019 - Does SC-002's 300 ms budget survive the plan's own ~250 ms onset guard, leaving a stated margin for transport and cancellation? [Measurability, Spec §SC-002 vs Plan Phase D]
- [ ] CHK020 - Is SC-003's "200 ms in 100 of 100 tries" measurable without a defined start event? [Measurability, Spec §SC-003]
- [ ] CHK021 - Is SC-004's "15 turns in 10 minutes" given an environment, a provider class, and a way to count what a hands-free turn is? [Measurability, Spec §SC-004]
- [ ] CHK022 - Is SC-005's "talking within 20 seconds" given a start point and an observer? [Measurability, Spec §SC-005]
- [ ] CHK023 - Does SC-006 name the existing checks that prove the speech behaviour is unchanged? [Measurability, Spec §SC-006]
- [ ] CHK024 - Is every "at least X% of attempts" criterion given a sample size and a stopping rule? [Measurability, Spec §SC-002/SC-004/SC-005]

## Scenario Coverage

- [ ] CHK025 - Are primary, alternate (typing mid-session), exception (provider failure) and recovery (reconnect) flows each covered by at least one requirement? [Coverage, Spec §FR-018/FR-032/FR-033]
- [ ] CHK026 - Is the "session keeps listening after a reply ends" path required without the person pressing anything? [Coverage, Spec §FR-015]
- [ ] CHK027 - Is the case "a second person or a television is speaking" given a requirement outcome the implementer can meet with an energy gate? [Coverage, Gap, Spec §Edge Cases]
- [ ] CHK028 - Is the case "the agent is working rather than speaking" given exactly one behaviour for both interruption paths? [Coverage, Spec §FR-021/FR-027]
- [ ] CHK029 - Is a build without the capability stated as a first-class scenario, including what the client sees and what the existing clients see? [Coverage, Spec §FR-041/FR-040]

## Edge Case Coverage

- [ ] CHK030 - Is the precedence between the onset guard and a genuinely short utterance ("no", "stop") written down? [Conflict, Edge Cases vs Plan Phase D]
- [ ] CHK031 - Is the outcome defined when the person speaks while the agent is thinking and nothing is audible? [Coverage, Spec §Edge Cases/FR-027]
- [ ] CHK032 - Is the outcome defined when the connection returns after the reconnect window with a turn that had already produced text? [Coverage, Spec §FR-033/FR-034]
- [ ] CHK033 - Is the "page reloaded mid-session" case given the same end-reason treatment as an explicit stop? [Coverage, Spec §Edge Cases]
- [ ] CHK034 - Are mobile suspension cases (screen lock, backgrounded WebView) given a reason code rather than only an outcome? [Coverage, Spec §Edge Cases/FR-028]

## Non-Functional Requirements

- [ ] CHK035 - Is the privacy boundary stated for both directions (audio in, spoken audio out) and for every provider call? [Completeness, Spec §FR-016/FR-036, Assumptions]
- [ ] CHK036 - Is the cost expectation per session stated well enough to decide whether the idle stop is the only guard? [Clarity, Spec §Assumptions]
- [ ] CHK037 - Is there a documented bound on what a slow or stalled client costs the server, and is it expressed in the same units in spec, plan and contract (frames vs seconds vs bytes)? [Consistency, Plan Fallbacks, Contracts §Limits]
- [ ] CHK038 - Is the CPU/battery expectation for continuous capture stated, or explicitly declared out of scope? [Gap, Non-Functional]
- [ ] CHK039 - Is the accessibility expectation for a voice-only feature stated (keyboard reach to both controls, a non-audio path to every state)? [Gap]

## Dependencies & Assumptions

- [ ] CHK040 - Is every assumption about the hosted provider (latency class, echo behaviour, segment synthesis) marked as an assumption with the fallback it implies? [Assumption, Spec §Assumptions, Plan §Latency]
- [ ] CHK041 - Does the "no new mandatory dependency" claim cover the **test** toolchain (Playwright, vitest, fixtures) as well as the shipped bundle? [Assumption, Plan §Technical Context]
- [ ] CHK042 - Is the browser-permission lifecycle (refusal, withdrawal, retry without reload) specified for each of the three surfaces the docs claim? [Coverage, Spec §FR-030/FR-035]
- [ ] CHK043 - Is the desktop CSP constraint recorded as a requirement-level limit on VAD choice rather than only as an implementation note? [Assumption, Plan §Constraints]

## Ambiguities & Conflicts

- [ ] CHK044 - Does any requirement depend on an API symbol that does not exist in the repository as named? [Conflict, Tasks §T044]
- [ ] CHK045 - Are the counts quoted inside the artifacts (task totals, phase sizes) consistent with the files they describe? [Consistency, Tasks §Delta, Review §6]
- [ ] CHK046 - Is every requirement that the QA check must observe traceable to at least one task by ID? [Traceability, Gap, Spec §FR-016/FR-035/FR-037]
- [ ] CHK047 - Is the measurement instrumentation for the latency gates an owned task rather than a phase note? [Traceability, Gap, Plan §Latency]
- [ ] CHK048 - Are the feature's checks scripted in `quickstart.md` as dummyplug steps (message in, expected reply/history entry) as the constitution requires? [Conflict, Constitution §Development Workflow, Gap]

## How this checklist was re-checked

The artifacts moved while this checklist was written (see `analysis.md` §4-§5). Answers that changed
in the working tree between the first read and the last:

- **CHK044** — now **passes**: `tasks.md` T044 calls `AgentsClient::abort(agent_id, session)`, and
  `spec.md` FR-027 names the same path. (It failed on the tree this checklist started from.)
- **CHK012** — now **passes** (uncommitted): FR-027 defers to FR-023's two cases.
- **CHK014** — now **passes** (uncommitted): the flow node reads "Reply spoken in segments once its
  message exists".
- **CHK047** — now **passes**: T073 defines the three gates with client marks and server spans.
- **CHK048** — still **fails**: `quickstart.md` has no dummyplug steps.
- Everything else is unchanged from first read.

Two of these are worth keeping as habits: an item that fails here is a request to the owner of the
sentence, not a bug report, and an item that starts passing because someone edited the sentence is a
pass — no need to keep it open for the sake of appearance.

## Re-check at `HEAD 7454df5f` and items added — `@mamang-lead-be`, 2026-11-08

Re-run authorised by `@bro-pm`, on the tree analysed in `analysis.md` §6 (same snapshot). Two things:
the items of the first read are re-tested, and the decisions taken *after* that read get items of their
own, because a decision nobody wrote a requirement-quality question about is a decision nobody checked.

### Re-test of the first read

| Item | Was | Now | Why |
|---|---|---|---|
| CHK048 (dummyplug steps in `quickstart.md`) | fails | **passes** | `quickstart.md:77-95` scripts the agent-observable half: setup, the message, the expected reply, and the stored `Request` with `spoken: true`; the audio half is explicitly marked as needing a real provider |
| CHK012 (FR-027 vs FR-023) | fails, then passing | **passes, committed** | `spec.md:224` defers to FR-023's two cases in the requirement itself |
| CHK014 (story says "spoken while it is still being written") | fails, then passing | **passes, committed** | `spec.md:55` and the flow node now say "in segments as soon as the turn's message exists" |
| CHK044 (the abort path exists) | fails, then passing | **passes, committed** | T044 calls `AgentsClient::abort`; the earlier symbol was a test fake |
| CHK047 (measurement is an owned task) | fails, then passing | **passes, committed** | T073 names the three marks; T074 the spans |

### Added items

- [ ] CHK049 - Is it stated, in the requirement and not only in the plan, that the reply's audio is delivered to **one** connection and never through the session broadcast, with the reason (a broadcast drops all-or-nothing per subscriber)? [Consistency, Ambiguity, Spec §FR-012/FR-019 + Contract §Delivery path]
- [ ] CHK050 - Does the contract say which frame types may be dropped and which may never be (audio yes, `state`/`transcript`/`error` no), and what the person observes when a drop happens (`stall`)? [Completeness, Edge Case, Contract §Delivery path]
- [ ] CHK051 - Is the difference between "the queue is full" and "the socket is closed" defined, so that a busy socket cannot be mistaken for a closed tab? [Edge Case, Clarity, Contract §Delivery path, `review.md` D10]
- [ ] CHK052 - Are the measurement points for SC-001/SC-002/SC-003 named precisely enough that two people measuring the same run get the same number (which event starts the clock, which event stops it, on which side)? [Measurability, Clarity, `latency.md` via T073]
- [ ] CHK053 - Is it stated which leg of SC-002's "within 300 ms" is meant — the audible stop on the client or the abort reaching the agent — given that the onset guard consumes 250 ms of the server leg? [Ambiguity, Consistency, Spec §SC-002 vs T045]
- [ ] CHK054 - Is "raw audio is never stored" expressed as an observable outcome (no attachment, no history entry, nothing on disk) rather than as an intention? [Completeness, Spec §FR-016, T080]
- [ ] CHK055 - Is "the chat surface only" written as something a test can fail on — no `live_voice` frame and no control on the Discord/Telegram paths — rather than as a scope note? [Completeness, Spec §FR-035, T081]
- [ ] CHK056 - Does the spec define what "no lost turns" means for SC-004 in observable terms (how many turns, how long, what counts as lost), so a soak test can be written against it? [Measurability, Spec §SC-004, T082]
- [ ] CHK057 - Is the resume window after a reconnect (30 s) bound to a rule about the message in flight — may it be re-submitted, must it be deduplicated, who decides? [Consistency, Edge Case, Spec §FR-033, T012/T065]
- [ ] CHK058 - Are the two interruption paths distinguishable in the requirement text by *who starts them* and *what follows* (a message, or nothing), rather than only by which control was used? [Clarity, Spec §FR-019/FR-020/FR-024]

### How these were checked

Each item is answered by a sentence, not by code: CHK049-CHK051 by `contracts/live-voice-frames.md` §Delivery
path (answered, passes); CHK052 by T073's marks (passes as a plan item, but see CHK053); CHK053 is
**open** — the spec says "within 300 ms" and nothing there says which leg, so it stays a request to the
spec owner (recorded as X3 in `analysis.md` §6.2); CHK054-CHK056 pass because T080-T082 exist as of this
pass; CHK057 passes on FR-033 plus T012/T065; CHK058 passes on FR-024's "unless they were too short to
be one".

One open item out of ten added, and it is the one that only the spec owner can close.
