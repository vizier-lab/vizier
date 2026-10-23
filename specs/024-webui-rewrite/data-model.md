# Data Model: WebUI Rewrite

Covers only what this feature adds or changes on the server, plus the WebUI's client-side state. Everything else the WebUI reads already exists and is described in earlier specs (015 to 023).

## 1. Storage changes (SQLite)

All are additive. Each column add goes through `add_column_if_missing` in its `init_*_schema` and gets a #82 checklist entry (CLAUDE.md, migration convention).

| Table | Change | Written by | Read by |
|-------|--------|-----------|---------|
| `core_revision` | `+ dream_cycle_id TEXT NULL`, index `(agent_id, dream_cycle_id)` | `set_agent_core` (from `RevisionOrigin.dream_cycle`) | dream nights/cycles (§2.4) |
| `memory_revision` | `+ dream_cycle_id TEXT NULL`, index `(agent_id, dream_cycle_id)` | `BundleMemoryStore::{write_memory, delete_memory, delete_bundle, import_bundle}` | dream nights/cycles |
| `"user"` | `+ last_seen_at INTEGER NULL`, `+ previous_visit_at INTEGER NULL` | auth middleware (throttled, JWT only) | away feed lower bound |
| `session_file.data` (JSON) | `+ sender`, `+ history_uid` (both `serde(default)`, no DDL) | the 8 `save_session_file` callers | session files list |
| `session_history` | new `content_type = "recall"` rows (no DDL) | `StepRecorder` (auto) and the agent loop (search) | history, Recalled memory panel |

No new tables. The away feed, the overview and the conversation list are queries over existing tables (research R8 and R9).

## 2. Server types

### 2.1 Paging (`src/channels/http/models/page.rs`)

```rust
pub struct ListQuery {        // query string
    pub q: Option<String>,
    pub filter: Option<String>,   // one filter key, e.g. "active", "failed", "builtin"
    pub sort: Option<String>,     // entity's sort key
    pub order: Option<SortOrder>, // asc | desc
    pub offset: Option<usize>,
    pub limit: Option<usize>,     // presence switches the response to Page<T>
}

pub struct Page<T> {
    pub items: Vec<T>,
    pub total: usize,                       // after q + filter, before offset/limit
    pub counts: BTreeMap<String, usize>,    // per filter key, after q, ignoring filter
}

pub trait Listing<T> {
    fn matches(&self, item: &T, q: &str) -> bool;
    fn filters(&self) -> &[(&'static str, fn(&T) -> bool)];   // "all" is implicit
    fn sort_key(&self, item: &T, sort: &str) -> SortKey;       // unknown sort → default
}
```

Validation: `limit` is clamped to 1..=100 (default page size is 25 in the WebUI). An unknown `filter` → `400 unknown_filter`. An unknown `sort` falls back to the entity's default.

Filters per entity (from the design's filter chips):

| Entity | Filters | Sorts (default first) |
|--------|---------|------------------------|
| agents | `mine`, `live`, `failed` | `last_active`, `name` |
| tasks | `active`, `last_run_failed` | `next_run`, `name`, `last_run` |
| skills | `agent`, `global` | `name`, `updated` |
| users | one per role name | `created`, `username` |
| roles | `builtin`, `custom` | `people`, `name` |
| MCP servers | `down` | `name`, `tools` |
| API keys | — | `created`, `last_used` |
| bundles | — | `documents`, `updated`, `links` (Memory "Largest / Recently changed / Most linked") |

### 2.2 Overview (FR-018, FR-018a)

```rust
pub struct Overview {
    pub agents: AgentCounts,              // total, live, failed (visible agents only)
    pub cost_today_usd: Option<f64>,      // None when no priced usage today
    pub accounting_since: Option<DateTime<Utc>>,
}

pub struct HourlyActivity {
    pub hours: Vec<DateTime<Utc>>,        // 24 hour starts, oldest first, last = current hour
    pub rows: Vec<ActivityRow>,           // ≤ 6 agent rows (top 5, or all ≤ 6)
    pub all: Option<ActivityRow>,         // present when there are ≥ 7 visible agents
    pub agent_total: usize,
    pub today_by_kind: Option<TurnKinds>, // only when exactly one visible agent
}

pub struct ActivityRow { pub agent_id: Option<AgentId>, pub label: String, pub turns: [u32; 24], pub live: bool }
pub struct TurnKinds  { pub chat: u32, pub scheduled: u32, pub dream_stages: u32, pub background: u32 }
```

Rules:
- A turn is counted once, in the hour of its first `usage_event`, by `COUNT(DISTINCT turn_id)`. Events with `turn_id IS NULL` (handover, vision) are not turns.
- `live` means the agent has a session with `is_thinking = true` now.
- The agent ranking is by total turns in the 24 hours, with ties broken by name. `rank=mine|live|failed` switches the ranking pool, for the design's "Most active · Mine · Live · Failed" switch.

### 2.3 Away feed (FR-019)

```rust
pub struct FeedEvent {
    pub id: String,                 // "<source>:<row id>", stable, the cursor's tiebreak
    pub at: DateTime<Utc>,
    pub agent_id: AgentId,
    pub kind: FeedKind,             // start_failed | task_run | dream | background | integration | channel_reply
    pub tone: FeedTone,             // default | failed
    pub text: String,               // one line, server-rendered from row data (no model output)
    pub href: Option<String>,       // WebUI path to open: run conversation, journal entry, topic …
}
pub struct FeedPage { pub events: Vec<FeedEvent>, pub since: DateTime<Utc>, pub next: Option<String> }

trait FeedSource { async fn events(&self, deps, agents: &[AgentId], since, before: Cursor, limit) -> Result<Vec<FeedEvent>>; }
```

Rules:
- `since` = `previous_visit_at`, else now minus 24 h.
- `start_failed` events come from live health. They are pinned first on the first page and never paged.
- `channel_reply` collapses to one event per (conversation, hour).
- The text is assembled from stored fields only (task title, run outcome, counts), so no conversation content leaks into the feed beyond titles.

### 2.4 Dream nights and cycles (FR-018c)

```rust
pub enum ChangeKind { Wrote, Revised, Removed, Core }

pub struct DreamChange {
    pub kind: ChangeKind,
    pub bundle: Option<String>, pub path: Option<String>,   // memory changes
    pub seq: i64,                                           // revision seq (doc or CORE)
    pub at: DateTime<Utc>,
}

pub struct DreamCycle {
    pub cycle_id: String,
    pub started_at: DateTime<Utc>, pub ended_at: Option<DateTime<Utc>>,
    pub state: DreamCycleState,          // running | complete | failed
    pub sessions_read: usize, pub sources: Vec<String>,     // source session titles
    pub written: u32, pub revised: u32, pub removed: u32, pub core: bool,
    pub changes: Vec<DreamChange>,       // capped at 50, with `more: u32`
    pub model: Option<String>, pub cost_usd: Option<f64>,
}

pub struct DreamNight { pub date: NaiveDate, pub cycle_id: Option<String>, pub memories: u32, pub core: bool, pub state: Option<DreamCycleState> }
pub struct DreamNights { pub nights: Vec<DreamNight>, pub unattributed: u32 }
```

Derivations:
- **Change kind**, per memory revision: `deleted` → Removed; no earlier revision of the same `(bundle, path)`, or the earlier one deleted → Wrote; otherwise → Revised. Any CORE revision → Core.
- **Attribution**: by `dream_cycle_id`. For legacy rows (`trigger='dream' AND dream_cycle_id IS NULL`), by the cycle's `[first journal ts − 1 min, last journal ts + 1 min]` window. Legacy rows in no window count toward `unattributed`.
- **Night**: the person's local date (`tz` query) of `started_at`. More than one cycle on a night (e.g. "Dream now") is summed into one `DreamNight`, and `cycle_id` is the latest.
- **State**: `running` while `DreamStatus` is extracting or consolidating for that cycle; `failed` when the cycle has extraction entries but no consolidation entry and isn't running; otherwise `complete`.
- **`memories`** = written + revised + removed.

### 2.5 Recalled passages (FR-018e)

```rust
pub enum RecallOrigin { Auto, Search }
pub struct RecalledPassage { pub bundle: String, pub path: String, pub ordinal: u32, pub score: f32, pub title: Option<String> }

// SessionHistoryContent
Recall { origin: RecallOrigin, passages: Vec<RecalledPassage> }
```

Rules:
- At most one `Recall { Auto }` per turn (before the first model call), and at most one `Recall { Search }` per round.
- Duplicate `(bundle, path, ordinal)` within one entry keep the highest score.
- `history_entries_to_messages` skips `Recall`. Dream and task sessions record recalls too (harmless). They're shown only where the WebUI shows that session.

### 2.6 Session files (FR-018f)

```rust
pub struct SessionFileRecord {        // existing, + two fields
    pub id, session_slug, agent_id, filename, mime_type, size, file_id, added_at,
    #[serde(default)] pub sender: Option<String>,      // person username, or the agent id
    #[serde(default)] pub history_uid: Option<String>, // the request/response entry it came with
}
// SessionFrame::File(SessionFileRecord)  → socket frame "session_file"
```

### 2.7 Conversation listing (FR-020)

```rust
pub struct ConversationSummary {
    pub channel: String,              // channel slug (the conversation)
    pub kind: ConversationKind,       // webui | discord | telegram
    pub title: String,                // latest topic title, or channel name
    pub place: Option<String>,        // "Vizier Lab", "DM · ana", "topic 3 of 5"
    pub topics: usize,
    pub last_activity: DateTime<Utc>,
    pub latest_topic: Option<String>,
}
pub struct ConversationPage { pub items: Vec<ConversationSummary>, pub counts: BTreeMap<String, usize>, pub total: usize, pub next: Option<String> }
```

Only person channels (`!is_non_user_channel`), and only for people who can view the agent. A WebUI person sees only their own WebUI channel's topics (same rule as today's `list_topics`).

### 2.8 Status (FR-023) and stale saves (FR-025)

```rust
pub struct ServerStatus { pub online: bool, pub version: String, pub setup_required: bool }

UpdateCoreRequest   { content, #[serde(default)] base_seq: Option<i64> }
UpdateSkillRequest  { …, #[serde(default)] base_hash: Option<String> }
// 409 { code: "stale_revision", latest_seq, content } / { code: "stale_content", content }
```

## 3. WebUI client state

Stores (Zustand, `webui/app/stores/`). Each owns one concern and is filled only by `app/api/`:

| Store | Holds | Fed by |
|-------|-------|--------|
| `session` | token, current user, permissions, theme | `/auth/*`, localStorage (try/catch) |
| `agents` | visible agents, health, setup progress | `/agents`, `/agents/health`, `/utils/setup-state` (polled only while some agent is starting) |
| `connection` | one WebSocket per open topic, state (`open \| reconnecting \| offline`), declared client tools | `…/topic/{topic}/chat` socket |
| `turns` | per topic: history entries merged by uid, the live turn, queued messages, jobs, recalls, files, context use | history endpoint + socket frames (reducer in `lib/frames.ts`) |
| `drafts` | unsaved CORE/skill/memory text per document | localStorage (try/catch) |
| `toasts` | notifications | any page |

The turn reducer is a pure function `(TopicState, Frame) → TopicState`, unit-tested. It keeps the spec 023 rule: the running trail is built from `history_entry` frames merged by uid, not from `thinking`/`tool_choice` frames.

State transitions the WebUI draws (Chat states sheet):

```
Idle → Composing → Sent → Thinking → Working → Answered
                     │                  │
                     ├─ send while busy → Queued → (turn ends) → Sent
                     ├─ Stop / /abort ──→ Aborted (queue dropped) → Idle
                     ├─ error frame ────→ Error(kind) → Idle (with action)
                     └─ socket closed ──→ Offline → (reconnect) → resume from history
Background report arrives (idle topic) → Woken → Thinking …
/checkpoint, /lobotomy, auto at threshold → Checkpoint divider
```
