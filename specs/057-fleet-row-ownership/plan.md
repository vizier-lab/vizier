# Implementation Plan: Fleet row ownership, fencing and lease-based recovery

**Branch**: `057-fleet-row-ownership` | **Date**: 2026-11-13 | **Spec**: [spec.md](spec.md) | **Issue**: #164 (epic #159)

**Input**: Feature specification from `specs/057-fleet-row-ownership/spec.md`

## Summary

Recovery today happens at startup and assumes one process: `VizierDependencies::new` sweeps open task runs, background jobs, pending client calls, sensitive asks, scratch files and thinking marks, and `AgentHost::take` re-sends every pending turn. This feature makes **ownership** the thing recovery depends on.

- **Ownership scopes.** Each scope (`agent:{id}`, `control`) has one row in a new `owner_fence` table, whose `token` the database bumps on every **claim**. In a Fleet the bump is guarded by the bus lease's fencing token; in single node it is unconditional (research R1).
- **Fenced writes.** Writes that advance in-flight work check that token inside their own transaction, under a shared lock that a claim must wait for (R2). The writer's token comes from a process-local registry at the storage boundary, so the ~40 history call sites do not change (R3).
- **Stop on loss.** A refused write stops the agent on that node, through `AgentHost::release` (R4).
- **Recovery on claim.** Two routines, `recover_agent` and `recover_control`, run when a scope is claimed and replace the five startup sweeps plus `resend_pending_turns`. They act only on rows whose `owner_token` is older than the new fence, and re-own each row in the statement that resolves it (R5).
- **Exactly-once turns.** Turns are claimed when the owner receives them, which also drops duplicate deliveries (R6).
- **Single node unchanged.** One start claims every agent and `control` once, so it does exactly what the sweeps did.

## Technical Context

**Language/Version**: Rust, 2024 edition (root `Cargo.toml` `[workspace.package]`)

**Primary Dependencies**: existing only. `vizier-schema` (`FencingToken`, `Leases`), the backend-neutral SQL layer (`storage::sql`), tokio. No new crate: `OwnershipLost` has a hand-written `Error` impl.

**Storage**: SQLite (default, single-process) and PostgreSQL (opt-in, shared). New table `owner_fence`; `owner_node`/`owner_token` columns on `session_turn`, `task_run`, `background_job`, `client_tool_call`, `sensitive_ask` ([data-model.md](data-model.md)). PostgreSQL migration `0007_row_ownership.sql`; SQLite through `CREATE TABLE IF NOT EXISTS` and `add_column_if_missing`.

**Testing**: `cargo test --workspace` and `cargo clippy --workspace`. Two-owner tests run on PostgreSQL through `postgres_tests!` and a new `tests_support::postgres_store_pair()` (two pools, one schema), in the `managed-storage` CI job. End-to-end: the dummyplug steps in [quickstart.md](quickstart.md) for the single-node restart (mid-turn resume, picked-up and interrupted runs and jobs, no refusals).

**Target Platform**: the server (Linux, macOS, Docker) and the desktop app (single node, SQLite only, unaffected beyond the identical-behaviour requirement)

**Project Type**: engine library (`vizier-core`) plus server binary

**Performance Goals**: no measurable single-node regression (SC-007). The added cost is one primary-key read of `owner_fence` per fenced write, in a transaction the write already opens.

**Constraints**: single-node behaviour identical (FR-017, FR-018). No clock comparisons across nodes (FR-001). Small PRs that land back to back, because `transport/mod.rs`, `agents/mod.rs`, `agents/process.rs`, `dependencies.rs` and `runtime.rs` are shared with #160–#163.

**Scale/Scope**: exactly-once recovery verified over ≥1,000 rows with two concurrent recoverers (SC-003).

## Constitution Check

*GATE: passed before Phase 0; re-checked after Phase 1 design (below).*

| Gate | Status | Notes |
|---|---|---|
| I. Lean by default | ✅ | No new dependency. Two new storage methods (`claim`, `ownership_lost`) and one registry. No `scope` column (derived from existing columns). No per-row lease expiry (R1, FR-001) |
| II. Trait-based, no type branching | ✅ | Fencing lives once at the storage boundary (like encryption), not as branches at call sites. The only backend difference is a `Dialect` fragment (`FOR SHARE`). Scope derivation is a per-table rule in storage, not a `match` over callers |
| III. Self-contained default | ✅ | SQLite single-node path needs no setup; PostgreSQL stays opt-in |
| IV. Portability | ✅ | Pure SQL and Rust; nothing OS-specific |
| V. Unified errors & observability | ✅ | `OwnershipLost` travels inside `anyhow::Error` like other storage errors, is recognised by one helper, and is logged with `tracing::warn!` once per scope |
| Feature flags never change API shape | ✅ | No new flag: fencing is always on, and inert in single node |
| Roles depend on trait seams only | ✅ | Roles will call `FenceStorage::claim` and `recovery::recover_*` ([contracts](contracts/)), not `AgentHost` internals |
| Verify what a change touches | ✅ | Touches `vizier-core` storage and agents, which runs the core, flag and `managed-storage` jobs |
| Startup/migration changes run the binary | ✅ | quickstart §2–§4 |
| dummyplug e2e for agent-observable change | ✅ | quickstart §2–§4 |
| Design before UI code | n/a | Backend only, no UI change, so no FigCraft pass |
| Compatibility code tracked in #82 | ✅ | `add_column_if_missing` calls and the null-owner-means-legacy rule get #82 entries (R9, R10) |

**Post-design re-check**: still passing. One deliberate deviation from the issue text, recorded in the spec: background jobs are owned by their **launching agent's** scope, not `control`, because their runner lives in the launching process (spec FR-002, research R5).

## Project Structure

### Documentation (this feature)

```text
specs/057-fleet-row-ownership/
├── plan.md              # this file
├── research.md          # Phase 0: R1–R12
├── data-model.md        # Phase 1
├── quickstart.md        # Phase 1
├── audit.md             # FR-020 deliverable, seeded from research; completed during implementation
├── contracts/
│   ├── ownership.md     # claim, fence registry, fenced writes, OwnershipLost
│   └── recovery.md      # recover_agent / recover_control
├── checklists/requirements.md
└── tasks.md             # Phase 2 (/speckit-tasks)
```

### Source Code (repository root)

```text
crates/vizier-core/src/
├── storage/
│   ├── fence.rs                         # NEW: Scope, Fence, OwnershipLost, Fences registry, FenceStorage trait
│   ├── sqlite/fence.rs                  # NEW: claim + the shared `check_fence(tx, scope)` helper (both backends)
│   ├── sqlite/mod.rs                    # owner_fence DDL, add_column_if_missing ×10, new indexes; SqlStorage holds Arc<Fences>
│   ├── sqlite/{history,turn,client_call,background_job,task_run,question}.rs
│   │                                    # fenced writes; owner columns written; recovery selects/re-owns
│   ├── {turn,client_call,background_job,task_run}.rs
│   │                                    # trait additions: claim_turn_on_receipt, recovery queries scoped by agent/owner
│   ├── scratch.rs, session.rs           # per-agent clear_scratch_except / reset_thinking_except
│   ├── sql/mod.rs, sql/postgres.rs, sql/sqlite.rs
│   │                                    # Dialect::share_lock_suffix()
│   └── sql/postgres/migrations/0007_row_ownership.sql   # NEW
├── recovery.rs                          # NEW: recover_agent, recover_control (moved logic from dependencies.rs)
├── dependencies.rs                      # the five sweeps and record_interrupted removed
├── agents/mod.rs                        # take(): claim + recover_agent (+ retry); watch ownership_lost → release
├── agents/process.rs                    # claim on receipt, drop duplicates
├── agents/steps.rs                      # record_step returns early on OwnershipLost
├── agents/background/mod.rs             # recover narrowed to one origin agent
├── runtime.rs                           # claim control + recover_control before the scheduler; job recover call moves into recover_agent
└── tests_support.rs                     # postgres_store_pair()
```

**Structure Decision**: all code goes in `vizier-core` (engine). `vizier-schema` is untouched (its `FencingToken` is enough). `vizier-fleet` is untouched; its roles wire the contracts later. No `vizier-http` or WebUI change.

## Delivery order (small PRs, back to back)

Each step is independently mergeable and leaves single node unchanged:

1. **Schema + fence primitive**: migration, DDL, `fence.rs`, `claim`, `Dialect::share_lock_suffix`, claim tests (monotonic, lease order, single node). Nothing calls it yet.
2. **Fenced writes**: `check_fence` in each write of the ownership contract table, owner columns written on open, `OwnershipLost`, the lost signal; stale-writer tests on both backends. Inert until something claims.
3. **Claim and recovery**: `recovery.rs`, `AgentHost::take` and the runtime claim and recover, receipt claim in `agent_process`, release on loss; sweeps removed from `dependencies.rs`; takeover, race, resume-limit, legacy and single-node-outcome tests; quickstart §2–§4.
4. **Audit**: complete [audit.md](audit.md), open the two follow-up issues it names, add the #82 entries.

## Complexity Tracking

No constitution violation to justify.
