# Feature Specification: Fleet Agent Service Seam

**Feature Branch**: `048-agent-service-seam`

**Created**: 2026-11-03

**Status**: Draft

**Input**: User description: "https://github.com/vizier-lab/vizier/issues/163 - Fleet: agent service seam (AgentsClient, AgentHost, Placement)"

## Overview

Vizier's HTTP interface and service layer reach into the agent manager in the same process: the manager registers every stored agent's request channel and runs a supervisor per agent. That is fine for one process and impossible for a cluster. A gateway on one machine has to be able to reach an agent that runs on another, and it must never read a running agent's memory to answer a question about it.

This feature puts one seam between the gateway side and the agent side - the agent client (`AgentsClient`). It has a local implementation - today's single process, unchanged - and a bus implementation that reaches the agent's owner, which a later issue delivers. The agent manager splits into a worker-side agent host (`AgentHost`), which builds and runs only the agents it has been told to hold, and a control-side placement (`Placement`), which decides which host owns which agent. Every place the HTTP interface and the service layer reads agent-side state is audited and given a home that survives a process boundary: a stored row, a shared entry, or a request to the agent's owner.

It does not start a second process, does not move any agent, and does not change what a person can do.

**Delivery state at the time of writing**: the bus contract, the in-process backend and the role test harness landed with the previous Fleet issue (#162) and are on `develop`. Nothing of this feature is on `develop` yet: there is no seam, the agent manager is still one piece, and the HTTP interface still reads the manager's memory directly.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Single node behaves exactly as before (Priority: P1)

An operator running Vizier as one process sees no difference after the seam lands: agents answer, are created, updated, deleted and restarted; health and setup progress are shown; deferred client calls are delivered; and the WebUI is unchanged. Internally, the gateway side reaches agents only through the seam.

**Why this priority**: The seam is worthless unless the existing product runs on it unchanged. This is the regression guard for the whole Fleet epic.

**Independent Test**: Run the full automated test suite, the conformance suite and the offline end-to-end agent turn; all pass with no test changed to accommodate the work.

**Acceptance Scenarios**:

1. **Given** a running single node, **when** a person sends a chat message, **then** the reply frames arrive in the same order and shape as before.
2. **Given** an agent in the WebUI, **when** it is created, updated, deleted or restarted, **then** the response, and any error, are the same as before.
3. **Given** an agent that is still being set up, **when** the WebUI polls its setup progress, **then** the progress shown is the same as before.
4. **Given** a request for an agent that does not exist, **when** it is submitted, **then** it fails at once rather than after a timeout.
5. **Given** a background job, a task run or a dream cycle, **when** it fires, **then** it completes and reports exactly as before.
6. **Given** the HTTP interface, **when** its routes are compared with the previous release, **then** the request and response shapes are identical.

---

### User Story 2 - The gateway side reaches agents only through the seam (Priority: P1)

A developer working on the HTTP interface or the service layer never asks "which process runs this agent?" and never reads a running agent's state. They call the seam for the agent's operations, and read stored configuration, shared state or the agent's owner for everything else. A mechanical check refuses the change when a new direct read appears.

**Why this priority**: This is the point of the issue. Without it a gateway can never run in another process, and every later Fleet issue inherits the same coupling.

**Independent Test**: Add a deliberate direct read of a running agent's state to the HTTP interface and show the check fails and names the read; remove it and the check passes.

**Acceptance Scenarios**:

1. **Given** the HTTP interface or the service layer, **when** it performs an operation on an agent, **then** it goes through the seam.
2. **Given** a check that forbids reading a running agent's state from outside the process that owns it, **when** such a read is added, **then** the check fails and names where the read is.
3. **Given** the audit record, **when** a reviewer reads it, **then** every entry names where that state comes from now: a stored row, a shared entry, or a request to the agent's owner.

---

### User Story 3 - Placement decides who owns an agent; a host runs only what it is told (Priority: P1)

The control side decides which node owns which agent; in one node it assigns every agent to the one local host. The worker-side host builds and runs only the agents it has been told to hold, and it can be built and driven on its own, with no gateway and no other role running.

**Why this priority**: The split is the mechanical prerequisite for putting gateways and workers in different processes, and a host that cannot run alone cannot be tested alone.

**Independent Test**: Build a host with the in-process bus and embedded storage, hand it one agent, drive it with a scripted request source, and assert on the frames it produces. No gateway is running.

**Acceptance Scenarios**:

1. **Given** a single node, **when** placement runs, **then** every stored agent is assigned to the one local host and runs as before.
2. **Given** a host told to hold two agents, **when** it starts, **then** it builds and runs exactly those two.
3. **Given** a host that is told to let an agent go, **when** it does, **then** the agent's running turns end the way a delete ends them today.
4. **Given** a host running alone, **when** it is asked about an agent it does not hold, **then** it refuses with the agent-unavailable error rather than building the agent on the spot.

---

### User Story 4 - A gateway is tested without any worker (Priority: P2)

A developer tests the gateway side against a fake seam: no agent host, no bus, no storage beyond a stub. The fake records what the gateway asked for and answers with scripted results, so the gateway's behaviour can be checked on its own.

**Why this priority**: Every later role needs a seam that can be replaced by a fake. Without it, the gateway can only be tested with the whole system running.

**Independent Test**: Run the gateway-side test with no agent host running; it passes.

**Acceptance Scenarios**:

1. **Given** a fake seam, **when** the gateway sends a request, **then** the fake receives the agent's request and can answer with frames.
2. **Given** a fake seam that answers "unavailable", **when** the gateway reports it, **then** the person sees the same unavailable error they see today.
3. **Given** the seam's written contract, **when** a second implementation of it is written from that contract alone, **then** the gateway side needs no change to use it.

---

### User Story 5 - "No owner yet" and "owner lost" stay apart (Priority: P2)

When an agent has no owner, the caller is told so at once. When the owner disappears in the middle of a request, the request ends with the unavailable error instead of hanging. The two are told apart, because one is a normal, temporary state and the other is a failure an operator has to see.

**Why this priority**: The seam's error vocabulary is what every later role depends on. Collapsing the two cases makes a cluster impossible to operate.

**Independent Test**: Ask for an agent that has no owner (the answer comes back at once), then lose the owner mid-request (the request ends with the unavailable error), and check that the two answers differ.

**Acceptance Scenarios**:

1. **Given** an agent with no owner, **when** a request is sent, **then** it fails at once, saying there is no owner yet.
2. **Given** an owner that disappears while a request is in flight, **when** the caller waits, **then** the request ends saying the owner was lost, rather than hanging.
3. **Given** either case, **when** the error reaches the person, **then** it is still the agent-unavailable kind.

---

### User Story 6 - A deferred client call is claimed once (Priority: P2)

A tool call that waits for a person's client socket is recorded durably. When two gateways both see it pending, exactly one claims it and the other finds nothing to do. A claim that is abandoned goes back to pending for the next gateway.

**Why this priority**: The claim lives in one process's memory today. In a cluster, two gateways would answer the same call twice and the person's client would see two answers for one call.

**Independent Test**: Two gateways look for the same pending call; one wins; the loser finds no work; the winner's abandoned claim returns to pending.

**Acceptance Scenarios**:

1. **Given** two gateways and one pending deferred call, **when** both look for work, **then** exactly one claims it.
2. **Given** a gateway that claimed a call and then died, **when** another gateway looks later, **then** the call is pending again.

---

### User Story 7 - Maintainers read one record of what moved where (Priority: P3)

The audit is a single document: every read of a running agent's state, where it lived before, and where it lives now. It is the reference the later worker and gateway issues build on.

**Why this priority**: The audit is the deliverable that keeps the cluster work honest. It matters most once other people start changing the same files.

**Independent Test**: A reviewer takes each entry and finds the replacement in the code.

**Acceptance Scenarios**:

1. **Given** the audit record, **when** it is read, **then** every entry names the old source and the new one.
2. **Given** an entry whose replacement is a shared entry, **when** it is read, **then** the entry states how long the entry lives.
3. **Given** an entry with no replacement yet, **when** it is read, **then** it is marked as still open, not left blank.
4. **Given** an entry for a state that deliberately stays where it is, **when** it is read, **then** the entry says so and says why.
5. **Given** the gateway and worker role contracts, **when** they are read, **then** each states the subjects it uses and the leases it takes after the split.

---

### Edge Cases

- An agent with no owner yet is unavailable at once; it is never a timeout.
- The owner disappears between "available" and the request arriving: the request is served by the next owner or refused as unavailable, never silently dropped.
- The owner disappears while a request is in flight: the request ends with the unavailable error and the turn is not left hanging.
- A request arrives while the agent is restarting: it is queued and served after the restart, as today.
- An update arrives while a turn is running: the running turn resumes.
- A delete arrives while a turn is running: the turn is dropped, as today.
- A create whose build fails: the agent is not persisted, and the caller is told why.
- Two gateways send requests to the same agent at once: both are served; the seam does not queue them behind each other.
- Health or setup progress that is older than its lifetime: the gateway reports it as unknown rather than as a stale value.
- A deferred client call claimed by a gateway that then dies: the claim lapses and the call becomes claimable again.
- An agent that no host holds: refused with the unavailable error, never built on the spot by a gateway.
- A tool listing or a tool-category listing for an agent that is not running: answered from stored configuration, not from a running agent.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST define one agent seam (`AgentsClient`) that covers, at minimum: sending a request to an agent and receiving its frames until the turn ends; aborting a request in flight; creating, updating, deleting and restarting an agent; asking for an agent's health; asking for an agent's integration status; and asking an agent to refresh its integrations. Every operation the service layer and the HTTP interface perform on an agent today MUST be reachable through it.
- **FR-002**: The seam MUST have a local implementation that reaches the in-process agent manager, and a bus implementation that reaches the agent's owner across the bus. Both MUST satisfy one contract. The bus implementation is delivered by the later worker and gateway issues; the contract and its local side land now.
- **FR-003**: No code outside the agent host may read a running agent's in-memory state. A mechanical check MUST refuse a change that introduces such a read, and MUST be proven to fail on a deliberate violation.
- **FR-004**: The agent manager MUST be split into an agent host (`AgentHost`) - the worker side, which builds and runs only the agents it is told to hold, and owns their supervisors, their running instances and their resumption - and a placement (`Placement`) - the control side, which decides which host owns which agent.
- **FR-005**: In a single node, placement MUST assign every stored agent to the one local host, and every agent MUST behave exactly as it does today.
- **FR-006**: The guarantees that hold today MUST continue to hold through the split: an agent is built before it is persisted or swapped; requests queued while an agent restarts are not lost; an update resumes the running turns; a delete drops them.
- **FR-007**: The seam MUST address the agent and never a host, a node or a process. A caller MUST NOT be able to choose, or learn, which host owns an agent.
- **FR-008**: The system MUST keep an audit of every place the HTTP interface and the service layer read agent-side state. Each entry MUST name the old source and its replacement, and the replacement MUST be one of: a stored row, a shared entry, or a request to the agent's owner.
- **FR-009**: The audit MUST cover, at minimum: agent health and the agent listing; setup progress; channel connection health; tool and tool-category listings; MCP server status; deferred client-call claims; and rate limiters.
- **FR-010**: Agent health and setup progress MUST be published to shared state with a lifetime of at most ten seconds, and MUST be read from there by the HTTP interface and the service layer. A read that finds nothing, or an entry that has expired, MUST report "unknown" rather than a stale value.
- **FR-011**: Deferred client-call claims MUST be held in a stored row guarded by compare-and-swap, so that exactly one gateway claims a given call, and a claim abandoned by a dead gateway becomes claimable again.
- **FR-012**: Rate limiters MUST stay per node in this release. The audit MUST record this as a deliberate decision, and say why, rather than leaving it unexamined.
- **FR-013**: Agent availability MUST keep its meaning across the seam, and MUST distinguish "no owner yet" from "owner lost".
- **FR-014**: A request to an agent with no owner MUST fail at once with the unavailable error; it MUST NOT wait for a timeout.
- **FR-015**: A request whose owner disappears while it is in flight MUST end with the unavailable error and MUST NOT hang.
- **FR-016**: The gateway side MUST be testable against a fake seam with no agent host running, and the agent host MUST be testable alone with a scripted request source and no gateway running. Both tests MUST run in continuous integration.
- **FR-017**: The seam's contract MUST be written down where a later bus implementation can be built from it without reading the local one, and MUST state every operation it promises and every error it can return.
- **FR-018**: The role contracts for the gateway and worker roles MUST record what each side owns after the split: which subjects it uses and which leases it takes.
- **FR-019**: The public HTTP API's request and response shapes MUST be unchanged, so that the WebUI needs no change.
- **FR-020**: No new configuration MUST be required: running Vizier with no configuration MUST behave as before.
- **FR-021**: Every existing automated test MUST pass without modification, except tests that pinned a behaviour this work deliberately replaces; those MUST be rewritten to pin the new behaviour.
- **FR-022**: The conformance suite and the offline end-to-end agent turn MUST still pass unchanged.

### Key Entities

- **Agent**: the unit a person talks to. It has an id, a stored configuration, and at any moment exactly one owner.
- **Owner**: the host that currently holds an agent. In this release there is one owner per agent; an agent with no owner is waiting for placement.
- **Agent host**: the worker-side component that builds and runs the agents it has been told to hold, and is the only thing allowed to read a running agent's state.
- **Placement**: the control-side decision of which host owns which agent. In a single node it assigns every agent to the one local host.
- **Agent request**: a message sent to an agent, answered by a stream of frames until the turn ends.
- **Availability**: whether an agent can be reached now, and if not, whether it has no owner yet or has lost its owner.
- **Deferred client call**: a tool call that waits for a person's client to answer it; claimable by exactly one gateway at a time.
- **Audit entry**: one read of agent-side state, the source it had before, and the source it has now.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Every automated test in the repository passes, with no test modified to accommodate this work except those that pinned a behaviour it deliberately replaces; the conformance suite and the offline end-to-end agent turn pass unchanged.
- **SC-002**: Every agent-side read in the HTTP interface and the service layer appears in the audit with a named replacement; the mechanical check fails when one deliberate direct read is added and passes when it is removed.
- **SC-003**: No code outside the agent host reads a running agent's in-memory state, proven by the mechanical check refusing a deliberate violation.
- **SC-004**: A gateway-side test of the agent operations passes with no agent host running, and a host-side test passes with no gateway running; both run in continuous integration.
- **SC-005**: A request to an agent with no owner is answered with the unavailable error within one second.
- **SC-006**: A request whose owner is lost mid-flight ends with the unavailable error within five seconds.
- **SC-007**: Two gateways looking for the same deferred client call result in exactly one claim; the loser finds no work.
- **SC-008**: Health and setup progress shown by the WebUI are never more than ten seconds old, and an entry that has expired is shown as unknown rather than as a stale value.
- **SC-009**: The operations a person can trigger on an agent from the WebUI - chat, create, update, delete, restart, health, integration status - behave the same as before: the same requests, the same responses and the same error kinds.
- **SC-010**: A reviewer can confirm, from the audit alone, where every audited state now comes from, without asking the author.
- **SC-011**: The single node starts with no new settings and serves the same API and WebUI as before.

## Assumptions

- The bus contract, the in-process backend and the role test harness landed with the previous Fleet issue (#162) and are on `develop`; they are the basis for the bus side of the seam.
- The bus implementation of the seam lands in the later worker and gateway issues (#168, #169). This work delivers the seam, its local side, the split and the audit.
- Single node stays the default and unchanged, and the desktop app is not affected.
- Authorization stays in core: the seam is a transport seam, not an authorization boundary. Callers authorize before they call, and the seam does not become a second place where authorization is decided.
- One owner per agent. Sharding an agent across hosts stays out of scope.
- Rate limiters stay per node in this release, following the Fleet decision, so the audit records them rather than moving them.
- The audit covers reads of *running* agent state. A gateway reading stored configuration from the database is already correct and is out of scope.
- The guarantees referenced as #52 (a delete drops the running turns) and #43 (requests queued during a restart are not lost) keep their current meaning.
- The public HTTP API's shapes stay as they are, so the WebUI needs no change and no client needs to adapt.
- "Short lifetime" for shared state means seconds, not minutes: the values are a live view of a running agent, not a record.
