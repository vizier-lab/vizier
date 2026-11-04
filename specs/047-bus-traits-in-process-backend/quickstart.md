# Quickstart: Verifying the Fleet Bus

Run from the repository root. Steps 1–5 need no network and no keys; step 6 uses the offline `dummyplug` provider.

## 1. The contract crate and backend

```sh
cargo test -p vizier-schema
```

Expect: all unit tests, the conformance suite against `InProcess` (all 15 checks pass), and the faulty-backend tests (each fault fails exactly its matching check).

## 2. Layering policy

```sh
cargo metadata --format-version 1 --locked | jq -r '.packages[] | select(.name=="vizier-schema") | .dependencies[] | select(.kind==null) | .name'
```

Expect: only `serde`, `serde_json`, `tokio`, `async-trait`, `uuid` (and similar external crates). No `vizier-*`, `async-nats`, `tonic`, `axum*`, `clap`.

## 3. The role harness and sample role test

```sh
cargo test -p vizier-fleet --test roles
```

Expect: the sample test builds a role from `Harness`, a `ScriptedPeer` publishes on a contract subject, and the role's published reply is asserted.

## 4. The engine, unchanged

```sh
cargo test -p vizier-core --lib
cargo test -p vizier-http
cargo test --workspace
cargo clippy --workspace --all-targets
cargo test -p vizier-core --no-default-features
```

Expect: every pre-existing test passes. The one rewritten test is `a_piece_with_no_terminal_frame_has_no_outcome` (previously `a_closed_channel_without_an_answer_reads_as_a_restart`). On Windows, `projects::path` and `skill::scopes` have known pre-existing failures.

## 5. Transport behaviour tests

```sh
cargo test -p vizier-core transport
```

Expect: request round trip over the bus; an unregistered agent refuses at once; an inbox closes on unregister; the trace header round-trips and `queue wait` is computed from `queued_at_ms`; a realistic chat request survives the envelope; a client binding with an unknown connection id reads as closed (a deferrable tool defers, others return `client_disconnected`).

## 6. End to end through a dummyplug agent (constitution gate)

Start dummyplug (`cd dummyplug && npm start`, or set `DUMMYPLUG_URL`), then:

```sh
cargo run --example headless -- --workspace "$TMPDIR/vizier-bus"
```

Expect: the example creates an agent, sends one message and prints the reply, then shuts down cleanly (exit code 0). Then run the server and use a dummyplug agent through the real loop:

```sh
just run
```

| Step | Send | Expect |
|---|---|---|
| a | `tools` | the tool list, unchanged from before this feature |
| b | a tool name (e.g. `list_skills`) | dummyplug asks for the JSON input |
| c | the JSON request | the tool result is echoed, streamed in order |
| d | `context` | the per-request context block, as before |
| e | WebUI: open the same conversation in two tabs during a turn | both tabs receive the remaining frames (session events) |
| f | edit the agent (change its name/description) mid-conversation | queued request is served by the new instance; no `AgentUnavailable` |
| g | `background_subagents` with one piece, then wait | the report arrives in the originating session once |
| h | `vizier agent ps` in another terminal | lists the running agents (command socket over the bus) |
| i | `vizier shutdown` | the server stops with exit code 0 |

## 7. Docs

```sh
grep -n "bus-contract" docs/src/SUMMARY.md
```

Expect: the page is linked, and every statement in `contracts/bus-semantics.md` appears on it.

## Live-provider steps

None. Nothing here depends on real model output.
