# Contract: lease-based recovery

**Feature**: `057-fleet-row-ownership` | **Replaces**: the five startup sweeps in `VizierDependencies::new` and `AgentHost::resend_pending_turns`.

## Entry points (`crates/vizier-core/src/recovery.rs`)

```rust
/// After `claim(Scope::Agent(id), ..)` and before the agent's instance starts.
pub async fn recover_agent(deps: &VizierDependencies, agent_id: &AgentId, fence: &Fence) -> Recovered;

/// After `claim(Scope::Control, ..)` and before the scheduler's first tick.
pub async fn recover_control(deps: &VizierDependencies, fence: &Fence) -> Recovered;

pub struct Recovered { pub complete: bool }   // false: some step failed; call again later
```

Callers today:
- `AgentHost::take` claims `agent:{id}` (single node: `lease = None`) and calls `recover_agent` where `resend_pending_turns` was. When `complete` is false, the host retries `recover_agent` in the background with the supervisor's backoff (2 s doubling to 5 min) until it is.
- `VizierRuntime::start` claims `control` when it runs the scheduler and calls `recover_control` before `VizierScheduler::run`, whose existing `recover` then re-attaches the runs kept open.

Next: the worker role (#166) calls the same pair on lease acquisition, with `lease = Some(token)`, and control (#168) does so on becoming leader.

## What each one does, in order

`recover_agent(id, fence)`. Every step acts only on rows of the agent with `owner_token IS NULL OR owner_token < fence.token`:

1. `client_tool_call` with status `pending` → `interrupted`, re-owned. `deferred` calls are left as they are (they survive owners, as they survive restarts).
2. `sensitive_ask` open → `abandoned`, re-owned.
3. Thinking marks reset, and scratch files cleared, for the agent's sessions that have **no** `session_turn` row.
4. `background_job` with origin `id`: a job a pending turn backs is re-owned and picked up (`running`: its runner restarts and awaits its pieces; `reporting`: marked `reported`, since its report turn is re-sent in step 5). Every other open job → `interrupted`, re-owned, one `job_ended` notification.
5. `session_turn` rows of the agent, in id order, re-sent; `resumed` when `running`. They are **not** re-owned here: the owner claims each on receipt (research R6).

`recover_control(fence)`, on rows with `owner_token IS NULL OR owner_token < fence.token`:

1. Open `task_run` rows that no pending turn backs → `interrupted`, re-owned, one `task_run_ended` notification each.
2. The rest are re-owned and left `running` for `VizierScheduler::recover`.

## Guarantees

- **Only expired rows**: a row whose `owner_token` equals a live owner's fence is never selected (FR-014). Rows a new owner creates carry its token and are never selected by its own later passes.
- **Exactly once**: re-owning and resolving happen in one fenced statement per row, so two recoverers of one scope cannot both resolve a row; the older one is refused and stops (SC-003).
- **Retry-safe**: a failed step leaves its rows selectable, and a successful one has moved them out of the predicate, so calling `recover_*` again repeats nothing (FR-016).
- **Same outcome as today in single node**: one start claims every agent and `control` once, so every in-flight row of the previous process is recovered, with the outcomes, notifications and kept scratch files that the sweeps produce today (FR-018, SC-006). Log lines move from startup to each agent's take.
