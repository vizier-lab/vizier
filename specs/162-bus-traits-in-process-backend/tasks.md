---

description: "Task list for the Fleet message bus, in-process backend and role test harness"
---

# Tasks: Fleet Message Bus, In-Process Backend and Role Test Harness

**Input**: Design documents from `/specs/162-bus-traits-in-process-backend/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Requested by the spec (FR-012, FR-015, SC-001–SC-008): the conformance suite, the faulty-backend proof, the sample role test, the transport tests and the dummyplug end-to-end are all in scope and appear as tasks.

**Organization**: One pull request on `162-vizier-schema`, **one commit per slice** (plan.md, *Delivery slices*). Phases below are grouped by user story; each ends with a **COMMIT** task naming the slice and its conventional-commit message. Every commit must pass `cargo test --workspace` and `cargo clippy --workspace --all-targets` before the next phase starts.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: US1–US5 from spec.md; none for Setup, Foundational and Polish
- Paths are repository-relative. `core` = `crates/vizier-core/src`, `schema` = `crates/vizier-schema/src`, `fleet` = `crates/vizier-fleet/src`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: The new crate exists, builds, and CI knows about it.

- [X] T001 Create `crates/vizier-schema/Cargo.toml` (workspace package fields, `description`, `license`, `include`; dependencies only `serde`, `serde_json`, `tokio` with `sync`/`time`/`rt`/`macros`, `async-trait`, `uuid`; dev-dependency `tokio` with `test-util`) and an empty `crates/vizier-schema/src/lib.rs` with a crate-level doc comment stating the dependency rule
- [X] T002 Add `vizier-schema = { path = "crates/vizier-schema", version = "<workspace version>" }` to `[workspace.dependencies]` in `Cargo.toml` and add `vizier-schema.workspace = true` to `crates/vizier-core/Cargo.toml` and `crates/vizier-fleet/Cargo.toml`; run `cargo build --workspace` to refresh `Cargo.lock`
- [X] T003 [P] Add `crates/vizier-schema/**` to the `rust` and `core` path filters of the `changes` job in `.github/workflows/ci.yml` so a change to it runs every job (it is a shared crate)
- [X] T004 [P] Add a `crates/vizier-schema/README.md` (one paragraph: what the crate is, that it depends on no workspace crate) and list it in the crate's `include`

**Checkpoint**: `cargo build --workspace` succeeds with the empty crate.

---

## Phase 2: Foundational (Blocking Prerequisites) — slice 1: the contract

**Purpose**: The vocabulary every other task uses. No consumer yet. Covers FR-001–FR-006.

**⚠️ CRITICAL**: No user-story work begins until this phase is committed.

- [X] T005 [P] Implement `BusError` (variants per data-model.md: `PayloadTooLarge{size,limit}`, `NoResponders`, `Timeout`, `InvalidSubject`, `InvalidPattern`, `Closed`, `Unsupported(Capability)`, `LeaseHeld`, `LeaseLost`, `Conflict`, `Decode`) with hand-written `Display` and `std::error::Error` in `crates/vizier-schema/src/error.rs`
- [X] T006 [P] Implement `Capability`, `Capabilities { messaging, leases, shared_state }`, `missing(&needed)` and `all()` in `crates/vizier-schema/src/capabilities.rs`
- [X] T007 Implement `Subject` (token grammar, 255-byte cap, `parse`, percent-encoding of identifiers into one token, `decode_token`) and the typed builders (`agent_req`, `agent_memop`, `session_events`, `reply`, `lease`, constants for `agents.cmd`, `files.cmd`, `dream.cmd`, `process.cmd`, `integration.changed`) per contracts/subjects.md in `crates/vizier-schema/src/subject.rs`
- [X] T008 Implement `SubjectPattern` (`parse`, `*`, `>` in last position only, `matches`, rejecting wildcards inside a token) in `crates/vizier-schema/src/subject.rs`
- [X] T009 [P] Implement `Envelope` (`v`, `kind`, `reply_to`, `Headers`, `queued_at_ms`, `payload`), `MAX_PAYLOAD = 1_048_576`, `Envelope::new`/`json`/`decode` with the `PayloadTooLarge` and unknown-version checks in `crates/vizier-schema/src/envelope.rs`
- [X] T010 Define `trait Messaging` (`publish`, `request`, `subscribe`), `Subscription` (`recv`, `close`, `lagged` notification) and the `Reply` helper in `crates/vizier-schema/src/messaging.rs`, with the bus-semantics rules 1–7 as doc comments
- [X] T011 [P] Define `trait Leases` (`acquire`, `renew`, `release`, `watch`), `Lease`, `FencingToken`, `LeaseEvent`, `LeaseWatch` in `crates/vizier-schema/src/leases.rs`, with rules 8–12 as doc comments
- [X] T012 [P] Define `trait SharedState` (`get`, `put`, `cas`, `delete`, `watch`), `Versioned`, `Revision`, `StateEvent`, `StateWatch` in `crates/vizier-schema/src/state.rs`, with rules 13–14 as doc comments
- [X] T013 Implement `Bus { messaging, leases, state, capabilities }` with `Bus::new`, `Bus::require(&Capabilities) -> Result<(), BusError>` (error names the missing capability) in `crates/vizier-schema/src/bus.rs`; re-export everything from `crates/vizier-schema/src/lib.rs`
- [X] T014 [P] Unit tests for subjects and patterns (encoding of `my.agent`, wildcard rules, invalid patterns, length cap) in `crates/vizier-schema/src/subject.rs` `#[cfg(test)]`
- [X] T015 [P] Unit tests for the envelope (limit and limit+1, version check, round trip, headers) in `crates/vizier-schema/src/envelope.rs` `#[cfg(test)]`
- [X] T016 [P] Unit test for `Bus::require` refusing a stub backend that lacks leases, naming `leases`, in `crates/vizier-schema/src/bus.rs` `#[cfg(test)]`
- [X] T017 **COMMIT (slice 1)**: `cargo test -p vizier-schema && cargo clippy -p vizier-schema --all-targets`, then `feat: add vizier-schema, the Fleet bus contract (#162)`

**Checkpoint**: contract compiles, tests pass; nothing else depends on it yet.

---

## Phase 3: User Story 3 — A backend author proves a backend correct (Priority: P2) — slice 2, part 1

**Goal**: An in-process messaging and shared-state backend, and a reusable conformance suite that fails faulty backends.

**Independent Test**: `cargo test -p vizier-schema` runs the suite against `InProcess` (messaging and state checks pass) and against faulty decorators (each fails exactly its matching check).

- [X] T018 [US3] Implement `InProcess` messaging in `crates/vizier-schema/src/inprocess.rs`: subject-routed subscriptions (patterns via `SubjectPattern::matches`), per-subscriber unbounded-or-bounded queue that never blocks the publisher (lag reported), `request` with a unique reply subject subscribed before publish, `NoResponders` when nothing is subscribed, `Timeout` otherwise, payload-limit refusal on publish
- [X] T019 [US3] Implement `InProcess` shared state (revisioned `BTreeMap`, `cas`, prefix `watch` via broadcast) in `crates/vizier-schema/src/inprocess.rs`
- [X] T020 [US3] Add `InProcess::bus()` returning a `Bus` with the available capabilities, in `crates/vizier-schema/src/inprocess.rs` and re-export from `crates/vizier-schema/src/lib.rs`
- [X] T021 [US3] Implement the conformance harness types (`Report`, `CheckResult { name, outcome: Passed|Failed(String)|Skipped(Capability) }`, `run(make_backend)`) and the messaging checks 1–8 from contracts/conformance-suite.md in `crates/vizier-schema/src/conformance.rs`
- [X] T022 [US3] Add the shared-state checks 14–15 to `crates/vizier-schema/src/conformance.rs`
- [X] T023 [US3] Add `#[test] in_process_passes_messaging_and_state_conformance` in `crates/vizier-schema/tests/conformance_inprocess.rs`
- [X] T024 [P] [US3] Add faulty decorators (answers `Timeout` instead of `NoResponders`; reorders messages; ignores the payload limit) and tests asserting each is reported as failing its matching check in `crates/vizier-schema/tests/conformance_bites.rs`
- [X] T025 **COMMIT (slice 2, messaging and state)**: `feat: add the in-process bus backend and the conformance suite (#162)`

**Checkpoint**: US3 acceptance scenarios 1–3 hold for messaging and state.

---

## Phase 4: User Story 4 — Leader work stops when the lease is lost (Priority: P2) — slice 2, part 2

**Goal**: In-process leases with fencing tokens, expiry, takeover and watch, proved by the suite.

**Independent Test**: lease checks 9–13 pass on `InProcess`, and a faulty decorator that reuses a fencing token fails `fencing_token_monotonic`.

- [X] T026 [US4] Implement `InProcess` leases (per-name state, monotonic clock via `tokio::time::Instant`, TTL expiry, `LeaseHeld`, strictly increasing `FencingToken` per name, `renew` returning `LeaseLost` after takeover, `release`, `watch` events) in `crates/vizier-schema/src/inprocess.rs`
- [X] T027 [US4] Add lease checks 9–13 (using `tokio::time::pause`/`advance`) to `crates/vizier-schema/src/conformance.rs`
- [X] T028 [P] [US4] Add the fencing-token-reuse faulty decorator and its test to `crates/vizier-schema/tests/conformance_bites.rs`
- [X] T029 [US4] Extend `in_process_passes_messaging_and_state_conformance` into `in_process_passes_conformance` covering all 15 checks in `crates/vizier-schema/tests/conformance_inprocess.rs`
- [X] T030 **COMMIT (slice 2, leases)**: `feat: add leases with fencing tokens to the in-process bus (#162)`

**Checkpoint**: the in-process backend passes all 15 conformance checks; SC-002 holds.

---

## Phase 5: User Story 2 — A role author tests a role alone (Priority: P1) — slice 3

**Goal**: Role contract skeletons, a scripted peer, a harness, roles that take a `Bus` first, a sample role test in CI and a guide.

**Independent Test**: `cargo test -p vizier-fleet --test roles` builds one role from the harness and drives it with a scripted peer.

- [X] T031 [P] [US2] Add `RoleContract { role, subscribes, publishes, leases }` and `validate()` in `crates/vizier-schema/src/contracts/mod.rs`, plus one empty constant per role in `contracts/gateway_http.rs`, `gateway_grpc.rs`, `worker.rs`, `channels.rs`, `control.rs` (all under `crates/vizier-schema/src/contracts/`)
- [X] T032 [P] [US2] Add unit tests that every role contract validates and that `validate` rejects a bad pattern and a duplicate entry, in `crates/vizier-schema/src/contracts/mod.rs` `#[cfg(test)]`
- [X] T033 [US2] Implement `ScriptedPeer` (`on(pattern, handler)`, `publish`, `expect(subject) -> Envelope` with timeout, `reply`) in `crates/vizier-schema/src/testing.rs`
- [X] T034 [US2] Make every role constructor take `Bus` first and store it: `crates/vizier-fleet/src/worker.rs`, `gateway_http.rs`, `gateway_grpc.rs`, `channels.rs`, `control.rs`; add a `needs() -> Capabilities` associated function to each; update the module docs in `crates/vizier-fleet/src/lib.rs`
- [X] T035 [US2] Implement `Harness` (in-process bus, embedded SQLite via the `storage::open_store` seam, local blobs via `storage::blob::open`, a temp workspace, `build_worker()`-style helpers returning a role from trait objects) in `crates/vizier-fleet/src/testing.rs` and `pub mod testing;` in `crates/vizier-fleet/src/lib.rs`
- [X] T036 [US2] Add the sample role test `crates/vizier-fleet/tests/roles.rs`: build a role via `Harness`, let a `ScriptedPeer` play its neighbour on a contract subject, assert the role's reply; add a second test that `Bus::require` refuses a role whose `needs()` the backend lacks
- [X] T037 [P] [US2] Write the role-test guide as a module doc in `crates/vizier-fleet/src/testing.rs` and link it from `crates/vizier-fleet/README.md`
- [X] T038 **COMMIT (slice 3)**: `feat: add role contracts, a scripted peer and the role test harness (#162)`

**Checkpoint**: US2 acceptance scenarios 1–3 hold; the sample test runs under `cargo test --workspace`.

---

## Phase 6: User Story 1 — Everything works over the bus (Priority: P1) — slices 4 to 7

**Goal**: `VizierTransport` runs on the bus with no user-visible change. Covers FR-007–FR-011 and FR-018.

**Independent Test**: all pre-existing tests pass; the dummyplug end-to-end in `quickstart.md` step 6 behaves as before; `cargo test -p vizier-core transport` passes.

### Slice 4 — serialisable-message groundwork

- [X] T039 [US1] Change `RequestTrace` to `{ traceparent, tracestate, enqueued_at_ms }` with `capture()` filling W3C headers from the current span (behind the `telemetry` feature) and `queue_wait()`, and update `invoke_agent_span` to adopt the parent from the headers, in `crates/vizier-core/src/telemetry.rs` and `crates/vizier-core/src/agents/agent/mod.rs`
- [X] T040 [P] [US1] Add a `From<BusError> for VizierError` conversion in `crates/vizier-core/src/error.rs`
- [X] T041 [US1] Replace `ClientBinding.connection: Arc<ClientConnection>` with `node: String` + `connection_id: String`; add `ClientConnections` (`register`, `get`, `remove`) in `crates/vizier-core/src/agents/client/mod.rs`; hold it on `VizierDependencies` in `crates/vizier-core/src/dependencies.rs`
- [X] T042 [US1] Register and remove sockets in `ClientConnections` and build the binding from the id in `crates/vizier-http/src/topic.rs` and `crates/vizier-http/src/api/v1/agents/channel.rs`; resolve through `ClientConnections` in `crates/vizier-core/src/agents/client/mod.rs`, treating a miss as a closed connection (deferrable tool defers, others `client_disconnected`)
- [X] T043 [P] [US1] Tests: trace headers round trip and queue wait computed from `enqueued_at_ms`; a binding with an unknown id reads as closed for both a deferrable and a non-deferrable tool, in `crates/vizier-core/src/telemetry.rs` and `crates/vizier-core/src/agents/client/mod.rs` `#[cfg(test)]`
- [X] T044 **COMMIT (slice 4)**: `refactor: [**breaking**] make trace context and the client binding serialisable (#162)`

### Slice 5 — the turn path

- [X] T045 [US1] Turn `crates/vizier-core/src/transport.rs` into a module: `git mv` it to `crates/vizier-core/src/transport/mod.rs`, add empty `reply.rs`, `inbox.rs`, `wire.rs`; keep every public name compiling
- [X] T046 [US1] Add `Bus` to `VizierTransport` (constructed from `InProcess::bus()` in `new()`/`durable()`), and a `Bus` accessor on `VizierDependencies` in `crates/vizier-core/src/transport/mod.rs` and `crates/vizier-core/src/dependencies.rs`
- [X] T047 [US1] Implement `ResponseSender`/`ResponseStream` over a reply subject (`send_async`, `recv_async`, `ReplyClosed`, subscribe-before-return) and `reply_channel()` in `crates/vizier-core/src/transport/reply.rs`
- [X] T048 [US1] Implement `Inbox` (cloneable shared subscription, closed flag, `recv_request` restoring trace and `turn_id`/`resumed` headers, `is_disconnected`) in `crates/vizier-core/src/transport/inbox.rs`
- [X] T049 [US1] Define the request wire type: `VizierRequestEnvelope` carries `reply_to: Option<Subject>` instead of `response_tx`; add `x-vizier-turn-id` / `x-vizier-resumed` header handling in `crates/vizier-core/src/transport/wire.rs`
- [X] T050 [US1] Port `register_agent`, `unregister_agent`, `is_agent_registered`, `send_request`, `resend` and `broadcast_sender` onto the bus (`agent.{id}.req`; an unregistered agent still fails at once) in `crates/vizier-core/src/transport/mod.rs`
- [X] T051 [US1] Update every `response_tx` / `flume::Sender<VizierResponse>` user to `ResponseSender`/`ResponseStream`: `crates/vizier-core/src/agents/process.rs`, `agents/mod.rs`, `agents/supervisor.rs`, `agents/hook/handover.rs`, `agents/hook/thinking.rs`, `agents/hook/tool_calls.rs`, `agents/tools/consult/mod.rs`, `channels/discord/mod.rs`, `channels/telegram/mod.rs`, `scheduler/mod.rs`, `service/agents.rs`, `crates/vizier-http/src/topic.rs`
- [X] T052 [US1] Update `await_session_outcome` and the background-piece tests in `crates/vizier-core/src/agents/background/` so a piece with no terminal frame ends at its own timeout; rewrite `a_closed_channel_without_an_answer_reads_as_a_restart` as `a_piece_with_no_terminal_frame_has_no_outcome`
- [X] T053 [P] [US1] Transport tests: request round trip over the bus, an unregistered agent refuses at once, an inbox closes on unregister while clones observe it, queued requests survive an instance swap, and a realistic chat request survives the envelope, in `crates/vizier-core/src/transport/mod.rs` `#[cfg(test)]`
- [X] T054 [US1] Run `cargo test -p vizier-core --test dummyplug_e2e` and `cargo run --example headless` (quickstart step 6a); fix any e2e helper that answers on a detached bus (it must use the runtime's transport)
- [X] T055 **COMMIT (slice 5)**: `refactor: [**breaking**] run the request path on the bus (#162)`

### Slice 6 — command channels (one commit each)

- [X] T056 [US1] Agent commands: add `AgentCommandRequest`/`AgentCommandResult` wire types in `crates/vizier-core/src/transport/wire.rs`; replace the `oneshot` in `AgentCommand` (`crates/vizier-core/src/schema/commands.rs`); implement `send_agent_command` as a bus request on `agents.cmd` and a `serve_agent_commands` handler for `VizierAgents`; update `agents/mod.rs`, `integrations/service.rs`, `service/agents.rs`, `service/usage.rs`, `crates/vizier/src/cli/agent.rs`, `examples/headless.rs`, `tests/dummyplug_e2e.rs`; keep `agent_health`'s 5 s deadline with its two messages mapped from `NoResponders`/`Timeout`
- [X] T057 **COMMIT (slice 6a)**: `refactor: [**breaking**] send agent commands over the bus (#162)`
- [X] T058 [US1] File commands (bytes staged in the blob store, only a key on the bus): `FileReply` wire type; `send_file_upload`/`send_file_resolve` as bus requests on `files.cmd`; handler in `crates/vizier-core/src/file_manager/mod.rs`; update `agents/agent/mod.rs`, `channels/discord/mod.rs`, `channels/telegram/mod.rs`, `service/agents.rs`, `service/files.rs`, `crates/vizier-http/src/topic.rs`, `schema/commands.rs`
- [X] T059 **COMMIT (slice 6b)**: `refactor: [**breaking**] send file commands over the bus (#162)`
- [X] T060 [US1] Memory ops: replace `MemoryOpEnvelope`'s oneshot with request/reply on `agent.{id}.memop`; update `crates/vizier-core/src/agents/memory_ops.rs`, `agents/mod.rs`, `agents/supervisor.rs`, `service/memory.rs`, `schema/commands.rs`, `schema/mod.rs`
- [X] T061 **COMMIT (slice 6c)**: `refactor: [**breaking**] send memory operations over the bus (#162)`
- [X] T062 [US1] Dream commands: publish `DreamCommand` (add serde derives) on `dream.cmd`; subscribe in `crates/vizier-core/src/agents/process.rs`; update `scheduler/mod.rs`, `service/dream.rs`
- [X] T063 [US1] Process commands: `CommandRequest`/`CommandResponse` as request/reply on `process.cmd`; serve it from `VizierTransport::run`; update `crates/vizier/src/command/mod.rs`; keep `exit_channel`/`exit_signal` process-local (research R7) and verify `crates/vizier-core/src/runtime.rs` still exits on it
- [X] T064 **COMMIT (slice 6d)**: `refactor: [**breaking**] publish dream and process commands on the bus (#162)`

### Slice 7 — events

- [X] T065 [US1] Add `Serialize`/`Deserialize` to every `SessionFrame` payload type that lacks them (`ClientCallFrame`, question and sensitive-ask records, and any other) in `crates/vizier-core/src/transport/mod.rs` and the modules that define them
- [X] T066 [US1] Publish `SessionEvent` on `session.{id}.events` and `IntegrationChanged` on `integration.changed`; keep one `session.>` subscription demultiplexed in process so `subscribe_session_events` / `subscribe_integration_changes` keep their signatures and lag behaviour, in `crates/vizier-core/src/transport/mod.rs`
- [X] T067 [P] [US1] Verify and adjust the session-event consumers without changing their logic: `agents/agent/mod.rs`, `agents/background/mod.rs`, `agents/client/mod.rs`, `agents/process.rs`, `agents/steps.rs`, `channels/reactions.rs`, `channels/woken.rs`, `dependencies.rs`, `questions/sensitive.rs`, `questions/submit.rs`, `runtime.rs`, `crates/vizier-http/src/api/v1/agents/channel.rs`, `topic.rs`; and the integration consumers `agents/mod.rs`, `channels/connections.rs`, `integrations/service.rs`
- [X] T068 [P] [US1] Tests: two subscribers on one session both receive a frame once; a lagging subscriber logs and carries on; an integration change reaches `ChannelConnections` and the agent manager, in `crates/vizier-core/src/transport/mod.rs` `#[cfg(test)]`
- [X] T069 [US1] Remove every now-unused `flume` field and import from `crates/vizier-core/src/transport/` and confirm `grep -rn "flume::" crates/vizier-core/src/transport` returns nothing
- [X] T070 [US1] Check `Bus::require` is called for the selected roles before `run_server` starts them, in `crates/vizier/src/cli/server.rs`
- [X] T071 **COMMIT (slice 7)**: `refactor: [**breaking**] carry session and integration events on the bus (#162)`

**Checkpoint**: US1 acceptance scenarios 1–6 hold; SC-001, SC-003, SC-005, SC-007 verified.

---

## Phase 7: User Story 5 — Maintainers read the contract (Priority: P3) — slice 8, plus Polish

**Goal**: One docs page with every stated semantic, the per-role contract table, and CI policy that enforces the layering.

**Independent Test**: each rule in contracts/bus-semantics.md appears on the page and maps to a conformance check or test.

- [X] T072 [US5] Write `docs/src/fleet/bus-contract.md` from contracts/bus-semantics.md, contracts/subjects.md and the per-role contract table (empty skeletons noted), and add it to `docs/src/SUMMARY.md` under a new *Fleet* section
- [X] T073 [P] [US5] Add a rule-to-check table at the end of `docs/src/fleet/bus-contract.md` mapping each numbered rule to its conformance check name
- [X] T074 [P] Add the `vizier-schema` layering check to the `policy` job in `.github/workflows/ci.yml`: `vizier-schema` has no direct dependency on any `vizier-*` crate, `async-nats`, `tonic`, `axum`, `axum-extra`, `tower-http`, `clap`, `inquire`, `termimad`, `daemonize`; keep `vizier-core` forbidden from `async-nats`/`tonic`
- [X] T075 [P] Update `CLAUDE.md` (workspace table row for `vizier-schema`, a *Fleet bus* paragraph under Architecture replacing the "in-process message bus built on `flume`" text, `exit_channel` note) and `crates/vizier-fleet/README.md`
- [ ] T076 (partly done: library tests, dummyplug e2e test, clippy; the real-binary steps need Linux or WSL) Run the full verification from quickstart.md: steps 1–5 and 7, then step 6 (dummyplug end-to-end: tools, tool call, `context`, two WebUI tabs, agent edit mid-turn, background piece, `vizier agent ps`, `vizier shutdown` exit code 0)
- [X] T077 Run `cargo test --workspace`, `cargo clippy --workspace --all-targets`, `cargo test -p vizier-core --no-default-features`, and `cargo test -p vizier --no-default-features --features role-worker`; record any pre-existing Windows-only failures (`projects::path`, `skill::scopes`) in the PR description rather than fixing them here
- [X] T078 **COMMIT (slice 8)**: `doc: add the bus contract page and the vizier-schema layering policy (#162)`
- [ ] T079 Open the pull request from `162-vizier-schema` with a conventional title (`feat: [**breaking**] add the Fleet message bus and run the transport on it (#162)`), listing the eight slices, the accepted piece-timeout behaviour change and the unverified CI jobs (`managed-storage`, Windows server build)

---

## Dependencies & Execution Order

### Phase dependencies

- **Setup (Phase 1)** → **Foundational (Phase 2)** → everything else.
- **US3 (Phase 3)** needs Foundational. **US4 (Phase 4)** needs US3 (same file, same suite). **US2 (Phase 5)** needs the backend from Phase 3 (the harness uses `InProcess`).
- **US1 (Phase 6)** needs Phases 2–3 (the bus and the backend); it does not need the harness, but the commit order keeps slice 3 first so later role work can use it.
- **US5/Polish (Phase 7)** last; T072 can start once Phase 4 is done, but T076–T079 need everything.

### Within US1

Slice 4 → 5 → 6 → 7 in order (shared hot files). Inside slice 6 the four commits are independent of each other once slice 5 lands, but go one at a time to avoid conflicts in `transport/mod.rs` and `schema/commands.rs`.

### Parallel opportunities

- Phase 2: T005, T006, T009, T011, T012 are different files; T014–T016 after their targets.
- Phase 3/4: T024 and T028 (faulty decorators) alongside suite work.
- Phase 5: T031/T032 in parallel with T033.
- Phase 6: T040, T043, T053, T067, T068 are independent of the main edit chain.
- Phase 7: T073, T074, T075 are different files.
- Writing is one PR and one author at a time on the shared hot files; parallelism is mostly about test and doc files.

## Implementation Strategy

### MVP first

Phases 1–2 and 5 give a testable contract plus harness; the real MVP of the *product* is Phase 6 slice 5 (the turn path on the bus), because it proves the contract carries real traffic. Do not stop after the contract: the spec's SC-003 needs every channel moved.

### Incremental delivery inside the one PR

1. Commit after each slice (T017, T025, T030, T038, T044, T055, T057/T059/T061/T064, T071, T078) with the gate green.
2. If the PR must be split later, each slice commit is already a self-contained cut.
3. Rebase on `develop` after every second commit; conflicts will concentrate in `transport/mod.rs`, `agents/process.rs`, `dependencies.rs` and `runtime.rs`.

## Notes

- `[P]` tasks touch different files; the many `response_tx` call sites in T051 are one task because they must compile together.
- Do not add a Cargo feature for the bus or for `testing` (research R15).
- Nothing in this list touches `webui/`, `design/`, `landing/` or `dummyplug/`, so no submodule branch is needed.
- Total: 79 tasks (Setup 4, Foundational 13, US3 8, US4 5, US2 8, US1 33, US5 and Polish 8).
