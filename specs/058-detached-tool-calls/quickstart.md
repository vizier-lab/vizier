# Quickstart: verifying detached scripts

**Feature**: `058-detached-tool-calls`

Setup for every dummyplug step: `just run` with an agent `dp` on the offline `dummyplug` provider, local shell enabled, open its chat in the WebUI. Dummyplug runs one tool per message and then writes a canned reply, so it **cannot read** a delivery; each step checks history and stored state instead. On Windows use `Start-Sleep -Seconds N` where a step says `sleep N`. Steps that need a real model are marked **live**.

## 1. Automated tests

```bash
cargo test -p vizier-core detached
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

Against PostgreSQL (what the `managed-storage` CI job runs), from a shell with a disposable database; without the variable these tests pass trivially, so CI is the authority:

```bash
VIZIER_TEST_DATABASE_URL=postgres://postgres:vizier-test@127.0.0.1:5432/vizier cargo test -p vizier-core
```

```bash
cd webui && npm run typecheck && npx vitest run
```

```bash
cargo clippy --workspace
```

```bash
cargo test --workspace
```

Expected: all pass. The unit tests cover the launch refusals, the catalogue of a detached script (live-turn tools absent), the claim (including two racing claimers), cancel versus finish racing, the cascade, redaction of a stored outcome, and a task whose turn's sender is closed.

## 2. Detach returns at once (dummyplug)

1. Send:
   ```json
   {"tool": "execute_python", "arguments": {"intent": "run a slow command", "detached": true, "code": "shell_exec(command='sleep 10')\n'done'"}}
   ```
   Expected: the call returns in under a second with `{"detached": true, "id": "d_…", "state": "running", …}`, and the trail shows it as running in the background. The Background work item in the status strip shows `1` with a live dot **within 2 seconds** of sending (SC-004; time it).
2. `GET /api/v1/agents/dp/channel/<channel>/topic/<topic>/detached-scripts` (or open the panel): one script, `running`, with the intent as its title.

## 3. Idle delivery wakes the conversation (dummyplug)

Continue from step 2 and wait for the sleep to end.
Expected: within a second of it ending, a new turn appears in the conversation whose request is the delivery (`[Background work finished]`, the id, `succeeded`, `result: done`), the agent replies, and the script reads `delivered: true`. A bell notification (`background_work`) arrives once.

## 4. Mid-turn delivery goes into the next round (dummyplug)

1. Send step 2's request with `sleep 1` in place of `sleep 10`; it returns its handle at once.
2. Immediately send an ordinary inline script: `{"tool": "execute_python", "arguments": {"intent": "stay busy", "code": "shell_exec(command='sleep 5')\n'done'"}}`.
Expected: the detached script ends while the second turn's script is still running, and nothing is injected into it (its `result` is `done`). History for the second turn then shows, **before** the agent's closing reply and inside the same turn, one user message `[Background work finished]`. The conversation gets **no extra turn**, and the script reads `delivered: true`.
Also check: the queued delivery request found no rows and produced no turn (log line `detached delivery: nothing to deliver`).

## 5. A turn that closes just as a script ends (dummyplug)

Run step 2 with `sleep 3` and an inline `sleep 3` script right after it, ten times.
Expected: every script is delivered exactly once, either in the turn's closing round or as one wake-up turn, never both and never neither (count `delivered_at IS NOT NULL` rows against delivery messages in history: equal).

## 6. A detached sub-agent fan-out (dummyplug)

`parallel_subagents` runs its pieces on the calling agent, each in a fresh `Subagent` conversation. Send to `dp`:
```json
{"tool": "execute_python", "arguments": {"intent": "fan out two things", "detached": true, "code": "r = parallel_subagents(tasks=[{'prompt': 'tools'}, {'prompt': 'tools'}])
[x['state'] for x in r['results']]"}}
```
Expected: one row; one delivery carrying `["answered","answered"]` when the script ends; each piece exists under its own `Subagent` session. No error above `debug` in the log from hooks that could not publish to the ended turn.

## 7. Inspect and cancel (dummyplug)

1. Detach `shell_exec(command='sleep 60')`; send `{"tool": "get_detached_script", "arguments": {"id": "<id>"}}`.
   Expected: intent, state `running`, elapsed, limit, `tool_calls` listing `shell_exec` once it has started, no `outcome`.
2. Open the Background work panel: the script shows with no cancel control (WebUI cancel is deferred); expand it and see its code, the calls made so far, and no result yet, as the activity trail would show it.
3. Send `{"tool": "cancel_detached_script", "arguments": {"id": "<id>"}}`.
   Expected: the script reads `cancelled` (the panel updates without a reload), the sleeping process is gone, no delivery turn and no notification appear, and a repeat cancel answers `already_finished`.
4. Detach a script that calls `parallel_subagents` whose piece detaches its own script; cancel the outer.
   Expected: the outer and the inner both read `cancelled`.

## 8. Refusals and what is absent (dummyplug)

- A detached script that calls `ask_question(...)` ends `failed` with a `NameError`; `list_tools()` inside it does not list `ask_question` or any `ui_*` function.
- Detaching four levels deep (each level a sub-agent that detaches) is refused with `nesting limit (3) reached`.
- In a scheduled-task run, `execute_python` with `detached: true` answers `cannot detach here: …`.
- A detached call with `timeout_secs: 0` answers the range error; an inline call with `timeout_secs` set runs as usual (the field is ignored).

## 9. Timeout (dummyplug)

`{"intent": "too slow", "detached": true, "timeout_secs": 2, "code": "shell_exec(command='sleep 30')"}`.
Expected: after about 2 s the script reads `timed_out`, the process is gone, and the delivery says so.

## 10. Restart (dummyplug, the constitution's runtime gate)

1. Detach `sleep 60`, then kill the process hard.
2. `just run` again.
   Expected: the script reads `interrupted`; the agent is woken once with a delivery that says the server restarted; no row is left `running`; the `owner_fence` token for `agent:dp` went up by one.
3. Scale (SC-002, automated in `recovery_tests.rs`): with 50 running rows across several sessions, a claim of the agent marks all 50 `interrupted` and the deliveries that follow mark each delivered exactly once.

## 10b. Agent update and delete (dummyplug)

1. Detach `sleep 60`, then change `dp`'s configuration in the WebUI and save (an update restarts the instance).
   Expected: the script reads `interrupted` at once, the sleeping process is gone, and the new instance is woken once with the interruption.
2. Detach `sleep 60` again, then delete `dp`.
   Expected: the sleeping process is gone and no `detached_script` row of `dp` is left.

## 10c. A Workspace (dummyplug)

In a Workspace with `dp` as a member, mention `@dp` with step 2's request.
Expected: when the script ends, `dp` is woken in its own session for that topic, and its reply appears on the Workspace timeline like any reply to a mention.

## 11. Secrets (dummyplug)

`set_secret` a value, then detach a script that returns `get_secret(name=…)`.
Expected: the stored `outcome`, the delivery message, history and logs show `[redacted]` where the value would be.

## 12. Files (dummyplug)

Detach a script that writes a file with `write_file`-style tool output (an artifact or attachment-producing tool).
Expected: the file is a session file of the conversation and is named in the delivery.

## 13. Gone (static checks)

```bash
git grep -nE "background_subagents|list_background_jobs|cancel_background_job|BackgroundReport|background_job" -- crates webui/app docs CLAUDE.md
```

Expected: only the `RETIRED_TOOL_NAMES` entries, the `DROP TABLE` and notification `DELETE` migration lines and the #82 checklist entry (SC-007). After a first start on an existing workspace, `SELECT COUNT(*) FROM notification WHERE category = 'background_job'` is 0.

## 14. Live (needs a real model)

**Live**: ask an agent on a real provider to "check these three pages and run the tests in the background, and tell me while it works". Expected: it detaches without being told the syntax (the guidance is enough), keeps talking, and reacts to the delivery in its next round. This is the only step that tests the wording of the guidance.

## 15. WebUI

With the build from `just build`: the status strip shows Background work with a live dot and a count; the panel matches the redrawn frames of `Vizier shell.figcraft` in both themes; entries collapse and expand; the panel closes on ×, Escape and an outside press (Dismissal convention); a second tab sees the same state without reloading.
