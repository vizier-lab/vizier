# Research: Client Tools

**Feature**: `019-client-tools` | **Date**: 2026-10-17

Each decision below records what was chosen, why, and what was rejected. All of them were taken against the code as it stands on `master` at `45d411d`.

## R1. Where the declaration lives on the request

**Decision**: `VizierRequest` gains two wire fields and one in-process handle:

```rust
#[serde(default, skip_serializing)] pub client_tools: Vec<ClientToolDeclaration>,
#[serde(default, skip_serializing)] pub client_categories: Vec<ClientCategoryDeclaration>,
#[serde(skip)] #[schemars(skip)] #[schema(ignore)] pub client: Option<ClientBinding>,
```

The WebSocket handler (`channels/http/api/v1/agents/channel.rs::handle_socket`) already deserialises a `VizierRequest` straight from each text frame, so the two wire fields arrive with no new envelope. It validates them (R3), then replaces them with `client: Some(ClientBinding { connection, tools, categories })` and clears the wire fields. `skip_serializing` means the declarations never reach the `Request` history entry (FR-007: never stored), and `#[serde(skip)]` on `client` follows the precedent of `trace` (in-process only, never accepted from a client).

**Rationale**: It is what the user asked for ("make `VizierRequest` have a `client_tools` parameter"). It keeps one request type across the transport. It also makes the turn, not the socket, the owner of the tool set: a request queued behind a running turn carries its own declaration with it.

**Alternatives considered**: a wrapper frame `{ request, client_tools }`, rejected because it adds a second request shape to the WebSocket protocol for no gain. Remembering the set on the socket, rejected because the user decided that client tools are per-request and never remembered.

The REST chat endpoint (`POST …/chat`, `agents/mod.rs`) builds its own `VizierRequest` from a body that only has `content`, so it cannot carry declarations at all. FR-032 holds with no code.

## R2. How a call reaches the client: a per-connection link, not the broadcast

**Decision**: Each WebSocket connection gets one `ClientConnection` when it opens:

```rust
pub struct ClientConnection {
    pub id: String,                                   // uuid v4
    outbound: mpsc::Sender<axum::extract::ws::Message>, // a clone of the socket's write_tx
    pending: Mutex<HashMap<String, oneshot::Sender<ClientAnswer>>>,
    closed: AtomicBool,
}
```

A call is registered in `pending` and its frame is written **directly** to the originating socket through `outbound`. A *display copy* (`SessionFrame::ClientToolCall { connection_id, call }`) also goes on the session-event broadcast. Every socket forwards it with `"answer": false`, except the originating one, which skips it because it already has the direct frame.

When `handle_socket` exits, it calls `connection.close()`. That sets `closed` and drains `pending`, answering every waiting call with `ClientAnswer::Disconnected` (FR-015, SC-005).

**Rationale**: The broadcast can drop frames (`RecvError::Lagged`, capacity 1024). A dropped *display* frame is cosmetic, because the page is redrawn from history on reload. A dropped *call* frame would leave the script waiting until its timeout. The direct path is also what makes "only the originating connection answers" (FR-013) structural: answers are looked up in that connection's own `pending` map, so an answer arriving on another socket is "not pending" by construction (FR-016).

**Alternatives considered**: broadcast only, with a per-socket `answer` flag. Rejected because of the lag risk above. A global pending map on the transport keyed by call id, rejected because a foreign socket could then answer a call by guessing its id.

## R3. Validating a declaration

**Decision**: `agents/client/declare.rs::validate(&[ClientToolDeclaration], &[ClientCategoryDeclaration]) -> Result<(), DeclarationError>` is a pure function that runs in the WebSocket handler before the request is sent. Rules (FR-002, limits from the spec's Assumptions):

| Rule | Limit |
|------|-------|
| tool `name` and `category` | `^[a-z][a-z0-9_]*$`, at most 64 bytes |
| tools per request | 32 |
| distinct categories per request | 16 |
| duplicate `(category, name)` | refused |
| `description` | at most 2 KB, non-empty |
| category `description` | at most 512 bytes |
| `input_schema` / `output_schema` | a JSON object whose `type` is `"object"` (input) or any object (output); serialised size at most 16 KB each |
| `timeout_secs` | 1 to 600 |

A failure is answered with one `client_error` frame `{ code: "invalid_client_tools", tool, rule, message }` to that socket only, and the request is **not** sent (FR-002: no turn starts).

**Rationale**: "Valid JSON Schema" is checked structurally rather than by a full JSON Schema validator. The crate tree has none (`jsonschema`, `boon` and `valico` are all absent from `Cargo.lock`), and Principle I asks for a concrete reason before adding one. MCP tool schemas are already accepted without validation and handed to the model as-is, so this matches existing practice. Arguments the model sends to a client tool are not validated against the schema either: the client owns the tool, so the client validates.

**Alternatives considered**: adding `jsonschema`, rejected (weight, Principle I). Validating in the agent at turn start, rejected because FR-002 wants the refusal before any turn starts, and a queued request would otherwise be refused minutes late.

## R4. Naming and categories: reuse the MCP rules

**Decision**: Names are resolved at **turn start**, inside `VizierAgent::chat`. Only there is the agent's catalogue known: built-in names, addon ids, MCP server keys and the MCP tools' assigned names.

- **Category**: `client_category_name(category, reserved)` returns `"{category}_client"` when the category is a built-in category, an addon id, an MCP server key, or an MCP category name; otherwise the category unchanged. The default category is `client`. The existing `mcp_category_name` is refactored onto the same helper, `reserved_category_name(name, reserved, suffix)`, so the clash rule exists once (Principle II).
- **Tool name**: `assign_names(taken, &[(category, tool)])`, the same function MCP uses, with `taken` = every name already in the router's catalogue (native, addon and MCP). A client tool can never take an existing name, and two client tools that collide get `_2`, `_3`, … (FR-005).
- The resolved set is reported to the connection once per turn, before the model is called, as `client_tools_resolved { protocol: 1, tools: [{ category, tool, name, category_name }] }` (FR-006). Every later `client_tool_call` frame repeats the client's own `category` and `tool` next to the final `name`, so a client can dispatch on its own names without keeping the mapping.

**Rationale**: It is the MCP precedent the user agreed to. Resolving against the live catalogue at turn start is the only way the rule "built-ins and MCP first" can be exact.

**Edge**: an MCP server that lists a new tool *during* the turn (`tools/list_changed`) could produce a name a client tool already holds. The turn's catalogue (R5) lists router entries first, so the MCP tool wins the lookup and the client tool becomes unreachable for the rest of that turn. This is accepted and documented: the next turn resolves again.

## R5. Reaching client tools from scripts: through `ToolContext`, not the router

**Decision**: `ToolContext` gains `client: Option<Arc<ClientTools>>`, where `ClientTools` is the turn's resolved set plus its `ClientConnection`. Every other `ToolContext` literal (`process.rs` ×2, `agent/mod.rs` ×1 more, `background/mod.rs`) sets `client: None`, which is also what makes FR-008 hold structurally: dreams, handovers, scheduled tasks, background pieces and reports, and consult/delegate sessions never build a `ClientTools`.

A new helper `tools::turn_catalogue(&ToolRouter, &ToolContext) -> Vec<CatalogueEntry>` returns `router.catalogue()` followed by the client entries (`ToolTarget::Client { category, tool }`). `turn_categories(&[CategorySummary], &ToolContext)` does the same for category lines. The three places that read the catalogue go through them:

- `RouterBridge` (the in-script `list_tools`, `describe_tool` and calls),
- `ListToolFunctions` and `DescribeToolFunction` (they already receive `ctx`, currently unused).

`RouterBridge::dispatch` sends a `ToolTarget::Client` entry to `ctx.client.call(...)` instead of `router.call(...)`. Hooks (`on_nested_tool_call`, `on_tool_response`) and the per-call timeout wrap it exactly as they wrap every other nested call.

**Rationale**: `ExecutePython`, `ListToolFunctions` and `DescribeToolFunction` each hold a `ToolRouter` clone baked in at compose time (`tools::compose`), so a per-turn layer cannot ride on the router without rebuilding the sandbox toolset every turn. `ToolContext` is already the per-turn carrier (`background_depth`, `hooks`, `loaded_images`), and every sandbox tool already receives it.

**Alternatives considered**: a `ToolRouter.client` field plus recomposing `VizierTools` per turn. Rejected: it clones every toolset `HashMap` per turn and touches `compose`, which spec 016/017 rely on staying instance-scoped.

The added `ToolTarget::Client` arm in `RouterBridge::dispatch` follows the existing `Native`/`Mcp` match over *dispatch targets*. It is not a branch over tool kinds, so Principle II is unaffected (see the plan's Constitution Check).

## R6. The system prompt and labelling

**Decision**: `prepare_system_prompts` is passed the turn's categories: the snapshot's `categories` followed by the client ones. Each client line reads

```
- palette: (provided by the person's client) Pick colours
```

and the description defaults to `Tools provided by the person's client` when none was declared. Client categories keep declaration order after the built-in, addon and MCP lines. `describe_tool_function` and in-script `describe_tool` return the client's description prefixed with `(Provided by the person's client.) ` (FR-010). `list_tool_functions` needs no label, because its category already carries one.

**Rationale**: FR-011 and SC-007 hold because `SANDBOX.md` lists categories only, and the same declaration gives the same lines in the same order. The prefix keeps a client-written description from reading as vizier's own instructions on a shared agent.

## R7. The client call itself

**Decision**: `ClientTools::call(entry, arguments, ctx)`:

1. Saves a history entry `SessionHistoryContent::ClientToolCall { name, category, tool, arguments }` through `save_session_history` (busy-retry like `save_history_or_log`) and inserts a `client_tool_call` row with no outcome. The **history uid is the call id**. If the save fails, a fresh uuid is used, the call still goes out, and the failure is logged.
2. Registers a oneshot in `connection.pending`, writes the `client_tool_call` frame directly to the connection, and publishes the display copy.
3. Awaits the answer under `min(declared timeout_secs or 60 s, 600 s)`.
4. Records the outcome (`ok`, `error`, `disconnected`, `timeout`, `cancelled`) and the result or error text on the row, and publishes `SessionFrame::ClientCallOutcome`.
5. Returns the result as a `ToolResponse`, or an `Err` that the script sees as a catchable `RuntimeError` (FR-014).

A drop guard covers the abandoned case: `/abort` aborting the turn's task, or the script's wall-clock limit dropping the bridge future. On drop without an outcome, the guard removes the pending entry, `try_send`s `client_tool_cancelled { call_id }` to the connection, and spawns the `cancelled` outcome write (FR-015, Story 5 scenario 4). The same pattern as `TurnGuard` in spec 018.

**No double storage**: `RouterBridge` records a client call's `ToolInvocationRecord.arguments` as `{"client_call": "<call id>"}` instead of the arguments. Otherwise a 256 KB page would be stored a second time, inside the `execute_python` report that history and the live report frame carry. The full arguments are in the `ClientToolCall` entry the id points to.

**Effective timeout**: the whole `execute_python` run is bounded by the agent's `tools.timeout` (wall clock, `python/mod.rs`), and `RouterBridge` applies the same limit per nested call. A client tool's own limit is therefore `min(declared, 600 s, agent tool timeout)`. The protocol documentation says so.

**Rationale**: Saving *at call time*, not with the turn's other entries (which `chat()` writes only after `prompt_with` returns, `agent/mod.rs` ~L650–700), is what keeps a page on screen after a failed or aborted turn (FR-026a), and gives the call a stable id for saved state and client events. The entry lands after the turn's `Request` and before its trail entries, so `groupHistory` puts it in the right turn (R11).

## R8. The outcome and saved state live outside the history blob

**Decision**: A new table `client_tool_call` (see data-model) holds, per call, the outcome, the result or error, and the saved state. `list_session_history` fills them into the `ClientToolCall` variant's `#[serde(default)]` fields (`outcome`, `state`) on every read, in one chunked `IN (…)` query: the same shape as `fill_reactions`, which exists for the same reason (spec 013 Decision 1: anything that changes after the insert must not live in the `data` blob).

`history_entries_to_messages` skips the variant, so the model never re-reads 256 KB of HTML on later turns. The turn's own `execute_python` report already tells the model what it called. `content_type_discriminant` gains `"ClientToolCall"`.

**Rationale**: History rows are written once and never updated anywhere in the storage layer. Keeping mutable state in a side table keyed by the history uid follows the precedent rather than adding the first `UPDATE session_history`.

## R9. Saved state

**Decision**: The WebSocket gains `{ "client_call_state": { "call_id", "state" } }`. The handler:

1. Checks that the call id is a `ClientToolCall` entry in **this socket's own session** (`get_history_entry`, the way `webui_reaction` scopes a reaction).
2. Checks that the serialised `state` is at most 64 KB, and the per-call rate (2 saves/s).
3. Upserts `client_tool_call.state`.
4. Publishes `SessionFrame::ClientCallState { call_id, state }` to every socket on the session, the saving one included as its confirmation (FR-018c).

A refusal is a `client_error { code, call_id, message }` to that socket only, and the stored state is untouched (FR-018d). State never touches the agent: no request, no history entry, nothing in `context_md` (FR-018b). Last write wins; there is no version check.

**Alternatives considered**: storing state in the browser, rejected because the user asked for it to follow the person to another browser. Optimistic concurrency with a version number, rejected (Principle I): last-write-wins with every tab told the result is what the spec asks for.

## R10. Client events

**Decision**: A new content kind `VizierRequestContent::ClientEvent(ClientEvent { text, data: Option<Value>, source_call_id: Option<String> })`, wire key `client_event`. It joins the `Chat | AudioChat | BackgroundReport` arm in `process.rs` (history, auto-context, skills, `ThinkingStart`) and its `kind()` is `"client_event"`. The WebSocket handler:

- checks the per-session limit (10 per rolling minute) and answers `client_error { code: "rate_limited" }` beyond it (FR-022);
- drops `source_call_id` when it is not a `ClientToolCall` entry in this session (spec edge case: delivered without the reference);
- lets the request declare client tools like any other (FR-023).

Frontmatter keeps the person as the sender and adds where it came from:

```yaml
sender: alice
via: client_event
source_call: <history uid>
metadata: {}
```

The body is `text`, followed by a fenced JSON block of `data` when present.

**Rationale / refinement of the spec**: The brainstorm sketched `sender: client`. FR-020 also requires the event to carry "the same identity … as the sending person", and BOOT.md tells the agent to read `sender` to know who it is talking to. Replacing it would repeat the bug spec 011 fixed for scheduled runs, in reverse. `via` satisfies FR-021 without that cost.

**WebUI side**: the user bubble shows `text`, with a small "from a page" marker, and never shows `data`.

## R11. Placing pages in the reply bubble

**Decision**: The WebUI's `Turn` (`lib/trail.ts`) gains `clientCalls: ClientCallView[]`. `groupHistory` collects `ClientToolCall` entries of the open turn in order, rather than treating them as trail events, so they render whether or not the trail is expanded. A live helper `appendClientCall` adds them from `client_tool_call` frames, both the direct and the display copy. The bubble renders `turn.clientCalls` (each through its registered renderer, else the collapsed chip) above the turn's outcome text, and renders them for a turn with no outcome too (failed, aborted, or still running). *Amended by R19: a renderer may ask to be drawn only once its turn has ended, and only for the turn's latest call.* That gives FR-026a and the reload, other-tab and other-browser scenarios from the same pure function, keeping spec 010's rule that live and reloaded turns come from one module.

## R12. The page sandbox

**Decision**: `components/client/InteractivePage.tsx` renders

```html
<iframe sandbox="allow-scripts" srcdoc="…" referrerpolicy="no-referrer" style="border:0;width:100%">
```

with `srcdoc` = `<meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; font-src data:; media-src data:">` + the bridge script + the agent's HTML, in that order, so the policy and bridge are parsed before any agent markup.

- **No `allow-same-origin`**: the frame's origin is opaque, so it cannot read `localStorage` (`auth_token`), cookies or the parent DOM.
- **No `allow-forms`, `allow-popups`, `allow-modals` or `allow-top-navigation`**: no `alert()` (which would also break the browser automation rule against dialogs), no navigation.
- The CSP blocks every network fetch, including `connect-src`, which falls back to `default-src 'none'`.

The bridge exposes `vizier.send(text, data?)`, `vizier.state` (the initial state), `vizier.saveState(value)` and `vizier.onState(cb)`, and posts its height from a `ResizeObserver`. The parent accepts a message only when `event.source === iframe.contentWindow`.

- **Interaction gate (FR-028)**: the parent marks a page "interacted" when focus moves into its iframe (`window` `blur` with `document.activeElement === iframe`, which is what a click inside the frame produces), and forwards `send` only for 10 s after that and at most once per second. Anything else is dropped with a console warning.
- **State saves**: not gated (FR-027a), but coalesced to the latest value per 500 ms before sending.
- **Height**: capped at 80% of the viewport, then the frame scrolls.

**Alternatives considered**: a separate origin (a sub-domain) for pages, rejected because it needs deployment configuration and breaks single-binary, config-less mode (Principle III). Shadow DOM, rejected: no script isolation.

## R13. The WebUI extension mechanism

**Decision**: `app/lib/clientTools.ts` holds a module-level registry:

```ts
registerClientTool({ definition, handle, render })  // returns an unregister function
declaredClientTools(): { client_tools, client_categories }
registerClientCategory({ name, description })
```

`connectionStore.sendMessage` attaches `declaredClientTools()` to every *request* frame it sends: one place, so every send site (text, audio, retry, quick chat) declares them (FR-024). Incoming `client_tool_call` frames with `answer: true` are dispatched to the matching `handle(args, ctx)`. Its returned value (or thrown error) becomes `client_tool_result`. A tool the WebUI does not recognise is answered with an error, `"this client does not provide <tool>"`, rather than left to time out.

`render_interactive_ui` is registered from `app/lib/clientTools/builtin.ts` through the same call, in category `ui` with description "Show interactive pages in the conversation". Its `handle` answers at once with `{ ui_id: call_id, status: "rendered" }` after checking the 256 KB page limit (FR-030).

## R14. The end-to-end harness

**Decision**: Dummyplug already runs any sandbox tool from a JSON message (`specs/008`, §3), so `{"tool":"execute_python","arguments":{"code":"ui_render_interactive_ui(title='t', html='<button>…')","intent":"…"}}` drives a client tool through the real loop with no protocol change. The test client is a dependency-free Node script, `scripts/client-tools-e2e.mjs`, using Node ≥ 22's global `WebSocket`. Node is already required to build the WebUI. It declares a tool, answers its call, saves state, sends a client event, and asserts on the frames and on `GET …/history`. The quickstart drives the same steps by hand.

**Alternatives considered**: a Rust example with `tokio-tungstenite`, rejected: a new dev-dependency for one script. `websocat`, rejected: not installed by default and cannot assert.

## R15. Rate limiting

**Decision**: One small `RateLimiter` (sliding window: `Mutex<HashMap<String, VecDeque<Instant>>>`, emptied keys pruned on access) in `channels/http/rate_limit.rs`. Two instances live on `HTTPState`, so the limits hold across a person's sockets: client events keyed by session slug (10 per 60 s) and state saves keyed by call id (2 per 1 s). The WebUI's own throttles (R12) keep a well-behaved page under both.

## R16. Lifecycle of stored rows

**Decision**:

- `delete_session` deletes `client_tool_call` rows of that `(agent, channel, topic)`.
- `delete_agent` deletes all of that agent's rows (FR-018e).
- `dependencies.rs` gains `interrupt_orphaned_client_calls`, next to the task-run and background-job sweeps, which marks rows with no outcome as `interrupted` at startup. A call cannot outlive the process that awaits it.

None of this is a migration or a compatibility shim, so nothing is added to #82.

(Note: `delete_session` today leaves the topic's `session_history` rows in place. This feature does not change that; it only removes its own rows.)

## R17. Protocol versioning and documentation

**Decision**: The contract in `contracts/client-tools-protocol.md` becomes `docs/src/api-integration/client-tools.md` (linked from `SUMMARY.md` and from the WebSocket section of `rest-api.md`, which also gains the `client_event` row). It is version 1. `client_tools_resolved.protocol` carries the number, so a client can detect a server speaking a different version. Additive changes (new optional fields, new frame kinds a client may ignore) keep version 1; anything else bumps it.

## R18. Deferrable tools (added after the first hands-on test)

**Decision**: `ClientToolDeclaration.deferrable` (default `false`). In `ClientTools::call`, when the connection is already closed (`register` returns `None`) or answers `Disconnected` and the tool is deferrable, the call is moved `pending → deferred` (`ClientCallStorage::defer_client_call`), `ClientCallOutcome::Deferred` is published, and the script gets `Ok({"status": "deferred", "call_id", "message"})` rather than an error. The description the agent reads is prefixed to say so.

Delivery has two triggers, both handled by `handle_socket`: a socket opening (it lists the session's `deferred` calls, `deferred_client_calls(session)`) and `SessionFrame::ClientCallDeferred` (a call deferred while another tab is open). Either way the socket claims the call in a process-wide `DeferredClaims` set on `HTTPState` (so two tabs never both run it), registers it on its own `ClientConnection`, sends the call frame with `answer: true, deferred: true`, and awaits it under the default 60 s limit. Ok/error/timeout finish it (`finish_client_call` accepts `pending` or `deferred`); `Disconnected` releases the claim and leaves it `deferred`. The call's name and arguments are read back from its history entry, so nothing new is stored.

`interrupt_pending_client_calls` sweeps only `pending`: a deferred call does not depend on the process that made it, so it survives a restart.

**Rationale**: the user's scenario is a turn still running after the tab is closed. Scoping it to the declaring turn keeps FR-007 ("never stored") and US4 intact. At-least-once delivery (a mid-call drop can re-deliver) is accepted and documented: the call id lets a client recognise a repeat.

**Alternatives considered**: delivering only to the next request that declares the tool (rejected by the user: nothing would happen until the person writes); remembering declarations for later client-less turns such as background reports (rejected by the user: reverses FR-007).

## R19. When and how many pages a reply shows (added after the first hands-on test)

**Decision**: `ClientToolRegistration` gains two display options: `renderAfterTurn` (draw only once the turn has ended: it has an outcome, or it is not the turn still running) and `latestPerTurn` (of this tool's calls in a turn, draw only the last). `render_interactive_ui` sets both, and its description tells the agent a reply shows one page and a later call replaces it. A failed `render_interactive_ui` call (`outcome.status == "error"`, e.g. over 256 KB) draws as the chip, not as a page.

**Rationale**: the user found pages appearing mid-turn distracting and several pages per reply cluttered. Putting the rule in the registration keeps `chat.tsx` free of tool-specific branches (Principle II) and lets an extension choose the same behaviour.
