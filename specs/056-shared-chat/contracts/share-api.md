# Contract: Shares HTTP API

Base `/api/v1`. Envelope: the existing `APIResponse<T>` (`api_response`/`err_response`). Errors map from `ServiceError`. Every route is mirrored in the desktop dispatcher except `GET /shares/{id}` (public, no desktop twin; listed in `conformance/pending.txt`).

## POST /shares  — create

Auth: `require_auth` + permission `owned_agents:share_conversations` for an agent the caller owns, or `all_agents:share_conversations` (system bypasses). Body limit 5 MiB (`DefaultBodyLimit::max`, FR-046). Rate: 10 / hour / person; at most 200 live shares per person (FR-047; the 250 MB total is DEFERRED, T096).

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
`people` is required and 1–50 long iff `audience = specific`, and must be absent otherwise. Every id must be an existing user (accounts have no disabled state yet); the author may be in the list.

Responses: `201 {share_id, url_path: "/share/<id>", created_at}` · `400` invalid shape/`format_version`/title/people · `401` · `403 permission` (also when the caller cannot see the agent) · `413` body or content over 5 MiB · `422 trail_mismatch` if `include_trail=false` but the document holds trail parts (cheap structural check, no rewriting) · `422 secret_detected` if any text of the snapshot or its title holds a PEM private-key block or a vendor-prefixed token (list below); the message names the kind, never the text · `429` rate or 200-active cap (with `retry_after_secs` when rate; the refusal names the limit, its value and how to free room, FR-047) · `503` storage.

The server **does not** read the document beyond: JSON parses; `format_version == 1`; top-level keys are the known ones; `include_trail` agrees with the presence of `trail` parts; byte length. It stores the received text unchanged.

## GET /shares/{share_id}  — read (public route, optional sign-in)

Auth: `optional_auth`. `share_id` must match `^[A-Za-z0-9_-]{21}$`.

| Situation | Status | Body | Cache-Control |
|---|---|---|---|
| active, `public` | 200 | `{share_id, title, created_at, include_trail, format_version, snapshot}` | `public, max-age=300, s-maxage=300` + `ETag: "<id>"` (304 on `If-None-Match`) |
| active, `signed_in`, signed in | 200 | same | `private, no-store` |
| active, `specific`, signed in and named (or the author) | 200 | same | `private, no-store` |
| no session, and the share is not active-`public` or does not exist | 401 `sign_in_required` (same body in both cases) | | `no-store` |
| signed in, and unknown / revoked / not named / malformed id | 404 `share_unavailable` (one body) | | `no-store` |
| over the miss rate (401/404 only, 60 a minute per address, FR-049) | 429 | | `no-store` |

The address is the connection's peer address; `X-Forwarded-For` / `X-Real-IP` count only when the peer is this machine (a local reverse proxy), and then the entry the proxy appended (last) is used. IPv6 counts per /64.

Always: `Content-Type: application/json`, `Referrer-Policy: no-referrer`, `X-Content-Type-Options: nosniff`, `X-Robots-Tag: noindex, nofollow`, and for non-public `Vary: Authorization, Cookie`. The body carries **no viewer-specific field**, so a cached public body is correct for everyone; the author's name (FR-007) therefore belongs inside the snapshot, chosen at create time, not in the read response (see `analysis.md` A3). The body has no `audience` field (FR-030, T104); only the author's own list (`GET /shares`) carries it.

**`secret_detected` (create).** A last-resort backstop for FR-051, not redaction and not a promise of a clean snapshot: the client removes what it can, the person reviews the rest, and the server refuses only what it can tell with near certainty. Every string of the snapshot (turn text, trail arguments, titles, alt text, UI spec strings, object keys) and the title is scanned, except base64 `media` data. Matched: `-----BEGIN … PRIVATE KEY-----`; `sk-` keys (32+ chars with a digit); `ghp_/gho_/ghu_/ghs_/ghr_` (36+), `github_pat_`; `AKIA`/`ASIA` + 16; `glpat-`; `xox[abpr]-`; `hf_` (30+); `npm_` (36); `SG.x.y`; `ya29.`; `GOCSPX-`. Not matched, on purpose: generic `password=`, `token=`, bearer or high-entropy shapes (too many false positives). Nothing is stored on refusal.

The handler returns the stored `snapshot_json` text inside the envelope without parsing it into a value.

## GET /shares/people?q=  — find people to name

Auth: `require_auth` + any share permission. `q` ≥ 2 characters (else 400), case-insensitive literal prefix match on username (`%` and `_` are not wildcards), ≤ 10 results (FR-012), `[{user_id, username}]` and nothing else (no email, role or agent; the spec says "display name and username", but accounts have only a username, `analysis.md` A6). 30 searches a minute per person, then `429` with `Retry-After`. Needed because `GET /auth/users` requires `users:manage`. It is a static path under `/shares`; share ids are exactly 21 characters, so `people` can never be mistaken for one, and a route test pins the precedence.

## GET /shares  — list

Auth: `require_auth`. Query: `before`, `limit` (≤100), `all=true` (needs the system role or `all_agents:share_conversations`; metadata of every share).
`200 [{share_id, title, agent_id, agent_name, topic_id, audience, people_count, include_trail, size_bytes, created_at, revoked_at, removals, creator_name?}]`, newest first. Never the content. `creator_name` only with `all=true`. With `all=true`, `title` is **absent** for a share whose audience does not include the caller (FR-043); the names of named people are never returned, only `people_count`.

## DELETE /shares/{share_id}  — revoke

Auth: `require_auth`. Author, or an administrator (system role or `all_agents:share_conversations`). `204`. Already revoked: `204`. Not yours and not admin, or unknown: `404` (not `403`, so ids are not confirmed). Revoking deletes the snapshot text for good (FR-039; the row stays as a tombstone so the id keeps answering like an unknown one). When someone other than the author revokes, the author is to be told which share it was (FR-044, DEFERRED T093). Takes effect at the origin at once; shared caches follow within 5 minutes.

## Not in this contract

No update, no re-share, no "widen audience" (FR-041), no import/fork endpoint. Also missing, and DEFERRED: removing one named person from a live share (FR-041, T091), the record of share actions (FR-052, T099).
