# Contract: Backend Conformance Suite

`vizier_schema::conformance::run(make_backend)` takes a factory that builds a fresh `Bus` per check and returns a `Report` of named results. A backend crate runs it from its own test; the in-process backend runs it in `vizier-schema`'s tests.

| # | Check name | Rule (bus-semantics) | Passes when |
|---|---|---|---|
| 1 | `request_reply` | 4 | a responder's reply reaches the requester |
| 2 | `no_responder_error` | 4 | a request with no subscriber returns `NoResponders`, promptly |
| 3 | `request_timeout` | 4 | a subscriber that never answers yields `Timeout`, not a hang |
| 4 | `per_subject_ordering` | 2 | 1,000 messages from one publisher arrive in order |
| 5 | `wildcard_matching` | 6 | `*` matches one token only, `>` matches ≥1 trailing tokens, exact matches exactly |
| 6 | `id_encoding_is_one_token` | 7 | an id containing `.` cannot be matched by a sibling's pattern |
| 7 | `payload_limit_refusal` | 5 | a payload of limit+1 bytes is refused, limit bytes is accepted |
| 8 | `publish_without_subscribers` | 3 | publish to an empty subject succeeds |
| 9 | `lease_exclusive` | 8 | a second `acquire` on a held name returns `LeaseHeld` |
| 10 | `lease_expiry_and_takeover` | 9 | an unrenewed lease can be taken after its TTL |
| 11 | `fencing_token_monotonic` | 10 | tokens strictly increase across 5 successive holders |
| 12 | `lease_renew_after_loss_fails` | 11 | the old holder's `renew` errors after takeover |
| 13 | `lease_watch` | 12 | a watcher sees acquire, expire and release |
| 14 | `state_cas` | 13 | CAS succeeds on the right revision, `Conflict` otherwise |
| 15 | `state_watch` | 14 | a prefix watcher sees puts and deletes |

Backends that do not implement a capability skip that group and report it as `skipped` with the capability name; skipping is only legal when the capability is not advertised.

## The suite must bite

A test in `vizier-schema` wraps `InProcess` in a deliberately faulty decorator for each of: answering `Timeout` instead of `NoResponders`, delivering out of order, ignoring the payload limit, reusing a fencing token. For each, `run` MUST report the matching check as failed. (The issue's example is the `Timeout`-for-`NoResponders` fault.)

## Time

Lease checks use `tokio::time::pause`/`advance` so TTL expiry is deterministic and the suite runs in milliseconds.
