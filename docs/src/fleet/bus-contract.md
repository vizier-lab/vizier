# Fleet Bus Contract

The Fleet roles (HTTP gateway, gRPC gateway, workers, channel readers, control) talk to each other
over a **bus**. The contract lives in the `vizier-schema` crate, which depends on no other
workspace crate: the engine and the roles depend on it, never the other way round. This page is the
text of that contract. Every rule below is checked by a named test in the backend conformance suite
(see [Rules and checks](#rules-and-checks)).

The bus is three traits bundled in a `Bus`, plus a description of what the backend behind them
implements:

- `Messaging`: `publish`, `request` and `subscribe`.
- `Leases`: `acquire`, `renew`, `release` and `watch`, with fencing tokens.
- `SharedState`: `get`, `put`, `cas`, `delete` and `watch`.
- `Capabilities`: which of the three a backend implements.

Today one backend exists, `InProcess`, and the whole server runs on it with no change in behaviour.
A networked backend only has to implement the traits and pass the suite.

## Messaging

1. **Delivery is at-most-once.** A published message reaches each current subscriber zero or one
   times. There is no retry, no redelivery and no persistence in the contract.
2. **Ordering.** Messages published by one publisher to one subject are delivered to a given
   subscriber in publish order. There is no ordering across subjects or across publishers.
3. **Publish never blocks on subscribers.** A slow subscriber may lose messages (and is told it
   lagged); it never stalls a publisher. A subject with no subscribers is not an error for
   `publish`.
4. **Request and reply.** `request` publishes with a reply address and waits for the first reply.
   It returns an error in exactly two cases and never hangs: `Timeout` (a responder existed but did
   not answer in time) and `NoResponders` (nothing was subscribed when sent).
5. **Payload limit.** An envelope whose payload exceeds 1 MiB is refused with `PayloadTooLarge`; it
   is never truncated or partly sent. Anything bigger travels by reference: the engine stages file
   bytes in the blob store and sends the key.
6. **Subject patterns.** `exact`, `*` (exactly one token) and `>` (one or more trailing tokens,
   last position only). Anything else is an `InvalidPattern`.
7. **Identifiers are one token.** An id containing `.` or any non-token character is
   percent-encoded into a single token, so an id can never add levels or widen a subscription.

## Leases

8. **Exclusive.** At most one holder per lease name at a time; `acquire` on a held lease returns
   `LeaseHeld`.
9. **Expiry and takeover.** A lease not renewed within its TTL expires and can be acquired by
   another holder.
10. **Fencing.** Every acquisition of a name yields a strictly greater fencing token than any earlier
    one for that name, including across expiry takeovers. A storage write can reject a stale
    leader by its token.
11. **Lost lease means stop.** If `renew` returns an error, the holder has lost the lease and must
    stop the work it protected immediately.
12. **Watch.** `watch` yields acquire, expire and release events for a name.

## Shared state

13. **Compare-and-swap.** `cas(key, expected_revision, value)` succeeds only if the key's current
    revision equals `expected_revision` (`None` meaning the key must not exist), else `Conflict`;
    revisions increase on every write.
14. **Watch.** `watch(prefix)` yields put and delete events for keys under the prefix.

## Capabilities

15. A backend reports which of messaging, leases and shared state it implements. Startup refuses a
    role that needs one the backend lacks, naming it (`vizier run` does this for every selected
    role before serving).

## Out of contract

Persistence, replay, exactly-once delivery, ordering across subjects, authentication and encryption
are not part of this contract. A backend that adds them must not change the rules above.

## Subjects

A subject is dot-separated tokens of `[A-Za-z0-9_%~-]`, at most 255 bytes, no empty token. An
identifier is written as one token by percent-encoding every byte outside `[A-Za-z0-9_-]`, so
`my.agent` becomes `my%2Eagent`.

| Subject | Carries | Direction |
|---|---|---|
| `agent.{id}.req` | a turn's request | into an agent's inbox |
| `reply.{uuid}` | the frames of one request, then an end frame | back to the requester, who subscribes first |
| `agent.{id}.memop` | a memory operation | request and reply |
| `agents.cmd` | an agent lifecycle command | request and reply |
| `files.cmd` | a file upload or read | request and reply (bytes are staged in the blob store) |
| `dream.cmd` | a dream trigger | fire and forget |
| `process.cmd` | a command from the command socket | request and reply |
| `session.{id}.events` | a session event | fan-out; subscribers use `session.>` |
| `integration.changed` | an integration write | fan-out |
| `lease.{name}` | the lease name space | the `Leases` trait |

The `gateway.{node}.client` subject is reserved for the HTTP gateway work (#169) and is not created
yet. The process exit signal and a client tool's live socket stay local to the process: they are
that process's own lifecycle, not traffic between roles.

## Messages

Every message is an `Envelope`: a version (`v`, currently 1; a higher one is a decode error), a
`kind`, an optional `reply_to`, headers (`traceparent` and `tracestate` for W3C trace context, plus
`x-vizier-turn-id` and `x-vizier-resumed` on a request), the unix-millisecond time it was first
queued, and a JSON payload. Unknown JSON fields are ignored, so adding an optional field is not a
version bump.

A reply stream ends with an explicit end frame when the sender goes away, which is how a requester
learns its agent stopped.

## Per-role contract

Each role has a `RoleContract` constant listing the patterns it subscribes to, the subjects it
publishes and the leases it takes, with a `validate()` that checks any entry added. They are empty
today; the sub-issue that builds the role fills its row in.

| Role | Needs from the bus | Subscribes | Publishes | Leases | Filled in by |
|---|---|---|---|---|---|
| `gateway-http` | messaging | none yet | none yet | none yet | #169 |
| `gateway-grpc` | messaging | none yet | none yet | none yet | #170 |
| `worker` | messaging, leases | none yet | none yet | none yet | #168 |
| `channels` | messaging, leases | none yet | none yet | none yet | #167 |
| `control` | messaging, leases | none yet | none yet | none yet | #166 |

## Testing a role alone

`vizier-fleet::testing::Harness` builds a role on an in-process bus with embedded storage, and a
`ScriptedPeer` plays its neighbours. See the module documentation of `vizier-fleet`'s `testing`
module and `crates/vizier-fleet/tests/roles.rs`.

## Writing a backend

Implement `Messaging`, `Leases` and `SharedState` (or only the ones you support, and advertise
exactly those in `Capabilities`), then run the suite from your crate's own test:

```rust
#[tokio::test(start_paused = true)]
async fn my_backend_passes_conformance() {
    let report = vizier_schema::conformance::run(|| my_backend_bus()).await;
    assert!(report.is_ok(), "\n{report}");
}
```

The suite waits out lease TTLs with `tokio::time::sleep`, so run it on a paused clock. A group for a
capability the backend does not advertise is reported as skipped. The suite is tested against
deliberately faulty backends (a timeout in place of `NoResponders`, reordered messages, an ignored
payload limit, a reused fencing token) to prove it fails when it should.

## Rules and checks

| Rule | Conformance check |
|---|---|
| 1 | not testable on one process; covered by rule 3's check |
| 2 | `per_subject_ordering` |
| 3 | `publish_without_subscribers` |
| 4 | `request_reply`, `no_responder_error`, `request_timeout` |
| 5 | `payload_limit_refusal` |
| 6 | `wildcard_matching` |
| 7 | `id_encoding_is_one_token` |
| 8 | `lease_exclusive` |
| 9 | `lease_expiry_and_takeover` |
| 10 | `fencing_token_monotonic` |
| 11 | `lease_renew_after_loss_fails` |
| 12 | `lease_watch` |
| 13 | `state_cas` |
| 14 | `state_watch` |
| 15 | `Bus::require` unit test (`require_names_the_missing_capability`) and the harness test in `vizier-fleet/tests/roles.rs` |
