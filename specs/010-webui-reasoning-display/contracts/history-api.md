# Contract: session history ordering and paging

**Feature**: `010-webui-reasoning-display` · **Breaking change** (ordering of pre-existing rows is not repaired)

Covers the storage-level ordering guarantee and the HTTP surface that exposes it.

---

## 1. Storage: `seq` and ordering

### Assignment

Every `save_session_history` insert assigns `seq` as
`(SELECT IFNULL(MAX(seq), 0) + 1 FROM session_history)`, under the existing connection mutex.

| # | Guarantee | Requirement |
|---|---|---|
| H1 | `seq` is globally monotonic: for any two entries written in order, the later has the greater `seq`. | FR-001 |
| H2 | No caller supplies `seq`. Only `save_session_history` assigns it. | FR-001 |
| H3 | `seq` is immutable after insert. `update_history_reactions` does not touch it. | FR-001 |
| H4 | Entries written before this feature have `seq IS NULL`, permanently. No backfill. | FR-006 |

### Ordered reads

Every ordered read over `session_history` applies the tie-break. The sites, all in
`src/storage/sqlite/history.rs`:

| Site | Current | Becomes |
|---|---|---|
| `list_session_history` | `ORDER BY timestamp DESC` + stable `sort_by_key(timestamp)` | `ORDER BY timestamp DESC, seq DESC` + stable sort by `(timestamp, seq)` |
| `list_session_history` (stats variant, ~line 359) | `ORDER BY timestamp DESC` + stable sort | same tie-break |
| agent-wide range read (~line 383) | `ORDER BY timestamp DESC` | `ORDER BY timestamp DESC, seq DESC` |
| checkpoint lookup (~line 442) | `ORDER BY timestamp DESC LIMIT 1` | `ORDER BY timestamp DESC, seq DESC LIMIT 1` |
| `list_session_history_until_checkpoint` (~line 492) | `ORDER BY timestamp ASC` | `ORDER BY timestamp ASC, seq ASC` |

| # | Guarantee | Requirement |
|---|---|---|
| H5 | Two entries sharing a `timestamp`, both with non-`NULL` `seq`, are returned in `seq` order on every read, independent of query plan. | FR-002, FR-003 |
| H6 | The in-Rust re-sort is keyed on `(timestamp, seq)`. Keying on `timestamp` alone reintroduces the defect, because the sort is **stable** and would preserve the SQL `DESC` order for ties. | FR-002 |
| H7 | A tie group of `NULL`-`seq` entries returns in arbitrary order — today's behaviour, neither improved nor worsened. | FR-007, FR-008 |
| H8 | A read over a mix of `NULL` and non-`NULL` `seq` succeeds and orders the non-`NULL` entries correctly among themselves. | FR-008 |

**H6 is the actual bug.** `ORDER BY` alone is not enough: `list.sort_by_key(|a| a.timestamp)` runs
afterwards and, being a stable sort, preserves the descending order it was handed for every tie group.
Both halves must change together.

---

## 2. HTTP: `GET /agents/{agent_id}/channel/{channel_id}/topics/{topic_id}/history`

### Query parameters

```rust
pub struct HistoryQuery {
    before: Option<DateTime<Utc>>,
    before_seq: Option<i64>,   // NEW
    limit: Option<usize>,
}
```

Predicate:

| `before` | `before_seq` | WHERE clause |
|---|---|---|
| absent | absent | no bound |
| present | absent | `timestamp < :before` — today's behaviour, unchanged |
| present | present | `timestamp < :before OR (timestamp = :before AND seq < :before_seq)` |
| absent | present | `before_seq` ignored |

| # | Guarantee | Requirement |
|---|---|---|
| H9 | Paging a conversation with `(before, before_seq)` taken from the oldest entry of the previous page yields every entry exactly once. | FR-005, SC-003 |
| H10 | A caller passing only `before` gets exactly today's behaviour. Additive, backward compatible. | — |

### Response body

`Vec<SessionHistory>`, each entry now carrying `seq` (nullable) in its JSON.

**Filter change**: the endpoint currently drops `AssistantMessage` entries
(`src/channels/http/api/v1/agents/channel.rs:137`). **That filter is removed.**

| # | Guarantee | Requirement |
|---|---|---|
| H11 | `AssistantMessage` entries are returned, not filtered. | FR-010 |
| H12 | Every other entry kind is returned as before. No kind is newly dropped. | FR-023 |
| H13 | `seq` is present on the serialized entry, `null` for pre-existing rows. | FR-007 |

---

## 3. Replay shape (internal, but contractual)

`history_entries_to_messages` must produce a message sequence a provider accepts.

| # | Guarantee | Requirement |
|---|---|---|
| H14 | A narrated tool turn replays as **one** `Message::Assistant` containing a leading `Text` block followed by its `ToolCall` blocks, then one `Message::User` of `ToolResult`s. | FR-012 |
| H15 | No two consecutive `Message::Assistant` are emitted for a single narrated turn. | FR-012 |
| H16 | Every `ToolResult` is immediately preceded by the assistant message carrying its `ToolCall`. | FR-012 |
| H17 | History containing no `AssistantMessage` replays byte-identically to today. | FR-013 |

H17 is the regression guard: the change must be invisible for every conversation recorded before it.

### Write-order precondition

`messages_to_history_entries` must emit `AssistantMessage` **before** the `ToolCall` entries from the
same assistant message (FR-011). H14 depends on it: the replay accumulator can only merge narration it
has already seen when the tool calls arrive.
