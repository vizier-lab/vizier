# Contract: Artifacts HTTP API

**Feature**: `035-add-artifacts`

Base: `/api/v1`. Every route here is behind `require_auth`. Artifact **view** routes require `all_agents:view_artifacts`, or `owned_agents:view_artifacts` and ownership; **manage** routes require `all_agents:manage_artifacts`, or `owned_agents:manage_artifacts` and ownership. Manage implies view; the system role bypasses. These permissions are additive to the agent's other pages and do not grant them. Errors use the existing `APIResponse` envelope (`{ status, message, data }`).

All timestamps are RFC 3339 in JSON. All IDs are strings.

---

## 1. List artifacts

```http
GET /api/v1/agents/{agent_id}/artifacts
```

Query:

| Name | Type | Default | Meaning |
|---|---|---|---|
| `q` | string? | — | Case-insensitive name search. |
| `kind` | `image` \| `pdf` \| `text` \| `markdown` \| `csv` \| `audio` \| `video` \| `other`? | — | Exact kind filter. |
| `from` | `YYYY-MM-DD`? | — | Inclusive lower bound on `updated_at` (UTC start of day). |
| `to` | `YYYY-MM-DD`? | — | Inclusive upper bound on `updated_at` (UTC end of day). |
| `cursor` | string? | — | Opaque cursor from a previous page. |
| `limit` | integer? | 50 | 1–100. |

Response `200`:

```json
{
  "items": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "agent_id": "dp-a",
      "name": "Q3-report.pdf",
      "kind": "pdf",
      "mime_type": "application/pdf",
      "size": 184320,
      "version": 2,
      "created_at": "2026-10-28T09:00:00Z",
      "updated_at": "2026-10-29T10:00:00Z",
      "available": true
    }
  ],
  "next": "eyJhdCI6MTc2MTcyMjQwMDAwMCwiaWQiOiI1NTBlODQwMC4uLiJ9",
  "total": 12,
  "total_bytes": 1843200,
  "agent_total": 20,
  "agent_total_bytes": 4819200
}
```

- `total`/`total_bytes` match the filters, before the cursor; `agent_total`/`agent_total_bytes` ignore filters.
- `next` is `null` on the last page; an invalid cursor is `400 malformed cursor`.
- Order is `updated_at DESC, id DESC`.
- `available` is `false` when the row exists but its content file is missing.
- `403` if the caller lacks artifact view permission for the agent; `404` if the agent does not exist.

---

## 2. Artifact detail

```http
GET /api/v1/agents/{agent_id}/artifacts/{artifact_id}
```

Response `200`:

```json
{
  "artifact": { "...": "the list item fields plus origin/source/creator" },
  "available": true,
  "references": [
    {
      "kind": "message",
      "label": "WebUI conversation",
      "href": "/dp-a/chat/topic-abc",
      "created_at": "2026-10-29T10:00:00Z"
    },
    {
      "kind": "memory",
      "label": "Q3 planning",
      "href": "/dp-a/memory/default/q3-planning",
      "created_at": "2026-10-29T10:05:00Z"
    }
  ],
  "events": [
    {
      "id": "…",
      "action": "replaced",
      "actor": { "user": "user-id" },
      "actor_name": "ana",
      "source_filename": "report-v2.pdf",
      "created_at": "2026-10-29T10:00:00Z"
    }
  ]
}
```

- `origin` is `{ "channel": "http__ana__topic-abc", "topic": "topic-abc", "source_filename": "report.pdf" }` (nullable parts).
- `references` can be empty; each has a display label and a `href` when the destination is a readable conversation/memory.
- `events` are newest first (the activity view is the paged version).
- `404` for a missing artifact, `403` for no artifact view permission.

---

## 3. Download content

```http
GET /api/v1/agents/{agent_id}/artifacts/{artifact_id}/content
```

- Returns the current `content.v<n>` bytes.
- `Content-Type` is the artifact's stored MIME type.
- `Content-Disposition: inline; filename*=UTF-8''<percent-encoded display name>` (a quoted ASCII fallback is optional); the WebUI download uses the artifact's display name from metadata, so a quote/Unicode name does not corrupt the header.
- Supports a single `Range: bytes=…` exactly like `GET /files/{file_id}` (shared range parser).
- `404` when the artifact row is missing; `410` is not used. A row whose content file is missing is `404` with `message: "artifact content is unavailable"`.
- View permission only; no new permission. A person without `view_artifacts` gets `403` even if they can see the surrounding message/memory; the WebUI shows the attachment as unavailable.

---

## 4. Activity

```http
GET /api/v1/agents/{agent_id}/artifacts/activity?cursor=&limit=50
```

Response:

```json
{
  "items": [
    {
      "id": "…",
      "artifact_id": "550e…",
      "artifact_name": "Q3-report.pdf",
      "action": "deleted",
      "actor": { "agent": "dp-a" },
      "actor_name": "Dp A",
      "source_filename": null,
      "created_at": "2026-10-29T11:00:00Z"
    }
  ],
  "next": null
}
```

- Order `created_at DESC, id DESC`; the same opaque `(at, id)` cursor shape.
- Deletion records remain after the artifact row is gone.
- `403`/`404` as above.

---

## 5. Delete artifact

```http
DELETE /api/v1/agents/{agent_id}/artifacts/{artifact_id}
```

- Requires artifact manage permission for the agent (system bypass). Deletes the row, its references, its content directory and writes a `deleted` event. The event remains.
- Before deleting the row, collect the memory references; after the row/refs/event transaction, remove the artifact attachment from each memory (`detach_artifact_from_memories` with a memory revision). Message references remain in history and become unavailable.
- Response `200`:

```json
{ "removed": true, "references": 3, "memory_links_removed": 2 }
```

- `403` when the caller has only view access; `404` when the artifact is already gone.
- A message that still holds the artifact id shows the unavailable/tombstone state; a memory normally has no attachment after the cleanup and records a revision. A failed memory cleanup is logged and leaves a stale link that reads as deleted.

---

## 6. Promote a session file

```http
POST /api/v1/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/files/promote
```

Body:

```json
{ "filename": "report.pdf", "name": "Q3-report.pdf" }
```

- `name` is optional; it defaults to `filename`.
- The route requires artifact **manage** permission for the agent, plus the same conversation access as `GET …/files`: `user_can_view_agent` and the existing `show_<platform>_chats` rule for Discord/Telegram.
- The source must be a session file in that conversation; the bytes are copied into `artifacts/` and the source stays where it is.
- Response `201`: the artifact detail summary (same shape as a list item plus origin/source/creator).
- Errors:
  - `404` agent/conversation/file not found;
  - `409` an artifact with that name already exists (the message points to replacement);
  - `422` invalid name or empty/unreadable source;
  - `403` no manage permission or no conversation access.
- Actor: `Requester::User(user.user_id)`.

---

## 7. Clear a conversation's session files

```http
DELETE /api/v1/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/files
```

- Same conversation visibility as `GET …/files` (agent visibility plus the existing platform-chat rule); artifact permissions do not gate session-file clear.
- Refused while the conversation has a running or queued turn (`409`, message explains it).
- Deletes every `session_file` record in that conversation and each `uploads/<file_id>/` blob that no other session-file record references. Artifacts are untouched.
- Response `200`:

```json
{ "removed": 4, "bytes": 7340032 }
```

- `removed` is the number of session-file records; `bytes` is the sum of their stored sizes (best effort).
- A confirmation dialog is the client's responsibility; the API performs the action on the `DELETE`.
- Legacy messages/memories that referenced the removed files will render as unavailable; the confirmation states this.

---

## 8. Shared response/error rules

- `401` unauthenticated, `403` visible-agent/permission failure, `404` missing agent/artifact/file, `409` conflict/active turn, `422` validation, `500` storage/disk failure.
- No endpoint returns artifact bytes in a JSON body; content is only the content route.
- No endpoint accepts a filesystem path. Promotion names a session file; artifact content paths are resolved from the row.
- The artifact list/detail/activity routes never mutate data. Promotion/replace/delete/clear are the only writes.
- `VizierAttachmentContent::Artifact` in message history is documented in [agent-tools-and-memory.md](./agent-tools-and-memory.md) §6.
