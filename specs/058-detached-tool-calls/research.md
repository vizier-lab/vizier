# Research: Detached scripts

Every decision below was checked against the code as of branch `058-detached-tool-calls` (master `d1409a34`). No NEEDS CLARIFICATION remains. This revision follows the decision that **only `execute_python` carries the flag**: a per-call keyword and named groups were designed first and dropped.

## D1. Where the `detached` option lives

**Decision**: two new fields on `ExecutePythonInput` (`agents/tools/python/mod.rs`): `detached: bool` (default false) and `timeout_secs: Option<u64>` (1..=3600, default 600, used only when `detached` is true; setting it without `detached` is an error). `ExecutePython::call` keeps its inline path unchanged and, when `detached`, hands the same script to `DetachedCalls::launch` and returns a handle.

**Rationale**: every tool is called from `execute_python`, so one flag there detaches any tool call and needs no change to the sandbox, the bridge's argument handling, or any tool's schema. It also removes two problems of the per-call design: a tool that declares its own `detached` argument, and group semantics.

**Alternatives**: a reserved keyword on every tool function (needs argument interception, a collision test, a `detachable()` trait method and groups to get one combined result); a separate `detach_python` tool (a second copy of `execute_python`'s schema and description).

## D2. What a detached script may and may not call

**Decision**: a detached script runs in a context with `interactive: false`, `client: None` and a new `ToolContext.detached: true`. `VizierTool::available(ctx)` (already the mechanism that takes the question tools out of the catalogue when `interactive` is false) therefore removes `ask_question`, `ask_sensitive_input` and every client tool from the script's catalogue and from `list_tools()`; `turn_catalogue` additionally drops entries with `ends_turn` when `ctx.detached`. A script calling a missing function gets the usual `NameError`, with the retired-style hint from the catalogue's "did you mean". No new trait method.

**Refused at launch**: a session that is a scheduled task or a dream (unattended: no one to deliver to; `VizierSession::is_scheduled_task()` and the dream-session check), and a launch at `MAX_BACKGROUND_DEPTH` (3, kept as the one depth constant, the field `ToolContext.background_depth`). A woken delivery turn *may* detach again.

**Rationale**: reuses the existing `available`/`interactive` seam (constitution II) instead of adding a flag per tool.

## D3. How a detached script runs

**Decision**: `ExecutePython::call` is split: `run_script(code, limits, bridge) -> (ExecutionReport, attachments)` holds everything from "build the bridge" to "redact the report" and is called inline (as today) or from the detached task. `DetachedCalls::launch` records the row and `tokio::spawn`s a task that builds its own `RouterBridge` (context: a clone of the launching `ToolContext` with `background_depth + 1`, `interactive: false`, `client: None`, `detached: true`, fresh `pending_attachments`), and runs `run_script` with `SandboxLimits { timeout: the call's limit, .. }`. The limit therefore covers the whole script, tool waits included, as the spec says.

**Rationale**: the existing bridge already gives the same permissions, hooks, per-tool timeout, redaction and invocation records (FR-009), and `RouterBridge` needs no change except deleting the `tool == "parallel_subagents"` name check: cancelling a detached script calls `SandboxBridge::cancel` (which the bridge already implements to stop an in-flight nested call) and aborts the task, and `parallel_subagents` aborts its own pieces when dropped.

**Hooks after the turn ends**: the session hooks publish thinking and tool frames to the turn's channel; once the turn is gone those sends fail quietly. The task ignores send failures. What a person sees is the Background work panel and the delivery. Verified in the quickstart (step 6).

**Tracing**: the task is instrumented with a `detached_script` span created inside the launching span, so it is a child of the turn's trace (FR-009); `vizier.detached.id`, `.timeout_secs`, `.depth` are span attributes.

**Attachments**: files a detached script produces cannot ride a turn that has ended. They are saved as session files with the same path the agent loop uses for tool attachments (`send_file_resolve` → `send_file_upload` → `save_session_file`) and named in the delivery.

## D4. Records and ownership (spec 057)

**Decision**: one `detached_call` table behind a new `DetachedCallStorage` supertrait member, replacing `background_job` and `background_piece`. Every write that advances a script (`open`, `close`, `cancel`, `claim`) goes through `check_fence` in its own transaction, carries `owner_node`/`owner_token`, scope `agent:{id}`; `fence_tests.rs` gets one row per method. `recover_agent` marks the claimed agent's `running` rows `interrupted` and re-sends the delivery request for sessions with undelivered final rows.

**Dropped, not migrated**: PostgreSQL migration `0008_detached_calls.sql` creates the table and drops the two old ones; SQLite's `init_*_schema` creates it and runs `DROP TABLE IF EXISTS` on the old two. Both are migration code and get entries on #82.

**Stored text**: the row keeps the script's `intent` (the person-facing sentence, 500 characters), its `code` (so the panel can show it the way the trail does), the tool calls made so far (`ToolInvocationRecord`: name, argument preview, ok, duration), and on finish a bounded (32 KiB) copy of the outcome (the `ExecutionReport`). All of it passes the turn's `SecretRedactor` before it is written.

## D5. Delivery: one atomic claim, two paths

**Decision**: a final script is **delivered** by a guarded SQL claim: set `delivered_at` on the session's rows whose state is final and not cancelled and whose `delivered_at` is null, in one fenced transaction, returning them. Two callers, and the claim decides who wins:

1. **Round boundary** (FR-004). In `prompt_with` (`agents/agent/mod.rs`), after a round's tool results are collected and before the next completion, the loop claims for its session. With rows it adds one user message from `render_delivery`, after the round's results (the pattern the loop already uses for loaded images). Nothing is injected into a running script.
2. **Idle wake-up** (FR-005). When a script becomes final the task sends the agent a `VizierRequestContent::DetachedDelivery` request on the normal request channel (queued behind a running turn, non-durable). The handler in `agents/process.rs` claims; **no rows means no turn**; rows run a turn whose message is the rendered delivery. This replaces `BackgroundReport` and reuses the woken-turn republish (`channels/woken.rs`, the `SessionFrame` broadcast), so every channel keeps working with no per-channel code.

**Rationale**: the same claim in both paths gives exactly-once (FR-013) and the "turn closes just as a script ends" edge case for free: a result the loop did not take is taken by the queued request. Several scripts that finish together arrive together because one claim returns all rows.

**Restart and fleet**: `recover_agent` re-sends the request for each session with an undelivered final row.

## D6. Cancel, inspect, list

**Decision**: three tools in the existing `agents` category, in `agents/tools/detached.rs`: `list_detached_calls`, `get_detached_call`, `cancel_detached_call`; not in `DREAM_TOOL_NAMES`. They replace `list_background_jobs` and `cancel_background_job`. Cancel is a guarded transition `running → cancelled`; the in-memory cancel signal (a `watch` per live task, as `BackgroundJobs.cancels` was) only stops the task, so losing the map in a restart cannot break exactly-once. Cancel cascades to rows whose `parent_id` is the cancelled script. `get_detached_call` includes the tool calls made so far, which the task keeps current by writing the list to the row when a call finishes (batched to at most one write per second).

**Rationale**: reusing the category avoids a 19th built-in category (which would touch `SANDBOX.md`, the reserved-name list and the table pinned by `every_tool_is_in_its_category`).

**Parent id**: a detached script that calls `parallel_subagents` causes sub-agent turns on other sessions; if one of those turns runs `execute_python` detached, its `parent_id` is the launching script (passed on the request as the existing `background_depth` and a new optional `parent_detached_id` on `VizierRequest`).

## D7. Names the model and the people see

**Decision**: tools and API say "detached"; the WebUI says **Background work**. The three retired tools join `RETIRED_TOOL_NAMES` with a hint to run `execute_python` with `detached=true`. Guidance (in `boot.rs` and `execute_python`'s description) says when to detach, that results arrive later, and what is unavailable inside.

## D8. HTTP and frames

**Decision**: `/agents/{id}/channel/{channel}/topic/{topic}/detached-calls` (list), `/{call_id}` (get), `/{call_id}/cancel`. The `pieces/{ordinal}/history` route goes away. `SessionFrame::Job` becomes `SessionFrame::DetachedCall` carrying a snapshot; the WebUI frame `background_job` becomes `detached_call`. OpenAPI is regenerated (`webui/scripts/openapi.mjs`, `schema.d.ts`).

## D9. Notifications

**Decision**: a final, non-cancelled script notifies once, category `background_work`, wording in `notifications/text.rs`. Old `background_job` rows age out under the 30-day retention; the WebUI must tolerate them. To be confirmed in the first WebUI task; fallback is a one-line skip of the old category at read time.

## D10. WebUI and design system

**Decision**: change `design/` first (branch `058-detached-tool-calls`): `BackgroundJob` becomes a collapsible `BackgroundWork` entry (chevron, trash, Inspect details), `StatusStripItemId` `jobs` becomes `work`. Then `webui/`, then vizier's pins (design → webui → vizier). Entry shape: header is the script's **intent** (not a tool name), then handle, elapsed time, a trash icon (running) and a chevron; expanded, it is the trail's own `RunDetail` (code with line numbers and Copy, "Calls inside" as `ScriptCall` rows, output, result or traceback), reused as is, so a script reads the same in the trail and in the panel. A still-running script shows Code and the calls finished so far (`ScriptCall` has no in-flight state, and none is added). The FigCraft drawing (frames "058 · A" and "058 · B") shows this shape (constitution: design before code).

**Result rendering**: a detached `execute_python` call returns a handle, not an `ExecutionReport`; the trail and message renderers that read `ok`, `result`, `stdout` from an `execute_python` result must show the handle as "running in the background" (`lib/trail.ts`, `MessageList.tsx`), and `ExecutionReport::ending_call` must ignore it (it has no `tool_calls`).

## D11. What is removed

`agents/background/` (`mod.rs`, `report.rs`, `wait.rs`), `agents/tools/background_jobs.rs`, the `background_subagents` half of `agents/tools/subagents.rs`, `schema/background.rs`, `storage/background_job.rs` + `storage/sqlite/background_job.rs`, `service/jobs.rs`, `vizier-http/src/api/v1/agents/jobs.rs`, the `BackgroundReport` request kind and its arms (`process.rs`, `tools/mod.rs`, `recovery*.rs`), the job frames, and the WebUI job tray code (`ChatStatusStrip`, `statusFilters`, `frames`, `trail`, `MessageList`). `MAX_BACKGROUND_DEPTH`, the timeout limits and `Canceller` move to the new module. CLAUDE.md's "Background jobs" paragraphs are rewritten, `docs/src/configuration/agents.md` updated, `public-api.txt` regenerated.

## D12. Verification

Per the constitution: a dummyplug end-to-end script (`quickstart.md`), unit tests beside the bridge and the claim, fence tests for the new writes, `recovery_tests.rs` rows, conformance vectors for the new routes (`tests/conformance/pending.txt` may only shrink), WebUI vitest for the frame reducer and the panel, `npm run typecheck`, `cargo clippy --workspace`, `cargo test --workspace` and the per-flag runs the CI matrix does. Dummyplug needs a slow, deterministic tool: `shell_exec "sleep N"` inside a script is enough; no protocol change is planned.
