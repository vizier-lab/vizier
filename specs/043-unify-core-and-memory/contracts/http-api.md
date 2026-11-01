# Contract: HTTP API

Two rules govern every endpoint here: **nothing existing changes shape**, and **one document backs everything**.

## Deprecated — `/agents/{agent_id}/core` (kept, FR-034 / FR-035)

| method | path | status |
|---|---|---|
| `GET` | `/core` | works, **deprecated** |
| `PUT` | `/core` | works, **deprecated** |
| `GET` | `/core/history` (`offset`/`limit`, `seq`, `from`+`to`) | works, **deprecated** |
| `POST` | `/core/history` (rollback) | works, **deprecated** |

**Contract**

1. **Responses are byte-compatible with today.** `CoreContentResponse { content, seq }`, `CoreUpdateResponse`, `PaginatedCoreRevisions`, `CoreRevision`, `RevisionDiff`, `RollbackResponse` all keep their field names and meanings.
2. `seq` is the `memory_revision` ordinal of `core/soul`, not a `core_revision` ordinal. A client that stores `seq` and sends it back as `base_seq` still gets the stale-revision conflict it expects.
3. `content` is the document body (no frontmatter), matching what `GET /core` returned before.
4. Every one of the four is marked deprecated in its OpenAPI description, naming the memory endpoint that replaces it. The `/swagger` surface is where a consumer learns this.
5. `404 "agent has no CORE"` for an agent with no identity document stays as it is: it now means the document is absent, which is a real state since deletion is allowed.

**What must not happen**: the deprecated surface and the memory endpoints must never disagree. They read and write the same document through the same write path (FR-035, SC-013).

## Extended — `/agents/{agent_id}/memory`

### `POST /memory` and `PUT /memory/doc/{bundle}/{*path}`

The success body gains an optional field. Existing fields are unchanged.

```jsonc
{
  "agent_id": "...", "bundle": "vizier", "path": "workqueue", "title": "...",
  "message": "memory created successfully", "tags": [], "relations": [],
  "warning": "6144 bytes — over the 4096-byte comfortable size. ..."   // NEW, omitted when absent
}
```

**Rules**

1. `warning` is additive and optional. A client that ignores it behaves exactly as before.
2. A write over the ceiling answers **`422`** with the refusal text in `message`, matching the existing `ServiceError::Invalid` envelope. The status is new for this endpoint; the envelope is not.
3. Writing to `core` at a path other than `soul` answers **`422`** with the accepted-path message.
4. `DELETE /memory/doc/core/soul` **succeeds** (422 only if the guard's other rules are touched); `DELETE /memory/bundles/core` answers **`409`** — the reserved-bundle refusal uses the same shape as today's "bundle still has N concepts".

### `GET /memory/bundles`

`core` appears with a new field:

```jsonc
{ "name": "core", "concept_count": 1, "link_count": 0, "updated_at": "...", "protected": true }
```

`protected` is additive; existing clients ignore it.

## Dream endpoints

Any endpoint returning a night or cycle gains `core_changes: u32` beside the existing `core: bool`. Identity changes are no longer counted in `written`/`revised`/`removed` (FR-032), which is a **behaviour change in the numbers** that clients display — the WebUI is updated in the same change, and the field is additive so nothing else breaks.
