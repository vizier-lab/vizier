# Research: Fleet row ownership, fencing and lease-based recovery

**Feature**: `057-fleet-row-ownership` | **Spec**: [spec.md](spec.md) | **Issue**: #164 (epic #159, depends on #162)

What the code does today, read before deciding anything:

- `VizierDependencies::new` (`crates/vizier-core/src/dependencies.rs`, around line 217) runs **five sweeps** at construction, on every node, before anything else starts: `interrupt_orphaned_task_runs`, `interrupt_orphaned_background_jobs`, `interrupt_orphaned_client_calls`, `questions::sensitive::abandon_open_asks`, and `clear_orphaned_scratch` (thinking marks plus scratch files). Runs and jobs that a pending `session_turn` row still backs are **kept** by the sweep and **re-attached** later: `VizierScheduler::recover` (`scheduler/mod.rs:57`) and `BackgroundJobs::recover` (`agents/background/mod.rs:404`, called from `runtime.rs:197` after `AgentHost::new`). Both re-attach by watching session events (`transport.await_session_outcome`), which already travel the bus, not by holding anything in the old process.
- `AgentHost::take` (`agents/mod.rs:168`) registers the agent's inbox, then `resend_pending_turns` re-sends every `session_turn` row of the agent, `resumed` when its state is `running`.
- `session_turn` rows are written by the **sender** (`VizierTransport::send_request`, `transport/mod.rs:482`, so a gateway in a Fleet), marked running by the agent (`process.rs:946`), and deleted when the turn ends (`process.rs:283`). `resume_count`/`steps_at_resume` **already live in the row** (`note_resume`, `storage/sqlite/turn.rs:110`).
- `FencingToken` and `Leases` exist in `vizier-schema` (#162). The in-process backend issues tokens from a counter that **starts at 1 in every process**, so in-process tokens are not comparable across restarts.
- Nobody takes `lease.agent.{id}` or `lease.leader` yet; the worker and control roles that will (#166, #168) are skeletons. `SingleNodePlacement` assigns every stored agent to one node; `runtime::Gate` (`AlwaysYes` by default) gates the scheduler.
- Every storage trait has one implementation over the backend-neutral `Connection` (`storage/sql/mod.rs`). `transaction()` opens `BEGIN IMMEDIATE` on SQLite; PostgreSQL runs READ COMMITTED. Backend differences are `Dialect` fragments (`key_lock_sql` is `pg_advisory_xact_lock` on PostgreSQL, none on SQLite).

---

## R1. The fencing token written on rows is a database epoch, guarded by the lease token

**Decision**: A new table `owner_fence (scope PK, node, token, lease_token, claimed_at)` holds one row per **ownership scope** (`agent:{id}`, `control`). Taking ownership is one statement, `claim(scope, node, lease_token)`:

- **Fleet** (`lease_token = Some(t)`, the token of the bus lease just acquired): upsert that sets `token = token + 1`, `node`, `lease_token = t`, **only where** the stored `lease_token` is null or smaller than `t`. No row updated means a later lease holder has already claimed: the claim is refused (`OwnershipLost`).
- **Single node** (`lease_token = None`): an unconditional `token = token + 1` with `lease_token = NULL`. The workspace lock (`.runtime/vizier.lock`) already guarantees one process.

The returned `token` is the **fence**, the number every row and every fenced write carries. It is strictly increasing per scope across process restarts, backends and modes, because the database assigns it.

**Rationale**: Comparing raw lease tokens across processes breaks in single node (the in-process counter restarts at 1, so a restarted node would look *older* than its previous run). Letting the database number the epochs fixes that, and guarding the bump with the lease token keeps the one race that matters closed: an ex-holder whose lease expired *before it claimed* cannot claim after the new holder, because its lease token is smaller. Switching a deployment from single node to Fleet works (a stored null `lease_token` is overtaken by any lease), and two single-node processes wrongly pointed at one PostgreSQL now fence each other instead of corrupting each other's turns (each claim refuses the other's writes).

**Alternatives considered**:
- *Write the bus lease token on rows directly.* Rejected: not comparable across in-process restarts (above).
- *Seed the in-process lease counter from storage at startup.* Rejected: it changes `vizier-schema` to depend on storage state, and still leaves the single-to-Fleet switch comparing two unrelated counters.
- *Unguarded database epoch* (`token + 1` for every claimant). Rejected: a paused ex-holder that claims late would become the newest owner and fence out the real one.

## R2. Fenced writes check the fence row in the write's own transaction, under a shared lock

**Decision**: A fenced write, inside the transaction it already opens, reads `SELECT token FROM owner_fence WHERE scope = ?` with a dialect suffix, `FOR SHARE` on PostgreSQL and nothing on SQLite (its `BEGIN IMMEDIATE` already serialises writers). If the stored token is not the writer's token, the transaction is rolled back and the write fails with `OwnershipLost`. A claim updates the fence row, so it takes the row lock and **waits for every in-flight fenced write of that scope to commit**. No fenced write can land after the claim that superseded it, and recovery (which runs after the claim) sees every write that did land.

**Rationale**: FR-006 asks that the check and the write be indivisible. On READ COMMITTED a plain `SELECT` or an `INSERT … WHERE EXISTS` reads a snapshot from the statement's start, so a claim committed between the check and our commit would not stop the write. `FOR SHARE` lets writers of one scope (parallel sessions of one agent) run together, while still excluding a claim.

**Alternatives considered**:
- *`lock_key` (advisory exclusive lock per scope).* Rejected: it serialises every write of an agent, including unrelated sessions running in parallel.
- *A row predicate on the written row only* (`… AND owner_token = ?`). Rejected as the sole check: history inserts have no row to predicate on, and it misses a claim that has not yet touched the row.

## R3. The writer's token comes from a process-local fence registry at the storage boundary

**Decision**: `SqlStorage` holds an `Arc<Fences>`: a process-local map `scope → Fence { token, node }` filled by `claim`. A fenced write looks up its scope in the registry. **If this process holds a fence for the scope, the write is checked against it; if it holds none, the write is unfenced** (a gateway writing a person's question answer, a workspace post into a member agent's session, a migration). Entries are never removed while the process runs: a claim by another process makes the stored token newer, so the stale entry refuses every later write of that scope, which is exactly what fencing must do.

Scope of a write is derived from data the write already has: `session.0` (the agent id) for history, turns, client calls and sensitive asks; the job's `origin` agent for background jobs and pieces; `control` for task runs.

**Rationale**: About 40 call sites save history (`save_session_history`, `save_with_busy_retry`, `record_step`, …) and the turn, job, run and client-call writers add more. Threading a `Fence` argument through every storage trait method and every caller is a large, error-prone diff across the core-touching files the issue warns about (`transport.rs`, `agents/process.rs`, `agents/mod.rs`). A boundary concern implemented once at the storage layer is how vizier already handles encryption at rest (`SqlStorage::with_secrets`), so this follows an existing pattern (Principle II).

**Alternatives considered**:
- *An explicit `&Fence` parameter on every write.* Rejected for the diff size and churn above. It would also make "forgot to pass it" a silent unfenced write, which is no safer than the registry.
- *A tokio task-local.* Rejected: turns spawn tasks (`tokio::spawn` in usage recording, background runners) that do not inherit task-locals, so writes would silently fall out of the fence.
- *Fence every write of an agent's sessions, even from non-owners.* Rejected: gateways legitimately write into agent sessions and never hold a fence.

## R4. "Ownership lost" is a distinct error type, and it stops the owner, not just the write

**Decision**: `storage::fence::OwnershipLost { scope }`, returned inside the storage traits' `anyhow::Error` and recognised with `downcast_ref` (helper `fence::is_ownership_lost(&err)`). When the registry sees a refusal it also flips a per-scope **lost** signal (`Fences::lost(scope) -> watch::Receiver<bool>`). `AgentHost` watches the signal for every agent it holds and **releases** the agent (unregister the inbox, stop the instance, abort its turns) exactly as `release` does today. The same path is what #166 will call when `renew` returns `LeaseLost` (lease rule 11).

The ex-owner's own clean-up writes on stop (`seal_open_calls`, `finish_turn`, `close_task_run`, the background job transition) are fenced too, so they are refused. FR-008 ("no reply, no seal, no finish") is therefore enforced by the fence rather than by new branches in the stop path. The requester's reply stream ends without a final answer; the person's page receives the new owner's frames through session events, as a restart already does.

`record_step` keeps its "a failed save never fails the turn" rule for ordinary errors, but returns early on `OwnershipLost`, because the release is already under way.

**Rationale**: One refusal anywhere is proof the whole scope is gone (tokens are per scope), so stopping the agent once is both simpler and more correct than handling the error at each of the ~40 call sites.

**Alternatives considered**: *A new `ErrorKind::OwnershipLost` on `VizierError`.* Deferred: storage traits return `anyhow::Result`, and nothing outside core needs to match on it yet. Revisit when a gateway does (#169).

## R5. Recovery is two routines, run on claim, replacing the five startup sweeps

**Decision**: A new module `crates/vizier-core/src/recovery.rs` with two functions:

- `recover_agent(deps, agent_id, fence)`, called by `AgentHost::take` **after** the claim and **before** the instance starts (where `resend_pending_turns` runs today). In order: (1) sweep the agent's `pending` client calls and abandon its open sensitive asks; (2) reset thinking marks and clear scratch files of the agent's sessions that have no pending turn; (3) resolve the agent's open background jobs (origin = this agent): pick up the ones a pending turn backs (today's `BackgroundJobs::recover`, narrowed to one origin), interrupt and notify the rest; (4) re-send the agent's pending turns, `resumed` when running (today's `resend_pending_turns`).
- `recover_control(deps, fence)`, called by the runtime when it takes `control` (in single node, at start, before `VizierScheduler::run`'s `recover`): interrupt and notify the open task runs no pending turn backs, then let `VizierScheduler::recover` re-attach the rest, unchanged.

Every step selects **only rows whose `owner_token` is null or smaller than the new fence**, and moves each selected row to the new fence in the same fenced statement that resolves it (`UPDATE … SET owner_token = ?new … WHERE … AND (owner_token IS NULL OR owner_token < ?new)`). Rows created by the new owner after recovery started carry the new token and are never touched, and two recoverers racing for one scope cannot both win: the fence check refuses the older one (R2).

The five sweeps and `record_interrupted` leave `VizierDependencies::new`. Turn rows are the exception to "move to the new fence": recovery re-sends them without claiming, and the receiving owner claims them (R6).

**Rationale**: Today's single-node startup *is* a claim of every agent and of control by a new process. Running the same routine at the same points makes single node behave identically (FR-018), while in a Fleet the routine runs only for the scopes a node actually takes (FR-010, FR-014). Background jobs and client calls move to agent scope, because that is where the process that awaits them lives (spec FR-002).

**Ordering checked against today**: the kept-or-interrupt decision for runs and jobs reads pending `session_turn` rows, not re-sent requests, so doing it per agent after the claim gives the same answer as doing it once at construction. A job's pieces run in other agents' sessions; the re-attached runner checks history first, then waits on session events (`await_session_outcome`), so it does not depend on the order in which agents are taken.

**Alternatives considered**: *Keep the sweeps in `dependencies.rs` and filter them by owner.* Rejected: `dependencies.rs` runs on every role, including gateways that own nothing, and before any lease is taken, so it cannot know which scopes are this node's.

## R6. Turn rows are claimed on receipt, which also drops duplicate deliveries

**Decision**: `session_turn` gains `owner_node`/`owner_token`, null while only accepted. The agent claims the row **when the request reaches its inbox** (`agent_process`, before queueing): `UPDATE session_turn SET owner_token = ?mine, owner_node = ? WHERE id = ? AND (owner_token IS NULL OR owner_token < ?mine)` (fenced). Zero rows updated means another copy was already claimed by this owner, so the delivery is a duplicate and is dropped without a reply. Every later transition of the turn (`start_turn`, `set_turn_request_uid`, `note_resume`, `finish_turn`) is fenced.

**Rationale**: In a Fleet, a queued row may be both re-sent by recovery and still in flight from the gateway, or re-sent by two racing recoverers. A claim-on-receipt compare-and-swap makes re-sending always safe and turn execution exactly-once per owner. In single node a request is delivered once, so the claim always succeeds and nothing changes.

## R7. The resume counter already survives a node change; keep it there

**Decision**: No schema change for `resume_count`/`steps_at_resume`. The claim on receipt (R6) must not reset them, and `note_resume` becomes fenced. A test pins that the count carries across two owners and that `exceeds_resume_limit` trips after `MAX_RESUMES_WITHOUT_PROGRESS` resumes in total, not per node (SC-005).

## R8. Exactly-once transitions keep their compare-and-swap and gain the fence

**Decision**: `transition_background_job` (`running → reporting` and the others), `close_background_piece`, `close_task_run`, `finish_client_call` and `defer_client_call` keep their guarded `WHERE state = ?from` predicates and are additionally fenced through the registry (R3). No new SQL shape: the fence read is added to the transaction each already runs.

## R9. Rows from before this feature

**Decision**: Rows with a null `owner_token` are treated as owned by an expired owner (spec FR-004). Every recovery step's predicate already selects them, so the first claim of their scope after upgrade recovers them exactly once, and moves them to the new token. No data move is needed. The null-means-legacy reading is still a compatibility rule, so it gets an entry in #82 (FR-022).

## R10. Schema changes, both backends

**Decision**:
- **PostgreSQL**: `storage/sql/postgres/migrations/0007_row_ownership.sql` creates `owner_fence` and adds `owner_node TEXT COLLATE "C"`, `owner_token BIGINT` (nullable) to `session_turn`, `task_run`, `background_job`, `client_tool_call` and `sensitive_ask`, plus partial indexes on `(agent_id, owner_token)` where the recovery predicates scan.
- **SQLite**: `CREATE TABLE IF NOT EXISTS owner_fence` in the schema init, and `add_column_if_missing` for the same columns (the existing idempotent pattern). `add_column_if_missing` calls are compatibility code, so they are listed in #82.

## R11. Testing two owners

**Decision**: Two `SqlStorage` instances over **two pools on the same PostgreSQL schema**, each with its own `Fences` registry, stand in for two nodes. `tests_support` gains `postgres_store_pair()` (today `postgres_store()` makes a fresh schema per call). The tests are storage-level and recovery-level (no bus needed: the claim takes the lease token as a plain argument). SQLite runs the same tests where they make sense for one process (claim bump, legacy rows, single-node recovery outcome), using the existing `postgres_tests!` macro to run each test on both backends.

Tests to write (each maps to a spec criterion):
- `stale_token_write_refused` — A claims, B claims, A's history save, turn start, note_resume, job transition and task-run close are each refused and leave nothing behind (SC-002).
- `late_claim_refused` — A acquires lease token 5, B acquires 6 and claims, A's claim with 5 is refused (R1).
- `takeover_recovers_exactly_expired_rows` — rows of agents X (owner A, lost) and Y (owner C, live); B takes X; only X's rows change (SC-001).
- `racing_recoverers_once` — 1,000 open rows, B and C recover concurrently; every row is resolved exactly once (SC-003).
- `claim_on_receipt_drops_duplicates` — the same turn delivered twice runs once (R6).
- `resume_limit_across_owners` (SC-005), `legacy_rows_recovered_once` (R9), `single_node_restart_outcome_unchanged` (SC-006: compare each record kind with today's sweep result).

## R12. Single-process assumptions on the shared-database path

**Decision**: The audit is a deliverable of this feature (FR-020) and lives in [audit.md](audit.md). Research found nine assumptions: three are fixed here (the startup sweeps, scratch clearing, sensitive asks), four are safe as they are, and two need follow-up issues (cancelling a background job from another node, and one-time migrations run by every node at startup).

---

All unknowns from the Technical Context are resolved above; nothing is left as NEEDS CLARIFICATION.
