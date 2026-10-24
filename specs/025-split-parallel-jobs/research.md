# Research: Blocking and Background Parallel Subagents

**Feature**: `025-split-parallel-jobs` · **Date**: 2026-10-24

The spec left one open question (the names), settled as `parallel_subagents` / `background_subagents`. Everything below comes from reading the current code: `src/agents/background/mod.rs`, `src/agents/tools/subtasks/mod.rs`, `src/agents/tools/python/bridge.rs`, `src/agents/process.rs` (instance stop and hand-on), `src/schema/request.rs` (`is_durable`) and the agent loop's tool-call site in `src/agents/agent/mod.rs`.

## R1. Where the blocking runner lives

**Decision**: In `src/agents/background/`, as `BackgroundJobs::run_and_wait(ctx, pieces, limit)` in a new `background/wait.rs`. It shares two primitives with `run_job`, which are pulled out of `run_job`/`launch` first:

- `dispatch_piece(session, prompt, origin, depth, transient) -> Result<flume::Receiver<VizierResponse>>`: the `send_request` of a `Prompt` that `launch` does today.
- `await_piece(rx, limit, session, origin) -> (PieceState, String)`: `first_terminal` under `tokio::time::timeout`, sending the existing `abort` command to a piece that runs out of time.

`run_job` keeps its storage writes and publishes around `await_piece`; `run_and_wait` calls the same two functions and nothing else.

**Rationale**: Constitution II: a piece is dispatched, awaited, timed out and aborted in exactly one place. `BackgroundJobs` already owns the transport handle and the abort helper. The blocking tool needs none of the job bookkeeping (rows, cancel map, publishing, report delivery), so it is a second caller of the primitives, not a flag on `launch`.

**Alternatives considered**:
- A `blocking: bool` on `launch` that awaits the runner. Rejected: it would still write a job row and publish it to the tray (against FR-007), and every branch of `run_job` would have to ask "is anyone waiting?".
- A separate `src/agents/subagents/` module that both tools use. Rejected for now: it moves code that works without changing it, for no second caller beyond these two. If it grows, the split is mechanical.

## R2. A blocking run is not a job

**Decision**: No `background_job`/`background_piece` rows, no entry in the cancel map, nothing published on the session-event broadcast, no `BackgroundReport`. Piece sessions are still `VizierChannelId::Subagent` with a fresh topic each, exactly as batch pieces, so their history is kept and readable (FR-009). The only handle on them is the `topic` in each result entry (R7).

**Rationale**: FR-007. The person sees the run as one tool call in the turn's trail, which already shows its arguments and result. A tray entry would describe work the turn is visibly waiting on, and could be cancelled from under a running script.

**Alternatives considered**: A job row with a new `JobKind::Parallel` and reporting suppressed. Rejected: a new kind every reader of jobs must learn (WebUI tray, `list_background_jobs`, startup sweep, `recover`), for state that lives for a single tool call.

## R3. The time limit: per-task limit and the tool deadline

**Decision**: Add `ToolContext.deadline: Option<tokio::time::Instant>`. The agent loop sets it on the per-call context clone, right before `traced_tool_call`, to `now + config.tools.timeout`. `execute_python` passes its context to `RouterBridge`, so every call a script makes sees the deadline of the script's own call. `run_and_wait` waits at most `min(timeout_secs, deadline - now - GRACE)`, with `GRACE = 2s`. If less than `GRACE + 1s` is left, the call is refused with `not enough time left in this tool call (Ns); call it earlier in the script or from a new execute_python`. Pieces still running when the wait ends are aborted and marked `timed_out` with `No answer within Ns`, where N is the limit that applied. The finished results are returned (FR-006).

**Rationale**: The whole `execute_python` call is bounded by `tools.timeout` (`traced_tool_call` drops it at the limit), and each nested call is also bounded by it in `RouterBridge::dispatch_to`. A script that spends 25 of 30 minutes before calling the blocking tool has 5 left, not 30. Only a deadline fixed at the start of the outer call knows that. The grace period leaves the script time to receive and use the result before the outer drop. The default `tools.timeout` is 30m and the default per-task limit is 600s, so the deadline only applies to long scripts or agents with a short tool timeout.

**Alternatives considered**:
- Pass `tools.timeout` to the tool when it is built. Rejected: it ignores the time the script has already used, so the outer timeout would still cut the call off and lose every result.
- Let the outer timeout drop the call (the plain tool-timeout error). Rejected by FR-006: finished answers would be thrown away.

## R4. Stopping with the turn, including restarts and crashes

**Decision**: Two pieces, both small:

1. **Drop guard.** `run_and_wait` holds a guard listing the sessions of unfinished pieces. Dropping it (turn aborted, tool future dropped by a timeout, instance stopping) spawns the existing `abort` command for each one. A normal return disarms the guard.
2. **Transient piece requests.** Add `VizierRequest.transient: bool` (`#[serde(skip)]`, default `false`). `is_durable` returns `false` for it. Blocking pieces are sent `transient: true`, so they never get a `session_turn` row.

**Rationale**: The parent turn is durable. On an update or restart, `process.rs` seals its open calls and hands it to the next instance with `resumed: true`, so the blocking call is cut and the agent sees a `Stopped:`/`Interrupted:` notice (FR-008, the spec's edge case). Its pieces run on the same agent. If they were durable, they would be handed on too and run to completion with nobody waiting, and after a crash `resend_pending_turns` would re-send them. A transient piece has no row (`turn.id` is `None`), so `stop_all` ends it with `notify_turn_stopped` and startup finds nothing to re-send. The guard covers the cases where the agent keeps running: `/abort`, the tool deadline, and the turn's `prompt_timeout`. Step recording does not depend on durability (`StepRecorder` takes only the session), so piece history is still saved as it happens.

**Alternatives considered**:
- The guard alone. Rejected: after a crash, the pieces' journal rows re-send them at startup.
- Excluding all `Subagent` sessions from the journal. Rejected: it would break spec 023 recovery for background batch pieces, which `BackgroundJobs::recover` relies on.

## R5. Nesting

**Decision**: Move the depth check from `launch` into `fn check_depth(ctx) -> Result<()>`, used by both `launch` and `run_and_wait`. Blocking pieces are sent `background_depth: ctx.background_depth + 1`, like batch pieces. The refusal message becomes `nesting limit (3) reached: this turn was itself started by subagent or background work`.

**Rationale**: FR-011. Without it, blocking calls could nest without limit: each level waits on the next, and every waiting level holds an agent turn.

**Alternatives considered**: A separate limit for blocking nesting. Rejected: one rule is easier to state in guidance, and the cost (turns held open) is the same kind.

## R6. Cancelling nested work

**Decision**: When a blocking run aborts a piece (deadline, per-task limit, drop), the background jobs that piece launched are **not** cancelled. This matches spec 012, where only `cancel` cascades and a timed-out piece's own jobs are left to run.

**Rationale**: It matches existing behaviour and needs no new code. Such a job reports into the piece's own `Subagent` session (history only), so nothing reaches the person.

**Alternatives considered**: Cascading through `BackgroundJobs::cancel` for each aborted piece session. Deferred: it is worth doing for both tools at once, not for one of them in this change.

## R7. Result shape

**Decision**: The tool returns

```json
{"results": [{"task": 0, "state": "answered", "text": "…", "truncated": false, "topic": "<uuid>"}]}
```

`task` is the 0-based position in the input. `state` is a `PieceState` value: `answered`, `failed` or `timed_out`. `text` is the answer or the reason, cut with `report::truncate` at `REPORT_TEXT_LIMIT` (4000 characters), the same as a background report. `topic` is the piece's conversation. Input order is kept and there is always one entry per task.

**Rationale**: Code mode means the caller is a script, which wants a list to loop over (US1 AS4), not markdown. The truncation limit matches the background tool, so moving work between the two never changes what the agent sees of an answer. The prompt is not echoed: the script already has it, and echoing it doubles the result size.

**Alternatives considered**:
- Reusing `ReportEntry`. Rejected: it is part of the WebUI wire shape (`BackgroundJobSnapshot`), it carries the prompt, and it has no topic. A tool-local `SubagentResult` struct keeps the two contracts independent.
- Returning answers in full. Rejected: one long answer could fill the script's result and the turn's context. When `truncated` is set, the topic leads to the full answer.

## R8. The retired name

**Decision**: `paralel_subtasks` is removed from every toolset and from the category table. A small `RETIRED_TOOL_NAMES: &[(&str, &str)]` in `src/agents/tools/mod.rs` maps it to a hint: `"paralel_subtasks was replaced by parallel_subagents (waits for the results) and background_subagents (results arrive later as a background report)"`. `RouterBridge::call` checks the table when a name is not in the catalogue and returns `BridgeError::Tool(hint)` instead of `UnknownFunction`, which the script sees as a catchable `RuntimeError`. `describe_tool_function` returns the same hint in its not-found answer. Add an entry for the table to #82.

**Rationale**: FR-002 asks for the standard refusal plus suggestions wherever suggestions exist. The fuzzy `did_you_mean` (edit distance ≤ 3 or a prefix) would not connect `paralel_subtasks` to either new name, and agents' CORE.md and saved skills will keep naming the old tool. The hint is not an alias: nothing runs.

**Alternatives considered**:
- No hint. Rejected: a bare `NameError` in a script leaves the model guessing.
- A real alias to `background_subagents`. Rejected by the spec (dispatch names are the key; the `memory_detail` precedent).

## R9. The background tool

**Decision**: `SubtasksTool` becomes `BackgroundSubagentsTool` (`tools/subagents.rs`, which replaces `tools/subtasks/`), named `background_subagents`, with its description, validation and `JobKind::Batch` launch unchanged. The new `ParallelSubagentsTool` lives in the same file and shares the argument type (`SubagentsArgs`) and its validation.

**Rationale**: FR-001 and FR-010. One argument type and one validation function means the two tools can't drift apart.

## R10. Labels and guidance

**Decision**:
- `utils/mod.rs` status text: `parallel_subagents` → `⚡ Running subagents`, `background_subagents` → `⚡ Starting background subagents`.
- `list_background_jobs` description and the module docs in `background/mod.rs` and `schema/request.rs` name `background_subagents`.
- The boot prompt's "Large tasks" section names both tools and the rule from US3 AS1.
- Usage breakdown (`usage/mod.rs`): the `SUBAGENT` session sublabel changes from `Background job` to `Subagent`, since blocking pieces share that channel. The activity stays `Background`.
- Docs: `docs/src/configuration/tools-embedding.md` (tool table), `agents.md` (usage row) and `observability.md` (the span example and the `background_job` paragraph).
- Remove the unused `Subtask`/`SubtaskArgs` structs in `src/agents/agent/mod.rs` (dead, not migration code).

## R11. WebUI

**Decision** (submodule `vizier-lab/webui`, committed before vizier's pin bump):
- `lib/trail.ts`: `JOB_LAUNCHING_TOOLS = {'paralel_subtasks', 'background_subagents', 'delegate_agent'}`. The old name stays so saved history renders as before (FR-015). `parallel_subagents` is not in the set, so it renders as an ordinary tool call.
- `lib/trail.test.ts`: cases for all three names.
- `BackgroundJobsPanel.tsx` and `MessageList.tsx`: batch label `background_subagents`. It is chosen by job kind, so old batch jobs show the new name too, which is accepted.
- `api/frames.ts` comment, and `api/schema.d.ts` regenerated from the server's OpenAPI after the `request.rs` doc change.
- No `design/` change.

## R12. Telemetry

**Decision**: No new span. The blocking call already has its `execute_tool parallel_subagents` span (`ToolRouter::call`), and piece requests sent from inside it continue that trace through `VizierRequest.trace`, so each piece's `invoke_agent` is a child of the call.

## R13. Breaking change

The commit that removes `paralel_subtasks` is flagged `[**breaking**]`. Also add an entry to #82 for `RETIRED_TOOL_NAMES` (R8). The WebUI keeping `paralel_subtasks` in `JOB_LAUNCHING_TOOLS` is display of stored history, not a migration, but it is noted on the same entry so the two are reviewed together.
