# Contract: HTTP API (`/api/v1/agents/{id}/memory`)

Location: `src/channels/http/api/v1/agents/memory.rs`. Every route below that lists, filters,
sorts, or renders a graph is served from the sqlite-backed Memory Graph Index
(data-model.md, research.md §9) — it does not read concept documents from the `DocumentStore` on
the request path; only a single-document detail fetch or an export does.

**HTTP routes intentionally do *not* mirror the `memory_list`/`memory_graph` tool's "bundle:
None = top-level view" zoom convention (research.md §12) on `GET /` itself.** A tool's `None`
default can be freely redefined because its only caller is an LLM re-reading the tool's
description/`BOOT.md` from context on every turn — there's no compiled call site to break. `GET
/` has real compiled callers (the WebUI frontend, any external API integration) that hard-code
an expected response *shape*; having the same route return a `Memory[]` array in one case and a
`BundleSummary[]` array in another, switching on whether a query param is present, is the kind
of REST anti-pattern (one route, two response types) that's fine for a single flexible tool
schema but not for a fixed HTTP contract. So: `GET /` keeps today's shape and default exactly —
a flat `Memory[]` array, `?bundle=` filtering it down when present, omitted meaning "every
bundle." The **top-level view** is its own dedicated resource, `GET /bundles`, returning
`BundleSummary[]` (data-model.md) — never overloaded onto `GET /`.

## Existing routes — gain bundle scoping

| Route | Change |
|---|---|
| `GET /` (`get_all_memories`) | Accepts optional `?bundle=` query param; omitted = all bundles, flattened — unchanged shape and default from today, just now bundle-aware. Response items include `bundle` and full nested `path` in place of bare `slug`. Drops `visibility` from filters/response. |
| `POST /` (`create_memory`) | Body accepts optional `bundle`; `slug` may be a multi-segment path. Drops `visibility`/`shared_to` from the body. Returns `409`-class error on `(bundle, path)` collision (FR-011) — no silent overwrite. |
| `GET /query` (`query_memories`) | Accepts optional `?bundle=`; omitted = search across all bundles (matches `memory_read`'s default, research.md §12 — search's default is "everywhere," independent of the browse-route decision above). |
| `GET /{slug}`, `PUT /{slug}`, `DELETE /{slug}` (`get_memory_detail`, `update_memory`, `delete_memory`) | Path becomes `/{bundle}/{path}` where `path` may itself contain `/` (nested); `{slug}`-only routes remain as a convenience alias resolving into the default bundle. |
| `GET /{slug}/related` (`get_related_memories`) | Same path-shape change; response can include cross-bundle related memories, each annotated with its `bundle`. |
| `GET /graph` (`get_memory_graph`) | Becomes bundle-scoped: `GET /{bundle}/graph` returns that bundle's concept-level graph (today's shape, unchanged `MemoryGraph`/`MemoryGraphNode`/`MemoryGraphEdge`, plus the new `bundle`/`boundary` node fields — data-model.md). |

## New routes

| Route | Purpose |
|---|---|
| `GET /bundles` | The dedicated top-level view: `BundleSummary[]` (name, concept count, last-updated) for all of the agent's bundles (FR-006). This is what the WebUI's bundle picker calls, and what backs `memory_list`'s top-level view at the tool layer — not `GET /` with an omitted param. |
| `GET /bundles/graph` | Bundle-level graph: nodes = bundles, edges = cross-bundle links between them (FR-019). Same `MemoryGraph` shape as `/{bundle}/graph`, at the `bundle: None` level — both are the same underlying `get_memory_graph` call, bundle filled in or not. |
| `DELETE /bundles/{bundle}` | Deletes an already-empty bundle (its `index.md`/`log.md`). `409`-class error if it still holds any concept document — never a silent/cascading delete of content. Mirrors the `memory_delete_bundle` agent tool. |
| `GET /bundles/{bundle}/export` | Streams a `.zip` containing that bundle's concept documents, index/log documents, and attachments (FR-020). `Content-Type: application/zip`. |
| `POST /bundles/import` (multipart) | Body: a `.zip` file + a destination `bundle` name field. Validates the archive is a well-formed bundle structure before writing anything (Edge Cases: malformed zip rejected atomically). Concept-level collisions against an existing destination bundle are reported and skipped, not overwritten (FR-021) — response is an `ImportReport` listing imported vs. skipped paths. |

## Error contract

- Malformed/non-bundle-shaped `.zip` on import → `400`-class error, **before** any write
  (Edge Cases: reject before partial-importing corrupt data).
- `(bundle, path)` collision on create/import → reported per FR-011/FR-021, not overwritten.
- A bundle name that doesn't exist on a scoped route (`/{bundle}/graph`, `/{bundle}/export`) →
  `404`-class error, not an empty/broken graph silently returned as success.
- `DELETE /bundles/{bundle}` on a bundle that still has concept documents → `409`-class error
  naming how many remain, not a cascading delete.
