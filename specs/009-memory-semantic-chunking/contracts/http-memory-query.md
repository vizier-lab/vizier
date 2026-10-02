# Contract: `GET /api/v1/agents/{agent_id}/memory/query`

**Feature**: `009-memory-semantic-chunking` | Handler: `query_memories`, `src/channels/http/api/v1/agents/memory.rs:484`

Breaking response change, driven by FR-017. The endpoint reads the same `query_memory` that is
becoming passage-level, so its current `Vec<MemoryDetail>` return type stops being accurate.

**No user-interface work is in scope.** `webui/app/services/vizier.tsx:417` defines a
`queryMemories` client function, and nothing in the shipped web interface calls it — there is no
memory search screen. This contract exists so the response is correct and useful for whoever
builds that screen later.

## Request (unchanged)

| Param | Required | Notes |
|---|---|---|
| `query` | yes | |
| `bundle` | no | Omit for all bundles |
| `limit` | no | |
| `threshold` | no | |

## Response — was `Vec<MemoryDetail>`, now passage results

```json
{
  "status": 200,
  "data": [
    {
      "bundle": "work",
      "path": "ops/deploys",
      "title": "Deployment practice",
      "ordinal": 3,
      "ordinal_end": 4,
      "line_start": 48,
      "line_end": 71,
      "score": 0.78,
      "text": "Deploys go out Tuesday and Thursday mornings ..."
    }
  ]
}
```

Same per-result shape as `memory_search`, so one type serves both surfaces rather than two
near-identical ones (Principle II).

## Unchanged endpoints

`GET /{slug}` and `GET /doc/{bundle}/{*path}` still return the whole document — that is the read
path, and `memory_read` is its agent-facing twin. `GET /{slug}/related` keeps returning documents;
link traversal is document-scoped and untouched by chunking.

## Auth

Unchanged: `require_agent` plus the existing `AuthenticatedUser` extension.
