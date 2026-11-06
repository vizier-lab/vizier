# Research: Unified Local Time

Findings come from reading this repository at `a539e886` and the `croner` 4.0.1 source. Nothing below is left open for planning.

## R1. One time type: `Timestamp = DateTime<Local>`

**Decision**: Add `crates/vizier-core/src/time.rs` with `pub type Timestamp = chrono::DateTime<chrono::Local>` and `time::now()`. Replace `DateTime<Utc>` and `Utc::now()` across the Rust crates, including schema types, services, HTTP models and tests. The sweep is mechanical.

**Rationale**:

- Schema types are the wire format and the stored JSON format at once (`Task` is stored in `task.data` and returned by the API), so changing the type moves both together. That is the behaviour the request describes.
- `DateTime<Local>` compares and hashes by instant, so `Ord`, `Eq`, `HashMap` keys and derived `PartialEq` on `VizierChannelId::Task` keep working.
- Its `Deserialize` accepts any RFC 3339 offset, so every value already stored as `Z` still reads, and converts to local on read.
- Its `Serialize` writes `Z` at a zero offset, so a UTC instance's output is byte-identical to today's. This keeps the conformance baselines valid under CI's UTC. **Check it with a unit test** when the type lands.
- `schemars` and `utoipa` (`chrono` feature) treat any `DateTime<Tz>` as a `date-time` string, so the OpenAPI document and `schema.d.ts` do not change shape.

**Alternatives rejected**:

- *Keep `Utc` internally and convert at the edges.* This needs separate wire and storage types, or a `serde(with)` on every time field, which is about the same churn. It also keeps the second clock that caused the confusion.
- *A newtype.* It adds impls for `Add`, `Sub`, `Ord`, serde, schemas and SQL with no behaviour the alias lacks (Principle I).
- *A `Timestamp` over `chrono_tz::Tz`.* chrono has no generic `Deserialize` for `DateTime<Tz>`, and a stored zone per value is not needed.

## R2. Text columns keep a fixed sort form (FR-006)

**Finding**: Most time columns are epoch milliseconds and are unaffected. About 15 text columns hold `to_rfc3339()` and are compared as text: `ORDER BY created_at` and keyset cursors in `integration.rs`, lease comparisons on `client_tool_call.claimed_at`, and `COLLATE "C"` columns on PostgreSQL. Offsets in that text would sort wrongly, and a daylight-saving change would break them even within one instance.

**Decision**: These columns stay exactly as they are, in UTC RFC 3339, written through one helper: `time::stored(t)` and `time::stored_now()`. They are read back into a `Timestamp`.

**Rationale**: These values are sort keys. No person or agent reads them raw. Existing rows stay valid with no migration, and FR-006 requires true chronological order. Times read from them are still presented as local (FR-003), so the literal "writes local" of FR-003 is deliberately narrowed to what is observable, and to stored JSON and documents.

**Alternatives rejected**: converting every text column to epoch milliseconds, which is a schema migration on two backends with no user benefit.

**Audit before the sweep**: a stored session or channel JSON string compared as text in SQL would differ once a `Task` channel's time serializes with an offset. `VizierChannelId::to_slug` uses milliseconds and is safe; each use of a serialized session as a key must be checked.

## R3. Cron and dreams need only the type change

**Finding**: `Cron::find_next_occurrence` is generic over `TimeZone`, so a `Timestamp` argument evaluates the expression in local time. `croner` 4 documents and tests its daylight-saving behaviour: a fixed-time job in a gap runs at the first valid moment after it, an interval job skips occurrences in the gap, and a job in an overlap runs once.

**Decision**: Rely on it, and pin it with unit tests over `chrono_tz` zones through a function generic on the zone. No new scheduler code.

**Catch-up (FR-019)**: `tick_tasks` computes the next run from `last_executed_at`. If a cron's local occurrence falls between its last UTC-based run and now, it comes due at once, fires, and `last_executed_at` moves to now, so there is exactly one catch-up and no loop. West of UTC the opposite can happen: a task that already ran today can run again later that day. Both follow from the cron decision and are stated in the release notes. Dreams behave the same through `get_last_dream_time`.

## R4. What "local" resolves to, and refusing a bad zone

**Finding**:

- chrono's `Local` reads `TZ`, then `/etc/localtime`, and on Windows the OS API.
- It **silently falls back to UTC** for an unknown `TZ`, and for a valid name when zone files are missing.
- The Docker runtime image installs no `tzdata`, so `TZ=Asia/Jakarta` would quietly mean UTC there today.
- `iana-time-zone` (already in the tree through chrono) gives the host's name but ignores `TZ`.

**Decision**: `time::init_zone()` runs once at the start of `VizierRuntime::start`, so the server and desktop hosts share it, and resolves in this order:

1. On Unix, a non-empty `TZ`, stripped of a leading `:`, must be an IANA name known to `chrono-tz`. Anything else, including POSIX rule strings like `UTC-7`, is a startup error naming the value.
2. Otherwise the name comes from `iana-time-zone`.
3. **Self-check**: when a name is known, chrono-tz's current offset for it must equal `Local`'s. A mismatch is a startup error saying zone data is missing, with the fix. This catches the silent fallback.
4. If no name can be determined, the name is `None`, a warning is logged, and the instance runs on `Local`'s offset. A host with no zone set at all resolves to UTC.

On Windows `TZ` is ignored, matching chrono, so step 1 is Unix-only and isolated in this one function (Principle IV).

The Docker runtime image adds `tzdata`, and the docs show `TZ` for compose and Kubernetes.

**Alternatives rejected**:

- *A `timezone` config key.* It would be a second source of truth beside what chrono and SQLite read (Principle I).
- *Accepting POSIX rules.* The WebUI, PostgreSQL and the time tools all need a name, so it adds a nameless case for a rare input. A refusal message is cheap to act on.

## R5. `chrono-tz` becomes a direct dependency of `vizier-core`

**Decision**: Add `chrono-tz` 0.10 to the workspace dependencies and to `vizier-core`. `convert_time` and `time_between` need a zone database, and step 1 of R4 needs the list of names.

**Rationale**: The database cannot be hand-rolled. The crate is pure Rust with its data compiled in, so it works on `musl` and needs no system files. Version 0.10.4 is already in both lockfiles (through `ironcalc` in the office addon), so `app/Cargo.lock` stays aligned and no new native code arrives. The build-weight cost falls on builds without the office addon, which is accepted.

## R6. Removing time from the prompt

**Findings**:

- `context_md_with` is the only place that writes the current time.
- `system_prompt/boot.rs` carries a `## Time` section telling the agent to read the context block and use UTC for tools.
- `reactions.rs` already renders local time, and its section is volatile anyway (FR-014).
- `sent_at` on Discord and Telegram, and the `timestamp` a scheduled run puts in its metadata, are fixed once written and are replayed as stored, so they do not disturb the cache.

**Decision**:

- Delete the section from `context_md_with`. Tests that locate other sections relative to `## Time` change to the block header.
- Rewrite boot's section: the agent does not know the current time, must ask the `time` tools, and all times on this instance are local. It stays static, so the system prompt stays byte-identical across turns.
- The zone's name is **not** placed in the prompt, because it would change the cached prefix when a host is reconfigured, and `current_time` answers it.
- New `sent_at` ([R14](#r14-server-stamped-sent_at-on-every-channel)) and scheduled-run metadata write local RFC 3339. Old rows are replayed as stored.

**Note on caching**: The context block rides the newest user message, which is not itself replayed with its block on later turns, so the block's value was already outside the longest cached prefix. The main measurable effect is the removal of per-minute variance in the block. Hence SC-003 tests byte equality directly, and SC-004 only requires no regression.

## R7. Time tools

**Decision**: Four tools in a new `ToolCategory::Time` (`"time"`) in `agents/tools/time.rs`, on `default_toolset` and in `DREAM_TOOL_NAMES`:

| Tool | Purpose |
|---|---|
| `current_time` | now, in the instance zone or a named one |
| `convert_time` | a moment shown in another zone |
| `shift_time` | add days, hours or minutes; move to the next or previous weekday; set a time of day |
| `time_between` | the difference between two moments |

Calendar units (days, weekdays) keep the wall clock across a daylight-saving change, and hours and minutes are absolute. Bad input is a `status` result with the expected form and nearest zone names, like the scratchpad tools, never `Err`. The next-weekday rule is the one that wall-clock scheduling requests actually need; a natural-language date parser is out of scope, because the model does that part.

**Alternatives rejected**: one `time` tool with a mode field (harder to describe and to document per function), and leaving arithmetic to scripts (the sandbox has no reliable zone database).

## R8. Parsing a schedule (FR-009, FR-011)

**Decision**: One `time::parse_schedule(&str) -> Result<Timestamp, ScheduleTimeError>`, used by `schedule_one_time_task` and `service::tasks::validate_schedule`:

- RFC 3339 with an offset is that exact moment.
- `YYYY-MM-DDTHH:MM[:SS]` (or with a space) is local wall-clock time: a single match is used, an ambiguous one takes the earliest, and a nonexistent one is an error naming the gap and the first valid time after it.

`ScheduleRequest::OneTime.datetime` becomes a `String` on input, with `#[schema(value_type = String, format = DateTime)]`, so a gap is a 400 with the existing "Invalid schedule" response, not a serde rejection.

## R9. Day boundaries

**Finding**: `start_of_today` and `night_of`/`build_nights` take client offset minutes, which is wrong across daylight saving. `Dialect::day_of_millis` groups by UTC day on both backends.

**Decision**:

- Replace the offset argument with local-day helpers in `time`, generic over the zone so they can be tested.
- SQLite uses `date(col / 1000, 'unixepoch', 'localtime')`, which reads the same OS setting as chrono.
- PostgreSQL uses `AT TIME ZONE '<zone name>'`, the name coming only from the validated resolver, or a numeric offset if the name is `None`.
- The `tz` query parameter stays accepted and ignored. `parse_offset` goes, and the parameter is an entry on #82.

Usage rows hold instants, so past usage regroups correctly with no migration.

## R10. WebUI: one clock, no design-component change

**Finding**: Display logic is spread over `lib/format.ts`, `conversations.ts`, `schedule.ts`, `dreams.ts`, `overview.ts`, `messages.ts`, `routes/home.tsx` and the schedule screens, and it relies on `Date`'s browser-local getters. Design's `DateTimeField` also works with browser-local `Date`s.

**Decision**: A small `lib/timezone.ts` holds the instance zone and provides:

- `zoned(instant): Date`, a `Date` whose browser-local fields equal the instance's wall clock, so existing calendar and getter code stays correct;
- `wallClockString(date)`, the zone-less `YYYY-MM-DDTHH:mm:ss` the form sends, leaving the server to resolve gaps and overlaps (R8);
- `Intl.DateTimeFormat` with `timeZone` for formatting, or a fixed offset when the name is `null`.

`AppShell` loads `GET /api/v1/utils/timezone` before rendering times. If that fails, it falls back to the browser's zone and logs it. Vitest runs these helpers with a zone unlike the runner's. The `tz` parameter stops being sent.

**Alternatives rejected**: threading a timezone through `@vizier/design` (a larger change with no visible benefit), and a date library (the platform's `Intl` covers it, Principle I).

## R11. Logs

**Decision**: Enable `tracing-subscriber`'s `chrono` feature and use `ChronoLocal::rfc_3339()` on the fmt layer. OTLP export keeps its protocol's epoch timestamps, and external formats (`sigv4`, HTTP dates, JWT `exp`) are out of scope.

## R12. Keeping it unified

**Decision**: `scripts/052-utc-check.sh`, following `scripts/048-agent-state-read-check.sh`, refuses `Utc::now`, `DateTime<Utc>` or `with_timezone(&Utc)` outside an allow-list (`time.rs`, `llm/sigv4.rs`, `llm/error.rs`, the S3 date parser, and tests of external formats), naming the file and line. CI's `policy` job runs it. This is how FR-004 stays true after the sweep.

## R13. The migration

**Finding**: Existing task migrations run through `on_sqlite`, but this one must also run on PostgreSQL, and a `.sql` file cannot know the zone.

**Decision**: `migrate_one_time_tasks_to_local` is written on the shared `SqlStore` connection, in one transaction, with the state marker `migration.one_time_task_local_v1`:

- it parses each row's `data` as JSON;
- it rewrites only `schedule.OneTimeTask` to the local RFC 3339 form of the same instant;
- it leaves cron rows untouched and skips an unparseable row with a warning.

Re-running it is a no-op even without the marker, because the same instant formats to the same text in the same zone. A task in `vizier storage migrate`'s output is valid in either form. Listed on #82 with the parser leniency it relies on.

## R14. Server-stamped `sent_at` on every channel

**Findings**:

- The WebUI builds `timestamp: new Date().toISOString()` and `metadata: null` in the browser (`webui/app/stores/turns.ts`). `crates/vizier-http/src/topic.rs` parses that request and does not replace the timestamp, so the browser's clock reaches stored history and the history cutoff that uses `request.timestamp`.
- Discord and Telegram already write `sent_at` on the server at three sites. Their other request sites use `metadata: {}`; these appear to be commands rather than messages, and **each needs checking when the code changes**.
- History replay rebuilds a message with `VizierRequest::to_prompt`, whose frontmatter prints `metadata`. So a `sent_at` stored with the request reaches the agent and is replayed byte-identically.

**Decision**: One core helper, `VizierRequest::stamp_received(now)`, sets `timestamp` and `metadata.sent_at`, keeping other metadata keys. A `null` or non-object `metadata` becomes an object holding just `sent_at`.

- **WebUI and API sockets**: called in the `VizierRequest` arm of `TopicConnection::handle_inbound`, next to the existing `background_depth = 0` reset. That is the one path shared by the WebSocket handler and the desktop app's IPC, so client events and question answers are covered too. The `timestamp` field stays required on input, so existing clients keep working, and its value is discarded.
- **Discord and Telegram**: every request carrying a person's message goes through the same helper, so the format has one source. `sent_at` remains the receipt time.
- **Resumed turns**: `resend_pending_turns` re-sends the stored request without passing through the socket, so it is never stamped again (FR-025).

**Not stamped**: `/btw` side chats (never stored, and they read the host turn's history), and machine-made requests such as scheduled runs, background reports and inter-agent turns, which carry their own framing.

**Alternatives rejected**:

- *Trusting the client's time.* A wrong clock would corrupt history order and the cutoff.
- *Adding `sent_at` only in `to_prompt`, computed from `timestamp`.* This would change how old stored messages replay, which disturbs cached history.
- *The platform's own message time on Discord and Telegram.* It is more accurate for late delivery, but it is a separate change and would make the channels differ from the WebUI's rule.

**Display**: `lib/messages.ts` renders a message's time in the instance's zone. The WebUI also creates a browser-timed optimistic message before the server answers. Whether the saved entry replaces that time needs checking when the screens change; if it doesn't, the bubble must take the server's time.
