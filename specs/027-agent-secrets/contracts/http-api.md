# Contract: HTTP API

**Feature**: `027-agent-secrets`

All routes are under `/api/v1` and require the existing bearer authentication. Errors keep the existing envelope:

```json
{ "status": 403, "message": "…", "data": { "error": "forbidden", "…": "…" } }
```

A secret manager is a system role, a holder of `all_agents:secret`, or a holder of `owned_agents:secret` who owns the agent. There is no read-only secret permission.

## Routes

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/agents/{agent_id}/secrets` | List names, masks and last change |
| `PUT` | `/agents/{agent_id}/secrets/{name}` | Create or replace one value |
| `DELETE` | `/agents/{agent_id}/secrets/{name}` | Delete one value |
| `GET` | `/agents/{agent_id}/secrets/events?limit=20` | Change log, newest first |

### `GET /agents/{agent_id}/secrets`

`200`:

```json
{
  "items": [
    {
      "name": "LINEAR_TOKEN",
      "masked": "••••7c1e",
      "created_at": "2026-10-24T09:12:00Z",
      "updated_at": "2026-10-24T10:00:00Z",
      "updated_by": { "agent": "dp-a" },
      "updated_by_name": "Archivist"
    }
  ]
}
```

- `items` is sorted by name.
- `masked` is computed server-side by `utils::mask_secret`; the raw value never enters the response.
- `updated_by_name` resolves the username or agent name. If it cannot be resolved, the id is shown.
- An agent with no secrets returns `{ "items": [] }`.

### `PUT /agents/{agent_id}/secrets/{name}`

Request:

```json
{ "value": "lin_api_…" }
```

`201` when created, `200` when replaced:

```json
{
  "status": "created",
  "secret": {
    "name": "LINEAR_TOKEN",
    "masked": "••••7c1e",
    "created_at": "2026-10-24T09:12:00Z",
    "updated_at": "2026-10-24T09:12:00Z",
    "updated_by": { "user": "user-1" },
    "updated_by_name": "alice"
  }
}
```

`status` is `"created"` or `"replaced"`. The name comes from the path, so the body carries only the value. Replacing preserves `created_at`; it changes `updated_at` and `updated_by`.

### `DELETE /agents/{agent_id}/secrets/{name}`

`200`:

```json
{ "name": "LINEAR_TOKEN", "deleted": true }
```

The row and a `deleted` event are written in one transaction.

### `GET /agents/{agent_id}/secrets/events?limit=20`

`limit` defaults to 20 and is capped at 100. `200`:

```json
{
  "items": [
    {
      "id": "8b1e…",
      "name": "LINEAR_TOKEN",
      "action": "replaced",
      "actor": { "user": "user-1" },
      "actor_name": "alice",
      "at": "2026-10-24T10:00:00Z"
    }
  ]
}
```

`action` is `created`, `replaced` or `deleted`. The response contains no value, mask or hash.

## Status codes

| Case | Status | `data.error` |
|---|---|---|
| No/invalid token | `401` | existing auth envelope |
| Agent does not exist | `404` | `not_found` |
| Caller is not a secret manager | `403` | `forbidden` with `required: ["all_agents:secret", "owned_agents:secret (own agents)"]` |
| Delete a name that does not exist | `404` | `not_found` |
| Name/value fails validation, or body is malformed | `422` | `invalid` |
| Storage failure | `500` | `error` |

Permission is checked before the operation; hiding the page is not the protection. Revoking a permission takes effect on the next request because the role is read from the authenticated request, not cached in the page.

## OpenAPI

Add the four handler functions to the `paths(...)` list and their response/view types to `components(schemas(...))` in `src/channels/http/mod.rs`. Regenerate `webui/openapi.json` and `webui/app/api/schema.d.ts` with `node scripts/openapi.mjs` against a running server, then `npm run gen:api` (the script does both).

## Explicit non-goals

- No endpoint returns a value.
- No PUT/GET-by-name that reveals a value.
- No share, transfer, export, version or expiry route.
- No route on another agent's store: the `agent_id` in the path is the only store addressed.
