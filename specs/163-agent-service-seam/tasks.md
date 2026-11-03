---
description: "Task list for the Fleet agent service seam (#163)"
---

# Tasks: Fleet Agent Service Seam

**Input**: Design documents from `/specs/163-agent-service-seam/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Included. The specification requires them explicitly - FR-016 ("the gateway side MUST be
testable against a fake seam with no agent host running, and the agent host MUST be testable alone
with a scripted request source and no gateway running. Both tests MUST run in continuous
integration"), FR-003 ("the check MUST be proven to fail on a deliberate violation") and FR-021/022
(existing tests, the conformance suite and the offline end-to-end turn must pass unchanged).

**Organization**: Tasks are grouped by user story so each story can be implemented, tested and
delivered independently.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story this task belongs to (US1-US7, from spec.md)
- Include exact file paths in descriptions

## Path Conventions

- Rust workspace: `crates/vizier-{schema,core,http,fleet}/src/`, integration tests in
  `crates/vizier-{core,http}/tests/`
- Policy scripts: `scripts/NNN-name.sh`
- CI: `.github/workflows/ci.yml`
- Docs: `docs/src/fleet/`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Establish the baseline the work is measured against, and confirm the constraints the
plan puts on it.

- [ ] T001 Record the test baseline in `specs/163-agent-service-seam/quickstart.md`: run `cargo test -p vizier-core --lib`, `cargo test -p vizier-schema` and `cargo test -p vizier-fleet`, and write down which tests fail on this branch. The two Windows-only failures that are NOT yours are `projects::path::tests::absolute_escaping_and_missing_paths_are_refused` and `skill::scopes::tests::a_slug_in_both_project_roots_appears_once_and_agents_wins`; any other failure is yours to fix before continuing
- [ ] T002 [P] Confirm the no-new-dependency rule in `crates/vizier-schema/Cargo.toml`, `crates/vizier-core/Cargo.toml`, `crates/vizier-http/Cargo.toml` and `crates/vizier-fleet/Cargo.toml`: diff them against `develop` and check that the only changes are `path`/`version` entries for workspace crates. No third-party crate is added by this feature (plan.md, Principle I)
- [ ] T003 [P] Confirm the two unchanged gates in `crates/vizier-fleet/tests/roles.rs` and `crates/vizier-http/tests/conformance/main.rs` pass on this branch before any code changes - these are the FR-022 gates and they must still pass at the end

**Checkpoint**: The baseline is written down and the constraints hold.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The seam's contract - the trait, its error vocabulary, the subject it addresses, and
the wiring that makes it reachable. Nothing else can start without it.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [ ] T004 [P] Add `Subject::agent_cmd(id)` to `crates/vizier-schema/src/subject.rs`, returning `agent.{id}.cmd` with the id percent-encoded into exactly one token - beside `agent_req` (`:104`) and `agent_memop` (`:109`), using the same `with_id` helper
- [ ] T005 [P] Add the subject tests to the `mod tests` block of `crates/vizier-schema/src/subject.rs`: `Subject::agent_cmd("my.agent")` is the three tokens `agent.my%2Eagent.cmd`, it matches the pattern `agent.*.cmd`, it does not match `agent.*.req`, and `Subject::decode_token` round-trips the id
- [ ] T006 Create `crates/vizier-core/src/agents_client.rs` with the `AgentsClient` trait: nine object-safe async methods - `request`, `abort`, `create`, `update`, `delete`, `restart`, `health`, `integration_status`, `refresh_integrations` - each returning `crate::Result<T>` (`VizierError`). The signatures are in `specs/163-agent-service-seam/contracts/agents-client.md`; the trait must be `Send + Sync`
- [ ] T007 Add the availability error to `crates/vizier-core/src/agents_client.rs`: an error type carrying `ErrorKind::AgentUnavailable` (`crates/vizier-core/src/schema/response.rs:69-70`) and the reason - `NoOwner` or `OwnerLost`. The kind must stay the one the WebUI and the channels read today; only the reason is new
- [ ] T008 Register the module in `crates/vizier-core/src/lib.rs`: add `pub mod agents_client;` to the stable tier beside `pub mod transport;` (`:77`) and export the trait and its error from the `prelude` block (`:52-68`)
- [ ] T009 Add `pub agents: Arc<dyn AgentsClient>` to `VizierDependencies` in `crates/vizier-core/src/dependencies.rs` (the struct at `:46-88`) and initialise it in the constructor (`:292-316`). `VizierDependencies` is built in exactly one place, so this is the only construction site to touch
- [ ] T010 [P] Fill in the gateway's contract in `crates/vizier-schema/src/contracts/gateway_http.rs`: it publishes `agent.*.req` and `agent.*.cmd` and takes no lease. Note in the file's doc comment that a cluster gateway also uses the session and integration subjects the transport already carries
- [ ] T011 [P] Fill in the worker's contract in `crates/vizier-schema/src/contracts/worker.rs`: it subscribes to `agent.*.req`, `agent.*.cmd` and `agents.cmd`, and takes `lease.agent.*`. Note in the doc comment that `agents.cmd` (the fleet-wide create) belongs to the control role in a cluster and stays with the local host in single node - that handover is #166's work, not this issue's
- [ ] T012 Update the skeleton note at the top of `crates/vizier-schema/src/contracts/mod.rs` (`:3-5`): the gateway and worker contracts are no longer empty, so the "every constant is empty today" sentence is now wrong
- [ ] T013 Run `cargo test -p vizier-schema` and `cargo test -p vizier-core --lib agents_client` - the contract in `crates/vizier-core/src/agents_client.rs` compiles, the subject tests in `crates/vizier-schema/src/subject.rs` pass and `RoleContract::validate` accepts the two filled-in contracts in `crates/vizier-schema/src/contracts/`

**Checkpoint**: The seam's contract exists and is testable - the foundation every story needs.

---

## Phase 3: User Story 2 - The gateway side reaches agents only through the seam (Priority: P1)

**Goal**: `vizier-http` and `core::service` perform every agent operation through `AgentsClient`,
and a mechanical check refuses a new direct read of a running agent's state.

**Independent Test**: Run the gateway-side test with no agent host running; it passes. Add a
deliberate direct read to `crates/vizier-core/src/service/`, run the check, and it fails naming the
file and line; remove the read and it passes again.

### Tests for User Story 2

- [ ] T014 [P] [US2] Write the seam's contract test in the `mod tests` block of `crates/vizier-core/src/agents_client.rs`: a `FakeAgentsClient` records the calls it receives and answers from a script; assert that `health("alpha")` carries the agent id and returns the scripted status, and that `update` carries the config it was given
- [ ] T015 [P] [US2] Write the failing test that proves the mechanical check bites: add `scripts/163-agent-state-read-check.sh` with a deliberate direct read of `deps.channel_board` in `crates/vizier-core/src/service/agents.rs`, assert the script exits non-zero and names that file and line, then remove the deliberate read and assert it exits zero. The check must be shown failing - a check never seen to fail is not a check

### Implementation for User Story 2

- [ ] T016 [US2] Implement `LocalAgentsClient` in `crates/vizier-core/src/agents_client.rs`: `request` and `abort` over `agent.{id}.req`, using `VizierTransport`'s existing request and reply-channel path (`crates/vizier-core/src/transport/mod.rs:329`, `:257`). `abort` sends the request whose content is `Command("abort")` - the same request that aborts a turn today (`crates/vizier-core/src/agents/process.rs:193-215`) - so no new message kind is needed
- [ ] T017 [US2] Implement the command operations of `LocalAgentsClient` in `crates/vizier-core/src/agents_client.rs` (`create`, `update`, `delete`, `restart`, `health`, `integration_status`, `refresh_integrations`), each addressing `agent.{id}.cmd` except `create`, which addresses `agents.cmd`. The trait must not expose a host, a node or a lease
- [ ] T018 [US2] Add the per-agent command wire types to `crates/vizier-core/src/transport/wire.rs`: extend `AgentCommandRequest` (`:97`) and `AgentCommandReply` (`:108`) so an agent-scoped command carries its agent id, and add the `KIND_*` constants beside `KIND_AGENT_COMMAND` (`:90`)
- [ ] T019 [US2] Teach the transport to address a single agent in `crates/vizier-core/src/transport/mod.rs`: add `send_agent_command_to(agent_id, cmd)` beside `send_agent_command` (`:462`), publishing on `Subject::agent_cmd(agent_id)` instead of `Subject::agents_cmd()`, and map `BusError::NoResponders` to *agent unavailable / no owner* and `BusError::Timeout` to *agent unavailable / owner lost*
- [ ] T020 [US2] Serve the per-agent command subject from the agent side in `crates/vizier-core/src/agents/mod.rs`: the manager subscribes to `agent.{id}.cmd` for each agent it holds, beside the fleet-wide `agents.cmd` subscription it has today (`crates/vizier-core/src/transport/mod.rs:485`), and answers with that agent's health, integration status or command result
- [ ] T021 [US2] Route the service layer through the seam in `crates/vizier-core/src/service/agents.rs`: the per-agent reads and writes - the ping endpoint (`:815`), the restart endpoint (`:644`) and the integration status (via `crates/vizier-core/src/service/integrations.rs:267`) - go through `deps().agents` instead of the manager's memory. The request and response shapes must not change (FR-019)
- [ ] T022 [US2] Route the HTTP adapter through the seam in `crates/vizier-http/src/api/v1/agents/mod.rs`: the handlers call `service::agents::*`, which now calls the seam - no handler reads a running agent's state. `HTTPState` already holds a `VizierDependencies` (`crates/vizier-http/src/state.rs:29`), so no new plumbing is needed
- [ ] T023 [US2] Write the policy script `scripts/163-agent-state-read-check.sh`, following the house pattern of `scripts/037-baseline-check.sh` (shebang, `set -euo pipefail`, `cd "$(dirname "$0")/.."`, exit non-zero on a finding): it scans `crates/vizier-http/src` and `crates/vizier-core/src/service` for reads of the handles only the agent host may touch - the supervisor's process map, the setup board's live snapshot, the channel board's status lookup and the deferred-claims registry - and prints the file and line of each one
- [ ] T024 [US2] Add the check to the `policy` job in `.github/workflows/ci.yml`, beside the "Roles stay independent" step (`:409`), running `bash scripts/163-agent-state-read-check.sh` and failing the job on a non-zero exit
- [ ] T025 [US2] Run the seam's contract test and the gateway-side test: `cargo test -p vizier-core --lib agents_client` (the test in `crates/vizier-core/src/agents_client.rs`) and `cargo test -p vizier-http --test agents_seam` (the test in `crates/vizier-http/tests/agents_seam.rs`)

**Checkpoint**: US2 is complete - the gateway side reaches agents only through the seam, and a
deliberate direct read is refused and named.

---

## Phase 4: User Story 3 - Placement decides who owns an agent; a host runs only what it is told (Priority: P1)

**Goal**: The agent manager splits into `AgentHost` (worker side) and `Placement` (control side). The
host runs only the agents it is told to hold, and can be built and driven on its own.

**Independent Test**: Build a host with the in-process bus and embedded storage, hand it one agent,
drive it with a scripted request source, and assert on the frames it produces. No gateway is
running.

### Tests for User Story 3

- [ ] T026 [P] [US3] Write the host's standalone test in the new file `crates/vizier-core/tests/agent_host.rs`: build an `AgentHost` on a `VizierTransport` with one agent, drive it with a scripted request source, and assert that the frames it produces are the ones the request asked for - with no gateway and no other role running
- [ ] T027 [P] [US3] Write the test that a host refuses an agent it does not hold, in `crates/vizier-core/tests/agent_host.rs`: ask the host about an agent it was not given and assert the answer is *agent unavailable*, not a freshly built agent

### Implementation for User Story 3

- [ ] T028 [US3] Create `crates/vizier-core/src/agents/placement.rs` with the `Placement` trait and `SingleNodePlacement`: `assign()` returns every stored agent with the one local host as its owner (FR-005). Placement is the only component that may answer "which node owns this agent", and it answers only to the control role, never to a gateway (FR-007)
- [ ] T029 [US3] Split the manager in `crates/vizier-core/src/agents/mod.rs`: rename `VizierAgents` to `AgentHost` and remove its decision of *which* agents to run - the loop that lists every stored agent and starts it (`agents/mod.rs:111-116`) moves to `Placement`. The host keeps the supervisors, the running instances, the builds and the resumption of pending turns
- [ ] T030 [US3] Give the host `take` and `release` in `crates/vizier-core/src/agents/mod.rs`: `take` subscribes to the agent's subjects and builds it; `release` ends the agent's running turns the way a delete ends them today. The guarantees hold (FR-006): built before persisted or swapped, requests queued during a restart are not lost, an update resumes the running turns, a delete drops them
- [ ] T031 [US3] Wire the split in `crates/vizier-core/src/runtime.rs`: the runtime asks `Placement` what the local host should hold (`:183`) and hands that list to `AgentHost`. The four subsystems and their order must not change - `crates/vizier-core/tests/roles.rs:80` pins them
- [ ] T032 [US3] Run `cargo test -p vizier-core --lib agents` and `cargo test -p vizier-core --test agent_host` (the new file `crates/vizier-core/tests/agent_host.rs`): the existing agent tests in `crates/vizier-core/src/agents/mod.rs` pass **without being modified**, and the new host tests pass

**Checkpoint**: US3 is complete - the host runs alone, and placement decides what it holds.

---

## Phase 5: User Story 1 - Single node behaves exactly as before (Priority: P1)

**Goal**: Every read the gateway side used to make of a running agent's memory now comes from a
published view that carries the same information, so the WebUI and the API show exactly what they
show today.

**Independent Test**: Run the full automated test suite, the conformance suite and the offline
end-to-end agent turn; all pass with no test changed to accommodate the work.

### Tests for User Story 1

- [ ] T033 [P] [US1] Write the view's tests in `crates/vizier-core/src/agents_client.rs`: a missing entry reads as *unknown*, an expired entry reads as *unknown* (not as a stale value), and a live entry reads as its payload
- [ ] T034 [P] [US1] Write the test that the view carries everything the reader used to get, in `crates/vizier-core/src/agents_client.rs`: the published health payload contains the same fields `AgentStatus::health` produces (`crates/vizier-core/src/agents/supervisor.rs:226-233`), including the channel-reader health it folds in

### Implementation for User Story 1

- [ ] T035 [US1] Add the published view type in `crates/vizier-core/src/agents_client.rs`: a JSON envelope `{"expires_at_ms": <ms>, "data": <payload>}` with the deadline **in the value**, because `SharedState` has no TTL (`crates/vizier-schema/src/state.rs:33-49`). The deadline is at most 10 s after the publish (FR-010, SC-008)
- [ ] T036 [US1] Publish the health view from the agent side in `crates/vizier-core/src/agents/supervisor.rs`: the host publishes `agent.{id}.health` **when it takes the agent** and **whenever that agent's state changes** - not on a timer (`specs/163-agent-service-seam/contracts/shared-state-views.md`, rules 5-7). Without this, single node would show *unknown* for a running agent
- [ ] T037 [US1] Publish the setup view from the agent side in `crates/vizier-core/src/setup.rs`: the setup board keeps its in-memory form and publishes a copy to `agent.{id}.setup` (the board is the one publisher - Principle II)
- [ ] T038 [US1] Read the published views from the gateway side in `crates/vizier-core/src/service/agents.rs`: the agent list (`:346-356`), the health endpoint (`:799`) and `crates/vizier-core/src/service/overview.rs:70` read the views instead of the manager's memory. A missing or expired view reads as *unknown*, never as a stale value
- [ ] T039 [US1] Read the published setup view in `crates/vizier-core/src/service/setup_state.rs:14`: the setup-state endpoint the WebUI polls reads `agent.{id}.setup` from shared state instead of `deps.setup.snapshot()`. The person's authorisation filter (`:19-34`) stays as it is
- [ ] T040 [US1] Run the parity gate: `cargo test --workspace`, `cargo test -p vizier-schema` and `cargo test -p vizier-http --test conformance` (`crates/vizier-http/tests/conformance/main.rs`). Every test passes **without being modified** except those that pinned a behaviour this work deliberately replaces (FR-021, FR-022)

**Checkpoint**: US1 is complete - single node shows exactly what it showed before.

---

## Phase 6: User Story 4 - A gateway is tested without any worker (Priority: P2)

**Goal**: The gateway side can be tested against a fake seam, with no agent host, no bus and no
storage beyond a stub.

**Independent Test**: Run the gateway-side test with no agent host running; it passes.

### Tests for User Story 4

- [ ] T041 [P] [US4] Write the gateway-side test in the new file `crates/vizier-http/tests/agents_seam.rs`: build the HTTP state with a fake seam, ask for an agent's health, and assert the fake received the agent id the gateway sent and that its answer reached the response
- [ ] T042 [P] [US4] Write the test that an "unavailable" answer reaches the person unchanged, in `crates/vizier-http/tests/agents_seam.rs`: the fake answers *unavailable*, and the response carries the same error kind the gateway returns today

### Implementation for User Story 4

- [ ] T043 [US4] Publish the fake seam from `crates/vizier-core/src/agents_client.rs`: a `FakeAgentsClient` that records what it was asked for and answers from a script, usable by `vizier-http`'s tests and by the Fleet harness
- [ ] T044 [US4] Add the fake to the Fleet harness in `crates/vizier-fleet/src/testing.rs`: `Harness` gains a `fake_agents()` that returns one, so a role test can stand in for the agent side
- [ ] T045 [US4] Run `cargo test -p vizier-http --test agents_seam` (`crates/vizier-http/tests/agents_seam.rs`) and `cargo test -p vizier-fleet --test roles` (`crates/vizier-fleet/tests/roles.rs`): both pass with no agent host running

**Checkpoint**: US4 is complete - the gateway side is testable alone.

---

## Phase 7: User Story 5 - "No owner yet" and "owner lost" stay apart (Priority: P2)

**Goal**: A request to an agent with no owner fails at once; a request whose owner disappears
mid-flight ends with the unavailable error rather than hanging; the two are told apart.

**Independent Test**: Ask for an agent with no owner (the answer comes back at once), then lose the
owner mid-request (the request ends with the unavailable error), and check that the two answers
differ.

### Tests for User Story 5

- [ ] T046 [P] [US5] Write the no-owner test in `crates/vizier-core/src/agents_client.rs`: a request to an agent nobody subscribes to returns *agent unavailable / no owner* within one second (SC-005), and never as a timeout
- [ ] T047 [P] [US5] Write the owner-lost test in `crates/vizier-core/src/agents_client.rs`: a responder that never answers ends the request with *agent unavailable / owner lost* within five seconds (SC-006), and the request never hangs

### Implementation for User Story 5

- [ ] T048 [US5] Map the two bus failures to the two reasons in `crates/vizier-core/src/agents_client.rs`: `BusError::NoResponders` is *no owner* and `BusError::Timeout` is *owner lost* (`crates/vizier-schema/src/error.rs:13-16`). The reason must be in the error, not only in the log (FR-013)
- [ ] T049 [US5] Carry the reason to the person in `crates/vizier-core/src/service/agents.rs`: the service layer maps the seam's failure onto the existing `ServiceError` shape, so the HTTP status and the error kind the WebUI reads are the ones it reads today
- [ ] T050 [US5] Run `cargo test -p vizier-core --lib agents_client`: both availability tests in `crates/vizier-core/src/agents_client.rs` pass

**Checkpoint**: US5 is complete - the two reasons are told apart, and neither hangs.

---

## Phase 8: User Story 6 - A deferred client call is claimed once (Priority: P2)

**Goal**: Two gateways looking for the same deferred client call result in exactly one claim, and a
claim abandoned by a dead gateway becomes claimable again.

**Independent Test**: Two gateways look for the same pending call; one wins; the loser finds no
work; the winner's abandoned claim returns to pending.

### Tests for User Story 6

- [ ] T051 [P] [US6] Write the two-gateway test in `crates/vizier-core/src/storage/sqlite/client_call.rs`: two claim attempts on one deferred call, and exactly one of them changes the row (SC-007)
- [ ] T052 [P] [US6] Write the lapsed-claim test in `crates/vizier-core/src/storage/sqlite/client_call.rs`: a claim older than the timeout is free again, and the call is still `deferred`

### Implementation for User Story 6

- [ ] T053 [US6] Add the claim methods to the storage trait in `crates/vizier-core/src/storage/client_call.rs`: `claim_client_call(uid, claimant, timeout)` and `release_client_call_claim(uid, claimant)`, beside the existing `defer_client_call` (`:31`). The claim is a lease on the row, **not** a new status - `ClientCallOutcome` is the final-outcome vocabulary the WebUI reads, and an unknown status reads as `interrupted` (`crates/vizier-core/src/schema/history.rs:230-249`)
- [ ] T054 [US6] Implement the conditional update in `crates/vizier-core/src/storage/sqlite/client_call.rs`: one statement that sets `claimed_by` and `claimed_at` only where the status is `deferred` and the row is unclaimed or its claim has lapsed. Exactly one gateway's statement changes a row, so exactly one claims it
- [ ] T055 [US6] Add the two columns to SQLite in `crates/vizier-core/src/storage/sqlite/mod.rs`: `add_column_if_missing(conn, "client_tool_call", "claimed_by", "TEXT")` and the same for `claimed_at`, beside the table's schema init (`:613-633`) - the pattern already used for `integration.creator` (`:906`)
- [ ] T056 [US6] Add the PostgreSQL migration in the new file `crates/vizier-core/src/storage/sql/postgres/migrations/0002_client_call_claim.sql` and register it in `MIGRATIONS` in `crates/vizier-core/src/storage/sql/postgres/migrate.rs` (`:20-24`). **`0001_initial.sql` is never edited** - the runner skips applied versions (`:76-78`), so editing it would leave an existing deployment without the columns
- [ ] T057 [US6] Use the stored row instead of the in-memory set in `crates/vizier-core/src/agents/client/mod.rs`: the `DeferredClaims` registry (`:491-518`) is replaced by the claim on the row, and `deliver_deferred` (`:523`) takes the claim from storage
- [ ] T058 [US6] Update the HTTP topic in `crates/vizier-http/src/topic.rs` (`:101`, `:110`, `:443`) and `crates/vizier-http/src/api/v1/agents/channel.rs` (`:417`, `:425`, `:439-446`): the claim comes from storage, and `HTTPState.deferred_client_calls` (`crates/vizier-http/src/state.rs:38`) goes away
- [ ] T059 [US6] Run `cargo test -p vizier-core --lib client_call` and `cargo test -p vizier-core --lib storage::sqlite::client_call`: both claim tests in `crates/vizier-core/src/storage/sqlite/client_call.rs` pass

**Checkpoint**: US6 is complete - one claim per call, and an abandoned claim lapses.

---

## Phase 9: User Story 7 - Maintainers read one record of what moved where (Priority: P3)

**Goal**: The audit names every gateway-side read of a running agent's state, the source it had
before and the source it has now, so a reviewer can confirm the cluster work is honest.

**Independent Test**: A reviewer takes each entry and finds the replacement in the code.

### Tests for User Story 7

- [ ] T060 [P] [US7] Write the audit's completeness check into `scripts/163-agent-state-read-check.sh`: it also reports how many entries the audit file has and how many the check's handle list covers, so a new handle without an audit row is visible

### Implementation for User Story 7

- [ ] T061 [US7] Re-verify every row of `specs/163-agent-service-seam/contracts/agent-state-audit.md` against the code as it stands after Phases 3-8, and correct any row whose file:line or classification has moved. Every row must name the old source, the new one and the kind of state
- [ ] T062 [US7] Resolve or re-confirm the one open row in `specs/163-agent-service-seam/contracts/agent-state-audit.md` - the channel-reader health - and state its owner (#167's scope) or its replacement. An entry with no replacement stays **marked open**, never blank
- [ ] T063 [US7] Record in `specs/163-agent-service-seam/contracts/agent-state-audit.md` the entries that deliberately stay where they are - the rate limiters (per node, FR-012), the client's open sockets, the questions registry and the project read ledger - with the reason each one stays, so a later reader does not re-open them
- [ ] T064 [US7] Update the bus contract page `docs/src/fleet/bus-contract.md` with the new per-agent command subject, the two availability reasons, and the published views' keys and lifetime

**Checkpoint**: US7 is complete - the record stands alone.

---

## Phase 10: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories.

- [ ] T065 [P] Refresh the stable-tier API snapshot `crates/vizier-core/public-api.txt` for the new `agents_client` module (the stable tier is `crates/vizier-core/src/lib.rs:70-79`); regenerate with `cargo public-api -p vizier-core --simplified` and commit the result in the same change
- [ ] T066 [P] Check the new code against the constitution's rules in `crates/vizier-core/src/agents_client.rs` and `crates/vizier-core/src/agents/placement.rs`: one error type (`crate::Result`), logging through `tracing` only, no `unwrap()` outside tests, no type-tag branching where a trait dispatch belongs
- [ ] T067 Run `cargo clippy --workspace --all-targets -- -D warnings` over `crates/vizier-schema/`, `crates/vizier-core/`, `crates/vizier-http/` and `crates/vizier-fleet/`, and fix every warning this change introduced - the new code is in `crates/vizier-core/src/agents_client.rs`, `crates/vizier-core/src/agents/placement.rs` and `crates/vizier-schema/src/subject.rs`
- [ ] T068 Run the full gate over `crates/`: `cargo test --workspace`, `cargo test -p vizier-schema`, `cargo test -p vizier-fleet` and `cargo test -p vizier-http --test conformance` (`crates/vizier-http/tests/conformance/main.rs`). Every test passes, with no test modified except those that pinned a behaviour this work deliberately replaces
- [ ] T069 Run the agent-observable end-to-end check with a dummyplug agent on a running binary (the constitution's e2e gate): walk the dummyplug steps in `specs/163-agent-service-seam/quickstart.md` and in `scripts/dummyplug-vizier-e2e.mjs`
- [ ] T070 Run `scripts/163-agent-state-read-check.sh` one last time and record its output in `specs/163-agent-service-seam/quickstart.md`
- [ ] T071 Commit the work in conventional-commit format on the branch `163-agent-service-seam`, one slice per commit (the plan's six slices), staging by path (`crates/`, `.github/workflows/ci.yml`, `scripts/163-agent-state-read-check.sh`) - never `git add -A`, which would sweep `app/src-tauri/gen/schemas`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phase 3+)**: All depend on Foundational phase completion
- **Polish (Phase 10)**: Depends on all desired user stories being complete

### User Story Dependencies

- **US2 (P1)**: Can start after Foundational - no dependency on other stories. It builds the seam the
  others use, so it goes first
- **US3 (P1)**: Can start after Foundational. It needs the per-agent command subject from US2's
  `T018`/`T019` to serve `agent.{id}.cmd`, so run it after US2 or in parallel with T016-T017 only
- **US1 (P1)**: Depends on US2 and US3. Its content is the published views that keep the fleet-wide
  reads working *after* the gateway side moved behind the seam - the agent list cannot read the
  views before they exist. It is the verification phase for the whole feature
- **US4 (P2)**: Depends on US2 (the fake implements the seam). Independent of US1, US3, US5, US6, US7
- **US5 (P2)**: Depends on US2 (the error vocabulary). Independent of the rest
- **US6 (P2)**: Independent of the other stories - it is a storage change plus the HTTP topic
- **US7 (P3)**: Depends on US1-US6 - the audit records what they changed

### Within Each User Story

- Tests are written and FAIL before implementation
- The contract before the implementation that honours it
- The storage before the code that reads it
- Story complete before moving to the next priority

### Parallel Opportunities

- All Setup tasks marked [P] can run in parallel
- All Foundational tasks marked [P] can run in parallel (within Phase 2)
- Once Foundational completes, US4, US5 and US6 can proceed in parallel with each other
- All tests for a user story marked [P] can run in parallel
- The two role-contract files in Phase 2 marked [P] can run in parallel
- US3's implementation can run in parallel with US4's tests

---

## Parallel Example: User Story 2

```bash
# Launch the seam's tests together:
Task: "T014 Write the seam's contract test in crates/vizier-core/src/agents_client.rs"
Task: "T015 Write the failing test that proves the mechanical check bites"

# Launch the contract files together:
Task: "T010 Fill in the gateway's contract in crates/vizier-schema/src/contracts/gateway_http.rs"
Task: "T011 Fill in the worker's contract in crates/vizier-schema/src/contracts/worker.rs"
```

---

## Parallel Example: User Story 6

```bash
# Launch the storage work together:
Task: "T053 Add the claim methods to the storage trait in crates/vizier-core/src/storage/client_call.rs"
Task: "T054 Implement the conditional update in crates/vizier-core/src/storage/sqlite/client_call.rs"
Task: "T056 Add the PostgreSQL migration in crates/vizier-core/src/storage/sql/postgres/migrations/0002_client_call_claim.sql"
```

---

## Implementation Strategy

### MVP First (User Stories 2, 3 and 1)

US1 alone delivers nothing - it is the "nothing breaks" story, and it can only be verified once the
seam and the split exist. The smallest useful increment is therefore **US2 + US3 + US1**:

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL - blocks all stories)
3. Complete Phase 3: User Story 2 - the seam
4. Complete Phase 4: User Story 3 - the host and placement split
5. Complete Phase 5: User Story 1 - the published views and the parity gate
6. **STOP and VALIDATE**: the whole suite, the conformance suite and the offline end-to-end turn pass
   unchanged; the WebUI shows the same agent list, the same health and the same setup progress

### Incremental Delivery

1. Complete Setup + Foundational -> the contract exists
2. Add US2 -> the gateway side reaches agents only through the seam -> test independently
3. Add US3 -> the host runs only what it is told -> test independently
4. Add US1 -> the published views keep single node identical -> test independently (MVP!)
5. Add US4 -> the gateway side is testable without a worker
6. Add US5 -> the two availability reasons are told apart
7. Add US6 -> a deferred call is claimed once
8. Add US7 -> the audit records what moved where
9. Each story adds value without breaking previous stories

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together
2. Once Foundational is done:
   - Developer A: User Story 2 (the seam and the mechanical check)
   - Developer B: User Story 6 (the storage change - independent of everything else)
3. After US2 lands:
   - Developer A: User Story 3 (the host and placement split)
   - Developer B: User Story 5 (the availability reasons)
4. After US3 lands:
   - Developer A: User Story 1 (the published views)
   - Developer B: User Story 4 (the gateway-side test)
5. US7 last, once everything it records has landed

---

## Notes

- **`[P]`** tasks = different files, no dependencies
- **`[Story]`** label maps task to specific user story for traceability
- Each user story should be independently completable and testable
- Verify tests fail before implementing
- Commit after each task or logical group, in conventional-commit format (`just commit "feat: ..."`
  commits the submodules and the workspace together) - but stage by path, never `git add -A`
- Stop at any checkpoint to validate story independently
- Avoid: vague tasks, same file conflicts, cross-story dependencies that break independence
- **Do not edit `crates/vizier-core/src/storage/sql/postgres/migrations/0001_initial.sql`** - an
  applied migration is never edited; add a new numbered file instead
- No submodule branch is needed: the WebUI does not change (FR-019)
