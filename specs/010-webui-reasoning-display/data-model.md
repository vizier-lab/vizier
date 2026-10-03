# Phase 1 Data Model: WebUI Reasoning & Tool Activity Display

Two layers: what is stored (SQLite + the Rust value types) and what the WebUI derives from it. Nothing
new is persisted except one column and one tool input — the trail model is entirely derived.

---

## Stored: `session_history` gains `seq`

```sql
CREATE TABLE IF NOT EXISTS session_history (
    uid          TEXT PRIMARY KEY,
    agent_id     TEXT NOT NULL,
    channel      TEXT NOT NULL,
    topic        TEXT,
    timestamp    INTEGER NOT NULL,
    content_type TEXT NOT NULL,
    data         TEXT NOT NULL,
    seq          INTEGER            -- NEW, nullable
);

CREATE INDEX IF NOT EXISTS idx_sh_seq ON session_history(seq);
```

| Field | Change | Notes |
|---|---|---|
| `seq` | **new** | Nullable `INTEGER`. Global monotonic counter assigned at insert. `NULL` for every row written before this feature — no backfill (FR-006). |

**Assignment**: on insert, `(SELECT IFNULL(MAX(seq), 0) + 1 FROM session_history)`. Serialized by the
existing `self.conn.lock()`, so the read-then-write cannot race. `idx_sh_seq` makes `MAX(seq)` an index
lookup.

**Validation rules**:
- `seq` is never written by any caller; only `save_session_history` assigns it.
- `seq` is never updated after insert. `update_history_reactions` must not touch it.
- A `NULL` `seq` is valid and means "recorded before this feature" (FR-007).

**Ordering contract**: every ordered read becomes `ORDER BY timestamp <dir>, seq <dir>`. For rows with
`NULL seq` this degenerates to today's behaviour, which is the accepted breaking change — SQLite sorts
`NULL` first ascending, last descending, and within an all-`NULL` tie group the order is arbitrary,
exactly as it is now.

**Schema application**: a fresh database gets `seq` from the `CREATE TABLE`. An existing database needs
`ALTER TABLE session_history ADD COLUMN seq INTEGER`, applied through a
`add_column_if_missing(conn, table, column, decl)` helper guarded by `PRAGMA table_info`. First column
addition in the project — see research Decision 2.

---

## Stored: `SessionHistoryContent::AssistantMessage` starts being written

No type change. The variant already exists (`src/schema/history.rs:28`) and the replay path already
handles it; it is simply never written, being filtered on both the write path
(`src/agents/agent/mod.rs:459,495`) and the read path
(`src/channels/http/api/v1/agents/channel.rs:137`). Both filters are removed.

**Position rule (FR-011)**: within one assistant turn, the `AssistantMessage` entry MUST be recorded at
a position *before* the `ToolCall` entries it accompanies. Today `messages_to_history_entries` pushes it
after. This is a hard ordering constraint, not a preference — see the state transition below.

### Replay state transition

`history_entries_to_messages` accumulates pending items and flushes them into `rig_core` messages. It
gains one pending slot:

| Pending state | Added by | Flushed as |
|---|---|---|
| `pending_text: Option<String>` | **new** — an `AssistantMessage` entry | merged into the next assistant-message flush as a leading `Text` block |
| `pending_tool_calls: Vec<ToolCall>` | a `ToolCall` entry | `Message::Assistant { content: [Text(pending_text)?, ToolCall…] }` |
| `pending_tool_results: Vec<ToolResult>` | a `ToolResult` entry | `Message::User { content: [ToolResult…] }` |

An `AssistantMessage` entry **sets `pending_text` and does not flush**. This is the change that matters:
today that arm flushes and pushes its own assistant message.

Required replay shape for a narrated tool turn:

```
stored:   AssistantMessage("Let me check…")   ← must come first
          ToolCall(memory_search, …)
          ToolResult(…)

replays:  Message::Assistant [ Text("Let me check…"), ToolCall(memory_search) ]
          Message::User      [ ToolResult(…) ]
```

Two shapes that MUST NOT be produced, both of which providers reject:

```
✗ Assistant[ToolCall] → Assistant[Text] → User[ToolResult]
    tool result no longer adjacent to its call — today's write order
✗ Assistant[Text] → Assistant[ToolCall] → User[ToolResult]
    two consecutive assistant messages — write-order fix alone
```

---

## Stored: `ExecutePythonInput` gains `intent`

```rust
pub struct ExecutePythonInput {
    /// Python source to run. The value of the last expression is returned as `result`.
    pub code: String,
    /// NEW — required. One short sentence, for the person watching, saying what
    /// this script is for.
    pub intent: String,
}
```

| Field | Change | Validation |
|---|---|---|
| `intent` | **new**, required | Non-`Option<String>`, so a call omitting it fails deserialization and returns an error naming the field. FR-025. |

`ExecutionReport` is **unchanged**. The intent is already persisted and already streamed as part of the
tool call's arguments — history stores `ToolCall { name: "execute_python", arguments: { code, intent } }`
— so duplicating it into the report would spend tokens echoing the model's own input back to it. See
research Decision 6.

---

## Derived: the WebUI trail model

Nothing here is persisted. These types exist in `webui/app/interfaces/types.ts` and are produced by the
pure functions in `webui/app/lib/trail.ts`.

```ts
type TrailEvent =
  | { kind: 'narration'; id: string; text: string }
  | { kind: 'thought';   id: string; text: string }
  | { kind: 'tool';      id: string; name: string; args: Record<string, unknown> }
  | { kind: 'python';    id: string; intent: string | null; report: ExecutionReport | null }

interface Turn {
  key: string
  request?: ChatMessage        // absent for an agent-initiated turn (scheduler, dream)
  trail: TrailEvent[]
  outcome?: ChatMessage        // Response: message | error | audio reply
  live: boolean                // true while streaming — trail renders expanded
}
```

| Entity | Derived from (history) | Derived from (live stream) |
|---|---|---|
| `narration` | `AssistantMessage(text)` | not streamed — history only |
| `thought` | `ToolCall { name: "think" }` → `arguments.thought` | `Thinking(text)` frame |
| `tool` | `ToolCall` where name is not `think`/`execute_python` | `ToolChoice { name, args }` |
| `python` | `ToolCall { name: "execute_python" }` paired with its `ToolResult` by `call_id`; report parsed from `ToolResult.content` | `ToolChoice { name: "execute_python" }` then the following `ToolResponse` whose shape `looks_like` a report |

**Grouping rules** (pure function, ordered input → `Turn[]`):

1. A `Request` entry opens a turn.
2. `AssistantMessage`, `ToolCall` and `ToolResult` entries accumulate into the open turn's trail. If no
   turn is open, one is opened with no `request` — agent-initiated turns are legitimate.
3. A `Response` entry closes the open turn as its `outcome`. A `Response` carrying `Error` closes it
   too (FR-021).
4. `Checkpoint` and `Command` entries are standalone and belong to no turn — they render as dividers,
   as they do today.
5. Adjacent `thought` events merge into one block (FR-020). Adjacent `narration` events likewise.
6. A turn whose trail is empty renders no trail element at all (FR-017).
7. An entry kind the function does not recognise is skipped, not fatal (FR-023).

**Pairing note**: `call_id` makes python pairing exact when derived from history. The live stream has no
correlation id on `ToolChoice`, so a report is matched to the most recent unpaired `execute_python`
event — which is what the current code does implicitly by appending two sequential inline events.

**What is deliberately absent**: no nested tool calls. They never reach history (they bypass the model's
message list entirely), and the live path stops emitting them via `on_nested_tool_call`. So the model
has no representation for them, which is the point of FR-032/FR-034.

**Collapsed label**: derived, not stored — counts by kind plus the turn duration from
`VizierResponseStats.duration` on the closing `Response`. Bounded to two lines regardless of trail size
(FR-018).
