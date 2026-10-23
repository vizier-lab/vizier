# Baseline (before changes, branch 023-persist-turn-activity)

- `cargo test`: 532 passed, 0 failed, 7 ignored.
- `cargo clippy`: 86 warning/error lines pre-existing (count of `^(warning|error)` lines).
- `cd webui && npm run typecheck`: clean.
- `reference-history.json`: NOT yet captured (needs a running binary with a dummyplug agent); captured in T013 against the pre-change behaviour is no longer possible, so compare against the documented kinds/order instead.
