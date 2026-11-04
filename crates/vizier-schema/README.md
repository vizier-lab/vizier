# vizier-schema

The contract of the Fleet bus (vizier-lab/vizier#162): subjects, envelopes, the `Messaging`,
`Leases` and `SharedState` traits, an in-process backend and a conformance suite any backend
can run. It depends on no other workspace crate, so the engine, the roles and a future network
backend can all share it.
