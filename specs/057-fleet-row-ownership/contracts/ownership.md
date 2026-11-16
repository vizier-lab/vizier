# Contract: ownership and fencing

**Feature**: `057-fleet-row-ownership` | **Consumers**: `AgentHost` and the runtime today; the worker (#166) and control (#168) roles next.

This is the seam the Fleet roles call. A networked worker or a control leader built from this page alone must behave correctly.

## Types (`crates/vizier-core/src/storage/fence.rs`)

```rust
pub enum Scope { Agent(AgentId), Control }        // Display: "agent:{id}" | "control"

pub struct Fence { pub scope: Scope, pub node: String, pub token: u64 }

/// Display: "ownership of {scope} moved to a newer owner". A hand-written `std::error::Error`
/// impl, as `agents_client.rs` does (no new dependency).
#[derive(Debug)]
pub struct OwnershipLost { pub scope: String }

pub fn is_ownership_lost(err: &anyhow::Error) -> bool;
```

## Storage trait: `FenceStorage` (on `VizierStorageProvider`)

```rust
/// Take ownership of `scope` for `node`. `lease` is the bus fencing token of the lease just
/// acquired, or `None` in single node. On success, every later fenced write of `scope` from
/// this process is checked against the returned fence.
async fn claim(&self, scope: &Scope, node: &str, lease: Option<FencingToken>)
    -> Result<Fence>;   // Err(OwnershipLost) when a newer lease already claimed

/// Fires `true` the first time a fenced write of `scope` from this process is refused.
fn ownership_lost(&self, scope: &Scope) -> watch::Receiver<bool>;
```

## Rules

1. **Monotonic.** Each successful `claim` of a scope returns a `token` strictly greater than every earlier one for that scope, across processes, restarts and backends.
2. **Lease order.** A Fleet claim carrying lease token `t` is refused if a claim with a lease token `≥ t` has already succeeded. So a holder whose lease expired before it claimed can never overtake the holder that replaced it.
3. **Fenced writes.** While this process holds a fence for scope `S`, every write that advances in-flight work of `S` (the list below) succeeds only if `owner_fence.token` for `S` still equals this process's token. Otherwise it changes nothing and fails with `OwnershipLost`.
4. **Indivisible.** The check and the write are in one transaction, and a `claim` of `S` waits for fenced writes of `S` already in progress. No fenced write commits after the claim that superseded it.
5. **Unfenced callers.** A process that holds no fence for `S` (a gateway) writes as today. Fencing protects against stale *owners*, not against other roles.
6. **Stop on loss.** After the first `OwnershipLost` for `S`, `ownership_lost(S)` reads `true` and the owner MUST stop the work of `S` (for an agent: `AgentHost::release`). The same applies when the bus lease's `renew` fails (lease rule 11); the role calls `release` for that too.
7. **Single node.** `claim(_, node, None)` always succeeds, and nothing else can claim, so no write is ever refused (FR-017).

## Fenced writes

| Scope | Writes |
|---|---|
| `agent:{id}` | `save_session_history` and every other history mutation of a session of `id` (checkpoint, entry updates), `save_with_busy_retry`/`record_step`; `session_turn`: claim on receipt, `start_turn`, `set_turn_request_uid`, `note_resume`, `finish_turn`, `cancel_session_turns`; `client_tool_call`: open, finish, defer, state save; `sensitive_ask`: open, close; `background_job`/`background_piece` of origin `id`: open, close piece, transition |
| `control` | `task_run`: open, close |

A new write path that advances in-flight work of a scope MUST go through a fenced method. Review checks this; a test per row of the table pins it.

## Not fenced, on purpose

Reads; deletes of a whole agent, session or topic (people's actions, not an owner's); notification rows (idempotent by `event_key`); usage rows (append-only, spawned after the call); memory writes (versioned by revision, not by turn); scratch files (per conversation; cleared by recovery).
