# Contract: `GET /api/v1/agents/{agent_id}/link-targets`

Backing both link rendering (broken-state decisions) and composer suggestions. **Version 2** (2026-11-25) — revised after scale review: no kind ships unbounded; the endpoint has three modes (index, search, resolve) on one route family, all names-only, no schema changes.

## Mode 1 — bounded index (no parameters)

`GET /api/v1/agents/{agent_id}/link-targets`

```jsonc
{
  "artifacts": ["report", "draft-v2"],              // latest 200 by updated_at — the unbounded kind is capped
  "functions": ["fetch_weather"],                    // all custom tools, disabled included
  "tasks":     [ { "slug": "weekly-report", "title": "Weekly report" } ],   // active and inactive
  "skills":    [ { "slug": "summarize", "scope": "agent" } ],               // workspace scopes only
  "memory": {
    "bundles": ["research"],                         // complete
    "docs":    [ { "bundle": "research", "path": "topics/energy" } ]  // latest 500
  },
  "truncated": { "artifacts": true, "memory.docs": false }   // true wherever a cap bit
}
```

- `truncated` tells the client which kinds it saw only partially, so it knows to fall back to Mode 2 instead of trusting absence as broken.
- Entry shapes are as slim as possible (artifact names are plain strings; no timestamps on the wire).

## Mode 2 — search (composer fallback)

`GET /api/v1/agents/{agent_id}/link-targets?kind=<kind>&q=<text>&limit=<n≤50>`

- `kind` required: `artifact` | `function` | `skill` | `task` | `memory` (memory searches bundles then docs).
- `q`: case-insensitive substring match on name/slug; `limit` default 20.
- Returns the same per-kind array shape as Mode 1 (plus `truncated: {}`), so one client type consumes all modes.
- This mirrors the existing `@` file-mention pattern (`suggestProjectFiles`): the composer filters its cached index locally first and only calls this when the local result is empty or the kind was truncated — one debounced request per suggestion pass, not per keystroke.

## Mode 3 — batch resolve (renderer)

`POST /api/v1/agents/{agent_id}/link-targets/resolve`

```jsonc
// request  (max 100 links per call; oversized → 422)
{ "links": [ { "kind": "artifact", "name": "report" }, { "kind": "skill", "name": "summarize" } ] }
// response
{ "resolved": { "artifact:report": true, "skill:summarize": false } }
```

- Exact-existence check per link, one request per visible conversation at most. Used for (a) names outside a truncated kind, (b) re-checking stale cache entries before rendering them broken.
- Keys are `kind:name` — the same token the grammar produces.

## Auth & errors (unchanged from v1)

`require_auth`; load agent → `404` if missing → `can_view_agent` else `403`, decided in the service layer. `401` unauthenticated; `422` only for an oversized/malformed resolve body. Storage errors surface as the existing `500` envelope. No pagination beyond the caps; no ETag in v1.

## Scale rationale

| Kind | Bound | Why |
|---|---|---|
| artifacts | 200 + search/resolve | the only unbounded kind (one agent can accumulate thousands); full index at 5k entries ≈ 0.5 MB per conversation open — refused |
| memory.docs | 500 | same reason, capped since v1 |
| functions/tasks/skills/bundles | none needed | naturally tens-to-hundreds per agent |
| resolve | 100 links/call | bounds one message-list render |

## Compatibility

Additive route; no existing route or shape changes; no flag (core read capability). Future kinds extend Mode 1 with new arrays and accept the same `kind` value in Modes 2–3. The grammar contract's reserved-word list and this route's kinds stay in sync (pinned by the `entityLinks.test.ts` fixtures).
