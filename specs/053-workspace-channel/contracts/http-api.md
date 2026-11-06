# Contract: Workspace HTTP API

All under `/api/v1`, behind `require_auth`, with handlers in `crates/vizier-http/src/api/v1/workspaces.rs` that only translate. Operations and authorization live in `vizier_core::service::workspaces`. Every route gets a conformance vector in `tests/conformance/groups/workspaces.rs`; `pending.txt` may only shrink.

## Access

| Who | May |
|---|---|
| Anyone with `workspaces:create` | Create |
| A member | Read, post, stop turns, answer questions, add topics |
| An owner, or `all_workspaces:manage` | Above, plus rename, delete, manage members, delete topics |
| A non-member without the permission | Nothing. **Every route answers `404`**, so existence does not leak (FR-004) |

## Routes

| Method and path | Purpose | Notes |
|---|---|---|
| `GET /workspaces` | The caller's Workspaces | Each: `id, name, role, unread, working`, and `topics` with their own `unread` (drives the rail) |
| `POST /workspaces/{id}/topics/{tid}/read` | Mark a topic read | Any member. Marks its notifications read |
| `POST /workspaces` | Create | `{name, first_topic?}`, default topic `general`. `409 name_in_use`, `422 invalid_name` |
| `GET /workspaces/{id}` | Detail | With members and their states |
| `PATCH /workspaces/{id}` | Rename | Owner |
| `DELETE /workspaces/{id}` | Delete | Owner. Stops turns first |
| `GET /workspaces/{id}/candidates?q=&limit=` | Search to add | Owner. Below |
| `POST /workspaces/{id}/members` | Add | `{kind, handle, role?}`. Below |
| `PATCH /workspaces/{id}/members/{kind}/{handle}` | Change role | Owner |
| `DELETE /workspaces/{id}/members/{kind}/{handle}` | Remove | Owner. `409 last_owner`. Aborts an agent's turns |
| `POST /workspaces/{id}/topics` | Add topic | `{name}`. `409 name_in_use` |
| `PATCH`/`DELETE /workspaces/{id}/topics/{tid}` | Rename, delete | Owner. `409 last_topic` |
| `GET /workspaces/{id}/topics/{tid}/entries?before=&limit=` | Timeline page | Default 50, max 200. Below |
| `POST /workspaces/{id}/topics/{tid}/entries` | Post | `{text, files?}`. Max 32,000 characters. `/<command> @<agent-id>` is parsed here (research R19): `422 not_in_workspace` for `/btw` and `/clear-files`, `403 cannot_lobotomy`. A command answers `{note}` and posts no message |
| `GET /workspaces/{id}/commands` | For the `/` menu | Any member. `{commands: [{name, description}], skills: [{name, description, agents}]}`. Names only, no skill content |
| `POST /workspaces/{id}/topics/{tid}/files` | Upload | Then pass ids to post |
| `POST /workspaces/{id}/topics/{tid}/turns/{agent}/stop` | Stop one | Any member |
| `POST /workspaces/{id}/stop` | Stop all | Any member. Answers `{stopped, dropped}` |
| `POST /workspaces/{id}/topics/{tid}/questions/{set}/answer` | Answer a question | Any person who is a member. Records the answerer |
| `GET /workspaces/{id}/topics/{tid}/stream` | Socket | See `socket-frames.md` |

## Candidates

```json
{
  "agents": { "items": [{ "id": "sentinel", "name": "Sentinel", "member": false, "addable": true, "reason": null }], "more": 4 },
  "people": { "items": [{ "username": "samir", "name": "Samir", "member": false }], "more": 0 }
}
```

- `limit` defaults to 5 per group, max 20. `q` is required and at least 1 character.
- Matches name, username or agent id, case-insensitively.
- Order: addable, then not addable, then members; within each, starts-with before contains.
- An agent the caller may not view is absent. One they may view but not edit has `addable: false, reason: "cannot_edit"`.

## Adding a member

Refusals: `403 cannot_edit_agent` (adding an agent you may not edit), `409 handle_in_use`, `404 not_found`. The warning in FR-005 is client copy, shown before this call. It is not a server step.

## Timeline entry

A page has two lists, so a client merges them by `at`:

```json
{
  "entries": [{
    "seq": 812,
    "kind": "person | agent | note",
    "author": { "kind": "agent", "id": "pathfinder", "name": "Pathfinder", "available": true },
    "body": "text",
    "at": 1793968660043,
    "mentions": [{ "kind": "agent", "handle": "archivist", "state": "woken" }],
    "origin": null,
    "is_final": false,
    "activated_by_seq": 790,
    "history_uid": "...",
    "reactions": [],
    "stats": {}
  }],
  "steps": [{ "agent_id": "pathfinder", "entry": {} }],
  "next_before": 790
}
```

`entries` are ordered by `seq`. `steps` are the `session_history` rows of this topic's sessions in the page's time span, excluding `Request` rows, in the `ChatMessage` shape `groupHistory` reads today. `stats` and `reactions` are filled from the entry's history row when `is_final`.
