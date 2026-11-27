# Contract: HTTP changes

Only response shapes change. No route is added or removed.

## Usage report (`GET /api/v1/agents/{id}/usage`, `/api/v1/usage`, the conversation report)

`by_model[]` (`ModelRow`):

```jsonc
{
  "key": "venice/qwen-image-2-pro", "label": "…", "kind": "image",
  "cost_usd": 0.12,
  "price_source": "venice",            // was "catalogue" | "table" | "local" | "none"
  "borrowed_from": null,               // provider id when price_source == "borrowed"
  "price_alternatives": [              // other sources that price this row differently, ≤ 3
    { "source": "curated", "cost_usd": 0.10 }
  ]
}
```

`price_source` is an open string. Today's values: `models_dev`, `openrouter`, `venice`, `curated`,
`inference_profile`, `borrowed`, `local`, `subscription`, `none`. A client shows `sources[].name`
for a source id and its own wording for the rule labels.

Top level gains:

```jsonc
"sources": [
  { "id": "models_dev", "name": "models.dev", "fetched_at": "2026-11-27T10:00:00Z" },
  { "id": "openrouter", "name": "OpenRouter", "fetched_at": "…" },
  { "id": "venice",     "name": "Venice",     "fetched_at": "…" },
  { "id": "curated",    "name": "vizier's price table", "fetched_at": "2026-11-24T00:00:00Z" }
]
```

`unpriced` is unchanged: it lists only `price_source == "none"` rows (FR-017).

**Breaking**: `catalogue` → `models_dev`, `table` → `curated`. The value is computed per read and
never stored, so there is no compatibility alias. The commit is `feat(catalogue)!:` and the API
docs (`docs/src/configuration/usage.md`) say so.

## Media calls (`GET /api/v1/agents/{id}/usage/calls?ids=`)

`MediaCall.price_source` gets the same open string.

## Catalogue status (`GET /api/v1/catalogue`)

The top-level fields stay as they are: models.dev's `source`, `fetched_at`, `checked_at` and
`refresh`, so the Providers page keeps working, plus the merged `provider_count`/`model_count`.
It gains:

```jsonc
"sources": [
  { "id": "openrouter", "name": "OpenRouter", "origin": "cached", "fetched_at": "…",
    "checked_at": "…", "model_count": 548, "unmapped": 0,
    "refresh": { "enabled": true, "ttl": "24h", "in_progress": false, "last_error": null, "failed_at": null } }
]
```

`POST /api/v1/catalogue/refresh` refreshes every source. It answers 200 when models.dev succeeds,
as today, and `sources[].refresh.last_error` carries other sources' failures.

## Model detail (`GET /api/v1/catalogue/providers/{id}/models[/{model}]`)

Same shape. The list now also includes models only another source knows for that provider
(text kind only, FR-022), and their `caps`/`limit` come from the merged view.
