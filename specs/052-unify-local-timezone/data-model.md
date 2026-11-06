# Data Model: Unified Local Time

There are no new tables or columns. This feature changes how time values are typed and formatted.

## Types

### `Timestamp` (`vizier_core::time`)

`DateTime<chrono::Local>`. It replaces `DateTime<Utc>` everywhere outside the R12 allow-list. Equality, ordering and hashing are by instant.

| Direction | Form |
|---|---|
| Serialized (API, stored JSON, tool results) | RFC 3339 with the local offset: `2026-11-07T09:00:00+07:00`; `Z` at a zero offset |
| Deserialized | Any RFC 3339 offset; converted to local on read |

### `Zone` (`vizier_core::time`)

The instance's timezone, resolved once by `time::init_zone()` before any write.

| Field | Type | Notes |
|---|---|---|
| `name` | `Option<String>` | IANA name, such as `Asia/Jakarta`. `None` only when a host's zone cannot be named |
| `offset` | `String` | Current offset, such as `+07:00`. Computed on each read, because it changes with daylight saving |

Validation rules are in [research R4](research.md#r4-what-local-resolves-to-and-refusing-a-bad-zone). It is not persisted and not configurable per agent or person.

### `ScheduleTimeError`

Returned by `time::parse_schedule`.

| Variant | Meaning | Surfaces as |
|---|---|---|
| `Unparseable` | Neither RFC 3339 nor a local date and time | Tool error; HTTP 400 |
| `InGap { first_valid }` | The local time does not exist on that date | Tool error; HTTP 400, naming `first_valid` |

## Stored values

| Value | Where | Before | After | Existing rows |
|---|---|---|---|---|
| One-time schedule | `task.data` → `schedule.OneTimeTask` | `…Z` | Local offset | **Migrated**, same instant |
| Cron expression | `task.data` → `schedule.CronTask` | Text | Unchanged; now means local | **Never rewritten** |
| Dream schedule | Agent config `dream_schedule` | Text | Unchanged; now means local | **Never rewritten** |
| Other JSON times (`last_executed_at`, `timestamp`, history data) | JSON blobs | `…Z` | Local offset on new writes | Left as is; read as local |
| Message time: `VizierRequest.timestamp` | Request | Client's clock on the WebUI and API; server's on channels | **Server's clock everywhere**, the client's value discarded. The only stored time for a message; shown to agents as `sent_at` | Left as is; shown in local time |
| `metadata.sent_at` | Request metadata | UTC on Discord and Telegram; absent on the WebUI | **No longer written** | Left in old rows, not shown (#82) |
| Scheduled-run `timestamp` | Request metadata | UTC | Local RFC 3339 | Left as is; replayed verbatim |
| Memory frontmatter (`created_at`, `updated_at`) | Memory documents | `…Z` | Local offset on new writes | Left as is; read as local |
| Epoch-millisecond columns | Many tables | Instant | Unchanged | No change |
| Text sort columns (`created_at`, `claimed_at`, …) | About 15 columns | UTC RFC 3339 | **Unchanged**, through `time::stored` | No change; see [R2](research.md#r2-text-columns-keep-a-fixed-sort-form-fr-006) |

## Migration state

| Key (`state` table) | Meaning | Removal |
|---|---|---|
| `migration.one_time_task_local_v1` | The one-time task rewrite has completed | Listed on #82 |

Rows move through one transaction, so an interrupted run leaves none changed, and the next start repeats it. An unparseable row is skipped with a warning and does not block the marker.

## Tool category

`ToolCategory::Time`, name `time`, is a built-in and therefore reserved: an MCP server keyed `time` becomes `time_mcp`, and a client tool category `time` becomes `time_client`. It is not an `IntegrationKind` and has no configuration.
