# Audit: single-process assumptions on the shared-database path

**Feature**: `057-fleet-row-ownership` (FR-020, SC-008) | **Started**: 2026-11-13, from [research.md](research.md)

What this lists: every place that is correct only because vizier is the one process touching its data. Each entry has a disposition:
- **fixed here**: this feature changes it;
- **safe**: it stays as it is, with the reason;
- **follow-up**: it is unsafe in a Fleet and is tracked in the named issue.

Completed by the implementation (T053). Follow-ups: #202, #203, #204 (T054). The feature is not done while an entry has no disposition.

## Required areas (from #164)

| # | Assumption | Where | Disposition |
|---|---|---|---|
| 1 | In-process locks used as mutual exclusion | see entries 5-8 | covered by 5-8 |
| 2 | Scratch files cleared at startup for every session without a pending turn | `dependencies.rs::clear_orphaned_scratch` | **fixed here**: cleared per agent when its scope is claimed, only for that agent's sessions with no pending turn (FR-021). Thinking marks follow the same rule |
| 3 | Notification watermarks | WebUI `lastSeenId` (`stores/notifications.ts`); server `purge_orphans`, `purge_before` at startup | **safe**: the watermark is per browser tab and never shared. The purges are idempotent deletes (by age, by missing agent); running them on several nodes deletes nothing extra. Purges belong to control (#168) |
| 4 | Migration state keys (`migration.*_v1`) checked and then run by every node at startup | `dependencies.rs`, `encryption/migrate.rs`, `projects/migrate.rs`, `provider_migration.rs` | **follow-up** #203: two nodes booting together on PostgreSQL can both see the marker unset and run a one-time migration twice. Move them to control and take `lock_key` around check-then-run. SQLite-only migrations are unaffected (one process) |

## Found during research

| # | Assumption | Where | Disposition |
|---|---|---|---|
| 5 | Startup sweeps: open task runs, background jobs and pending client calls are interrupted at startup | `dependencies.rs` (`interrupt_orphaned_*`, `record_interrupted`) | **fixed here**: recovery on claim (research R5) |
| 6 | Open sensitive asks abandoned at startup (their waiter is a `oneshot` in the owner's memory) | `questions/sensitive.rs::abandon_open_asks` | **fixed here**: per agent on claim; `sensitive_ask` gains owner columns |
| 7 | `BackgroundJobs.cancels`: an in-memory map from a job id to its runner's cancel sender | `agents/background/mod.rs:184` | **follow-up** #202: in a Fleet, a cancel received by a node other than the runner's finds no sender. Send the cancel over the bus to the job's owner. Safe in single node and `--role all` |
| 8 | Session-title de-dupe (`in_flight` set) | `agents/process.rs:531` | **safe**: titling runs inside the agent, and one owner runs an agent |
| 9 | Provider-logo download de-dupe | `catalogue/refresh.rs:35` | **safe**: at worst a logo is fetched twice; the cache write is last-writer-wins |
| 10 | Catalogue refresh triggered by a person's read on any gateway | `catalogue/refresh.rs` | **safe**: the cache row is last-writer-wins and refreshes are idempotent |

## Found by the implementation search (T053)

A search of `crates/vizier-core/src` for process-local maps and cells (`Mutex<HashMap`, `OnceLock`, `LazyLock`), every `migration.*` state key, and the startup-time writes left in `dependencies.rs`.

| # | Assumption | Where | Disposition |
|---|---|---|---|
| 11 | A client tool's live socket and its answer slot | `agents/client/connection.rs:34` | **safe**: process-local on purpose (CLAUDE.md, Fleet bus text); a call whose owner is not the socket's node reads as closed and is deferred or interrupted, both recoverable |
| 12 | Sensitive-ask waiters, slots and drafts (`oneshot` per ask, in the owner's memory) | `questions/mod.rs:66-70` | **follow-up** #204: an answer submitted on a gateway that is not the owner has no waiter to resolve. Route the answer to the owner over the bus. A takeover already abandons the ask (item 6), so nothing is left dangling |
| 13 | Channel readers and their side-chat exchanges | `channels/connections.rs:110`, `channels/side_chat.rs:281` | **safe**: owned by the channels role, whose gate (#161) lets one node run each reader; side chats are memory-only by design |
| 14 | Language servers, the project read ledger, file suggestion cache | `projects/lsp/`, `projects/ledger.rs`, `projects/suggest.rs` | **safe**: all belong to the node that runs the agent's turn (the owner) and are rebuilt on demand; none is shared state |
| 15 | Device-flow state of a provider sign-in | `service/providers_sso.rs:41` | **safe** for single node and sticky gateways; a poll that lands on another gateway finds no flow and the person starts again. Noted for the gateway role (#167) |
| 16 | Reachability cache, PostgreSQL statement rewrite cache, logo/codec caches | `service/usage.rs:130`, `storage/sql/postgres.rs:53`, `catalogue/seed.rs`, `stt/whisper.rs` | **safe**: read-through caches, correct per node |
| 17 | The remaining startup-time writes in `dependencies.rs` | `usage_event` legacy rewrite (448), one-time task rewrite (1217), state-key cleanup (1514) | **safe**: each is guarded by its `migration.*` marker or idempotent; covered by item 4 for the marker race |
| 18 | The in-flight sweeps that remained in `VizierDependencies::new` | `dependencies.rs` | **fixed here**: none remains. `rg "interrupt_orphaned|abandon_open_asks|clear_orphaned_scratch" crates/vizier-core/src/dependencies.rs` prints nothing |
