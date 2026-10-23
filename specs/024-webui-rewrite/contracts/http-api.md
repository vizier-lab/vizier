# Contract: HTTP API additions and changes

All paths are under `/api/v1`. All are JSON, in the existing `APIResponse<T>` envelope (`{ data }` / `{ error }`). Each new route carries `#[utoipa::path]`, so it appears in `/openapi.json`, which the WebUI's types are generated from. Access checks use the existing helpers (`user_can_view_agent`, `user_can_edit_agent`, `integrations/access.rs`). Nothing listed here changes a response for a caller that doesn't send the new parameters.

## New

### `GET /status`: public

```json
{ "online": true, "version": "0.14.0", "setup_required": false }
```

### `GET /overview?tz=+07:00`

The signed-in person's agents only.

```json
{ "agents": { "total": 48, "live": 2, "failed": 1 }, "cost_today_usd": 4.12, "accounting_since": "2026-09-12T00:00:00Z" }
```

### `GET /overview/activity?rank=most_active|mine|live|failed`

```json
{
  "hours": ["2026-10-22T15:00:00Z", "…24 entries…"],
  "rows": [ { "agent_id": "archivist", "label": "Archivist", "turns": [0,0,3,…], "live": true } ],
  "all": { "agent_id": null, "label": "All 48", "turns": [4,2,…], "live": true },
  "agent_total": 48,
  "today_by_kind": null
}
```

- `rows` holds at most 6 entries: the top 5 when `all` is present, otherwise every agent up to 6.
- `all` is present only when `agent_total ≥ 7`.
- `today_by_kind` is present only when `agent_total == 1`: `{ "chat": 94, "scheduled": 5, "dream_stages": 2, "background": 0 }`.

### `GET /overview/feed?before=<cursor>&limit=20`

```json
{
  "since": "2026-10-23T03:00:00Z",
  "events": [
    { "id": "health:scribe", "at": "…", "agent_id": "scribe", "kind": "start_failed", "tone": "failed",
      "text": "Failed to start: model \"llama3.3:70b\" not found. Retrying in 40s.", "href": "/scribe/config" },
    { "id": "task_run:812", "at": "…", "agent_id": "treasurer", "kind": "task_run", "tone": "default",
      "text": "Task \"Morning cost digest\" answered.", "href": "/treasurer/tasks?task=morning-cost-digest&run=812" }
  ],
  "next": "eyJhdCI6…"
}
```

- `limit` is 1–50, default 20.
- `start_failed` events appear only on the first page (no `before`).

### `GET /agents/{id}/conversations?q=&channel=webui|discord|telegram&cursor=&limit=25`

```json
{
  "items": [ { "channel": "http__ganesha", "kind": "webui", "title": "vector-db-review", "place": "topic 3 of 5",
               "topics": 5, "last_activity": "…", "latest_topic": "8f2…" } ],
  "counts": { "all": 600, "webui": 38, "discord": 412, "telegram": 150 },
  "total": 600,
  "next": null
}
```

`q` matches titles, and message text within the agent's latest 5,000 messages (this bound is stated in the OpenAPI description).

### `GET /agents/{id}/dream/nights?days=30&tz=+07:00`

```json
{ "nights": [ { "date": "2026-10-19", "cycle_id": "…", "memories": 2, "core": false, "state": "complete" },
              { "date": "2026-10-20", "cycle_id": null, "memories": 0, "core": false, "state": null } ],
  "unattributed": 0 }
```

`days` is 1–90. There is always one entry per night, oldest first.

### `GET /agents/{id}/dream/cycles?before=<cursor>&limit=20`

```json
{ "cycles": [ {
    "cycle_id": "…", "started_at": "…", "ended_at": "…", "state": "complete",
    "sessions_read": 6, "sources": ["vector-db-review", "#releases", "Telegram DM"],
    "written": 1, "revised": 2, "removed": 0, "core": true,
    "changes": [ { "kind": "wrote", "bundle": "infra", "path": "vector-dbs/summary.md", "seq": 1, "at": "…" },
                 { "kind": "core", "bundle": null, "path": null, "seq": 41, "at": "…" } ],
    "more": 0, "model": "anthropic/claude-haiku-5-5", "cost_usd": 0.04 } ],
  "next": null }
```

`cost_usd` is from `usage_event` rows whose topic is the cycle id.

### `GET /agents/{id}/channel/{channel}/topic/{topic}/files?offset=0&limit=5`

```json
{ "items": [ { "id": "…", "filename": "p99-results.csv", "mime_type": "text/csv", "size": 4096, "file_id": "…",
               "added_at": "…", "sender": "archivist", "history_uid": "…" } ],
  "total": 2, "counts": {} }
```

## Changed

### List endpoints: opt-in paging (`ListQuery` → `Page<T>`)

When the request has `limit`, these routes return `{ items, total, counts }` and honour `q`, `filter`, `sort`, `order` and `offset`. Without `limit`, they return exactly what they return today.

| Route | Filters | Sorts |
|-------|---------|-------|
| `GET /agents` | `mine`, `live`, `failed` | `last_active`, `name` |
| `GET /agents/{id}/tasks` | `active`, `last_run_failed` | `next_run`, `name`, `last_run` |
| `GET /skills`, `GET /agents/{id}/skills` | `agent`, `global` (merged list on the agent route with `scope=all`) | `name`, `updated` |
| `GET /auth/users` | `role:<name>` | `created`, `username` |
| `GET /auth/roles` | `builtin`, `custom` | `people`, `name` |
| `GET /agents/{id}/integrations?family=mcp` | `down` | `name`, `tools` |
| `GET /auth/api-keys` | — | `created`, `last_used` |
| `GET /agents/{id}/memory/bundles` | — | `documents`, `updated`, `links` |

Errors: `400 unknown_filter`. `limit` is clamped to 1–100.

The task list items gain `last_run: { id, ran_at, outcome } | null`, needed for the "LATEST RUN" column and the `last_run_failed` filter. The field is added to both shapes, which is additive.

### `PUT /agents/{id}/core`

The body gains an optional `base_seq`. A mismatch returns `409`:

```json
{ "error": { "code": "stale_revision", "message": "CORE changed since you opened it", "latest_seq": 42, "content": "…" } }
```

### `PUT /skills/{slug}`, `PUT /agents/{id}/skills/{slug}`

The body gains an optional `base_hash` (sha256 hex of the loaded content). A mismatch returns `409 stale_content` with the current `content`.

### `GET /files/{file_id}`

Honours a single `Range: bytes=a-b` request header and answers `206 Partial Content` with `Content-Range`. Without the header, the behaviour is unchanged.

### History entries (`GET …/topic/{topic}/history`, `history_entry` frames)

Entries may have `content_type: "recall"`:

```json
{ "uid": "…", "content": { "type": "recall", "origin": "auto",
  "passages": [ { "bundle": "infra", "path": "qdrant.md", "ordinal": 3, "score": 0.82, "title": "Qdrant" } ] } }
```

Older clients that switch on known content types ignore it, as they already do for unknown types.

## Unchanged, newly used by the WebUI

`GET /utils/setup-state` (start progress, R15) · `GET /agents/health` (failure, retry) · reply `stats.current_context_size`/`context_window` (context ring, R14) · `GET /agents/{id}/sharing` · `/auth/api-keys` · `/auth/roles` · `/auth/roles/available-permissions` · `/agents/{id}/dream/{trigger,status,journal}` · `/agents/{id}/core/history` · memory revision history · `/agents/{id}/integrations/{grants,events}`.
