# Contract: `time` tools

Category `time`, on `default_toolset`, available on every turn kind and in `DREAM_TOOL_NAMES`. All four are read-only and reached from `execute_python`, like every other tool.

Every result has `status`. A refusal is a result, never an error, so a script and the turn carry on.

## The moment shape

Successful results describe moments like this:

```json
{
  "iso": "2026-11-07T09:00:00+07:00",
  "date": "2026-11-07",
  "time": "09:00",
  "weekday": "Saturday",
  "zone": "Asia/Jakarta",
  "offset": "+07:00"
}
```

`zone` is `null` only when the instance's zone cannot be named.

## `current_time`

| Input | Type | Default |
|---|---|---|
| `zone` | IANA name | Instance zone |

Result: `{"status": "ok", "now": <moment>}`

## `convert_time`

| Input | Type | Default |
|---|---|---|
| `at` | Moment text (see below) | required |
| `to_zone` | IANA name | required |
| `from_zone` | IANA name, for `at` with no offset | Instance zone |

Result: `{"status": "ok", "from": <moment>, "to": <moment>}`

## `shift_time`

Applied in this order: `add_*`, then `to_weekday`, then `at_time`.

| Input | Type | Notes |
|---|---|---|
| `at` | Moment text | Default: now |
| `zone` | IANA name | Default: instance zone |
| `add_days` | Integer | Calendar days; keeps the wall clock across daylight saving. May be negative |
| `add_hours`, `add_minutes` | Integer | Absolute time. May be negative |
| `to_weekday` | `monday` … `sunday` | Moves to that weekday, never staying on the same day |
| `direction` | `next` or `previous` | Default `next`. Used with `to_weekday` |
| `at_time` | `HH:MM` | Sets the wall-clock time |

Result: `{"status": "ok", "result": <moment>}`

A result falling in a daylight-saving gap moves to the first valid moment after it, with `"note"` saying so. A result in an overlap uses the earlier occurrence.

## `time_between`

| Input | Type | Default |
|---|---|---|
| `from` | Moment text | required |
| `to` | Moment text | Now |

Result: `{"status": "ok", "seconds": 5400, "text": "1 hour 30 minutes", "direction": "future"}`

`seconds` is signed. `direction` is `future`, `past` or `same`.

## Moment text

Same grammar as scheduling (`time::parse_schedule`): RFC 3339 with an offset is that exact moment, and `YYYY-MM-DDTHH:MM[:SS]` (or with a space) is local to the relevant zone.

## Refusals

| `status` | When | Extra fields |
|---|---|---|
| `unknown_zone` | The zone is not an IANA name | `did_you_mean`: up to 3 nearest names |
| `invalid_time` | Moment text or `at_time` cannot be read | `expected`: the accepted forms |
| `invalid_argument` | Bad weekday or direction, or no shift given | `expected` |
| `in_gap` | `convert_time`'s input does not exist locally | `first_valid`: a moment |

## Scheduling tools (changed)

`schedule_one_time_task.schedule` accepts moment text and is described as local. Its result reads:

`Task 'Standup' scheduled for Saturday 2026-11-07 09:00 +07:00 (Asia/Jakarta)`

`schedule_cron_task.cron` is described as evaluated in the instance's local time.
