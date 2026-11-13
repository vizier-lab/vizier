# Tasks: Fleet row ownership, fencing and lease-based recovery

**Input**: Design documents from `specs/057-fleet-row-ownership/`
**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/ownership.md](contracts/ownership.md), [contracts/recovery.md](contracts/recovery.md), [quickstart.md](quickstart.md), [audit.md](audit.md)

**Tests**: requested. Issue #164 and spec SC-001–SC-006 ask for two-owner tests against PostgreSQL and unchanged single-node tests. Within each story, write the tests first and see them fail before implementing.

**Paths**: everything is in `crates/vizier-core/src/` (abbreviated `core/` below) unless a full path is given. `core/storage/sqlite/` holds the one `SqlStorage` implementation that runs on **both** SQLite and PostgreSQL (spec 045). Backend-specific SQL goes through `Dialect`, never an `if` on the backend.

**Two-backend tests**: every storage or recovery test below is registered with `crate::postgres_tests!(…)` in its module, so it runs once on SQLite and once on PostgreSQL. Two-owner tests take their stores from `tests_support::postgres_store_pair()` (T006) and return early on SQLite, where one process is the only owner.

**PR boundaries** (plan "Delivery order"): Phases 1–2 = PR 1, Phase 3 = PR 2, Phases 4–7 = PR 3, Phases 8–9 = PR 4. Single node must behave the same after every PR.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: US1–US6 from [spec.md](spec.md)

---

## Phase 1: Setup (shared infrastructure)

**Purpose**: the schema every story writes to.

- [X] T001 Create `core/storage/sql/postgres/migrations/0007_row_ownership.sql`. It creates `owner_fence (scope TEXT COLLATE "C" PRIMARY KEY, node TEXT COLLATE "C" NOT NULL, token BIGINT NOT NULL, lease_token BIGINT, claimed_at TEXT COLLATE "C" NOT NULL)` and adds `owner_node TEXT COLLATE "C"` and `owner_token BIGINT` (both nullable, `ADD COLUMN IF NOT EXISTS`) to `session_turn`, `task_run`, `background_job`, `client_tool_call` and `sensitive_ask`. It adds indexes `client_tool_call (agent_id, status)`, `sensitive_ask (agent_id, state)` and `background_job (origin_agent, state)` (`CREATE INDEX IF NOT EXISTS`). Head comment in the style of `0002_client_call_claim.sql`: why the columns exist, and that `0001` is never edited. Register the file wherever `core/storage/sql/postgres/migrate.rs` lists migrations.
- [X] T002 [P] In `core/storage/sqlite/mod.rs`, add `CREATE TABLE IF NOT EXISTS owner_fence (scope TEXT PRIMARY KEY, node TEXT NOT NULL, token INTEGER NOT NULL, lease_token INTEGER, claimed_at TEXT NOT NULL)`. Add the ten owner columns with `add_column_if_missing(conn, "<table>", "owner_node", "TEXT")` and `(…, "owner_token", "INTEGER")` next to each table's existing schema init, and the three indexes from T001.
- [ ] T003 [P] Add one checklist entry to issue #82 (comment on it with `gh issue comment 82 --repo vizier-lab/vizier`, after confirming the text with the user). It covers the ten `add_column_if_missing` calls from T002 and the rule "a null `owner_token` is a row from before 057, owned by an expired owner" (research R9), naming `storage/sqlite/mod.rs` and the recovery predicates in `core/recovery.rs` as the code that depends on it.

---

## Phase 2: Foundational (blocking prerequisites)

**Purpose**: the ownership primitive from [contracts/ownership.md](contracts/ownership.md). Nothing calls it yet, so single node is unchanged.

**⚠️ No user-story work can begin until this phase is complete.**

- [X] T004 Create `core/storage/fence.rs`, with `mod fence;` added in `core/storage/mod.rs`. It contains:
  - `enum Scope { Agent(AgentId), Control }`, with `Display` as `agent:{id}` / `control`.
  - `struct Fence { scope, node: String, token: u64 }`.
  - `struct OwnershipLost { scope: String }`, with a hand-written `Display` ("ownership of {scope} moved to a newer owner") and `std::error::Error`, following `agents_client.rs`.
  - `fn is_ownership_lost(&anyhow::Error) -> bool`, using `downcast_ref`.
  - `struct Fences`: `Mutex<HashMap<String, Fence>>` for held fences, plus `Mutex<HashMap<String, watch::Sender<bool>>>` for lost signals. Methods: `hold(Fence)`, `held(&Scope) -> Option<u64>`, `mark_lost(&Scope)` (sets the signal, logs `tracing::warn!` once per scope), `lost(&Scope) -> watch::Receiver<bool>`. Entries are never removed (research R3).
  - `trait FenceStorage { async fn claim(&self, scope: &Scope, node: &str, lease: Option<vizier_schema::FencingToken>) -> anyhow::Result<Fence>; fn ownership_lost(&self, scope: &Scope) -> watch::Receiver<bool>; }`.
- [X] T005 Add `fn share_lock_suffix(&self) -> &'static str` to the `Dialect` trait in `core/storage/sql/mod.rs`, returning `" FOR SHARE"` in `core/storage/sql/postgres.rs` and `""` in `core/storage/sql/sqlite.rs`. Its doc comment says why: a claim must wait for fenced writes in flight (research R2).
- [X] T006 [P] Add `pub fn postgres_store_pair() -> Option<(SqlStore, SqlStore)>` to `core/tests_support.rs`: two pools on **one** freshly migrated schema, built like `postgres_store()` but with the schema created and migrated once. Also add a helper that builds an `SqlStorage` with its own `Fences` from a store, so a test has two independent "nodes".
- [X] T007 Add `pub(crate) fences: Arc<Fences>` to `SqlStorage` in `core/storage/sqlite/mod.rs`, initialised empty in every constructor (`new`, `with_secrets`, and any test constructor).
- [X] T008 Create `core/storage/sqlite/fence.rs`, with `mod fence;` in `core/storage/sqlite/mod.rs`. It contains:
  - `impl FenceStorage for SqlStorage`, where `claim` is one upsert:
    - **Fleet** (`lease = Some(t)`): `INSERT … VALUES (?scope, ?node, 1, ?t, ?now) ON CONFLICT (scope) DO UPDATE SET token = owner_fence.token + 1, node = excluded.node, lease_token = excluded.lease_token, claimed_at = excluded.claimed_at WHERE owner_fence.lease_token IS NULL OR owner_fence.lease_token < excluded.lease_token RETURNING token`. No row returned → `Err(OwnershipLost)`.
    - **Single node** (`lease = None`): the same upsert without the `WHERE`, writing `lease_token = NULL`.
    - Use `time::stored_now()` for `claimed_at`. On success, call `self.fences.hold(fence)`.
  - `pub(crate) fn check_fence(tx: &Connection, fences: &Fences, scope: &Scope) -> anyhow::Result<()>`. It returns `Ok` at once when `fences.held(scope)` is `None`. Otherwise it runs `SELECT token FROM owner_fence WHERE scope = ?1{share_lock_suffix}` and, when the stored token differs from the held one, calls `fences.mark_lost(scope)` and returns `Err(OwnershipLost)`.
- [X] T009 Add `FenceStorage` to the `VizierStorageProvider` supertrait list in `core/storage/mod.rs`, and delegate `impl FenceStorage for VizierStorage` (the same `self.0.…` pattern as `storage/turn.rs`).
- [X] T010 [P] In `core/storage/sqlite/agent.rs`, make `delete_agent` delete the `owner_fence` row `agent:{id}` in its transaction (data-model.md, "Rules").
- [X] T011 Unit tests in `core/storage/sqlite/fence.rs`, registered with `postgres_tests!`:
  - `claim_is_monotonic_across_instances`: two `SqlStorage` over one store, alternating single-node claims, so each token is strictly greater than the last.
  - `late_claim_refused`: claim with `Some(6)`, then with `Some(5)`, which returns `OwnershipLost` and leaves the token unchanged.
  - `single_node_after_fleet_and_back`: a `None` claim after `Some(9)` succeeds, and a later `Some(1)` succeeds because the stored `lease_token` is null.
  - `check_fence_unfenced_without_hold`: a store that holds no fence passes `check_fence`.

**Checkpoint**: `cargo test -p vizier-core fence` passes on both backends; nothing outside storage changed.

---

## Phase 3: User Story 2 - A former owner cannot write after takeover (Priority: P1)

**Goal**: every write in the [contracts/ownership.md](contracts/ownership.md) "Fenced writes" table is refused, whole, when a newer owner holds the scope (FR-005 – FR-009).

**Independent Test**: owner A claims `agent:x`, B claims it, and each fenced write by A fails with `OwnershipLost` and leaves no row change, while the same writes by B succeed (SC-002). This needs no recovery and no running agent.

### Tests for User Story 2 ⚠️ (write first, see them fail)

- [ ] T012 [P] [US2] Create `core/storage/sqlite/fence_tests.rs`, with `#[cfg(test)] mod fence_tests;` in `core/storage/sqlite/mod.rs`. Write one test per scope row of the contract table, each using `postgres_store_pair()` with A and B where B claimed after A:
  - `stale_history_save_refused`: covers `save_session_history` and `save_checkpoint`.
  - `stale_turn_transition_refused`: covers `start_turn`, `set_turn_request_uid`, `note_resume`, `finish_turn` and `cancel_session_turns`.
  - `stale_client_call_write_refused`: covers open, finish, defer and state save.
  - `stale_sensitive_ask_write_refused`.
  - `stale_background_job_write_refused`: covers open, close piece and transition.
  - `stale_task_run_write_refused`: scope `control`, open and close.

  Each test asserts `is_ownership_lost(&err)`, that A's `ownership_lost` receiver reads `true`, that the row or entry is unchanged when read back, and that B's identical write succeeds.
- [ ] T013 [P] [US2] In the same file, add `claim_waits_for_inflight_write`, which runs on PostgreSQL only. A opens a fenced history write and holds its transaction (a test hook, or a direct `Connection` transaction running `check_fence` then sleeping). B's `claim` must not return until A commits, and A's write must then be visible to a read after the claim (research R2).
- [ ] T014 [P] [US2] In the same file, add `unfenced_writer_unaffected`: a third store holding no fence writes history into `agent:x`'s session after B's claim and succeeds (contract rule 5).

### Implementation for User Story 2

- [ ] T015 [US2] In `core/storage/sqlite/history.rs`, call `check_fence(&tx, &self.fences, &Scope::Agent(session.0.clone()))` first in `save_session_history`, in `save_checkpoint` and in every other history method that inserts or updates entries of a session. A method that writes without a transaction today gets one, using `conn.unchecked_transaction()` as `save_session_history` does.
- [ ] T016 [P] [US2] In `core/storage/sqlite/turn.rs`, wrap `start_turn`, `set_turn_request_uid`, `note_resume`, `finish_turn` and `cancel_session_turns` in a transaction that calls `check_fence` for `agent:{agent_id}`. Read `agent_id` from the row, in the same transaction, for the id-only methods. Leave `accept_turn` unfenced: the sender writes it, and in a Fleet that is a gateway.
- [ ] T017 [P] [US2] In `core/storage/sqlite/client_call.rs`, fence `open_client_call`, `finish_client_call`, `defer_client_call` and `save_client_call_state` for `agent:{agent_id}`, and write `owner_node`/`owner_token` from the held fence in `open_client_call` (`NULL` when none is held). Leave `claim_client_call` and `release_client_call_claim` unfenced: they are the gateway's delivery lease (#163).
- [ ] T018 [P] [US2] In `core/storage/sqlite/question.rs`, fence the sensitive-ask open and close writes for `agent:{agent_id}`, and write the owner columns on open.
- [ ] T019 [P] [US2] In `core/storage/sqlite/background_job.rs`, fence `open_background_job`, `close_background_piece` and `transition_background_job` for `agent:{origin_agent}`, keeping every existing `WHERE state = ?from` guard. Write the owner columns on open. For the id-only methods, read `origin_agent` in the same transaction.
- [ ] T020 [P] [US2] In `core/storage/sqlite/task_run.rs`, fence `open_task_run` and `close_task_run` for `Scope::Control`, and write the owner columns on open.
- [ ] T021 [US2] In `core/agents/steps.rs`, change `record_step` so that on `fence::is_ownership_lost(&e)` it logs at `debug` and returns `None` without the `error!` line. Keep `seal_open_calls` as it is: its saves are fenced, so an ex-owner's seal is refused by T015 (research R4, FR-008).
- [ ] T022 [US2] Add a short doc comment on each fenced method in `core/storage/turn.rs`, `client_call.rs`, `question.rs`, `background_job.rs`, `task_run.rs` and `history.rs` (the trait files): "Fenced: refused with `OwnershipLost` when a newer owner holds `<scope>` (specs/057 contracts/ownership.md)."

**Checkpoint**: T012–T014 pass on PostgreSQL. `cargo test -p vizier-core` passes on SQLite, unchanged: nothing claims yet, so every write is unfenced.

---

## Phase 4: User Story 1 - A turn survives the loss of the node running it (Priority: P1)

**Goal**: when `agent:{id}` is claimed, the agent's expired turns are re-sent and resumed exactly once, its conversations' scratch files and thinking marks are kept or cleared correctly, and an owner that loses the scope stops (FR-010, FR-011, FR-013–FR-016, FR-021).

**Independent Test**: with two owners on PostgreSQL, a multi-step turn under A is resumed by B after B claims, completes once, and a turn of agent `y` under a live owner C is untouched (SC-001). The resume counter carries across owners (SC-005).

### Tests for User Story 1 ⚠️

- [ ] T023 [P] [US1] In `core/storage/sqlite/turn.rs` tests (registered with `postgres_tests!`):
  - `claim_on_receipt_once`: claiming the same row twice with one token returns `true` then `false`.
  - `claim_on_receipt_keeps_resume_count`.
  - `expired_turns_for_agent_only`: rows of `x` under token 1, rows of `y` under C's token; after B claims `x`, only `x`'s rows are selected.
- [ ] T024 [P] [US1] Create `core/recovery_tests.rs` (`#[cfg(test)] mod recovery_tests;` in `core/lib.rs`) with:
  - `takeover_resends_exactly_expired_turns`: uses a test `VizierDependencies` from the existing test helpers, plus a transport whose `resend` is recorded.
  - `resume_limit_across_owners`: A, B and C each resume the same turn without progress, and the third resume past `MAX_RESUMES_WITHOUT_PROGRESS` is given up.
  - `scratch_kept_for_pending_turn_cleared_otherwise`.
  - `racing_recoverers_resend_safely`: B and C both run the turn step on 1,000 queued rows; every row is received and claimed exactly once (SC-003, turn half).
- [ ] T025 [P] [US1] In `crates/vizier-core/tests/agent_host.rs`, add `ownership_lost_releases_agent`. Start an `AgentHost` holding `x`, make another store claim `agent:x`, then trigger a fenced write from the host. The agent's inbox is unregistered and its instance stopped, and no reply content is sent on the requester's stream (FR-008).

### Implementation for User Story 1

- [ ] T026 [US1] In `core/storage/turn.rs` (trait + `VizierStorage` delegate) and `core/storage/sqlite/turn.rs`:
  - Add `claim_turn_on_receipt(&self, id: i64) -> Result<bool>`: `UPDATE session_turn SET owner_token = ?mine, owner_node = ?node WHERE id = ? AND (owner_token IS NULL OR owner_token < ?mine)`, fenced, with `?mine` taken from the held `agent:{agent_id}` fence. It returns `true` when no fence is held (single process before any claim).
  - Change `pending_turns(agent_id)` to also take `below: u64` and select only rows with `owner_token IS NULL OR owner_token < below`.
  - Make `sessions_with_pending_turns` take `agent_id: Option<&str>`.
- [ ] T027 [US1] In `core/storage/scratch.rs` / `core/storage/sqlite/scratch.rs` and `core/storage/session.rs` / `core/storage/sqlite/session.rs`, add an `agent_id: &str` parameter to `clear_scratch_except` and `reset_thinking_except`, limiting both to that agent's rows (FR-021).
- [ ] T028 [US1] Create `core/recovery.rs` (`pub mod recovery;` in `core/lib.rs`) with `pub struct Recovered { pub complete: bool }` and `pub async fn recover_agent(deps: &VizierDependencies, agent_id: &AgentId, fence: &Fence) -> Recovered`. Implement steps 3 and 5 of [contracts/recovery.md](contracts/recovery.md):
  - **Scratch and thinking**: reset thinking marks and clear scratch of the agent's sessions with no pending turn, using T027 and `sessions_with_pending_turns(Some(agent_id))`.
  - **Turns**: re-send the agent's `pending_turns(agent_id, fence.token)` in id order. Move the body of `AgentHost::resend_pending_turns` here (same `VizierRequestEnvelope`, `broadcast_sender`, `resumed` when `running`).
  - A failed step sets `complete = false` and logs `warn!`.
  - Leave numbered placeholders, as comments naming the task that fills them: T045 for steps 1 and 2, T037 for step 4.
- [ ] T029 [US1] In `core/agents/process.rs`, where an envelope with a `turn_id` arrives at `agent_process` (before it is queued or spawned), call `storage.claim_turn_on_receipt(id)`. On `Ok(false)` drop the envelope with a `debug!` ("duplicate delivery") and send nothing on its `response_tx`. On `Err(e)` with `is_ownership_lost(&e)`, drop it too; the release (T031) follows.
- [ ] T030 [US1] In `core/agents/mod.rs`, change `AgentHost::take`:
  - After `register_agent` and `subscribe_scoped`, call `self.deps.storage.claim(&Scope::Agent(id), node, None)`. The node is the id `SingleNodePlacement` already carries; pass it into `AgentHost::new` from `runtime.rs`.
  - Then call `recovery::recover_agent` in place of `resend_pending_turns`, and delete that method.
  - When `Recovered.complete` is false, spawn a retry loop with the supervisor's backoff (2 s doubling to 5 min) that calls `recover_agent` again until it is complete, and stops if the agent is released.
  - A failed `claim` logs an `error!` and does not start the agent.
- [ ] T031 [US1] In `core/agents/mod.rs`, for every held agent, select on `storage.ownership_lost(&Scope::Agent(id))` in `AgentHost::run`'s loop (a per-agent watch merged into the existing `select!`, as `scoped_rx` is). When it fires, call `self.release(&id)` and log `warn!(agent_id, "ownership moved to another node; released")` (contract rule 6).
- [ ] T032 [US1] In `core/dependencies.rs`, delete `clear_orphaned_scratch` and its call: it now runs per agent inside `recover_agent`.

**Checkpoint**: T023–T025 pass. A single-node `just run` restart resumes a mid-turn conversation as before (quickstart §2).

---

## Phase 5: User Story 3 - Scheduled runs and background jobs are recovered once when their owner changes (Priority: P2)

**Goal**: task runs recover on the `control` claim, and background jobs on their origin agent's claim. Records a pending turn backs are picked up; every other one is interrupted with exactly one notification (FR-009, FR-012).

**Independent Test**: an open run under control owner A and an open job under worker owner A; B takes over. Unbacked records become interrupted with one notification each, backed ones are picked up, and records of live owners are untouched (SC-004).

### Tests for User Story 3 ⚠️

- [ ] T033 [P] [US3] In `core/recovery_tests.rs`, add:
  - `control_takeover_interrupts_unbacked_runs_once`.
  - `control_takeover_keeps_backed_runs`.
  - `agent_takeover_interrupts_unbacked_jobs_once`.
  - `agent_takeover_picks_up_backed_jobs`: a `running` job with a piece's turn pending, and a `reporting` job whose report turn is pending, which becomes `reported`.
  - `jobs_of_live_origin_untouched`.
  - `racing_job_transition_one_winner`: two owners race `running → reporting` against an interrupt; exactly one wins.

  Each test counts the notification rows created.

### Implementation for User Story 3

- [ ] T034 [P] [US3] In `core/storage/task_run.rs` / `core/storage/sqlite/task_run.rs`:
  - Change `open_task_runs` to take `below: u64` (rows with `owner_token IS NULL OR owner_token < below`).
  - Change `interrupt_open_task_runs_except(keep)` to also take `below` and set `owner_token`/`owner_node` from the held control fence in the same fenced `UPDATE`.
  - Add `reown_task_runs(ids: &[i64])` for the kept ones.
- [ ] T035 [P] [US3] In `core/storage/background_job.rs` / `core/storage/sqlite/background_job.rs`:
  - Add an `origin: &str` and a `below: u64` parameter to `open_background_jobs` and `interrupt_open_background_jobs_except`, with re-owning in the same fenced statement.
  - Add `reown_background_jobs(ids)`.
- [ ] T036 [US3] In `core/agents/background/mod.rs`, change `BackgroundJobs::recover()` to `recover_origin(&self, origin: &AgentId, below: u64)`. It only acts on that origin's open jobs, re-owns the kept ones before restarting `run_job` or marking `reported`, and keeps the rest of its logic unchanged.
- [ ] T037 [US3] In `core/recovery.rs`, fill step 4 of `recover_agent`. Move `background_jobs_backed_by_turns` from `core/dependencies.rs`, narrowed to the origin agent, then:
  - interrupt the unbacked jobs (T035);
  - send one `notifications.job_ended(…, JobState::Interrupted, …)` per interrupted job (the job half of `record_interrupted`);
  - call `deps.background_jobs.recover_origin(agent_id, fence.token)`.
- [ ] T038 [US3] In `core/recovery.rs`, add `pub async fn recover_control(deps, fence) -> Recovered`. Move the `interrupt_orphaned_task_runs` logic from `core/dependencies.rs` (keep runs whose session has a pending turn, interrupt the rest with `below = fence.token`). Re-own the kept runs, and send one `task_run_ended(…, Interrupted, …)` per interrupted run (the run half of `record_interrupted`, including the task-title lookup).
- [ ] T039 [US3] In `core/scheduler/mod.rs`, make `VizierScheduler::recover` read `open_task_runs(below)` with the token from `recover_control`'s fence. Pass the fence in, or store it on the scheduler when `VizierScheduler::new` is given it.
- [ ] T040 [US3] In `core/runtime.rs`:
  - Before `VizierScheduler::new`/`run` (only when the scheduler role is spawned), call `deps.storage.claim(&Scope::Control, node, None)`, then `recovery::recover_control`. Retry an incomplete one as in T030, and refuse to spawn the scheduler if the claim fails.
  - Remove the `deps.background_jobs.recover().await` call after `AgentHost::new`: it now runs per agent (T037).
- [ ] T041 [US3] In `core/dependencies.rs`, delete `interrupt_orphaned_task_runs`, `interrupt_orphaned_background_jobs`, `background_jobs_backed_by_turns`, `record_interrupted`, their call sites, and the `interrupted_runs`/`interrupted_jobs` locals.

**Checkpoint**: T033 passes on both backends; quickstart §3 shows the same picked-up and interrupted outcomes as before, with one notification each.

---

## Phase 6: User Story 4 - Pending client tool calls follow the owner of their turn (Priority: P2)

**Goal**: on an agent claim, its pending client calls become interrupted and its open sensitive asks are abandoned; deferred calls survive and are still delivered once (contracts/recovery.md steps 1–2).

**Independent Test**: a pending and a deferred client call and an open sensitive ask under owner A; B claims `agent:x`. The pending call is interrupted, the ask is abandoned, and the deferred call is still delivered to exactly one tab. Calls of agent `y` are untouched.

### Tests for User Story 4 ⚠️

- [ ] T042 [P] [US4] In `core/recovery_tests.rs`, add:
  - `agent_takeover_interrupts_pending_client_calls_only`.
  - `deferred_client_call_survives_takeover`: after B's claim, `claim_client_call` by one claimant succeeds and a second fails.
  - `agent_takeover_abandons_open_asks`.
  - `client_calls_of_other_agent_untouched`.

### Implementation for User Story 4

- [ ] T043 [P] [US4] In `core/storage/client_call.rs` / `core/storage/sqlite/client_call.rs`, change `interrupt_pending_client_calls()` to `interrupt_pending_client_calls(agent_id: &str, below: u64)`: `status = 'pending' AND agent_id = ? AND (owner_token IS NULL OR owner_token < ?)`, re-owned in the same fenced `UPDATE`.
- [ ] T044 [P] [US4] In `core/storage/question.rs` / `core/storage/sqlite/question.rs`, add `abandon_open_sensitive_asks(agent_id, below) -> Result<usize>`, a single fenced `UPDATE` that re-owns. In `core/questions/sensitive.rs`, replace `abandon_open_asks(storage)` with a per-agent version that calls it.
- [ ] T045 [US4] In `core/recovery.rs`, fill steps 1 and 2 of `recover_agent` with T043 and T044, logging counts as the old sweeps did.
- [ ] T046 [US4] In `core/dependencies.rs`, delete `interrupt_orphaned_client_calls`, the `abandon_open_asks` call and its comment.

**Checkpoint**: T042 passes; `VizierDependencies::new` no longer runs any in-flight sweep. Check with `rg "interrupt_orphaned|abandon_open_asks|clear_orphaned_scratch" crates/vizier-core/src/dependencies.rs`, which must print nothing.

---

## Phase 7: User Story 5 - A single node behaves exactly as today (Priority: P1)

**Goal**: one process, one start, the same outcomes for every in-flight record kind; never a refused write (FR-017, FR-018, FR-019, SC-006, SC-007).

**Independent Test**: the existing suite passes unchanged on SQLite and PostgreSQL. A restarted single node with a running turn, an open run, an open job, a pending client call and an open ask produces exactly the outcome the old sweeps produced. The dummyplug quickstart passes.

### Tests for User Story 5 ⚠️

- [ ] T047 [P] [US5] In `core/recovery_tests.rs`, add `single_node_restart_outcome_unchanged` (registered with `postgres_tests!`). Seed one of each in-flight record kind with `owner_token` set by a first process's claim, start a second test "process" (a new `SqlStorage` with fresh `Fences` on the same store), and claim every agent and control with `None`. Assert the per-kind outcome against a table written from today's sweep semantics: turn re-sent `resumed`; backed run and job picked up; unbacked ones interrupted with one notification each; pending call interrupted; deferred call kept; ask abandoned; scratch kept only for sessions with a turn.
- [ ] T048 [P] [US5] In `core/recovery_tests.rs`, add `legacy_rows_recovered_once`. Rows with a null `owner_token` are recovered by the first claim and not by a second claim of the same process.
- [ ] T049 [P] [US5] In `core/recovery_tests.rs`, add `single_node_never_refused`. Run a full turn's writes (history, turn transitions, a client call, a job) after a `None` claim; none returns `OwnershipLost`.

### Implementation for User Story 5

- [ ] T050 [US5] Read `core/runtime.rs` and `core/agents/mod.rs` end to end and confirm the startup order. In a single node: `VizierDependencies::new` (no sweeps), then `AgentHost::new` (per agent: claim, then `recover_agent`), then the control claim and `recover_control`, then the scheduler's `recover`, then `run`. Fix anything that makes a recovered job or run start before the pending turns it waits on have been re-sent, or say why it is safe in a code comment at the call site (research R5, "Ordering checked against today").
- [ ] T051 [US5] Run quickstart §2–§4 against `just run` with a `dummyplug` agent and record the outcomes (pass or fail, with log excerpts) in a "Verification" section appended to `specs/057-fleet-row-ownership/quickstart.md`.
- [ ] T052 [US5] Time 200 dummyplug turns of five tool calls each on SQLite, before and after the branch, using `scripts/` if a harness exists, otherwise a short loop over the HTTP API. Record both numbers in the same Verification section (SC-007).

**Checkpoint**: `cargo test --workspace` and `cargo clippy --workspace` pass; the `managed-storage` job passes on the PR.

---

## Phase 8: User Story 6 - The operator knows where other single-process assumptions remain (Priority: P3)

**Goal**: a complete audit with a disposition for every entry (FR-020, SC-008).

**Independent Test**: `specs/057-fleet-row-ownership/audit.md` covers the four required areas, every entry has a disposition, and every follow-up names a real issue.

- [ ] T053 [US6] Search for anything [audit.md](audit.md) missed:
  - `rg "Mutex<Hash|static .*Lazy|OnceLock" crates/vizier-core/src`;
  - every `migration.` marker;
  - every startup-time `DELETE`/`UPDATE` left in `crates/vizier-core/src/dependencies.rs`.

  Add a row for each new finding, with its disposition.
- [ ] T054 [US6] Open the two follow-up issues the audit names (cross-node cancel of a background job; one-time migrations run by every node), after confirming their titles and bodies with the user. Use `gh issue create --repo vizier-lab/vizier`, label `enhancement`, referencing #159 and #164. Replace both `#TBD` in `specs/057-fleet-row-ownership/audit.md` with the issue numbers.

---

## Phase 9: Polish & cross-cutting concerns

- [ ] T055 [P] Create `docs/src/fleet/ownership-and-recovery.md`, explaining ownership scopes, the fence, what is refused, and what happens on takeover and in single node, in the voice of `docs/src/fleet/bus-contract.md`. Add it to `docs/src/SUMMARY.md` under the Fleet section.
- [ ] T056 [P] Update `CLAUDE.md`. In **Durable turns**, replace "At startup `AgentHost::resend_pending_turns` re-sends …" with the claim-and-`recover_agent` description. Apply the same change to the startup-sweep sentences under **Background jobs** (storage), **Scheduler/Task runs**, **Scratchpad** and **Client tools**. Add a short **Row ownership** paragraph under the Fleet bus text: `owner_fence`, `FenceStorage::claim`, fenced writes, `recovery.rs`.
- [ ] T057 [P] Update the "Durable turns" and startup-recovery sentences in `specs/023-persist-turn-activity/` only if they are referenced as current behaviour by `docs/src/`. Otherwise leave the old specs as history.
- [ ] T058 Run `cargo test --workspace`, `cargo clippy --workspace`, `cargo test -p vizier-core --no-default-features`, and the PostgreSQL run from quickstart §1. Paste the results into the PR description, with the delivery-order PR it belongs to.

---

## Dependencies & execution order

### Phase dependencies

- **Setup (1)** → **Foundational (2)** → **US2 (3)** → **US1 (4)** → **US3 (5)** and **US4 (6)** (independent of each other) → **US5 (7)** → **US6 (8)** → **Polish (9)**.
- **US2 before US1**: recovery is only exactly-once because fenced writes refuse the older recoverer (research R5).
- **US5 last among the code phases**: it verifies the whole replacement of the startup sweeps, which US1, US3 and US4 each remove a part of.
- **US6** depends only on the code being in place (T053 searches it). It can start any time after Phase 4 if someone is free.

### Within each story

Tests first and failing, then storage methods, then `recovery.rs`, then the callers (`agents/`, `runtime.rs`), then removal from `dependencies.rs`.

### Shared files to watch (no [P] across them)

`core/recovery.rs` (T028, T037, T038, T045), `core/dependencies.rs` (T032, T041, T046), `core/agents/mod.rs` (T030, T031), `core/recovery_tests.rs` (T024, T033, T042, T047–T049: write in sequence or in separate `mod` blocks).

## Parallel opportunities

- **Phase 1**: T002 and T003 alongside T001.
- **Phase 2**: T006 and T010 alongside T004–T005; T008 after T004, T005 and T007.
- **Phase 3**: T012, T013 and T014 together; then T016–T020 together (one file each), with T015 on its own (history is the biggest).
- **Phase 4**: T023, T024 and T025 together.
- **Phase 5 / Phase 6**: T034 ∥ T035 ∥ T043 ∥ T044 (four storage files), once Phase 4 is in.
- **Phase 9**: T055 ∥ T056 ∥ T057.

### Example: Phase 3 in parallel

```text
Task: "T016 fence session_turn transitions in core/storage/sqlite/turn.rs"
Task: "T017 fence client_tool_call writes in core/storage/sqlite/client_call.rs"
Task: "T018 fence sensitive_ask writes in core/storage/sqlite/question.rs"
Task: "T019 fence background_job writes in core/storage/sqlite/background_job.rs"
Task: "T020 fence task_run writes in core/storage/sqlite/task_run.rs"
```

## Implementation strategy

### MVP

Phases 1–4 (Setup, Foundational, US2, US1): a turn moves between owners exactly once, and a stale owner cannot write. That is what makes more than one worker possible at all. Stop and validate with T012–T014 and T023–T025 on PostgreSQL and quickstart §2 on SQLite.

### Incremental delivery

1. **PR 1** = Phases 1–2: schema and the primitive. Inert.
2. **PR 2** = Phase 3: fenced writes. Inert in single node, because nothing claims yet.
3. **PR 3** = Phases 4–7: claims and recovery replace the sweeps. This is the behaviour change, verified by US5.
4. **PR 4** = Phases 8–9: audit, follow-up issues, docs.

Rebase each PR on `master` before opening it: #160–#163 touch `transport/mod.rs`, `agents/mod.rs`, `agents/process.rs`, `dependencies.rs` and `runtime.rs` too.
