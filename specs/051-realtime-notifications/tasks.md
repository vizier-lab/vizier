---
description: "Task list for realtime notifications"
---

# Tasks: Realtime Notifications

**Input**: Design documents from `specs/051-realtime-notifications/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/inbox-stream.md](contracts/inbox-stream.md), [quickstart.md](quickstart.md)

**Tests**: Included where the plan's testing section and the constitution's end-to-end gate call for them: pure logic, the stream's delivery rules and the dummyplug script. Run with `cargo test -p vizier-core notifications`, `cargo test -p vizier-http --test inbox_stream` and `cd webui && npm test`.

**Format**: `[ID] [P?] [Story] Description`. `[P]` means different files and no dependency on unfinished tasks.

**Blockers**:

- T002 needs WSL or Linux, because `just checkout` is not supported on Windows.
- T034 needs the FigCraft server, and every design or WebUI task in US2 waits on its review.

**Commit order across repos**: design → webui → vizier.

---

## Phase 1: Setup

**Purpose**: Branches and a clean baseline

- [X] T001 Commit the plan pointer change in `CLAUDE.md` on `051-realtime-notifications`
- [X] T002 Create the matching branch in every submodule with `just checkout 051-realtime-notifications` from WSL or Linux, and confirm with `just status`
- [ ] T003 Record the baseline by running `cargo test --workspace` and `cd webui && npm test`, noting any failure that predates this work in `specs/051-realtime-notifications/quickstart.md`

---

## Phase 2: Foundational

**Purpose**: The columns, event and shared rules every story needs

**⚠️ CRITICAL**: No story can start until this phase is complete.

- [X] T004 [P] Add `Subject::inbox_events()` returning `inbox.events`, with a test, in `crates/vizier-schema/src/subject.rs`
- [X] T005 [P] Add nullable `source_session`, `source_title` and `excerpt` through `add_column_if_missing` in `init_notification_schema` in `crates/vizier-core/src/storage/sqlite/mod.rs`
- [X] T006 [P] Create `crates/vizier-core/src/storage/sql/postgres/migrations/0003_notification_source.sql` adding the same three columns, and register it wherever migrations are listed
- [X] T007 Add `source_session`, `source_title` and `excerpt` to `NotificationDraft`, and `title`, `excerpt` and `source_session` (`serde(skip)`) to `NotificationItem`, omitting absent values when serialized, in `crates/vizier-core/src/schema/notification.rs`, and set the new draft fields to `None` in the `draft` helper in `crates/vizier-core/src/notifications/mod.rs`
- [X] T008 Write and read the new columns in `crates/vizier-core/src/storage/sqlite/notification.rs`, and change the insert to return the rows it actually inserted using `ON CONFLICT DO NOTHING RETURNING id`. Confirm on both SQLite and PostgreSQL; if `SqlConn` can't express it, select by `event_key` after inserting (plan.md, Risks). Update the trait in `crates/vizier-core/src/storage/notification.rs`
- [X] T009 Define `InboxEvent` per data-model.md and add `KIND_INBOX_EVENT`, `Outgoing::Inbox`, a third `select!` arm, a `broadcast` channel, `publish_inbox_event` and `subscribe_inbox_events` in `crates/vizier-core/src/transport/events.rs` and `crates/vizier-core/src/transport/mod.rs`, with the same local-delivery fallback and tests as the session events
- [X] T010 Add a transport slot with `install_transport` to `Notifications`, publish `InboxEvent::Created` for each inserted row after commit in `record` and `record_all`, in `crates/vizier-core/src/notifications/mod.rs`, and install it where the transport is built in `crates/vizier-core/src/dependencies.rs`. A duplicate `event_key` must publish nothing, pinned by a test
- [X] T011 Change `job_ended` to take the origin `&VizierSession` in place of `agent_id` in `crates/vizier-core/src/notifications/mod.rs`, and update the callers in `crates/vizier-core/src/agents/background/mod.rs` and `crates/vizier-core/src/dependencies.rs`, confirming the startup-sweep row exposes its origin
- [X] T012 Extract a `can_see_setup(user, agents, agent_id, owner_id)` rule from `states` in `crates/vizier-core/src/service/setup_state.rs` for the stream to share, with no behaviour change
- [X] T013 Regenerate `crates/vizier-core/public-api.txt` for the changed public types, using the same command CI's check documents

**Checkpoint**: The workspace builds and tests pass. Nothing is visible to a client yet.

---

## Phase 3: User Story 1 - See what happened the moment it happens (Priority: P1) 🎯 MVP

**Goal**: One live connection delivers notifications, setup and read state, and the polls are gone.

**Independent Test**: With the app open on an unrelated page, finish a task run and start an agent that downloads. The count, toast and setup progress update with no page action, over one connection, with no periodic requests (quickstart steps 1 to 5).

### Tests for User Story 1

- [X] T014 [P] [US1] Unit test the delivery decision in `crates/vizier-core/src/service/inbox.rs`: visible agent, hidden agent, `audience_user_id` match and mismatch, setup of a not-yet-stored agent as owner and as stranger, `Read` for another person
- [X] T015 [P] [US1] Unit test in `crates/vizier-core/src/setup.rs` that `download_progress` publishes at most one event per 250 ms per agent, that every other transition publishes at once, and that an entry `snapshot()` no longer returns publishes `SetupCleared`
- [X] T016 [P] [US1] Create `crates/vizier-http/tests/inbox_stream.rs` covering `hello` first, a `notification` frame with counts, `read` across two sockets of one person, revoked access receiving nothing, and a socket staying open past `ws_idle_timeout_secs`
- [X] T017 [P] [US1] Create `webui/app/lib/inboxFrames.test.ts` for each frame type against store state, ignoring unknown types, ETA samples from `setup` frames, and removing a terminal entry after 5 s

### Implementation for User Story 1

- [X] T018 [US1] Make `SetupBoard::publish` also publish `InboxEvent::Setup` with `owner_id` as its own field and `SetupCleared` when pruned, throttling only `download_progress` per research R6, in `crates/vizier-core/src/setup.rs`, and install the transport wherever `install_publisher` is called
- [X] T019 [US1] Create `crates/vizier-core/src/service/inbox.rs` with `snapshot(services, principal)` returning counts, `latest_id` and setups, and `decide(services, principal, event)` returning the frame to send or none, re-resolving the principal per call. Register it in `crates/vizier-core/src/service/mod.rs`
- [X] T020 [US1] Publish `InboxEvent::Read` from `set_read` and `read_all` in `crates/vizier-core/src/service/notifications.rs`
- [X] T021 [US1] Create `crates/vizier-http/src/inbox.rs` with `InboxConnection::open(state, principal, out)` and `run()`: subscribe first, send `hello`, then deliver per `decide`, coalesce count lookups in a burst, and close when the person no longer resolves. Register it in `crates/vizier-http/src/lib.rs`
- [X] T022 [US1] Add `GET /inbox/stream` to `crates/vizier-http/src/api/v1/notifications.rs` and mount it in `crates/vizier-http/src/api/v1/mod.rs`, with a socket loop that pings every 30 s and has **no** idle deadline (research R10), plus the OpenAPI entry in `crates/vizier-http/src/lib.rs`
- [X] T023 [US1] Add the route to `crates/vizier-http/tests/conformance/pending.txt` as the deliberate exception noted in the contract
- [X] T024 [US1] Add `Dispatcher::open_inbox` beside `open_topic` in `crates/vizier-http/src/embedded.rs`
- [X] T025 [US1] Add an `Inboxes` collection beside `Topics` in `app/src-tauri/src/ipc.rs`, add `inbox_open` and `inbox_close` commands in `app/src-tauri/src/shell.rs` to both `generate_handler!` lists, and close them in the same place topics close
- [X] T026 [US1] Generalise `TopicSocket` in `webui/app/api/transport/websocket.ts` so both connections share it, keeping the existing behaviour for chat
- [X] T027 [US1] Add `openInbox` to `Transport` in `webui/app/api/transport/common.ts`, and implement it in `webui/app/api/transport/http.ts`, `webui/app/api/transport/tauri.ts` and `webui/app/api/transport/index.ts`
- [X] T028 [US1] Regenerate `webui/app/api/schema.d.ts` with `scripts/openapi.mjs`, and add `title` and `excerpt` to `webui/app/api/endpoints/notifications.ts`
- [X] T029 [US1] Create `webui/app/lib/inboxFrames.ts` with pure handlers for `hello`, `notification`, `read`, `setup`, `setup_cleared` and `resync`
- [X] T030 [US1] Replace the timer in `start()` with `transport.openInbox` in `webui/app/stores/notifications.ts`, keeping the first-page merge and "fresh" logic for `resync`, and update `webui/app/stores/notifications.test.ts`
- [X] T031 [US1] Remove `pollDelay` from `webui/app/lib/desktop.ts` and `webui/app/lib/desktop.test.ts`
- [X] T032 [US1] Remove the setup interval and `refreshSetups` from `webui/app/stores/agents.ts`, keeping `setups`, `samples` and the health poll, fill them from frames, and add the 5 s terminal timer
- [X] T033 [US1] Create `scripts/inbox-e2e.mjs` on the model of `scripts/client-tools-e2e.mjs` against a dummyplug agent, covering quickstart steps 1 to 4, and run it

**Checkpoint**: The existing panel updates live and the network panel shows no polling. US1 is shippable alone.

---

## Phase 4: User Story 2 - Check notifications in a small popup (Priority: P1)

**Goal**: A minimal popup anchored to the bell replaces the docked column and the phone slide-over.

**Independent Test**: Press the bell on any page at desktop and phone widths. The popup opens next to the bell, the page does not shift, and all three ways of closing work (quickstart step 10).

**Depends on**: Foundational only. It works on the polling store, so it can be built before or after US1.

### Design for User Story 2

- [x] T034 [US2] draw the popup on the notifications surface of `Vizier.figcraft`, in both themes, at desktop and 375 px, and have it reviewed. Match the agreed mockup: filters with icons on one line, setup row, sigil rows with no dot and no dividers, the number on the bell. Decide there how the read toggle is reached on touch and whether the bell keeps its busy indicator
- [x] T035 [P] [US2] Create `design/src/components/NotificationItem/` with the component and stories: sigil, title, two-line excerpt, agent and time, read and unread text, failed tone, no-title form, and a toggle shown on hover and focus and reachable on touch
- [x] T036 [P] [US2] Create `design/src/components/SetupItem/` with the component and stories: sigil, step, size and percent, progress bar, agent and time left, unknown size, and failed
- [x] T037 [US2] Add an indeterminate state to `design/src/components/ProgressBar/ProgressBar.tsx` with a story, if it doesn't draw one already (`ProgressBar` already had an indeterminate state and story; nothing to add)
- [x] T038 [US2] Commit in `design/` and bump the pin in the workspace

### Implementation for User Story 2

- [x] T039 [US2] Add the bell's anchor element and a toggle to `webui/app/stores/notifications.ts`, and narrow the WebUI's `filter` to `unread | all`, keeping the REST `read` filter
- [x] T040 [US2] Register the anchor and call the toggle in `webui/app/screens/shared/NotificationsBell.tsx`, keeping `count`
- [x] T041 [US2] Map rows onto the design component in `webui/app/screens/notifications/NotificationRow.tsx`, falling back to the summary when `title` and `excerpt` are absent
- [x] T042 [US2] Map setup onto the design component in `webui/app/screens/notifications/SetupActivity.tsx`, keeping `eta` and `setupProgress` and showing one row per agent
- [x] T043 [US2] Create `webui/app/screens/notifications/NotificationsPopup.tsx` using `Floating` (`align="end"`, width `min(360, viewport − 16)`) and `useDismiss` with the bell as `ignore`: filter line with mark-all and × icons, setup rows, a scrolling list and "Load older". Add a fixed top-right anchor for when no bell is mounted
- [x] T044 [US2] Delete `webui/app/screens/notifications/NotificationsPanel.tsx`, and in `webui/app/layouts/AppShell.tsx` render one `NotificationsPopup` in both layouts and drop the `aside` prop and `SlideOver` panel
- [ ] T045 [P] [US2] Test in `webui/app/screens/notifications/NotificationsPopup.test.tsx`: the order of sections, all three dismissals, the bell toggling, and opening with no bell mounted
- [ ] T046 [US2] Check `webui/app/screens/shared/NotificationToasts.tsx`, `webui/app/screens/chat/AgentStateBanner.tsx` and `webui/app/root.tsx` still open the popup, then run `cd webui && npm run typecheck && npm test` and quickstart step 10

**Checkpoint**: The popup works in both themes and at both widths, with 0 pixels of page movement.

---

## Phase 5: User Story 3 - Know what a notification is about (Priority: P2)

**Goal**: Rows lead with a title and a short excerpt.

**Independent Test**: Have an agent reply in two titled conversations. Each row shows the right title and a readable excerpt (quickstart steps 2 and 8).

### Tests for User Story 3

- [X] T047 [P] [US3] Create tests in `crates/vizier-core/src/notifications/excerpt.rs`: Markdown removed, code blocks, links and tables, a multi-byte cut at 140 characters, an all-code reply, and the "Sent a page", "Sent N files" and "Sent a voice reply" forms
- [X] T048 [P] [US3] Add tests in `crates/vizier-core/src/notifications/mod.rs`: chat, task and job rows carry the right fields, `summary` stays free of model output, and an excerpt built from a saved entry holds `[redacted]`, not the secret (SC-008)
- [X] T049 [P] [US3] Add service tests in `crates/vizier-core/src/service/notifications.rs`: a title follows a rename, a missing title reads "New conversation", another person's conversation yields neither title nor excerpt, and a deleted conversation's rows are gone

### Implementation for User Story 3

- [X] T050 [US3] Create `crates/vizier-core/src/notifications/excerpt.rs` per research R7 using `pulldown-cmark`, and register it in `crates/vizier-core/src/notifications/mod.rs`
- [X] T051 [US3] Add a read of one history entry by uid to `crates/vizier-core/src/storage/history.rs` and `crates/vizier-core/src/storage/sqlite/history.rs`, with a stub or real read in `crates/vizier-core/src/storage/fs/history.rs` as that migration-only backend requires
- [X] T052 [US3] In `chat_replied`, build the excerpt from the saved entry and set `source_session`, in `crates/vizier-core/src/notifications/mod.rs`, passing what it needs from the call in `crates/vizier-core/src/agents/process.rs`
- [X] T053 [US3] In `task_run_ended`, set `source_title` and the excerpt from `storage::task_run::task_run_response`, in `crates/vizier-core/src/notifications/mod.rs`, with no change for its callers
- [X] T054 [US3] In `job_ended`, set `source_session` and use the outcome text as the excerpt, in `crates/vizier-core/src/notifications/mod.rs`
- [X] T055 [US3] Fill `title` once per distinct session on the page and clear `title` and `excerpt` for another person's conversation, in `list` in `crates/vizier-core/src/service/notifications.rs`, and apply the same fill and gate to `Created` in `decide` in `crates/vizier-core/src/service/inbox.rs`
- [X] T056 [US3] Delete rows whose `source_session` matches in the transaction of `delete_session` in `crates/vizier-core/src/storage/sqlite/session.rs`
- [X] T057 [US3] Publish `InboxEvent::Stale` after a conversation or agent delete, from `delete_conversation` in `crates/vizier-core/src/session_files/mod.rs` and from agent deletion, and for the conversation's person when a title arrives at `SessionFrame::Title` in `crates/vizier-core/src/agents/process.rs`. Map `Stale` to the `resync` frame in `decide`
- [ ] T058 [US3] Extend `scripts/inbox-e2e.mjs` for the title, excerpt and deletion steps (quickstart steps 2 and 8)

**Checkpoint**: Titles and excerpts appear over REST and the stream, and only for people allowed to see them.

---

## Phase 6: User Story 4 - Keep external channel activity out (Priority: P2)

**Goal**: Discord, Telegram and future platforms create no conversation notifications.

**Independent Test**: A WebUI reply notifies, while a Discord or Telegram reply and a background job launched there do not (quickstart step 9).

**Depends on**: T011 only.

### Tests for User Story 4

- [X] T059 [P] [US4] Add tests in `crates/vizier-core/src/notifications/mod.rs`: `job_ended` with a Discord or Telegram origin records nothing, with a WebUI origin it records one row, and an integration change on a Discord connection still records. Keep the existing `chat_replied` Discord assertion and correct its comment
- [X] T060 [P] [US4] Add a schema test in `crates/vizier-core/src/storage/sqlite/notification.rs`: `channel_replies` rows and their read rows are gone after init and the counts agree

### Implementation for User Story 4

- [X] T061 [US4] Return early from `job_ended` when `origin.1.platform().is_some()` in `crates/vizier-core/src/notifications/mod.rs`
- [X] T062 [US4] Remove `project_replies`, `REPLIES_THROUGH`, `HOUR_MS`, `CATCH_UP_HOURS` and their tests from `crates/vizier-core/src/notifications/mod.rs`, and `channel_replies` and its tests from `crates/vizier-core/src/notifications/text.rs`, along with `channel_href` if nothing else uses it
- [X] T063 [US4] Remove the `project` helper and its calls from `crates/vizier-core/src/service/notifications.rs`
- [X] T064 [US4] Remove `ChannelReplies` and `ReplyBatch` from `crates/vizier-core/src/schema/notification.rs` and its re-export in `crates/vizier-core/src/schema/mod.rs`, and `channel_reply_batches` from `crates/vizier-core/src/storage/notification.rs` and `crates/vizier-core/src/storage/sqlite/notification.rs`
- [X] T065 [US4] Add the idempotent `DELETE FROM notification WHERE category = 'channel_replies'` to `init_notification_schema` in `crates/vizier-core/src/storage/sqlite/mod.rs` and to `crates/vizier-core/src/storage/sql/postgres/migrations/0003_notification_source.sql`
- [X] T066 [US4] Remove `project_replies` from the `control` role's description in `crates/vizier-fleet/src/control.rs`
- [X] T067 [US4] Regenerate `crates/vizier-core/public-api.txt` and `webui/app/api/schema.d.ts`, and remove `channel_replies` from `webui/app/api/endpoints/notifications.ts`

**Checkpoint**: A workspace with old rows upgrades cleanly with correct counts.

---

## Phase 7: User Story 5 - Stay correct through disconnects (Priority: P3)

**Goal**: The inbox recovers by itself after any drop.

**Independent Test**: Disconnect, record notifications and finish a setup, then reconnect. Every missed row appears once, the count is right, and the finished setup no longer shows as running (quickstart step 6).

**Depends on**: US1.

- [X] T068 [P] [US5] Test in `webui/app/stores/notifications.test.ts`: `hello` ahead of the mark fetches once and raises one burst toast, `hello` level with it fetches nothing, a failed fetch keeps the last list, and the second `hello` never re-toasts
- [X] T069 [P] [US5] Test in `crates/vizier-http/tests/inbox_stream.rs`: a lagged subscriber gets `resync`, and a socket closes at the token's expiry
- [X] T070 [US5] Handle `hello` when `latest_id` is above `lastSeenId` through the existing "fresh" path, priming only on the first one, in `webui/app/stores/notifications.ts`
- [X] T071 [US5] Send `resync` on a `broadcast` `Lagged` in `crates/vizier-http/src/inbox.rs`
- [X] T072 [US5] Pass the token's expiry from `require_auth` as a request extension in `crates/vizier-http/src/auth/middleware.rs`, and close the socket at that time in `crates/vizier-http/src/api/v1/notifications.rs`
- [X] T073 [US5] After `OFFLINE_AFTER` failures make one `GET /notifications/count`, so a real sign-out goes through `settle`'s 401 handling, in `webui/app/stores/notifications.ts`
- [X] T074 [US5] Reopen the inbox when the engine restarts, as topics do, in `webui/app/api/transport/tauri.ts`, and check that `app/src-tauri/src/ipc.rs` closes inboxes on restart
- [ ] T075 [US5] Extend `scripts/inbox-e2e.mjs` for reconnect (quickstart step 6) and run quickstart steps 5 and 11

**Checkpoint**: Every story works, including across a dropped connection and an engine restart.

---

## Phase 8: Polish

**Purpose**: Documentation, tracking and the full verification pass

- [X] T076 [P] Document the stream, its frames and the new item fields in `docs/src/api-integration/rest-api.md`, and remove the `channel_replies` category and projection sentence
- [X] T077 [P] Add `inbox.events` to the subject table in `docs/src/fleet/bus-contract.md`
- [X] T078 [P] Add excerpts to the readable-data list in `docs/src/configuration/encryption.md`, and state how live notifications arrive in `docs/src/getting-started/desktop-app.md`
- [X] T079 Update the Notifications section of `CLAUDE.md`: the stream, the bus subject, the popup, titles and excerpts, and no external channels, and correct its stale description of the panel
- [ ] T080 Ask the person to approve, then add #82 entries for the SQLite `channel_replies` delete, the stale `notifications.replies_through` state key and the unpolled `GET /utils/setup-state` route
- [ ] T081 Run `cargo clippy --workspace`, `cargo test --workspace`, `cargo test -p vizier-core --no-default-features`, `cd app && cargo test`, and `cd webui && npm run typecheck && npm test`
- [ ] T082 Run all of `specs/051-realtime-notifications/quickstart.md` on the running binary, and mark steps 9.1 to 9.3 as not run unless Discord or Telegram credentials were available
- [ ] T083 Commit across repos in the order design → webui → vizier, and bump both submodule pins

---

## Dependencies & Execution Order

```text
Setup → Foundational ─┬─▶ US1 ──▶ US5
                      ├─▶ US2  (blocked on T034, FigCraft)
                      ├─▶ US3  (T055 and T057 touch files from US1: do those after T019)
                      └─▶ US4
All stories ──▶ Polish
```

- **US1**: needs only Foundational. It is the MVP.
- **US2**: needs only Foundational, plus T034's review. It works on either the polling or the stream store, but T039 edits the same store as T030, so don't run those two at once.
- **US3**: can start after Foundational. Its row display arrives with US2; until then it shows through REST and frames.
- **US4**: needs T011. It touches `notifications/mod.rs` and `service/notifications.rs`, as US3 does, so run them one at a time.
- **US5**: needs US1.

### Parallel opportunities

- **Foundational**: T004, T005 and T006 together, across three crates or files.
- **US1 tests**: T014, T015, T016 and T017 together. After T021, the Rust (T022 to T025) and WebUI (T026 to T032) tracks can run side by side.
- **US2**: T035 and T036 together after T034.
- **US3 tests**: T047, T048 and T049 together.
- **Polish**: T076, T077 and T078 together.

Example for US1 tests:

```text
T014 service/inbox.rs delivery decisions
T015 setup.rs throttle
T016 tests/inbox_stream.rs
T017 webui/app/lib/inboxFrames.test.ts
```

## Implementation Strategy

**MVP**: Setup, Foundational and US1. The existing panel goes live and both polls disappear, which is the core complaint, and it doesn't wait on FigCraft.

**Incremental delivery**:

1. Foundational + US1: realtime, in today's panel.
2. US4: the quick noise win, and mostly deletion.
3. US3: titles and excerpts, through REST and frames.
4. US5: recovery hardening.
5. US2: the popup, as soon as FigCraft is back and the drawing is reviewed. If FigCraft returns early, do this sooner, since it's what people will see most.

Each step can merge on its own. US3's excerpts only become visible on screen with T041, so if US2 is last, add them to the existing row in that step instead of waiting.
