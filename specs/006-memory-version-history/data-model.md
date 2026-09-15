# Data Model: Version History for CORE.md and Memories

**Feature**: `006-memory-version-history` | **Date**: 2026-09-15

## Entities

### Versioned Document (identity only — no new table)

A document that has a history. Identified by:

| Kind | Identity | Notes |
|------|----------|-------|
| CORE | `agent_id` | One CORE per agent → history in `core_revision` |
| Memory | `agent_id`, `bundle`, `path` | Normalized concept path within the bundle (no `.md`, e.g. `friends/bred`) → history in `memory_revision` |

The document's *current* content still lives where it lives today (`agent_core.content` for CORE; the `DocumentStore` file for memory). History never becomes the source of truth for current content.

### Revisions — two tables, one per document kind (NEW, append-only)

CORE and memory documents are different things — CORE is one-per-agent and can never be deleted; a memory is `(bundle, path)`-addressed and can be deleted/restored — so each gets its own table **and its own implementation**. They happen to share some column names (actor, trigger, timestamps), but no code is shared between them beyond the kind-agnostic value types listed under *Shared value types* below.

#### `core_revision`

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `id` | INTEGER | PRIMARY KEY AUTOINCREMENT | |
| `agent_id` | TEXT | NOT NULL | |
| `seq` | INTEGER | NOT NULL | 1-based per agent; `UNIQUE(agent_id, seq)` |
| `content` | TEXT | NOT NULL | The CORE markdown verbatim |
| `actor_kind` | TEXT | NOT NULL | `'agent'` \| `'user'` \| `'system'` |
| `actor_id` | TEXT | NULL | `user_id` when `actor_kind = 'user'` |
| `actor_name` | TEXT | NULL | `username` snapshot for display |
| `trigger` | TEXT | NOT NULL | `'conversation'` \| `'dream'` \| `'webui'` \| `'api'` \| `'rollback'` \| `'baseline'` (never `'import'`) |
| `restored_from` | INTEGER | NULL | Source `seq` when `trigger = 'rollback'` |
| `created_at` | INTEGER | NOT NULL | Unix millis (UTC) |

Indexes: `idx_core_rev_doc ON core_revision(agent_id, seq DESC)`.

#### `memory_revision`

| Column | Type | Constraints | Notes |
|--------|------|-------------|-------|
| `id` | INTEGER | PRIMARY KEY AUTOINCREMENT | |
| `agent_id` | TEXT | NOT NULL | |
| `bundle` | TEXT | NOT NULL | Bundle name |
| `path` | TEXT | NOT NULL | Normalized concept path (no `.md`, e.g. `friends/bred`) |
| `seq` | INTEGER | NOT NULL | 1-based per `(agent_id, bundle, path)`; `UNIQUE(agent_id, bundle, path, seq)` |
| `content` | TEXT | NULL | Canonical snapshot; `NULL` **only** when `deleted = 1` |
| `deleted` | INTEGER | NOT NULL DEFAULT 0 | 1 = this entry records a deletion |
| `actor_kind` | TEXT | NOT NULL | as above |
| `actor_id` | TEXT | NULL | as above |
| `actor_name` | TEXT | NULL | as above |
| `trigger` | TEXT | NOT NULL | `'conversation'` \| `'dream'` \| `'webui'` \| `'api'` \| `'import'` \| `'rollback'` \| `'baseline'` |
| `restored_from` | INTEGER | NULL | as above |
| `created_at` | INTEGER | NOT NULL | Unix millis (UTC) |

Indexes: `idx_mem_rev_doc ON memory_revision(agent_id, bundle, path, seq DESC)`, `idx_mem_rev_agent ON memory_revision(agent_id)`.

**Canonical `content`**:
- `kind = core`: the CORE markdown verbatim.
- `kind = memory`: `serialize_markdown(RevisionFrontMatter { title, tags, attachments }, body)` — YAML frontmatter with exactly those three keys, then the body. Excludes `created_at`, `updated_at`, `read_count`, `keywords`, `relations`, `slug`, `agent_id`, `bundle` (all derived/bookkeeping or already part of the identity).

**Invariants** (both tables unless noted):
1. `seq` for a document is contiguous starting at 1; new entries always get `max(seq) + 1`. Rows are never updated or deleted except by agent deletion.
2. No two consecutive entries have identical `(content, deleted)` — enforced by `record` (no-op skip, FR-003).
3. `memory_revision` only: a `deleted = 1` entry is never immediately followed by another `deleted = 1` entry. `core_revision` has no deletion entries at all.
4. The first entry for a document that pre-dates the feature is `trigger = 'baseline'`, `actor_kind = 'system'`.
5. `restored_from` (when set) references an existing `seq` of the same document that is strictly less than this row's `seq`.

**State transitions** (per document; the `Deleted` state exists only for memory):

```
(no history) --baseline (lazy)--> Live(seq=1)
Live --save (content differs)--> Live(seq+1)
Live --save (identical)--> Live (no new row)
Live --delete--> Deleted(seq+1, deleted=1, content=NULL)
Deleted --rollback to seq k (content revision)--> Live(seq+1, restored_from=k)
Live --rollback to seq k--> Live(seq+1, restored_from=k)   [skipped if content identical]
any --agent deleted--> (all rows removed)
```

### AuthMethod (small addition to `AuthenticatedUser`)

```rust
pub enum AuthMethod { Jwt, ApiKey }
pub struct AuthenticatedUser { user_id, username, role, permissions, auth_method: AuthMethod }
```

### Shared value types (`src/schema/revision.rs`, `utoipa::ToSchema`)

These describe *a save* and *a diff*, not a document kind, so they are used by both sides unchanged:

```rust
pub struct RevisionOrigin { pub actor: RevisionActor, pub trigger: RevisionTrigger }
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RevisionActor { Agent, User { user_id: String, username: String }, System }
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RevisionTrigger { Conversation, Dream, WebUi, Api, Import, Rollback { restored_from: i64 }, Baseline }
```
Constructors: `from_session(&VizierSession)`, `from_user(&AuthenticatedUser)` (trigger from `auth_method`), `system(trigger)`, `.with_trigger(t)`. Derives `Serialize/Deserialize/Clone/Debug` (travels inside `MemoryOpRequest`).

| Type | Fields |
|------|--------|
| `RevisionDiff` | `from_seq: i64`, `to_seq: i64`, `hunks: Vec<DiffHunk>`, `additions: usize`, `deletions: usize` |
| `DiffHunk` | `old_start: usize`, `old_lines: usize`, `new_start: usize`, `new_lines: usize`, `lines: Vec<DiffLine>` |
| `DiffLine` | `op: "equal" \| "insert" \| "delete"`, `old_line: Option<usize>`, `new_line: Option<usize>`, `text: String` |
| `RollbackResponse` | `no_change: bool`, `new_seq: Option<i64>`, `restored_from: i64` |

### CORE API types (`src/schema/revision.rs`)

| Type | Fields |
|------|--------|
| `CoreRevisionSummary` | `seq: i64`, `actor: RevisionActor`, `trigger: RevisionTrigger`, `created_at: DateTime<Utc>`, `is_current: bool`, `size_bytes: usize` |
| `CoreRevision` | `CoreRevisionSummary` fields + `content: String` |
| `PaginatedCoreRevisions` | `revisions: Vec<CoreRevisionSummary>`, `total`, `offset`, `limit` |

### Memory API types (`src/schema/revision.rs`)

| Type | Fields |
|------|--------|
| `MemoryRevisionSummary` | `seq: i64`, `deleted: bool`, `actor: RevisionActor`, `trigger: RevisionTrigger`, `created_at: DateTime<Utc>`, `is_current: bool`, `size_bytes: usize` |
| `MemoryRevision` | `MemoryRevisionSummary` fields + `content: Option<String>` (canonical text; `None` on a deletion entry), `title: Option<String>`, `tags: Vec<String>` (parsed from the canonical text) |
| `PaginatedMemoryRevisions` | `revisions: Vec<MemoryRevisionSummary>`, `total`, `offset`, `limit` |

`is_current` is true for the highest `seq` (for memory: whether or not it is a deletion entry).

### Memory `RevisionFrontMatter` (internal, `src/storage/revision.rs`)

```rust
#[derive(Serialize, Deserialize)]
struct RevisionFrontMatter { title: String, #[serde(default)] tags: Vec<String>, #[serde(default)] attachments: Vec<VizierAttachment> }
```

Serialized/parsed with the existing `serialize_markdown` / `parse_markdown_bytes` helpers in `memory_bundle.rs` (made `pub(crate)`).

## Relationships

- `core_revision.agent_id` → `agent_core.agent_id`; cascade-deleted in `delete_agent`.
- `memory_revision.agent_id` → agent (`agent_config.agent_id`); cascade-deleted in `delete_agent`.
- `memory_revision (bundle, path)` → the on-disk concept document at `{workspace}/agents/{agent_id}/memory/{bundle}/{path}.md`; loosely coupled (history survives the file's deletion — that's the point).
- `restored_from` → same-document `seq`.

## Validation rules

- `memory_revision::record` rejects an empty `path`. `core_revision::record` has no deletion form at all (its `content` is `&str`, not `Option`).
- `list`: `limit` clamped to `1..=200`, default 50; `offset ≥ 0`.
- `get`/`diff`/`rollback`: unknown `seq` ⇒ 404. `diff` with `from == to` ⇒ 200 with zero hunks. Rollback target with `deleted = 1` ⇒ 400 ("cannot restore a deletion entry; pick a content version").
- Rollback of a memory whose bundle no longer exists recreates the bundle implicitly (existing `write_memory` behavior).

## Not modeled (explicitly)

- No per-version note/message field (not requested; can be added as a nullable column later).
- No history for `index.md`/`log.md`, session files, dream journal, skills.
- No retention/pruning columns.
