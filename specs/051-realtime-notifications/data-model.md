# Data Model: Realtime Notifications

## `notification` (changed)

Three nullable columns; existing rows keep `NULL` and render with their summary (no backfill).

| Column | Type | Meaning |
|---|---|---|
| `source_session` | `TEXT` | The originating session as JSON (`VizierSession`). Set for chat replies and for background jobs launched from a non-platform conversation. Used to fill `title` on read and to delete with the conversation |
| `source_title` | `TEXT` | A stored title for a source with no session: a task's title when its run ended |
| `excerpt` | `TEXT` | Plain text, at most 140 characters plus an ellipsis. Reply text for chat, answer for a task run, outcome for a job. Never tool arguments, tool results or reasoning |

- **SQLite**: `add_column_if_missing` in `init_notification_schema`, plus the idempotent delete below.
- **PostgreSQL**: `migrations/0003_notification_source.sql` with the same columns and delete.
- **Removed rows**: `DELETE FROM notification WHERE category = 'channel_replies'`; `notification_read` cascades.
- **Deleted with**: the conversation (`delete_session` removes rows whose `source_session` matches, in its transaction) and the agent (already done). Serialization of `VizierSession` is deterministic, so equality on the JSON text is sound.
- **Unchanged**: `event_key` as the exactly-once guard, `audience_user_id`, read rows, 30-day retention.

## Rust types

### `NotificationDraft` (changed)

Gains `source_session: Option<VizierSession>`, `source_title: Option<String>` and `excerpt: Option<String>`. `summary` keeps its rule: built from stored facts, never model output.

### `NotificationItem` (changed, additive)

| Field | Serialized | Meaning |
|---|---|---|
| `title` | yes, omitted when absent | The current conversation title, or `source_title`. Absent when the viewer may not see the source, or when there is none |
| `excerpt` | yes, omitted when absent | From the column. Cleared along with the title when the viewer may not see the source |
| `source_session` | no (`serde(skip)`) | Used by the service to fill and gate `title` |

Fill rule: `source_session` set → title is the session's current title, or "New conversation" when it has none yet, provided its channel's `web_user()` is `None` or the viewer. Otherwise `title` and `excerpt` are both cleared.

### `NotificationCategory` (changed)

`ChannelReplies` removed. `ChatReply`, `TaskRun`, `BackgroundJob` and the rest are unchanged.

### `InboxEvent` (new, on the bus, `transport/events.rs`)

Subject `inbox.events`, envelope kind `inbox_event`. Never sent to a client as is.

```rust
pub enum InboxEvent {
    /// One row was inserted. `audience` is `audience_user_id`.
    Created { item: NotificationItem, audience: Option<String>, source_session: Option<VizierSession> },
    /// One person's read state changed. `id: None` means mark-all.
    Read { user_id: String, id: Option<i64>, read: bool },
    /// Live setup. `owner_id` is carried here because `SetupState` skips it in serde.
    Setup { state: SetupState, owner_id: Option<String> },
    SetupCleared { agent_id: String },
    /// Rows may have changed. `user`: only that person; `agent_id`: viewers of that agent.
    Stale { user: Option<String>, agent_id: Option<String> },
}
```

`source_session` needs an explicit field on `Created` because the item's own is `serde(skip)`.

### `Notifications` (changed)

- Gains a transport slot (`install_transport`, called once after the transport exists, like `SetupBoard::install_publisher`). A row inserted before then simply isn't announced.
- Storage inserts return the inserted rows (`ON CONFLICT DO NOTHING RETURNING id`), so a duplicate `event_key` announces nothing.
- `chat_replied` builds the excerpt from the saved entry; `task_run_ended` from `task_run_response`; `job_ended` takes the origin session and returns early when `platform()` is set.

## State transitions

Setup frames as a client applies them:

```text
(absent) ── setup{starting|downloading} ──▶ running ── setup{...} ──▶ running
running ── setup{ready|failed} ──▶ terminal ── client timer, 5 s ──▶ (absent)
running ── setup_cleared ──▶ (absent)
any ── hello ──▶ replaced by the snapshot
```

A client's notification state has one rule: an id at or below `lastSeenId` is never new.

## WebUI store

`stores/notifications.ts`: `start()` opens the stream instead of a timer and returns its close; the page-merge and "fresh" logic stays, called from `hello` when `latest_id` is ahead and from `resync`. `filter` narrows to `unread | all`. `pollDelay` in `lib/desktop.ts` is removed if nothing else uses it. Gains the bell's anchor element.

`stores/agents.ts`: `setups` and `samples` stay, filled from frames rather than the setup interval, with a timer for terminal entries. The health poll is untouched (out of scope).
