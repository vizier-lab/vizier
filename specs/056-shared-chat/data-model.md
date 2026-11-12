# Data Model: Shared Chat Snapshots

Spec: [spec.md](./spec.md) · Decisions: [research.md](./research.md) R4, R10, R11, R12.

Nothing here alters an existing table (a rule of this plan, checked by T014; the spec's FR-051 and SC-010 depend on shares being self-contained). Both tables are created with `CREATE TABLE IF NOT EXISTS` (SQLite, `init_share_schema`) and by migration `0007_shared_snapshot.sql` (PostgreSQL). Times are Unix milliseconds (`crate::time::Timestamp`), as everywhere else.

## Table `shared_snapshot`

| Column | Type | Notes |
|---|---|---|
| `share_id` | TEXT PK | 21-char URL-safe nanoid. Validated `^[A-Za-z0-9_-]{21}$` before any query |
| `creator_id` | TEXT NOT NULL | user id of the author. No foreign key (users are soft-deleted elsewhere); cleanup is explicit (below) |
| `agent_id` | TEXT NOT NULL | the agent the conversation was with; kept for listing and for a later import |
| `topic_id` | TEXT NOT NULL | the source topic; informational only, never dereferenced when serving |
| `title` | TEXT NOT NULL | ≤ 120 chars, plain text |
| `audience` | TEXT NOT NULL | `specific` \| `signed_in` \| `public` |
| `include_trail` | INTEGER NOT NULL | 0/1 |
| `format_version` | INTEGER NOT NULL | 1 |
| `size_bytes` | INTEGER NOT NULL | length of `snapshot_json` in bytes at creation (kept after revocation, for the list; the FR-047 total counts live shares only, DEFERRED T096) |
| `created_at` | INTEGER NOT NULL | |
| `revoked_at` | INTEGER NULL | non-null = revoked. Revoking also sets `snapshot_json` to the empty string (FR-039): the row stays as a **tombstone** so the id keeps answering like an unknown one and the author's list can say "revoked" |
| `revoked_by` | TEXT NULL | author or admin user id |
| `removals_json` | TEXT NOT NULL | the removal report (counts by kind), shown in the author's list |
| `snapshot_json` | TEXT NOT NULL | the document of `contracts/snapshot-format.md`, stored as received (bytes the person previewed); `''` after revocation |

Indexes: `(creator_id, created_at DESC)` for "my shares"; `(created_at DESC)` for the admin list. `snapshot_json` is never selected by list queries.

## Table `shared_snapshot_audience`

| Column | Type | Notes |
|---|---|---|
| `share_id` | TEXT NOT NULL | `REFERENCES shared_snapshot(share_id) ON DELETE CASCADE` |
| `user_id` | TEXT NOT NULL | a named person |
| PK | `(share_id, user_id)` | |

Index: `(user_id)` (for a later "shared with me" list). Rows exist only when `audience = 'specific'`; the service refuses an empty list and more than 50 people.

## Rust types (`vizier-core`)

- `Audience` enum `{ Specific, SignedIn, Public }` with **one** method `allows(&self, viewer: Option<&Principal>, named: &[String]) -> Access` where `Access ∈ {Allowed, NeedSignIn, Denied}`. Nothing else branches on the variant (constitution II); cache policy comes from `Audience::cache_policy()`.
- `ShareRecord` (everything but the content) and `ShareContent { record, snapshot_json: String }`.
- `ShareStorage` trait: `insert_share`, `get_share` (record + content + named list), `list_shares_by_creator`, `list_all_shares` (metadata, paged), `revoke_share(share_id, by, at) -> bool` (conditional update, idempotent), `count_active_by_creator`, `delete_shares_by_creator`.

## Lifecycle

```
(none) ── create ──▶ active ── revoke (text deleted) ──▶ tombstone ── purge (later) ──▶ (row deleted)
```

- **create**: one transaction inserts the row and the audience rows. A primary-key clash is retried once with a new id.
- **revoke** (FR-039): `UPDATE … SET revoked_at = ?, revoked_by = ?, snapshot_json = '' WHERE share_id = ? AND revoked_at IS NULL`: the snapshot body is deleted for good and the row remains as a tombstone; a second call changes nothing and still answers success to the author (idempotent, so a double click is harmless). The audience rows of a revoked share are no longer consulted.
- **serve**: only `revoked_at IS NULL`.
- **conversation deleted**: nothing happens (the snapshot is a copy: FR-005, User Story 5 scenario 8, SC-010).
- **author's account deleted**: the user-deletion service calls `delete_shares_by_creator` and removes the person from the audiences of other people's shares (FR-045). Losing the permission to share, or access to the agent, changes nothing (FR-045). Accounts have no disabled state yet.
- **purge of revoked rows**: not in this version. A tombstone costs a few hundred bytes (the content is gone) and is kept so the author's list can say "revoked"; the 200-active cap does not count revoked ones. A retention purge can be added with the notification purge pattern when there is a reason.

## Concurrency

Two shares of one conversation, or a double click, are independent inserts with independent ids; there is no uniqueness on `(creator_id, topic_id)` (spec edge case "Two simultaneous Share clicks"; FR-050 is the client's single-flight guard). The per-person active cap is checked inside the insert transaction so two parallel creates cannot both slip past it.

## Not stored

No conversation id lookup on read, no history rows, no file ids, no memory, no secrets: the snapshot is self-contained.
