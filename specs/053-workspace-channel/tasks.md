---
description: "Task list for the Workspace channel and mid-turn messages"
---

# Tasks: Workspace Channel

**Input**: Design documents from `specs/053-workspace-channel/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [quickstart.md](quickstart.md), [mockup.html](mockup.html), contracts: [http-api.md](contracts/http-api.md), [socket-frames.md](contracts/socket-frames.md), [agent-tools.md](contracts/agent-tools.md), [turn-rendering.md](contracts/turn-rendering.md)

**Tests**: Included where the constitution and the plan call for them: pure logic, access rules, conformance vectors for every route, and the dummyplug end-to-end script.

**Format**: `[ID] [P?] [Story] Description`. `[P]` means different files and no dependency on unfinished tasks.

**Path shorthand**: `core/` is `crates/vizier-core/src/`, `http/` is `crates/vizier-http/src/`.

**Commit order across repos**: design → webui → vizier.

**Story order**: US7 comes first though it is P2, because the plan ships it first and the Workspace topic screen reuses its turn model. Then by priority. US5 is one delivery with US2: the old tools are removed only once their replacement works.

---

## Phase 1: Setup

**Purpose**: Branches and a clean baseline

- [ ] T001 Commit the plan pointer change in `CLAUDE.md` on `053-workspace-channel`
- [ ] T002 Check the uncommitted `package-lock.json` changes in `webui/` and `landing/` with `just status`, ask the owner what to do with them, then create the matching branch everywhere with `just checkout 053-workspace-channel`
- [ ] T003 Record the baseline by running `cargo test --workspace` and `cd webui && npm test`, noting any failure that predates this work in `specs/053-workspace-channel/quickstart.md`
- [ ] T004 [P] Confirm that `save_session_file` in `core/storage/session_file.rs` accepts an existing `file_id` (research R9). If not, record the fallback in `specs/053-workspace-channel/research.md` before T016

---

## Phase 2: Foundational

**Purpose**: Channel identity, storage, access, and the routes every story needs. No agent activation yet.

**Blocks**: all stories except US7.

- [ ] T005 Add `VizierChannelId::Workspace(String)` with slug `workspace__{id}` in `core/schema/session.rs`, covering `to_slug`, `usage_place` (`workspace:{id}:{topic}`) and `usage_activity`, and the span recording match in `core/agents/agent/mod.rs`
- [ ] T006 Add `UsageActivity::Workspace`, labelled **Workspaces**, in `core/schema/usage.rs`, and `workspace__` in `slug_kind` in `core/usage/mod.rs`
- [ ] T007 Review each slug prefix check named in research R8 and exclude Workspace sessions from agent conversation pages: `core/service/conversations.rs` (`kind_of`), `core/service/topics.rs` (`readable_channel`), `core/access.rs`, `core/storage/session.rs`, `core/storage/sqlite/session.rs`. Leave `is_non_user_channel` in `core/storage/history.rs` false so dreams reflect on it
- [ ] T008 Regenerate `crates/vizier-core/public-api.txt`
- [ ] T009 Add `WorkspaceStorage` in `core/storage/workspace.rs` per `specs/053-workspace-channel/data-model.md`, and add it to `VizierStorageProvider` in `core/storage/mod.rs`
- [ ] T010 Implement `init_workspace_schema` and `impl WorkspaceStorage` in `core/storage/sqlite/workspace.rs` (tables `workspace`, `workspace_topic`, `workspace_member`, `workspace_entry`, `workspace_mention`, `workspace_read`, with cascade and `(workspace_id, handle)` unique), and register it in the list in `core/storage/sqlite/mod.rs`
- [ ] T011 Add `session_history(channel, topic, timestamp)` index, and nullable `workspace_id` and `author` on `notification`, in `core/storage/sqlite/mod.rs` and `core/storage/sqlite/notification.rs`
- [ ] T012 Add `core/storage/sql/postgres/migrations/0004_workspaces.sql` with the same schema, and its entry in `core/storage/sql/postgres/migrate.rs`
- [ ] T013 [P] Add the name rule with tests in `core/workspaces/names.rs`: 1 to 64 characters, `^[a-z0-9][a-z0-9-]*$`, for Workspaces and topics
- [ ] T014 Add `workspaces:create` and `all_workspaces:manage` to `AVAILABLE_PERMISSIONS` in `core/storage/user.rs`, with labels in `webui/app/lib/permissions.ts`, and decide which seeded roles get them
- [ ] T015 Add `core/workspaces/access.rs` on the pattern of `core/projects/access.rs`, the only place deciding member, owner and `all_workspaces:manage`, with tests for all three fixture kinds
- [ ] T016 Add `core/service/workspaces.rs` and register it in `core/service/mod.rs`: create, get, list, topics create, members add and remove (`can_edit_agent` for agents, `409 handle_in_use`, `409 last_owner`), mapping failures onto `ServiceError`. A non-member gets `not_found` everywhere
- [ ] T017 Add `http/api/v1/workspaces.rs` with the create, list, detail, topic and member routes from `specs/053-workspace-channel/contracts/http-api.md`, translating only, and nest it in `http/api/v1/mod.rs`
- [ ] T018 Add `crates/vizier-http/tests/conformance/groups/workspaces.rs` with a vector per route for three fixture users, register it, and record with `CONFORMANCE_RECORD=1`. `tests/conformance/pending.txt` must not grow

**Checkpoint**: Workspaces, topics and members exist over HTTP, with access rules pinned. Nothing wakes yet.

---

## Phase 3: User Story 7 - See what an agent says while it works (P2, ships first) 🎯

**Goal**: Mid-turn text appears as messages in every WebUI and desktop chat: trail, message, trail, final.

**Independent Test**: Quickstart §12. A turn writing text between two tool rounds shows as its own message between separate trails, live and after a reload, including in a conversation saved earlier.

**Depends on**: nothing, so this can merge as its own PR. T019 and T020 are in `crates/`; everything else is in `webui/`.

- [ ] T019 [US7] Add the additive `say` key to `crates/vizier-core/src/agents/agent/model/dummyplug.rs`, emitting the text in the same model message as the tool call, with a unit test
- [ ] T020 [P] [US7] Document `say` in `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`
- [ ] T021 [US7] Write failing vitest cases for blocks in `webui/app/lib/trail.test.ts`: text between rounds, text first, no empty trail, `ToolResult` pairing across blocks, a stopped turn, per-block time, old saved history
- [ ] T022 [US7] Change `Turn` to `blocks: TurnBlock[]` in `webui/app/api/frames.ts` and implement it in `groupHistory` in `webui/app/lib/trail.ts` per `specs/053-workspace-channel/contracts/turn-rendering.md`. Remove `narration` from `TrailEvent` and `note` from `trailSummary`, and delete the stale comment on `liveEvent`
- [ ] T023 [US7] Draw blocks in `block` and the live branch of `webui/app/screens/chat/MessageList.tsx`: one trail per block, live and expanded only for the last while running, a message at `type-prose` with no stats, reactions or copy
- [ ] T024 [US7] Give each trail its own time and counts in `webui/app/screens/chat/Trail.tsx`, and keep total time on the final message's stats in `webui/app/lib/messages.ts`
- [ ] T025 [US7] Update tests that assume one trail per turn in `webui/app/screens/chat/MessageList.test.tsx`, and run `cd webui && npm test && npm run typecheck`
- [ ] T026 [US7] Run quickstart §12 against a dummyplug agent and record the result in `specs/053-workspace-channel/quickstart.md`

**Checkpoint**: Mid-turn messages work everywhere. Can ship alone.

---

## Phase 4: User Story 1 - Talk to several agents in one shared conversation (P1) MVP

**Goal**: A shared topic where agents read silently and answer when mentioned, with skills and commands, on a real screen.

**Independent Test**: Quickstart §2 to §4, §5a, §5b, §6. An unmentioned message makes no reply and no model call. A mentioned agent answers, knowing what it read. Both people see the same conversation.

### Tests

- [ ] T027 [P] [US1] Unit tests in `core/workspaces/mentions.rs`: code spans, fenced blocks, quoted lines, longest handle, punctuation, repeats, non-members
- [ ] T028 [P] [US1] Unit tests in `core/workspaces/commands.rs`: every row of the table in research R19, including `@a /foo`, `/foo please @a` and `/skill name @a input`

### Engine

- [ ] T029 [US1] Implement `mentions::parse` in `core/workspaces/mentions.rs` and `commands::parse` in `core/workspaces/commands.rs`, both pure
- [ ] T030 [US1] Insert a `workspace_entry` inside `save_session_history` in `core/storage/sqlite/history.rs`, only for `AssistantMessage` and a `Response` with a message on a `Workspace` channel, in the existing transaction, resolving mentions against current members into `workspace_mention` (research R3, R4). Check that PostgreSQL shares this path
- [ ] T031 [US1] Implement `workspaces::post` in `core/workspaces/mod.rs`: one transaction for entry and mentions, then publish and dispatch. Validate the 32,000 character limit
- [ ] T032 [US1] Implement dispatch in `core/workspaces/mod.rs`: a `Chat` request through `deps.agents.request` with leading mentions removed, `metadata.workspace_entry`, `activated_by_seq`, and `workspace_plain` unless it is the command form, republishing frames as `core/agents/background/mod.rs` does. Move mentions to `woken` or `unavailable`
- [ ] T033 [US1] Skip `invocation::parse` when `metadata.workspace_plain` is set, in the Chat arm of `core/agents/process.rs`, with a test that `@a /foo` runs no skill
- [ ] T034 [US1] Implement `core/workspaces/catch_up.rs`: copy unread `person` and `agent` entries up to the activating entry into the agent's session as `Request(SilentRead)` with the author as `user`, skipping its own `source_session`, moving `workspace_read` in the same transaction, capped at 500. Include tests that nothing is missed or repeated, and that another agent's tool calls never enter
- [ ] T035 [US1] Call `catch_up` at the start of a Workspace turn in `chat` in `core/agents/agent/mod.rs`, before history is read, and not on a resumed turn. Share any files per research R9
- [ ] T036 [US1] Add the `## Workspace` section to `context_md_with` in `core/agents/agent/system_prompt/context.rs`: name, topic, members with handles, who mentioned the agent, the mention rule, and how many earlier messages were skipped
- [ ] T037 [US1] Set `read_seq` to the head when an agent joins or a topic is created, in `core/service/workspaces.rs`
- [ ] T038 [US1] Skip titling for `Workspace` in `refresh_session_detail` in `core/agents/process.rs`
- [ ] T039 [US1] Admit `Workspace` for `Chat` in `is_interactive_turn` in `core/agents/tools/mod.rs`, refuse it in `available` in `core/questions/tool_sensitive.rs`, and exclude it from `job_ended` in `core/notifications/mod.rs`
- [ ] T040 [US1] Call `workspaces::after_save` from `record_step` in `core/agents/steps.rs` and from the reply save in `core/agents/agent/mod.rs`, to publish and dispatch a saved message's mentions
- [ ] T041 [US1] Implement commands in `core/workspaces/mod.rs` per research R19: `/abort @a`, `/checkpoint @a`, `/lobotomy @a` (with its permission), refusals for `/btw` and `/clear-files`, `note` entries, people only
- [ ] T042 [US1] Implement stop in `core/workspaces/mod.rs` per research R7: `stopped_at_seq`, drop `pending` mentions, abort each session, one agent or all, and apply the guard in dispatch. `/abort` alone calls this
- [ ] T043 [US1] Re-dispatch `pending` mentions at startup, in `core/dependencies.rs` or `core/runtime.rs`, after the agent host can accept requests

### Transport and routes

- [ ] T044 [US1] Add `workspace_entry`, `workspace_mentions` and `member_changed` as `workspace.{id}.events` in `core/transport/events.rs`, with the local fan-out
- [ ] T045 [US1] Add `http/workspace_topic.rs`: `WorkspaceConnection` with `open`, `handle_inbound`, `outbound().run()` and `close`, the session filter tagged with `agent_id`, per-delivery membership checks, no idle deadline, `hello`, `resync`, per `specs/053-workspace-channel/contracts/socket-frames.md`
- [ ] T046 [US1] Add timeline, post, upload, stop, commands and the stream routes in `http/api/v1/workspaces.rs`, with the timeline merge in `core/service/workspaces.rs`: page `workspace_entry`, then one query for steps in its span
- [ ] T047 [US1] Add `Dispatcher::open_workspace` in `http/embedded.rs` and `workspace_open`/`workspace_close` IPC in `app/src-tauri/src/ipc.rs`
- [ ] T048 [US1] Add conformance vectors for the new routes in `crates/vizier-http/tests/conformance/groups/workspaces.rs`

### Design system

- [ ] T049 [P] [US1] Add `side="left"` and a mentioned highlight to `design/src/components/UserMessage/UserMessage.tsx`, with stories
- [ ] T050 [P] [US1] Add `MentionChip` in `design/src/components/MentionChip/`, `MentionNote` in `design/src/components/MentionNote/` and `OriginBadge` in `design/src/components/OriginBadge/`, with stories
- [ ] T051 [P] [US1] Extend `design/src/components/MentionMenu/MentionMenu.tsx` for agents and people with a sigil or avatar and status, and `design/src/components/SkillMenu/SkillMenu.tsx` to name the agents that have a skill, with stories
- [ ] T052 [US1] Add a `workspaces` prop to `design/src/components/Shell/Rail.tsx` per the mockup: initials tile, unread count, working dot, marker, separators, one `+` through `onCreate`. Draw `StatusDot` only for `starting`, `retrying`, `failed` and `stopped`, and no lit sigil cell. Update `Shell.stories.tsx`
- [ ] T053 [US1] Add `CreateChooser` in `design/src/components/CreateChooser/`, with stories, closing on ×, Escape and an outside press

### WebUI

- [ ] T054 [US1] Add `webui/app/api/endpoints/workspaces.ts` and frame types in `webui/app/api/frames.ts`, regenerate `webui/app/api/schema.d.ts` with `scripts/openapi.mjs`, and add socket and IPC paths inside `webui/app/api/transport/` only
- [ ] T055 [US1] Add `webui/app/lib/workspaces.ts` with vitest cases: merge entries and steps by time into people's messages, notes and agent runs, turn identity, a repeated head where another author lands, `mention_agent` entries as `message` blocks
- [ ] T056 [US1] Add `webui/app/stores/workspaces.ts`: Workspaces for the rail, the open topic, frames, recovery by `seq`
- [ ] T057 [US1] Add routes in `webui/app/routes.ts` and `webui/app/routes/workspace.tsx`, and show Workspaces in `webui/app/screens/shell/ShellRail.tsx` with a topics sidebar in `webui/app/screens/workspaces/WorkspaceSidebar.tsx`
- [ ] T058 [US1] Add the topic screen in `webui/app/screens/workspaces/WorkspaceTopic.tsx`, `WorkspaceHeader.tsx` and `WorkspaceTimeline.tsx`, reusing `Trail` and the US7 blocks, per the mockup: Stop all while working, member stack, the failed mention note, centred notes, and the phone layout
- [ ] T059 [US1] Add `webui/app/screens/workspaces/WorkspaceComposer.tsx`: `@` menu, `/` menu then agents-only `@`, the attachment notice, and refused commands explained in place

### Check

- [ ] T060 [US1] Add `scripts/workspace-e2e.mjs` with its `execute_python` helper, scripting quickstart §1 to §4, §5a, §5b, §6 and §7 against a dummyplug agent, and run it
- [ ] T061 [US1] Add a test in `crates/vizier-core/tests/` that a Workspace silent read creates no `session_turn` row and no `usage_event`

**Checkpoint**: The MVP. People and agents share a topic, on screen, in both themes.

---

## Phase 5: User Story 2 - Agents coordinate through mentions (P1)

**Goal**: An agent's message wakes another, in the open, by text or with `mention_agent`.

**Independent Test**: Quickstart §5. Both ways wake B the same. B's reply wakes A only if it mentions A, and A has read it either way.

- [ ] T062 [US2] Add `ToolCategory::Workspaces`, category name `workspaces`, in `core/agents/tools/mod.rs`, reserved like other built-ins, with a description line
- [ ] T063 [US2] Add `mention_agent` in `core/agents/tools/workspaces.rs` per `specs/053-workspace-channel/contracts/agent-tools.md`, with `available` true only in a `Workspace` session, placing mentions after a leading `/command`. It inserts `workspace_entry` directly with no `history_uid`, so it never touches history (research R6). Refusals are `status` results
- [ ] T064 [US2] Register it on `default_toolset` in `core/agents/tools/mod.rs`, add it to `every_tool_is_in_its_category`, and confirm it is absent from `DREAM_TOOL_NAMES`
- [ ] T065 [US2] Test in `core/agents/tools/workspaces.rs`: non-member, self, all unavailable, partly unavailable, and that a call during a running `execute_python` adds no history row
- [ ] T066 [US2] Make `groupHistory`'s run building in `webui/app/lib/workspaces.ts` keep a python call paired across a `mention_agent` message, with a test
- [ ] T067 [US2] Extend `scripts/workspace-e2e.mjs` with quickstart §5 and run it

**Checkpoint**: Agents coordinate in a Workspace.

---

## Phase 6: User Story 5 - Upgrade from `consult_agent` and `delegate_agent` (P2)

**Goal**: The old tools are gone, fail helpfully, and old data still reads.

**Independent Test**: Quickstart §9. Neither tool is offered, a script gets the new guidance, and saved delegation history reads as before.

**Depends on**: US2. Land together with it, and never before.

- [ ] T068 [US5] Delete `core/agents/tools/consult/`, and `ConsultAgent`, `DelegateAgent` and `other_agents` in `core/agents/tools/mod.rs`, including the `every_tool_is_in_its_category` table and `entry::<DelegateAgent>()`
- [ ] T069 [US5] Add both names to `RETIRED_TOOL_NAMES` in `core/agents/tools/mod.rs`, with the text in `specs/053-workspace-channel/contracts/agent-tools.md` and a test
- [ ] T070 [US5] Update the `Agents` category description in `core/agents/tools/mod.rs` and `list_background_jobs`'s description in `core/agents/tools/background_jobs.rs`, and remove the labels in `core/utils/mod.rs`
- [ ] T071 [US5] Keep `InterAgent`, `JobKind::Delegation` and `delegated_to` compiling for reads, and adjust `launch` in `core/agents/background/mod.rs` if it is now the only constructor path, with no `allow(dead_code)` where a read path exists
- [ ] T072 [US5] Keep `delegate_agent` in `JOB_LAUNCHING_TOOLS` in `webui/app/lib/trail.ts` with a comment for saved history, and confirm `webui/app/screens/chat/MessageList.tsx` and `ChatStatusStrip.tsx` still render an old delegation
- [ ] T073 [US5] Add a checklist entry to issue #82 naming each kept item from `specs/053-workspace-channel/data-model.md`. This writes to GitHub, so confirm the text with the owner first
- [ ] T074 [US5] Extend `scripts/workspace-e2e.mjs` with quickstart §9 and run it

**Checkpoint**: The breaking change is complete and fails helpfully.

---

## Phase 7: User Story 6 - Orchestrate a Workspace from outside it (P2)

**Goal**: An agent reads and posts from any conversation, including scheduled tasks.

**Independent Test**: Quickstart §8. A task run posts with its origin shown and wakes B. A non-member gets the same result as a made-up name.

**Depends on**: US2 (category and tool file).

- [ ] T075 [US6] Add `send_message_to_workspace`, `list_workspaces` and `read_workspace_messages` in `core/agents/tools/workspaces.rs` per the contract, with `available` false in `Dream` and `Subagent` sessions, a topic created on send, and `not_found` identical for non-members
- [ ] T076 [US6] Set `origin` from the caller's session in `core/workspaces/mod.rs`: task slug, chat, Discord, Telegram or Workspace, with none when sent from the same topic
- [ ] T077 [US6] Register the three tools in `core/agents/tools/mod.rs`, add them to `every_tool_is_in_its_category`, and test availability in each conversation kind
- [ ] T078 [US6] Run `catch_up` for idle Workspace sessions before a dream cycle in `core/scheduler/dream/mod.rs`, skipping a session with a turn running, so dreams see what was read
- [ ] T079 [US6] Draw the origin badge in `webui/app/screens/workspaces/WorkspaceTimeline.tsx`
- [ ] T080 [US6] Extend `scripts/workspace-e2e.mjs` with quickstart §8 and §11's dream step, and run it

**Checkpoint**: Scheduled tasks can orchestrate a Workspace.

---

## Phase 8: User Story 3 - Be told about new messages, and mention people (P2)

**Goal**: Every message notifies every other person in the Workspace, and mentions are highlighted.

**Independent Test**: Quickstart §10. One notification per message, none for the author, none for notes, read when the topic is open, and gone on removal.

- [ ] T081 [US3] Add `NotificationCategory::WorkspaceMessage` in `core/schema/notification.rs` with `as_str` and `parse`, wording in `core/notifications/text.rs`, and handle it wherever categories are matched in `core/service/inbox.rs`, `core/transport/events.rs` and `core/storage/sqlite/notification.rs`
- [ ] T082 [US3] Create one notification per `person` and `agent` entry in `core/workspaces/mod.rs` with `event_key = workspace:{seq}`, including the startup sweep. Never for `note`
- [ ] T083 [US3] Add the visibility clause `workspace_id IN (my Workspaces) AND author != me` to list, counts, per-id read and mark-all in `core/storage/sqlite/notification.rs`, with tests for author, member, non-member and removed member (research R20)
- [ ] T084 [US3] Apply the same rule in `judge` in `core/service/inbox.rs`, and fill the title and excerpt for Workspace rows
- [ ] T085 [US3] Add `POST /workspaces/{id}/topics/{tid}/read` in `http/api/v1/workspaces.rs` and `core/service/workspaces.rs`, return `unread` per Workspace and topic from `GET /workspaces`, and add its vector
- [ ] T086 [US3] Delete a topic's or Workspace's notifications with it in `core/service/workspaces.rs`
- [ ] T087 [US3] Link a row to its topic and auto-mark read while viewing in `webui/app/stores/notifications.ts` and `webui/app/lib/notifications.ts`, and show the row in `webui/app/screens/notifications/NotificationsPopup.tsx` with the author's sigil or avatar
- [ ] T088 [US3] Update rail and sidebar counts from frames in `webui/app/stores/workspaces.ts`, and highlight a mention of the signed-in person in `webui/app/screens/workspaces/WorkspaceTimeline.tsx`
- [ ] T089 [US3] Extend `scripts/workspace-e2e.mjs` with quickstart §10 and run it

**Checkpoint**: People hear about messages when away.

---

## Phase 9: User Story 4 - Manage a Workspace and who is in it (P2)

**Goal**: Create, rename, delete, and search-first membership, with the access boundary enforced.

**Independent Test**: Quickstart §1. Non-members get `404` everywhere, removal takes effect at once, and adding starts from search.

- [ ] T090 [US4] Add rename, delete, role change and remove to `core/service/workspaces.rs` and `http/api/v1/workspaces.rs`: `409 name_in_use`, `409 last_owner`, `409 last_topic`, topic rename and delete, stopping turns first, and deleting all agents' sessions by channel slug per `specs/053-workspace-channel/data-model.md`
- [ ] T091 [US4] Close a removed person's sockets and abort a removed agent's turns in `core/workspaces/mod.rs` and `http/workspace_topic.rs` (SC-006)
- [ ] T092 [US4] Remove `workspace_member` and `workspace_read` rows in `delete_agent`, keeping entries with `author_name`, and return `available: false` on timeline authors
- [ ] T093 [US4] Add `GET /workspaces/{id}/candidates` in `core/service/workspaces.rs` and `http/api/v1/workspaces.rs` per research R15, with name search in `core/storage/user.rs`, capped results, and addable first
- [ ] T094 [US4] Add vectors for each new route, including `candidates` with an agent the caller can't view, in `crates/vizier-http/tests/conformance/groups/workspaces.rs`, and pin in `crates/vizier-http/tests/` that someone who can view an agent gets no Workspace session content through its conversation routes
- [ ] T095 [P] [US4] Add `MemberSearch` in `design/src/components/MemberSearch/`, with stories, built like `Picker`
- [ ] T096 [US4] Add the members dialog in `webui/app/screens/workspaces/MembersDialog.tsx`: search first, member list when empty, fixed height, the FR-005 warning before adding an agent, keyboard, and dismissal on ×, Escape and an outside press
- [ ] T097 [US4] Add `webui/app/screens/workspaces/NewWorkspaceDialog.tsx`, the empty topic state, and topic create and rename, and wire `CreateChooser` in `webui/app/screens/shell/ShellRail.tsx`, skipping it when the person may create only one, and hiding `+` when neither
- [ ] T098 [US4] Extend `scripts/workspace-e2e.mjs` with quickstart §1 and removal, and run it

**Checkpoint**: The full feature works.

---

## Phase 10: Polish

- [ ] T099 [P] Add `docs/src/configuration/workspaces.md` and its line in `docs/src/SUMMARY.md`: members, mentions, the command format, notifications, what sharing an agent means, the accepted risks, and the naming difference from the data folder
- [ ] T100 [P] Update `docs/src/configuration/tools-embedding.md`, `docs/src/configuration/agents.md` and `docs/src/configuration/observability.md` for the removed and new tools and the Workspaces usage activity
- [ ] T101 Add a Workspace section to `CLAUDE.md`, and update Tools and Background jobs for the removal
- [ ] T102 Extend `scripts/workspace-e2e.mjs` with quickstart §11's usage step, and run the whole script
- [ ] T103 Check quickstart §13 against `specs/053-workspace-channel/mockup.html` in both themes at desktop width and 375 px, and record the result
- [ ] T104 Run quickstart §13.7 with a real model, *live provider*, and record how agents use mentions and how noisy filler and notifications are
- [ ] T105 Run `cargo test --workspace`, `cargo clippy --workspace`, `cargo test -p vizier-core --no-default-features`, `cd webui && npm test && npm run typecheck`, `cd design && npm test`, and `scripts/048-agent-state-read-check.sh`

---

## Dependencies

```text
Setup → Foundational ─┬→ US1 (MVP) → US2 → US5 (with US2)
                      │                └→ US6
                      │              US1 → US3
                      │              US1 → US4
US7 ──────────────────┴→ (US1's screen reuses US7's blocks)
```

- **US7** needs only Setup, and can merge first.
- **US1's WebUI tasks** (T054 onward) need US7. Its engine tasks don't.
- **US2 and US5** land together.
- **US3, US4 and US6** are independent of each other once US1 is in.
- **Design before WebUI**: T049 to T053 precede T057 to T059, and T095 precedes T096. Bump pins in the order design → webui → vizier.

## Parallel Opportunities

| When | Can run together |
|---|---|
| Foundational | T013 with T004 |
| US1 tests | T027, T028 |
| US1 design | T049, T050, T051 |
| After US1 | US3, US4 and US6, on different files except `core/service/workspaces.rs`, `core/workspaces/mod.rs` and `scripts/workspace-e2e.mjs` |
| Polish | T099, T100 |

## Implementation Strategy

1. **Ship US7 alone** (T001 to T003, T019 to T026). It's useful at once and carries the least risk.
2. **MVP**: Foundational plus US1. People and agents share a topic. Stop here to check that lazy catch-up and the timeline hold up with real use.
3. **US2 with US5**: the only breaking step, delivered as one change.
4. **US6, US3 and US4**: in any order. Until US4 lands, membership is changed over the API only.
5. **Polish**, including the live-provider check, which is what will show whether notification volume and filler need changing.
