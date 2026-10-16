# Contract: HTTP API changes

All routes are under `/api/v1` and authenticated, with the same access check as today's `GET /agents/{agent_id}/usage` (FR-020). Errors keep the shape `{status, message, data: {error: <code>, …}}`.

Money is USD as a JSON number. It is `null` (never `0`) when nothing in scope has a known price. Dates are RFC 3339; a `date` is `YYYY-MM-DD` in UTC.

## `GET /agents/{agent_id}/usage` — changed `[**breaking**]`

| Query param | Required | Notes |
|---|---|---|
| `start_date` | no | RFC 3339. Default: `end_date` − 30 days. |
| `end_date` | no | RFC 3339. Default: now. |
| `activity` | no | `chat` \| `background` \| `scheduled` \| `dreams` \| `compaction` \| `vision` \| `other`. Narrows every figure (FR-021). |

`200`:

```json
{
  "range": { "start": "2026-09-16T00:00:00Z", "end": "2026-10-16T23:59:59Z" },
  "accounting_since": "2026-09-28T09:12:44Z",
  "activity": null,
  "summary": {
    "cost_usd": 48.2,
    "tokens": 12400000, "input_tokens": 11900000, "cached_input_tokens": 8560000,
    "cache_creation_input_tokens": 120000, "output_tokens": 380000,
    "calls": 3182,
    "cache_hit_rate": 0.72,
    "failed": { "cost_usd": 2.86, "tokens": 740000, "turns": 14 },
    "share_of_all": null
  },
  "previous": { "cost_usd": 40.9, "tokens": 10100000, "calls": 2870 },
  "daily": [
    { "date": "2026-10-16",
      "by_activity": { "chat": { "cost_usd": 1.12, "tokens": 290000, "calls": 64 },
                       "dreams": { "cost_usd": 0.36, "tokens": 95000, "calls": 3 } } }
  ],
  "by_activity": [
    { "key": "chat", "label": "Chat", "detail": null, "cost_usd": 23.1, "share": 0.48, "tokens": 6100000, "calls": 1170 }
  ],
  "by_model": [
    { "key": "anthropic/claude-sonnet-4-5", "label": "anthropic / claude-sonnet-4-5", "detail": null,
      "provider": "anthropic", "model": "claude-sonnet-4-5",
      "served_models": ["claude-sonnet-4-5-20250929"],
      "cost_usd": 34.2, "share": 0.71, "tokens": 9000000, "calls": 2210,
      "cache_hit_rate": 0.78, "failed_tokens": 190000, "priced": true }
  ],
  "by_place": [
    { "key": "discord:1180…", "label": "#general", "detail": "Discord", "cost_usd": 14.9, "share": 0.31, "tokens": 3900000, "calls": 802 }
  ],
  "costliest": [
    { "channel": "http__alice__7f3c…", "topic": "7f3c…", "label": "Refactor the crawler", "detail": "WebUI topic",
      "activity": "background", "model": "anthropic/claude-sonnet-4-5",
      "tokens": 1080000, "cost_usd": 4.12, "failed_share": 0.96,
      "last_at": "2026-10-09T14:02:11Z", "exists": true }
  ],
  "unpriced": { "tokens": 210000, "models": ["custom/local-qwen"] }
}
```

- `previous` covers the period of equal length just before `start`. It is `null` when that period has no usage.
- `daily` has one entry per date in range, including dates with no usage, so the chart's x-axis is complete.
- `by_activity` is `[]` when `activity` is set. The page then hides that dimension.
- `share` is a fraction of `summary.cost_usd`. It is `null` for unpriced rows.
- `summary.share_of_all` is set only when `activity` is set: that activity's cost as a fraction of all activities' cost.
- `costliest` has at most 10 entries, ranked by `cost_usd` with unpriced entries last by tokens. When `activity` is set, each entry's figures are its spend *in that activity*.

| Error | Code |
|---|---|
| Unknown agent | `404` |
| `start_date` > `end_date`, or a range longer than 366 days | `400 invalid_range` |
| Unknown `activity` | `400 unknown_activity` |

## `GET /agents/{agent_id}/usage/conversation` — new

| Query param | Required | Notes |
|---|---|---|
| `channel` | yes | A `channel` value from `costliest` (a session slug). |
| `topic` | no | As given in `costliest`. |
| `start_date`, `end_date`, `activity` | no | As above. |
| `limit` | no | Number of calls to return, 1–100, default 20. |

`200`:

```json
{
  "label": "Refactor the crawler",
  "detail": "WebUI topic",
  "summary": { "cost_usd": 4.12, "tokens": 1080000, "input_tokens": 1060000, "cached_input_tokens": 330000,
               "cache_creation_input_tokens": 0, "output_tokens": 21000, "calls": 58, "cache_hit_rate": 0.31,
               "failed": { "cost_usd": 3.94, "tokens": 1030000, "turns": 1 }, "share_of_all": null },
  "insight": "1 turn failed (errored, timed out or was aborted) after 46 answered rounds; $3.94 of $4.12 bought no answer. The longest ran 46 rounds.",
  "calls": [
    { "at": "2026-10-09T14:02:11Z", "activity": "background", "round": 46,
      "provider": "anthropic", "model": "claude-sonnet-4-5", "served_model": "claude-sonnet-4-5-20250929",
      "input_tokens": 182000, "cached_input_tokens": 61000, "output_tokens": 900,
      "cost_usd": 0.56, "failed": true }
  ],
  "more": 38,
  "link": { "kind": "chat", "path": "/aria/chat/7f3c…" },
  "no_link_reason": null
}
```

- `calls` are ordered by `cost_usd` descending (unpriced calls last, by tokens). `more` is the count of the rest.
- `insight` is non-null only when failed cost is at least 50% of the conversation's cost (R10).
- `link.kind`, with the page `path` relative to the WebUI root:

| `link.kind` | Conversation |
|---|---|
| `chat` | WebUI topic |
| `task_run` | Task run |
| `dream` | Dream journal |
| `topic` | Background piece whose job was launched from a WebUI topic |

  `path` is resolved by the server.
- When `link` is `null`, `no_link_reason` is `no_web_page` (Discord, Telegram, or a background job not launched from a WebUI topic) or `deleted` (the session no longer exists).

| Error | Code |
|---|---|
| Unknown agent | `404` |
| Missing `channel` | `400 missing_channel` |
| `limit` out of range | `400 invalid_limit` |
| Range or activity errors | As above |

An unknown `channel` is not an error: it returns empty `calls`, `summary` zeros and `no_link_reason: "deleted"`.

## OpenAPI

Both handlers keep `#[utoipa::path]`. `AgentUsageStats` and the old helper schemas are removed from the `components(schemas(…))` list in `src/channels/http/mod.rs`, and `UsageReport` and `ConversationUsage` are registered in their place.
