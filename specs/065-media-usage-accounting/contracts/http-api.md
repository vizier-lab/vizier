# Contract: HTTP API changes

Extends `specs/018-usage-accounting/contracts/http-api.md`. All additions are new fields or new
query values; nothing existing is renamed or removed, so the change is additive. Access is 018
FR-020 everywhere (`can_view_agent`; otherwise `404`).

## `GET /agents/{agent_id}/usage` — extended

| Query param | Change |
|---|---|
| `activity` | also accepts `indexing` |
| `kind` | **new**, optional: `text` \| `image` \| `speech` \| `transcription` \| `embedding`. Narrows every figure; composes with `activity` (FR-015). Unknown → `400 unknown_kind`. |

New and extended fields in `200` (abridged):

```json
{
  "kind": null,
  "media_accounting_since": "2026-11-24T09:00:00Z",
  "summary": { "cost_usd": 61.7, "kind_share_of_all": null },
  "daily": [
    { "date": "2026-11-25",
      "by_activity": { "chat": { "cost_usd": 1.40, "tokens": 290000, "calls": 70 } },
      "by_kind": { "text": { "cost_usd": 1.12, "tokens": 290000, "calls": 64 },
                   "image": { "cost_usd": 0.25, "tokens": 0, "calls": 6 },
                   "embedding": { "cost_usd": 0.03, "tokens": 1500000, "calls": 410 } } }
  ],
  "by_kind": [
    { "key": "image", "kind": "image", "label": "Images", "detail": null,
      "cost_usd": 9.8, "share": 0.16, "tokens": 0, "calls": 233,
      "units": { "images": 233 } },
    { "key": "speech", "kind": "speech", "label": "Speech", "detail": null,
      "cost_usd": 2.1, "share": 0.03, "tokens": 0, "calls": 640,
      "units": { "characters": 140200, "audio_seconds": 9120 } }
  ],
  "by_model": [
    { "key": "openai/gpt-image-1", "label": "openai / gpt-image-1", "kind": "image",
      "provider": "openai", "model": "gpt-image-1", "served_models": [],
      "cost_usd": 9.8, "share": 0.16, "tokens": 0, "calls": 233,
      "units": { "images": 233 },
      "cache_hit_rate": null, "failed_tokens": 0, "priced": true, "price_source": "table" },
    { "key": "whisper/base", "label": "whisper / base", "kind": "transcription",
      "provider": "whisper", "model": "base", "served_models": [],
      "cost_usd": 0.0, "share": 0.0, "tokens": 0, "calls": 52,
      "units": { "audio_seconds": 1310 },
      "cache_hit_rate": null, "failed_tokens": 0, "priced": true, "price_source": "local" }
  ],
  "unpriced": { "tokens": 210000, "models": ["custom/local-qwen", "huggingface/some-image-model"],
                "units": { "image": { "images": 12 } } }
}
```

- `by_kind` is `[]` when `kind` is set; the page then hides that dimension.
- `summary.kind_share_of_all` is set only when `kind` is set: that kind's cost over all kinds' cost
  under the same `activity` filter.
- `units` holds only the fields of the row's kind (`images`; `characters` + `audio_seconds`;
  `audio_seconds`; `tokens` for embeddings). `audio_seconds` sums known durations only.
- `by_model[].price_source`: `catalogue | table | local | none` (FR-020); `priced` is
  `price_source != "none"`.
- A local row is priced at `0.0`, never listed in `unpriced` (FR-010).
- `by_activity`, `by_place` and `costliest` include media spend; each costliest entry's `model` is
  its costliest model of any kind.

## `GET /agents/{agent_id}/usage/conversation` — extended

Accepts `kind` as above. Each call row gains:

```json
{ "kind": "image", "variant": "1024x1024/high", "units": { "images": 1 }, "measured": true }
```

## `GET /agents/{agent_id}/usage/calls` — new (Story 4)

| Query param | Required | Notes |
|---|---|---|
| `ids` | yes | Comma-separated `execute_python` call ids (history `ToolCall.call_id`), 1–50. |

`200`:

```json
{
  "calls": {
    "call_9f2…": [
      { "kind": "image", "provider": "openai", "model": "gpt-image-1", "variant": "1024x1024/high",
        "units": { "images": 1 }, "measured": true, "local": false,
        "cost_usd": 0.167, "price_source": "table", "at": "2026-11-25T10:02:11Z" }
    ]
  }
}
```

An id with no media records is absent from `calls`. Only records of this agent are returned.

| Error | Code |
|---|---|
| No `ids`, or more than 50 | `400 invalid_ids` |
