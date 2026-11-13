# Research: Detached tool calls

Every decision below was checked against the code as of branch `058-detached-tool-calls` (master `d1409a34`). No NEEDS CLARIFICATION remains.

## D1. Where the `detached` option lives

**Decision**: a reserved keyword argument, `detached`, accepted by every tool function a script calls: `detached=True`, or `detached={"timeout": 900, "group": "vendors"}`. `RouterBridge::call` (`agents/tools/python/bridge.rs`) removes it from the arguments before dispatch.

**Rationale**: the sandbox already hands keyword arguments to the bridge verbatim as one JSON object (`sandbox/runtime.rs`, "keyword arguments verbatim, positional ... mapped onto required parameters"), so the option needs no sandbox change, works for built-in, MCP, addon and project tools alike, and appears in the same place for the model as every other argument. `execute_python` itself gains no parameter: a whole-script option would be a second mechanism for the same thing (spec assumption 1 is narrowed accordingly: per-call only).

**Collision rule**: a tool whose input schema already has a property named `detached` would be shadowed. A test over the full catalogue (`every_tool_is_in_its_category` sits beside it) fails if any tool declares `detached`; an MCP tool that does is called inline and the option is refused for it with a clear message.

**Alternatives**: a `detach(fn, **kw)` helper function (a second calling convention, and positional mapping gets awkward); a flag on `execute_python` (detaches the whole script, so the script's own later logic cannot use the handle).

## D2. What may be detached

**Decision**: a `detachable()` method on the tool trait, default `true`, surfaced as `CatalogueEntry.detachable`. It is `false` for `ask_question` and `ask_sensitive_input`, for any entry with `ends_turn`, and for client tools (`ToolTarget::Client`). Detaching is also refused when `ToolContext.interactive` is false for a reason other than the request being a woken delivery, and in dream and scheduled-task contexts (unattended).

**Rationale**: constitution II forbids a `match` over tool names. The refusals in spec FR-007 are exactly the tools that need the live turn; each states it itself. The old `cancellable = tool == "parallel_subagents"` name check in the bridge is deleted: cancelling a detached call drops its future, and `parallel_subagents` already aborts its own pieces when dropped.

## D3. How a detached call runs

**Decision**: a `DetachedCalls` service (replaces `BackgroundJobs`, held on `VizierDependencies`). `launch` records the row, spawns one tokio task, and returns the handle. The task runs the **same** invocation the inline path uses; that body (hooks, `ToolRouter::call`, per-tool timeout, redaction, attachments) is lifted out of `RouterBridge::dispatch_to` into a shared function so the two paths cannot drift. The task's context is a clone of the launching `ToolContext` with `background_depth + 1`, `deadline = now + limit`, `client: None`.

**Rationale**: FR-006 (same permissions, hooks, timeout, redaction, telemetry). One shared function keeps it one code path (constitution II) and the existing bridge test (`a_value_passed_to_a_nested_call_is_scrubbed_from_every_copy`) covers both.

**Hooks after the turn ends**: the session hooks publish thinking/tool frames to the turn's channel; once the turn is gone those sends fail quietly. The detached task runs with the hooks it was launched with and ignores send failures; what a person sees is the Background work panel and the delivery (D5), not live frames. Verified in the quickstart (step 6).

**Tracing**: the task is instrumented with a `detached_call {tool}` span created inside the launching span, so it is a child of the turn's trace (FR-009), exactly as `background_job` spans were.

**Sub-agents**: `parallel_subagents` is the tool that reaches other agents without a Workspace (`consult_agent` and `delegate_agent` are retired). Detached, it blocks inside its own task, not the turn. Its `deadline` is the call's limit, not the turn's tool timeout, so the limit may be up to 3600 s.

## D4. Records and ownership (spec 057)

**Decision**: one `detached_call` table behind a new `DetachedCallStorage` supertrait member, replacing `background_job` and `background_piece`. Every write that advances a call (`open`, `close`, `cancel`, `claim`) goes through `check_fence` in its own transaction and carries `owner_node`/`owner_token`, scope `agent:{id}`; `fence_tests.rs` gets one row per method. `recover_agent` marks the claimed agent's `running` calls `interrupted` (unless the process still holds them in memory, which a fresh claim never does) and re-sends the delivery poke (D5) for sessions with undelivered final calls.

**Rationale**: CLAUDE.md "Row ownership": a new write path for in-flight work must be fenced. The old tables are **dropped**, not migrated (the spec says no compatibility): a PostgreSQL migration `0008_detached_calls.sql` creates the new table and drops the old two; SQLite's `init_*_schema` creates the new table and runs `DROP TABLE IF EXISTS` on the old two. Both drops are migration code and get entries on #82.

**Outcome storage**: the row keeps a bounded (32 KiB), already-redacted copy of the outcome until the conversation is deleted. The spec said a detached call "holds no result text"; that was wrong for this design, because delivery and Inspect both need the outcome after the task has ended and a tool result (unlike a piece's answer) has no session to be read from. The spec is amended.

## D5. Delivery: one atomic claim, two paths

**Decision**: a final call is **delivered** by a guarded SQL claim: `UPDATE detached_call SET delivered_at = now WHERE session = ? AND state IN (final states, not cancelled) AND delivered_at IS NULL AND (group_id IS NULL OR no member of the group is still running)` returning the claimed rows, fenced. Two callers use it and the claim decides who wins:

1. **Round boundary** (spec FR-004). In `prompt_with` (`agents/agent/mod.rs`), after a round's tool results are collected and before the next completion, the loop claims for its session. If it gets rows it adds one user message, rendered by `render_delivery`, after the round's results (the same pattern the loop already uses for loaded images). Nothing is injected into a running script.
2. **Idle wake-up** (FR-005). When a call becomes final the runner sends the agent a `VizierRequestContent::DetachedDelivery` request on the normal request channel (queued behind a running turn like any request, non-durable). The handler in `agents/process.rs` claims; **no rows means no turn** (the running turn already took them, or the group is not complete); otherwise it runs a turn whose message is the rendered delivery. This replaces `BackgroundReport` and reuses the woken-turn republish (`channels/woken.rs`, the `SessionFrame` broadcast), so every channel keeps working with no per-channel code.

**Rationale**: the same claim in both paths gives FR-014 (exactly once) and the "turn closes just as a call finishes" edge case for free: a result the loop did not take is taken by the queued poke. There is no extra round and no timing window to manage, which is the simplification the review asked for.

**Restart and fleet**: `recover_agent` re-sends the poke for each session that has an undelivered final call, so a restart loses nothing but in-flight work (those rows are `interrupted` and deliverable).

## D6. Groups

**Decision**: `group` is scoped to one script run, not a whole turn: the stored group id is `{run uuid}/{label}`, the run uuid being generated per `RouterBridge`. A claim skips a group that still has a running member.

**Rationale**: `ToolContext` carries no turn id, and a script run is the unit that spawns the group in practice (`background_subagents` was one call). Spec FR-011 and the Key Entities line are amended to say "one script run".

## D7. Cancel, inspect, list

**Decision**: three tools in the existing `agents` category, in a new `agents/tools/detached.rs`: `list_detached_calls`, `get_detached_call`, `cancel_detached_call`. Not in `DREAM_TOOL_NAMES`. They replace `list_background_jobs` / `cancel_background_job`. `get_detached_call` returns the tool, an arguments summary (redacted, 500 characters), state, start, elapsed, limit, group, parent and, once final, the outcome. Cancel is a guarded transition `running → cancelled`; the in-memory cancel signal (one `watch` per live task, as `BackgroundJobs.cancels` was) only drops the task's future, so losing the map in a restart cannot break exactly-once. Cancel cascades to rows whose `parent_id` is the cancelled call and never delivers.

**Rationale**: reusing the category avoids a 19th built-in category (which would touch `SANDBOX.md`, the reserved-name list and the category table pinned by `every_tool_is_in_its_category`).

## D8. Names the model and the people see

**Decision**: tools and API say "detached call"; the WebUI says **Background work** (decided, spec Assumptions). The three retired tools join `RETIRED_TOOL_NAMES` with a hint that says to pass `detached=True`; that is guidance to the model, not compatibility.

## D9. HTTP and frames

**Decision**: `/agents/{id}/channel/{channel}/topic/{topic}/detached-calls` (list), `/{call_id}` (get), `/{call_id}/cancel`. The `pieces/{ordinal}/history` route goes away (a call has no session; a sub-agent's own conversation is reached from its outcome). `SessionFrame::Job` becomes `SessionFrame::DetachedCall` carrying a `DetachedCallSnapshot`; the WebUI frame `background_job` becomes `detached_call`. OpenAPI is regenerated (`webui/scripts/openapi.mjs`, `schema.d.ts`).

## D10. Notifications

**Decision**: a final, non-cancelled call notifies once, category `background_work`, wording in `notifications/text.rs`. Old `background_job` rows age out under the 30-day retention; the WebUI renders an unknown category generically. To be confirmed in the first WebUI task (T-list), and a fallback is a one-line skip for the old category. Discord/Telegram activity still creates no notification.

## D11. WebUI and design system

**Decision**: change `design/` first (branch `058-detached-tool-calls`): `BackgroundJob` becomes a collapsible `BackgroundWork` entry (chevron, trash, Inspect details), `StatusStripItemId` `jobs` becomes `work`. Then `webui/`, then vizier's pins (design → webui → vizier). The drawing is `Vizier shell.figcraft`, Shell · chosen, frames "058 · A" and "058 · B"; the build matches it.

## D12. What is removed

`agents/background/` (`mod.rs`, `report.rs`, `wait.rs`), `agents/tools/background_jobs.rs`, the `background_subagents` half of `agents/tools/subagents.rs`, `schema/background.rs`, `storage/background_job.rs` + `storage/sqlite/background_job.rs`, `service/jobs.rs`, `vizier-http/src/api/v1/agents/jobs.rs`, the `BackgroundReport` request kind and its arms (`process.rs`, `tools/mod.rs`, `recovery*.rs`), the job frames, and the WebUI job tray code (`ChatStatusStrip`, `statusFilters`, `frames`, `trail`, `MessageList`). `MAX_BACKGROUND_DEPTH`, `validate_timeout` and the depth/limit constants move to the new module. CLAUDE.md's "Background jobs" paragraphs are rewritten, `docs/src/configuration/agents.md` updated, `public-api.txt` regenerated.

## D13. Verification

Per the constitution: a dummyplug end-to-end script (`quickstart.md`), unit tests beside the bridge, fence tests for the new writes, `recovery_tests.rs` rows, `service` and conformance vectors for the new routes (`tests/conformance/pending.txt` may only shrink), WebUI vitest for the frame reducer and the panel, `npm run typecheck`, `cargo clippy --workspace`, `cargo test --workspace` and the per-flag runs the CI matrix does. Dummyplug needs a way to make a tool slow and deterministic: its `say` / sample-request protocol is enough with a `shell_exec "sleep N"` or the `time` tools; no protocol change is planned.
