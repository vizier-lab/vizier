# Data Model: Client Tools

**Feature**: `019-client-tools` | **Date**: 2026-10-17

## Wire types (declared by the client, never stored)

### `ClientToolDeclaration` (`agents/client/declare.rs`)

| Field | Type | Rules |
|-------|------|-------|
| `name` | string | `^[a-z][a-z0-9_]*$`, ≤ 64 bytes; unique per `category` within a request |
| `category` | string? | same pattern; default `client` |
| `description` | string | non-empty, ≤ 2 KB |
| `input_schema` | JSON object | `type` must be `"object"`; ≤ 16 KB serialised |
| `output_schema` | JSON object? | ≤ 16 KB serialised |
| `timeout_secs` | integer? | 1–600; default 60 |
| `deferrable` | bool | default `false`; when `true`, a call made while the declaring connection is gone is deferred until a client reconnects (R18) |

### `ClientCategoryDeclaration`

| Field | Type | Rules |
|-------|------|-------|
| `name` | string | same pattern as a category; a declaration for a category no tool uses is ignored |
| `description` | string? | ≤ 512 bytes |

Per request: ≤ 32 tools, ≤ 16 distinct categories. Both lists sit on `VizierRequest` as `#[serde(default, skip_serializing)]`.

## In-process types (never serialised)

### `ClientConnection` (`agents/client/connection.rs`)

One per WebSocket connection, created when `handle_socket` starts, closed when it exits.

| Field | Meaning |
|-------|---------|
| `id` | uuid v4, also used to skip the socket's own display copies |
| `outbound` | clone of the socket's `write_tx` (`mpsc::Sender<ws::Message>`) |
| `pending` | `Mutex<HashMap<call_id, oneshot::Sender<ClientAnswer>>>` |
| `closed` | `AtomicBool`; `close()` sets it and answers every pending call `Disconnected` |

`ClientAnswer = Ok(Value) | Err(String) | Disconnected`.

### `ClientBinding`

`VizierRequest.client: Option<ClientBinding>` (`#[serde(skip)]`): `{ connection: Arc<ClientConnection>, tools: Vec<ClientToolDeclaration>, categories: Vec<ClientCategoryDeclaration> }`. Set by the WebSocket handler after validation; absent on every other construction site.

### `ClientTools`

`ToolContext.client: Option<Arc<ClientTools>>`, built in `VizierAgent::chat` at turn start from the binding, and only if `!connection.closed`.

| Field | Meaning |
|-------|---------|
| `connection` | the binding's connection |
| `entries` | `Vec<ClientEntry { name, category_name, category, tool, declaration }>`, after R4 resolution |
| `categories` | `Vec<CategorySummary>` labelled per R6, in declaration order |
| `session`, `storage`, `transport` | for persisting calls and publishing frames |

### `ToolTarget::Client { category, tool }`

New dispatch target in `tools/mod.rs`, produced only by `turn_catalogue`.

## Stored types

### `SessionHistoryContent::ClientToolCall` (`schema/history.rs`)

Saved once, when the agent calls the tool. The history uid **is** the call id.

```rust
ClientToolCall {
    name: String,          // final name, e.g. "ui_render_interactive_ui"
    category: String,      // the client's declared category, e.g. "ui"
    tool: String,          // the client's declared name, e.g. "render_interactive_ui"
    arguments: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    outcome: Option<ClientCallOutcome>,   // filled on read from client_tool_call
    #[serde(default, skip_serializing_if = "Option::is_none")]
    state: Option<serde_json::Value>,     // filled on read from client_tool_call
}
```

The `data` blob is written with `outcome` and `state` absent. They are filled on every read (R8). `history_entries_to_messages` skips the variant, and `content_type_discriminant` returns `"ClientToolCall"`.

### `ClientCallOutcome`

```rust
#[serde(rename_all = "snake_case", tag = "status")]
enum ClientCallOutcome {
    Pending,
    Ok { result: Value },
    Error { message: String },
    Disconnected,
    Timeout,
    Cancelled,
    Interrupted,
    /// Waiting for a client to reconnect (deferrable tools only); not final.
    Deferred,
}
```

### Table `client_tool_call` (`init_client_call_schema` in `storage/sqlite/mod.rs`)

```sql
CREATE TABLE IF NOT EXISTS client_tool_call (
    history_uid      TEXT PRIMARY KEY,
    agent_id         TEXT NOT NULL,
    channel          TEXT NOT NULL,      -- VizierChannelId::to_slug()
    topic            TEXT,
    status           TEXT NOT NULL,      -- pending | ok | error | disconnected | timeout | cancelled | interrupted
    detail           TEXT,               -- JSON: result (ok, ≤ 1 MB) or error message
    state            TEXT,               -- JSON, ≤ 64 KB; NULL = never saved
    state_updated_at TEXT,
    created_at       TEXT NOT NULL,
    finished_at      TEXT
);
CREATE INDEX IF NOT EXISTS idx_client_call_session ON client_tool_call(agent_id, channel, topic);
CREATE INDEX IF NOT EXISTS idx_client_call_pending ON client_tool_call(status) WHERE status = 'pending';
```

### `ClientCallStorage` (new trait, `storage/client_call.rs`, part of `VizierStorageProvider`)

| Method | Use |
|--------|-----|
| `open_client_call(uid, session)` | insert `pending` at call time |
| `finish_client_call(uid, outcome)` | set `status`, `detail`, `finished_at`; only from `pending` (a late write never overrides) |
| `save_client_call_state(uid, state) -> Result<()>` | upsert `state` and `state_updated_at` |
| `interrupt_pending_client_calls() -> usize` | startup sweep (`pending` only; `deferred` survives) |
| `defer_client_call(uid) -> bool` | `pending → deferred` |
| `deferred_client_calls(session) -> Vec<uid>` | a session's deferred calls, oldest first, for delivery on connect |
| `delete_session_client_calls(session)` / via `delete_agent` | cleanup |

Filling on read is internal to `sqlite/history.rs` (`fill_client_calls`, next to `fill_reactions`), so no trait method is needed for it.

## State transitions

```
            ┌───────────────► ok
            ├───────────────► error
 pending ───┼───────────────► disconnected   (answering socket closed)
            ├───────────────► timeout        (no answer within the limit)
            ├───────────────► cancelled      (turn aborted / script dropped the call)
            ├───────────────► interrupted    (process restarted; startup sweep)
            └───────────────► deferred ──┬──► ok | error | timeout   (delivered to a reconnected client)
                                         └──► (stays deferred: delivering socket dropped; survives restarts)
```

Every transition but `pending → deferred` is final. `finish_client_call` applies from `pending` or `deferred`. State (`state` column) is independent of the status and may be saved in any status, including `pending`.

## Request content

### `VizierRequestContent::ClientEvent(ClientEvent)` (`schema/request.rs`)

```rust
pub struct ClientEvent {
    pub text: String,                     // shown as the person's bubble; non-empty, ≤ 4 KB
    #[serde(default)] pub data: Option<serde_json::Value>,      // ≤ 64 KB
    #[serde(default)] pub source_call_id: Option<String>,       // dropped if not a ClientToolCall in this session
}
```

`kind()` → `"client_event"`; `Display` → `text`, plus a fenced JSON block of `data`. Frontmatter (R10): `sender: <user>`, `via: client_event`, `source_call: <uid>` when present.

## Session frames (`transport.rs::SessionFrame`)

| Variant | Wire frame | Who receives it |
|---------|-----------|-----------------|
| `ClientToolCall { connection_id, call }` | `{"client_tool_call": {…, "answer": false}}` | every socket on the session except `connection_id` (the display copy) |
| `ClientCallOutcome { call_id, outcome }` | `{"client_tool_outcome": {…}}` | every socket on the session |
| `ClientCallState { call_id, state }` | `{"client_call_state": {…}}` | every socket on the session |
| `ClientCallDeferred { call_id }` | — (not forwarded) | every socket on the session tries to claim and deliver the call |

The direct frames (`client_tools_resolved`, `client_tool_call` with `answer: true`, `client_tool_cancelled`, `client_error`) go only through the connection's `outbound` and never through the broadcast. See `contracts/client-tools-protocol.md`.

## WebUI model (`interfaces/types.ts`, `lib/trail.ts`)

- `Turn.clientCalls: ClientCallView[]`, where `ClientCallView = { callId, name, category, tool, args, outcome?, state?, live: boolean }`.
- `ChatMessage.content.ClientToolCall?` mirrors the stored variant.
- `VizierRequestContent` gains `{ client_event: { text, data?, source_call_id? } }`.
