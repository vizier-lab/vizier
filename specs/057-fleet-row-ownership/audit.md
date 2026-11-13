# Audit: single-process assumptions on the shared-database path

**Feature**: `057-fleet-row-ownership` (FR-020, SC-008) | **Started**: 2026-11-13, from [research.md](research.md)

What this lists: every place that is correct only because vizier is the one process touching its data. Each entry has a disposition:
- **fixed here**: this feature changes it;
- **safe**: it stays as it is, with the reason;
- **follow-up**: it is unsafe in a Fleet and is tracked in the named issue.

The implementation completes the table (it fills the two `#TBD` issue numbers and adds anything else it finds). The feature is not done while an entry has no disposition.

## Required areas (from #164)

| # | Assumption | Where | Disposition |
|---|---|---|---|
| 1 | In-process locks used as mutual exclusion | see entries 5-8 | covered by 5-8 |
| 2 | Scratch files cleared at startup for every session without a pending turn | `dependencies.rs::clear_orphaned_scratch` | **fixed here**: cleared per agent when its scope is claimed, only for that agent's sessions with no pending turn (FR-021). Thinking marks follow the same rule |
| 3 | Notification watermarks | WebUI `lastSeenId` (`stores/notifications.ts`); server `purge_orphans`, `purge_before` at startup | **safe**: the watermark is per browser tab and never shared. The purges are idempotent deletes (by age, by missing agent); running them on several nodes deletes nothing extra. Purges belong to control (#168) |
| 4 | Migration state keys (`migration.*_v1`) checked and then run by every node at startup | `dependencies.rs`, `encryption/migrate.rs`, `projects/migrate.rs`, `provider_migration.rs` | **follow-up** #TBD: two nodes booting together on PostgreSQL can both see the marker unset and run a one-time migration twice. Move them to control and take `lock_key` around check-then-run. SQLite-only migrations are unaffected (one process) |

## Found during research

| # | Assumption | Where | Disposition |
|---|---|---|---|
| 5 | Startup sweeps: open task runs, background jobs and pending client calls are interrupted at startup | `dependencies.rs` (`interrupt_orphaned_*`, `record_interrupted`) | **fixed here**: recovery on claim (research R5) |
| 6 | Open sensitive asks abandoned at startup (their waiter is a `oneshot` in the owner's memory) | `questions/sensitive.rs::abandon_open_asks` | **fixed here**: per agent on claim; `sensitive_ask` gains owner columns |
| 7 | `BackgroundJobs.cancels`: an in-memory map from a job id to its runner's cancel sender | `agents/background/mod.rs:184` | **follow-up** #TBD: in a Fleet, a cancel received by a node other than the runner's finds no sender. Send the cancel over the bus to the job's owner. Safe in single node and `--role all` |
| 8 | Session-title de-dupe (`in_flight` set) | `agents/process.rs:531` | **safe**: titling runs inside the agent, and one owner runs an agent |
| 9 | Provider-logo download de-dupe | `catalogue/refresh.rs:35` | **safe**: at worst a logo is fetched twice; the cache write is last-writer-wins |
| 10 | Catalogue refresh triggered by a person's read on any gateway | `catalogue/refresh.rs` | **safe**: the cache row is last-writer-wins and refreshes are idempotent |
