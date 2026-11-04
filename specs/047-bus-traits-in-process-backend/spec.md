# Feature Specification: Fleet Message Bus, In-Process Backend and Role Test Harness

**Feature Branch**: `047-vizier-schema`

**Created**: 2026-11-03

**Status**: Draft

**Input**: User description: "https://github.com/vizier-lab/vizier/issues/162 — Fleet: vizier-schema, bus traits, in-process backend and role test harness"

## Overview

Vizier's subsystems talk to each other over one in-process bus, and that bus is the only coupling between them. To let the Fleet roles (gateway, worker, channels, control) later run in separate processes, the bus must become a contract that any transport can implement, with no caller tied to a particular broker, and every role must be testable by itself against a stand-in for its peers.

This feature delivers that contract, a default in-process implementation that keeps today's single-process behaviour exactly, and the test tooling that makes each later role independently verifiable. It does not introduce a networked backend or move any role into its own process.

**Delivery state at the time of writing**: none of this is on `develop`. There is no contract crate, no bus trait and no bus documentation, and the transport still runs on private channels. An earlier local attempt was discarded, so this spec describes the whole capability and planning scopes all of it.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Everything Vizier does today still works over the bus (Priority: P1)

An operator running Vizier as a single process sees no difference after the bus becomes a contract: agents answer, background jobs report back, sessions stream to open WebUI tabs and channels, integrations refresh, and shutdown behaves as before. Internally, every message between subsystems travels through the bus contract instead of private channels.

**Why this priority**: The contract is worthless unless the existing product runs on it unchanged. This is the regression guard for the whole Fleet epic.

**Independent Test**: Run the full existing test suite and the offline end-to-end agent turn (no provider keys needed) against the in-process backend; all pass with no test modified to accommodate the change.

**Acceptance Scenarios**:

1. **Given** a running single-process Vizier, **when** a person sends a chat message, **then** the agent's reply frames stream back in the same order and shape as before.
2. **Given** a request sent to an agent that is not registered, **when** it is submitted, **then** it fails immediately rather than after a timeout.
3. **Given** an agent update or restart, **when** requests are queued, **then** they are served by the next instance and none are lost.
4. **Given** a background job, task run or dream cycle, **when** it fires, **then** it completes and reports exactly as before.
5. **Given** an integration change or a session event, **when** it is published, **then** every interested subscriber in the process receives it once.
6. **Given** a request that carries trace context, **when** it crosses the bus, **then** the receiving side continues the same trace and still reports how long the request waited in the queue.

---

### User Story 2 - A role author tests a role alone (Priority: P1)

A developer building or changing one Fleet role (for example the worker) writes a test that builds only that role on an in-process bus with embedded storage, plays its neighbours with a scripted peer that publishes and asserts on the role's contract subjects, and verifies behaviour without starting any other role.

**Why this priority**: Every later Fleet sub-issue depends on testing a role in isolation; without the harness each would need the whole system running.

**Independent Test**: Run the sample role test from CI: it builds one role from the harness, drives it with a scripted peer, and asserts on what it publishes.

**Acceptance Scenarios**:

1. **Given** the harness, **when** a developer builds a role, **then** it receives only the bus and storage handles and never learns which backend sits behind them.
2. **Given** a scripted peer, **when** it publishes a message on a subject the role subscribes to, **then** the role reacts and the peer can assert on the role's published reply.
3. **Given** a written guide, **when** a new contributor follows it, **then** they can add a role test without reading harness internals.

---

### User Story 3 - A backend author proves a backend correct (Priority: P2)

A developer implementing another backend (for example a networked broker) runs one reusable conformance suite against it. A backend that breaks any stated rule fails a named check.

**Why this priority**: The semantics (ordering, no hanging, fencing) are what make multi-process operation safe; they must be checkable by anyone, not just by the people who wrote the first backend.

**Independent Test**: Run the suite against the in-process backend (all checks pass) and against a deliberately faulty backend (the matching check fails).

**Acceptance Scenarios**:

1. **Given** a backend, **when** the suite runs, **then** it checks request/reply, the no-responder error, per-subject ordering, wildcard matching, payload-limit refusal, lease expiry and takeover, fencing-token monotonicity, lease watching and shared-state compare-and-swap.
2. **Given** a backend that answers a request with no subscribers using a timeout instead of the no-responder error, **when** the suite runs, **then** the matching check fails.
3. **Given** a request with no reply, **when** the timeout elapses, **then** the caller receives an error and never hangs.

---

### User Story 4 - Leader work stops when the lease is lost (Priority: P2)

A role that holds a lease (such as a leader role) learns promptly when it can no longer renew it and stops its work at once, and the next holder always carries a higher fencing token so stale work from the old holder can be rejected.

**Why this priority**: Prevents two processes acting as leader later; it must be a property of the contract, not of one backend.

**Independent Test**: Acquire a lease, let it expire, take it over from a second holder, and check the second token is higher and the first holder's renewal fails.

**Acceptance Scenarios**:

1. **Given** a held lease, **when** renewal fails, **then** the holder is told and must stop immediately.
2. **Given** two successive holders, **when** the second acquires after expiry, **then** its fencing token is strictly greater than the first's.
3. **Given** a watcher on a lease, **when** ownership changes, **then** the watcher is notified.

---

### User Story 5 - Maintainers read the contract (Priority: P3)

A maintainer or integrator reads one documentation page that states the bus semantics (at-most-once delivery, ordering per subject and publisher, request error behaviour, lease behaviour, payload limit, subject patterns, capabilities) and lists, per role, the subjects it subscribes to, publishes and the leases it takes.

**Why this priority**: The semantics are only a contract if they are written down next to the code.

**Independent Test**: A reviewer finds each stated semantic on the page and finds a matching conformance check or test for it.

**Acceptance Scenarios**:

1. **Given** the bus contract page, **when** read, **then** every semantic in the requirements below is stated.
2. **Given** a role, **when** its contract file is opened, **then** it lists its subjects and leases (initially empty skeletons that later sub-issues fill in).

---

### Edge Cases

- A payload larger than the limit is refused with a clear error, never truncated or sent.
- An identifier containing a separator character becomes one subject token and cannot widen a subscription or inject extra levels.
- A tail wildcard anywhere but the last position, or a wildcard mixed into a token, is rejected as an invalid pattern.
- A backend lacking a capability a role needs: startup refuses that role with a message naming the capability.
- A vanished agent no longer closes a background piece instantly; the piece ends at its own time limit instead (a known, accepted behaviour change; a faster path is a separate issue).
- A client-tool binding whose connection is not on this node is treated as a closed connection: deferrable tools defer, others report the client as disconnected.
- A reply that arrives before the requester has finished subscribing must not be lost.
- A subscriber that falls behind or a subject with no subscribers must not stall publishers.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST define the wire contract (validated subjects, subject patterns, the versioned envelope with kind, reply address and trace headers, error types and the payload limit) in a standalone crate that depends on none of Vizier's other crates and on no broker or web framework.
- **FR-002**: The system MUST define three capability-split bus contracts: messaging (request, publish, subscribe), leases (acquire, renew, release, watch, each lease carrying a fencing token) and shared state (get, put, compare-and-swap, delete, watch), so a backend implements only what it can.
- **FR-003**: No public signature of the contract or of the engine MAY mention a specific broker's types or syntax.
- **FR-004**: Subjects MUST be built from identifiers and validated; subject patterns MUST support exact match, a one-level wildcard and a tail wildcard in the last position only.
- **FR-005**: The contract MUST state: delivery is at-most-once and ordered per subject and publisher; a request returns an error on timeout or no responder and never hangs; a failed lease renewal means the holder stops work immediately; fencing tokens are strictly increasing across holders; the payload limit is part of the contract.
- **FR-006**: Backends MUST advertise their capabilities, and startup MUST refuse a role that needs a capability the configured backend lacks.
- **FR-007**: The system MUST provide an in-process backend implementing all three contracts and preserving today's single-process behaviour.
- **FR-008**: All inter-subsystem traffic currently carried by private channels MUST travel over the bus on the in-process backend: agent requests and reply streams, agent lifecycle commands, file commands, memory operations, dream commands, process commands, session events and integration-change events. The process-exit signal is lifecycle of one operating-system process, not traffic between subsystems, and stays process-local.
- **FR-009**: Messages that hold in-process-only handles (reply senders, one-shot response slots, live trace spans, live client connections) MUST be replaced by serialisable equivalents: a reply address, a request/reply pair with a serialisable result, W3C trace-context headers with a queued timestamp, and a node-plus-connection-id binding resolved locally.
- **FR-010**: Trace continuity and queue-wait measurement MUST survive a bus hop.
- **FR-011**: A request to an unregistered agent MUST fail at once, and requests queued during an agent restart MUST be served by the next instance.
- **FR-012**: The system MUST provide a reusable backend conformance suite that any backend runs, covering request/reply, no-responder error, per-subject ordering, wildcard matching, payload-limit refusal, lease expiry and takeover, fencing-token monotonicity, lease watch and compare-and-swap; the suite itself MUST be proven to fail a backend that breaks a rule.
- **FR-013**: Each Fleet role MUST have a bus contract file listing the subjects it subscribes to, publishes and the leases it takes, with a validation entry point; later role work fills them in.
- **FR-014**: The system MUST provide a role test harness: an in-process bus, embedded storage and local blob storage, a helper that builds a role from bus and storage handles, and a scripted peer that publishes and asserts on contract subjects.
- **FR-015**: A sample role test using the harness MUST run in CI, and a short guide MUST explain how to write one.
- **FR-016**: The bus contract page MUST document the semantics in FR-005, subject patterns, capabilities and the per-role contracts, and live beside the contract code.
- **FR-017**: Continuous-integration policy checks MUST enforce that the contract crate depends on none of Vizier's crates and on no broker, web framework or CLI crate.
- **FR-018**: Every existing automated test MUST pass without modification caused by this change, other than tests that pinned a deliberately changed behaviour, which MUST be rewritten to pin the new behaviour.

### Key Entities

- **Subject**: A validated, dot-separated address built from identifiers (agent requests, session events, lease names, gateway client addresses). Identifiers containing separators are encoded into a single token.
- **Subject Pattern**: A subscription filter over subjects: exact, one-level wildcard, or trailing tail wildcard.
- **Envelope**: The versioned message wrapper carrying kind, optional reply address, trace headers, queue timestamp and payload, bounded by the payload limit.
- **Capabilities**: A descriptor of which of messaging, leases and shared state a backend supports.
- **Lease**: A time-limited exclusive claim on a named duty, carrying a monotonically increasing fencing token.
- **Bus**: The single handle a role receives, bundling the three contracts so the role never learns which backend is behind it.
- **Role Contract**: A per-role declaration of subjects subscribed, subjects published and leases taken.
- **Scripted Peer**: A test double that plays another role by publishing and asserting on contract subjects.
- **Client Binding**: A reference from a request to the gateway node and connection that declared client-side tools, resolved locally to a live connection or treated as closed.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of the pre-existing automated tests pass after the change, except tests rewritten for the one documented behaviour change.
- **SC-002**: The in-process backend passes all conformance checks, and a deliberately faulty backend fails at least the check matching its fault.
- **SC-003**: Zero private point-to-point channels remain between subsystems; every inter-subsystem message type is carried on the bus.
- **SC-004**: A developer can write and run a new role test using only the guide and the harness, with no other role running, in a single test file.
- **SC-005**: The offline end-to-end agent turn completes with the same observable replies as before the change.
- **SC-006**: Searching the public signatures of the contract and engine finds no broker-specific types or syntax.
- **SC-007**: A queued request survives an agent restart in 100% of tested restarts.
- **SC-008**: The continuous-integration policy check fails if the contract crate gains a forbidden dependency.

## Assumptions

- This feature is the first of the Fleet epic's core-touching sub-issues and does not depend on the others; sub-issues for the clock/gate and for individual roles proceed in parallel or after it.
- A networked backend, a gateway subject for client tools, and each role's real contract content are out of scope and belong to later sub-issues.
- Single-process behaviour is the only supported deployment until a networked backend exists; the in-process backend therefore needs no persistence or cross-process guarantees.
- The payload limit is 1 MiB, as already decided for the contract.
- Background pieces ending at their time limit after a vanished agent, instead of instantly, is accepted; a faster explicit terminal signal is a separate issue.
- Wire reply types for the remaining command channels are small serialisable enums, since arbitrary error values cannot be serialised.
- Tests that cannot run on the contributor's platform (managed storage needing Postgres and an object store; Windows-only pre-existing failures) are validated in CI.
