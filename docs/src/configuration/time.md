# Time

A Vizier instance runs on one clock: the timezone of the machine or container it is on. Every time you see from it, in the WebUI, the API, the CLI, the logs or an agent's reply, is in that zone and carries its offset. Agents, the scheduler and people all mean the same thing by "09:00".

The WebUI and the desktop app show the instance's zone, whatever zone the browser is in. If you open the WebUI from somewhere else, the times are the instance's, not your own. The zone isn't drawn on screen; `GET /api/v1/utils/timezone` returns it.

## Choosing the zone

There is no Vizier setting for it. The instance uses the operating system's own:

1. `TZ`, if it's set (Linux and macOS). It must be an IANA name, such as `Asia/Jakarta`.
2. Otherwise, the host's configured zone. On Windows this is always the source.
3. A host with no zone set at all runs on UTC.

The startup log names the result, and `GET /api/v1/utils/timezone` returns it:

```json
{ "name": "Asia/Jakarta", "offset": "+07:00" }
```

### Docker and Compose

The image includes the zone database, so setting `TZ` is enough:

```sh
docker run -e TZ=Asia/Jakarta vizierlab/vizier
```

```yaml
services:
  vizier:
    image: vizierlab/vizier
    environment:
      TZ: Asia/Jakarta
```

### Kubernetes

```yaml
env:
  - name: TZ
    value: Asia/Jakarta
```

### What Vizier refuses

Vizier stops at startup, naming the problem, rather than quietly running on UTC, in these cases:

| Setting | Why it's refused |
|---|---|
| A misspelled name, such as `TZ=Asia/Jakrta` | The operating system would silently treat it as UTC |
| A POSIX rule, such as `TZ=UTC-7` | The WebUI, PostgreSQL and the `time` tools all need a zone name. A POSIX rule also reads backwards: `UTC-7` means seven hours *ahead* of UTC |
| A valid name on a host with no zone files | The operating system would silently treat it as UTC. Install the `tzdata` package |

## Scheduling

| Input | Meaning |
|---|---|
| `2026-11-07T09:00` or `2026-11-07 09:00:00` | Local time |
| `2026-11-07T09:00:00+07:00` or `2026-11-07T02:00:00Z` | That exact moment |
| A local time that happens twice, when the clocks go back | The earlier one |
| A local time that doesn't exist, when the clocks go forward | Refused, naming the first valid time |

Cron expressions and dream schedules are evaluated in local time. A job set to a time the clocks skip runs once, at the first valid time after it. A job set to a time that happens twice runs once.

## How agents tell the time

Agents aren't told the current time in their prompt, which keeps the prompt identical from turn to turn and makes prompt caching more effective. They have four tools in the `time` category instead:

| Tool | Purpose |
|---|---|
| `current_time` | The time now, in the instance's zone or a named one |
| `convert_time` | A moment shown in another zone |
| `shift_time` | Add days, hours or minutes, move to a weekday, or set a time of day |
| `time_between` | The difference between two moments |

Each message an agent reads shows when it arrived as `sent_at`. The server sets this time when it receives the message, on every channel. A time sent by a client is discarded.

## Changing the zone

Set `TZ`, or change the host's zone, and restart. Stored moments stay the same moments and are shown in the new zone. Cron expressions keep their text, so they run at the same wall-clock time in the new zone. Conversations are re-rendered once with the new offsets, which costs one prompt-cache miss each.

## Upgrading

> [!WARNING]
> On an instance that isn't running on UTC, **cron tasks and dream schedules move**. An expression's text is kept, and it now means local time, where it used to mean UTC.

A task set to `0 9 * * *` used to run at 09:00 UTC. On a `TZ=Asia/Jakarta` instance it now runs at 09:00 Jakarta time, seven hours earlier. If you want the old moment, change the expression before or after upgrading.

During the first day after the upgrade:

- A cron task whose new local time has already passed runs **once** to catch up, then follows the local schedule.
- West of UTC, a task that has already run today can run a second time later that day.

On a UTC instance nothing moves.

**One-time tasks keep their moment.** They are rewritten once at startup to the local form of the same instant, so a task that was due at `02:00Z` reads `09:00+07:00` and fires at the same moment.

**A POSIX `TZ` no longer starts.** An instance with `TZ=UTC-7` or similar must switch to an IANA name, such as `Etc/GMT-7` or `Asia/Jakarta`.

**API responses show local offsets.** Field names are unchanged, and any client that parses RFC 3339 sees the same instants. The `tz` query parameter on `/overview/*` and `/agents/{id}/dream/nights` is accepted and ignored, because days follow the instance's zone. A `timestamp` sent on a chat socket is discarded too. If you put a returned time in a URL yourself, percent-encode it: an offset can contain `+`.
