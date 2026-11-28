# Contract: HTTP

## Public

`POST /api/v1/hooks/{trigger_id}` (bearer secret) or `.../{trigger_id}/{secret}`

| Result | Response |
|---|---|
| accepted | `202 {"event_id": n, "outcome": "accepted|ignored|dropped|duplicate"}` |
| unknown, inactive, bad secret, signature or timestamp | `404`, one body |
| too many misses | `429` with `Retry-After` |
| body over 256 KiB | `413`, only to a sender whose secret is right |
| content type not JSON, form or text | `415` |
| storage unavailable | `503` |

## Signed in

| Route | Needs |
|---|---|
| `GET /trigger-kinds` | signed in |
| `GET /agents/{id}/triggers`, `GET .../{slug}` (each with `latest_run`, `runs_last_hour`, `needs_person`) | view_triggers |
| `GET /agents/{id}/trigger-summary` (`total`, `active`, `needs_person`) | view_triggers |
| `POST`, `PUT`, `DELETE .../triggers[/{slug}]` | manage_triggers |
| `POST .../{slug}/secret` (replace) | manage_triggers + public_webhooks |
| `POST .../{slug}/test` | manage_triggers |
| `POST .../{slug}/approve` (sets the triage model; returns a webhook's fresh secret once) | manage_triggers; + public_webhooks for a webhook; + model choice for a triage model |
| `GET .../{slug}/events` (cursor, verdicts) | view_triggers |
| `GET .../{slug}/runs` (cursor), `.../runs/{run_id}` (the run, its events and conversation) | view_triggers |

Each permission is `all_agents:` or `owned_agents:` (the owner only). Creating or activating a
webhook also needs `public_webhooks`. Creating returns the
secret and address once. Reads mask secrets.
