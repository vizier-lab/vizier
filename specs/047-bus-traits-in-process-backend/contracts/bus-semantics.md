# Contract: Bus Semantics

This is the text the published page `docs/src/fleet/bus-contract.md` carries. Every statement has a conformance check (see [conformance-suite.md](conformance-suite.md)).

## Messaging

1. **Delivery is at-most-once.** A published message reaches each current subscriber zero or one times. There is no retry, no redelivery and no persistence in the contract.
2. **Ordering.** Messages published by one publisher to one subject are delivered to a given subscriber in publish order. There is no ordering across subjects or across publishers.
3. **Publish never blocks on subscribers.** A slow subscriber may lose messages (and is told it lagged); it never stalls a publisher. A subject with no subscribers is not an error for `publish`.
4. **Request/reply.** `request` publishes with a reply address and waits for the first reply. It returns an error in exactly two cases and never hangs: `Timeout` (a responder existed but did not answer in time) and `NoResponders` (nothing was subscribed when sent).
5. **Payload limit.** An envelope whose payload exceeds 1 MiB is refused at construction or publish with `PayloadTooLarge`; it is never truncated or partly sent.
6. **Subject patterns.** `exact`, `*` (exactly one token), `>` (one or more trailing tokens, last position only). Anything else is an `InvalidPattern`.
7. **Identifiers are one token.** An id containing `.` or any non-token character is percent-encoded into a single token, so an id can never add levels or widen a subscription.

## Leases

8. **Exclusive.** At most one holder per lease name at a time; `acquire` on a held lease returns `LeaseHeld`.
9. **Expiry and takeover.** A lease not renewed within its TTL expires and can be acquired by another holder.
10. **Fencing.** Every acquisition of a name yields a strictly greater fencing token than any earlier one for that name, including across expiry takeovers.
11. **Lost lease means stop.** If `renew` returns an error, the holder has lost the lease and MUST stop the work it protected immediately.
12. **Watch.** `watch` yields acquire, expire and release events for a name.

## Shared state

13. **Compare-and-swap.** `cas(key, expected_revision, value)` succeeds only if the key's current revision equals `expected_revision`, else `Conflict`; revisions increase on every write.
14. **Watch.** `watch(prefix)` yields put and delete events for keys under the prefix.

## Capabilities

15. A backend reports which of messaging, leases and shared state it implements. Startup refuses a role that needs one the backend lacks, naming it.

## Out of contract

Persistence, replay, exactly-once, cross-subject ordering, authentication and encryption are not part of this contract; a backend that adds them must not change the rules above.
