# Phase 0 Research: Fleet Message Bus

Every item below resolves a design question the spec left open. There were no `NEEDS CLARIFICATION` markers; these are the decisions planning had to make.

## R1. Where the trait implementation of the in-process backend lives

- **Decision**: `InProcess` lives in `vizier-schema` beside the traits, together with the conformance suite and `ScriptedPeer`.
- **Rationale**: The harness in `vizier-fleet` and the engine both need it; putting it in core would force the harness to depend on core for a bus, and a future networked backend crate would have to pull in core to reuse the conformance suite. `tokio` (already in the workspace) is its only extra dependency, which the policy check does not forbid (it forbids broker, web-framework and CLI crates).
- **Alternatives**: backend in `vizier-core` (rejected: couples every consumer to the engine); a separate `vizier-bus-inprocess` crate (rejected under Principle I: a third crate for ~400 lines).

## R2. Object safety and async

- **Decision**: Traits use `#[async_trait]` so `Bus` can hold `Arc<dyn Messaging>`, `Arc<dyn Leases>`, `Arc<dyn SharedState>`.
- **Rationale**: Roles must not learn the backend, so they take trait objects. Native `async fn` in traits (edition 2024) is not dyn-compatible. `async-trait` is already in the workspace dependency set.
- **Alternatives**: generics on every role (rejected: leaks the backend type through every signature and every `VizierDependencies` field); hand-boxed futures (rejected: same cost as `async-trait`, more code).

## R3. Subject grammar and identifier encoding

- **Decision**: Dot-separated tokens. A token is `[A-Za-z0-9_%~-]+`. An identifier that contains anything else (notably `.`, `*`, `>`, whitespace, `/`) is percent-encoded into **one** token, so `my.agent` becomes `my%2Eagent`. Patterns: exact, `*` for exactly one token, `>` for one-or-more trailing tokens and only as the final token.
- **Rationale**: Agent ids and usernames are user-chosen and may contain dots. If they were inserted raw, `agent.a.b.req` could widen or collide with another agent's subject. Percent-encoding is reversible and needs no escape-table. This matches the three wildcard forms the issue allows and nothing more.
- **Alternatives**: forbid dots in ids (rejected: breaks existing agents); hash ids (rejected: not reversible, hurts debugging).

## R4. Envelope shape

- **Decision**: `Envelope { v: u8, kind: String, reply_to: Option<Subject>, headers: Headers, queued_at_ms: Option<u64>, payload: Vec<u8> }`, serialised as JSON for the in-process backend's byte accounting. `v` starts at 1. `headers` carries `traceparent` / `tracestate` (W3C Trace Context). The payload is the JSON of the typed message.
- **Rationale**: The issue names `Envelope { v, kind, reply_to, trace, payload }`; `trace` becomes the W3C headers so the context survives a process hop, and `queued_at_ms` (wall clock, milliseconds) replaces `Instant`, which is meaningless across processes. The queue-wait metric stays measurable end to end. The 1 MiB limit is checked on the encoded payload at publish and at `Envelope::new`.
- **Alternatives**: binary codec such as bincode (rejected: payload types already derive `serde` for JSON and storage, JSON keeps frames inspectable and matches the wire a networked backend would carry); `Instant`-based queue time (rejected: not portable).

## R5. How a request/reply and a streamed turn travel

- **Decision**: A turn's request is published on `agent.{id}.req` with `reply_to = reply.{uuid}`. The sender side subscribes to the reply subject **before** publishing. `ResponseSender` publishes each `VizierResponse` frame to that subject; `ResponseStream` is the subscription. Both keep the method names `send_async` / `recv_async` and return a `ReplyClosed` error, so the ~60 call sites in `agents/process.rs` and the 20-odd `while let Ok(frame) = rx.recv_async().await` loops change type but not shape.
- **Rationale**: Subscribing first closes the race where an immediate answer is published before anyone listens (a spec edge case). Keeping method names keeps the diff reviewable in the shared hot files.
- **Consequence (accepted)**: A reply address has no "closed" signal, so an agent that vanishes no longer ends a background piece instantly; it ends at its own `timeout_secs`. The test pinning instant detection (`a_closed_channel_without_an_answer_reads_as_a_restart`) is rewritten to pin the new behaviour. An explicit terminal frame published when an agent process ends is the honest fast path and is left to a follow-up issue.
- **Alternatives**: keep `flume::Sender` and special-case in-process (rejected: unserialisable, defeats the purpose); add a close frame now (rejected: scope creep, the issue asks for no behaviour change beyond what the port forces).

## R6. Per-agent inbox

- **Decision**: `register_agent` subscribes `agent.{id}.req` and returns a cloneable `Inbox` (a shared subscription plus a closed flag). The supervisor clones it per instance and asks `is_disconnected`; `unregister_agent` closes it and ends the running process.
- **Rationale**: `CLAUDE.md` records that the channel's receiver passes from supervisor to supervisor across updates so requests queue instead of failing; the inbox must keep exactly that. `send_request` to an agent nothing registered must fail at once, not after a timeout, so the registry check stays and the bus publish reports `NoResponders`.
- **Alternatives**: one subscription per instance (rejected: loses queued requests across updates).

## R7. Which channels move and which stay process-local

- **Decision**: Move onto the bus: agent requests, agent commands, file commands, memory ops, dream commands, command-socket request/response, session events, integration changes. **Keep process-local: `exit_channel`.**
- **Rationale**: Exit is "this process should stop" raised by the command handler and consumed by `run_server`; it is lifecycle of one OS process, not traffic between roles. Putting it on a shared bus would let one node's `vizier shutdown` stop another's process. The spec's FR-008 and SC-003 are amended to say "inter-subsystem traffic" excludes this single-process lifecycle signal.
- **Alternatives**: bus subject `process.exit` (rejected: semantically wrong across nodes).

## R8. Replacing `oneshot` inside command payloads

- **Decision**: Each of `AgentCommand` (7 variants), `FileCommand` (`Upload`, `Resolve`), `MemoryOpEnvelope`, and `HealthCheck` stops carrying a `tokio::sync::oneshot`. The sender issues a bus `request` on a per-channel subject and the handler replies with a small serialisable reply enum (`AgentCommandResult`, `FileReply`, `MemoryOpResponse`, health `Vec<AgentHealthStatus>`). Errors cross as `{ message }` strings because `anyhow::Error` is not serialisable; the caller rebuilds a `VizierError` at the boundary. `DreamCommand` and `CommandRequest`/`CommandResponse` are already serialisable and need only a subject.
- **Rationale**: Request/reply is the one primitive the trait gives for "ask and wait", and it carries the "never hangs" guarantee for free (timeout or no-responder).
- **Alternatives**: keep oneshots alongside and publish only the data half (rejected: the handler would still be tied to an in-process pointer, so a remote handler could not answer).

## R9. Session and integration events

- **Decision**: Publish `SessionEvent` on `session.{id}.events` and `IntegrationChanged` on `integration.changed`. In-process, one `session.>` subscription is demultiplexed locally to the existing subscribers; a lagging subscriber behaves as today (logs, reconciles).
- **Rationale**: This is exactly today's broadcast semantics, but addressed so a remote gateway could subscribe to one session. `SessionFrame` payloads all already derive serde; the work is adding the missing derives on the few that do not.
- **Alternatives**: one global `session.events` subject (rejected: every gateway would receive every session's traffic).

## R10. Client-tool binding

- **Decision**: `ClientBinding` carries `node` and `connection_id` instead of `Arc<ClientConnection>`. A per-engine registry, `ClientConnections`, on `VizierDependencies` resolves the id. A miss means the socket is not on this node and is treated as closed: a deferrable tool defers, anything else returns `client_disconnected`. The cross-node subject `gateway.{node}.client` is **not** created here; it is a gateway capability delivered with the HTTP gateway role (#169).
- **Rationale**: `gateway.{node}.client` belongs to one gateway and needs a gateway to be tested against. Deferral already finds work by session out of storage, so only the live round trip is process-local. This follows the decision recorded on the issue.
- **Alternatives**: define the subject now with no peer (rejected: untestable, violates "testable alone").

## R11. Trace propagation

- **Decision**: `RequestTrace` becomes `{ traceparent: Option<String>, tracestate: Option<String>, enqueued_at_ms: Option<u64> }`. `send_request` fills it from the current span; `invoke_agent_span` adopts it as parent and reports `vizier.queue.wait_ms` from `now - enqueued_at_ms`. The headers travel in the envelope and are restored on arrival in `Inbox::recv_request`, the only place that knows they belong to a request.
- **Rationale**: Keeps the existing "a request sent from inside a span continues that trace" behaviour documented in CLAUDE.md. Behind the `telemetry` feature, extraction/injection uses the already-present OpenTelemetry propagator; without the feature the fields are simply absent.
- **Alternatives**: carry the OTel `SpanContext` directly (rejected: not serialisable).

## R12. Capabilities and startup refusal

- **Decision**: `Capabilities { messaging, leases, shared_state }` (booleans, reported by the backend). Each role declares the capabilities it needs in `vizier-fleet`; `Bus::require(&needed)` returns an error naming the missing capability and `run_server` calls it for the selected roles before starting. `InProcess` reports all three, so the refusal is exercised by a unit test with a stub backend.
- **Rationale**: Satisfies FR-006 without inventing a configuration surface; a networked backend later just reports fewer capabilities.
- **Alternatives**: runtime discovery on first use (rejected: fails late, mid-turn).

## R13. Lease semantics

- **Decision**: `acquire(name, ttl) -> Lease` (error if held), `renew(&Lease) -> Result<()>` (error means the holder has lost it and must stop), `release`, `watch(name) -> Stream<LeaseEvent>`. A `Lease` carries a `FencingToken(u64)` that is strictly increasing per lease name across holders, including across expiry takeovers. `InProcess` uses a monotonic clock so tests can use `tokio::time::pause`.
- **Rationale**: Fencing tokens let a storage write reject a stale leader; the monotonic clock makes expiry deterministic in tests.
- **Alternatives**: wall-clock TTL (rejected: flaky tests, clock skew once networked).

## R14. Shared state

- **Decision**: `get`, `put`, `cas(key, expected_revision, value)`, `delete`, `watch(prefix)`. Values are bytes with a revision counter. Backed in process by a `BTreeMap` behind a mutex plus a broadcast for watches.
- **Rationale**: The smallest surface that supports the compare-and-swap check and leader-style coordination later; nothing in the engine uses it yet.
- **Alternatives**: omit until needed (rejected: the issue lists it as one of the three traits and the conformance suite covers CAS).

## R15. Role harness placement and flags

- **Decision**: `ScriptedPeer` in `vizier-schema::testing`; `Harness` (in-process bus, embedded SQLite via `storage::open_store`, local blobs via `storage::blob::open`, `build(role)` helper) in `vizier-fleet::testing`. Both are plain `pub mod testing`, not behind a Cargo feature.
- **Rationale**: The constitution's flag rule covers optional channels, addons, providers and gateways; a test helper is none of those, and a flag would complicate CI for no gain. The helpers add no dependency the crates do not already carry.
- **Alternatives**: `cfg(test)` only (rejected: other crates' integration tests could not use it); a `testing` feature (rejected as above).

## R16. CI and policy

- **Decision**: Add `crates/vizier-schema/**` to the `changes` job's `rust`/`core` paths (a shared crate runs everything); add a `policy` check that `vizier-schema` has no direct dependency on any workspace crate, `async-nats`, `tonic`, `axum*`, `tower-http`, `clap`, `inquire`, `termimad`, `daemonize`; keep `vizier-core` forbidden from depending on `async-nats`/`tonic`. The sample role test is part of `cargo test --workspace`, so it needs no new job.
- **Rationale**: Matches the existing `policy` structure (the file already anticipates `vizier-schema` in a comment).

## R17. Prior art

- **Decision**: An earlier local attempt exists in the reflog (tip `8485c3df`; notable commits "run the request path on the bus", "send agent commands over the bus", "send memory operations over the bus", "publish dream commands on the bus", "name a client-tool socket instead of holding it"). It is not on any branch and the owner called it a failed attempt, so the plan does not depend on it. Reading it for naming and gotchas (the reply-subscribe-first race, the e2e helper answering on a detached bus) is allowed; cherry-picking is the owner's call.
