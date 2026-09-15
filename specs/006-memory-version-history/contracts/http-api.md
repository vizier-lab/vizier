# Contract: HTTP API — document history, diff, rollback

All routes live under the existing `/api/v1/agents/{agent_id}` prefix, require the existing auth middleware, and apply `user_can_view_agent` (same rule as reading the document — FR-014). Responses use the existing `APIResponse<T>` envelope (`api_response` / `err_response`). All new handlers are annotated with `#[utoipa::path]` like their neighbors.

## CORE (`src/channels/http/api/v1/agents/core.rs`, nested at `/{agent_id}/core`)

| Method | Path | Query / Body | Purpose |
|--------|------|--------------|---------|
| `PUT` | `/core` | `{ "content" }` | **(existing, modified)** now records a revision with `origin = from_user(user)` |
| `GET` | `/core/history` | `?offset=0&limit=50` | Paginated list, newest-first → `PaginatedCoreRevisions` |
| `GET` | `/core/history` | `?seq=N` | One version with content → `CoreRevision` |
| `GET` | `/core/history` | `?to=B` or `?from=A&to=B` | Line diff → `RevisionDiff`. Semantics: "changes introduced by `to` relative to `from`". Only `to` ⇒ `from = to - 1` (diff vs previous, FR-008). Both ⇒ any pair (FR-009). |
| `POST` | `/core/history` | `{ "seq": N }` | Restore version `seq` → `RollbackResponse` |

One `GET` handler dispatches on which query params are present (`seq` ⇒ one; `to` ⇒ diff; otherwise list).

## Memory (`src/channels/http/api/v1/agents/memory.rs`, nested at `/{agent_id}/memory`)

History routes address a document as `/history/{bundle}/{*path}` (the `*path` capture allows nested concept paths, like the existing `/doc/{bundle}/{*path}` routes). Because `*path` is greedy, the sub-operation is selected by query params / request body rather than by extra path segments — so a concept path that happens to contain `at`, `diff`, etc. can never be misparsed.

| Method | Path | Query / Body | Response |
|--------|------|--------------|----------|
| `POST` | `/memory` | (existing body) | **(existing, modified)** records revision, `origin = from_user(user)` |
| `PUT` | `/memory/{slug}`, `/memory/doc/{bundle}/{*path}` | (existing body) | **(existing, modified)** same |
| `DELETE` | `/memory/{slug}`, `/memory/doc/{bundle}/{*path}` | — | **(existing, modified)** records a deletion entry |
| `DELETE` | `/memory/bundles/{bundle}` | `?force=true` | **(existing, modified)** records one deletion entry per concept |
| `POST` | `/memory/bundles/import` | (existing multipart) | **(existing, modified)** records `trigger = import` per imported concept |
| `GET` | `/memory/history/{bundle}/{*path}` | `?offset&limit` | `PaginatedMemoryRevisions` |
| `GET` | `/memory/history/{bundle}/{*path}` | `?seq=N` | `MemoryRevision` (includes parsed `title`/`tags`) |
| `GET` | `/memory/history/{bundle}/{*path}` | `?to=B` or `?from=A&to=B` | `RevisionDiff` (same semantics as CORE) |
| `POST` | `/memory/history/{bundle}/{*path}` | `{ "seq": N }` | `RollbackResponse` |

The memory `GET`/`POST` history handlers forward to the agent's memory-ops channel (`MemoryOpRequest::{ListRevisions, GetRevision, DiffRevisions, Rollback}`), matching every other memory route in the file (research Decision 6). The CORE handlers call `state.storage.{list_core_revisions, get_core_revision, diff_core_revisions, rollback_core}` directly, like `get_core`/`update_core` do today.

## Response shapes (JSON)

```jsonc
// PaginatedCoreRevisions (CORE rows have no `deleted` field)
{ "revisions": [
    { "seq": 7, "is_current": true, "size_bytes": 1832,
      "actor": { "type": "user", "user_id": "u_1", "username": "alice" },
      "trigger": { "type": "webui" },
      "created_at": "2026-09-15T10:22:01Z" },
    { "seq": 6, "is_current": false, "size_bytes": 1790,
      "actor": { "type": "agent" }, "trigger": { "type": "dream" }, "created_at": "..." },
    { "seq": 5, "is_current": false, "size_bytes": 1700,
      "actor": { "type": "user", "user_id": "u_1", "username": "alice" },
      "trigger": { "type": "rollback", "restored_from": 2 }, "created_at": "..." }
  ],
  "total": 7, "offset": 0, "limit": 50 }

// CoreRevision
{ "seq": 7, "is_current": true, "size_bytes": 1832, "actor": {...}, "trigger": {...}, "created_at": "...",
  "content": "# CORE\n..." }

// PaginatedMemoryRevisions — same shape, each row additionally has "deleted": bool

// MemoryRevision
{ "seq": 6, "deleted": false, "is_current": false, "size_bytes": 1790,
  "actor": { "type": "agent" }, "trigger": { "type": "conversation" }, "created_at": "...",
  "content": "---\ntitle: Bred\ntags:\n- friend\nattachments: []\n---\nBred likes ...",
  "title": "Bred", "tags": ["friend"] }

// MemoryRevision (deletion entry)
{ "seq": 8, "deleted": true, "content": null, "title": null, "tags": [], ... }

// RevisionDiff
{ "from_seq": 6, "to_seq": 7, "additions": 2, "deletions": 1,
  "hunks": [ { "old_start": 3, "old_lines": 4, "new_start": 3, "new_lines": 5,
               "lines": [
                 { "op": "equal",  "old_line": 3, "new_line": 3, "text": "tags:" },
                 { "op": "delete", "old_line": 4, "new_line": null, "text": "- friend" },
                 { "op": "insert", "old_line": null, "new_line": 4, "text": "- best-friend" },
                 { "op": "insert", "old_line": null, "new_line": 5, "text": "- runner" } ] } ] }

// RollbackResponse
{ "no_change": false, "new_seq": 9, "restored_from": 6 }
{ "no_change": true,  "new_seq": null, "restored_from": 7 }
```

## Status codes

| Situation | Status |
|-----------|--------|
| Agent not found / user cannot view agent | 404 / 403 (existing helpers) |
| Document has no history and does not currently exist | 404 on list/get/diff/rollback |
| Document exists but has no history yet | list ⇒ 200 with the lazily-created baseline (seq 1) |
| Unknown `seq` (`?seq`, `from`, `to`, rollback body) | 404 |
| Rollback target is a deletion entry | 400 |
| `limit` > 200 | clamped, 200 |
| Rollback succeeded / no-op | 200 (`no_change` distinguishes) |
| Storage error | 500 |

## Provenance mapping in handlers

| Handler | `origin` |
|---------|----------|
| `PUT /core`, `POST/PUT/DELETE memory` | `RevisionOrigin::from_user(&user)` → actor `user`, trigger `webui` (JWT) / `api` (API key) |
| `POST /memory/bundles/import` | `from_user(&user).with_trigger(Import)` |
| rollback endpoints | `from_user(&user).with_trigger(Rollback { restored_from: seq })` |
