# Ownership and Recovery

In a Fleet more than one process can read the same database, but only one of them may *run* a given
agent, and only one may run the scheduler. This page is the text of how that is decided, what a
node that has lost its place is refused, and how the next owner picks up the work the last one left.
The code is in `vizier-core` (`storage/fence.rs`, `recovery.rs`); the checks are the tests in
`storage/sqlite/fence_tests.rs` and `recovery_tests.rs`.

## Scopes and the fence

Ownership is held per **scope**: `agent:{id}` for each agent, and `control` for the scheduler's
work (task runs). Each scope has one row in the `owner_fence` table. Its `token` is the **fence**:
the database adds one every time the scope is claimed, so it only ever grows, across processes and
restarts.

1. **A claim bumps the fence.** `FenceStorage::claim(scope, node, lease)` returns the new fence.
2. **A Fleet claim carries the bus lease token.** It is refused when a claim with an equal or newer
   lease token already succeeded, so a node whose lease expired before it claimed can never overtake
   the one that replaced it.
3. **A single node claims with no lease** and always succeeds. Nothing else can claim, so nothing is
   ever refused.

A process remembers the fence it obtained. A process that never claimed a scope (a gateway) holds
none, and is never refused: the fence protects against a stale *owner*, not against other roles.

## What a former owner cannot do

Every write that advances in-flight work checks, in its own transaction, that the stored token is
still the one this process holds. If it is not, the write changes nothing and fails with
`OwnershipLost`. On PostgreSQL the check reads the fence row `FOR SHARE`, so a claim waits for the
writes already in progress and none commits after the claim that replaced it.

| Scope | Fenced writes |
|---|---|
| `agent:{id}` | history saves and checkpoints; every `session_turn` transition except accepting a request; client tool calls (open, finish, defer, state); sensitive asks (open, close); detached scripts of the agent (open, progress, finish, cancel, the delivery claim, interrupt) |
| `control` | task runs (open, close) |

The first refusal of a scope flips a signal. The agent host watches it and releases the agent, so the
old owner stops its turns instead of running on without being able to save them. Reads, deletes of a
whole agent or conversation, notification rows, usage rows, memory writes and scratch files are not
fenced.

## What the next owner does

When a node claims a scope it runs a recovery pass before the scope's work starts. The pass acts
only on rows whose `owner_token` is null (written before this feature) or older than the new fence,
and re-owns each row in the same statement that resolves it. Two recoverers of one scope therefore
cannot both resolve a row: the older one is refused and stops.

For `agent:{id}`:

1. Pending client tool calls become interrupted. Deferred calls survive owners, as they survive
   restarts.
2. Open sensitive asks are abandoned.
3. Thinking marks are reset and scratch files cleared for the agent's conversations that have no
   pending turn.
4. Pending turns are re-sent in the order they were accepted, a turn that had begun marked
   `resumed`. The new owner claims each turn when it receives it, and drops a delivery of a turn it
   already holds, so a turn runs once. The resume counter lives in the turn's row, so the limit on
   resumes without progress holds across owners.
5. Detached scripts the previous owner left running become `interrupted` (with one notification
   each), and every conversation with a script waiting to be delivered is poked once. The delivery
   claim takes each script exactly once, however many pokes arrive.

For `control`: task runs that no pending turn backs become interrupted, with one notification each;
the rest are re-owned and left running for the scheduler to close when their turn ends.

A pass that fails part way is repeated in the background with the supervisor's backoff (2 s doubling
to 5 min). Repeating it does nothing twice.

## Single node

One start claims every agent and `control` once, which is exactly what the old startup sweeps did:
every in-flight row of the previous process is recovered, with the same outcomes and notifications.
No write is ever refused. The log lines that used to appear once at startup now appear when each
agent is taken.

## Not yet covered

The [audit](https://github.com/vizier-lab/vizier/blob/develop/specs/057-fleet-row-ownership/audit.md)
lists the remaining single-process assumptions and where each is tracked: a cancel of a background
job that reaches a node other than its runner's, an answer to a sensitive ask that reaches a gateway
other than the owner, and one-time migrations that two nodes booting together could both run.
