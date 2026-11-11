# Shared Chat API

A share is a frozen snapshot of one conversation, stored on the instance and addressed by an unguessable 21-character id. The link is `/share/<id>` in the WebUI; the data behind it is `/api/v1/shares/<id>`. The snapshot is built and cleaned **in the browser** before upload. The server validates its shape and size, and stores the text unchanged.

All routes use the standard `APIResponse<T>` envelope under `/api/v1`.

> **Implementation status.** `POST /shares` and `GET /shares/{share_id}` are implemented. `GET /shares/people`, `GET /shares` (list) and `DELETE /shares/{share_id}` (revoke) are specified but arrive in a later phase; see [Sharing](../configuration/sharing.md).

## Create: `POST /shares`

Requires authentication and the permission `owned_agents:share_conversations` (for an agent the caller owns) or `all_agents:share_conversations`. The body limit is 5 MiB; creation is limited to 10 per hour per person and 200 active shares.

```json
{
  "agent_id": "…", "topic_id": "…",
  "title": "up to 120 characters",
  "audience": "specific | signed_in | public",
  "people": ["user_id"],
  "include_trail": false,
  "removals": { "credentials": 0, "paths": 0, "recall": 0, "reasoning": 0, "media": 0 },
  "snapshot": { "format_version": 1 }
}
```

`people` is required (1–50 enabled users) when `audience` is `specific`, and must be absent otherwise. The audience cannot be changed later.

The server checks only that the JSON parses, `format_version` is 1, the top-level keys are known, `include_trail` agrees with the presence of trail parts, and the byte length. It does not rewrite the document.

| Status | Meaning |
|---|---|
| 201 | `{share_id, url_path, created_at}` |
| 400 | Invalid shape, format version, title or people |
| 401 / 403 | Missing credentials / missing permission (also when the caller cannot see the agent) |
| 413 | Body over 5 MiB |
| 422 | `trail_mismatch`: `include_trail` is false but the document holds trail parts |
| 429 | `rate_limited` (with `retry_after_secs` and a `Retry-After` header), or the 200-share cap |
| 503 | Storage unavailable |

## Read: `GET /shares/{share_id}`

Public route with optional sign-in. The id must match `^[A-Za-z0-9_-]{21}$`.

| Situation | Status | `Cache-Control` |
|---|---|---|
| Active, audience `public` | 200 | `public, max-age=300, s-maxage=300`, plus `ETag: "<share_id>"` |
| Same, `If-None-Match` matches | 304 | same as above |
| Active, `signed_in` or `specific`, viewer allowed | 200 | `private, no-store`, `Vary: Authorization, Cookie` |
| No session, and the share is not an active public one (or does not exist) | 401 `sign_in_required` | `no-store` |
| Signed in, and unknown, revoked, not for this person or malformed | 404 `share_unavailable` | `no-store` |
| Too many misses from one address | 429 | `no-store` |

The 401 body is identical for restricted, revoked and non-existent links, and the 404 body is identical for all its causes, so a response never confirms that a link exists. The error code is in `data.error`.

The 200 body is `{share_id, title, created_at, audience, include_trail, format_version, snapshot}`. It holds **no viewer-specific field**, which is why a cached public response is correct for everyone (the author's name is deliberately absent). The stored snapshot text is placed in the envelope without being parsed into a value.

### Headers on every read

Success or failure: `Referrer-Policy: no-referrer`, `X-Content-Type-Options: nosniff`, `X-Robots-Tag: noindex, nofollow`.

## Cloudflare

Cloudflare does not cache JSON by default (it caches by file extension), and it honours an origin `Cache-Control` only for responses that are eligible. To get edge caching for public shares, add a Cache Rule for `/api/v1/shares/*` with **Eligible for cache** and **Edge TTL: Use cache-control header if present**.

- Without the rule everything still works: each request reaches the origin, which serves a primary-key read. The cache is an optimisation, not a requirement.
- Non-public responses are `private, no-store` with `Vary`, so the rule never stores them. Do not override the origin TTL to force it.
- Do not set a fixed Edge TTL longer than 300 seconds; it would outlive the revoke guarantee below.

## Why there is no `immutable`

A snapshot never changes, so a long, `immutable` cache would be technically valid. But a response cached for a year cannot be recalled by revoking the share. Public responses are therefore capped at five minutes with no `immutable` and no `stale-while-revalidate`: revoking takes effect at the origin at once and in shared caches within 5 minutes. Cache purging by API token was rejected because it needs an external dependency.
