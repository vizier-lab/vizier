# Work Queue (decision board)

Owner: `@mamang-lead-be`. Agreed 9 Nov 2026 (kang muruyung ACC in Workspace `squad-leluhur`).
Purpose: one place anyone can read, so that "queue" is not just politeness.

Document status: **RATIFIED** — kang's ACC 9 Nov 2026 **20:42 WIB** (gate/QA/designer flow) and **20:55 WIB** (spec ownership + `R:` field + `[DOD+]` + tripwire). Applies to all projects.

## Basic rules
- **WIP = 1 per agent**, not per team. `@sepuh-be` never holds two briefs at once; QA/designer may run in parallel.
- A new task only enters `aktif` if that agent's slot is empty. If full -> `antre`, and **`@bro-pm` is the one who reports to kang**, not the executing agent.
- Status: `antre` -> `aktif` -> `gate` -> `siap-kirim` -> `selesai` (plus `batal`).
  - `gate` = three signatures not yet complete. `siap-kirim` = three signatures complete, **kang has not accepted yet**. `selesai` = kang accepted. There is no `selesai` without kang.
- Who changes a row: the Lead (technical status) and `@bro-pm` (priority). Executing agents only report.
- Every `aktif`/`gate` row MUST have a DoD (verification command + expected output) **and** the `R:` field (see below). Without that = not yet allowed to be `aktif`.

## Artefact ownership (ACC 20:55)
| Artefact | Owner | Contents |
|---|---|---|
| `spec.md` | `@system-analyst` | `FR-###`, acceptance criteria, scope in/out, actors, Delta Log |
| `tasks.md` | Lead | technical breakdown + DoD (command + expected output) |
| Priority & trade-off | `@bro-pm` | what matters, what is postponed |

Spec = **what is correct**. DoD = **how it is proven**. `specs/**`: the Lead may edit **mechanically** (paths, links, format); **requirement semantics = `@system-analyst`**. Numbering uses the existing `FR-###` — creating a second scheme is **banned**. The registry MUST distinguish **live / struck / deferred** (including lettered variants such as `FR-036a`).

## Field `R:` — mandatory on every row
- `R: FR-0xx` **or** `R: tanpa-spec:<alasan>`. Both empty = **MUST NOT be `aktif`**.
- `tanpa-spec` is only valid if `git diff --name-only` is **entirely** within the allowlist: `specs/**`, `docs/**`, `*.md`, `scripts/**`, test files. One path in `crates/**`, `webui/**`, `app/**` -> **MUST have an FR**, not a discussion. Verified on **paths**, not on the reason sentence.
- The `R:` value MUST appear in the `siap-kirim` report.
- The allowlist is verified as **`<base>..<commit>`**, not `git diff --name-only` in the working tree: another agent's uncommitted changes will indict a row whose commit is clean (9 Nov: 13 `crates/**` files modified when AQ-001 was checked).

The next `tasks.md` is born from `.specify/templates/tasks-template.md`, and since commit `80cadbac` that template carries `R:`, `[DOD+]`, and two evidence markers — so this rule survives when `speckit-tasks` is re-run. Not retroactive: an old `tasks.md` without `R:` is not void; the obligation is checked per row when that task enters `aktif`.

## `[DOD+]` — extra criteria from the Lead
- Criteria beyond the spec MUST be labelled `[DOD+]` and **machine-checkable** (performance budget, file size, contrast). "Quality" without a verification command is not a criterion.
- Appears in **3 tasks** -> **MUST be decided** within one spec cycle: promoted to `FR` or **revoked**. There is no eternal `[DOD+]`.
- A promotion touching behaviour/scope still goes through **kang's ACC** (drafted by `@system-analyst`, submitted by `@bro-pm`).


## The `@system-analyst` task path (`@bro-pm` decision 9 Nov 2026 20:49; written here at his request)

**Status: RATIFIED by kang muruyung, 10 Nov 2026 09:36** (*"Iya terima juga"*, answering item 2 in workspace `vizier-dev`/`general`). From here on this section is **binding**: spec/analysis tasks to SA come from `@bro-pm`. Before that date it was only a PM decision not yet valid — recorded so that nobody thinks it applies retroactively.

- Spec/analysis tasks to SA come **from `@bro-pm`**, not from the Lead: their output *precedes* implementation, so if the Lead assigned them, upstream author and downstream consumer become one person.
- The boundary is threefold: (1) what the PM sends is pre-implementation artefacts (requirements, SRS/FRD, analysis, spec, Notion); (2) spec changes caused by downstream findings (C-13/C-14, a qa-plan that finds a missing requirement) come in **through the Lead**; (3) SA does not receive code tasks or briefs with a code DoD.
- SA's signatures are two, not three: the User's approval at Gate 1 + the Lead's statement that the spec is sufficient to be broken into tasks.
- The prohibition "the PM never sends tasks to implementers" is unchanged. The `@sepuh-be`, QA and designer paths remain as in §Gate below.

## `speckit*` skills mandatory in vizier (User's order 9 Nov 2026 20:57)

The full flow is in memory `vizier/speckit-workflow-roles`; only one thing binds this board: **every vizier task names the `speckit*` skill used and the artefact produced**, or `tanpa-skill:<alasan>` for a task that is genuinely not spec-kit work (e.g. path fixes, document reconciliation). Without that, the brief is not complete.

- Step owners: specify/clarify = `@system-analyst` · plan/tasks/constitution/git = Lead · implement = `@sepuh-be` · analyze/checklist = `@teteh-qa`.
- `tasks.md` is a `speckit-tasks` artefact, not a loose document; numbering defects in it are checked with `speckit-analyze`, not a manual grep repeated every round.
- The automatic requirement check (§ above) is **not** a replacement for `speckit-analyze`/`speckit-checklist` — an extra tool at the Lead's gate, not a new step in the flow.

## Gate: one gate, three signatures
There is no separate gate per discipline. One `gate` status, three conditions, one truth.

| Signature | Owner | Mandatory evidence |
|---|---|---|
| Technical | Lead | command + exit code + commit SHA (name the repo: `vizier@…` / `webui@…`) + `file:line` |
| Behaviour vs spec | `@teteh-qa` | repro steps + actual vs expected + severity |
| Visual vs handoff | `@teteh-desener` | screenshot + numeric delta (px/token) + handoff reference |

- A **visual signature only for tasks labelled `ui`**; backend tasks are not given an empty status.
- A designer veto **MUST carry evidence**. A veto without evidence = taste, not a gate.
- The QA verdict is sent as **one message, mentioning the Lead + `@bro-pm`**.
- QA does not touch production code, does not change tests to make them pass, does not decide priority.
- **QA and the designer receive tasks from the Lead**, complete with the DoD + acceptance criteria from the spec.

## Finding rounds
- **A round = one bundle** (all blockers + majors in one submission). Minors do not count; minors become follow-up tasks with an ID.
- **Max 2 fix rounds.** Round 3 = **escalation to `@bro-pm`**, not another round.
- The fix deadline per round: **one work chunk**.
- P0/blocker (crash, data loss, security) is reported **immediately**.

## qa-plan (contract stage)
- Tiered: touches user behaviour / API contract -> **3-5 critical scenarios**; internal refactor -> `qa-plan: N/A - no behavior change`.
- **qa-plan only derives, it does not add.** A scenario without an `FR` reference is not written into the qa-plan; report it to the Lead.
- Scenarios are written by QA, test code is written by `@sepuh-be`, **QA reviews the test-file diff** (read only).

## Bugs that slip through to kang
MUST have one line of **"why it slipped"**: (a) there was no DoD for that case · (b) a DoD existed but was not run · (c) it was run but the tool did not catch it · (d) a known issue consciously accepted.
- (d) MUST immediately become a task with an ID. What gets fixed is the gate, not the person.

## Tripwire (not a new rule)
```
tripwire: task 1-3 | diblokir-spec: 0/3 | tanpa-spec: 0/5
```
If **2 of 3** tasks are blocked by "spec not ready", or **>2 of 5** rows use `tanpa-spec`, the rule is brought to `@bro-pm`/kang for revision — not loosened on one's own.


## Automatic requirement check (ACC 20:55, point 6; scope decided by `@bro-pm` 9 Nov 21:22)

One command in the repo — code written by `@sepuh-be`, semantics owned by `@system-analyst`, run at the Lead's gate — prints: defined ids, struck/deferred, referenced-but-undefined, lettered variants, and the live count. The output is pasted into the `siap-kirim` report; the spec is not used before the output is clean.

- **Default scope: per feature.** `--all` is only for audits and **MUST NOT** be used as the basis of a gate: broken FRs in another feature must not block the feature being worked on. The output **MUST** name its scope (`scope: 055`), so that "clean" is never read as "the whole repo is clean".
- **`@system-analyst` proposal (9 Nov 2026, not yet ACCed):** the output also names `live-without-carrier` (a defined FR not referenced by any task; named `hidup-tanpa-pembawa` in the original Indonesian proposal) and `deferred-still-referenced-by-qa-plan` (`deferred-yang-masih-dirujuk-qa-plan`). The reason: a command that only checks one direction can come out clean while a live requirement has no implementer — a green gate for something that will not be built. **MUST NOT be held back for the first version of the checker.**

## Row format
`ID . judul . status . pemilik . sejak . R: <FR-### | tanpa-spec:<alasan>> . diblokir oleh`

**Evidence correction, 9 Nov 2026 (Lead's technical gate, after the commit):** the evidence above once named commit **`5a8b3f0f`**, which **does not exist in the repo** (`git cat-file -t 5a8b3f0f` -> `fatal: Not a valid object name`). The correct one is **`4ca56796`**. Three other evidence defects were found and fixed in this correction commit, all in `tasks.md`:
- **T074, T075** were ticked **with no hash at all** -> `vizier@5eba4226` added.
- **Six `Landed` annotations** named a hash **without a repo** (`fbb204f` x3, `0c9c89c`, `3bf44a7`, `cc1d648`). All four **exist only in the `webui` repo**; checked from the vizier root they come out "missing" even though they are valid. All were given the `webui@` prefix.
- All `vizier` annotations now carry the `vizier@` prefix so that an honest `grep` equals an easy `grep`.

Re-check result: **65 `[x]`, 65 annotated, 22 unique `repo@hash` references, 22/22 resolve** (`git cat-file -t`). No tests were run; the `In the tree` marker still means "code present + commit present", not a passed gate.

**Reaffirmation of the technical signature (Lead, 9 Nov 2026):** I **wrote this signature line myself**, in commit `557739fe` — not another party, and this correction replaces the earlier note claiming "written by another party" (that was wrong, and I have withdrawn it). The Lead has already re-verified it himself — 110 task rows intact, 65 `[x]`, 0 references to `screens/livevoice/`, 22/22 `repo@hash` resolve — and reaffirms that the signature **is valid from commit `c953f4e9`**, not from when it was written. Evidence chain: `aa27590e` (reconciliation) -> `4ca56796` (paths) -> `c953f4e9` (evidence given a repo, phantom hash corrected).

## Process incident — AQ-001 (9 Nov 2026)

Recorded separately from task status, at `@bro-pm`'s request:

| Fact | Evidence |
|---|---|
| The row was opened `aktif` **without a written DoD** on the row | `11db2834` — violating the board's own rule ("no DoD = not yet allowed to be `aktif`") |
| Reconciliation, technical signature, and `siap-kirim` status were written **by the same person** (Lead), with no DoD first | `aa27590e`, `557739fe` |
| The board row named commit **`5a8b3f0f`**, which does not exist — the hash was invented before its commit was made | `git cat-file -t 5a8b3f0f` -> `fatal: Not a valid object name`; the correct one is `4ca56796` |
| "Who held the pen" **cannot be answered from the repo** | all seven 9 Nov commits have an identical git author (`Robi Naufal Kaosar`); there is no agent marker |

**The rule hole this opens:** a row owned by the Lead has no second reviewer — the Lead is both the executor *and* the owner of the technical signature.

> **Rule: second reviewer for `docs` rows owned by the Lead — RATIFIED by kang muruyung, 10 Nov 2026 09:37** (*"Ok setuju"*, item 3). Status: **trial**, not permanent.

Four fences, and breaking them voids that review:

1. **Read only, no tests.** The reviewer reads the diff and the written evidence; he runs no commands, touches no code, adds no tests.
2. **Only rows labelled `docs` whose owner is the Lead.** Code rows, or rows owned by another agent, are out of scope here.
3. **May only hold back if the evidence does not support the claim** — not because of taste, not because of format, not because "I think it is tidier". Holding back MUST name which evidence does not match.
4. **Trial:** if in practice this rule holds back tasks without a machine-checkable reason, it is revoked at `@bro-pm`/kang, not quietly loosened.

A reasonable reviewer: `@teteh-qa` (she already reads test diffs as part of her gate). For AQ-001 this rule is **not retroactive** — kang has already accepted that row, so nothing needs reopening.

## Board

| ID | Title | Status | Owner | Since | R: | Blocked by |
|---|---|---|---|---|---|---|
| `AQ-001` | Reconcile the checkboxes in `specs/055-live-voice/tasks.md` with git history (label `docs`) | **`selesai`** | Lead | 9 Nov 2026 20:45 WIB | `tanpa-spec:docs` — allowlist `specs/**` | - · accepted by kang **10 Nov 2026 09:36** (*"Ok terima"*) |
| `T110` | A revoked/expired key ends the live session it opened (FR-063, FR-063a) | **`siap-kirim`** | `@sepuh-be` | 9 Nov 2026 21:32 WIB | `FR-063` | three signatures complete 10 Nov 11:51 — technical (me, `30500bf5` + `093c2b92`), behaviour (`@teteh-qa` **PASS WITH KNOWN ISSUES**), visual not requested (not `ui`). **`FR-063` is not closed yet**: `GI-01` (1 second not measurable), `GI-02` (revoke paths via composition + route with no tests), `GI-05` (a revoked key can still `start` on an open socket) |
| `GI-01` | SC-013: the session ends **≤1 second** after the key is revoked — there is no tool yet that measures the real gap from the HTTP revoke until the frame reaches the client | `antre` | `@sepuh-be` | 10 Nov 2026 09:20 WIB | `FR-063` | needs a timing test with a live server; not a unit test |
| `GI-02` | **The revoke path has no test that touches either the socket or the route.** Two holes: (a) `revoking_a_key_ends_its_call_and_leaves_the_others_alone` (`session.rs:970`) only asserts counts/state — **zero** asserts of `Command("abort")`; (b) `grep -rn 'delete_api_key' crates/vizier-http/tests crates/vizier-http/src/api` → **0 test hits**, so `api_keys.rs:110-121` (delete → `end_owned_by_key`) is never executed by any test | `antre` | `@sepuh-be` | 10 Nov 2026 09:20 WIB | `FR-063` | one test in the in-crate harness round 2: call `delete` via service/route for a key that has a running session → session `Ended` + `key_revoked` + exactly one `command:abort`. The old description ("not a single test asserts") is corrected: that test now **exists**, for the expiry path |
| `GI-03` | **Expiry is enforced on the next frame**, not at the second itself (`topic.rs:410-415`). A **silent** session lives on past its key's validity and nothing ends it — not a "mislabel as `idle`", because idle-stop (`T062`) does not exist yet | `antre` | `@sepuh-be` | 10 Nov 2026 09:40 WIB | **`FR-063a`** | **decided 10 Nov 09:48:** this behaviour is now **promised** by FR-063a (`spec.md` @ `28ed1668`), not a defect. What remains is only the interaction with `T062` (ordering dependency), not a sweeper timer |
| `GI-04` | qa-plan scenario 4 (the `key_revoked` reason reaching the screen) passes **by composition**, not by one assert: the sentence is tested in `session.test.ts:210-213`, the terminal mechanism is tested with the reason `idle` at `:702` | `antre` | `@sepuh-be` | 10 Nov 2026 09:40 WIB | `FR-063` | one client test that sends `key_revoked` and reads the sentence in the row |
| `GI-05` | FR-064: **revocation is not checked per frame.** `key_lapsed` (`topic.rs:67-74`) only reads `principal.method`, which is stamped once when the connection is opened (`topic.rs:243`); revocation is only pushed by `delete_api_key`. A socket still open can send `live_voice_start` again with a key that has already been revoked | **`aktif`** | `@sepuh-be` | 16 Nov 2026 17:05 WIB | `FR-064` | brief + DoD in **§Brief `GI-05`** below; base `vizier@b2d0504e`; red expected as **an assert on the session path**, not a compile failure **Status change recorded, per `@bro-pm` (17:29):** this row moved `antre -> aktif` on **16 Nov 2026 17:05**, which *replaces* the 15:06 decision that it would not start that day. The reason the 15:06 decision existed was "T113 is not committed yet and WIP=1 is taken"; both no longer hold (T113 and T114 are closed, the slot is empty), so `aktif` is correct **now** and would be wrong if read as the 15:06 ruling. Written down because the board has changed without a trace three times today. |
| `GI-06` | `lsp_tests` 8 failures are **not** because the fixture has not been built: `fake_lsp()` computes the path from `current_exe()`, and on this machine the lib test binary lives in `target/debug/build/vizier-core/<hash>/out/`, so it looks for `target/debug/build/vizier-core/examples/fake_lsp`, which does not exist | `antre` | `@sepuh-be` | 10 Nov 2026 11:05 WIB | `FR-041` | proven: the `fake_lsp` example **is** built in `target/debug/examples/` and the 8 tests still fail (`the server never became ready`, 20 s); the fix = a layout-proof fixture path, not `cargo build --example` |
| `GI-07` | **The expiry path on the socket has no test that goes through the socket.** | **`selesai`** | `@sepuh-be` | 10 Nov 2026 11:20 WIB | `FR-063` | closed by `093c2b92` (round 2 of T110): two in-crate tests, 6 passed, and their failing proven by mutation — see §Gate technical — T110 round 2 |
| `GI-08` | **`live_voice::session::filler_tests::a_dropped_filler_never_costs_the_reply` is intermittent — two logged observations, low rate, and cell **A3** in `specs/055-live-voice/release-checklist.md` that promises `56 passed, 0 failed` as if absolute.** **The observer is `@teteh-qa`, and that can be checked in the topic record of `vizier-dev`/`fitur-live-voice-rev1`:** **seq 2768 (16 Nov 15:59:55)** — `run 1 : test result: FAILED. 55 passed; 1 failed`, `panicked at crates/vizier-core/src/live_voice/session.rs:2007`, `"and it is the reply, not the filler"`, runs 2-11 green; **seq 2810 (16:34)** — the same report with `EXIT=101` plus *"8 consecutive green runs afterwards, and 3 runs of that test alone"*. `@bro-pm` only **forwarded** it (seq 2769/2774), he is not its source. **Accurate status:** *reported with a specific error message, not reproduced afterwards.* **One line of history that MUST be read before quoting:** an earlier version of this row read *"its attribution was rebutted by the observer"* — **that was wrong**; what `@teteh-qa` rebutted was the **hash label** (`df7a55ca`, which was only born at 16:20 after its run at 15:54-15:58), **not the occurrence**, and she counted the rate herself (1 failure out of 32 runs). That version is withdrawn; nobody should read it as "QA once invented a finding", because the record itself comes from QA. The Lead tried to reproduce it at `vizier@39d16fed`, `CARGO_TARGET_DIR=/home/muruyung/vz-gate-target`: **0 failures out of 21 runs** (1 warm-up block of 56 + **10** runs of that test alone + **10** full block-of-56 runs) — so the rate is low, it does **not** mean the race is absent. Not a T113/C17 effect: `session.rs` was last touched by `485e1a91` (10 Nov), before the C17 gate was revoked. The hypothesis, not yet proven: a race on the real clock, not `start_paused` — the test sleeps **500 ms** (`session.rs:1998`) to empty the lane while the filler fires at **400 ms** (`:1997`), so a scheduler held up by load can drain the lane **before** the filler tries -> the filler slips through and `all(\|len\| *len > 8)` (`:2007`) goes red. No load average was recorded in any log that day, so "the machine was loaded" is not evidence this row holds. The fix makes the test deterministic (`start_paused` + `advance`, like `idle_tests`), not loosening the margin. **Extra measurement 16:37-16:38 WIB (Lead, `f9da945b`, idle machine, `CARGO_TARGET_DIR=/home/muruyung/vz-gate-target`):** **10 consecutive blocks from row A3, `0 failed`** — and all ten finished in an identical **2.81 s** (`1966 filtered out`), a sign the machine really was not loaded. The Lead's total is now **31 runs / 0 failures**; the combined total with `@teteh-qa` is **1 failure in 42 runs (~2,4 %)**. What matters is not the rate but the fact that **the same binary went red and then green within three seconds** (`@teteh-qa`, 17:25): `qa-t113-core.log` 15:30:56 green at 2.81 s, `qa-a3.log` 15:54:15 red at 3.14 s, `qa-a3b.log` 15:54:18 green at 2.81 s — and the red log opens with `Finished `test` profile ... in 0.47s`, i.e. **no rebuild**. Load cannot change much in three seconds, so **load is not a necessary condition** for this failure; at most it raises the rate. **One caveat that keeps the comparison honest:** the red and the green run are **not the same command** - the red one is `--lib live_voice::` (`running 56`, `1966 filtered`) and the green three seconds later is `--lib live_voice` without `::` (`running 62`, `1960 filtered`), so the set of tests running in parallel differs. That is enough to rule out **external load** (it cannot change in 3 s) but it does **not** rule out **concurrency/ordering** inside the block, which is exactly why `--test-threads` matters and why the next measurement MUST name the filter and the thread count. A controlled rerun should hold the filter fixed and vary only `--test-threads`. That kills the "correlation with load" reading that an earlier version of this row carried. **THE RAW LOG HAS BEEN FOUND (17:0x, by `@sepuh-be`) — so the hold on this row is satisfied:** `/tmp/qa-a3.log`, mtime **15:54:15**, target dir **`/tmp/vz-qa-t113`** (owned by `@teteh-qa`), containing `test live_voice::session::filler_tests::a_dropped_filler_never_costs_the_reply ... FAILED` + `panicked at crates/vizier-core/src/live_voice/session.rs:2007:9: and it is the reply, not the filler` + `test result: FAILED. 55 passed; 1 failed; 1966 filtered out; finished in 3.14s`. **Three runs, one binary, same `crates/`:** 15:30:56 green `56/0` 2.81 s · 15:54:15 **red `55/1`** 3.14 s · 15:54:18 green `62/0` 2.81 s (filter `live_voice`, which includes the red test). **Two things that log changes from the guesses on this row:** (a) the failing run was **not slow** — `3.14 s`, while the same 56-test block in the Lead's green run took `2.81 s`; so **loadavg 11-12 is not recorded in this log** and the load correlation remains a **guess**, not a measurement. If load played a part, the candidate is the load of **the test block itself** (56 parallel tests, `--test-threads` **not recorded** because it was never mentioned in the command). (b) `:2007` **does not print `heard`** (what prints `{heard:?}` is `:2006`), so this log proves the **symptom**, not the **ordering**: `filler slipped into the lane` and `reply discarded because the lane is still full` both end at the same `:2007`. **Consequence for the brief:** the first fix that must go in is not `start_paused`, but **`{heard:?}` in the `:2007` assert** — without it the next red run is still mute and this debate repeats. Only after that, the deterministic test; `--test-threads` MUST be named in the report.  **The three-part evidence — what EXISTS and what will NEVER exist:** (a) **path + time + content: EXISTS** — `/tmp/qa-a3.log`, mtime **15:54:15.760**, 6164 B, target dir `/tmp/vz-qa-t113`, `running 56 tests` (the `--lib live_voice::` filter, not a single test), panic on lines 77-79, line 85 `FAILED. 55 passed; 1 failed; 1966 filtered out; finished in 3.14s`. All `/tmp/*.log` were swept: **only this file** contains `FAILED` for that test; `d1`/`d3`/`d3b` (16:06) all `0 failed`. (b) **the binary hash of the failing run: WILL NEVER EXIST** — the file at that path is now `md5 f84a8d98…`, **mtime 16:37:07**, rebuilt after the 15:54 run (Lead verified with `ls -l` + `md5sum`). The failing bytes are gone from disk; only the path and the log remain. (c) **`--test-threads`: not recorded** — `cargo` does not print it and the command never named it (`grep -c test-threads` = 0). Do not fill it with a guess. **Because (b) and (c) are incomplete, this row's own requirement of 'raw log + hash + threads' CANNOT be fully met — and that must be written down, not covered up.** What remains usable is still enough to work with, and two pieces are stronger than we assumed: (1) **`:2006` did NOT fire** — `grep -c 'the reply still reaches the person' /tmp/qa-a3.log` = **0**. That is the only assert that prints `{heard:?}`, and it stayed silent, so `heard` is **not empty**, and what went red at `:2007` is **a single frame with `len <= 8`**. (2) **Two different mechanisms, separated by one number:** that frame is either `len == 4` (the test's own frame, `send_speech(&[0u8; 4])` at `:1981`, which `rx.try_recv()` at `:2002` did not take) **or** `len` equal to the length of the filler word (the filler slipped into the lane). **These are two different causes, not one** — and `{heard:?}` at `:2007` separates them with a single number, no reproduction needed. That makes adding the payload **not cosmetic** but the only way to pick the mechanism from the next red run. (3) **The line seal still holds:** `session.rs` at HEAD (`f0d19bef`) is md5 **`745853dd…`** = working tree, and `:2007` at HEAD is exactly the line in the panic. The fix can be written without re-mapping line numbers.  | `antre` | `@sepuh-be` | 16 Nov 2026 16:10 WIB | `FR-042` (filler = the voice side of the "thinking" state; there is no FR specific to the filler — recorded as is) | **deterministic reproduction**, not a log: the error message and the test name already exist (two reports above). What is missing: one run that can be forced to fail before the fix. Without that, every "fix" cannot be distinguished from loosening the margin |
| `GI-09` | **`crates/vizier-http/tests/topic.rs` has no TOTAL time budget — the test can hold the `cargo` lock ~8 minutes without failing, so "the process is hanging" and "the machine is saturated" cannot be told apart from outside.** Figures: `test result: ok. 1 passed … finished in **499.09 s**` in `@sepuh-be`'s 16:29 run at loadavg 8-12 with two or three `cargo` at once; the same test on an idle machine: **12,03 s / 12,21 s / 13,21 s / 14,73 s** (~40x). Not a deadlock: it is all in the same test, not a build script. The cause is in the test itself — it spawns `node dummyplug/src/server.mjs` then waits with a **bounded** loop (`0..600` x 100 ms for frames, `0..50` x 100 ms for connect, a 60 s deadline per phase), and there is no bound that covers **node startup** plus all the phases at once; each iteration calls `TcpListener::bind`/`connect_async` to loopback, so a slow node burns the per-phase budget, many times over. Process consequence: the `-p vizier-http` gate that normally takes ~2 minutes can become 8+ minutes, and the next measurer concludes "hang" when it was only saturation — exactly what happened in the 16:29 round. The fix is small and **not** a loosening: one `tokio::time::timeout(Duration::from_secs(120), …)` wrapping the test body (or `--test-threads` capped), so that it fails with a message instead of waiting. | `antre` | `@sepuh-be` | 16 Nov 2026 16:58 WIB | `FR-063` (this test is the socket path for key expiry, `093c2b92`) | not a test written by the holder of this slot, and not the T113/T114 round area — **MUST NOT be worked on without a brief**; there is no log yet showing a failure message, consistent with the nature of the problem (it never fails, it is only slow) |
| `GI-10` | **The chain "pill reason reaches the screen" has neither a render nor an integration test — the proof is grep.** `webui/app/routes/` still has **zero `*.test.tsx`**, so what proves `liveVoiceMissingFrom` is actually used is three grep hits in `chat.tsx` (`:14` import, `:127` use, `:550` prop) plus a pure function with 6 tests (`voice.test.ts`) — **wiring, not rendering**. Concrete consequence: if `chat.tsx` stops passing that prop, or `Composer.tsx:487` changes `missing={liveVoiceMissing ?? null}` to `null`, **no test goes red**; all that changes is a pill that quietly goes generic again. Finding by `@teteh-qa` at the AQ-003 gate (16 Nov 16:50), number assigned by the Lead. | `antre` | `@sepuh-be` | 16 Nov 2026 16:55 WIB | `FR-004` | **Form decided by the Lead, 16 Nov 17:47 — option (A), and (B) is rejected with reasons read from the repo:** a render test in `app/routes/` would be a **false green**, because `vitest.config.ts` sets `environment: 'node'` and 21 of 21 `.test.tsx` files use `renderToString`, which **does not run `useEffect`** — and the whole chain lives inside one (`chat.tsx:115`); `renderToString` would paint `voice === undefined`, prop `null`, pill absent, **test green even with `:127` cut out**. Option (B) (adding `jsdom` + `@testing-library/react`) contradicts a decision already written into the repo (`app/lib/liveVoice/control.test.ts:129`: *"no jsdom, no happy-dom, no @testing-library/react, so a component cannot be mounted"*) and would need a User decision on a new dependency. **Option (A): move the body of the effect into a pure `voiceFromDetail(detail)` in `app/lib/liveVoice/voice.ts` — the same shape as `liveVoiceMissingFrom` that passed T114 — and test it in the `node` env: both halves, one half, absent field, and `catch` -> `null`. Red-first via a mutant that passes `detail.live_voice_missing` through raw.** The residual gap is stated, not papered over: **one line (`chat.tsx:550`)** stays untested; prove it by mutation (delete `:550`, whole suite still green = the gap is documented) rather than by a test that cannot fail. |
| `GI-11` | **A second surface without the pill: `WorkspaceComposer.tsx` does not pass live voice at all.** `git grep -c 'liveVoice\|live_voice' -- webui/app/screens/workspaces/WorkspaceComposer.tsx` = **0**, even though it renders the same `<Composer>` (`:96`). So an agent used from the workspace screen has no pill, no "which half is missing" reason, and cannot start a live voice session from there. **Not yet scheduled** — not a defect found by a test, but a surface that has never entered 055's scope until now. Finding by `@teteh-qa` (her turn at the AQ-003 gate), number assigned by the Lead. | `antre` | not yet assigned | 16 Nov 2026 16:55 WIB | `FR-004` (the second surface of the same requirement) | **product decision**: whether the workspace really must not have live voice, or it simply has not been done. Without that answer, this is not a task |

**Numbering note (10 Nov 09:15):** the AQ-001 evidence below applies to **110 tasks** and is frozen at its commits. After that `@system-analyst` added **T111 [PROVISIONAL]** (`a53bfcb5`, refined by `ef846fbd`), so today's count is **111 tasks: 65 `[x]` / 46 `[ ]`** — not 110/45. That addition was accepted by the Lead: its content is provisional, awaiting my assessment + Q12 from kang, and it is **not** part of this release gate.


**Figure note (10 Nov 11:55):** `tasks.md` now has **111 tasks, 65 `[x]` / 46 `[ ]`** — the **same** number as the 09:15 note, but with a **different composition**: `T026` has not been re-ticked (`77117f9d`) and `T110` was only ticked today. Do not quote 65/46 without naming its composition; two different states can produce identical numbers.

## Signature — AQ-001

| Signature | Owner | Status | Evidence |
|---|---|---|---|
| Technical | Lead | ✅ 9 Nov 2026 | below |
| Behaviour vs spec | `@teteh-qa` | `qa-plan: N/A` — **accepted by QA 21:23, without a signature** | label `docs`, does not touch code (tiered qa-plan: only behaviour/API tasks MUST have 3-5 scenarios). QA may rebut with evidence; if rebutted, the status returns to `gate` |
| Visual vs handoff | `@teteh-desener` | not requested | label `docs`, not `ui` (the board's own rule) |

**Signature note:** there is no signature I wrote on behalf of another agent. The technical signature on this row is mine after I checked it myself: 110 tasks intact, `[x]` 9 -> 65, `[ ]` 101 -> 45, `screens/livevoice/` references 6 -> 0, and 22 of 22 `repo@hash` annotations resolve in the repos named (`git cat-file -t`; the only one that failed is the `<hash>` placeholder at the head of `tasks.md`).


**Technical evidence** (command → output, run 9 Nov 2026 before the commit):
- `grep -cE '^- \[[ x]\] T[0-9]{3}' specs/055-live-voice/tasks.md` → `110` before **and** after: no task was added or removed, only statuses changed.
- `grep -c '^- \[ \] T' …` → `101` → **`45`**; `grep -c '^- \[x\] T' …` → `9` → **`65`**.
- `grep -c 'app/screens/livevoice/' specs/055-live-voice/tasks.md` → `6` → **`0`** (the line is in `webui/app/screens/chat/LiveVoiceRow.tsx`, not `screens/livevoice/`).
- Commit: **`aa27590e`** (reconciliation, `specs/055-live-voice/tasks.md` only) + **`4ca56796`** (path correction `screens/livevoice/` -> `screens/chat/` in `plan.md`/`architecture.md`/`tasks.md`, plus T037/T040/T089).

**Limit that MUST be read before quoting the number 65**: the 56 new ticks only prove **the code is in HEAD** (`git grep … HEAD -- <file>`) and name the commit that introduced it. **No test was run** in this reconciliation. That is why two markers are distinguished at the head of `tasks.md`: `Landed … gate passed` (gate run) vs `In the tree … not gated in AQ-001` (presence only). The gate remains QA's signature.

- **Quoting rule (requested by `@teteh-qa` 21:23):** the number **65** MUST NOT be quoted without the limit sentence above. Without it, 65 circulates as "already tested" in the next brief, and QA is asked to answer for it at the gate.
- **Breakdown of the 22 `repo@hash` annotations** (re-measured 21:35 with `grep -oE` + `sort -u`): **vizier 11 · webui 10 · design 1**. The "vizier 13" mention in the 21:16 conversation was a miscount; the total remains 22 and still 22/22 resolve.
**The 45 deliberately still `[ ]`**: VAD fixture (T004), baseline (T006), FR-042 e2e indicator (T039), the archive button that is no longer the entry point (T037), per-agent switch (T041), backlog counter (T042), ducking & short burst (T047, T048), barge-in/interrupt e2e (T049, T056), the entire interrupt backlog (T051–T057), second connection cycle & rejection (T061–T065), server failure test (T066), superseded (T093), registry & lagging reasons (T095–T097), server instrumentation (T077), and the entire release gates (T078–T088) plus the third-party key phase (T100–T110).



## DoD — T110 round 1 (10 Nov 2026 09:40): the expiry path MUST cancel the turn

**The defect, from the code not from a report:** two paths lead to `KeyExpired`, only one cancels the turn.
- **Revoke** — `delete_api_key` → `end_owned_by_key` (`session.rs:651-675`): `abort_turn(BargeIn)` → `end(reason)` → `remove`. The turn stops.
- **Expiry** — `topic.rs:417-419`: `session.end(EndReason::KeyExpired)` → `sessions.remove(...)`. **Without `abort_turn`.** `end()` only cancels the user utterance (`session.rs:219-221`), it does not send `Command("abort")`; that is `abort_turn`'s job (`session.rs:246-262`).

The consequence is exactly what `session.rs:236-238` warns about: the agent keeps working, its answer is still written and still enters the history, even though the session is already `ended`. FR-063 demands three: the reason, the mic released, **the turn cancelled** — the expiry path meets two.

**The fix requested:** one place. The expiry path in `topic.rs` uses the same sequence as `end_owned_by_key` — `abort_turn` first, then `end`, then `remove`. No new API, no new type. If `topic.rs` has no access to the async `abort_turn` at that point, move the sequence into one registry method (`end_one(&session, reason)`) so both paths use one function — do **NOT** copy those three lines.

**DoD (commands + output that must be visible):**

| # | Command | Expected output |
|---|---|---|
| 1 | `cargo test -p vizier-core --features provider-live-voice live_voice::session::key_tests` | `running ≥5 tests`, all pass — the revoke path is not broken |
| 2 | A **new** test in the same module: a session with a running turn, then reason `KeyExpired` | asserts **three things**: `ended` is announced with `key_expired`; `busy` is `false` again; **`Command("abort")` is actually sent** to the transport (captured through the test transport — the mechanism already exists in `session_on_transport`) |
| 3 | `cargo test -p vizier-core --features provider-live-voice` (featured crate) | `0 failed`; state the number of `passed` as it is |
| 4 | `cd webui && npm test -- session` | unchanged: 65 passed / 1 skipped |

**Limit:** test #2 MUST fail on the current code and pass afterwards — if it passes before the fix, the test is in the wrong place, not the code is right. Attach **both** of those outputs in the report.

> **Note 2026-11-16 (C17):** two commands in this table use `--features provider-live-voice`, which **has been removed** — running them now fails hard (`does not contain this feature`), not passes. The row above is **not changed** because it is a record of the gate result run that day. Today's equivalent: `cargo test -p vizier-core --lib live_voice::` (56 tests) and `cargo test -p vizier-http` (32 tests in four binaries) — without `--features`.

**WIP:** `T110` is back to `aktif` (round 1 of max 2). This is the same round, not a new task — it does not add a slot.


## Round-1 code in the working tree — author unknown (10 Nov 2026 11:05)

`git status` at 11:00 shows **uncommitted** changes whose content is exactly DoD round 1, in files owned by T110:

| File | mtime | Content |
|---|---|---|
| `crates/vizier-core/src/live_voice/session.rs` | 10 Nov 09:53 | new method `end_one()` (abort → end → remove) + test `a_lapsed_key_cancels_the_turn_it_was_running` |
| `crates/vizier-http/src/topic.rs` | 10 Nov 09:54 | the expiry path calls `sessions.end_one(..., KeyExpired)` — the three old lines removed |

**What I have already proven (11:02):** `cargo test -p vizier-core --features provider-live-voice live_voice::session::key_tests` → `running 6 tests`, **6 passed / 0 failed**, exit 0 — including the new test that asserts `busy` is `false` again and `Command("abort")` is actually sent through the test transport.

**What is still open:** who the author is. Git cannot distinguish agents (identical author), and I do **not** state "not me" without evidence. The current weight of evidence: (a) the 09:53 timestamp is only **6 minutes** after my commit `28ed1668` (09:47); (b) `@sepuh-be`'s alibi (10:43) says his session was cut off **21:45 last night** and does not mention the 09:40–10:00 window — so his claim does not yet cover that window. I asked him one closed question before concluding.

**Gate limit:** the new test uses an API that does not yet exist in HEAD, so "must fail before the fix" is satisfied at compile time, **not** as behavioural evidence. The real behavioural evidence is in `topic.rs`: the expiry path no longer calls `end()` itself. That is what the reviewer must read, not its exit code.


## Technical gate — T110 round 1 (10 Nov 2026 11:15–11:20)

Code: **`30500bf5`** — `crates/vizier-core/src/live_voice/session.rs` + `crates/vizier-http/src/topic.rs`, 2 files, +140/−9. `git status --short crates/` is **empty** after the commit *and* after the gate, so the gate was run on the committed tree, not the working copy.

| # | Command | Output |
|---|---|---|
| 1 | `cargo test -p vizier-core --features provider-live-voice live_voice::session::key_tests` | `running 6 tests` · **6 passed / 0 failed**, exit 0 — including `a_lapsed_key_cancels_the_turn_it_was_running` |
| 2 | `cargo check -p vizier-core` | `Finished`, exit 0 — the build without the feature does not see this code (FR-041) |
| 3 | `cargo check -p vizier-core --features provider-live-voice` | `Finished`, exit 0 |
| 4 | `cargo test -p vizier-http --features live-voice --test live_voice_session` | `running 7 tests` · 7 passed / 0 failed, exit 0 — the existing socket path is not broken |

**Evidence limit, in `@sepuh-be`'s own words:** the new test is **red because of compilation, not because of an assertion** — `end_one` is not yet in HEAD, so "test fails before the fix" here means "cannot compile before the fix". If that sentence is quoted without its limit, it becomes another form of false green. The behavioural evidence is in `topic.rs`: the three lines `end()` + `remove()` are gone, replaced by one `end_one` call.

**Attribution:** the author of the code cannot be determined from git (identical author across agents). What is proven: the content matches the DoD I wrote at 09:40, mtime 09:53 = 6 minutes after my commit `28ed1668` (09:47), and `@sepuh-be` states he had no session in that window. I gated and committed on that basis, and record it as a **process defect** — the code should have been written by `@sepuh-be`, not the Lead.

**What has not been proven:** SC-013 "within 1 s" (needs a timing test with a live server) and **`GI-07`** (no test passes through the socket). Both MUST be included in the gate report, not tidied away.


## DoD — T110 round 2 (GI-07): the socket path tested through the socket, not through the registry

**Why round 2 and not a new row:** `GI-07` was born from T110 and QA MUST NOT sign T110 while its hole is open. One slot (`WIP=1`), one `R: FR-063`. `GI-07` stays on the board as a trace, its status **merged into this round**.

**The A/B options proposed by `@sepuh-be` do not need to be chosen — there is an option C, and its cost is zero production changes.** The evidence, all three I checked myself:

| Fact | Evidence |
|---|---|
| `HTTPState` cannot be named from `tests/` | `crates/vizier-http/src/lib.rs:26` → `mod state;` (private to the crate) |
| But it **can** be named from a test inside the crate | `crates/vizier-http/src/api/v1/catalogue.rs:249-…` builds `HTTPState { … }` as is; `new_state()` is also `pub(crate)` (`lib.rs:63`) |
| In-crate async tests are already the habit in this crate | `#[tokio::test]` at `api/v1/catalogue.rs:282`, `notifications.rs:393`, `health.rs` |
| `Principal` + `AuthMethod::ApiKey { expires_at }` can be built by a test | `crates/vizier-core/src/auth/mod.rs:52-57`, `:15-32`; hand example: `topic.rs:93-99` |
| A session can be injected into the process registry | `vizier_core::live_voice::sessions()` (`live_voice/mod.rs:45`), `set_key_origin` (`session.rs:157`), `run_turn` (`:512`) |

**My decision: the test goes in `crates/vizier-http/src/topic.rs`, `#[cfg(test)] mod tests`** — an existing module, not a new file. No production behaviour change. **If it turns out impossible without touching production code (visibility/refactor): stop and report, do NOT change visibility yourself** — that is a gate decision, not a work step.

**What must be proven** — two new tests, these exact names:

1. `a_lapsed_key_at_the_socket_cancels_the_turn_it_was_running`
   - `TopicConnection` with principal `AuthMethod::ApiKey { key_id: Some(..), expires_at: Some(masa lalu), .. }`.
   - A session in the registry for the same conversation, `key_origin` = that key, **turn running** (`run_turn` with a silent engine, the `SilentVoice` pattern in `session.rs` round 1).
   - One frame comes in through `handle_live_voice` (e.g. `live_voice_audio`).
   - Assert **five**: `state() == Ended`; the published reason `key_expired`; `busy == false`; `command:abort` sent **exactly 1**; the registry no longer contains that session.
2. `a_session_a_person_opened_survives_the_same_call`
   - The same session but `key_origin == None`, the same principal (expired key).
   - Assert: the session **stays** alive and there is no `abort` — this is what distinguishes "all sessions trimmed" from "the session owned by that key".


**Ordering note that MUST be read before writing the test** (I checked myself, so there are no surprises midway):
- The expiry branch sits **before** `match action` and does **not** `return` (`topic.rs:415-425`). After `end_one`, the same frame still enters its arm: `ClientAction::Audio` immediately `return`s because `sessions.get(...)` is already empty (`:553-556`). So there is no stray frame — but the assertions MUST be on **session state + abort + registry**, not on "no other frame".
- The negative test MUST NOT demand that the audio frame be processed successfully: only **the session does not stop** and **there is no `abort`** are asserted.

**Commands + expected output:**

| # | Command | Expected output |
|---|---|---|
| 1 | `cargo test -p vizier-http --features live-voice --lib topic::tests` | `running 6 tests` · **6 passed / 0 failed** · the names of both new tests appear in the list. Today's baseline is **4 passed** — the number 6 is mandatory, not just "ok" |
| 2 | `cargo test -p vizier-core --features provider-live-voice live_voice::session::key_tests` | `running 6 tests` · 6 passed (round 1 not broken) |
| 3 | `cargo check -p vizier-http --features live-voice` | `Finished`, exit 0 — no production code changed |
| 4 | `git status --short crates/` | **empty** after the commit, before and after the gate is run |

**Limits that MUST be included in the report:** this test proves the socket path for **expiry**. It does **not** prove `SC-013` "within 1 s" (needs a timing test with a live server, `GI-01`), and does **not** prove `GI-05` (revocation is not checked per frame) — but the same harness makes `GI-05` cheap after this: one extra test "revoke the key → `live_voice_start` again on the same socket". If you can close both in one file, say so in the report; if not, `GI-05` stays open and that does not block this round.

**WIP:** `T110` remains your only slot. This round is the 2nd of a maximum of 2 — if there is still a blocker after this, a 3rd round is no longer a round, but an escalation to `@bro-pm`.


## Technical gate — T110 round 2 (10 Nov 2026 11:48)

Code: **`093c2b92`** — `crates/vizier-http/src/topic.rs` only, +307/−1. The only deleted line is a `use` inside `mod tests`; **zero production code changes** (two hunks: `@@ -85` `use` expansion, `@@ -234` test contents).

**Re-measured by me, not accepted from a report:**

| # | Command | Output |
|---|---|---|
| 1 | `cargo test -p vizier-http --features live-voice --lib topic::tests` | `running 6 tests` · **6 passed / 0 failed** · both new names appear (`a_lapsed_key_at_the_socket_cancels_the_turn_it_was_running`, `a_session_a_person_opened_survives_the_same_call`) |
| 2 | `cargo test -p vizier-core --features provider-live-voice live_voice::session::key_tests` | `running 6 tests` · 6 passed — round 1 intact |
| 3 | `cargo check -p vizier-http --features live-voice` | `Finished`, exit 0 |
| 4 | `git status --short crates/` | empty, before and after the gate |

**"Fails before the fix" I reproduced myself, not quoted.** I temporarily reverted the expiry branch to its pre-`30500bf5` form (`session.end()` + `sessions.remove()`, without `end_one`), then ran the test:

```
test topic::tests::a_lapsed_key_at_the_socket_cancels_the_turn_it_was_running ... FAILED
panicked at crates/vizier-http/src/topic.rs:491:
assertion `left == right` failed: the running turn is cancelled by exactly one abort, got []
  left: 0   right: 1
```

It failed on a **behavioural assertion**, not compilation — exactly what `GI-07` asks for. The file was restored with `git checkout --`, md5 **identical** (`5db9cd91…`) and `git status` clean afterwards.

**One assert is weaker than the DoD, and I decide: accepted.** The DoD asks for the literal `busy == false`; `busy` is private in `vizier-core` (`session.rs:71`). What is used instead: `run_turn` publishes `Waiting` before the frame (asserted), and `abort_turn` **only** sends `Command("abort")` if the flag is set and then clears it in the same swap (`session.rs:250-251`) — so "exactly one abort" proves the turn was running and now is not. I accept it because behavioural evidence is stronger than reading a flag.

**Correction for the record:** the choice is **not** binary "a production accessor or nothing at all". There is a third way without touching production — call `abort_turn` once more after the frame (public API, `session.rs:246`) and assert there is **no** second abort; that proves `busy == false` literally. The price is one assert. Recorded as **minor, non-blocking**; if `GI-05` touches the same harness, add it there.


## QA verdict T110 round 2 + my corrections (10 Nov 2026 11:51–11:55)

`@teteh-qa`: **PASS WITH KNOWN ISSUES**, one message, mention Lead + PM (gate rule obeyed). Technical already ✅, visual not requested (not `ui`) → **`T110` = `siap-kirim`**; `selesai` only after kang accepts.

**Two QA corrections that I verified myself and accept:**

1. **"Three of the four qa-plan scenarios closed" is wrong, it is two.** `revoking_a_key_ends_its_call_and_leaves_the_others_alone` (`session.rs:970`) asserts `ended == 1`, the state of the three sessions, and the registry contents — **zero** asserts of `Command("abort")`. The test that asserts abort is the **expiry** path. So scenario 1 (revoke) passes **by composition**. I was the one who said it too quickly, and QA caught it with `grep`, not with taste.
2. **The `webui` pin in the gate commit is `8929bbed`, not `7b7c9a3`.** `git ls-tree 98a99a1a webui` → `8929bbed…`; `7b7c9a3` is its **ancestor** (`git merge-base --is-ancestor` → true). Its contents are in, but the pin is something else. I cited a range, not a commit — exactly the habit we ban.

**New hole from QA, I verified it and folded it in:** `grep -rn 'delete_api_key' crates/vizier-http/tests crates/vizier-http/src/api` → **0 hits**. The revoke route is never executed by a test; what is tested is only the registry underneath it (`api_keys.rs:110-121` calls `end_owned_by_key`). Its shape is exactly `GI-07` on the opposite side → **merged into `GI-02`**, not a new row: one seam (the revoke path), one `R: FR-063`.

**`tasks.md` — `T110` ticked** with the marker `Landed vizier@093c2b92 (with vizier@30500bf5); the gate it names passed.` and its three limits written on that line: `SC-013` 1 second not yet measured, revoke path by composition, `GI-05` open. A tick = task landed, **not** `FR-063` closed.

**Number collision:** `tasks.md` = **111 tasks, 65 `[x]` / 46 `[ ]`** — identical to the 09:15 note but with a **different composition** (`T026` not re-ticked in `77117f9d`, `T110` newly ticked). Same number, different state; it MUST be cited together with its composition.

## Key rotation decision (FR-063), 10 Nov 2026 09:20

**`@teteh-qa` finding:** `grep -rn rotate crates/vizier-http/src/api/v1` → **0 hits**. FR-063 says the key "can be rotated", but there is no rotation route.

**My decision, with its evidence:** rotation **needs no route of its own** — it is a composition of two routes that already exist: `create_api_key` (`crates/vizier-http/src/api/v1/auth.rs:294`) and `delete_api_key` (`:404`). The contract already writes it that way: *"key rotated (new key, old one retired) → the old key stops working; a session opened with it ends | reason `key_revoked`"* (`contracts/live-voice-frames.md:362`). So the requirement has a carrier; what is missing is only **one line in `spec.md`** stating "rotate = create a new key + revoke the old one, with the same consequences as revoke". That is spec semantics → `@system-analyst`, not me.

**What I did not decide:** whether rotation must have its own atomic endpoint (so that there is no window with two active keys). If kang wants that, it is a new requirement, not an interpretation of FR-063.
**Note 10 Nov 11:05:** the `@sepuh-be` report of 10:43 still used the DoD version of 9 Nov (#3 `end_owned_by_key`, #4 `cargo test -p vizier-core`). Both **were already replaced in this section at 09:15**, before that report came in. What needs adding is only the root cause of #4, in `GI-06`.

## DoD — T110 (v2, 10 Nov 2026 09:15 — old DoD #3 replaced because of a **fake green gate**)

**Why replaced:** `cargo test -p vizier-core --features provider-live-voice end_owned_by_key` exits with **exit 0 while running zero tests** — `end_owned_by_key` is a *method* name (`session.rs:519`), not a test name, so the filter matches zero functions and Rust still reports `test result: ok`. Re-measured: **7 targets, 7x `running 0 tests`, exit 0.** `@teteh-qa` finding 9 Nov 21:45; I reproduced it myself on 10 Nov.

> **New rule (applies to every DoD):** a `cargo test <filter>` command that runs **0 tests still exits 0**. A DoD MUST state **the number of tests that must run** (`running N tests`, N > 0) or use the full module path. Exit code alone is not evidence.

| # | Command | Expected output | Measured 10 Nov 09:15 |
|---|---|---|---|
| 1 | `cargo test -p vizier-core --features provider-live-voice live_voice::session::key_tests` | `running 5 tests` · 5 pass · 0 fail | ✅ **5 passed / 0 failed**, 1.33 s, exit 0 |
| 2 | `cargo check -p vizier-core` (without feature) | `Finished`, exit 0 — FR-041: a build without `provider-live-voice` still works | ✅ exit 0, 44,85 s |
| 3 | `cargo test -p vizier-core --lib live_voice` (without feature) | `running 6 tests` — **6 `schema::agent`/`service::agents::live_voice_input_tests`, 0 from the `live_voice` module**. The number 6 MUST NOT be read as "the live_voice module passes without the feature" | ✅ 6 passed / 0 failed, 0 from the `live_voice` module |
| 4 | `cd webui && npm test -- session` | `key_revoked` worded differently from `key_expired` and from `pressed_again` | ✅ measured by `@teteh-qa`: 2 files, **65 passed / 1 skipped**, 922 ms |

**What this DoD still does not prove — MUST be stated in the gate report:**
- **SC-013 "within 1 s"** — no instrument yet. Test #1 proves the path, the reason, and the registry cleanup; nothing measures the actual delay from the HTTP revoke until the frame reaches the client. It needs a timing test with a live server.
- **"Turn in flight is cancelled" is not asserted.** The code calls `abort_turn` before `end` (`session.rs:541-542`), but the five `key_tests` tests only state the count, the reason, the state, and the registry contents.
- **The client side is tested through the generic path**, not through the key reason (`session.test.ts:692` `provider_failed`, `:702` `idle`). The path is the same, but "same" is an inference.

**The wide gate limit, and it does not belong to this feature:** `cargo test -p vizier-core` (whole crate, without feature) = **1733 passed, 8 failed**; all eight failures are `projects::tools::lsp_tests` + `service::project_lsp` ("the server never became ready", 20 s), **reproduced on their own** after `cargo build -p vizier-core --example fake_lsp`. That is feature 054, not 055 — which is why DoD #2/#3 above were narrowed, rather than leaving the gate red because of another feature's inheritance.

**Owner:** my technical gate **✅ 10 Nov** (rows #1–#3) · behavioural signature `@teteh-qa` (behavioural task → the 4-scenario `qa-plan` has been written by QA; not yet signed).

## Q12 and T111 — what kang answered on 10 Nov 09:37, and what did **not** change

Kang answered item 4 (*"Boleh deh"*). Here is what changed and what did not:

- **Changed:** the direction *"answers arrive while being produced"* (level 1, FR-065…FR-070) was **confirmed for the second time** by the User — the first was 10 Nov 06:39 (*"level 1 dulu aja deh bro, tapi kalo pas gw tes terus latency nya masih kerasa ganggu baru gw minta level 2"*, already recorded in `spec.md:691` at `ef846fbd`). This second confirmation adds no condition; it repeats what already existed.
- **Not changed:** `T111` stays `[PROVISIONAL]`. That label is **not about permission** — permission has existed since 06:39 — but about **numbers**: there is no measurement yet saying how much is being bought. The spec writes it itself: *"until those numbers are re-measured against a real turn, a latency target would be a number invented to look finished"* (`spec.md:263-267`).
- **Because of that, what makes T111 schedulable is not "boleh", but these three things:** (1) my assessment with numbers re-measured on one real turn; (2) a written DoD; (3) an empty `@sepuh-be` WIP slot — right now it is not empty, because `T110` is in `gate` and `GI-01`/`GI-02` are in `antre`.

**If what kang meant by "Boleh" is *start building now*:** that is a priority decision and it is legitimate — but it still needs a DoD first, and it still queues behind T110. Say one line, and I will write the brief today.

## Latency: the approval line (2026-11-10 20:56, `@bro-pm` decision, written by Lead)

Written here **and** in `tasks.md:365` (`## Latency iteration — what is approved, what is not`)
so that nobody works on something that was only "approved in chat".

| Item | Status | Note |
|---|---|---|
| Partial STT **form A** (transcribe earlier, submit still when the utterance closes) | **APPROVED** | FR-010/FR-011 intact, Q12 untouched |
| Partial STT **form B** (submit a hypothesis before it closes) | **NO** | changes FR-011 `spec.md:515` + touches Q12 — a product decision |
| Hangover 800 → 250 ms | **NOT A DECISION** | 800 ms = FR-010 default `spec.md:514`; and it does **not** change SC-001 (mark & pipe shift together) — only feel, −550 ms |
| TTS segment-1 = first sentence | **DEBT, not new scope** | FR-013 `spec.md:517` already requires it; today it is violated |
| Level 2 (speculative turn) | **NOT DECIDED IN THE DOCUMENT** | taken to kang as one sentence; risk: a speculative tool call could act before being asked |
| Change TTS model | **NOT DECIDED IN THE DOCUMENT** | the agent's voice changing = kang's decision |

**Measured floor (not an assumption):** from 12 `synthesis started`→`segment sent` pairs across two binaries,
`ms ≈ 2329 + 474 × seconds of audio`; the shortest segment ever measured (1,76 s audio, 21 characters) still
takes **3 157 ms**. So the first segment ≈**3,2 s** with this provider, and a 3 second end-to-end target is not
reached without changing the voice. The numbers may be quoted: **3 turns = smoke · 20 = provisional · ≥50 = official**;
below 50 report median + range, not p90/p99.

**The 10 Nov sample is not a clean sample** — `LV-BARGE-BEFORE-AUDIO` discards the first turn of every call. Measure
after the three defects are closed.

## Merge `master@70d3509a` -> `055-live-voice` (16 Nov 2026)

**Not a task row**: no `FR-` governs the merge itself, and forcing one `FR-` onto this
would violate the `R:` rule. Recorded as a process fact so the numbers and the rest can be checked.

| Item | Value |
|---|---|
| Merge commit | **`vizier@893543d6`** — parents `474ead86` (branch) + `70d3509a` (master), merge-base `d36a4755` |
| Scale | 29 master-only commits vs 258 branch-only; 383 files changed (`+32.805 / −6.166`) |
| Submodules after the merge | `webui@0d5255f` (merge `5f3bef2`) · `design@35d633a` (merge `4c32bc2`) · `dummyplug@fba9813f` · `landing@a15d98d1` |
| Conflicts | 4 paths: `docs/src/SUMMARY.md` (Lead: Live Voice **2.23**, Custom Tools **2.24**), `crates/vizier-core/src/agents/agent/mod.rs` (Sepuh: steer **and** fast path alive), `webui` (Sepuh: `frames.ts` both sides), `design` (Sepuh: 0 conflicts) |
| Gate (run by Lead in this tree, not a quotation) | `cargo check -p vizier-core --features provider-live-voice` 0 · `-p vizier-http --features live-voice` 0 · `key_tests` **6 passed** · `fast_path` **6 passed** · `steer` **13 passed** + `dummyplug_e2e::messages_sent_while_busy_steer_the_running_turn` **1 passed (5,36 s)** · `webui` typecheck 0, `Test Files 114 passed`, `Tests 1298 passed | 1 skipped` · `design` typecheck 0 |
| Push | **already** — `origin/055-live-voice` = `e43a0c18` = HEAD (checked 16 Nov 11:28, `git log -1 origin/055-live-voice`); the old row `0 30` / "waiting for ACC" is **stale** |

### One red test that is not a red test (self-correction, 16 Nov 10:05)
I once concluded that `messages_sent_while_busy_steer_the_running_turn` was a **merge regression** because it failed 3/3
in the merge tree while pure master passed. **Wrong, and wrong in the same pattern I
warned Sepuh about:** the `dummyplug` submodule checkout in the main worktree was still `c6817ee` (the index was already
`fba9813f`, marked `+`), and `c6817ee` **does not yet have** the steer protocol (`test/steer-protocol.test.mjs`
does not exist; a 78 line diff in `src/protocol.mjs`). My master comparison worktree, by contrast, was initialised cleanly to
`fba9813f`, so the comparison was not equivalent. After `git submodule update --init dummyplug`, the same test
**1 passed, 5,36 s** in the merge tree. The derived rule (Lead memory `ops/tes-e2e-lulus-vakum`): a comparison MUST equalise the submodule **checkout**, not just its gitlink, and a duration of `0,00 s` on an e2e test means the test did not run.

### What is left unclosed
- `webui/app/api/schema.d.ts` is **not** regenerated yet (`node scripts/openapi.mjs` needs a live server at `VIZIER_URL`); not patched by hand.
- `design npm test` fails because `chromium_headless_shell-1248` is not installed yet (`npx playwright install chromium`); typecheck green.
- ~~Push to `origin/055-live-voice` waiting for kang's ACC~~ — **stale**: `git log -1 origin/055-live-voice` = `e43a0c18` = HEAD and `rev-list --left-right --count origin/055-live-voice...HEAD` = `0 0` (checked 16 Nov 11:28). The commits of this merge are already on the remote.

## ACC: drop the live voice feature gate (16 Nov 2026)

**The owner's decision, not a Lead proposal.** `muruyung` in `vizier-dev`/`fitur-live-voice-rev1` seq 2442:
*"Permintaan owner tuh jangan ada di feature gate."* Then seq 2444: *"Yaudah deh gas opsi B mang"*.
Option B = **drop the gate entirely** (not rename the flag). The Cargo gate is **not** a user button; the switch that
the user actually uses is the per-agent setting `live_voice.enabled` (default off,
`crates/vizier-core/src/schema/agent.rs:205`).

### Why B, with numbers already measured (at `e43a0c18`)

| Item | Measure |
|---|---|
| Feature contents | `provider-live-voice = []` — empty, 0 dependencies (`crates/vizier-core/Cargo.toml:38`) |
| Gate points | **16** `#[cfg]` (4 in `vizier-core`, 12 in `vizier-http`) + **4** `#![cfg]` in `crates/vizier-http/tests/` |
| Cost of dropping | 0 dep · `cargo check` **+0,3 s** (~5 %) · RSS **+20 MB** · symbols **+0,14 %** |
| What is gained | **+109 http tests always run** (wall +68 s, all green) |
| Value of the evidence lost | small: T014, the only "flag off" test, turns out to be **tautological** (`assert!(cfg!(feature = "live-voice"))` inside a file that is only compiled when the feature is on) |
| Defects of the gate today | absent from the CI flag matrix (`ci.yml:157`) · `AGENTS.md:16` does not list it · `crates/vizier/Cargo.toml:32` **forces** `vizier-http/live-voice`, so the binary is never without the feature · `app/src-tauri/Cargo.toml:30` on the contrary is **without** the feature → the desktop app today **does not** have live voice |

### Scope

- **Code** (`@sepuh-be`): delete the feature declaration in `crates/vizier-core/Cargo.toml` + `crates/vizier-http/Cargo.toml`, open the 16 `#[cfg]` + 4 `#![cfg]`, drop `features = ["live-voice"]` in `crates/vizier/Cargo.toml:32`, remove the tautological T014 test.
- **Spec** (`@system-analyst`): rewrite **FR-041** (no more "build without live voice"; the guarantee moves to the per-agent switch + default off) + a Delta Log entry. Route: downstream findings via Lead (rule §Jalur task SA, point 2).
- **Documents + board** (Lead): `docs/src/configuration/build-flavours.md`, `Readme.md:159/168`, `specs/055-live-voice/{plan,architecture,quickstart,tasks}.md` (T014/T108 rewritten), this ACC note.

### Status

| Step | Owner | Status |
|---|---|---|
| Brief + DoD | Lead | written 16 Nov, in `fitur-live-voice-rev1` |
| Code | `@sepuh-be` | `aktif` |
| FR-041 + Delta Log | `@system-analyst` | `aktif` (parallel, does not touch code) |
| Documents + board | Lead | **antre** — triggered by gate commit C17; file list + `[DOD+]` below |
| Gate | Lead | after code + spec |

`R:` **FR-041** (rewritten by SA in this cycle). Its history: this gate was born from an early
version of FR-041 ("a build without it MUST leave every existing speech feature working"); the owner decided
that guarantee is kept another way — always compiled, switched off per-agent.

### `[DOD+]` — additional criteria from Lead (machine-checkable)

Added 16 Nov 2026 13:03 WIB, at kang's request (scheduled task `brief-lepas-gate-live-voice`).
The criteria below are **outside the spec** and all of them are commands + expected output; this is the first round, so the
rule "3 tasks → MUST become an FR or be withdrawn" does not apply yet.

- **Zero `running 0 tests`.** `cargo test -p vizier-http` (without `--features`) MUST print `running N tests`
  with N>0 in all four binaries `live_voice_{session,frames,barge_in,delivery}`. One `running 0 tests` =
  DoD failed even if the rest is green — that is exactly the defect that made this gate never test anything.
- **0 dependencies.** MUST be **the same** as the baseline the agent recorded before the change, **measured with the
  exact same command**. The number depends on the command, so state the command: `cargo tree -p vizier-core
  --no-default-features -e normal --prefix none | sort -u | wc -l` -> **625**; the same form without `sort -u`
  -> **1318**; without `-e normal` -> **1498** (verified by Lead 2026-11-16 at `b7001100`, all three recomputed
  in the same tree). A change = a finding, not a "reasonable increase". **MUST NOT quote `cargo tree` numbers without the command**
  — `625` vs `1318` is not a contradiction, they are two different questions.
- **No leftover flag name** in the tree: `git grep -n 'provider-live-voice\|feature = "live-voice"' -- crates/ docs/
  Readme.md AGENTS.md .github/` **empty**; in `specs/055-live-voice/**` only historical quotations
  marked C17 may remain.
- **Test numbers come from `cargo test`, not from the brief.** The Lead brief (seq 2447) contains `15 tes / +14` which is
  wrong — the correct one is **32**, broken down `7 / 8 / 11 / 6`. The rule: the difference is computed against the agent's own
  baseline, and the final number = the command's output in the committed tree (`repo@hash`).
- **The runtime switch does not change**: `live_voice.enabled` stays `false` by default
  (`crates/vizier-core/src/schema/agent.rs:205`) and `agent_detail.live_voice` stays a **boolean**
  (`service/agents.rs:75`), not a nested object.

### Documents + board (Lead, after the gate commit lands)

Not done before the code lands: a `build-flavours.md`/`Readme.md` that states "there is no flag"
while HEAD still forces it is a document that lies about its own state — the kind of mistake
QA has already caught three times today. The order: **gate commit first, documents follow in the same round.**

| File | What changes |
|---|---|
| `docs/src/configuration/build-flavours.md` | line 15 (the `provider-live-voice` table row), §30 heading, §32–36 contents, §39 command example → this section becomes a note that live voice is **always** compiled and its switch is the per-agent setting |
| `Readme.md:159,168` | the `--features live-voice` build example and its 8m45s number are replaced by a build without the flag |
| `specs/055-live-voice/tasks.md` | **T014** rewritten (tautological assertion `assert!(cfg!(feature = "live-voice"))` → a test that is genuinely red if `topic.rs:798` is pulled out; its replacement is `a_start_is_ignored_while_the_agent_switch_is_off`), **T108** its command replaced (`cargo build -p vizier --no-default-features` proves nothing as long as `crates/vizier/Cargo.toml:32` forces the feature) |

Check already run 16 Nov 13:03 and the result **needs no change**: `AGENTS.md:16` zero mentions of
`provider-live-voice` (the line never actually contained it) and `.github/workflows/ci.yml` zero lines
containing `live` (`grep -n -i live` empty; the flag matrix at `:157` holds seven other flags). So "CI and
AGENTS.md drift" recorded in memory is **stale** for those two files.

### C17 round findings — queued (not active)

| Item | R | Owner | Status | Content |
|---|---|---|---|---|
| `AQ-002` FR-004 on the wire: `agent_detail.live_voice` is only a boolean, there is no reason *which one* is missing | FR-004 | `@sepuh-be` (after the gate brief is done — WIP=1) | **queued** | Found by `@system-analyst` while rewriting FR-041 (16 Nov). `LiveVoiceRow.tsx` has three reason variants (`voice`, `recognizer`, generic) but `liveVoiceMissing` is declared at `Composer.tsx:71`, destructured at `:114`, used at `:487` with `?? null` — **zero callers**, so the two specific variants never appear. On the server side `agent_detail.live_voice` is just one boolean (`service/agents.rs:75`, value `:605` = switch ∧ recogniser ∧ voice). Consequence: the reason says *that* it is not set, not *which one* is missing, and does not say where to set it → **FR-004 half satisfied on the wire**. Fix = a wire change (reason code beside the boolean) + spec amendment by SA. **Not in any brief yet.** |

Two other webui defects recorded but **not** queued (not blockers, and not this round's):
`LiveVoiceControl.tsx:131-135` — the branch `if (!capability) { onOpenSettings?.() }` is unreachable: the only giver of `start` is `onStart`/`onRestart` in `LiveVoiceRow.tsx:146/148`, and both are only rendered when `capability` is true; `onOpenSettings` is also never passed in from `Composer.tsx`.

## Commit anomaly 16 Nov 12:45–12:46 — two paths touching the same files

Recorded not to blame anyone, but because the pattern is dangerous and recurring.

**What happened.** Two commit paths ran on the same branch within three minutes:

| Time | Commit | Path |
|---|---|---|
| 12:42 | `394a61bd` (already on origin) | `analysis.md` + `release-checklist.md` |
| 12:45 | `2a87fc9d` | `analysis.md` + `release-checklist.md` + `CHK029` — **not "equivalent content"** (`@system-analyst`, 12:48): `git diff 394a61bd 2a87fc9d` = 2 lines, and `2a87fc9d` is the more complete one. What won in the merge: `2a87fc9d` |
| 12:45 | `e3861d12` | `state-matrix.md` repin `fb4315fc` |
| 12:46 | `5c84baae` | merge `origin` → local, current HEAD |

In between there was `reflog: reset: moving to fb4315fc` that **discarded `394a61bd` and `44ef1cb8`** from the local branch before being replaced by the commits above. The final content is correct (the blobs match what QA/SA reported), but:

- **`reset --soft` was run while the working tree held other agents' uncommitted work** (correction 12:52: this note first said `--hard`, wrong — the reflog reads `reset: moving to fb4315fc` and the index stayed staged, that is `--soft`). At the time there were 14 files under `crates/**` belonging to `@sepuh-be` in the tree, `+101/−61`.
  **What saved Sepuh's work was not a rule, but the accident of a flag choice**: `--soft` does not touch the working tree, `--hard` would have deleted it. So the conclusion is harsher, not softer: the prohibition below applies to **every** form of `reset`, and `--hard` in this repo is equivalent to deleting someone's work.
- **The history forks** and twin commits (`394a61bd` vs `2a87fc9d`) make it harder for the next reader, especially a CI that counts commits.

**Derived rule I use from now on:** before touching git in this shared repo, `git status --porcelain` MUST be read first; if there are modified files that are not your own, there is **no** `reset`, `checkout --`, `stash`, or `clean`. Before committing: `git fetch` + `git log origin/… ` — not commit then merge later. And push is done from one path only.

### Gate C17 lands — Lead review result (`c71f0e64`)

| Item | Evidence |
|---|---|
| Commit | **`c71f0e64`** `refactor(live-voice)!: lepas feature gate …` — 14 files, `+101/−61` |
| Content = Sepuh's report | `md5sum` of the files in the commit: `topic.rs` **`89fabe7d…`**, `live_voice_frames.rs` **`d500d827…`** — identical to what he reported from the working tree |
| Basis of Sepuh's claim | `git diff --stat bc5d3235 693fdcfd -- crates/ app/ Cargo.toml Cargo.lock` = **0 lines** |
| Remaining flag names | `git grep -n 'provider-live-voice\|feature = \"live-voice\"' -- crates/` → **empty** (exit 1) |
| Feature declarations | `vizier-core/Cargo.toml` no longer has `provider-live-voice`, `full` is clean; `vizier-http/Cargo.toml` **without a `[features]` block**; `vizier/Cargo.toml:32` → `vizier-http.workspace = true` |
| Tests in four files | **7 / 8 / 11 / 6 = 32** at `c71f0e64` (pattern `^[[:space:]]*#\[(tokio::)?test` without anchor) |
| Tautology | `fn the_flag_off_build_has_no_live_voice_surface` → **0 hits**; replaced by `a_start_is_ignored_while_the_agent_switch_is_off` at `live_voice_frames.rs:190` |
| Guard that remains | `topic.rs:797` `if !config.tools.live_voice.enabled` — present |

**Measured numbers (two parties, same tree):** @sepuh-be in a working tree whose content is identical to the commit; @teteh-qa measured directly at `c71f0e64` (`b7001100`). Both: `cargo test -p vizier-http` without flags → **12 `test result` blocks, 108 tests + 1 doc-test, 0 failed**, the four `live_voice_*` 7/8/11/6, **zero `running 0 tests`**; `cargo test -p vizier-core --lib live_voice::` → **56 passed**; `-p vizier-core --no-default-features` → lib **2015 passed, 7 ignored, 0 failed**; `cargo tree … -e normal | wc -l` **1318 = 1318**. Lead did **not** re-run `cargo` (one cargo at a time; the commit content is byte-identical to what both measured) — what Lead verified itself is static and listed in the table above.

### Sepuh's finding that changes the picture: the featureless build has been broken since `449aca26`

`fn owns_session` (`crates/vizier-http/src/topic.rs:57`, from T110/FR-063) uses `&vizier_core::live_voice::LiveVoiceSession` **without `#[cfg]`**, while the module is gated. So at `bc5d3235`:

- `cargo test -p vizier-http` **without flags does not compile** (`E0433`), not `running 0 tests`. The baseline premise in Lead's brief (and the early version of A4) **could never have happened**.
- The old FR-041 claim — "a build without it leaves every existing speech feature working" — is **wrong in the most literal sense**: that build does not produce a build.
- `app/src-tauri/Cargo.toml:29-30` depends on `vizier-core` and `vizier-http` **without features** → the desktop build **does not compile** since `449aca26`. Gate C17 fixes it as a side effect.
  **PROVEN FROM BOTH SIDES by `@sepuh-be` (2026-11-16 14:10):** `cd app && cargo check -p vizier-app --no-default-features` at HEAD `c9e9f0c8` → **EXIT 0** (8m56s, `CARGO_BUILD_JOBS=2`); in worktree `bc5d3235` → **EXIT 101** with `E0433: cannot find live_voice in vizier_core` at `vizier-http/src/topic.rs:58`. No longer an argument.
  **Its limit, and do not read further:** the `shell`/Tauri glue **was never** compiled — this machine has no `webkit2gtk-4.1`/`javascriptcoregtk-4.1`/`gtk+-3.0` (`pkg-config` fails). So what is proven = **the desktop crate's dependencies are no longer impossible to compile**, not "the desktop app builds". Installing those system packages is the User's decision, not the agent's authority.
  **An erratum Sepuh raised himself, and worth recording:** his first baseline attempt failed because `build.rs` demands `webui/build/client`, which was absent in the fresh worktree — not because of the gate. He staged `webui/build/client` (gitlink `webui@0d5255f` the same at both commits, checked before copying) and retried; `E0433` only appeared on the second attempt. Two attempts, two different causes; had he stopped at the first, the report would contain evidence not about the gate at all.

**Consequence for documentation:** T108 now has **two** dead reasons (not one), and `crates/vizier-http/Cargo.toml:23-25` (comment "still builds against an engine without the live voice feature") **has been untrue since `449aca26`** — removed along with the `[features]` block in this commit.

### Two things Sepuh reported and did not touch
1. `topic.rs:44-49` — two different doc comments (T109/FR-062 and T110/FR-063) piled on top of one function `owns_session`. **Pre-existing** (`474ead86`), not a gate effect. Lead's decision: **not tidied in the gate round** (mixing two things in one diff), and **not** made into a new task without `R:`; noted here so it does not get lost. If `topic.rs` is touched again for the T113/T114 era, tidy it then.
2. Tauri build: `cargo check -p vizier-app --no-default-features` has been run from both sides (see above). What is still not run: the `shell`/Tauri glue with default features, because its system prerequisites are absent on this machine.

## Status C17 + T113/T114 (16 Nov 2026, after the gate)

| Step | Owner | Status |
|---|---|---|
| Brief + DoD C17 | Lead | **`selesai`** — brief 2447, two number corrections (`+14` → `+32`, `31` → `32`) |
| Gate C17 (feature removal) | `@sepuh-be` | **`selesai`** — `c71f0e64`, reviewed by Lead, pushed |
| FR-041 + Delta Log | `@system-analyst` | **`selesai`** — `5d7a188e` |
| FR-004 (wire + amendment) | `@system-analyst` | **`selesai`** — `fb4315fc` then `d34103c7` (switch-on rule) |
| Documents + board | Lead | **`selesai`** — `ef129ab9` (`build-flavours.md`, `Readme.md`), board |
| **T113** (`live_voice_missing` in `AgentDetail`) | `@sepuh-be` | **`selesai`** (code + gate). Brief 2576; code `25689640` (+121/−14, 2 files), cosmetic `a06afd13`. **Gate AQ-002 PASS** (`@teteh-qa`; **honest ordering:** this entry was written 15:44 before the full run finished, the run was 15:54-15:58 at `5423d393` — what makes it valid is the measurement, not this entry) `@teteh-qa`: measured at `25689640`, repeated identically at `03ce4589`, holds at HEAD `5423d393` (`git diff 03ce4589 HEAD -- crates/` = 0 lines) — `git grep -c live_voice_missing HEAD -- crates/` 13 · `--test agents_seam` 12 · `-p vizier-http` 12 blocks / 115 passed (114 functions + 1 doc-test) · the four `live_voice_*` 7/8/11/6 · `--lib live_voice::` 56 · `--no-default-features` 2015 / 7 ignored · `check --features full` + `clippy` exit 0. A4 rose at `28d8b3bb`, stale tail `5423d393`. The earlier red is real: guard `if !enabled { return None }` removed -> 2 tests FAILED `left: Some(\"recognizer\"), right: None`. |
| **T114** (reason pill reaches the screen) | `@sepuh-be` (code) / Lead (commit) | **`gate` passes — `selesai` waits for the User.** Code `webui@7732f0a` (5 files, +84/−1) + gitlink in the superproject; gate **AQ-003 PASS WITH KNOWN ISSUES**, formal verdict signed **`@teteh-qa` 17:19** at that pin (measured in `e2bac5ff`; `git diff 7c9b3c50 HEAD -- crates/` = **0 lines**, gitlink `7732f0a` unchanged, so the figures carry to `7c9b3c50`). **Scope of that signature, corrected 17:19:** `@teteh-qa` signs **one** red-first form - the **assert** one (passthrough mutant -> `expected 'future-value' to be null`, `1 failed / 5 passed`). The **compile** form (`Cannot find module './voice'`) is **not QA's measurement and is not covered by AQ-003**; it came from `@sepuh-be`'s own report. Two process notes carried rather than tidied: the entry and `[x]` T114 were written at **17:11 (`b2d0504e`), eight minutes before the verdict existed** - the same shape as AQ-002; and the board is 21+ commits ahead of `origin` (`f78bd214`), so no third party can verify either hash from a clean clone yet. Green **6 / 22 / 13 files · 186 passed \| 1 skipped**, `typecheck` + `tsc` EXIT 0 with 0 `error TS`, grep chain **7 / 3 / 2**. QA also proved three of those row tests are **characterisation**: `LiveVoiceRow.tsx` unchanged `0d5255f..7732f0a` and the new-version tests pass against the old component (22 passed). **Remaining with IDs:** `GI-10` (client chain without render tests) · `GI-11` (second surface `WorkspaceComposer.tsx`). **Not closed:** the `GET /agents/{id}` route, `schema.d.ts`, the pill in the browser, the design column. |
| Block of 38 `code_ref` citations | `@teteh-qa` | **`aktif`** — pasted into `state-matrix.md`, then SA raises the pin |
| `AQ-002` | — | closed by T113 + T114 (not a separate task) |

**Three numbers that changed twice today, and the cause is the same — the counting tool, not the code:**
`15 / +14` (without `#[tokio::test]`), `49` (anchored on `\]`, missing 7 `start_paused`), `57` (counting test **names**, when the `cargo test` filter matches **paths**: `mod live_voice_input_tests` `agents.rs:1207` contributes 5 + 1 at `schema/agent.rs:612` = **62**).

**Derived rule:** test numbers only from `running N tests` in the output, or from a count that names its **exact pattern**; the `cargo test` filter = path, so always use `modul::` for a module.

**Leftovers that are not gate items:** `schema.d.ts` not regenerated yet (needs a live server; now two reasons: `AgentDetail` changed shape), `public-api.txt` stale since 2026-11-07 (pre-existing debt, `just public-api`, not guarded by CI), `docs` e2e `design npm test` needs `npx playwright install chromium`, and `WorkspaceComposer.tsx` does not pass live voice at all (0 hits) — a second surface without the pill, not scheduled yet.


## Brief `GI-05` — a revoked key MUST NOT be able to start a session again (FR-064)

**Sent 16 Nov 2026 (ws `vizier-dev`/`fitur-live-voice-rev1`). Code owner: `@sepuh-be`. WIP=1, one artefact.**
**Base: `vizier@b2d0504e`** (`crates/` has not moved since `a06afd13`: `git diff a06afd13 HEAD -- crates/` = 0 lines when this brief was written).

### The defect, read from code not from the board

- `key_lapsed` (`crates/vizier-http/src/topic.rs:69`) only reads `principal.method` — and that principal is **stamped once** when the connection opens. So on the next frame it still holds the old `key_id` and permissions.
- The per-frame check (`topic.rs:727`) calls `key_lapsed` only: **expiry** is caught, **revocation** is not.
- Revocation is only **pushed** by `delete_api_key` → `LiveVoiceSessions::end_owned_by_key` (`crates/vizier-core/src/service/api_keys.rs:93-121`, `session.rs:911`). That ends the session that is **currently running**, but does not prevent the next `live_voice_start` on the same socket.
- FR-064 (`spec.md:604`): *"A request whose key is missing, revoked, expired or scoped away from live voice MUST be refused **before any session state is created**."* Today the `Start` path only checks `may_start_live_voice` (`topic.rs:756` = scope), not whether the key exists.

**A consequence that can be shown:** revoke the key → session ends → the same socket sends `live_voice_start` again → **a new session is created** using a key that is no longer in storage.

### What must be proven (red form: **an assert on the session path**, not a compile failure)

Write the tests **first**, run them, paste the red. Two new tests in `crates/vizier-http/src/topic.rs`, `mod tests` — using the existing `socket(principal, topic, key_origin)` harness (`:331`), **with no new files and no production code changes to make them red**.

1. `a_revoked_key_cannot_start_a_second_call_on_the_same_socket`
   - Create a key in storage via `deps.storage` (`create_api_key`), the principal carries that `key_id`.
   - Revoke via the path production uses — `service::api_keys::delete(&services, &principal, key_id)` — so that `end_owned_by_key` also runs, not just `storage.delete_api_key`.
   - Send `live_voice_start` **on the same socket**.
   - Assert **three** things: the outgoing frame is `LiveVoiceControl::Error` (not `Started`); the session is **not** in the registry; the error message **does not** mention the presence or absence of agents, topics, or other sessions (the last sentence of FR-064).
2. `a_revoked_key_at_the_socket_still_ends_the_call_it_was_running`
   - A session is **already running** with that key, then it is revoked, then one audio frame arrives.
   - Assert: session `Ended`, reason `key_revoked`, `command:abort` **exactly one** — this guards the existing T110 path so the GI-05 fix does not break it.

If (1) turns out **green before any code change**, stop and report: that means the defect is not as written above, and this brief is wrong, not the test.

### Fix direction (pick the cheapest, not both)

FR-064 only demands rejection **before state is created**, so a check on the `Start` path is enough — do **not** add a storage query per audio frame (hot path, 20 ms/frame).

- **Way A (recommended):** in `ClientAction::Start`, before `sessions.get(...)`, if the principal carries a `key_id`, make sure that key **still exists** in storage (`storage.list_api_keys(&principal.user_id)` then find its id — the API already used by `service::api_keys::delete`). Not there → `LiveVoiceControl::Error` with `EndReason::PermissionWithdrawn`, `return` before anything is created.
- **Way B:** `TopicConnection` keeps an `Arc<HashSet<String>>` of keys revoked for the life of the process, filled by `end_owned_by_key`. Cheaper per frame, but adds process state and **does not** catch keys deleted through other paths (e.g. storage directly). If you pick B, state in the report what is **not** caught.
- **Do not** change `key_lapsed` to cover revocation — its name is about time, and `topic.rs:727` calls it every frame; putting a storage query there makes every audio frame touch the DB.

### DoD — exact commands + expected numbers

Baseline I measured myself today at `vizier@b2d0504e`, `CARGO_TARGET_DIR=/home/muruyung/vz-gate-target`, `env -u PYTHONHOME -u PYTHONPATH`, idle machine:

```
cargo test -p vizier-http --lib topic::tests
  → running 6 tests · 6 passed; 0 failed · 52 filtered out · 0.87s · EXIT=0
```

| # | Command | Expectation |
|---|---|---|
| 1 | `cargo test -p vizier-http --lib topic::tests` (with the two new tests) | **`running 8 tests`**, `8 passed`, 0 failed, EXIT 0 — and both new names appear in the list |
| 2 | `cargo test -p vizier-http` (full, without `--lib`) | **12 blocks**, all `ok`, **0 `running 0 tests`**, Σ `passed` = **117** (115 + 2). That 117 figure is an **expectation from arithmetic** — if your output differs, **report your output**, do not tune the number |
| 3 | `cargo test -p vizier-core --lib live_voice::` | `running 56`, 56 passed — **does not move** (no core test added) |
| 4 | `cargo check -p vizier-http` · `cargo clippy -p vizier-http` | EXIT 0 · EXIT 0 |
| 5 | `git status --porcelain -- crates/` after the commit | empty |

**Red-first in a separate `git worktree`** (queue rule 9) — not in the shared tree — and restore via `git show HEAD:<path>`, then prove `git diff` is empty. If the red form has to touch production code, just show the test **failing** against the current code, and state the form as it is.

### Limits that MUST be in the report

- Does **not** close silent sockets: FR-063a decided expiry/revocation is judged on the **next frame**, not by a timer. `GI-03` stays open for the interaction with T062.
- Does **not** close the route level: `GET /agents/{id}` and friends are still untested (a gap already written in T113).
- Does **not** touch `webui`: `key_revoked` reaching the screen is `GI-04` (composition, not a single assert).
- `public-api.txt` is not regenerated (pre-existing debt `8b616af0`).

**Done =** two new tests green in the commit, `-p vizier-http` 12 blocks, numbers quoted from the output, and **the earlier red pasted**. Commit by Lead, gate `@teteh-qa`. If any premise above does not match what you see at `b2d0504e`, **bounce this brief back** — do not adapt yourself.


### `@bro-pm` decision: `GI-05` stays `antre`, its own test (16 Nov 2026 15:06)

Accepted without objection. Three binding points:

1. **`GI-05` does not enter the WIP slot now.** One artefact per brief; a second artefact in the same crate makes the earlier red impossible to attribute to one owner. Plus: `GI-05` is **not test work alone** — `key_lapsed` (`crates/vizier-http/src/topic.rs:69`) only reads `principal.method`, which is stamped **once** when the connection opens, and `may_start_live_voice` (`crates/vizier-http/src/topic.rs:78`) only checks scope, not revocation (both line numbers verified at `25689640`, not quoted from a message). The test will be red and will demand a code change on the session path, right in the area just merged.
2. **`GI-05` test separate** from the FR-004 round: one artefact = one brief = one commit, its form being revoke key -> `live_voice_start` **again on the same socket**, red first.
3. **Binding order:** T113 commit -> QA runs A4 at that hash -> only then does `GI-05` get its own brief. The board records that steps 1 and 2 already happened after this decision was written (T113 `25689640`, A4 recorded by `@teteh-qa` at `03ce4589`); so `GI-05` may be briefed, **not that day** — `@bro-pm` stated there is no new job today.

**A4 target used (from `@teteh-qa`, unchanged):** `git grep -c live_voice_missing HEAD -- crates/` >=2 · `--test agents_seam` **12** · `-p vizier-http` **12 blocks / 115 passed**, the four `live_voice_*` still **7/8/11/6** · `--lib live_voice::` **56** · `-p vizier-core --no-default-features` **2015 / 7 ignored**.

**More urgent than `GI-05`, and why:** the first gate on the merged tree. `893543d6` is pushed, 0 conflicts, submodule clean — that is **tree shape**, not test results.

**Number note:** the head used is `10a3c180`. If anyone cites `e43a0c18` as HEAD, that is `@bro-pm`'s stale note, not a new commit.

## Number evidence rules (binding on all agents, 16 Nov 2026)

Written on the board because `@teteh-qa` had already written them in her private skill (v2) and **a skill is not a team document**. These rules were born from **six** wrong numbers in one day, all one disease: not the code being wrong, but **the thing being measured**.

### 1. Every number names five things
`repo@hash` · the **object** (commit or working tree) · the exact command · the **unit** (`N tests` vs the count of `passed`; occurrences vs kinds) · the **scope** (table rows vs the whole file).

Examples that failed today because one of them was missing:
- `15 tes / +14` — the `grep` pattern does not name itself, it misses `#[tokio::test]`.
- `49` — the pattern anchored on `\]` misses `7 × start_paused`.
- `57` — counting test **names**; the `cargo test` filter matches **paths** (`mod live_voice_input_tests` + 1 at `schema/agent.rs` = `62`).
- `42/35` — correct at `afc5e90f`, not at `281e51fd` (there `42/34`).
- `47/39 "di HEAD"` — that figure belongs to `e6eeffa4`; HEAD gives `52/43`.
- `25` — correct as **occurrences**, not kinds (`19 jenis` does not reproduce).
- `agent_detail 26→12 tag` — measured in the **working tree**, reported as a commit (`13`).

### 2. Do not measure the working tree for a number you will cite
`git log -1` first, `git status --porcelain -- <path>` must be empty, then run. If it is not empty: **"ready, but held"**, not a number.

### 3. `git grep` by default reads the working tree
A command written without `HEAD` **will happily read someone else's draft**. Tested 16 Nov: `git grep -c live_voice_missing -- crates/` = `agents.rs:4 · agents_seam.rs:9` (draft), `… HEAD --` = **exit 1** (commit). Always `git grep … HEAD --`.

### 4. Test numbers only from the output
`running N tests` / `passed` **per target**, not a sum, not arithmetic in your head ("minus one, so…") — that is the source of `+14` and `31`. Differences from your **own** baseline; the final number = the command output in the committed tree.

### 5. `cargo tree` numbers MUST come with their command
`-e normal --prefix none | sort -u | wc -l` → **625**; `-e normal | wc -l` → **1318**; without `-e normal` → **1498**. Three numbers, one tree; without the command, the number is not a number.

### 6. One `cargo` at a time
This includes `cargo check`. Two concurrent builds hold the lock and wreck the durations; duration itself is **not** used as a benchmark (conformance 91,88 s vs 45,16 s for the same test).

### 7. When numbers disagree: write the definition, do not swap the number
Today four readings over the same citation set (`27/7/6/2`, `22/7/9/2/2`, `30/7/2/3`, `33/7/2`) all sum correctly. What is used for work is not the breakdown, but **the list of tokens**.

### 8. Shared branch: no `--amend`, no `--force`, and read `git log -1` right before committing (16 Nov 2026)
Four incidents today, one pattern: **orphaned** commits because the shared branch is used in turn.
Binding rules:

- **Do not `git commit --amend`** on a shared branch. `--amend` rewrites a commit that may already have been read, measured, or cited by another agent; the numbers attached to it then point at an object that no longer exists.
- If you still must change your own commit, **`git log -1` right before** that command, and **only** if the top commit is still yours (judged from content, not author — all agents write with the same identity on this machine).
- **Do not `git push --force`** without mentioning it on the board first. If it has already happened, recover with `git reset` to the correct object and report the hash before/after, do not stay silent.
- `git status` before commit is **not enough**: it does not see someone else's commit landing between your read and your write. What catches it is `git log --oneline -1` + `git rev-list --count` after the commit.
- If your report's hash no longer equals `HEAD`, **state the new hash** in the next report. The old hash remains valid as "measured at" — as long as it is stated as the past, not the present.

### 9. The shared working tree is not a place to experiment (16 Nov 2026, real incident)
`@sepuh-be` placed a **deliberate mutation** (early-return removed) in `crates/vizier-core/src/service/agents.rs` around 16:04-16:07:30, **while two other agents' measurements were running** (QA `cargo test -p vizier-http`, Lead 10x `--lib live_voice::` loop). He restored it himself and admitted it before reporting progress - the right attitude, and it still has to become a rule:

- **Red-first is proven in your own clone/branch**, or through a test that fails to **compile** (the test file does not exist yet), not by changing source that others are reading.
- If the shared source **must** be touched to prove red: say so on the board before doing it, and stop if a measurement is running.
- **Restoration MUST come from the commit**, not from a working copy: `git show HEAD:<path> > <path>`, then compare `md5sum` with `git show HEAD:<path> | md5sum`. Today a `cp` from a 16:05 backup **brought back the shadowing that had already been fixed** at `a06afd13`; caught by `git diff`, not by a test.
- Numbers produced in that window are not automatically wrong, but must be declared vulnerable: state the measurement time and that the binary may come from source before/after the mutation.

### 10. Push the submodule **before** the superproject (16 Nov 2026, found after `origin` already pointed at a commit absent from the remote)
The incident is real and **still open** as this is written: `origin/055-live-voice` (superproject `vizier`) carries gitlink `webui` = `7732f0a...`, while `git -C webui ls-remote origin | grep 7732f0a` = **0 lines** — that commit exists only on this machine. Anyone who clones/fetches then runs `git submodule update --init webui` will fail (`reference is not a tree`), and no command in the superproject tells them why.

Rules:
- **Mandatory order: submodule first, superproject last.** A superproject pointing at a commit not yet on the remote is a broken repo for everyone except its author.
- Before pushing the superproject: `git -C <sub> rev-list --left-right --count origin/<branch>...HEAD` must be `0 0`, or its content is already known safe. If not, **do not push the superproject yet**.
- If it has already happened: there are two options and both must be stated — push the submodule, or point the gitlink back at a commit that is on the remote. Do not stay silent.
- This check is **not** caught by the superproject's `git status` (only `M webui` if dirty) nor by `git submodule status` (it reads the local tree). What catches it: `git ls-tree <remote-ref> webui` compared against `git -C webui ls-remote origin`.


### 11. Repo artefacts are English; pushing waits for the User (16 Nov 2026, User instruction 17:11)

Two binding rules, from the User directly:

- **Commit messages and every file under `specs/**` are written in English.** Chat with the User stays Bahasa Indonesia; a brief quoted in chat may be Indonesian, but what lands in the repo is English. This covers `spec.md`, `tasks.md`, `_antrean.md`, `checklists/**`, `state-matrix.md`, `plan.md`, `architecture.md`, `quickstart.md`, and anything added later.
- **Do not push on every commit.** Local commits pile up; `git push` happens only after the User says so. While waiting, a report states how many commits are ahead of origin and whether any of them touch `crates/` or `webui/` (`git diff --name-only origin/<branch>..HEAD -- crates/ webui/`), so the held-back set is checkable rather than assumed.
- **No retroactive rewrite.** `--amend`, `rebase` and force-push stay banned (rule 8), so existing Indonesian text is translated when a file is next touched, or in one dedicated pass the User approves. A translation is never mixed with a content change: two diffs, two commits.
- Submodule-first ordering (rule 10) still applies whenever the push does happen.
