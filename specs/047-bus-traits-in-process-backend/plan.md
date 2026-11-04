# Implementation Plan: Fleet Message Bus, In-Process Backend and Role Test Harness

**Branch**: `047-vizier-schema` | **Date**: 2026-11-03 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/047-bus-traits-in-process-backend/spec.md` (issue vizier-lab/vizier#162, part of the Fleet epic #159)

## Summary

Introduce a new leaf crate `vizier-schema` that holds the wire contract (validated `Subject`/`SubjectPattern`, a versioned `Envelope`, error types, the 1 MiB payload limit) and three capability-split traits (`Messaging`, `Leases`, `SharedState`) plus a `Capabilities` descriptor and a `Bus` handle that bundles them. Ship an `InProcess` backend that implements all three, then port `VizierTransport` (today: `flume` channels plus two `tokio::broadcast`s, 498 lines) onto it without behaviour change. Replace every non-serialisable field a message carries (reply `flume::Sender`, `oneshot` response slots, `RequestTrace` with a live `SpanContext`/`Instant`, `ClientBinding` with an `Arc<ClientConnection>`) with a serialisable equivalent. Add a reusable backend conformance suite, per-role bus contract skeletons, a role test harness with a scripted peer in `vizier-fleet`, a policy check, and the bus-contract docs page.

Approach is bottom-up and lands in independently shippable slices (see [Delivery slices](#delivery-slices)); the contract and backend come first because nothing in `vizier-core` changes until they pass their own conformance suite.

**State of the tree (important)**: `develop` contains none of this. There is no `vizier-schema`, no bus trait, no `docs/src/fleet/`, and `transport.rs` is still a single file on `flume`. An earlier attempt (commits on a since-deleted local branch, tip `8485c3df`, recoverable from the reflog) is treated as prior art only; this plan does not assume any of it exists. The spec's *Delivery state* paragraph has been corrected accordingly.

## Technical Context

**Language/Version**: Rust, edition 2024 (workspace `resolver = "3"`), stable toolchain

**Primary Dependencies**: `vizier-schema` adds only `serde`, `serde_json`, `tokio` (sync/time/rt, for the in-process backend), `async-trait` (already in the workspace, needed for `Arc<dyn Messaging>`), `uuid` (reply subjects, already in the workspace) and `bytes`-free plain `Vec<u8>` payloads; no crate from this repo, no broker, web framework or CLI crate. `vizier-core` gains a dependency on `vizier-schema`; `vizier-fleet` gains one too.

**Storage**: N/A for the bus itself. The role harness builds embedded SQLite + the local blob store through the existing `storage::open_store` / `storage::blob::open` seams. Turn journaling (`session_turn`) stays in `VizierTransport` and is unchanged.

**Testing**: `cargo test -p vizier-schema` (unit + the conformance suite run against `InProcess`, plus a deliberately faulty backend proving the suite bites), `cargo test -p vizier-fleet` (harness + the sample role test, `crates/vizier-fleet/tests/roles.rs`), `cargo test -p vizier-core --lib` (transport and trace tests, plus every pre-existing test unchanged), `cargo test --workspace`. End-to-end: a dummyplug agent turn through a real `VizierRuntime`, scripted in [quickstart.md](quickstart.md) per the constitution's e2e gate (`crates/vizier-core/examples/headless.rs` and the standard `tools` / tool-name / JSON steps).

**Target Platform**: Linux server, macOS, and the Windows desktop app (the engine and `vizier-schema` must compile on all three; the `vizier` server binary stays unix-only as today)

**Project Type**: Rust workspace library crates (engine seam + new contract crate + role crate); no UI

**Performance Goals**: No user-visible regression on the turn path. A request/reply hop on `InProcess` should stay within the cost of today's `flume` send plus one JSON (de)serialisation of the request; frames streamed on a turn keep their order. No numeric target beyond "turn latency not measurably worse in the dummyplug e2e".

**Constraints**: 1 MiB payload limit enforced by `Envelope` construction/publish; at-most-once, per-subject-per-publisher ordering; `request` never hangs (timeout or no-responder error); a failed lease `renew` obliges the holder to stop; no broker type in any public signature; `vizier-schema` depends on no workspace crate; `vizier-core` stays free of `async-nats`/`tonic` (the existing `policy` check). No new runtime service (Principle III) and no new feature flag on the engine (the bus is not optional in the engine).

**Scale/Scope**: ~110 `response_tx` references in 13 files, ~105 `flume::` references in 24 files, 26 session-event sites in 14 files, 23 file-command sites in 8, 13 agent-command sites in 8, 10 integration-event sites in 4, 7 memory-op sites in 4, 6 command-socket sites in 2, 6 trace sites in 4. One new crate, one new docs page, one new CI path/policy entry.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design below.*

| Principle / Gate | Verdict | Notes |
|---|---|---|
| I. Lean by Default | **Justified deviation** | Three traits with one backend is the "trait with a single implementation" the principle warns about. The Fleet epic (#159) mandates it: a second real implementation (a networked bus) is the stated next step, and the conformance suite is how that second implementation is held to the same rules. Recorded in Complexity Tracking. No new third-party crate is added beyond ones already in the workspace. |
| II. DRY via traits | **Pass** | This feature *is* the application of Principle II to the bus: one behaviour (messaging, leases, shared state) behind traits, selected at construction, no `match` on a backend tag at call sites. Each existing channel pair collapses onto one `request`/`subscribe` shape through shared helpers (typed request/reply, reply stream, inbox) rather than seven bespoke wrappers. |
| III. Self-contained default | **Pass** | `InProcess` is the default and only backend; no broker is required, bundled or linked. Single binary, config-less mode and the desktop app are untouched. |
| IV. Portability | **Pass** | `vizier-schema` has no OS-specific code and is pure Rust over tokio; it must build for the `Cross.toml` targets and on Windows (the desktop app links the engine). |
| V. Unified errors & observability | **Pass with note** | `vizier-schema` is a leaf crate and cannot name `vizier_core::VizierError`, so it defines one `BusError` enum; `vizier-core` converts at the boundary with `throw_vizier_error`-style mapping (one `From<BusError> for VizierError`). Logging through `tracing` only. Trace context is carried as W3C headers so OTel behaviour is preserved. |
| Optional capabilities behind a flag | **Pass** | The bus is core, not optional, so it has no flag. A future networked backend is `bus-nats` (reserved name). A flag never changes a signature. |
| Roles/components stay independent | **Pass** | Roles receive a `Bus` and storage traits only; the harness tests one role alone. The `policy` job additionally enforces that `vizier-schema` depends on no workspace crate and that roles do not import each other. |
| Verify what a change touches | **Pass** | `ci.yml` `changes` job gets `crates/vizier-schema/**` paths; `vizier-schema` is a shared crate, so changes to it run everything. |
| Dummyplug e2e for agent-observable change | **Applies** | The turn path, background reports, session events and agent lifecycle are agent-observable. [quickstart.md](quickstart.md) scripts the dummyplug steps. |
| Matching submodule branches | **N/A** | No `design`, `webui`, `landing` or `dummyplug` change. |
| Conventional commits | **Applies** | Each slice lands as `feat:`/`refactor:`; behaviour changes and public-signature changes carry `[**breaking**]` (the `VizierTransport` surface is `#[doc(hidden)]` but consumed by `vizier-http` and `vizier`). |
| Manual verification for runtime-affecting changes | **Applies** | Process startup and shutdown paths change (command socket, exit signal); run the real binary, not just `cargo build`. |

No unjustified violation. **Gate: PASS.**

**Post-design re-check**: the Phase 1 design adds no dependency beyond the workspace set, keeps the backend selection in one constructor, and leaves `exit_channel` process-local (see [research.md](research.md) R7). Still **PASS**.

## Project Structure

### Documentation (this feature)

```text
specs/047-bus-traits-in-process-backend/
├── plan.md              # This file
├── research.md          # Phase 0: decisions and alternatives
├── data-model.md        # Phase 1: types of the contract and the wire messages
├── quickstart.md        # Phase 1: verification steps incl. dummyplug e2e
├── contracts/
│   ├── bus-semantics.md     # the stated guarantees (what the docs page publishes)
│   ├── subjects.md          # subject grammar, encoding, the subject table
│   ├── wire-messages.md     # message kinds and reply shapes per channel
│   └── conformance-suite.md # the named checks every backend runs
├── checklists/requirements.md
└── tasks.md             # Phase 2 (/speckit-tasks, not created here)
```

### Source Code (repository root)

```text
Cargo.toml                                  # + vizier-schema in [workspace.dependencies]

crates/vizier-schema/                       # NEW leaf crate (no workspace deps)
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── subject.rs          # Subject, SubjectPattern, id encoding, matching
    ├── envelope.rs         # Envelope, Kind, trace headers, MAX_PAYLOAD
    ├── error.rs            # BusError
    ├── capabilities.rs     # Capabilities, Capability
    ├── messaging.rs        # trait Messaging, Subscription, Reply
    ├── leases.rs           # trait Leases, Lease, FencingToken
    ├── state.rs            # trait SharedState
    ├── bus.rs              # Bus { messaging, leases, state, capabilities }
    ├── inprocess.rs        # InProcess: implements all three
    ├── contracts/          # one skeleton per role: subscribes / publishes / leases + validate()
    │   ├── mod.rs
    │   ├── gateway_http.rs gateway_grpc.rs worker.rs channels.rs control.rs
    ├── conformance.rs      # reusable suite; run(backend) -> Report
    └── testing.rs          # ScriptedPeer

crates/vizier-core/src/
├── transport/              # transport.rs becomes a module
│   ├── mod.rs              # VizierTransport facade (public surface kept)
│   ├── reply.rs            # ResponseSender / ResponseStream over a reply subject
│   ├── inbox.rs            # Inbox: the cloneable per-agent subscription
│   └── wire.rs             # message kinds + wire reply enums for each command channel
├── telemetry.rs            # RequestTrace -> W3C headers + queued timestamp
├── agents/client/mod.rs    # ClientBinding { node, connection_id, ... } + ClientConnections
├── dependencies.rs         # owns the Bus and ClientConnections
└── agents/{mod,process,supervisor,background/**}.rs, scheduler/**, channels/**   # call sites

crates/vizier-fleet/
├── src/lib.rs, <role>.rs   # every constructor takes `Bus` first
├── src/testing.rs          # Harness: in-process bus + embedded SQLite + local blobs
└── tests/roles.rs          # the sample role test run by CI

crates/vizier-http/src/**, crates/vizier/src/cli/**     # call-site updates only

docs/src/fleet/bus-contract.md              # NEW; linked from docs/src/SUMMARY.md
.github/workflows/ci.yml                    # changes paths, policy rules for vizier-schema
```

**Structure Decision**: a new leaf crate for the contract and backend (so core and every role depend on it, never the reverse), the engine port inside `vizier-core`'s existing `transport` module, and the harness inside `vizier-fleet` (it already depends on core and owns the role skeletons). This matches the epic's "wire contract and the three bus traits live in `vizier-schema`" decision and the existing workspace layering rules in CLAUDE.md.

## Delivery slices

All slices ship in **one pull request** (`047-vizier-schema`), with **one commit per slice**, in the order below, so the history reads as the slices and each commit can be reviewed or bisected alone. Each commit compiles and passes `cargo test --workspace` (and `cargo clippy --workspace --all-targets`) before the next begins; slice 6 is one commit per command channel. Commit messages are conventional (`feat:` for new crates and tooling, `refactor:` for the port, `[**breaking**]` where the `#[doc(hidden)]` `VizierTransport` surface changes), and the PR title is conventional too. Because `transport.rs`, `agents/mod.rs`, `agents/process.rs`, `dependencies.rs` and `runtime.rs` are shared hot files, rebase on `develop` often while the PR is open.

1. **Contract crate** (`vizier-schema`): subjects, patterns, envelope, errors, capabilities, the three traits, `Bus`. Docs-in-code for semantics. No consumer yet.
2. **In-process backend + conformance suite**: `InProcess`, `conformance::run`, a faulty-backend test proving the suite bites. CI path and policy entries.
3. **Harness + role contract skeletons**: `contracts/*`, `ScriptedPeer`, `vizier-fleet::testing::Harness`, every role constructor takes `Bus` first, `tests/roles.rs`, the guide.
4. **Serialisable-message groundwork in core**: `RequestTrace` as W3C headers + queued timestamp; `ClientBinding` as node + connection id with `ClientConnections`. Single-process behaviour unchanged; pinned by tests.
5. **Turn path on the bus**: `agent.{id}.req`, reply subjects, `ResponseSender`/`ResponseStream`, `Inbox`, `register_agent` returning an `Inbox`. Update the background-piece test that pinned instant restart detection (accepted behaviour change).
6. **Command channels on the bus** (one PR each, independently shippable): agent commands, file commands, memory ops, dream commands, process commands (command socket).
7. **Events on the bus**: session events and integration changes through `session.{id}.events` / `integration.changed`, demultiplexed in process.
8. **Docs + policy finish**: `docs/src/fleet/bus-contract.md`, `SUMMARY.md`, CLAUDE.md architecture note, final CI wiring, remove any now-unused `flume` plumbing from `transport`.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| Three traits (`Messaging`, `Leases`, `SharedState`) with a single in-process implementation (Principle I) | The Fleet epic (#159) needs a networked backend next, and capabilities differ per backend (a broker that can message but not lease, a SQL store that can lease but not stream). Splitting by capability lets a backend implement only what it can and lets startup refuse an under-capable pairing. The conformance suite is how a second backend is held to identical rules. | One fat `Bus` trait forces every backend to stub capabilities it lacks and hides the mismatch until runtime. Waiting for the second backend to introduce the abstraction would mean rewriting `VizierTransport` again under the same shared hot files the epic wants touched once. |
| A new workspace crate `vizier-schema` (Principle I) | The epic requires the contract to live outside `vizier-core` so gateways and a future backend depend on it without depending on the engine; the `policy` job can then enforce the direction. | Putting the traits in `vizier-core` makes every role and backend depend on the whole engine, and forces a later extraction across the same files. |
