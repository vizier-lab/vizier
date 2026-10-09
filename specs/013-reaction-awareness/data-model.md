# Data Model: Reaction Awareness

**Feature**: `013-reaction-awareness` · **Date**: 2026-10-10

## Tables (SQLite)

Both tables are created by a new `init_reaction_schema(conn)` in `storage/sqlite/mod.rs`, called from `init_schema` after `init_history_schema`. That follows the split-out pattern of `init_history_schema` and `init_background_job_schema`, so tests can stand up just these tables. `PRAGMA foreign_keys=ON` is already set, and history rows are never deleted today, so the cascades are a safety net, not a hot path.

### `message_reaction`

One row per person per emoji per agent message (Key Entity: **Reaction**).

| Column | Type | Notes |
|---|---|---|
| `history_uid` | TEXT NOT NULL | `REFERENCES session_history(uid) ON DELETE CASCADE`. Always an agent `Response` entry |
| `reactor_id` | TEXT NOT NULL | Channel-native identity: the WebUI username, a Discord user snowflake, or a Telegram user or chat id. It isn't namespaced, because a history entry belongs to exactly one channel and that channel defines the id space |
| `reactor_name` | TEXT | Display name at the time of reacting. NULL if it couldn't be resolved, in which case renderers fall back to `reactor_id` |
| `emoji` | TEXT NOT NULL | Unicode emoji. For custom emoji: Discord `<:name:id>` / `<a:name:id>`, Telegram `custom_emoji:<id>`. Telegram paid reactions are `⭐` |
| `added_at` | INTEGER NOT NULL | Unix milliseconds |

```sql
PRIMARY KEY (history_uid, reactor_id, emoji)
CREATE INDEX IF NOT EXISTS idx_reaction_uid ON message_reaction(history_uid);
```

- **Add**: `INSERT OR IGNORE`, so a re-add is a no-op that keeps the original `added_at`.
- **Remove**: `DELETE … WHERE history_uid=? AND reactor_id=? AND emoji=?`. Removing an absent reaction is a no-op, never an add (FR-002).
- **ClearEmoji / ClearAll** (Discord moderator actions): `DELETE … WHERE history_uid=? [AND emoji=?]`.

### `platform_message_link`

One row per platform message the agent posted (Key Entity: **Agent message link**).

| Column | Type | Notes |
|---|---|---|
| `agent_id` | TEXT NOT NULL | Different agents run different bot accounts |
| `platform` | TEXT NOT NULL | `discord` or `telegram` |
| `chat_id` | TEXT NOT NULL | Discord channel id, or Telegram chat id. Telegram message ids are only unique per chat |
| `message_id` | TEXT NOT NULL | |
| `history_uid` | TEXT NOT NULL | `REFERENCES session_history(uid) ON DELETE CASCADE` |

```sql
PRIMARY KEY (agent_id, platform, chat_id, message_id)
```

A reply split into N chunks plus M attachment posts has N + M rows with the same `history_uid`.

### One-time migration

This runs in `init_reaction_schema`. For every `session_history` row with `json_array_length(json_extract(data, '$.reactions')) > 0`:
1. Copy each `{user_id, emoji}` into `message_reaction`, with `reactor_name` NULL and `added_at` set to the row's `timestamp`, using `INSERT OR IGNORE`.
2. Rewrite `data` with `reactions` set to `[]`.

It's idempotent with no marker: after one run, no row qualifies.

## Rust types

### Changed

```rust
// schema/request.rs: the read model attached to SessionHistory, extended compatibly
pub struct ReactionEntry {
    pub user_id: String,                   // = reactor_id
    pub emoji: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_name: Option<String>,         // = reactor_name
}

// schema/response.rs
#[derive(Default, …)]
pub struct VizierResponse {
    pub timestamp: DateTime<Utc>,
    pub content: VizierResponseContent,    // #[default] Empty
    pub attachments: Vec<VizierAttachment>,
    /// The `session_history` uid this response was saved as. Set by whoever saved it,
    /// after the save, so it is never part of the stored `data`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history_uid: Option<String>,
}

// transport.rs
pub enum SessionFrame {
    Response(VizierResponse),
    Job(BackgroundJobSnapshot),
    /// The full current reaction set of one message, after any change to it.
    Reactions { history_uid: String, reactions: Vec<ReactionEntry> },
}
```

`SessionHistory.reactions` keeps its type and serde shape, but it's now **filled on read from `message_reaction`**, never written through `data`.

### New

```rust
// channels/reactions.rs
pub enum ReactionTarget {
    History(String),                                     // WebUI: a history uid
    Platform { agent_id: String, platform: Platform, chat_id: String, message_id: String },
}
pub enum Platform { Discord, Telegram }                  // as_str(): "discord" | "telegram"

pub struct Reactor { pub id: String, pub name: Option<String> }

pub enum ReactionKind { Add(String), Remove(String), ClearEmoji(String), ClearAll }

pub struct ReactionChange { pub target: ReactionTarget, pub reactor: Reactor, pub kind: ReactionKind }
```

`Reactor` is ignored for `ClearEmoji` and `ClearAll`.

### Removed

`ReactionEvent`, `VizierRequestContent::Reaction`, `channels/reaction_store.rs`, and `HistoryStorage::update_history_reactions`.

## Storage trait

There's a new `ReactionStorage`, added to the `VizierStorageProvider` supertrait and forwarded by `VizierStorage`. It's implemented for `SqliteStorage` only. The legacy `FileSystemStorage` isn't a `VizierStorageProvider`, so it needs nothing.

```rust
#[async_trait]
pub trait ReactionStorage {
    async fn add_reaction(&self, history_uid: &str, reactor: &Reactor, emoji: &str) -> Result<()>;
    async fn remove_reaction(&self, history_uid: &str, reactor_id: &str, emoji: &str) -> Result<()>;
    /// `None` clears every emoji.
    async fn clear_reactions(&self, history_uid: &str, emoji: Option<&str>) -> Result<()>;
    async fn list_reactions(&self, history_uid: &str) -> Result<Vec<ReactionEntry>>;

    async fn link_platform_messages(
        &self, agent_id: &str, platform: Platform, chat_id: &str,
        message_ids: &[String], history_uid: &str,
    ) -> Result<()>;
    async fn find_linked_message(
        &self, agent_id: &str, platform: Platform, chat_id: &str, message_id: &str,
    ) -> Result<Option<SessionHistory>>;

    /// For `ReactionTarget::History`: the entry, so `apply` can check it is an agent
    /// `Response` and recover its session for the broadcast.
    async fn get_history_entry(&self, history_uid: &str) -> Result<Option<SessionHistory>>;
}
```

`HistoryStorage::save_session_history` changes its return type from `Result<()>` to `Result<String>`, the uid. Callers that use `.await?;` as a statement compile unchanged.

## Validation rules

| Rule | Where | Requirement |
|---|---|---|
| The target must resolve to a `SessionHistory` whose content is `Response(Message \| AudioReply)` | `reactions::apply` | FR-012 |
| A WebUI target's session must equal the socket's session | WebSocket handler, before `apply` | So one user can't react inside another's conversation |
| A platform target with no link is dropped silently (debug log) | `reactions::apply` | FR-012 and the no-back-fill assumption |
| The reactor must not be the agent's bot or any bot | Each channel, before `apply` | FR-006 |
| Emoji is non-empty and ≤ 64 bytes | `reactions::apply` | Bounds the stored size and the digest |

## Reaction digest (derived, not stored)

`reaction_digest(entries: &[SessionHistory]) -> Option<String>` is pure and lives in `agents/agent/system_prompt/reactions.rs`.

- **Input**: the history slice the turn already loaded. Only `Response` entries with non-empty `reactions` count.
- **Selection**: the last `MAX_DIGEST_MESSAGES = 10` such entries, oldest first.
- **Per message**: a reference (`your reply N messages ago, at HH:MM`), a ≤ 80-character excerpt with think tags and newlines stripped, then each emoji ordered by count descending as `👍 ×3 (alice, bob, carol)`, with at most `MAX_NAMES_PER_EMOJI = 5` names and `+N more`.
- **Output**: `None` when nothing qualifies, otherwise the section text. The format is in `contracts/agent-context.md`.
