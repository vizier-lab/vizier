# Data Model: Fleet Message Bus

These are the types of the contract (`vizier-schema`) and the wire shapes the engine puts on it. Names are the planned Rust names; field lists are the contract, not an implementation script.

## Contract types (`vizier-schema`)

### Subject
- Validated string of one or more tokens joined by `.`.
- Built only through constructors that take identifiers and encode them (`Subject::agent_req(&AgentId)`, `session_events`, `lease("leader")`, …) or through `Subject::parse` for a literal.
- Invariants: no empty token; each token matches `[A-Za-z0-9_%~-]+`; total length ≤ 255; an identifier is percent-encoded into exactly one token.

### SubjectPattern
- Same grammar plus `*` (exactly one token) and `>` (one or more trailing tokens, **last position only**).
- `matches(&Subject) -> bool`. Rejects `>` in a non-final position and wildcards mixed into a token (`ag*nt`).

### Envelope
| Field | Type | Meaning |
|---|---|---|
| `v` | `u8` | Wire version, starts at 1; an unknown version is a decode error. |
| `kind` | `String` | Message kind, e.g. `request`, `response_frame`, `agent_command`; routes decoding. |
| `reply_to` | `Option<Subject>` | Where a reply goes. `None` for fire-and-forget. |
| `headers` | `Headers` | `traceparent`, `tracestate` (W3C) and open for others. |
| `queued_at_ms` | `Option<u64>` | Unix ms when first queued; kept across re-queue. |
| `payload` | `Vec<u8>` | JSON of the typed message. |

Validation: encoded payload ≤ `MAX_PAYLOAD` (1 MiB = 1,048,576 bytes); over-limit is `BusError::PayloadTooLarge { size, limit }`, never truncated.

### BusError
`PayloadTooLarge`, `NoResponders`, `Timeout`, `InvalidSubject`, `InvalidPattern`, `Closed`, `Unsupported(Capability)`, `LeaseHeld`, `LeaseLost`, `Conflict` (CAS), `Decode`. Hand-written `Display`/`Error`; `vizier-core` maps it into `VizierError` through one `From`.

### Capabilities
`{ messaging: bool, leases: bool, shared_state: bool }` plus `missing(&needed) -> Vec<Capability>`.

### Traits
- **Messaging**: `publish(subject, Envelope)`, `request(subject, Envelope, timeout) -> Envelope`, `subscribe(pattern) -> Subscription`. `Subscription::recv() -> Option<(Subject, Envelope)>` (`None` once closed).
- **Leases**: `acquire(name, ttl) -> Lease`, `renew(&Lease)`, `release(Lease)`, `watch(name) -> LeaseWatch`.
- **SharedState**: `get(key) -> Option<Versioned>`, `put(key, bytes) -> Revision`, `cas(key, expected, bytes) -> Result<Revision, Conflict>`, `delete(key)`, `watch(prefix) -> StateWatch`.

### Lease and FencingToken
- `Lease { name, holder, token: FencingToken, expires_at }`.
- `FencingToken(u64)`: strictly increasing per lease name across all holders.
- `LeaseEvent`: `Acquired { holder, token }`, `Expired`, `Released`.

### Bus
`Bus { messaging: Arc<dyn Messaging>, leases: Arc<dyn Leases>, state: Arc<dyn SharedState>, capabilities: Capabilities }`; `Bus::in_process()` builds the default; `require(&Capabilities)` for startup refusal.

### Role contract
`RoleContract { role, subscribes: &[SubjectPattern], publishes: &[Subject-shape], leases: &[&str] }` with `validate()`; one `const` per role, all empty today.

## Engine wire messages (`vizier-core::transport::wire`)

| Channel | Subject | Request kind | Reply kind |
|---|---|---|---|
| Agent turn | `agent.{id}.req` | `request` = `VizierRequestEnvelope { session, request, reply_to, turn_id?, resumed }` | frames on `reply_to`: `VizierResponse` |
| Agent lifecycle | `agents.cmd` | `agent_command` (Create/Update/Delete/Restart/HealthCheck/…) | `AgentCommandResult` |
| Files | `files.cmd` | `file_upload { filename, content }`, `file_resolve { attachment }` | `FileReply { Ok(record/bytes) | Err { message } }` |
| Memory ops | `agent.{id}.memop` | `MemoryOpRequest` | `MemoryOpResponse` or `{ message }` |
| Dreams | `dream.cmd` | `DreamCommand { agent_id, cycle_id }` | none (fire-and-forget) |
| Process | `process.cmd` | `CommandRequest` | `CommandResponse` |
| Session events | `session.{id}.events` | `SessionEvent { session, frame }` | none |
| Integration | `integration.changed` | `IntegrationChanged` | none |

Fields removed from messages because they were not serialisable: `response_tx` (→ `reply_to`), every `oneshot` in command payloads (→ request/reply), `RequestTrace.parent`/`enqueued_at` (→ headers + `queued_at_ms`), `ClientBinding.connection` (→ `node` + `connection_id`). `turn_id` and `resumed` were already `#[serde(skip)]` and are carried as envelope headers so a resumed request survives a hop.

## Client binding
`ClientBinding { node: NodeId, connection_id: String, tools, categories }`. `ClientConnections` (on `VizierDependencies`): `register(conn) -> id`, `get(id) -> Option<Arc<ClientConnection>>`, `remove(id)`. A `None` from `get` means "not on this node".

## State transitions
- **Lease**: `free → held(token n) → (renew)* → expired|released → held(token > n)`. A holder whose `renew` errors is in `lost` and must stop.
- **Inbox**: `open → closed` (on `unregister_agent`); clones observe `is_disconnected`.
- **Reply stream**: `open → closed` when the sender side is dropped or the requester drops the subscription.
