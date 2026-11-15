# Quickstart: verifying subagent workers

**Feature**: `059-subagent-workers`

**Automated**: steps 2–11 are scripted in [`scripts/subagent-e2e.mjs`](../../scripts/subagent-e2e.mjs)
(33 checks). Start dummyplug (`cd dummyplug && node src/server.mjs`) and vizier, then run
`VIZIER_URL=ws://localhost:9999 VIZIER_SETUP_USER=e2e node scripts/subagent-e2e.mjs`; set
`VIZIER_STOP`/`VIZIER_START` (commands that stop and restart vizier) to include step 8. The script
creates an agent `workers` with a local `shell` integration (the shell is an integration, not an
agent field). Checked on 2026-11-14 in WSL Ubuntu: 33/33, three runs in a row.

Setup for every dummyplug step done by hand: `just run` with an agent `dp` on the offline
`dummyplug` provider, a local `shell` integration attached, open its chat in the WebUI. A worker is a turn of `dp` too, so dummyplug
treats a worker's **task text** as its own message: a task that is a dummyplug JSON request makes
the worker run that tool. Dummyplug cannot read a delivery or write a report, so steps check
stored state and history. Steps that need a real model are marked **live**. On Windows use
`Start-Sleep -Seconds N` where a step says `sleep N`.

Below, `SPAWN(t1, t2, …)` means sending:

```json
{"tool": "execute_python", "arguments": {"intent": "spawn workers", "code": "r = spawn_subagents(tasks=[{'task': t1}, …])\nr"}}
```

with each `t` a JSON string such as
`{"tool": "execute_python", "arguments": {"intent": "slow", "code": "shell_exec(command='sleep 20')\n'done'"}}`.

## 1. Automated tests

```bash
cargo test -p vizier-core subagent
```

```bash
cargo test -p vizier-core delivery
```

```bash
cargo test -p vizier-core fence
```

```bash
cargo test -p vizier-core recovery
```

```bash
cargo test -p vizier-http
```

```bash
VIZIER_TEST_DATABASE_URL=postgres://postgres:vizier-test@127.0.0.1:5432/vizier cargo test -p vizier-core
```

```bash
cargo clippy --workspace
```

Expected: all pass. Unit tests cover the worker prompt layout and frontmatter, availability by
depth (default and raised), every guarded transition, a claim racing a wait on one outcome, a
second question after a reply, timers re-armed on recovery, and one fence row per storage method.

## 2. The worker framing (dummyplug)

`SPAWN("context")` — dummyplug answers `context` with the context block it received.
Expected: the worker's history shows a `subagent_task` request whose frontmatter has
`sender: dp`, `via: subagent`, `task: 1 of 1`; its system prompt (debug log, or `context`) starts
with the worker framing, carries the soul under `## The agent you work for`, and has no BOOT.md and
no configured system prompt.

## 3. Spawn returns at once (dummyplug)

`SPAWN(slow, slow)`. Expected: the call returns in under a second with two `running` summaries in
order (`position` 1 and 2), and `list_subagents()` from a second script shows both.

## 4. Wait for any, stop one, wait for all (dummyplug)

With a fast task (`'quick'`) and a slow one: `SPAWN(quick, slow)`, then
`wait_subagents(ids=[a, b], until='any')` → `met: true`, `a` answered, `b` running;
`stop_subagent(id=b)` → `stopped`, `by_agent`; `wait_subagents(ids=[a, b], until='all')` →
`met: true`. No delivery arrives for either (both seen or stopped).

## 5. Ask and reply (dummyplug)

Task: `{"tool": "execute_python", "arguments": {"intent": "ask", "code": "ask_main_agent(question='which tier?')"}}`.
`SPAWN(task)`, `wait_subagents(ids=[a])` → `a` is `asking` with `question: which tier?`.
`reply_subagent(id=a, text='context')` → `running`; `wait_subagents(ids=[a])` → `answered`, and
the worker's history holds the first step, the `subagent_reply` request and the reply's answer in
one conversation. `reply_subagent` on the answered worker → `status: not_asking`.

## 6. Workers survive the parent's `/abort` (dummyplug)

`SPAWN(slow)`, then send `/abort` in the chat. Expected: the worker stays `running`; when it ends
(`sleep 20`), a new turn of `dp` starts whose request is the background delivery with a
`[Subagent update] … state: answered` section, and it arrives once (the worker row's
`delivered_seq` equals its `outcome_seq`).

## 7. Mid-turn delivery (dummyplug)

`SPAWN(quick)` then, in the same script, `shell_exec(command='sleep 5')`. Expected: the delivery is
added at the next step boundary of the running turn (a `detached_delivery` history entry inside
the turn), and no extra turn starts afterwards.

## 8. Restart resumes a worker (dummyplug)

`SPAWN(slow)`, then `just shutdown` and `just run` within the sleep. Expected: the worker's turn is
re-sent `resumed`, the worker settles `answered`, and exactly one delivery arrives.

## 9. Only the main agent hires (dummyplug)

Task: `{"tool": "list_tool_functions", "arguments": {"category": "subagents"}}`. Expected: the
worker's result lists only `ask_main_agent`. In `dp`'s own chat the same call lists the six
management tools and not `ask_main_agent`. With `subagents: {max_depth: 2}` in the seed config, the
worker's list includes the six tools.

## 10. Retired names (dummyplug)

Send `{"tool": "execute_python", "arguments": {"intent": "old", "code": "parallel_subagents(tasks=[{'prompt': 'x'}])"}}`.
Expected: the script fails with guidance naming `spawn_subagents` and `wait_subagents`; the same for
`paralel_subtasks`; neither names a retired tool.

## 11. Deleting the conversation stops its workers (dummyplug)

`SPAWN(slow)`, delete the conversation. Expected: the worker's session is aborted, its row and
history are gone, and no delivery is attempted.

## 12. Report shape (live)

With a real model, ask the agent to split a small research question across two workers. Expected:
each worker's answer ends with what it did, what it could not do, and how sure it is, and contains
no greeting or "let me know" addressed to a person (SC-001).

## 13. Peek at the workers (WebUI, dummyplug)

Open the conversation from step 5 in the WebUI while one worker is asking. Expect: the status
strip shows a Subagents item (people icon) counting the unfinished workers with an amber dot; its
panel groups the workers by spawn, the asking row shows the question in amber, and expanding a
row shows the task, the worker's steps as the chat's activity trail and, once answered, its
result. `GET …/topic/{topic}/subagents` lists the same rows; another person's topic or an unknown
id answers 404. There is no stop or reply control.
