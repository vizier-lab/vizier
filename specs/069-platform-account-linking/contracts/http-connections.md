# Contract: Connections HTTP API

All routes are under `/api/v1`, behind `require_auth`, and act only on the signed-in person. The desktop app reaches them through the dispatcher. Errors use the standard envelope (`vizier-http/src/error.rs`).

## `GET /auth/users/me/connections`

```json
{
  "links": [
    { "platform": "discord", "platform_user_id": "381926457021",
      "platform_username": "rin_dev", "platform_display_name": "Rin",
      "linked_at": "2026-11-27T07:02:11Z" }
  ],
  "unverified": [
    { "platform": "telegram", "platform_user_id": "55120", "platform_username": "rin" }
  ],
  "events": [
    { "platform": "discord", "platform_user_id": "381926457021", "action": "linked", "at": "…" }
  ]
}
```

`unverified` holds the legacy hand-typed values (#82). It is empty once they are cleared or replaced.

## `POST /auth/users/me/connections/preview`

Body: `{ "code": "abcd-efgh" }`. Case and separators don't matter. **This consumes nothing.**

`200`:

```json
{ "platform": "discord", "platform_user_id": "381926457021",
  "platform_username": "rin_dev", "platform_display_name": "Rin",
  "vizier_username": "rin",
  "replaces": null }
```

`replaces` is the caller's current link on that platform, if any.

Errors:

| Status | Code | When |
|---|---|---|
| `404` | `connect_code_unavailable` | Unknown, expired or used. One message: "This code has expired or was already used. Send /connect again." |
| `409` | `platform_linked_elsewhere` | The account is linked to another Vizier user. Who is never named. |
| `429` | `rate_limited` | More than 10 attempts in 10 minutes, per person or per address. Carries `Retry-After`. |

## `POST /auth/users/me/connections`

Body: `{ "code": "ABCDEFGH" }`. Does the same checks as preview, then takes the code and links, all in one transaction.

`200`: `{ "link": { … }, "replaced": { … } | null }`.

Errors are the same as preview. A second redeem of the same code is `404 connect_code_unavailable`.

## `DELETE /auth/users/me/connections/{platform}`

`platform` is `discord` or `telegram`. `204` when unlinked, `404 not_linked` when there is nothing to unlink.

## `DELETE /auth/users/me/connections/{platform}/unverified`

Clears the legacy hand-typed values for that platform. `204` (#82).

## Changed: `PUT /auth/users/me/profile`

- `discord_id`, `discord_username`, `telegram_id`, `telegram_username` with a non-null value: `422 connect_required` ("Connect accounts with /connect").
- `null` or absent: unchanged behaviour.
- `alias` is unchanged.

## Conformance

Each route gets a vector in `tests/conformance/groups/connections.rs`, removed from `pending.txt`.
