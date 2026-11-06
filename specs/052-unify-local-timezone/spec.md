# Feature Specification: Unified Local Time

**Feature Branch**: `052-unify-local-timezone`

**Created**: 2026-11-06

**Status**: Draft

**Input**: User description: "currently this project operate on two timezone, the local timezone, mostly for appearance, and UTC the actual timezone system use. This causes confusion especially between user and agent, not too mention we have to inject the time on context which make cache less ineffective. to permanently remedy this, let's unify the time systems to use the local timezone. then we could remove the time from context, and just make dedicated time category tools. as for user existing tasks, we only migrate non cron one, track migration for deletion this issue #82 tho"

## Clarifications

### Session 2026-11-06

- Q: When a person's browser is in a different timezone from the server, which timezone does the WebUI show? → A: The instance's timezone. There is one clock for everyone, matching what agents say, and the WebUI makes that timezone discoverable. Client-supplied offsets are ignored.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A person and an agent mean the same time (Priority: P1)

A person tells an agent "remind me tomorrow at 9". The agent schedules it for 09:00 local time without converting anything, confirms "Saturday 09:00", and the task list, the task's run history and the reminder all say 09:00. A cron task written as `0 9 * * *` runs at 09:00 on the clock on the wall, not at 09:00 in a timezone nobody in the conversation is in.

**Why this priority**: This is the confusion that motivates the feature. Today an agent is told two clocks, its scheduling tools ask for UTC, and an off-by-offset reminder is the visible result.

**Independent Test**: On an instance whose timezone is not UTC, have an agent schedule a one-time task and a cron task from plain wall-clock wording, then compare the agent's confirmation, the task screens, the CLI and the actual firing moment. All must agree with no conversion.

**Acceptance Scenarios**:

1. **Given** an instance in UTC+7, **When** a person asks for a reminder "tomorrow at 9" and the agent schedules it with a local date and time, **Then** it fires at 09:00 local and every surface shows 09:00.
2. **Given** an instance in UTC+7, **When** a cron task or dream schedule `0 9 * * *` is created, **Then** it fires at 09:00 local each day, and its displayed next run says so.
3. **Given** any scheduling input that names its own offset (such as `2026-12-25T10:30:00Z`), **When** it is submitted, **Then** it is accepted as that exact moment and shown back in local time.
4. **Given** a reply, a task run, a notification and a log line about the same event, **When** a person compares them, **Then** they show the same local time and no surface labels a time as UTC.

---

### User Story 2 - Existing one-time tasks keep their moment through the upgrade (Priority: P1)

An operator upgrades an instance that already has one-time tasks waiting. Each one still fires at the same real moment it was scheduled for, and now reads in local time. The operator does nothing.

**Why this priority**: A reminder that silently moves or vanishes on upgrade breaks trust, and it cannot be repaired after it has been missed.

**Independent Test**: Create one-time tasks (pending, already fired, and inactive) on the previous version, upgrade, then confirm each is shown in local time, the pending one fires at its original moment, and restarting again changes nothing.

**Acceptance Scenarios**:

1. **Given** a pending one-time task stored for 2026-12-25 03:00 UTC on an instance in UTC+7, **When** the instance upgrades, **Then** the task shows 10:00 local on 2026-12-25 and fires at that same moment.
2. **Given** the migration has already completed, **When** the instance restarts, **Then** no task is changed again.
3. **Given** the migration is interrupted part-way, **When** the instance starts again, **Then** it finishes the remaining tasks and no task is shifted twice.
4. **Given** an existing cron task, **When** the instance upgrades, **Then** its expression is unchanged and is read as local time from then on (see User Story 3).

---

### User Story 3 - Operators know what the upgrade changes for cron tasks (Priority: P2)

An operator with existing cron tasks and dream schedules reads the upgrade notes and learns that these keep their written expressions and now mean local time, so a task written as `0 9 * * *` in UTC moves to 09:00 local. They can adjust the ones that matter. On a UTC instance nothing moves.

**Why this priority**: Not migrating cron tasks is a deliberate decision: an expression cannot always be translated across an offset, and most were probably written with the wall clock in mind. The consequence must still be visible rather than discovered.

**Independent Test**: Upgrade an instance in UTC+7 with a cron task, confirm its expression is byte-identical and its next run reads local, and that it fires at most once around the upgrade.

**Acceptance Scenarios**:

1. **Given** a cron task whose next local occurrence falls earlier than its last run suggests, **When** the instance upgrades, **Then** the task fires at most one catch-up run and then follows the local schedule.
2. **Given** an instance whose timezone is UTC, **When** it upgrades, **Then** no cron task or dream changes its firing moment.
3. **Given** the release notes and the documentation, **When** an operator reads them, **Then** the cron behaviour change and how to set the instance's timezone are stated plainly.

---

### User Story 4 - An agent asks for the time instead of being told (Priority: P2)

The current time no longer arrives with every message. When an agent needs it, it calls a tool in a dedicated `time` category to get the current date, time, weekday and timezone, to convert a moment to another person's timezone, or to work out a date ("next Tuesday", "in 90 minutes").

**Why this priority**: This removes a value that changes on every turn from what is sent to the model, which helps provider prompt caching on every conversation. It also gives agents correct date arithmetic, including across daylight-saving changes.

**Independent Test**: Run two turns a minute or more apart on an offline agent and compare the context each received: no time appears, and the context is identical. Then ask for the time and confirm the agent gets it through the tool.

**Acceptance Scenarios**:

1. **Given** any turn of any kind, **When** its context is assembled, **Then** it contains no current date or time.
2. **Given** two turns with the same related memories, skills and client notes, **When** they run at different times, **Then** the context blocks they receive are byte-identical.
3. **Given** an agent with default configuration, **When** it lists tool categories, **Then** `time` is present in chats, scheduled runs, dreams, background pieces and inter-agent turns, with no setup.
4. **Given** a message such as "what's today's date?", **When** the agent answers, **Then** it has obtained the date from a time tool rather than guessing, as its standing instructions direct.
5. **Given** an invalid input to a time tool (an unknown timezone, an unparseable date), **When** it is called, **Then** the agent receives a correctable explanation and the turn carries on.

---

### User Story 5 - Periods and day boundaries follow the local day (Priority: P3)

"Today" on Home, a usage report's daily breakdown and a dream's night boundaries all begin at local midnight, so a day's activity is the day a person would call it.

**Why this priority**: These are less visible than scheduling but are the same inconsistency: a usage day split at 07:00 local is an artefact of the old clock.

**Independent Test**: On an instance in UTC+7, record activity at 06:00 and 08:00 local on one day, then confirm both fall in the same day of the usage report and Home.

**Acceptance Scenarios**:

1. **Given** activity at 06:00 and 08:00 local on one date, **When** a person views daily usage, **Then** both count towards that one date.
2. **Given** usage recorded before the upgrade, **When** a person views it after, **Then** it is grouped by local day like new usage, with no change to its totals.

---

### Edge Cases

- **Daylight-saving gap**: a local time that does not exist on a date (02:30 on the spring-forward day). A cron occurrence fires once at the next valid moment; a one-time schedule is refused with a message naming the gap.
- **Daylight-saving overlap**: a local time that occurs twice. A cron occurrence fires once; a one-time schedule with no offset means the earlier occurrence.
- **Ordering across offsets**: records written before the upgrade (UTC) and after (local, possibly with different offsets either side of a daylight-saving change) must list, page and filter in true chronological order, never by the text of their timestamps.
- **The instance's timezone changes** (a new host setting, a moved machine): one-time tasks and past records keep their real moments and display in the new zone; cron tasks follow the new zone from the next restart.
- **Timezone cannot be determined**: the instance runs in UTC and logs that it did. **Timezone explicitly set to something invalid**: the instance refuses to start and names the bad value, because guessing would move every schedule silently.
- **Containers**: a container with no timezone set is on UTC, so it behaves as today until the operator sets one, and setting a named zone must work in the published image.
- **MCP server keyed `time`**: `time` becomes a reserved category name, so an existing server with that key has its tools listed under `time_mcp` from the upgrade, as for every other reserved name.
- **Old replayed messages**: Discord and Telegram messages stored with a UTC `sent_at` are replayed unchanged; rewriting them would invalidate cached history and they are already unambiguous because they name UTC. New messages carry local time.
- **A task row that cannot be read during migration**: it is left unchanged and logged, and the migration still completes for the others.
- **External protocols**: request signing, HTTP dates, token expiry and exported telemetry keep whatever format their protocol requires. They are not a second clock people see.

## Requirements *(mandatory)*

### Functional Requirements

**One timezone**

- **FR-001**: A Vizier instance MUST operate in exactly one timezone, the local timezone of the machine or container it runs on, as set through the operating system's standard setting.
- **FR-002**: The instance MUST report its timezone (name and current offset) to its clients and in its startup log.
- **FR-003**: Times the system writes or presents MUST be local with their offset, so each one remains an unambiguous moment. This covers agent-facing text and tool results, REST and WebSocket responses, notifications, channel message metadata (`sent_at`), CLI output and logs.
- **FR-004**: The system MUST NOT label a time as UTC, or present two clocks, on any surface, except where the value itself came from a stored record or caller that names UTC and is shown verbatim in history.
- **FR-005**: The WebUI and the desktop app MUST show and accept times in the instance's timezone, whatever timezone the person's browser is in, including date pickers and relative day labels ("today", "yesterday"). The WebUI MUST make the instance's timezone discoverable, so a person elsewhere can tell that a time is not their own.
- **FR-006**: Ordering, paging, filtering, retention and deduplication of time-stamped records MUST follow real chronological order regardless of the offset a record was written with.
- **FR-007**: Records written before the upgrade MUST remain readable and be presented in local time without being rewritten, except as FR-018 requires.

**Scheduling**

- **FR-008**: Cron expressions, for tasks and for dream schedules, MUST be evaluated in the instance's local timezone.
- **FR-009**: Scheduling inputs from agents, the API and the WebUI MUST accept a local date and time with no offset, and MUST keep accepting a date and time with an offset as that exact moment.
- **FR-010**: Scheduling tool descriptions and confirmations MUST speak in local time, and a confirmation MUST include the weekday and timezone so a wrong date is visible at once.
- **FR-011**: A cron occurrence falling in a daylight-saving gap or overlap MUST fire exactly once; a one-time local time MUST follow the Edge Cases rules.
- **FR-012**: Daily boundaries (Home counts and activity, usage reports, dream nights) MUST follow the instance's local day. A client-supplied UTC offset on these requests MUST be ignored and MUST NOT cause an error, so older clients keep working; this compatibility path is tracked as in FR-021.

**Removing time from context**

- **FR-013**: The per-request context block MUST NOT contain the current date or time on any turn kind.
- **FR-014**: Other per-turn prompt content MUST NOT carry the current time either. Times of past events that are fixed once written, and relative wording in sections that already vary (reactions), remain allowed.
- **FR-015**: An agent's standing instructions MUST tell it that it does not know the current time and how to find out, in text identical across turns.

**Time tools**

- **FR-016**: A new `time` tool category MUST be available to every agent on every turn kind, including dreams and scheduled runs, with no configuration, and MUST provide:
  - the current date, time, weekday, timezone name and offset;
  - conversion of a moment between timezones;
  - date arithmetic: adding a duration to a moment, the difference between two moments, and resolving a next or previous weekday or date, all correct across daylight-saving changes.
- **FR-017**: Time tools MUST be read-only and MUST report invalid input as a correctable result naming the expected form (for an unknown timezone, the nearest names), never by failing the turn. `time` MUST be a reserved category name.

**Migration**

- **FR-018**: On first start after upgrade, every existing **non-cron** task (pending, fired and inactive) MUST be migrated on every supported storage backend so that its schedule is held as local time denoting the same moment.
- **FR-019**: Cron tasks and dream schedules MUST NOT be migrated: expressions stay byte-identical. A cron task whose schedule shifts MUST fire at most one catch-up run because of the upgrade.
- **FR-020**: The migration MUST be idempotent, resumable after interruption, and MUST leave an unreadable row unchanged and logged without stopping startup.
- **FR-021**: All migration and backward-compatibility code from this feature MUST be added to the removal checklist in issue [#82](https://github.com/vizier-lab/vizier/issues/82), naming the code and what depends on it.

**Communication**

- **FR-022**: Release notes and documentation MUST state that cron tasks and dream schedules now mean local time, that their expressions were not changed, and how to set the instance's timezone (including in containers).

### Key Entities

- **Instance timezone**: the one timezone an instance operates in, with a name and a current offset. It comes from the host and is not stored as agent or per-person configuration.
- **One-time task schedule**: a specific moment, held as local time with its offset. Migrated from UTC by this feature.
- **Cron schedule**: an expression evaluated against local wall-clock time, for tasks and dreams. Its text is never rewritten by this feature.
- **Time tool**: an agent capability answering what time it is and working with moments and durations.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On an instance outside UTC, in a scripted run of 10 wall-clock scheduling requests ("tomorrow at 9", "Friday 14:30", ...), every task fires within one minute of the local time requested.
- **SC-002**: Across the WebUI, the CLI, notifications, agent replies and logs, one event shows one local time, and no surface labels a time UTC outside stored history, confirmed by an audit of each surface.
- **SC-003**: Two turns of one conversation at different times, with the same related memories and skills, receive byte-identical context, confirmed with the offline test agent.
- **SC-004**: After the upgrade, provider-reported cached input tokens on follow-up turns are no lower than before in a like-for-like conversation, and the time section no longer contributes to the changing part of any prompt.
- **SC-005**: 100% of readable one-time tasks fire at their original moment after migration, and 0 cron expressions or dream schedules differ from before the upgrade.
- **SC-006**: Running the migration a second time, or resuming it after interruption, changes 0 tasks' moments.
- **SC-007**: Across the two daylight-saving transitions of a test timezone, each cron occurrence fires exactly once, and records list in true chronological order.
- **SC-008**: An agent asked for the current date, a timezone conversion or "next Tuesday" answers correctly in all three, using `time` tools, on the offline end-to-end path.

## Assumptions

- "Local timezone" means the timezone of the machine or container running Vizier, read from the operating system's setting. There is no per-agent or per-person timezone in this feature.
- Moments that are already zone-free (the epoch-millisecond columns) are correct as they are; only their presentation and day grouping change. Non-cron tasks are the only stored data migrated, as requested.
- A one-time task migrates by keeping its moment, not its wall-clock digits: `03:00 UTC` becomes `10:00` in UTC+7, not `03:00` local.
- Time tools are the only source of "now" for agents, as requested. Discord and Telegram messages keep their existing `sent_at`, now local; WebUI messages, which have no `sent_at` today, do not gain one in this feature.
- Scheduled runs and dreams may spend a tool call to learn the time, and that cost is accepted in exchange for a stable prompt.
- A multi-node deployment is one logical instance and its nodes are expected to share a timezone. Fleet roles are still skeletons, so enforcing this is left to the sub-issues that wire them.
- The timezone indicator that FR-005 requires, and any change to the date picker's look, is drawn in FigCraft before code, per the project's design-first rule. A time merely displayed in a different zone needs no design pass.
- End-to-end verification uses the offline dummyplug provider, which the constitution requires for agent-observable changes.
