---

description: "Task list for Unified Local Time"
---

# Tasks: Unified Local Time

**Input**: Design documents from `/specs/052-unify-local-timezone/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/](contracts/), [quickstart.md](quickstart.md)

**Tests**: Included where the plan and quickstart name them. Time logic that depends on a zone is tested through functions generic over `chrono::TimeZone`, using `chrono_tz` zones, because a test cannot change the process's `Local`. End-to-end checks are the dummyplug steps in quickstart.md, which the constitution requires.

**Organization**: Tasks are grouped by user story. Decisions cited as R1–R14 are in research.md.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story the task belongs to
- Paths are relative to the repository root. `webui/` and `design/` are submodules.

---

## Phase 1: Setup

**Purpose**: Branches, dependencies and the container's zone data

- [X] T001 Create the matching submodule branches with `just checkout 052-unify-local-timezone`, using the recipes in `Justfile`. Run it from WSL or Linux, because `just` on Windows supports only the `app-*` recipes. Commit order across repos stays design → webui → vizier.
- [X] T002 Add `chrono-tz = "0.10"` to `[workspace.dependencies]` in `Cargo.toml` and to `crates/vizier-core/Cargo.toml`, then confirm `app/Cargo.lock` resolves the same 0.10.4 (R5)
- [X] T003 [P] Add the `chrono` feature to `tracing-subscriber` in the root `Cargo.toml` (R11)
- [X] T004 [P] Add `tzdata` to the runtime stage's `apt-get install` list in `Dockerfile` (R4)

---

## Phase 2: Foundational

**Purpose**: The one time type and the one zone. **No user story can start until this phase is complete.** When it ends, a UTC instance's output must still be byte-identical to today's.

- [X] T005 Create `crates/vizier-core/src/time.rs` and register it in `crates/vizier-core/src/lib.rs`: `pub type Timestamp = DateTime<Local>`, `now()`, `from_millis(i64)`, and `stored(Timestamp) -> String` plus `stored_now()`, which write the UTC RFC 3339 form text sort columns use today (R1, R2)
- [X] T006 Add `Zone { name: Option<String>, offset }`, `zone()` and `init_zone() -> crate::Result<()>` to `crates/vizier-core/src/time.rs`, resolving in the R4 order. Refuse a `TZ` that is not an IANA name, including POSIX rules, naming the value. Refuse a known name whose chrono-tz offset differs from `Local`'s, saying zone data is missing. Warn when no name can be found. Keep the Unix-only `TZ` check inside this function. Split the checks into pure functions so they can be tested.
- [X] T007 Add `parse_schedule(&str) -> Result<Timestamp, ScheduleTimeError>` and `ScheduleTimeError { Unparseable, InGap { first_valid } }` to `crates/vizier-core/src/time.rs` (R8), through a function generic over `TimeZone`: RFC 3339 with an offset is that moment; `YYYY-MM-DDTHH:MM[:SS]` or the space form is wall-clock time, with an overlap taking the earliest and a gap an error
- [X] T008 Add unit tests to `crates/vizier-core/src/time.rs`: `Timestamp` serializes `Z` at a zero offset and a local offset otherwise (R1); stored `Z` values deserialize to the same instant; the `init_zone` checks accept `Asia/Jakarta` and `:Asia/Jakarta`, and refuse `Asia/Jakrta` and `UTC-7`; `parse_schedule` handles an offset, a plain local time, and gap and overlap in `America/New_York`
- [X] T009 Call `time::init_zone()` first in `VizierRuntime::start` in `crates/vizier-core/src/runtime.rs`, returning its error, and log the zone name and offset at startup
- [X] T010 Before sweeping, audit `crates/vizier-core/src/` for serialized sessions or channels used as text keys in SQL, since a `Task` channel's time will serialize with an offset (R2). Record the result as a note in `specs/052-unify-local-timezone/research.md` and convert any such key to `to_slug`.
- [X] T011 Sweep `crates/vizier-core/src/schema/` and `crates/vizier-core/src/service/`: `DateTime<Utc>` to `Timestamp`, `Utc::now()` to `time::now()`
- [X] T012 Sweep `crates/vizier-core/src/storage/`: replace `Utc.timestamp_millis_opt` with `time::from_millis` and use `Timestamp` in signatures. Keep every text sort column in UTC by writing it through `time::stored`/`stored_now` in `sqlite/client_call.rs`, `sqlite/integration.rs`, `sqlite/scratch.rs`, `sqlite/secret.rs`, `sqlite/state.rs`, `sqlite/turn.rs` and `sqlite/user.rs`, and the database columns in `memory_bundle.rs`. Document frontmatter in `memory_bundle.rs` writes local.
- [X] T013 Sweep the remainder of `crates/vizier-core/src/` (`agents/`, `scheduler/`, `channels/`, `integrations/`, `notifications/`, `artifacts/`, `projects/`, `catalogue/`, `transport/`, `file_manager/`, `dependencies.rs`) and `crates/vizier-core/tests/` and `examples/`. Cron becomes local through this change (R3). Leave `llm/sigv4.rs`, `llm/error.rs` and the S3 date parser in `storage/blob/s3.rs` alone.
- [X] T014 Sweep `crates/vizier-http/` and `crates/vizier/`. In `crates/vizier-http/src/api/v1/agents/task.rs`, make the `before` query value and the `run_id` path value parse through a helper that also accepts the decoded form, where a space stands before the offset (contracts/http-api.md).
- [ ] T015 [P] Sweep `app/src-tauri/src/`, then run `just app-test`
- [X] T016 Create `scripts/052-utc-check.sh`, following `scripts/048-agent-state-read-check.sh`, which refuses `Utc::now`, `DateTime<Utc>` and `with_timezone(&Utc)` outside the R12 allow-list and names file and line. Add it to the `policy` job in `.github/workflows/ci.yml`.
- [ ] T017 Run `cargo test --workspace`, then `TZ=Asia/Jakarta cargo test --workspace`, then the conformance suite with `TZ=UTC` against `target/038-baseline/conformance/`. Fix tests that assumed a UTC process zone. Confirm no baseline changed.

**Checkpoint**: One type, one zone, UTC output unchanged, and the check script passing.

---

## Phase 3: User Story 1 - A person and an agent mean the same time (Priority: P1) 🎯 MVP

**Goal**: A wall-clock time means the same thing to the person, the agent, the scheduler and every surface.

**Independent Test**: quickstart.md steps 1, 4, 5 and 11, on an instance outside UTC.

### Backend

- [X] T018 [P] [US1] Add cron tests through a function generic over the zone in `crates/vizier-core/src/scheduler/mod.rs`: `0 9 * * *` in `Asia/Jakarta` is 09:00 local; in `America/New_York` a fixed-time job in the spring gap fires once at the first valid time, and one in the autumn overlap fires once (R3, FR-011, SC-007)
- [X] T019 [US1] In `crates/vizier-core/src/agents/tools/scheduler/mod.rs`, parse `schedule_one_time_task.schedule` through `time::parse_schedule`, rewrite the argument description for local input, and return the confirmation from contracts/time-tools.md with weekday, offset and zone name. Rewrite `schedule_cron_task.cron`'s description to say it is evaluated in local time. Bring the refusal messages into line.
- [X] T020 [US1] In `crates/vizier-core/src/service/tasks.rs`, change `ScheduleRequest::OneTime.datetime` to a `String` with `#[schema(value_type = String, format = DateTime)]`, resolve it through `time::parse_schedule` in `validate_schedule`, and return a gap as the existing 400 "Invalid schedule", naming the first valid time (R8)
- [X] T021 [P] [US1] Add `GET /utils/timezone` in `crates/vizier-http/src/api/v1/utils.rs`, under `require_auth`, per contracts/http-api.md, register it in OpenAPI, and add a vector in `crates/vizier-http/tests/conformance/groups/`
- [X] T022 [P] [US1] Use `ChronoLocal::rfc_3339()` on the fmt layer in `crates/vizier/src/main.rs` (R11)
- [X] T023 [P] [US1] Remove the UTC label from the retry time in `crates/vizier/src/cli/agent.rs`, printing local time with its offset
- [X] T024 [P] [US1] Convert the platform message timestamp to local in `crates/vizier-core/src/agents/tools/discord/mod.rs`
- [X] T025 [US1] Regenerate `webui/openapi.json` with `scripts/openapi.mjs`, then `webui/app/api/schema.d.ts` with `npm run gen:api` in `webui/`

### WebUI

- [X] T026 [US1] ~~Draw the timezone indicator~~ Dropped in review: the WebUI shows no timezone indicator, so there is no drawing and no design pass
- [X] T027 [US1] ~~Build a component for the indicator~~ Dropped with T026
- [X] T028 [US1] Create `webui/app/lib/timezone.ts` and `webui/app/lib/timezone.test.ts` (R10): the zone state, `zoned(instant)`, `wallClockString(date)` and a cached `Intl.DateTimeFormat`, falling back to a fixed offset when `name` is `null`. Test with a zone unlike the runner's, including a date either side of a daylight-saving change.
- [X] T029 [US1] Load the zone in `webui/app/layouts/AppShell.tsx` through `webui/app/api/` before rendering times. On failure, fall back to the browser's zone and log it. Add the call in `webui/app/api/endpoints/`.
- [X] T030 [US1] Convert `webui/app/lib/format.ts` and `webui/app/lib/conversations.ts` to the instance's zone, including "today" and "yesterday", and update their tests
- [ ] T031 [P] [US1] Convert `webui/app/lib/messages.ts` and `webui/app/lib/dreams.ts`, leaving the date-only strings in `dreams.ts` as they are. Check whether a saved history entry replaces the optimistic message's browser time from `webui/app/stores/turns.ts`; if it doesn't, make the bubble take the server's time (R14).
- [ ] T032 [P] [US1] Convert `webui/app/lib/schedule.ts` and `webui/app/screens/schedule/` (`ScheduleCalendar.tsx`, `DayViewPane.tsx`, `MonthView.tsx`, `MiniMonth.tsx`) so calendar positions and "now" use `zoned`
- [X] T033 [P] [US1] Convert `webui/app/lib/overview.ts` and the greeting hour in `webui/app/routes/home.tsx`
- [X] T034 [US1] In `webui/app/routes/task-edit.tsx`, show a saved one-time task with `zoned`, compare validity against `zoned(new Date())`, submit `wallClockString(at)`, and show the server's gap refusal. Keep design's `DateTimeField` unchanged.
- [X] T035 [US1] ~~Add the timezone indicator~~ Dropped with T026
- [ ] T036 [US1] Run `cd webui && npm run typecheck && npx vitest run`, then quickstart.md steps 1, 4, 5 and 11

**Checkpoint**: User Story 1 works alone. A person and an agent see one clock.

---

## Phase 4: User Story 2 - Existing one-time tasks keep their moment through the upgrade (Priority: P1)

**Goal**: One-time tasks read in local time after the upgrade and fire at their original moment.

**Independent Test**: quickstart.md step 6, on SQLite and on PostgreSQL.

- [X] T037 [US2] Add `migrate_one_time_tasks_to_local` in `crates/vizier-core/src/dependencies.rs` on the shared `SqlStore` connection, not `on_sqlite`, in one transaction, with the marker `migration.one_time_task_local_v1`. Rewrite only `schedule.OneTimeTask` in `task.data` to the local form of the same instant, leave cron rows untouched, and skip an unparseable row with a warning (R13). Run it from startup beside `migrate_task_requester`.
- [X] T038 [US2] Add tests in `crates/vizier-core/src/dependencies.rs`, keeping the rewrite in a function over a JSON value so it can be tested: a `Z` schedule keeps its instant; a cron row is byte-identical; a broken row is skipped and the others still migrate; a second run changes nothing
- [ ] T039 [US2] Add the migration, its marker and the task parser's acceptance of `Z` values as a checklist entry on [#82](https://github.com/vizier-lab/vizier/issues/82)
- [ ] T040 [US2] Run quickstart.md step 6 by upgrading from a build of `develop`, on SQLite and on PostgreSQL

**Checkpoint**: Upgrading moves no one-time task.

---

## Phase 5: User Story 3 - Operators know what the upgrade changes for cron tasks (Priority: P2)

**Goal**: Cron tasks and dream schedules keep their text, mean local time, and the change is stated plainly.

**Independent Test**: quickstart.md step 7.

- [X] T041 [P] [US3] Add tests to `crates/vizier-core/src/scheduler/mod.rs` with `last_executed_at` from before the upgrade: a cron task whose local occurrence has passed fires exactly one catch-up and then follows the local schedule; on a UTC zone nothing moves (R3, FR-019)
- [ ] T042 [P] [US3] Check that `crates/vizier-core/src/scheduler/dream/mod.rs` and `crates/vizier-core/src/service/dream.rs` follow the same catch-up rule through `get_last_dream_time`, and add a test for it
- [X] T043 [P] [US3] Create `docs/src/configuration/time.md` and add it to `docs/src/SUMMARY.md`: how an instance's zone is chosen, setting `TZ` for Docker, compose and Kubernetes, why a POSIX `TZ` is refused, and cron and dream schedules meaning local time with the catch-up and the second-run-west-of-UTC behaviour
- [ ] T044 [US3] Add an "Upgrading" section on the cron behaviour and the refusal of a POSIX `TZ` to `docs/src/configuration/time.md`, and state the same in a `BREAKING CHANGE:` footer on the `feat!:` commit, so the `cliff.toml` changelog carries it
- [ ] T045 [US3] Run quickstart.md step 7

**Checkpoint**: An operator can read what will move before upgrading.

---

## Phase 6: User Story 4 - An agent asks for the time instead of being told (Priority: P2)

**Goal**: No current time in any prompt; agents have `time` tools, and each message shows the server's time as `sent_at`.

**Independent Test**: quickstart.md steps 2, 3 and 10.

### Time tools

- [X] T046 [US4] Add `ToolCategory::Time` (`"time"`) in display order with its description, in `crates/vizier-core/src/agents/tools/mod.rs`
- [X] T047 [US4] Create `crates/vizier-core/src/agents/tools/time.rs` with `current_time`, `convert_time`, `shift_time` and `time_between`, per contracts/time-tools.md. Refusals are `status` results with `did_you_mean` from the existing nearest-name helper. Keep the arithmetic in functions generic over the zone.
- [X] T048 [US4] Add tests in `crates/vizier-core/src/agents/tools/time.rs` for each contract case: `add_days` keeping the wall clock and `add_hours` staying absolute across a New York change; next and previous weekday never staying on the same day; every refusal status
- [X] T049 [US4] Register the four tools on `default_toolset` in `BaseTools::new()`, add them to `DREAM_TOOL_NAMES`, and add them to `every_tool_is_in_its_category`, in `crates/vizier-core/src/agents/tools/mod.rs`
- [X] T050 [P] [US4] Add a test in `crates/vizier-core/src/agents/tools/mod.rs` that an MCP server keyed `time` lists under `time_mcp`

### Message time

- [X] T051 [US4] In `crates/vizier-http/src/topic.rs`, set `request.timestamp = time::now()` in the `VizierRequest` arm, next to `background_depth = 0`, so the client's value is discarded for the WebSocket and the desktop IPC (R14, FR-024). Update the client-tools docs if they describe the field, in `docs/src/api-integration/`.
- [X] T052 [US4] In `generate_frontmatter` in `crates/vizier-core/src/schema/request.rs`, print `sent_at` from `timestamp` in local RFC 3339 to the second on every branch, and leave `sent_at` and `timestamp` out of the metadata it renders, so old rows show one time (R14, FR-023, FR-025)
- [X] T053 [P] [US4] Remove `"sent_at"` from `metadata` in `crates/vizier-core/src/channels/discord/mod.rs` and `crates/vizier-core/src/channels/telegram/mod.rs`
- [X] T054 [P] [US4] Remove the `"timestamp"` key from the scheduled run's metadata in `crates/vizier-core/src/scheduler/mod.rs`, which is the same `now` as the request's `timestamp`, and update `crates/vizier-core/src/schema/request.rs` test fixtures that set it
- [ ] T055 [US4] Add tests in `crates/vizier-core/src/schema/request.rs` and `crates/vizier-core/src/schema/history.rs`: every request kind shows one `sent_at`; a row with UTC `metadata.sent_at` shows only the local one; repeated renders through `history_entries_to_messages` are identical; a resumed request keeps its time
- [X] T056 [US4] Add a test for T051 in `crates/vizier-http/tests/topic.rs`: a socket request with `"timestamp":"2001-01-01T00:00:00Z"` is stored with the server's time
- [ ] T057 [US4] Add hiding old `metadata.sent_at` and `metadata.timestamp` as a checklist entry on [#82](https://github.com/vizier-lab/vizier/issues/82)

### Removing time from the prompt

These land only once the tools above work, so an agent is never left with neither.

- [X] T058 [US4] Remove the `## Time` section and `Utc`/`Local` from `context_md_with` in `crates/vizier-core/src/agents/agent/system_prompt/context.rs`, update the doc comments, and change tests that locate sections relative to `## Time` to use the header. Add a test that two renders with the same inputs are byte-identical and contain no time.
- [X] T059 [US4] Rewrite the `## Time` section in `crates/vizier-core/src/agents/agent/system_prompt/boot.rs`: the agent does not know the current time and must ask the `time` tools; a message's `sent_at` says when it arrived; all times here are local. Keep it static text, without the zone's name (R6, FR-015).
- [X] T060 [US4] Update any example or fixture that mentions the old section in `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md` and `crates/vizier-core/src/agents/agent/model/dummyplug.rs`
- [ ] T061 [US4] Run quickstart.md steps 2, 3 and 10, then step 13 on a live provider if credentials are available, marking it as such

**Checkpoint**: Context is byte-stable across turns, and agents can still tell the time.

---

## Phase 7: User Story 5 - Periods and day boundaries follow the local day (Priority: P3)

**Goal**: "Today", daily usage and dream nights start at local midnight.

**Independent Test**: quickstart.md step 8.

- [X] T062 [US5] Add `start_of_local_day` and `local_date_of` to `crates/vizier-core/src/time.rs`, through functions generic over the zone, with tests including a zone whose midnight falls in a daylight-saving gap
- [X] T063 [P] [US5] Change `day_of_millis` in `crates/vizier-core/src/storage/sql/sqlite.rs` to `date(col / 1000, 'unixepoch', 'localtime')`, and test the usage day grouping in `crates/vizier-core/src/storage/sqlite/usage.rs` under `TZ=Asia/Jakarta`
- [X] T064 [P] [US5] Change `day_of_millis` in `crates/vizier-core/src/storage/sql/postgres.rs` to `AT TIME ZONE` with the name from `time::zone()`, or the numeric offset when the name is `None` (R9). The name comes only from the validated resolver.
- [X] T065 [US5] Replace the `offset_minutes` arguments in `crates/vizier-core/src/service/overview.rs` with the local-day helpers, and update its tests
- [X] T066 [US5] Replace the offset arguments of `night_of` and `build_nights` in `crates/vizier-core/src/schema/dream_journal.rs` and their callers in `crates/vizier-core/src/service/dream.rs` with the local-day helpers, and remove `parse_offset` and its test
- [X] T067 [US5] Keep `tz` accepted and ignored in `crates/vizier-http/src/api/v1/overview/mod.rs` and `crates/vizier-http/src/api/v1/agents/dream.rs`, mark it deprecated in OpenAPI, and regenerate the client as in T025
- [X] T068 [P] [US5] Stop sending `tz` from `webui/app/api/endpoints/overview.ts` and `webui/app/api/endpoints/dream.ts`
- [ ] T069 [US5] Add the ignored `tz` parameter and the query-space leniency from T014 as checklist entries on [#82](https://github.com/vizier-lab/vizier/issues/82)
- [ ] T070 [US5] Run quickstart.md step 8 on SQLite and on PostgreSQL

**Checkpoint**: A day is the day a person would call it.

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Final consistency across every surface

- [ ] T071 [P] Run quickstart.md step 9 (chronological order across offsets), and fix any text comparison the T010 audit missed
- [X] T072 [P] Update `CLAUDE.md`: add a Time section (one zone, `Timestamp`, `time::stored` for sort columns, the check script), add `time` to the category list, and correct `Utc` references in the Telemetry and Tools sections
- [ ] T073 [P] Update `docs/src/` where it describes UTC or tool categories, including the scheduling and tools pages and `docs/src/configuration/observability.md`, and the API docs for the changed `timestamp` behaviour
- [ ] T074 Run `cross build` for one `musl` target in `Cross.toml`, build the Docker image, and run quickstart.md step 1's Docker row
- [ ] T075 Run the desktop app with `just app-dev` on Windows and confirm the zone, times and task form match the OS clock
- [ ] T076 Run `cargo clippy --workspace`, the per-flag and per-role commands in `CLAUDE.md`, `scripts/052-utc-check.sh`, and quickstart.md step 12
- [ ] T077 Run a last audit for SC-002 by comparing one event across each surface in quickstart.md step 11, and confirm no surface labels a time UTC

---

## Dependencies & Execution Order

### Phases

- **Setup (Phase 1)**: No dependencies.
- **Foundational (Phase 2)**: Depends on Setup and blocks every story. T011–T015 depend on T005–T007. T017 ends the phase.
- **Polish (Phase 8)**: Depends on the stories you are shipping.

### User stories

| Story | Depends on | Notes |
|---|---|---|
| US1 (P1) | Foundational | MVP |
| US2 (P1) | Foundational | Independent of US1 |
| US3 (P2) | Foundational | Docs refer to US1's behaviour, but its tests do not need it |
| US4 (P2) | Foundational | Independent of US1; its tools use `parse_schedule` from T007 |
| US5 (P3) | Foundational | T067–T068 touch `schema.d.ts` after T025 if both ship |

### Within a story

- US1: T028 → T029 → conversions.
- US4: T046–T049 and T051–T053 before T058–T059.
- `webui/` and `design/` commits land before the vizier pin bump.

### Parallel opportunities

```text
Phase 1:  T003, T004
Phase 2:  T015 (after T005–T007)
US1:      T018, T021, T022, T023, T024, then T031, T032, T033 after T029
US3:      T041, T042, T043
US4:      T050, T053, T054
US5:      T063, T064, T068
Polish:   T071, T072, T073
```

US1 through US5 can run in parallel once Phase 2 is done, if each stays in its own files. The shared files are `crates/vizier-core/src/time.rs` (US5 only adds to it) and `schema.d.ts`.

## Implementation Strategy

### MVP

Phases 1 and 2, then US1 and US2. Both are P1 and they belong in one release: US1 alone would leave stored one-time tasks unmigrated. Stop and run quickstart.md steps 1, 4, 5, 6 and 11.

### Incremental delivery

1. Phases 1–2: no visible change on UTC, one type everywhere.
2. US1 + US2: one clock, upgrade safe.
3. US3: operators are told what moves. **Ship this with the first release containing US1**, because cron behaviour changes there.
4. US4: stable context and `time` tools.
5. US5: local days.
6. Polish.

### Notes

- Commit types: `refactor:` for the sweep, `feat!:` for the release that changes cron behaviour.
- Each #82 entry (T039, T057, T069) is an edit to a public issue. Make it when its code lands.
- Don't delete `metadata.sent_at` from stored rows, or rewrite old messages.
