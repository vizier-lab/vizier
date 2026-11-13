# Quickstart: verifying detached tool calls

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

```bash
cd webui && npm run typecheck && npx vitest run
```

```bash
cargo clippy --workspace
```

```bash
cargo test --workspace
```

Expected: all pass. The unit tests cover the reserved-kwarg parse, the refusals, the claim (including the group rule and two racing claimers), cancel versus finish racing, the cascade, and redaction of a stored outcome.

## 2. Detach returns at once (dummyplug)

1. Send:
   ```json
   {"tool": "execute_python", "arguments": {"intent": "start a slow command", "code": "h = shell_exec(command='sleep 10', detached=True)\nh"}}
   ```
   Expected: the call returns in under a second with `result` = `{"detached": true, "id": "d_…", "tool": "shell_exec", "state": "running", …}`. The Background work item in the status strip shows `1` with a live dot.
2. `GET /api/v1/agents/dp/channel/<channel>/topic/<topic>/detached-calls` (or open the panel): one call, `running`.

## 3. Idle delivery wakes the conversation (dummyplug)

Continue from step 2 and wait for the sleep to end.
Expected: within a second of it ending, a new turn appears in the conversation whose request is the delivery (`[Background work finished]`, the call's id, `succeeded`), the agent replies, and the call reads `delivered: true`. A bell notification (`background_work`) arrives once.

## 4. Mid-turn delivery goes into the next round (dummyplug)

1. Send:
   ```json
   {"tool": "execute_python", "arguments": {"intent": "start one and wait out a second", "code": "shell_exec(command='sleep 1', detached=True)\nshell_exec(command='sleep 3')\n'done'"}}
   ```
   Expected: the detached call ends while the script is still running, and nothing is injected into the script (its `result` is `done`). History for this turn then shows, **before** the agent's closing reply and inside the same turn, one user message `[Background work finished]` with the call. The conversation gets **no second turn**, and the call reads `delivered: true`.
2. Check the claim happened once: `delivered_at` is set, and the queued delivery request found no rows and produced no turn (log line `detached delivery: nothing to deliver`).

## 5. A turn that closes just as a call ends (dummyplug)

Send the step 4 script with `sleep 1` replaced by a call that ends right at the end of the script (`sleep 3` detached, `sleep 3` inline). Repeat ten times.
Expected: every call is delivered exactly once, either in the turn's closing round or as one wake-up turn, never both and never neither (count `delivered_at IS NOT NULL` rows against delivery messages in history: equal).

## 6. A detached sub-agent call, grouped (dummyplug, two agents)

With a second dummyplug agent `dq`, send to `dp`:
```json
{"tool": "execute_python", "arguments": {"intent": "ask two agents at once", "code": "for p in ['one', 'two']:\n    parallel_subagents(tasks=[{'agent': 'dq', 'prompt': p}], detached={'group': 'pair'})\n'started'"}}
```
Expected: two rows with the same group; no delivery until both are final; then **one** delivery message with both calls under `pair`. Each `dq` conversation exists under its own `Subagent` session. The call's hooks failing to publish to the ended turn produce no error in the log above `debug`.

## 7. Inspect and cancel (dummyplug)

1. Detach `shell_exec(command='sleep 60', detached=True)`; send `{"tool": "get_detached_call", "arguments": {"id": "<id>"}}`.
   Expected: tool, state `running`, elapsed, limit, no `outcome`.
2. Open the Background work panel: the call shows with a trash icon; expand it for the same details.
3. Press the trash icon (and, separately, send `{"tool": "cancel_detached_call", …}` for a second call).
   Expected: the call reads `cancelled`, the sleeping process is gone, no delivery turn and no notification appear, and a repeat cancel answers `already_finished`.
4. Detach a call that itself detaches another (nesting) and cancel the outer: both read `cancelled`.

## 8. Refusals (dummyplug)

- `ask_question(..., detached=True)` raises `cannot detach ask_question: it needs this turn`.
- A script that detaches four levels deep raises `nesting limit (3) reached`.
- In a scheduled-task run, `shell_exec(command='true', detached=True)` raises `cannot detach here: …`.

## 9. Timeout (dummyplug)

`shell_exec(command='sleep 30', detached={'timeout': 2})`.
Expected: after about 2 s the call reads `timed_out`, the process is gone, and the delivery says so.

## 10. Restart (dummyplug, the constitution's runtime gate)

1. Detach `sleep 60`, then kill the process hard.
2. `just run` again.
   Expected: the call reads `interrupted`; the agent is woken once with a delivery that says the server restarted; no row is left `running`; the `owner_fence` token for `agent:dp` went up by one.

## 11. Secrets (dummyplug)

`set_secret` a value, then detach `get_secret(name=…, detached=True)`.
Expected: the stored `outcome`, the delivery message, history and logs show `[redacted]` where the value would be.

## 12. Gone (static checks)

```bash
git grep -nE "background_subagents|list_background_jobs|cancel_background_job|BackgroundReport|background_job" -- crates webui/app docs CLAUDE.md
```

Expected: only the `RETIRED_TOOL_NAMES` entries, the `DROP TABLE` migration lines and the #82 checklist entry (SC-007).

## 13. Live (needs a real model)

**Live**: ask an agent on a real provider to "check these three pages and run the tests in the background, and tell me while it works". Expected: it detaches without being told the syntax (the guidance is enough), keeps talking, and reacts to the deliveries in its next rounds. This is the only step that tests the wording of the guidance.

## 14. WebUI

With the build from `just build`: the status strip shows Background work with a live dot and a count; the panel matches frames "058 · A" and "058 · B" of `Vizier shell.figcraft` in both themes; entries collapse and expand; the panel closes on ×, Escape and an outside press (Dismissal convention); a second tab sees the same state without reloading.
