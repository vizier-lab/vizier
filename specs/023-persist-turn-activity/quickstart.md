# Quickstart: Durable Turns

All checks run offline against an agent `plug` on the `dummyplug` provider (constitution e2e gate),
with the **local shell enabled** in its tools config. Dummyplug runs one tool per turn, so a
turn is held open with a script that sleeps:

```text
HOLD(n) = {"tool":"execute_python","arguments":{"intent":"hold","code":"shell_exec(command=\"sleep n\")"}}
```

(Check `shell_exec`'s argument name with `describe_tool_function` when writing the script.)

The checks are scripted in `scripts/durable-turns-e2e.mjs`, in the same style as
`scripts/client-tools-e2e.mjs` (Node ≥ 22, no dependencies). The script restarts vizier itself
through `VIZIER_START`/`VIZIER_STOP` commands, defaulting to `just run-d` / `just shutdown`.
A crash is `kill -9` of the vizier pid.

```sh
VIZIER_URL=ws://localhost:9999 VIZIER_TOKEN=<jwt> AGENT=plug node scripts/durable-turns-e2e.mjs
```

The reply checks below rely on dummyplug §4: after one tool call, the turn's reply is
`**Tool result** (\`<tool>\`):\n<output>`.

## §1 Steps reach history while the turn runs (US1, FR-001/002)

1. Socket **A**: send `HOLD(8)`. Wait for `thinking_start`.
2. Within the sleep, `GET …/history`. **Expect**: the `Request`, then a `ToolCall`
   (`execute_python`) entry, with no `ToolResult` yet.
3. **Expect**: A received a `history_entry` frame for that `ToolCall` with the same `uid`.
4. After the reply, the history entries for the turn are, in order: `Request`, `ToolCall`,
   `ToolResult`, `Response`. This is the same sequence a turn produced before this feature
   (compare with a turn recorded on `master`).

## §2 Reopening mid-turn: exactly once (US1, FR-004/005)

1. Socket A: send `HOLD(8)`. After A receives the `ToolCall` `history_entry`, open socket **B**,
   then read history.
2. **Expect**: the union of B's history read and B's `history_entry` frames, merged by `uid`,
   equals A's entries for the turn, and every `uid` appears once.

## §3 Resume after a crash (US2, FR-008/009/011, SC-002/003)

1. Socket A: send `HOLD(30)`. After the `ToolCall` entry is in history, `kill -9` vizier.
2. Check `session_turn`: one `running` row for the session, with `request_uid` set.
3. Start vizier and open socket A again.
4. **Expect**, within 10 s:
   - a `history_entry` `ToolResult` for that call whose content is the restart notice
     (contracts §3)
   - then a reply frame with `"resumed": true` whose text is
     `**Tool result** (\`execute_python\`):` followed by the notice
5. **Expect**: no `sleep` process was started again (the call was not re-run). The turn's
   history reads `Request`, `ToolCall`, `ToolResult` (notice), `Response` (resumed). The
   `session_turn` table is empty.

## §4 Resume after a clean shutdown (FR-018)

Repeat §3 with `just shutdown` instead of `kill -9`. Same expectations.

## §5 Queue survives (US3, FR-010, SC-004)

1. Socket A: send `HOLD(30)`, then `first`, then `second`. Wait until history shows the
   `ToolCall`.
2. `kill -9`. Start vizier, open A.
3. **Expect**: the resumed reply, then two lorem-ipsum replies, with history `Request`s in the
   order `HOLD`, `first`, `second`. `session_turn` is empty at the end.

## §6 Abort after restart (FR-017)

1. As §5 step 1. `kill -9`, start vizier, and immediately send `{"command":"abort"}`.
2. **Expect**: an `abort` frame, no lorem replies for `first`/`second`, and `session_turn`
   empty.

## §6b Abort mid-call (FR-021/022/023, SC-008)

1. Socket A: send `HOLD(30)`. After A receives the `ToolCall` `history_entry`, send
   `{"command":"abort"}`.
2. **Expect**:
   - A receives an `abort` frame and a `history_entry` `ToolResult` for that call whose content
     is the "aborted" notice (contracts §3).
   - History reads `Request`, `ToolCall`, `ToolResult` (aborted notice), `Command(abort)`.
   - No `sleep` is still running, and `session_turn` is empty.
3. Send `hello`. **Expect**: a normal lorem-ipsum reply. Every `ToolCall` in the session's
   history has a `ToolResult`.
4. Repeat steps 1–2, but instead of aborting, change the agent's settings (`PUT /agents/plug`)
   mid-call. **Expect**: the "agent was restarted" notice, then the same clean follow-up.

## §7 Crash-loop guard (FR-016, SC-007)

Dummyplug can't produce a turn that crashes the process, and every resume records a sealing
step, which counts as progress. So the guard is checked by unit tests, not end to end:
- `storage/sqlite/turn.rs`: `note_resume` returns 1 after progress and counts up when the step
  count is unchanged.
- `agents/process.rs`: a resumed job whose count exceeds 3 ends with an `Error` response, its
  row is deleted, and the next queued request starts.

## §8 Background job resumes and reports once (US4, FR-012, SC-005)

1. Socket A: run `paralel_subtasks` through `execute_python` with two pieces: `"hello"` (answers
   at once) and `HOLD(30)`.
2. After the first piece answers and the second has its `ToolCall` in history, `kill -9`, then
   start vizier.
3. **Expect**: the job leaves `running` exactly once, and one `BackgroundReport` request
   appears in A's history containing both answers (the second is the restart-notice echo). A
   receives the woken reply. The first piece's session has exactly one `Response` (it was not
   run again).

## §9 Task run resumes, and is not fired twice (US4/US5, FR-013/014)

1. Create a one-time task for `plug` due in 5 s, whose prompt is `HOLD(30)`.
2. Once its run is `running` and the `ToolCall` is in its session's history, `kill -9`, then
   start vizier.
3. **Expect**: the run ends `answered`, its response is the restart-notice echo, and the task
   has exactly one run.

## §10 Missed schedule runs once (US5, FR-014)

1. Create a cron task every minute. Stop vizier for 3 minutes, then start it.
2. **Expect**: exactly one new run within the first tick, not three.

## §11 WebUI by hand (US1/US2)

These steps use a live provider, marked as such, for a real multi-step trail.

1. Mid-turn: leave to Settings and come back, reload, and open a second tab. **Expect**: every
   step, in order, once, and still updating.
2. After the reply: the folded trail is identical in all three views and after another reload.
3. Kill vizier mid-turn and restart it. **Expect**: the page shows the steps before and after
   the restart as one list. The reply carries the "continued after a restart" label.
4. Same on a Discord or Telegram agent: the resumed reply is posted in the channel with the
   restart line.

## Gates

- `cargo clippy`. `cargo test`, which adds tests for: `TurnStorage` lifecycle and
  `note_resume`, the history split + seal for resumption, `is_durable`, and per-step entries
  being equal to `messages_to_history_entries(delta)` for a recorded turn.
- `cd webui && npm run typecheck`, plus `trail.test.ts` for merging by uid and the open turn's
  trail.
