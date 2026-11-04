# Phase 0 Research: Fleet Agent Service Seam

**Feature**: #163 - agent service seam (`AgentsClient`, `AgentHost`, `Placement`)
**Date**: 2026-11-03
**Inputs**: `specs/163-agent-service-seam/spec.md`, the Fleet epic (#159), the bus contract (#162, merged as `afc2890e`), and the source of `vizier-core`, `vizier-http` and `vizier-schema`.

Every decision below is grounded in code that was read during this phase. The file and line
references are the evidence.

---

## R1. Where the seam lives and how a caller reaches it

**Decision**: `AgentsClient` is a trait in `vizier-core`, in a new stable-tier module
`crates/vizier-core/src/agents_client.rs`. `VizierDependencies` gains one field,
`pub agents: Arc<dyn AgentsClient>`. `Services` (`service/mod.rs:43-45`) and `HTTPState`
(`vizier-http/src/state.rs:20-39`) already hold a `VizierDependencies`, so both reach the seam
without a new plumbing layer.

**Rationale**: the trait speaks in core types - `AgentConfig`, `VizierRequest`, `VizierResponse`,
`AgentHealthStatus`, `IntegrationStatus` - so it cannot live in `vizier-schema`, which depends on
nothing of ours and must stay a leaf (CI: "The bus contract stays a leaf"). The epic's "Where the
code lives" table lists `AgentsClient` among the seams that stay in core, next to `VizierTransport`.
`VizierTransport` is in the stable tier (`lib.rs:77`), and a seam a published crate will implement
belongs in the same tier as the seam it sits beside.

**Alternatives considered**:
- *Put it in `vizier-schema`*: impossible - the types would have to move down into the leaf crate,
  which the epic forbids ("Core types such as `VizierRequest` stay in core and convert at the
  boundary").
- *A new crate `vizier-agents`*: the epic's crate table has no such crate, and it would only hold a
  trait plus two implementations. One more crate for one trait is against Principle I.
- *More methods on `VizierTransport`*: the transport is the engine's own channel to its subsystems
  (`lib.rs:39-41`); the seam is the *gateway's* client. One type doing both would be two contracts
  in one signature, and the remote implementation would have to replace the transport wholesale.

---

## R2. The local implementation talks over the bus, not by method call

**Decision**: `LocalAgentsClient` reaches the agent manager **over the in-process bus**, on the same
subjects a networked implementation would use. It is a typed wrapper over `VizierTransport`, not a
direct call into `VizierAgents`.

**Rationale**: FR-002 requires a local and a bus implementation that satisfy *one* contract. If the
local implementation called `VizierAgents` by method, the contract would describe a method-call API
and the remote implementation would have to invent a message protocol that the local one never
exercised - the exact "contract the local implementation does not honour" failure. The bus already
carries agent commands and turn requests (`transport/mod.rs:462`, `:329`), so the local
implementation is the *same* code path a networked one takes.

**Alternatives considered**:
- *Direct in-process calls*: rejected above.
- *One implementation that picks a transport at runtime*: that is the branch-on-a-type-tag the
  constitution's Principle II forbids. Two implementations behind one trait is the shape the
  constitution asks for.

---

## R3. Addressing: the seam addresses an agent, on per-agent subjects

**Decision**: the seam reaches an agent on two subjects:
- `agent.{id}.req` - the turn and its frames. Exists today (`subject.rs:104-107`).
- `agent.{id}.cmd` - **new**: the agent's lifecycle commands. One new subject constructor in
  `vizier-schema`.

Creating an agent is the one operation with no owner yet, so it stays on the fleet-wide
`agents.cmd` (`subject.rs:13`, `:128-130`), which the control role owns in a cluster and the local
host owns in a single node.

**Rationale**: FR-007 says the seam "MUST address the agent and never a host, a node or a process".
A per-agent subject is the only address that satisfies that literally *and* routes to exactly one
handler: only the agent's owner subscribes to `agent.{id}.cmd`, so the bus delivers the command to
one subscriber. The fleet-wide health check cannot be addressed this way, which is why health moves
to shared state (R6).

**Alternatives considered**:
- *Keep every command on the fleet-wide `agents.cmd` and filter by ownership*: rejected. The bus is
  pub/sub, so every worker subscribed to `agents.cmd` receives every command; the ones that do not
  own the agent would consume and drop it. Delivery is at-most-once, so there is no redelivery to
  save it - the caller's update or delete would simply vanish. This is the reason the global
  `agents.cmd` cannot survive into a cluster, and it is why the split exists.
- *A reply-address scheme where every worker tries*: same failure, plus a timeout instead of an
  error.
- *Route every command through the control role*: turns each command into a two-hop operation and
  puts the control plane on the hot path of every chat turn.

---

## R4. Abort is a request, not a new message kind

**Decision**: the seam's `abort(agent_id, session)` sends the request that already aborts a turn
today: a `VizierRequest` whose content is `Command("abort")`, on `agent.{id}.req`.

**Rationale**: that is exactly how a turn is aborted now - the agent's request loop matches
`VizierRequestContent::Command("abort")` and calls `abort_session_notify`
(`agents/process.rs:193-215`, `:73-100`). The abort therefore already travels on the request
subject, and a networked implementation needs no new mechanism at all.

**Alternatives considered**:
- *A new `abort` message kind on the command subject*: a second way to do one thing, which Principle
  II calls a defect.

---

## R5. The host is told what to hold; placement decides

**Decision**: `AgentHost` (worker side) owns the supervisors, the running instances, the builds and
the resumption of pending turns. `Placement` (control side) decides which agents the local host
holds; in a single node it returns every stored agent.

**Rationale**: FR-016 requires the host to be testable alone with no gateway running, which means it
must not read the agent list itself - it must be *told*. Today `VizierAgents::new` does both: it
lists every stored agent (`agents/mod.rs:111-116`) and runs them. Splitting it means the host is
constructed from a bus handle, a storage handle and a list of agents to hold, exactly like the other
Fleet roles (`crates/vizier-fleet/src/worker.rs`).

**Alternatives considered**:
- *Keep `VizierAgents` and add a `hold_only` flag*: the host would still own the decision, so it
  could not be built alone and the worker role would still need the control role.
- *Move the split into `vizier-fleet`*: the epic's table says core keeps the `AgentHost`/`Placement`
  split ("the seams that stay in core"), and `vizier-fleet` must not reach into core's internals.

---

## R6. Health and setup progress move to shared state - and the lifetime is soft

**Decision**: the host publishes `agent.{id}.health` and `agent.{id}.setup` to shared state. The
value is a JSON envelope `{"expires_at_ms": <ms>, "data": <payload>}`; the reader treats a missing
entry, or one whose deadline has passed, as *unknown*. The deadline lives **in the value**, not in
the backend.

**Rationale**: `SharedState` has no TTL. The trait is `get`/`put`/`cas`/`delete`/`watch` over
`Vec<u8>` with a revision (`vizier-schema/src/state.rs:33-49`); the in-process backend is a
`RwLock<HashMap>` (`vizier-schema/src/inprocess.rs:148-179`). FR-010 nonetheless requires a
lifetime of at most ten seconds, so the expiry has to be carried by the value. A soft deadline is
honest about what the contract can promise: the entry is *ignored* after its deadline, and the next
publish overwrites it.

Shared state is also the only place the two sides can meet for a fleet-wide view. A gateway cannot
ask "who owns agent X" (FR-007 forbids learning the owner), so it cannot assemble a health list by
asking each owner; and the bus has no snapshot, so a subscriber that connects after the last
publish would see nothing.

**Alternatives considered**:
- *Extend `SharedState` with `put_expiring(key, value, ttl)`*: this is the shape NATS KV has, and it
  is the right long-term shape. But it changes the leaf contract crate in the middle of the #162-#164
  conflict window, adds a conformance check, and makes every backend implement expiry. Recorded as
  the option to take in #165, when the NATS backend lands and needs a real TTL anyway.
- *Publish health on a bus subject*: no snapshot for a late subscriber, and every gateway would
  receive every agent's health.
- *Keep asking the manager for health*: impossible in a cluster - the gateway does not know the
  owners, and the manager's health check is a fleet-wide command on the global subject.

---

## R7. The deferred client-call claim is a lease on the stored row

**Decision**: `client_tool_call.status` keeps its vocabulary. The claim becomes two new columns,
`claimed_by` and `claimed_at`, and the claim is one conditional update:

```sql
UPDATE client_tool_call SET claimed_by = ?1, claimed_at = ?2
 WHERE history_uid = ?3 AND status = 'deferred'
   AND (claimed_by IS NULL OR claimed_at < ?4)
```

The row's own compare-and-swap: exactly one gateway's update changes a row, so exactly one gateway
wins. A claim older than the timeout is free again, which is what makes an abandoned claim
re-claimable.

**Rationale**: `ClientCallOutcome` is the vocabulary the WebUI reads
(`schema/history.rs:205-249`), and `from_columns` maps an unknown status to `interrupted`. A
`claimed` status would leak a transient state into a final-outcome vocabulary and show up in the
WebUI as "interrupted". A lease on the row keeps the outcome vocabulary intact, survives a restart,
and gives the lapse rule for free.

**Alternatives considered**:
- *A new `claimed` status*: rejected above.
- *The bus's `SharedState::cas` on a key*: the claim would not be durable, while the call it claims
  is a stored row; a gateway restart would release the claim without anyone noticing.
- *Keep the in-memory `DeferredClaims` set* (`agents/client/mod.rs:491-518`): two gateways would
  deliver the same call twice, and the person's client would see two answers for one call. That is
  the bug the requirement exists to close.

---

## R8. The mechanical check is a path-aware grep in the `policy` job

**Decision**: the check is a new step in `.github/workflows/ci.yml`'s `policy` job, next to "Roles
stay independent": it refuses a read of a running agent's state from `crates/vizier-http/src` or
`crates/vizier-core/src/service`, and names the file and line.

**Rationale**: FR-003 requires the check to exist and to be *proven to fail* on a deliberate
violation. The `policy` job already owns the "where may this code live" rules, and its existing
step is a path-aware grep that skips doc comments (`grep -nE "(crate|super)::(${others})\b"`). The
same shape applies here, and the proof is a test that adds a deliberate read and shows the check
failing.

**Alternatives considered**:
- *A Rust lint or a type-level fence*: the states live in unrelated types - `SetupBoard`,
  `ChannelBoard`, `DeferredClaims`, the supervisor's `processes` map - so a type fence would need a
  wrapper type per state. That is the refactor this issue is meant to make unnecessary.
- *A review checklist*: not mechanical, so "must be proven to fail" is unmet.

---

## R9. Two of the issue's named surfaces are not separate reads

**Decision**: the audit answers *tool and tool-category listings* and *MCP server status* rather than
inventing work for them. Both are recorded as "covered by the integration-status operation".

**Rationale**: the sweep of the gateway side found that:
- The only function that lists an agent's live tools is `ToolRouter::catalogue`
  (`agents/tools/mod.rs:919`), and every caller is inside the agent
  (`agents/agent/mod.rs:752`, `agents/tools/python/bridge.rs:58`). There is no gateway-side read of
  an agent's tools, and no `/agents/{id}/tools` route. The tool facts the gateway serves are the
  stored config flags in `AgentDetail` (`service/agents.rs:426-472`).
- MCP server status is read only through the running agent's integration status
  (`agents/mcp/mod.rs:109-125` -> `integrations/mod.rs:243` -> `integrations/service.rs:107-121` ->
  `service/integrations.rs:267`). The gateway's other `mcp` use is the grant switch, which is SQLite
  (`service/integrations.rs:392`, `:422`).

**Alternatives considered**:
- *Add a "tool listing" operation to the seam*: an operation no caller needs today.

---

## R10. Process-wide registries stay where they are

**Decision**: `deps.questions`, `deps.client_connections`, `deps.notifications`,
`deps.project_ledger`, `deps.side_chat_limits` and the HTTP adapter's own rate limiters stay in
place. The audit lists them and gives the reason.

**Rationale**: they are not per-agent running state. They are process-wide registries keyed by
session or by connection, and a person's client socket lives in the gateway process by definition -
the person is connected to *that* gateway. FR-012 already keeps the rate limiters per node, and the
epic's decision table says the same ("Rate limiters are per node in v1").

**Alternatives considered**:
- *Move them behind the seam*: they would have to be addressed by something other than an agent,
  breaking FR-007's addressing rule; and a socket cannot be in another process.

---

## R11. Availability keeps two reasons apart

**Decision**: the seam's failure has one kind - *agent unavailable* - with two reasons: **no owner**
(nobody is subscribed: the bus answers `NoResponders`) and **owner lost** (a responder existed but
the reply never came). Both map onto the existing `ErrorKind::AgentUnavailable`
(`schema/response.rs:69-70`, `:99`).

**Rationale**: the bus contract already draws the line: `BusError::NoResponders` is "a request found
nobody subscribed" and `BusError::Timeout` is "a responder existed but did not answer in time"
(`vizier-schema/src/error.rs:13-16`). The seam inherits the distinction for free, which is what
makes FR-014 (fail at once, never a timeout) and FR-015 (end, never hang) both hold. `ErrorKind` is
already the vocabulary the WebUI and the channels read, so nothing new is introduced for the person.

**Alternatives considered**:
- *One undifferentiated "unavailable"*: an operator could not tell "the fleet has not placed this
  agent yet" from "the worker died" - which FR-013 forbids.

---

## Resolved clarifications

No `NEEDS CLARIFICATION` marker survives in the Technical Context. The questions the spec left open,
and where they were answered:

| Question | Answer | Where |
|---|---|---|
| Where does the seam live? | `vizier-core`, stable tier, `agents_client.rs` | R1 |
| How do callers reach it? | `VizierDependencies.agents: Arc<dyn AgentsClient>` | R1 |
| Is the local implementation a direct call? | No - it is over the bus | R2 |
| Which subjects does the seam use? | `agent.{id}.req`, `agent.{id}.cmd` (new), `agents.cmd` for creates | R3 |
| How is a turn aborted? | The existing `Command("abort")` request | R4 |
| How does the host learn what to hold? | `Placement` tells it; single node = every stored agent | R5 |
| How does shared state expire? | The deadline is in the value; the backend has no TTL | R6 |
| How is a deferred claim made exclusive? | A conditional update on the stored row | R7 |
| How is the mechanical check built? | A path-aware grep in the CI `policy` job | R8 |
| What about tool listings and MCP status? | Not separate gateway-side reads; covered by integration status | R9 |
| What about the process-wide registries? | They stay; the audit records why | R10 |
| How are "no owner" and "owner lost" told apart? | `NoResponders` vs `Timeout` on the bus | R11 |
