# Quickstart: verifying row ownership, fencing and recovery

**Feature**: `057-fleet-row-ownership`

Two halves: the **single-node checks** run the real binary with a `dummyplug` agent (the constitution's e2e gate) and prove nothing changed; the **two-owner checks** are the storage and recovery tests against PostgreSQL, since the worker and control roles that would run two real nodes do not exist yet (#166–#170).

## 1. Automated tests

```bash
cargo test -p vizier-core fence
```

```bash
cargo test -p vizier-core recovery
```

Against PostgreSQL (what the `managed-storage` CI job runs), from a shell with a disposable database:

```bash
VIZIER_TEST_DATABASE_URL=postgres://postgres:vizier-test@127.0.0.1:5432/vizier cargo test -p vizier-core
```

Expected: every test passes on both backends. The two-owner tests (research R11) report as `on_postgres::…`. Without `VIZIER_TEST_DATABASE_URL` they pass trivially, so CI is the authority for them.

```bash
cargo test --workspace
```

```bash
cargo clippy --workspace
```

## 2. Single node: a turn interrupted by a hard stop resumes (dummyplug)

Setup: `just run` with an agent `dp` on the `dummyplug` provider and a local shell enabled. Open its chat in the WebUI.

1. Send:
   ```json
   {"tool": "execute_python", "arguments": {"code": "shell_exec(command='sleep 30')\nprint('done')"}}
   ```
   Expected: the trail shows `execute_python` running.
2. Within 30 s, kill the process hard (not `just shutdown`): `kill -9` the `vizier` process, or close its terminal tab.
3. `just run` again and reopen the chat.
   Expected, as before this feature: the turn resumes. The open `execute_python` call is sealed with the `Interrupted: vizier restarted …` notice, the reply arrives marked resumed, and the log shows one line for agent `dp` re-sending one pending turn.
4. Stored state: no `session_turn` row is left for `dp` once the reply is in. The `owner_fence` row `agent:dp` has `token` one greater than before the restart, and `lease_token` is null.

## 3. Single node: open records are recovered once (dummyplug)

1. With `dp` running, start a background job:
   ```json
   {"tool": "execute_python", "arguments": {"code": "background_subagents(tasks=[{'prompt': '{\"tool\": \"execute_python\", \"arguments\": {\"code\": \"shell_exec(command=\\'sleep 60\\')\"}}'}])"}}
   ```
   Then create a one-time task for `dp` due in one minute, with the same sleeping request as its prompt.
2. Once the task has fired, kill the process hard, then wait for the piece's and the task's turns to be pending (they are journaled at once).
3. `just run` again.
   Expected, as before this feature: the job and the task run are **picked up**, since their turns are pending and resume. Each ends answered, with one notification each in the bell, not two.
4. Repeat step 1 and kill the process, then delete the `session_turn` rows of `dp` from the database before restarting (`sqlite3 ~/.vizier/vizier.db "DELETE FROM session_turn"`).
   Expected: the job and the run are marked **interrupted**, with exactly one notification each, as the startup sweep did before.

## 4. Single node: nothing is refused

Run a normal conversation with several tool calls, then a dream (`POST /agents/dp/dream`).
Expected: no `ownership moved` line in the log at any level, and history is identical in shape to a build without this feature.

## 5. Two owners (automated only, until #166)

These are the tests in research R11. To read one by hand, run it with `-- --nocapture` against PostgreSQL. The log shows owner A refused with `ownership of agent:… moved to a newer owner` after B's claim, and B's recovery resolving exactly A's rows.
