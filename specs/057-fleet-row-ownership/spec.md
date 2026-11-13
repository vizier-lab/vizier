# Feature Specification: Fleet row ownership, fencing and lease-based recovery

**Feature Branch**: `057-fleet-row-ownership`

**Created**: 2026-11-13

**Status**: Draft

**Input**: User description: "https://github.com/vizier-lab/vizier/issues/164 — let's tackle this" (Fleet: row ownership, fencing and lease-based recovery; part of the Fleet epic #159, depends on #162)

## Context

Today, when vizier starts, it assumes it is the only process that has ever touched its data. Everything a previous process left in flight is treated as abandoned: running turns are resumed, open task runs and background jobs are marked interrupted, pending client tool calls are swept, and leftover scratch files are cleared. That is correct for one process. In a Fleet, another node may be healthy and in the middle of those very turns, runs and jobs. A node starting up must not take them away.

This feature makes recovery depend on **who owns a piece of in-flight work and whether that owner's lease has run out**, not on a node starting. It also makes sure that an owner which lost its lease (for example because it was paused and came back) cannot keep writing over the work of the owner that took over.

Positions in conversation history are already safe for many concurrent writers (#157). The lease and fencing-token primitives exist in the Fleet bus contract (#162). This feature is what uses them for the in-flight records.

## User Scenarios & Testing *(mandatory)*

The "users" here are the operator running a Fleet, and the people whose conversations, scheduled tasks and background jobs run on it. Their experience is the test: work survives a node being lost, and nothing is duplicated, lost twice, or corrupted.

### User Story 1 - A turn survives the loss of the node running it (Priority: P1)

A person is chatting with an agent. Mid-turn, the worker node running that agent stops responding (crash, network partition, machine lost). Another worker takes over the agent. The person's turn continues on the new worker from its last saved step, and the reply reaches them. Other agents' turns, on nodes that are still healthy, are not touched.

**Why this priority**: This is the core promise of Fleet: losing a machine does not lose a conversation. Without it, Fleet is not usable with more than one worker.

**Independent Test**: With two simulated worker owners on the shared database, start a multi-step turn on owner A, let A's agent lease expire, have owner B take the agent over, and check that the turn resumes on B from its last step and completes exactly once, while a turn owned by a still-healthy owner C is left alone.

**Acceptance Scenarios**:

1. **Given** a turn is running on worker A with several steps saved, **When** A's lease on that agent expires and worker B takes the agent over, **Then** B resumes the turn from its last saved step, marks the reply as resumed, and the person receives it.
2. **Given** workers A and C each own different agents and both are healthy, **When** worker B starts, **Then** B recovers nothing that belongs to A or C.
3. **Given** a turn queued but not yet started on A, **When** B takes the agent over, **Then** B runs it in its original order behind any turn that was already running.
4. **Given** a turn that has already been resumed the maximum number of times without progress, **When** it is taken over by another node, **Then** the count carries over and the turn is given up rather than resumed again, so a turn that keeps killing nodes cannot cycle through the whole Fleet.

---

### User Story 2 - A former owner cannot write after takeover (Priority: P1)

Worker A is paused (a long garbage-collection pause, a frozen VM, a network split) long enough for its lease to expire. Worker B takes the agent over and resumes the turn. Then A wakes up and, not yet knowing it lost the lease, tries to save the next step of the same turn. That write is refused, and A stops working on the turn. The conversation contains only B's steps after the takeover.

**Why this priority**: Without this, takeover produces interleaved or duplicated steps, double replies, and corrupted history: worse than losing the turn. It is as essential as Story 1.

**Independent Test**: Have owner A hold a turn at ownership token N, have owner B take over (token N+1), then attempt a step save, a history save and a turn state change as A with token N. Each is refused, nothing A attempted is visible, and B's subsequent writes succeed.

**Acceptance Scenarios**:

1. **Given** B has taken over an agent from A, **When** A tries to save a step, a history entry or a change of turn state for that agent's work, **Then** the write is refused as a whole and no part of it is stored.
2. **Given** a write by A is refused because ownership moved, **When** A learns of the refusal, **Then** A stops the affected turn without replying to the person, without sealing open tool calls and without marking the turn finished — the new owner is responsible for all of these.
3. **Given** a write carrying the current owner's token, **When** it is saved, **Then** it succeeds exactly as today.

---

### User Story 3 - Scheduled runs and background jobs are recovered once when their owner changes (Priority: P2)

A scheduled task run belongs to the control leader that fired it; a background job belongs to the worker running the agent that launched it, because that is where the job waits on its pieces. When either owner is lost, the node that takes over decides each open record once: a run or job whose work is still alive (a pending turn behind it) is picked up and finished as usual, and the rest are marked interrupted, so they show as lost and the people who should be told are told once. Records still owned by a healthy node are not touched.

**Why this priority**: Scheduled work and background jobs are less interactive than a live turn, and today's outcome for them on restart is already "picked up if a turn still backs it, otherwise interrupted". What matters is that the outcome is applied once, to the right rows, and never to a run another node is still running.

**Independent Test**: With two simulated owners, open a task run under control owner A and a background job under worker owner A, expire A, have B take over, and check that exactly those rows are resolved (picked up if backed, otherwise interrupted with one notification each), while rows owned by a live owner remain running.

**Acceptance Scenarios**:

1. **Given** a task run open under control leader A with no pending turn behind it, **When** A's leadership expires and B becomes leader, **Then** the run is marked interrupted and produces exactly one notification.
2. **Given** a task run open under A whose turn is still pending or running on a live worker, **When** B becomes leader, **Then** B waits for that turn and records the run's outcome when it ends, as a restart does today.
3. **Given** a background job is moving from running to reporting, **When** two owners race to finish or interrupt it, **Then** exactly one transition wins and the job is either reported or interrupted, never both.
4. **Given** a background job whose launching agent is still owned by a live worker, **When** another agent or the control leader changes hands, **Then** that job is not touched.

---

### User Story 4 - Pending client tool calls follow the owner of their turn (Priority: P2)

A turn on worker A is waiting for a person's browser to answer a client tool call. A is lost. The call's state is recovered by whoever takes the agent over: calls that cannot be answered any more are swept to interrupted as they are today on restart, and deferred calls survive and are still delivered once.

**Why this priority**: Client calls are a smaller surface than turns, but leaving one pending forever leaves a person staring at a spinner.

**Independent Test**: Leave a pending and a deferred client call under owner A, expire A, take over as B, and check the pending call becomes interrupted while the deferred one is still delivered exactly once.

**Acceptance Scenarios**:

1. **Given** a pending client call whose turn belonged to A, **When** B takes the agent over, **Then** the call is marked interrupted.
2. **Given** a deferred client call, **When** ownership changes, **Then** it remains deferred and is still delivered to exactly one browser tab.

---

### User Story 5 - A single node behaves exactly as today (Priority: P1)

An operator runs vizier as one process, on the embedded database, as almost everyone does. They restart it. Every in-flight turn resumes, every open run and job is marked interrupted, pending client calls are swept and leftover scratch files are cleared, exactly as before this feature.

**Why this priority**: Single node is the default deployment and the desktop app. Any change in its behavior is a regression for every current user.

**Independent Test**: Run the existing single-node test suite and the dummyplug end-to-end checks unchanged; restart a single node with a running turn, open task run, open job and pending client call, and compare the result with today's behavior.

**Acceptance Scenarios**:

1. **Given** a single node with in-flight work, **When** it restarts, **Then** the outcome for each kind of in-flight record is identical to today's.
2. **Given** a single node running normally, **When** it writes steps and history, **Then** no write is ever refused for ownership.
3. **Given** an existing single-node workspace from a release before this feature, **When** it is upgraded, **Then** its in-flight rows are treated as owned by the previous process and recovered on the first start.

---

### User Story 6 - The operator knows where other single-process assumptions remain (Priority: P3)

The operator, or the next developer working on Fleet, can read one place that lists every remaining assumption that vizier is the only process (in-process locks used for mutual exclusion, scratch-file clearing, notification watermarks, migration state keys, and anything else found), with what was done about each: fixed here, safe as is and why, or tracked in a named follow-up issue.

**Why this priority**: The audit does not change behavior by itself, but without it unsafe assumptions are rediscovered as production incidents.

**Independent Test**: The audit document exists, covers at least the four named areas, and every entry has a disposition; every "follow-up" entry names an issue.

**Acceptance Scenarios**:

1. **Given** the audit, **When** a reader looks up scratch-file clearing at startup, **Then** they find whether it is now ownership-aware and, if not, why it is safe or where it is tracked.

---

### Edge Cases

- **No worker takes the agent over.** If the owner is lost and no other worker takes the agent, its in-flight rows stay as they are until someone takes it; they are never recovered by a node that does not own the agent.
- **The same node comes back.** A node that restarts and takes back an agent it held before is a new owner with a new token; its own previous rows are recovered as an expired owner's would be. (This is what keeps single-node restarts behaving as today.)
- **Lease taken over twice quickly.** If A loses to B and B loses to C before B finished recovering, C recovers every row still marked with an expired owner, A's and B's alike, and B's late writes are refused like A's.
- **Recovery itself fails partway.** Recovery that fails for one row (for example the database is briefly unavailable) does not stop the new owner from serving the agent; the rows not recovered are retried on the next recovery pass and never recovered twice.
- **Clock skew between nodes.** Whether a lease has expired is decided by the lease authority, not by comparing one node's clock with a time another node wrote, so skew cannot make a healthy owner look expired.
- **Rows written before this feature** (no owner recorded) are treated as belonging to an expired owner, so they are recovered on the first takeover, exactly once.
- **A stale write that partly succeeded.** Refusal is per write and all-or-nothing: a step save that would also change turn state is either fully stored or fully refused.
- **The refused ex-owner holds a reply sender.** A person's live connection must not get a second, conflicting reply from the ex-owner; the ex-owner's refusal ends its stream without a final answer, and the reply comes from the new owner.
- **Control-owned rows when the leader changes but the worker does not.** A task run's turn runs on a worker; the run record belongs to control. A new leader picks up a run whose turn is still pending (it waits for the turn to end and records the outcome, as a restart does today) and interrupts only the runs with nothing behind them.

## Requirements *(mandatory)*

### Functional Requirements

**Ownership**

- **FR-001**: Every in-flight turn, task run, background job and client tool call MUST record which owner holds it and the ownership token the owner held when it claimed the record. Whether that owner's lease is still valid MUST be decided by the lease authority and the current ownership token, never by comparing a time written by one node with another node's clock.
- **FR-002**: Turns, client tool calls and background jobs MUST be owned through the lease of their agent (for a background job, the agent that launched it, whose process runs the job); task runs MUST be owned through the control leader's lease.
- **FR-003**: Ownership tokens MUST only increase for a given lease, so a later owner's token is always greater than any earlier owner's.
- **FR-004**: Records that existed before this feature MUST be readable and MUST be treated as held by an expired owner.

**Fencing**

- **FR-005**: Every write that advances in-flight work — saving a turn step, saving history for a turn, changing a turn's state (queued, running, finished, resumed, resume count), and the guarded transitions of runs, jobs and client calls — MUST carry the writer's ownership token and MUST be refused if a later owner holds the record.
- **FR-006**: The ownership check and the write MUST happen as one indivisible operation, so no write by a former owner can land after the takeover that superseded it.
- **FR-007**: A refused write MUST leave no partial effect, and MUST be reported to the writer as a distinct "ownership lost" outcome, separate from ordinary storage errors.
- **FR-008**: An owner that receives "ownership lost" for a turn MUST stop that turn without replying, sealing tool calls, finishing the turn or notifying anyone; the new owner does all of these.
- **FR-009**: Exactly-once transitions (background job running → reporting, and the other existing guarded transitions) MUST keep their current single-winner guarantee and MUST additionally check the owner.

**Recovery**

- **FR-010**: The startup sweeps for turns, task runs, background jobs and client tool calls MUST be replaced by a recovery routine that acts only on records whose owner's lease has expired.
- **FR-011**: When a worker takes over an agent's lease, it MUST recover that agent's expired records: running turns are re-sent as resumed from their last saved step and queued turns are re-sent in their original order (as today), and pending client calls are swept to interrupted while deferred ones are kept.
- **FR-012**: The recovery of task runs (when a node becomes control leader) and of background jobs (when their launching agent is taken over) MUST keep today's outcome: a record a pending turn still backs is picked up and finished as usual, every other one is marked interrupted and recorded as a notification exactly once.
- **FR-013**: Recovery MUST claim each record for the new owner in the same indivisible step that decides it is expired, so two nodes recovering at once never both recover the same record.
- **FR-014**: Recovery MUST never touch a record whose owner still holds a valid lease.
- **FR-015**: The count of resumes without progress MUST be kept with the turn record and carried across a change of owner, and the existing limit on resumes without progress MUST apply across nodes.
- **FR-016**: A recovery failure for one record MUST NOT prevent the new owner from serving the agent, and the record MUST be retried on a later recovery pass.

**Single node**

- **FR-017**: In a single node, the one process MUST always be the owner, and its lease MUST never expire while it runs; no write MUST ever be refused for ownership.
- **FR-018**: A single-node start MUST recover all in-flight records of the previous process, producing the same outcome as today's startup sweeps for every kind of record, including the scratch files and thinking marks kept for sessions with a pending turn.
- **FR-019**: The single-process embedded database MUST keep working with no new setup by the operator; Fleet's multi-owner behavior is required only on the shared networked database.

**Audit**

- **FR-020**: The feature MUST produce an audit of remaining single-process assumptions on the shared-database path, covering at least in-process locks used as mutual exclusion, scratch-file clearing at startup, notification watermarks and migration state keys, each with a disposition (fixed here, safe and why, or a named follow-up issue).
- **FR-021**: Scratch-file clearing at startup MUST no longer clear the files of a conversation whose turn is owned by a live owner.

**Compatibility and tracking**

- **FR-022**: Schema changes MUST be applied on both database backends, and any data move or legacy-record handling (such as FR-004) MUST be added to the pre-1.0 removal tracker (#82) per the project convention.

### Key Entities

- **Owner**: a node holding a lease; identified by node, the lease's expiry, and the ownership token it received when it took the lease.
- **Ownership token**: a number given out with each acquisition of a lease, strictly greater than every earlier one for the same lease; the means by which a write proves it comes from the current owner.
- **In-flight record**: a turn, task run, background job or client tool call while it is not yet final; it carries its owner, the owner's lease expiry and the token at claim time.
- **Agent lease**: the lease that makes one worker the owner of one agent; owns that agent's turns and client calls.
- **Control leadership lease**: the lease that makes one node the control leader; owns task runs and background jobs.
- **Recovery pass**: the routine a new owner runs on takeover to claim and resolve the expired owner's in-flight records.
- **Audit entry**: one single-process assumption, where it lives, and its disposition.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: In a two-owner takeover test, 100% of the expired owner's running turns resume on the new owner and complete exactly once, and 0 records belonging to a live owner are touched.
- **SC-002**: In a stale-writer test, 100% of writes carrying a superseded token are refused, and none of their content is visible afterwards.
- **SC-003**: In a race test where two nodes recover the same expired owner at once, every record is recovered exactly once (no record recovered twice, none skipped) across at least 1,000 records.
- **SC-004**: Each interrupted task run and background job produces exactly one notification across a leader change, with no duplicates and none missing.
- **SC-005**: A turn that keeps causing node loss is given up after the existing resume limit in total, not per node.
- **SC-006**: All existing tests pass on both database backends, and the single-node restart outcome for every kind of in-flight record is unchanged from today's.
- **SC-007**: Single-node turn latency and throughput show no regression attributable to the ownership check (within normal run-to-run variation).
- **SC-008**: The audit covers all four named areas, and every entry has a disposition.

## Assumptions

- **Dependency**: the lease and fencing-token primitives of the Fleet bus contract (#162) are available, including strictly increasing tokens per lease name and expiry decided by the lease authority.
- **Worker and control roles are not yet live** (#166–#170). This feature builds the ownership, fencing and recovery mechanisms and proves them with simulated owners; wiring them to real worker takeover and leader election is done by those roles. The recovery routine is designed so those roles only need to call it at takeover.
- **Shared database** means the PostgreSQL backend; **embedded database** means SQLite, which stays single-process. Multi-owner tests run against PostgreSQL (the managed-storage CI job).
- **The resume counter already lives in the turn record** today; this feature keeps it there and makes sure it is carried, not reset, across owners.
- **A task run's record belongs to control while its turn runs on a worker.** The new leader re-attaches to runs whose turn is still pending (today's restart behaviour, which already waits on the session's events rather than on the old process) and interrupts only the rest.
- **A background job belongs to the agent that launched it**, not to control: its runner lives in the process that launched it, so a lost control leader must not interrupt it and a lost worker must. Moving the runner to control is a later Fleet decision (#168) and would only change which owner a new job is written under.
- **Ownership-lost handling on the ex-owner** stops the turn silently; the person sees only the new owner's reply. No notification or log beyond a warning is produced by the ex-owner.
- **Notification watermarks and migration state keys** are audited; they are changed here only if the audit finds them unsafe and the fix is small, otherwise they become follow-up issues.
- **No UI change**: this is backend work, so no FigCraft pass is needed.
