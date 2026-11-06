# Quickstart: Verifying Unified Local Time

Run these on Linux or WSL against a built binary, with an agent `tz-test` on the offline `dummyplug` provider. Protocol: `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`.

`TZ` values here are deliberately not UTC. For each `run` below, use a fresh workspace unless the step says otherwise.

## 1. Zone resolution (FR-001, FR-002, Edge Cases)

| Start with | Expected |
|---|---|
| `TZ=Asia/Jakarta` | Starts. Startup log names `Asia/Jakarta` and `+07:00`. Log lines show `+07:00` |
| `GET /api/v1/utils/timezone` | `{"name":"Asia/Jakarta","offset":"+07:00"}` |
| `TZ=Asia/Jakrta` | Exits 1, naming the value |
| `TZ=UTC-7` | Exits 1, saying an IANA name is required |
| `TZ` unset in a container with no zone | Starts on UTC |
| Docker image, `-e TZ=Asia/Jakarta` | Starts on `+07:00` (needs `tzdata`) |
| Binary on a host with no zone files, `TZ=Asia/Jakarta` | Exits 1, saying zone data is missing |

## 2. Context holds no time (FR-013, SC-003)

With `TZ=Asia/Jakarta`:

1. Send `context`. Expected: the block has no `## Time`, and no current date or time.
2. Wait at least 60 seconds and send `context` again. Expected: byte-identical to step 1.

## 3. Time tools (FR-016, FR-017, SC-008)

Send `tools`. Expected: `current_time`, `convert_time`, `shift_time`, `time_between` are listed under `time`. Then run each through its JSON request:

| Request | Expected |
|---|---|
| `current_time {}` | `now.zone` = `Asia/Jakarta`, `now.offset` = `+07:00`, matching the wall clock |
| `convert_time {"at":"2026-11-07T09:00:00","to_zone":"Europe/London"}` | `to.time` = `02:00` |
| `shift_time {"at":"2026-11-06T10:00:00","to_weekday":"tuesday","at_time":"09:00"}` | `result.date` = `2026-11-10`, `result.time` = `09:00` |
| `time_between {"from":"2026-11-07T09:00:00","to":"2026-11-07T10:30:00"}` | `seconds` = `5400` |
| `current_time {"zone":"Asia/Jakrta"}` | `status` = `unknown_zone`, `did_you_mean` contains `Asia/Jakarta`; the turn carries on |

Daylight saving: `shift_time {"at":"2026-03-07T09:00:00","zone":"America/New_York","add_days":1}` keeps `09:00`, with a new `offset`. The same call with `"add_hours":24` gives `10:00`.

## 4. Scheduling in local time (User Story 1, SC-001)

1. Find the current local time. Run `schedule_one_time_task` with `schedule` set to two minutes from now, with no offset. Expected: the confirmation has a weekday and `+07:00 (Asia/Jakarta)`.
2. `GET` the task. Expected: `schedule.OneTimeTask` has `+07:00`.
3. Within a minute of that time, a task run appears. Expected: `ran_at` has `+07:00`.
4. Run `schedule_cron_task` for the next local minute, such as `37 14 * * *`. Expected: `next_run` reads `14:37+07:00`, and it fires then.
5. Run `schedule_one_time_task` with `2030-01-01T00:00:00Z`. Expected: accepted, stored as `07:00+07:00`.

## 5. Daylight-saving gap (FR-011)

With `TZ=America/New_York`:

1. `POST` a one-time task for `2030-03-10T02:30:00`. Expected: `400`, naming the first valid time.
2. `POST` one for `2030-11-03T01:30:00`. Expected: accepted, stored with `-04:00` (the earlier occurrence).

## 6. Migration (User Story 2, SC-005, SC-006)

1. On the previous version with `TZ=Asia/Jakarta`, create three one-time tasks (far future, past and fired, inactive) and one cron task `0 9 * * *`. Copy `task.data` for all four.
2. Upgrade and start. Expected in the log: three rewritten. Expected in the data: each one-time schedule is the same instant with `+07:00`, and the cron row's text is **byte-identical**.
3. Restart. Expected: no rows change, and the marker `migration.one_time_task_local_v1` is set.
4. Add a task row with broken JSON to a copy of the old database and repeat. Expected: a warning, and the others still migrate.
5. Run steps 1–3 on PostgreSQL.

## 7. Cron through the upgrade (User Story 3)

1. On the previous version with `TZ=Asia/Jakarta`, create a cron task for a local time **earlier today**, with `last_executed_at` from yesterday.
2. Upgrade. Expected: at most one catch-up task run, then `next_run` is that time tomorrow.
3. Repeat with `TZ=UTC`. Expected: no change to its firing moment.

## 8. Day boundaries (User Story 5)

With `TZ=Asia/Jakarta`, record dummyplug usage at 06:00 and 08:00 local on one date (seed `usage_event` rows). Expected: `GET` usage by day shows one date with both, on SQLite and PostgreSQL. Home and dream nights ignore `?tz=-05:00`, and the request succeeds.

## 9. Chronological order (FR-006)

Seed integration events and client calls with times either side of the upgrade, then list and page them. Expected: true order, with no duplicates or gaps across pages.

## 10. Prompt and channels

- Send a Discord or Telegram message and check its stored `sent_at`. Expected: local RFC 3339. *(Needs real channel credentials.)*
- Read a history entry stored before the upgrade with a UTC `sent_at`. Expected: replayed verbatim.
- Give an MCP server the key `time`. Expected: its tools are listed under `time_mcp`.

## 11. Surfaces with the browser in a different zone (FR-005, SC-002)

Set the server to `TZ=Asia/Jakarta` and the browser to `America/New_York` (DevTools › Sensors › Location), then compare one event across the following. All should show one time, with no `UTC` label.

- The chat bubble and the task run list.
- The notification and the schedule calendar's position for it.
- `vizier agent ps` retry time and the log line.
- The task form: entering 09:00 stores `09:00+07:00`.
- Home's "today", and its greeting.
- The timezone indicator, visible on the drawn surfaces.

## 12. Automated checks

```bash
cargo test --workspace
```

```bash
TZ=Asia/Jakarta cargo test --workspace
```

```bash
TZ=America/New_York cargo test -p vizier-core time
```

```bash
scripts/052-utc-check.sh
```

```bash
cd webui && npm run typecheck && npx vitest run
```

## 13. Live provider *(optional; depends on real model output)*

Ask "remind me tomorrow at 9" and "what's today's date?" on a live provider. Expected: the agent calls `current_time` or `shift_time`, and neither guesses a date nor converts to UTC.
