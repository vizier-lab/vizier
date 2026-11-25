# Contract: `GET /api/v1/agents/{agent_id}/link-targets`

The one read endpoint backing both link rendering (broken-state decisions) and composer suggestions. Version: 1 (2026-11-25).

## Request

- `GET /api/v1/agents/{agent_id}/link-targets` — no parameters.
- Auth: `require_auth`. Authorization: load the agent (404 `agent_not_found` if missing), then `can_view_agent(principal, agent)` else `403` — the standard agent-read path (`service/tasks.rs::view_agent` shape), resolved in the service layer, not the handler.

## Response `200`

```jsonc
{
  "artifacts": [ { "name": "report", "updated_at": "…" } ],   // names unique per agent, updated_at desc
  "functions": [ { "name": "fetch_weather" } ],                // all custom tools, disabled included
  "tasks":     [ { "slug": "weekly-report", "title": "Weekly report" } ],  // active and inactive
  "skills":    [ { "slug": "summarize", "scope": "agent" } ],  // scope: "agent" | "global" (workspace scopes only)
  "memory": {
    "bundles": ["research", "people"],                         // complete
    "docs":    [ { "bundle": "research", "path": "topics/energy" } ]  // latest 500
  }
}
```

- Every array may be empty; no field is omitted. Names/slugs are exactly the strings a link target must match (no display-name aliasing).
- Content-type `application/json`; served through the existing axum router mount (`agents/mod.rs`).

## Errors (existing HTTP envelope, `vizier-http/src/error.rs`)

| Status | When |
|---|---|
| 401 | no/invalid token |
| 404 | agent does not exist |
| 403 | principal cannot view the agent |

No other failure mode: the underlying list calls are infallible reads; storage errors surface as the existing `500` envelope.

## Performance & caching semantics

- Names-only payload; composed from the same service calls the existing per-kind list endpoints use. Expected order: tens of KB at worst for pathological agents; typically a few KB.
- The endpoint is **cheap to re-read** but not free: clients cache per agent (Zustand store), refresh on conversation open and before a suggestion pass; no server-side cache, no ETag in v1.
- No pagination in v1: memory docs capped server-side (latest 500), everything else is naturally bounded by the agent's entity counts.

## Compatibility

- Additive route; no existing route or shape changes.
- Feature-flag rules do not apply (core read capability; no flag).
- Future kinds (conversations, agents) extend the response with new arrays — additive, per the grammar contract's versioning rule.
