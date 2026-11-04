# Implementation Plan: Fleet Agent Service Seam

**Branch**: `048-agent-service-seam` | **Date**: 2026-11-03 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/048-agent-service-seam/spec.md`

## Summary

Give the gateway side one way to reach an agent: a trait, `AgentsClient`, that `vizier-http` and
`core::service` call instead of reaching into the agent manager's memory. It has a local
implementation that reaches the manager over the in-process bus, and a bus implementation lands later
(#168/#169) against the same contract. Behind the seam, the agent manager splits into `AgentHost`
(worker side: builds and runs only the agents it is told to hold) and `Placement` (control side:
which host owns which agent; in one node, the one local host). Every gateway-side read of a running
agent's state is audited and given a home that survives a process boundary - a stored row, a shared
entry, or a request to the owner - and a CI check refuses a new direct read.

The technical approach is decided in [research.md](research.md); this plan states what is built, in
what order, and against which gates.

## Technical Context

**Language/Version**: Rust 1.99.1, edition 2024 (workspace `Cargo.toml`)

**Primary Dependencies**: none added. The work uses `tokio` (already in the tree), `async-trait`
(already used by the storage and bus traits), `serde`/`serde_json`, and the bus contract crate
`vizier-schema` (already a dependency of `vizier-core`).

**Storage**: unchanged. SQLite (bundled) and local files stay the default; PostgreSQL and S3 stay
opt-in. Two columns are added to the existing `client_tool_call` table
(`crates/vizier-core/src/storage/sqlite/mod.rs:613-633`).

**How the columns reach an existing database.** SQLite: `add_column_if_missing` at startup, the
pattern already used for `integration.creator` and `user.last_seen_at`
(`storage/sqlite/mod.rs:906`, `:440`). PostgreSQL: a **new** migration file
`migrations/0002_client_call_claim.sql`, appended to `MIGRATIONS`
(`storage/sql/postgres/migrate.rs:20-24`). The runner applies each file once and skips the ones
already applied (`:76-78`), and the module says outright that "an applied file is never edited" -
so `0001_initial.sql` is **not** edited. A deployment that already ran 0001 gets the columns from
0002; a deployment that runs 0002 before 0001 never happens, because the versions are ordered.

**Testing**: `cargo test` (workspace, with the per-crate gates below); `cargo clippy` with CI's
flags. End-to-end: the dummyplug agent turn for the agent-observable parts (the turn path), per the
constitution's e2e gate. The conformance suite (`vizier-schema`) and the offline end-to-end agent
turn are the regression gates for the bus traffic changes.

**Target Platform**: Linux server, macOS and Windows (the desktop app is unaffected and stays single
node).

**Project Type**: server workspace - `vizier-core` (engine), `vizier-http` (HTTP adapter),
`vizier-schema` (bus contract), `vizier-fleet` (roles).

**Performance Goals**: no request may get slower by more than a bus hop. A request to an agent with
no owner must fail within 1 s (SC-005); a request whose owner is lost mid-flight must end within
5 s (SC-006); shared-state views are never more than 10 s old (SC-008).

**Constraints**: the public HTTP API's shapes do not change (FR-019); no new configuration
(FR-020); no new external service (Principle III); the single node behaves exactly as before
(FR-021, FR-022); the seam never exposes which host owns an agent (FR-007).

**Scale/Scope**: one feature, four crates touched (`vizier-schema`, `vizier-core`, `vizier-http`,
CI). No new crate, no new dependency, no new configuration key, no WebUI change.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

**Principle I - Lean by Default: PASS, with one entry in Complexity Tracking.**
No new crate, no new third-party dependency, no new configuration. The one abstraction added - the
`AgentsClient` trait - is required by FR-002 and has more than one implementation in this issue
alone: the local one, and the fake the gateway-side test runs against (FR-016). The bus
implementation is not hypothetical: it is sub-issues #168 and #169 in the epic's own list. See
Complexity Tracking.

**Principle II - DRY via Trait-Based Extibility: PASS.**
The seam is one trait with implementations behind it, not a `match` over a type tag. The
host/placement split is two components with one job each. Shared-state publication is one publisher
in the host, not a `put` call scattered over call sites. The one place a type tag would have been
tempted - "local or remote" - is a trait dispatch, which is what Principle II asks for.

**Principle III - Self-Contained Default: PASS.**
No new required external service. Shared state in a single node is the in-process bus's key/value
store, which already ships. SQLite and local files remain the default. The desktop app is untouched.

**Principle IV - Portability: PASS.**
No OS-specific behaviour, no new filesystem or process assumptions. The claim's expiry uses the
clock already in use (`chrono::Utc::now()` in the storage layer).

**Principle V - Unified Errors & Observability: PASS.**
The seam returns `crate::Result<T>` (`VizierError`), with the bus error converted through the one
existing `From<BusError> for VizierError` (`crates/vizier-core/src/error.rs:23-26`). The two
availability reasons ride on the existing `ErrorKind::AgentUnavailable` rather than a new error type.
All logging goes through `tracing`.

**Workflow gates: PASS.**
`cargo clippy` and `cargo test` must pass; conventional commits; the branch name matches the spec
folder. The change is agent-observable (the turn path), so the dummyplug end-to-end agent turn runs
before it is done. No submodule branch is needed: the work touches no submodule. (The Fleet specs
#160, #161 and #162 created no submodule branches either.)

**Post-design re-check (after Phase 1): PASS.** The design added no new dependency, no new crate and
no new external service; the two additions to `vizier-schema` are one subject constructor and two
contract documents. The Complexity Tracking entry below is unchanged.

## Project Structure

### Documentation (this feature)

```text
specs/048-agent-service-seam/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
│   ├── agents-client.md         # the seam: operations, errors, subjects
│   ├── agent-host-placement.md  # what the host holds and who decides
│   ├── shared-state-views.md    # the published views, their keys and their lifetime
│   └── agent-state-audit.md     # every gateway-side read and where it comes from now
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
crates/vizier-schema/src/
├── subject.rs           # + agent.{id}.cmd constructor (R3)
└── contracts/
    ├── gateway_http.rs  # + the gateway's subjects and leases (FR-018)
    └── worker.rs        # + the worker's subjects and leases (FR-018)

crates/vizier-core/src/
├── lib.rs               # + pub mod agents_client (stable tier); prelude export
├── agents_client.rs     # NEW: the AgentsClient trait, its local implementation, the fake
├── agents/
│   ├── mod.rs           # VizierAgents -> AgentHost (+ the fleet-wide command part)
│   └── placement.rs     # NEW: the Placement trait and the single-node implementation
├── dependencies.rs      # + pub agents: Arc<dyn AgentsClient>
├── setup.rs             # the setup board also publishes to shared state
└── storage/
    ├── client_call.rs   # + claim_client_call / release_client_call_claim
    └── sqlite/client_call.rs   # + the conditional update, the two columns

crates/vizier-http/src/
└── (no new module; the handlers call deps.agents)

crates/vizier-fleet/src/
└── testing.rs           # the harness gains a fake AgentsClient (FR-016)

.github/workflows/ci.yml # + the "no direct read of a running agent's state" policy step
```

**Structure Decision**: the seam is a new top-level module in `vizier-core` rather than a module
under `agents/`, because it is the *gateway's* client, not the agent manager's business - and
because `vizier-fleet` must implement it without reaching into core's internals. `AgentHost` stays
in `agents/` (it is the manager, renamed and narrowed) and `Placement` is a sibling module, because
placement is control-side and the host is worker-side; keeping them in separate files keeps the
split visible in the tree.

## Work breakdown

Six slices, each one commit, in dependency order. Slices 1 and 2 are the contract; 3 and 4 are the
seam and the split; 5 is the audit; 6 is the mechanical check.

| # | Slice | Lands | Proves |
|---|-------|-------|--------|
| 1 | The seam's contract: the `agent.{id}.cmd` subject, the `AgentsClient` trait, its error vocabulary | `vizier-schema`, `vizier-core` | The contract compiles and validates; the subject matches a real pattern |
| 2 | The local implementation and the fake | `vizier-core`, `vizier-fleet` | The gateway-side test runs with no host; the fake answers |
| 3 | The host/placement split | `vizier-core` | Single node unchanged; the host runs alone with a scripted request source |
| 4 | Health and setup in shared state; the deferred claim on a stored row | `vizier-core` | A gateway reads the views; two gateways claim one call once |
| 5 | The audit | `specs/048-agent-service-seam/contracts/agent-state-audit.md` | Every gateway-side read has a named replacement |
| 6 | The mechanical check | `.github/workflows/ci.yml` | A deliberate direct read fails the check and is named |

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|------------------------------------|
| A trait (`AgentsClient`) introduced while only one production implementation exists | FR-002 requires the seam to have a local *and* a bus implementation against one contract, and FR-017 requires the contract to be written so the later implementation can be built from it alone. This issue also ships the second implementation: the fake the gateway-side test runs against (FR-016). Principle I allows an abstraction once the second real use case shows up - here it shows up in the same issue, and the bus one is sub-issue #168/#169 in the epic. | One concrete `LocalAgentsClient` with no trait: the gateway side would then be typed to it, and the remote implementation would have to change every call site - the exact churn FR-002 exists to prevent. |
| One new subject constructor in `vizier-schema` (`agent.{id}.cmd`) | FR-007 requires the seam to address the agent, never a host. A per-agent subject is the only address that routes to exactly one handler. Without it, every worker would receive every command and the wrong one would consume it (delivery is at-most-once). | Keeping commands on the fleet-wide `agents.cmd`: rejected in research R3 - the command is consumed by a non-owner and lost. |
| Two new columns on `client_tool_call` | FR-011 requires the deferred claim to be held in a stored row guarded by compare-and-swap, so exactly one gateway claims a call and an abandoned claim becomes claimable again. A lease on the row is the smallest durable form. | A new `claimed` status: it would leak a transient state into `ClientCallOutcome`, the final-outcome vocabulary the WebUI reads, where an unknown status reads as `interrupted` (research R7). |

---

## Requirement coverage

Every requirement in [spec.md](spec.md) and where this plan satisfies it.

| Requirement | Satisfied by |
|---|---|
| FR-001 | [contracts/agents-client.md](contracts/agents-client.md) - the trait's nine operations |
| FR-002 | [contracts/agents-client.md](contracts/agents-client.md) - "The two implementations"; research R1, R2 |
| FR-003 | [contracts/agent-state-audit.md](contracts/agent-state-audit.md) - "The mechanical check"; slice 6 |
| FR-004 | [contracts/agent-host-placement.md](contracts/agent-host-placement.md) - `AgentHost`; slice 3 |
| FR-005 | [contracts/agent-host-placement.md](contracts/agent-host-placement.md) - `SingleNodePlacement` |
| FR-006 | [contracts/agent-host-placement.md](contracts/agent-host-placement.md) - rule 4; the quickstart's guarantee table |
| FR-007 | [contracts/agents-client.md](contracts/agents-client.md) - "Addressing" rules 1-2; research R3 |
| FR-008 | [contracts/agent-state-audit.md](contracts/agent-state-audit.md) - the audit table |
| FR-009 | [contracts/agent-state-audit.md](contracts/agent-state-audit.md) - the audit table's surfaces |
| FR-010 | [contracts/shared-state-views.md](contracts/shared-state-views.md) - the value and the reading rules |
| FR-011 | [data-model.md](data-model.md) - "Deferred client call"; research R7 |
| FR-012 | [contracts/agent-state-audit.md](contracts/agent-state-audit.md) - the rate-limiter row |
| FR-013 | [contracts/agents-client.md](contracts/agents-client.md) - "Errors"; research R11 |
| FR-014 | [contracts/agents-client.md](contracts/agents-client.md) - "Errors", first row |
| FR-015 | [contracts/agents-client.md](contracts/agents-client.md) - "Errors", second row |
| FR-016 | Slice 2's fake and slice 3's host test; [contracts/agent-host-placement.md](contracts/agent-host-placement.md) - "Testable alone" |
| FR-017 | [contracts/agents-client.md](contracts/agents-client.md) - the whole document |
| FR-018 | Slice 1 - the gateway and worker role contracts in `vizier-schema` |
| FR-019 | The Technical Context's constraints; no WebUI work in any slice |
| FR-020 | The Technical Context's constraints; no configuration change in any slice |
| FR-021 | Slice 3's "the existing agent tests pass **without being modified**"; the quickstart's gates |
| FR-022 | Slices 3 and 4 change the bus traffic, so the conformance suite and the offline end-to-end turn are the gates |

| Criterion | Proved by |
|---|---|
| SC-001 | The quickstart's gate list: `cargo test --workspace`, the conformance suite and the offline end-to-end turn, unchanged |
| SC-002 | The audit's completeness, checked row by row; slice 6's check bites and is named |
| SC-003 | Slice 6's proof: the deliberate read is refused and named |
| SC-004 | Slice 2's gateway-side test and slice 3's host test, both in CI |
| SC-005 | The "Errors" table's first row: `NoResponders` answers at once |
| SC-006 | The "Errors" table's second row: the request deadline ends the request |
| SC-007 | Slice 4's claim test: two claims on one call, one row changed |
| SC-008 | The published view's 10 s deadline, and the reader's "unknown" for an expired entry |
| SC-009 | Slice 3's guarantee table and the unchanged API shapes |
| SC-010 | The audit table stands alone: every row names the old source and the new one |
| SC-011 | No slice adds a configuration key; `vizier run` with no settings behaves as before |
