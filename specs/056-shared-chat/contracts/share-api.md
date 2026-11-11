# Contract: Shares HTTP API

Base `/api/v1`. Envelope: the existing `APIResponse<T>` (`api_response`/`err_response`). Errors map from `ServiceError`. Every route is mirrored in the desktop dispatcher except `GET /shares/{id}` (public, no desktop twin; listed in `conformance/pending.txt`).

## POST /shares  — create

Auth: `require_auth` + permission `owned_agents:share_conversations` for an agent the caller owns, or `all_agents:share_conversations` (system bypasses). Body limit 5 MiB (`DefaultBodyLimit::max`). Rate: 10 / hour / person.

Request:
```json
{
  "agent_id": "…", "topic_id": "…",
  "title": "string ≤120",
  "audience": "specific | signed_in | public",
  "people": ["user_id", "…"],
  "include_trail": false,
  "removals": { "credentials": 0, "paths": 0, "recall": 0, "reasoning": 0, "media": 0 },
  "snapshot": { /* snapshot-format v1 */ }
}
```
`people` is required and 1–50 long iff `audience = specific`, and must be absent otherwise. Every id must be an existing, enabled user; the author may be in the list.

Responses: `201 {share_id, url_path: "/share/<id>", created_at}` · `400` invalid shape/`format_version`/title/people · `401` · `403 permission` (also when the caller cannot see the agent) · `413` body or content over 5 MiB · `422 trail_mismatch` if `include_trail=false` but the document holds trail parts (cheap structural check, no rewriting) · `429` rate or 200-active cap (with `retry_after_secs` when rate) · `503` storage.

The server **does not** read the document beyond: JSON parses; `format_version == 1`; top-level keys are the known ones; `include_trail` agrees with the presence of `trail` parts; byte length. It stores the received text unchanged.

## GET /shares/{share_id}  — read (public route, optional sign-in)

Auth: `optional_auth`. `share_id` must match `^[A-Za-z0-9_-]{21}$`.

| Situation | Status | Body | Cache-Control |
|---|---|---|---|
| active, `public` | 200 | `{share_id, title, created_at, audience, include_trail, format_version, snapshot}` | `public, max-age=300, s-maxage=300` + `ETag: "<id>"` (304 on `If-None-Match`) |
| active, `signed_in`, signed in | 200 | same | `private, no-store` |
| active, `specific`, signed in and named (or the author) | 200 | same | `private, no-store` |
| no session, and the share is not active-`public` or does not exist | 401 `sign_in_required` (same body in both cases) | | `no-store` |
| signed in, and unknown / revoked / not named / malformed id | 404 `share_unavailable` (one body) | | `no-store` |
| over the miss rate (401/404 only, per address) | 429 | | `no-store` |

Always: `Content-Type: application/json`, `Referrer-Policy: no-referrer`, `X-Content-Type-Options: nosniff`, `X-Robots-Tag: noindex, nofollow`, and for non-public `Vary: Authorization, Cookie`. The body carries **no viewer-specific field**, so a cached public body is correct for everyone; the author's name is therefore not in it (see `analysis.md` A3).

The handler returns the stored `snapshot_json` text inside the envelope without parsing it into a value.

## GET /shares/people?q=  — find people to name

Auth: `require_auth` + any share permission. `q` ≥ 2 characters (else 400), prefix match on username, enabled users only, ≤ 20 results, `[{user_id, username}]` and nothing else (no email, role or agent). Needed because `GET /auth/users` requires `users:manage`. It is a static path under `/shares`; share ids are exactly 21 characters, so `people` can never be mistaken for one, and a route test pins the precedence.

## GET /shares  — list

Auth: `require_auth`. Query: `before`, `limit` (≤100), `all=true` (needs the system role or `all_agents:share_conversations`; metadata of every share).
`200 [{share_id, title, agent_id, agent_name, topic_id, audience, people_count, include_trail, size_bytes, created_at, revoked_at, removals, creator_name?}]`, newest first. Never the content. `creator_name` only with `all=true`.

## DELETE /shares/{share_id}  — revoke

Auth: `require_auth`. Author, or an administrator (system role or `all_agents:share_conversations`). `204`. Already revoked: `204`. Not yours and not admin, or unknown: `404` (not `403`, so ids are not confirmed). Takes effect at the origin at once; shared caches follow within 5 minutes.

## Not in this contract

No update, no re-share, no "widen audience" (FR-017), no import/fork endpoint.
