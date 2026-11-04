# Quickstart: Fleet Agent Service Seam

**Feature**: #163 | **Date**: 2026-11-03

How to work on this feature, and how to prove each part of it.

---

## The shape of the work

Six slices, each one commit, in dependency order:

| # | Slice | Lands in |
|---|-------|----------|
| 1 | The contract: `agent.{id}.cmd`, the `AgentsClient` trait, its error vocabulary | `vizier-schema`, `vizier-core` |
| 2 | The local implementation and the fake | `vizier-core`, `vizier-fleet` |
| 3 | The host/placement split | `vizier-core` |
| 4 | Health and setup in shared state; the deferred claim on a stored row | `vizier-core` |
| 5 | The audit | `specs/163-agent-service-seam/contracts/agent-state-audit.md` |
| 6 | The mechanical check | `.github/workflows/ci.yml` |

The order matters: the contract is written before the code that implements it, so the local
implementation is built against the same document the networked one will be.

---

## Before you start

```powershell
just install
```

Then establish the baseline **before** changing anything - the Rust side is already partly red on
Windows:

```powershell
cargo test -p vizier-core --lib
cargo test -p vizier-schema
cargo test -p vizier-fleet
```

Two Windows-only failures are known and pre-existing (confirmed on a stashed tree):
`projects::path::tests::absolute_escaping_and_missing_paths_are_refused` and
`skill::scopes::tests::a_slug_in_both_project_roots_appears_once_and_agents_wins`. Note them; do not
count them as yours.

**The baseline, recorded on this branch** (`cargo test -p vizier-core --lib` before any code
change):

```
test result: FAILED. 1342 passed; 2 failed; 7 ignored; 0 measured; 0 filtered out
failures:
    projects::path::tests::absolute_escaping_and_missing_paths_are_refused
    skill::scopes::tests::a_slug_in_both_project_roots_appears_once_and_agents_wins
```

`cargo test -p vizier-schema` and `cargo test -p vizier-fleet` were green before any code change.

**After the work** (`cargo test -p vizier-core --lib`): `1346 passed; 2 failed` - the same two
failures, and four more tests (the seam's own). `vizier-schema` 19+4+1, `vizier-fleet` 2+3 and
`vizier-http` 35+1+2+1 all green, with no test changed to accommodate the work.

---

## Slice 1 - the contract

**What to build**

- `Subject::agent_cmd(id)` in `crates/vizier-schema/src/subject.rs`, next to `agent_req` and
  `agent_memop`: `agent.{id}.cmd`, with the id percent-encoded into one token.
- The `AgentsClient` trait in a new `crates/vizier-core/src/agents_client.rs`, and
  `pub mod agents_client;` in the stable tier of `lib.rs` with the prelude export.

**How to prove it**

```powershell
cargo test -p vizier-schema subject
cargo test -p vizier-core --lib agents_client
```

A test asserts the new subject is one token even for an id containing a dot, and that it matches the
pattern `agent.*.cmd` but not `agent.*.req`.

---

## Slice 2 - the local implementation and the fake

**What to build**

- `LocalAgentsClient`: a typed wrapper over `VizierTransport` that reaches the agent manager over the
  bus. It holds no reference to `VizierAgents`.
- `FakeAgentsClient`: records what it was asked for and answers from a script.
- `pub agents: Arc<dyn AgentsClient>` on `VizierDependencies`.

**How to prove it**

The gateway-side test runs with **no agent host running**:

```powershell
cargo test -p vizier-fleet --test roles
```

It asks the fake for an agent's health and for a turn, asserts the fake received what the gateway
sent, and asserts that an "unavailable" answer reaches the caller as the same error the person sees
today.

---

## Slice 3 - the host and the placement

**What to build**

- `AgentHost` in `crates/vizier-core/src/agents/mod.rs`: what `VizierAgents` is today, minus the
  decision of *which* agents to run.
- `Placement` in a new `crates/vizier-core/src/agents/placement.rs`, with the single-node
  implementation.

**How to prove it**

```powershell
cargo test -p vizier-core --lib agents
```

- The host runs alone: built from a bus handle, a storage handle and one agent, it answers a
  scripted request. No gateway is running.
- The single node is unchanged: every stored agent is assigned to the one local host, and the
  existing agent tests pass **without being modified**.

**The guarantees to check by hand** (they are the ones a refactor can quietly break):

| Guarantee | How to check |
|---|---|
| Built before persisted or swapped | The agent tests that cover a failed build |
| A queued request survives a restart | The test that queues during a swap |
| An update resumes the running turns | The update test |
| A delete drops them | The delete test (#52) |

---

## Slice 4 - the published views and the claim

**What to build**

- The host publishes `agent.{id}.health` and `agent.{id}.setup` to shared state, with a deadline of
  at most 10 s in the value.
- The gateway reads them and reports *unknown* for a missing or expired entry.
- `client_tool_call` gains `claimed_by` and `claimed_at`; the claim is one conditional update.

**How to prove it**

```powershell
cargo test -p vizier-core --lib shared_state
cargo test -p vizier-core --lib client_call
```

- An expired entry reads as unknown, not as a stale value.
- Two claims on one deferred call: the first changes the row, the second does not.
- A claim older than the timeout is free again.

---

## Slice 5 - the audit

Fill in `specs/163-agent-service-seam/contracts/agent-state-audit.md`. Every row must name the old
source, the new one and the kind of state. An entry with no replacement is marked **open**, not left
blank.

The starting table is already filled in from the sweep done during planning; the work is to confirm
each row against the code as it stands after slices 1-4, and to move the one open entry.

---

## Slice 6 - the mechanical check

A step in the `policy` job of `.github/workflows/ci.yml`.

**How to prove it bites**

1. Add a deliberate read of a running agent's state to `crates/vizier-core/src/service/`.
2. Run the check. It must fail **and name the file and the line**.
3. Remove the deliberate read. The check passes again.

A check that has never been seen to fail is not a check.

---

## The mechanical check, and how it was proven to bite

`scripts/163-agent-state-read-check.sh`, run from the CI `policy` job. It refuses a read of a
running agent's state from `crates/vizier-http/src` or `crates/vizier-core/src/service`, and
names the file and the line of each one it finds.

**The proof** (2026-11-03): a deliberate `deps.transport.agent_health()` was added to
`crates/vizier-core/src/service/agents.rs`, the check was run, and it failed with

```
::error::crates/vizier-core/src/service/agents.rs:789 reads a running agent's state: let _deliberate = deps.transport.agent_health().await;
```

The deliberate read was removed and the check passed again. The two reads that stay are the
audited exceptions (the channel-reader health, and this process's own setup board), each named
in `contracts/agent-state-audit.md`.

The check's output on a clean tree:

```
audited exception: crates/vizier-core/src/service/integrations.rs:206 status_of(&s.deps().channel_board, &integration.id)
audited exception: crates/vizier-core/src/service/setup_state.rs:19 let mut setups = deps.setup.snapshot();
agent-state audit: 19 rows, 5 handles covered, 2 audited exceptions
agent-state read check: no un-audited direct read of a running agent's state
```

## The gates before the work is done

```powershell
cargo clippy --workspace --all-targets -- -D warnings -A clippy::style -A clippy::complexity -A clippy::perf
cargo test --workspace
cargo test -p vizier-schema
cargo test -p vizier-fleet
```

**The clippy flags matter.** CI's set is `-D warnings -A clippy::style -A clippy::complexity
-A clippy::perf` (`.github/workflows/ci.yml`): the tree carried about a hundred style,
complexity and perf findings before CI existed, and CI lets them be warnings. A bare
`-D warnings` fails on those, not on this work.

**On Windows** `cargo clippy --workspace` also fails to compile `daemonize`, which is
Unix-only. That is a platform limitation, not a finding: CI runs on Linux. The four crates
this feature touches - `vizier-schema`, `vizier-core`, `vizier-http`, `vizier-fleet` - are
clean under the flags above on Windows too.

Plus the agent-observable gate: the turn path changes, so the dummyplug end-to-end agent turn runs
(the constitution's e2e gate). The conformance suite and the offline end-to-end agent turn must pass
**unchanged** (FR-022).

**The e2e gate, run on this branch (2026-11-03)**: two real dummyplug turns, both green.

| Test | What it does |
|---|---|
| `crates/vizier-core/tests/dummyplug_e2e.rs` -> `dummyplug_agent_answers_through_the_runtime` | one agent on the offline `dummyplug` provider, driven through `VizierRuntime` only, answering a real turn |
| `crates/vizier-http/tests/topic.rs` -> `a_turn_looks_the_same_over_the_socket_and_the_dispatcher` | the same turn over the WebSocket and over the desktop dispatcher, compared frame by frame |

Both start the Node `dummyplug` server from the submodule on a free port and skip themselves
without Node. The `vizier` **binary** is not built on Windows - `daemonize` is Unix-only, so
`cargo build -p vizier` fails there - but these two tests drive the same turn through the same
engine, which is what the gate is about.

**No submodule branch is needed.** The work touches no submodule - the WebUI does not change
(FR-019). The spec branch `163-agent-service-seam` exists in the workspace only, like the Fleet specs
before it.

---

## The conventions that apply

- Conventional commits, one slice per commit: `feat:`, `refactor:`, `doc:`, `test:`.
- Stage by path. `git add -A` would sweep the perpetually dirty `app/src-tauri/gen/schemas`.
- The public HTTP API's shapes do not change, so there is no WebUI work and no `design/` or `webui/`
  commit.

---

## Where the decisions are written down

| Question | Answer |
|---|---|
| Where does the seam live? | `vizier-core`, stable tier - [research.md](research.md) R1 |
| Why does the local implementation talk over the bus? | So the contract is the same one the networked implementation will use - R2 |
| Which subjects? | `agent.{id}.req`, `agent.{id}.cmd` (new), `agents.cmd` for creates - R3 |
| How is a turn aborted? | The existing `Command("abort")` request - R4 |
| What does the host run? | What `Placement` tells it - R5 |
| How does shared state expire? | The deadline is in the value; the backend has no TTL - R6 |
| How is a deferred claim exclusive? | A conditional update on the stored row - R7 |
| How is the mechanical check built? | A path-aware grep in the CI `policy` job - R8 |
| What about the tool listing and MCP status? | Not separate gateway-side reads - R9 |
| What about the process-wide registries? | They stay; the audit says why - R10 |
| How are "no owner" and "owner lost" told apart? | `NoResponders` vs `Timeout` on the bus - R11 |
