# Contract: HTTP and WebSocket changes

## New: `GET /api/v1/utils/timezone`

Requires authentication. Also reachable through the desktop dispatcher.

```json
{ "name": "Asia/Jakarta", "offset": "+07:00" }
```

`name` is `null` only when the host's zone cannot be named; a client then uses `offset`. It needs a vector in `tests/conformance/groups/`.

## Changed: every response time

All time fields are still RFC 3339 strings with the same names. The offset becomes the instance's: `2026-11-07T09:00:00+07:00` rather than `2026-11-07T02:00:00Z`. A client that parses RFC 3339 sees the same instant. **On a UTC instance the output is unchanged.**

## Changed: scheduling input

`POST` and `PUT /agents/{id}/tasks`, `schedule: {"type": "OneTime", "datetime": …}`:

| Input | Meaning |
|---|---|
| `2026-11-07T02:00:00Z`, `2026-11-07T09:00:00+07:00` | That exact moment, as today |
| `2026-11-07T09:00:00`, `2026-11-07 09:00` | **New**: local wall-clock time |
| A local time in a daylight-saving overlap | The earlier occurrence |
| A local time in a daylight-saving gap | `400`, naming the first valid time |

Cron expressions are evaluated in local time. `next_run` is the instant of the next local occurrence.

## Changed: times in URLs

A time with a positive offset contains `+`:

- **Path** (`/runs/{run_id}`): `+` is literal. Clients percent-encode as usual.
- **Query** (`before=`): a raw `+` decodes to a space. Clients must percent-encode. The server also accepts the decoded form, where a space stands before the offset, so a client that echoes a returned value unencoded keeps working.

## Deprecated: `tz` query parameter

On `/overview/*` and `/agents/{id}/dream/nights`. It is accepted and ignored; days always follow the instance's zone. The parameter stays in the OpenAPI document, marked deprecated. Removal is tracked on #82.

## Unchanged

- Field names, types and error envelopes.
- WebSocket frame shapes; only their time values change, as above.
- External protocols: AWS request signing, HTTP dates, JWT `exp`, OTLP.
