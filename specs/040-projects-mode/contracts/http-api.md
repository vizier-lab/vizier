# Contract: Project HTTP API

**Feature**: `040-projects-mode` | **Base path**: `/api/v1/agents/{agent_id}/projects`

All routes are authenticated (`require_auth`) and use the existing
`APIResponse` envelope:

```json
{ "status": 200, "message": "…", "data": { } }
```

Errors use the existing envelope with `status`, `message`, and an optional
`code`/`data`. Project authorization is evaluated in `service/projects.rs`;
handlers only translate.

---

## 1. Route table

| Method | Path | Purpose | Who |
|--------|------|---------|-----|
| `GET` | `/` | List non-removed projects of the agent | any permitted viewer of the agent |
| `POST` | `/` | Open an existing folder as a project | desktop principal with project permission |
| `GET` | `/{project_id}` | Project detail | any permitted viewer of the agent |
| `PATCH` | `/{project_id}` | Rename and/or toggle the shell switch | project owner or `all_agents:projects` |
| `DELETE` | `/{project_id}` | Soft-remove the project | project owner or `all_agents:projects` |
| `GET` | `/{project_id}/sessions` | List the caller's sessions | any permitted viewer of the agent |
| `POST` | `/{project_id}/sessions` | Create an empty session | any permitted viewer of the agent |

`channel_id` for the existing chat/history/file routes is
`"project__" + project_id`; no new history or file routes are introduced.

---

## 2. `GET /`

**200**

```json
{
  "status": 200,
  "message": "ok",
  "data": [
    {
      "id": "p3xY9aQ2mZ",
      "agent_id": "dp-a",
      "name": "vizier",
      "folder": "/home/alice/projects/vizier",
      "available": true,
      "shell_enabled": true,
      "owner_id": "u_123",
      "is_owner": true,
      "session_count": 3,
      "created_at": 1793400000000
    }
  ]
}
```

- Excludes rows with `removed_at` set.
- `available` is computed from the filesystem on each request.
- A viewer without a project permission receives `403` and an empty list is
  never returned as a substitute.
- An agent with no projects returns `200` with `[]`; the WebUI then hides the
  section (FR-005).

---

## 3. `POST /` (desktop only)

**Request**

```json
{ "folder": "/home/alice/projects/vizier" }
```

**Behaviour**

1. Refuse non-desktop principals: `403 forbidden`, `code: "desktop_only"` (FR-021).
2. Authorize the agent (`can_view_agent`) and the project permission.
3. Canonicalize and validate the folder (exists, directory, not the workspace
   or inside it) — `422` with `code: "invalid_folder"` or
   `"workspace_folder"`.
4. If `(agent_id, folder)` exists:
   - removed → clear `removed_at`, return `200` with the same id (sessions
     restored, FR-025/SC-006);
   - open → return `200` (no duplicate, FR-003).
5. Otherwise insert and return `201`.

**201 / 200** — a `ProjectSummary`, as in §2.

**Errors**

| Status | Code | Cause |
|--------|------|-------|
| `403` | `desktop_only` | Called from the browser / API key |
| `403` | — | Missing project permission or agent not visible |
| `404` | — | Unknown agent |
| `422` | `invalid_folder` | Missing, not a directory, empty path |
| `422` | `workspace_folder` | The app workspace or one of its descendants |
| `409` | — | A concurrent open won the unique constraint; the service re-reads and returns it as `200` |

The name defaults to the folder's final component; no name is accepted at
creation (rename afterwards).

---

## 4. `GET /{project_id}`

**200** — `ProjectDetail`: the summary plus

```json
{
  "instructions_present": true,
  "instructions_truncated": false
}
```

The instruction text is never returned over HTTP; only the agent prompt sees
it. A removed project is `404` for ordinary reads. A malformed id is `404`.

---

## 5. `PATCH /{project_id}`

**Request** (at least one field)

```json
{ "name": "Vizier monorepo", "shell_enabled": false }
```

- `name`: trimmed, 1–120 characters, otherwise `422 invalid_name`.
- `shell_enabled`: takes effect on the next turn; no agent restart.
- Authorization: project `owner_id == caller.user_id` **or**
  `all_agents:projects`; otherwise `403` (FR-022).
- **200** — updated `ProjectSummary`.

---

## 6. `DELETE /{project_id}`

- Authorization: owner or `all_agents:projects`; otherwise `403`.
- If any `session_turn` row exists for the project channel (any owner, any
  topic) and status is `queued`/`running`: `409`, `code: "turn_active"`, and
  nothing changes.
- Otherwise set `removed_at`; **no file in the folder and no history row is
  deleted** (FR-025).
- **200** `{ "removed": true }`.

Reopening the same folder returns the same id and the previous sessions
(reachable again by the same channel).

---

## 7. `GET /{project_id}/sessions`

**200**

```json
{
  "status": 200,
  "message": "ok",
  "data": [
    {
      "topic": "V1StGXR8_Z",
      "title": "Investigate the parser",
      "is_thinking": false,
      "last_activity": 1793400500000
    }
  ]
}
```

- Only the caller's sessions: the query uses the caller's
  `HTTP(username, project__<id>)` channel, so another person's topics are
  unreachable (FR-024).
- Newest activity first; a brand-new session with no history sorts last with
  `last_activity: null`.
- A viewer with the permission but not the owner sees the project row and
  `session_count`, but an empty list here — the project's other sessions are
  not theirs.

---

## 8. `POST /{project_id}/sessions`

**201**

```json
{
  "topic": "V1StGXR8_Z",
  "title": "New session",
  "channel_id": "project__p3xY9aQ2mZ"
}
```

- Generates the topic server-side (10-character nanoid); the client does not
  supply it.
- Writes `session_detail` with title `"New session"` so the session is listed
  before its first message.
- Allowed while the folder is unavailable (history remains readable; commands
  are refused later).
- `404` if the project is unknown/removed for this agent.

---

## 9. Project channel use of existing routes

No new route is needed for chat or history. The client uses:

| Existing route | Channel id |
|----------------|------------|
| `GET /agents/{a}/channel/{c}/topic/{t}/history` | `project__<id>` |
| `GET /agents/{a}/channel/{c}/topic/{t}/files` | `project__<id>` |
| `GET /agents/{a}/channel/{c}/topic/{t}/detail` | `project__<id>` |
| `DELETE /agents/{a}/channel/{c}/topic/{t}` | `project__<id>` |
| `GET /agents/{a}/channel/{c}/topic/{t}/chat` (WebSocket) | `project__<id>` |

Each of these gains the project check described in
[session-and-permissions.md](./session-and-permissions.md): the caller must be
permitted, the project must belong to the agent and be non-removed, and the
channel username remains the caller's.

---

## 10. OpenAPI and conformance

- Add the seven operations to `ApiDoc` paths and the DTOs to
  `components.schemas` in `crates/vizier-http/src/lib.rs`.
- Add `crates/vizier-http/tests/conformance/groups/projects.rs` with a vector
  per route (happy path, `403`, `404`, `422`, `409`). `pending.txt` may only
  shrink.
- Regenerate `webui/openapi.json` and `webui/app/api/schema.d.ts` with
  `just openapi` (do not hand-edit the generated file).
