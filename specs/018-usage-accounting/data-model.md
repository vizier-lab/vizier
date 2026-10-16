# Data Model: Complete Usage Accounting

**Feature**: `018-usage-accounting` | See [research.md](research.md) for the reasoning behind each choice.

## 1. Rust types

### `UsageScope` (src/agents/agent/model/usage.rs)

What a completion is for. Required on `VizierModel::complete` / `complete_until` (R1).

| Field | Type | Notes |
|---|---|---|
| `session` | `Option<VizierSession>` | The conversation the call belongs to. `None` only for a call with no session, which none of today's call sites make. |
| `purpose` | `UsagePurpose` | `Turn` \| `Handover` \| `Vision` |
| `turn` | `Option<TurnId>` | Set for every round of a `Turn` (R3). |
| `round` | `Option<u32>` | 1-based round within the turn (`turn_depth`). |

### `UsageActivity` (src/schema/usage.rs)

`chat` \| `background` \| `scheduled` \| `dreams` \| `compaction` \| `vision` \| `other`, serialised in snake_case and stored as text. It is derived once, at write time (R4):

- `Handover` → `compaction`
- `Vision` → `vision`
- `Turn` → `session.1.usage_activity()`:

| `VizierChannelId` | activity | `place` (R9) |
|---|---|---|
| `DiscordChanel(id)` | chat | `discord:{id}` |
| `TelegramChannel(id)` | chat | `telegram:{id}` |
| `HTTP(user, id)` | chat | `http:{channel slug}:{session topic}` (the slug, because one user talks through several HTTP channels) |
| `Task(slug, _)` | scheduled | `task:{slug}` |
| `Subagent`, `InterAgent(_)` | background | `background` |
| `Dream(_, _)` | dreams | `dreams` |
| `System` | other | `system` |

Vision and compaction calls take the `place` of their session, like a turn would.

### `TurnId` and `TurnGuard`

- `TurnId` is a `Uuid` newtype.
- `TurnGuard::new(agent_id, deps)` is created at the start of `prompt` and `dream_prompt`.
- `.succeeded()` consumes the guard without recording anything.
- `Drop` without `succeeded()` spawns `record_failed_turn` (R3).

### `UsageEvent` (the row written per call)

Fields as in the `usage_event` table (§2), minus `id`.

### `TokenCounts` (src/schema/metrics.rs)

`{ input, cached, cache_write, output }`: the argument of the generalised `estimate_cost(&TokenCounts, &Cost, cache_outside_input)` (R7).

## 2. SQLite schema (`init_usage_schema`, src/storage/sqlite/mod.rs)

```sql
CREATE TABLE IF NOT EXISTS usage_event (
    id                          INTEGER PRIMARY KEY,
    agent_id                    TEXT    NOT NULL,
    at                          INTEGER NOT NULL,          -- ms since epoch, when the answer arrived
    activity                    TEXT    NOT NULL,          -- UsageActivity
    channel                     TEXT,                      -- VizierChannelId::to_slug(), NULL without a session
    topic                       TEXT,                      -- session topic, if any
    place                       TEXT    NOT NULL,          -- Channel-breakdown key (§1)
    turn_id                     TEXT,                      -- TurnId, Turn rounds only
    round                       INTEGER,
    provider                    TEXT    NOT NULL DEFAULT '',  -- '' only on legacy rows without one
    model                       TEXT    NOT NULL DEFAULT '',  -- requested model id (pricing key)
    served_model                TEXT,                      -- CompletionResponse.model
    input_tokens                INTEGER NOT NULL DEFAULT 0,
    cached_input_tokens         INTEGER NOT NULL DEFAULT 0,
    cache_creation_input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens               INTEGER NOT NULL DEFAULT 0,
    duration_ms                 INTEGER NOT NULL DEFAULT 0,
    legacy                      INTEGER NOT NULL DEFAULT 0  -- 1 = carried over from a reply's stats (R6)
);
CREATE INDEX IF NOT EXISTS usage_event_agent_at   ON usage_event (agent_id, at);
CREATE INDEX IF NOT EXISTS usage_event_agent_conv ON usage_event (agent_id, channel, topic);
CREATE INDEX IF NOT EXISTS usage_event_turn       ON usage_event (turn_id) WHERE turn_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS usage_failed_turn (
    turn_id  TEXT    PRIMARY KEY,
    agent_id TEXT    NOT NULL,
    at       INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS usage_failed_turn_agent ON usage_failed_turn (agent_id);
```

**Failed** is defined by a join, never by a column: an event is part of a failed turn iff `turn_id IN (SELECT turn_id FROM usage_failed_turn)`.

**Lifecycle**:
- Rows are inserted, never updated.
- They are deleted only by `delete_agent` (FR-008), for both tables.
- Deleting a session leaves them in place, and the costliest list and details then show the conversation as `deleted`.

**State key**: `usage.accounting_since` (in `state`) is the RFC 3339 time the carry-over finished (R6). While it is absent, startup runs `migrate_reply_usage`.

## 3. `UsageStorage` trait (src/storage/usage.rs)

```rust
#[async_trait]
pub trait UsageStorage {
    async fn record_usage(&self, event: UsageEvent) -> Result<()>;
    async fn record_failed_turn(&self, turn: &TurnId, agent_id: &str, at: DateTime<Utc>) -> Result<()>;

    /// Token sums grouped by `group` (see `UsageGrouping`) for one agent, a time range and an
    /// optional activity; each group also carries (provider, model) so the caller can price it.
    async fn usage_groups(&self, q: &UsageQuery, group: UsageGrouping) -> Result<Vec<UsageGroup>>;

    /// One conversation's events in range, most expensive first is the caller's job (pricing);
    /// returns events with their failed flag, plus the total count.
    async fn conversation_usage(&self, q: &UsageQuery, channel: &str, topic: Option<&str>, limit: usize)
        -> Result<(Vec<UsageEventRow>, u64)>;

    /// Carry-over (R6): insert legacy rows in one transaction per batch.
    async fn insert_legacy_usage(&self, events: Vec<UsageEvent>) -> Result<()>;
}
```

- `UsageQuery { agent_id, start, end, activity: Option<UsageActivity> }`.
- `UsageGrouping` is one of:
  - `Day` (local date of `at`, in UTC, as today) × activity;
  - `Activity`;
  - `Model` (provider, model, served_model);
  - `Place`;
  - `Conversation` (channel, topic).

  Every grouping also splits by `(provider, model)` and by failed / not failed, so pricing, cache rate and failed figures come out of the same rows.
- `UsageGroup { key: Vec<String>, provider, model, failed: bool, tokens: TokenCounts, calls: u64, turns: u64, last_at }`.

## 4. Report types (src/schema/usage.rs), serialised by the HTTP API

These are summarised here; the full JSON is in contracts/http-api.md.

- **`UsageReport`**: `range`, `accounting_since`, `activity`, `summary: UsageSummary`, `previous: Option<PeriodTotals>`, `daily: Vec<DailyUsage>`, `by_activity: Vec<BreakdownRow>`, `by_model: Vec<ModelRow>`, `by_place: Vec<BreakdownRow>`, `costliest: Vec<ConversationRow>` (≤ 10), `unpriced: UnpricedNote`.
- **`UsageSummary`**: `cost_usd: Option<f64>`, `tokens`, `input_tokens`, `cached_input_tokens`, `cache_creation_input_tokens`, `output_tokens`, `calls`, `cache_hit_rate: Option<f64>`, `failed: FailedTotals { cost_usd, tokens, turns }`, `share_of_all: Option<f64>` (set only when filtered).
- **`BreakdownRow`**: `key`, `label`, `detail: Option<String>`, `cost_usd: Option<f64>`, `share: Option<f64>`, `tokens`, `calls`.
- **`ModelRow`**: `BreakdownRow` plus `provider`, `model`, `served_models: Vec<String>`, `cache_hit_rate`, `failed_tokens`, `priced: bool`.
- **`ConversationRow`**: `channel`, `topic`, `label`, `detail`, `activity` (the one with the most cost), `model` (likewise), `tokens`, `cost_usd`, `failed_share`, `last_at`, `exists: bool`.
- **`ConversationUsage`**: `label`, `detail`, `summary: UsageSummary`, `insight: Option<String>`, `calls: Vec<CallRow>`, `more: u64`, `link: Option<UsageLink { kind, path }>`, `no_link_reason: Option<"no_web_page" | "deleted">`.
- **`CallRow`**: `at`, `activity`, `round`, `provider`, `model`, `served_model`, `input_tokens`, `cached_input_tokens`, `output_tokens`, `cost_usd`, `failed`.

Removed: `AgentUsageStats`, `UsageSummary` (old), `ChannelUsage`, `ChannelTypeUsage`, `ChannelTypeUsageDetail`, `DailyChannelTypeUsage`, `DailyUsage` (old). The new types reuse the shorter names where they fit.

## 5. Validation rules

- `start_date` ≤ `end_date`, otherwise `400 invalid_range`. A range longer than 366 days is also `400 invalid_range`.
- An `activity` that is not a `UsageActivity` is `400 unknown_activity`.
- `channel` in the details call must be a slug of the agent's own sessions. An unknown slug returns an empty `calls`, not an error; the conversation may have been deleted.
- `limit` on details is between 1 and 100, default 20.
