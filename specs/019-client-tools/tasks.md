---

description: "Task list for 019-client-tools"
---

# Tasks: Client Tools

**Input**: Design documents from `specs/019-client-tools/`

**Prerequisites**: plan.md, spec.md, research.md (R1–R17), data-model.md, contracts/ (client-tools-protocol.md, webui-extension-api.md, agent-surface.md), quickstart.md

**Tests**: The spec does not ask for TDD. Unit tests are included where plan.md's Testing line names them (pure functions and the connection/storage invariants), written in the same file as the code under `#[cfg(test)]` (Rust) or in `trail.test.ts` (WebUI, run with `cd webui && node --test app/lib/trail.test.ts`). End-to-end checks follow the constitution's dummyplug gate (quickstart.md, `scripts/client-tools-e2e.mjs`).

**Organization**: Tasks are grouped by user story. The spec's three P1 stories are ordered by dependency rather than by spec number. **US3** (declaring and calling client tools) is the mechanism **US1** (pages) and **US2** (clicks start turns) run on, so it comes first. Then come US1, US2, and the P2 stories US5, US4 and US6.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: the user story from spec.md (US1–US6)
- Paths are from the repository root; the WebUI lives in `webui/app/`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Empty modules and files the rest of the work fills in.

- [X] T001 Create the module `src/agents/client/` with `mod.rs`, `connection.rs`, `declare.rs` (module docs only, citing `specs/019-client-tools/research.md`), and add `pub mod client;` to `src/agents/mod.rs`
- [X] T002 [P] Create `src/storage/client_call.rs` (empty `ClientCallStorage` trait stub with a module doc) and `src/storage/sqlite/client_call.rs`, and register both modules (`src/storage/mod.rs`, `src/storage/sqlite/mod.rs`)
- [X] T003 [P] Create `src/channels/http/rate_limit.rs` (empty, module doc) and register it in `src/channels/http/mod.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Types, storage, frames and connection plumbing that every story uses.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T004 [P] Implement `ClientToolDeclaration`, `ClientCategoryDeclaration` (fields and serde defaults per data-model.md) and `validate(tools, categories) -> Result<(), DeclarationError { tool: String /* "category/name" */, rule: &'static str, message: String }>` in `src/agents/client/declare.rs`, enforcing every rule and limit in research.md R3 (rule names exactly as in contracts/client-tools-protocol.md §1). Add unit tests for each rule, the default category `client`, and that a category description for an unused category is ignored.
- [X] T005 [P] Add `SessionHistoryContent::ClientToolCall { name, category, tool, arguments, outcome: Option<ClientCallOutcome>, state: Option<Value> }` (both options `#[serde(default, skip_serializing_if = "Option::is_none")]`) and `ClientCallOutcome` (`#[serde(tag = "status", rename_all = "snake_case")]`: `Pending`, `Ok { result }`, `Error { message }`, `Disconnected`, `Timeout`, `Cancelled`, `Interrupted`) in `src/schema/history.rs`. Make `history_entries_to_messages` skip the variant, with a unit test showing a turn with a `ClientToolCall` entry replays exactly as without it. Add `"ClientToolCall"` to `content_type_discriminant` in `src/storage/sqlite/history.rs`.
- [X] T006 Add to `VizierRequest` in `src/schema/request.rs`: `client_tools: Vec<ClientToolDeclaration>` and `client_categories: Vec<ClientCategoryDeclaration>` (`#[serde(default, skip_serializing)]`, `#[schemars(skip)]`, `#[schema(ignore)]`), and `client: Option<crate::agents::client::ClientBinding>` (`#[serde(skip)]`, `#[schemars(skip)]`, `#[schema(ignore)]`, as for `trace`). Define `ClientBinding { connection: Arc<ClientConnection>, tools, categories }` (deriving `Clone`, with a manual `Debug` that omits the connection) in `src/agents/client/mod.rs`. Fix every `VizierRequest { … }` literal that `cargo build` reports (e.g. `src/channels/http/api/v1/agents/mod.rs` REST chat) with empty values / `None`. Add a unit test that a serialised request omits all three fields.
- [X] T007 Define the `ClientCallStorage` trait in `src/storage/client_call.rs`: `open_client_call(uid, &VizierSession)`, `finish_client_call(uid, &ClientCallOutcome)` (only from `pending`; returns whether it applied), `save_client_call_state(uid, &Value)`, `interrupt_pending_client_calls() -> usize`. Add it to the `VizierStorageProvider` supertrait and hand-forward every method on `VizierStorage` in `src/storage/mod.rs`.
- [X] T008 Implement `init_client_call_schema` (table and indexes exactly as in data-model.md) in `src/storage/sqlite/mod.rs`, called wherever the other `init_*_schema` functions run. Implement `ClientCallStorage for SqliteStorage` in `src/storage/sqlite/client_call.rs` (status/detail/state/timestamps per data-model.md; `finish` uses `WHERE status = 'pending'`). Add unit tests on an in-memory database: open → finish ok; a second finish is a no-op; state save on any status; the interrupt sweep only touches `pending`.
- [X] T009 Implement `fill_client_calls(conn, entries)` in `src/storage/sqlite/history.rs` (chunked `IN (…)` on `history_uid`, like `fill_reactions`; sets `outcome`, mapping `status`+`detail` to `ClientCallOutcome`, and `state` on `ClientToolCall` entries only), and call it at every site that calls `fill_reactions`. Add a unit test that a saved call reads back with its outcome and state.
- [X] T010 Delete `client_tool_call` rows for the topic in `delete_session` (`src/storage/sqlite/session.rs`, matching `agent_id`, `channel` slug, `topic`) and for the agent in `delete_agent` (`src/storage/sqlite/agent.rs`).
- [X] T011 Add `interrupt_orphaned_client_calls` to `src/dependencies.rs` next to `interrupt_orphaned_background_jobs` (same logging and warn-on-error shape), and call it at startup where the other sweeps run.
- [X] T012 [P] Add `SessionFrame::ClientToolCall { connection_id: String, call: ClientCallFrame }`, `SessionFrame::ClientCallOutcome { call_id: String, outcome: ClientCallOutcome }` and `SessionFrame::ClientCallState { call_id: String, state: Value }` in `src/transport.rs`. `ClientCallFrame { call_id, name, category, tool, arguments }` goes in `src/agents/client/mod.rs`, serialising to the §3 field names. Confirm `src/channels/woken.rs`, `src/channels/reactions.rs` and `src/agents/background/mod.rs` still compile and ignore the new variants.
- [X] T013 [P] Implement `RateLimiter::new(max: usize, window: Duration)` with `check(&self, key: &str) -> bool` (sliding window over `Mutex<HashMap<String, VecDeque<Instant>>>`, pruning expired instants and empty keys on access) in `src/channels/http/rate_limit.rs`, with unit tests (`tokio::time::pause` or injected `Instant`). Add `client_events: Arc<RateLimiter>` (10 per 60 s) and `client_state_saves: Arc<RateLimiter>` (2 per 1 s) to `HTTPState` in `src/channels/http/state.rs`, and construct them where `HTTPState` is built.
- [X] T014 Implement `ClientConnection` in `src/agents/client/connection.rs` per data-model.md:
  - `new(outbound) -> Arc<Self>` (uuid id);
  - `register(call_id) -> Option<oneshot::Receiver<ClientAnswer>>` (`None` when closed);
  - `answer(call_id, ClientAnswer) -> bool` (false = not pending);
  - `cancel(call_id)` (removes the entry and `try_send`s `{"client_tool_cancelled": {call_id}}`);
  - `send(frame: serde_json::Value) -> bool` (`try_send` of a text message);
  - `close()` (sets `closed`, drains `pending` with `ClientAnswer::Disconnected`);
  - `is_open()`.

  Add unit tests: answer resolves the receiver; an unknown id is not pending; `close` disconnects every waiter; `register` after `close` returns `None`.

**Checkpoint**: Foundation ready. `cargo build` and `cargo test` pass, and behaviour is unchanged.

---

## Phase 3: User Story 3 - A developer adds their own client tool (Priority: P1) 🎯 MVP

**Goal**: A WebSocket client declares tools on a request. The agent discovers them under their categories and calls them from scripts. Each call goes to the declaring connection, which answers, and the result comes back to the script. The WebUI has the `registerClientTool` mechanism, declaring and answering through it.

**Independent Test**: quickstart.md §1, §2, §3 and §5 with `scripts/client-tools-e2e.mjs` against a dummyplug agent. In the WebUI, a tool registered from the browser console via `registerClientTool` is declared and answered.

### Server

- [X] T015 [US3] In `src/agents/tools/mod.rs`, add `reserved_category_name(name, reserved: &[String], suffix: &str) -> String` (a clash with a `ToolCategory::ALL` name or with `reserved` gets `_{suffix}`), refactor `mcp_category_name` onto it (suffix `mcp`, behaviour unchanged), and add `client_category_name(name, reserved)` (suffix `client`, where `reserved` is the addon ids plus the MCP server keys plus their category names). Unit tests: `memory` → `memory_client`; a category equal to an MCP key → `<key>_client`; unrelated names unchanged; MCP results unchanged.
- [X] T016 [US3] Add `client: Option<Arc<crate::agents::client::ClientTools>>` to `ToolContext` in `src/agents/tools/mod.rs`, and set `client: None` at every other construction site: `src/agents/process.rs` (two sites), `src/agents/agent/mod.rs` (the non-chat site near the second `ToolContext {`), and `src/agents/background/mod.rs`.
- [X] T017 [US3] Add `ToolTarget::Client { category: String, tool: String }`, plus `turn_catalogue(router: &ToolRouter, ctx: &ToolContext) -> Vec<CatalogueEntry>` (router entries first, then `ctx.client`'s entries) and `turn_categories(base: &[CategorySummary], ctx: &ToolContext) -> Vec<CategorySummary>` (base, then the client categories), in `src/agents/tools/mod.rs`. Make `ToolRouter::dispatch` return a clear error for a `Client` target (only `RouterBridge` dispatches those).
- [X] T018 [US3] Implement `ClientTools::resolve(binding, catalogue: &[CatalogueEntry], reserved: &[String], session, storage, transport) -> Arc<ClientTools>` in `src/agents/client/mod.rs`:
  - category names via `client_category_name`;
  - tool names via `assign_names(taken = every catalogue name, pairs = (category_name, tool))`;
  - `CatalogueEntry`s with a `ToolDefinition` built from each declaration, its description prefixed `(Provided by the person's client.) `;
  - `CategorySummary` lines `(provided by the person's client) <description or "Tools provided by the person's client">` in declaration order;
  - `resolved_frame()` returning the §2 JSON with `protocol: 1`.

  Unit tests: default category, the `_client` suffix, collision `_2`, label text, determinism for the same input.
- [X] T019 [US3] Implement `ClientTools::call(&self, entry, arguments: Value) -> crate::Result<VizierResponse>` in `src/agents/client/mod.rs` per research.md R7:
  1. Save the history entry (busy-retry like `save_history_or_log`; on failure fall back to a fresh uuid and `warn!`) and `open_client_call`.
  2. `register`, then send the direct `client_tool_call` frame with `answer: true`, and publish the `SessionFrame::ClientToolCall` display copy.
  3. Await under `min(timeout_secs.unwrap_or(60), 600)`.
  4. `finish_client_call` and publish `ClientCallOutcome`.
  5. Return `ToolResponse` on `Ok`, or `Err` with `"client_disconnected"` / `"timeout"` / the client's message.

  Wrap steps 2–4 in a drop guard that, if dropped before an outcome, calls `connection.cancel(call_id)` and spawns `finish_client_call(Cancelled)` plus its publish. On timeout, also `cancel` (this sends `client_tool_cancelled`). Run the call inside an `execute_tool {name}` span the way `ToolRouter::call` does, keeping arguments and results behind `llm::telemetry::capture_content()`.
- [X] T020 [US3] In `src/agents/tools/python/bridge.rs`, make `RouterBridge::entries` use `turn_catalogue(&self.router, &self.ctx)`, so `catalogue()`, `describe()` and `call()` see client entries. In `dispatch`, route a `ToolTarget::Client` entry to `ctx.client.call(...)` inside the same hooks and `tool_timeout` wrapping (dispatch on the entry's target, looked up by name). When recording a client call's `ToolInvocationRecord`, store `arguments` as `{"client_call": <call id>}` (research.md R7, "No double storage").
- [X] T021 [US3] In `src/agents/tools/python/docs_tools.rs`:
  - `ListToolFunctions::call` uses `turn_catalogue(&self.router, ctx)` for the listing and `turn_categories(&self.categories, ctx)` for the unknown-category check and `did_you_mean`;
  - `DescribeToolFunction::call` uses `turn_catalogue`;
  - `entry_doc` handles `ToolTarget::Client` (output schema from the declaration's `output_schema`, if any).
- [X] T022 [US3] In `VizierAgent::chat` (`src/agents/agent/mod.rs`), right after loading the tool snapshot:
  - if `req.client` is `Some` and its connection `is_open()`, compute `tools.router.catalogue()`, build `ClientTools::resolve(...)` and send `resolved_frame()` on the connection;
  - change `prepare_system_prompts` to take the turn's categories (`turn_categories(&tools.categories, &ctx)`; callers without a client pass `&tools.categories`);
  - set `client` on the chat `ToolContext`;
  - take `req.client` out before the request is cloned into history.
- [X] T023 [US3] Add unit tests in `src/agents/agent/system_prompt/sandbox.rs` for a category list with client lines (format from contracts/agent-surface.md, byte-identical for the same input, unchanged output with no client categories).
- [X] T024 [US3] In `handle_socket` (`src/channels/http/api/v1/agents/channel.rs`):
  - create one `ClientConnection` from a clone of `write_tx` at the start, and `close()` it after the loop;
  - for a parsed `VizierRequest` with declarations, run `declare::validate`; on error, send `{"client_error": {"code": "invalid_client_tools", tool, rule, message}}` and `continue` without sending the request; on success, set `request.client = Some(ClientBinding { … })` and clear the wire fields;
  - parse `{"client_tool_result": {call_id, ok | error}}` *before* the `VizierRequest` fallback; refuse a serialised `ok` over 1 MB with `result_too_large`, otherwise `connection.answer(...)`; on false, send `not_pending`;
  - in the `session_events` branch, serialise `SessionFrame::ClientToolCall` as `{"client_tool_call": {…call, "answer": false}}` and skip it when `connection_id == connection.id`; serialise `ClientCallOutcome` as `client_tool_outcome` and `ClientCallState` as `client_call_state` (wire shapes from contracts/client-tools-protocol.md).

### WebUI

- [X] T025 [P] [US3] Add the frame and history types to `webui/app/interfaces/types.ts`: `ClientToolDeclaration`, `ClientCategoryDeclaration`, `ClientCallOutcome`, `ClientToolCallFrame`, `ClientToolsResolvedFrame`, `ClientToolOutcomeFrame`, `ClientCallStateFrame`, `ClientToolCancelledFrame`, `ClientErrorFrame`; `ChatMessage.content.ClientToolCall`; `WebSocketMessage.client_tools?` / `client_categories?`; `VizierRequestContent` `{ client_event: { text, data?, source_call_id? } }`; `Turn.clientCalls: ClientCallView[]`.
- [X] T026 [P] [US3] Implement the registry in `webui/app/lib/clientTools.ts` exactly per contracts/webui-extension-api.md: `registerClientTool`, `registerClientCategory`, `declaredClientTools()`, `findClientTool(category, tool)`, and `handleClientCall(frame, ctx) -> Promise<{ ok } | { error }>` (unknown tool → `error: "this client does not provide <tool>"`; thrown `Error` → its message).
- [X] T027 [US3] In `webui/app/hooks/connectionStore.tsx`:
  - `sendMessage` adds `declaredClientTools()` to every request frame (any frame with `content`), never to reaction or `client_*` frames;
  - route incoming `client_tool_call` (`answer: true` → `handleClientCall`, then send `client_tool_result`), `client_tools_resolved`, `client_tool_outcome`, `client_call_state`, `client_tool_cancelled` and `client_error` to a new `subscribeClientFrames(listener)` set, mirroring `subscribeReactionFrames`, and never to `lastMessage`;
  - log `client_error` and show an error toast for `invalid_client_tools`.

### End-to-end

- [X] T028 [US3] Create `scripts/client-tools-e2e.mjs`: dependency-free, Node ≥ 22 global `WebSocket`, env `VIZIER_URL`, `VIZIER_TOKEN`, `AGENT`, fresh topic uuid per run, small helpers `send`, `nextFrame(predicate, timeoutMs)`, `history()` via `fetch`, exit non-zero with the failing check's name. Implement quickstart.md §1, §2, §3 and §5 (with a second connection for §5). Run it against a dummyplug agent and fix any failures.

**Checkpoint**: client tools work end to end over the raw protocol, and the WebUI declares and answers registered tools.

---

## Phase 4: User Story 1 - Agent shows data as an interactive page in the WebUI (Priority: P1)

**Goal**: `render_interactive_ui` draws agent HTML in a sandboxed frame inside the reply bubble as soon as it is called, and keeps it there on reload, in other tabs, and in other browsers.

**Independent Test**: quickstart.md §9 steps 1, 4 (page part), 6 and 7 in the browser with a dummyplug agent.

- [X] T029 [P] [US1] Extend `webui/app/lib/trail.ts`:
  - `groupHistory` puts each `ClientToolCall` entry into the open turn's `clientCalls` (opening a turn if none is open, like a trail event), keeping `outcome` and `state`;
  - add pure live helpers `appendClientCall(turn, frame)` (dedupe by `call_id`), `applyClientOutcome(turns, frame)` and `applyClientState(turns, frame)`;
  - `Turn` literals start with `clientCalls: []`.

  Add tests in `webui/app/lib/trail.test.ts`: a call lands in its turn between request and outcome; a turn with a call and no outcome keeps it; live and reloaded turns give the same `clientCalls`; outcome and state updates apply by `call_id`; unknown entry kinds still don't throw.
- [X] T030 [P] [US1] Create `webui/app/components/client/ClientCallChip.tsx`: a collapsed item showing the tool name, a status badge from `outcome.status` (pending shows a spinner), and arguments expandable as formatted JSON. Tailwind, matching `ActivityTrail.tsx`'s style.
- [X] T031 [P] [US1] Create `webui/app/components/client/InteractivePage.tsx` per research.md R12 and contracts/webui-extension-api.md:
  - `<iframe sandbox="allow-scripts" referrerpolicy="no-referrer" title={args.title}>` with `srcdoc` = CSP meta + bridge script (with `vizier.state` set to the call's state) + `args.html`;
  - borderless, full width;
  - height from `vizier:height` messages, capped at 80vh;
  - accepts `message` events only when `event.source === iframe.contentWindow`;
  - an interaction gate (window `blur` with `document.activeElement === iframe` opens a 10 s window) and a 1/s throttle on `vizier:send`, which call `ctx.sendEvent`; dropped sends `console.warn`;
  - `vizier:state` is coalesced to the latest value per 500 ms and calls `ctx.saveState`;
  - new state from props is posted into the frame as `{type: 'vizier:state', value}`, unless it is the value this frame just saved.
- [X] T032 [US1] Create `webui/app/lib/clientTools/builtin.tsx` registering `render_interactive_ui` (category `ui`, the description content listed in contracts/webui-extension-api.md, a `handle` that enforces 256 KB and returns `{ ui_id: callId, status: 'rendered' }`, `render` → `<InteractivePage>`) and `registerClientCategory({ name: 'ui', … })`, and import it once from `webui/app/root.tsx`.
- [X] T033 [US1] In `webui/app/routes/chat.tsx`:
  - keep the live turn's `clientCalls` from `subscribeClientFrames` (`client_tool_call` frames of both kinds via `appendClientCall`, outcome and state via the apply helpers);
  - render `turn.clientCalls` inside the agent's reply bubble, above the outcome text, each through `findClientTool(...)?.render` or `<ClientCallChip>`, also for turns with no outcome (running, failed, aborted);
  - provide a `ClientCallContext` (`sendEvent` and `saveState` may be no-ops until T037/T045).
- [X] T034 [US1] Walk quickstart.md §9 steps 1, 6 (the full SC-004 isolation list) and 7, plus a reload and a second-browser check of the page, against a dummyplug agent. Record the results in the PR description.

**Checkpoint**: pages render and persist. MVP demo-able.

---

## Phase 5: User Story 2 - Clicking inside a rendered page talks to the agent (Priority: P1)

**Goal**: `vizier.send(text, data?)` (and any client's `client_event`) starts a turn as the person. The agent sees the text, the data and the source call.

**Independent Test**: quickstart.md §6 via the e2e script, and §9 step 2 in the browser.

- [X] T035 [P] [US2] Add `ClientEvent { text, data: Option<Value>, source_call_id: Option<String> }` and `VizierRequestContent::ClientEvent(ClientEvent)` (wire key `client_event`) in `src/schema/request.rs`:
  - `kind()` → `"client_event"`;
  - `Display` → `text`, plus `\n\n```json\n<data>\n```` when `data` is present;
  - `generate_frontmatter` adds `via: client_event` and `source_call` (when present) while keeping `sender: self.user`.

  Unit tests for the frontmatter and Display.
- [X] T036 [US2] In `src/agents/process.rs`, add `VizierRequestContent::ClientEvent(_)` to the `Chat | AudioChat | BackgroundReport` dispatch arm (prompt text = `event.text`, chat passage budget as for `Chat`) and to the `ThinkingStart` match. Fix any other exhaustive matches the compiler reports, treating `ClientEvent` like `Chat`.
- [X] T037 [US2] In `handle_socket` (`src/channels/http/api/v1/agents/channel.rs`), for a request whose content is `ClientEvent`:
  - refuse when `state.client_events.check(&session.to_slug())` fails (`client_error` `rate_limited`, no send);
  - refuse `text` that is empty or over 4 KB, or `data` over 64 KB (`client_error` `invalid_client_event`);
  - clear `source_call_id` unless `get_history_entry` finds a `ClientToolCall` in this session.

  Pass the limiter from `HTTPState` into `handle_socket`.
- [X] T038 [US2] In `webui/app/hooks/connectionStore.tsx`, add `sendClientEvent(text, data?, sourceCallId?)` building a request with `content: { client_event: … }` (declarations attached by `sendMessage`), and make `ctx.sendEvent` in `webui/app/routes/chat.tsx` use it and add the optimistic user bubble the way a typed message does.
- [X] T039 [US2] In `webui/app/routes/chat.tsx`, render a `client_event` request as the person's bubble showing `text` with a small "from a page" marker (never `data`), at every site that reads `Request.content` (the user-message render, the queued-message render, and the turn prompt lookup near the end of the file).
- [X] T040 [US2] Add quickstart.md §6 to `scripts/client-tools-e2e.mjs` and run it against dummyplug, then walk §9 step 2 in the browser.

**Checkpoint**: pages are interactive. The core request (button → "hello" → agent reply) works.

---

## Phase 6: User Story 5 - Connection problems never hang a turn (Priority: P2)

**Goal**: disconnects, timeouts, aborts and stray answers end calls cleanly. (The mechanism is in T014, T019 and T024. This phase proves it and closes the gaps.)

**Independent Test**: quickstart.md §4 via the e2e script, and §9 step 7.

- [X] T041 [US5] Add unit tests in `src/agents/client/mod.rs` with an in-memory storage and a test connection:
  - closing the connection mid-call yields `client_disconnected` and outcome `Disconnected`;
  - a 1 s timeout yields `timeout`, outcome `Timeout` and a `client_tool_cancelled` frame;
  - dropping the call future (simulated abort) yields outcome `Cancelled` and a cancel frame;
  - a late answer after timeout is `not pending`.
- [X] T042 [US5] In `webui/app/hooks/connectionStore.tsx`, ignore the result of a `handle` whose call was cancelled (track cancelled `call_id`s from `client_tool_cancelled`, and don't send a `client_tool_result` for them).
- [X] T043 [US5] Add quickstart.md §4 to `scripts/client-tools-e2e.mjs` and run it against dummyplug.

---

## Phase 7: User Story 4 - Turns without a client behave sensibly (Priority: P2)

**Goal**: no client tools outside a declaring request's own turn, and the background-report turn is told so.

**Independent Test**: quickstart.md §8 via the e2e script.

- [X] T044 [US4] Append the closing line from contracts/agent-surface.md to `render_report` in `src/agents/background/report.rs`, and update its unit tests.
- [X] T045 [US4] Add a unit test in `src/agents/tools/mod.rs` that `turn_catalogue` and `turn_categories` with `client: None` equal the router's own catalogue and categories, and that an unknown client category from `list_tool_functions` returns `available: false` with `did_you_mean`.
- [X] T046 [US4] Add quickstart.md §8 to `scripts/client-tools-e2e.mjs` and run it against dummyplug.

---

## Phase 8: User Story 6 - Pages remember their own state (Priority: P2)

**Goal**: a page's (or any client's) saved state for a call is stored, broadcast to every tab, and restored on any browser.

**Independent Test**: quickstart.md §7 via the e2e script, and §9 steps 3–5 in the browser.

- [X] T047 [US6] In `handle_socket` (`src/channels/http/api/v1/agents/channel.rs`), handle `{"client_call_state": {call_id, state}}` (parsed before the `VizierRequest` fallback):
  - `not_found` unless `get_history_entry` is a `ClientToolCall` in this session;
  - `state_too_large` over 64 KB serialised;
  - `rate_limited` when `state.client_state_saves.check(call_id)` fails;
  - otherwise `save_client_call_state`, then publish `SessionFrame::ClientCallState`;
  - storage failures → `client_error` `state_not_saved` with a `warn!`.
- [X] T048 [US6] In `webui/app/hooks/connectionStore.tsx`, add `saveClientState(callId, state)`, and wire `ctx.saveState` in `webui/app/routes/chat.tsx` to it. Confirm `InteractivePage` (T031) receives state updates from `applyClientState` and posts them into the frame.
- [X] T049 [US6] Add quickstart.md §7 to `scripts/client-tools-e2e.mjs` and run it against dummyplug, then walk §9 steps 3–5 in the browser (counter persists across reload and other browser; other tab's `onState` fires).

---

## Phase 9: Polish & Cross-Cutting Concerns

- [X] T050 [P] Create `docs/src/api-integration/client-tools.md` from `specs/019-client-tools/contracts/client-tools-protocol.md` (user-facing wording; drop spec references). Add it to `docs/src/SUMMARY.md`, and in `docs/src/api-integration/rest-api.md` add the `client_event` row to the content table plus a link from the WebSocket chat section.
- [X] T051 [P] Add a "Client tools" paragraph to the Architecture section of `CLAUDE.md`:
  - declarations on `VizierRequest` → `ClientBinding` → `ClientTools` in `ToolContext`;
  - calls go direct to the declaring connection;
  - history uid = call id, with outcome and state in `client_tool_call`;
  - `client_event`;
  - the WebUI `registerClientTool`;
  - the `srcdoc` sandbox.

  Edit only that section; the file has unrelated uncommitted changes.
- [X] T052 Run `cargo clippy`, `cargo test`, `cd webui && npm run typecheck` and `cd webui && node --test app/lib/trail.test.ts`, and fix everything they report.
- [X] T053 Run quickstart.md validation end to end on a fresh `just run`: `node scripts/client-tools-e2e.mjs` (§1–§8) green, then the full §9 browser walk, then the Cleanup section (delete topic → rows gone; restart with a pending call → `interrupted`).
- [ ] T054 End-to-end check with a `dummyplug` agent on a running binary (constitution e2e gate): confirm a Discord/Telegram-free, client-tool-free WebUI chat and a scheduled task run behave exactly as before (SC-006), and, with `OTEL_EXPORTER_OTLP_ENDPOINT` set (`just run-otel`), that a client call appears as an `execute_tool` span under its `invoke_agent` span with no arguments unless content capture is on.

---

## Dependencies & Execution Order

### Phase dependencies

- **Setup (Phase 1)**: none.
- **Foundational (Phase 2)**: needs Setup. Blocks every story.
- **US3 (Phase 3)**: needs Foundational. **Blocks US1, US2, US5, US4 and US6**, which all run on its round trip.
- **US1 (Phase 4)**: needs US3.
- **US2 (Phase 5)**: needs US3. The server half (T035–T037) can run in parallel with US1. T038/T039 touch `chat.tsx` and `connectionStore.tsx` after US1's T033.
- **US5 (Phase 6)**, **US4 (Phase 7)**: need US3. Independent of US1/US2 and of each other.
- **US6 (Phase 8)**: server half (T047) needs only US3. The WebUI half (T048) needs US1's T031/T033.
- **Polish (Phase 9)**: after the stories you ship.

### Within phases

- T006 needs T004 (declaration types) and T014's type name (stub first, if needed).
- T009 needs T005 and T008. T010 and T011 need T008.
- T018 needs T015 and T017. T019 needs T014, T012 and T007/T008. T020/T021 need T017 and T019. T022 needs T018. T024 needs T014 and T013.
- T027 needs T025/T026. T028 needs T024 and T022.
- T033 needs T029–T032.

### Parallel opportunities

- Phase 1: T002 and T003 in parallel with T001.
- Phase 2: T004, T005, T012 and T013 are independent files. Then T007 → T008 → T009/T010/T011, while T014 runs alongside.
- Phase 3: the WebUI tasks T025 and T026 can run while the server tasks T015–T024 are in progress.
- Phase 4: T029, T030 and T031 are separate files.
- After US3: US5 (T041–T043), US4 (T044–T046) and the server halves of US2 (T035–T037) and US6 (T047) can run in parallel.

## Parallel Example: User Story 1

```text
Task: "T029 [US1] Turn.clientCalls in webui/app/lib/trail.ts + trail.test.ts"
Task: "T030 [US1] ClientCallChip in webui/app/components/client/ClientCallChip.tsx"
Task: "T031 [US1] InteractivePage in webui/app/components/client/InteractivePage.tsx"
# then T032 (builtin registration) → T033 (chat.tsx placement) → T034 (browser walk)
```

## Implementation Strategy

### MVP (US3 + US1)

1. Setup and Foundational (T001–T014).
2. US3 (T015–T028): the protocol works end to end over the raw socket. **Stop and validate** with the e2e script.
3. US1 (T029–T034): pages render in the bubble and persist. **Stop and validate** in the browser. This is the first demo.

### Incremental delivery

4. US2 (T035–T040): buttons talk to the agent. This completes what was originally asked for.
5. US5 (T041–T043) and US4 (T044–T046): hardening.
6. US6 (T047–T049): saved state.
7. Polish (T050–T054).

## Notes

- Conventional commits: `feat:` for each story's increment, `doc:` for T050/T051; tests ride in the same commit as their code.
- `CLAUDE.md` and `Cargo.lock` carry unrelated uncommitted changes in this working tree. Stage only the hunks you own.
- No migration or compatibility code is added, so there is nothing to add to #82 (research.md R16).
