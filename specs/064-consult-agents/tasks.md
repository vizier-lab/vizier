---
description: "Task list for 064: consulting other agents and removing Workspaces"
---

# Tasks: Consulting other agents (and removing Workspaces)

**Input**: Design documents from `specs/064-consult-agents/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md) (D1–D13),
[data-model.md](data-model.md), [contracts/](contracts/), [quickstart.md](quickstart.md)

**Tests**: These are included. The constitution requires dummyplug e2e for agent-observable
changes, and the spec's independent tests and spec 057's fence contract call for unit tests on the
new transitions.

**Organization**: by user story. The MVP is **US1 + US4**: a consultation must never ship without
its access check.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: US1 … US8, as in spec.md
- Paths are from the repository root. Core is `crates/vizier-core/src/`, HTTP is
  `crates/vizier-http/src/`.

---

## Phase 1: Setup

- [ ] T001 Create the submodule branches with `just checkout 064-consult-agents` (design, webui, landing, dummyplug); confirm with `just status`
- [ ] T002 Copy `C:\Users\ganes\OneDrive\Dokumen\Vizier.figcraft` into `C:\Users\ganes\AppData\Local\Temp\opencode` before any FigCraft edit (CLAUDE.md "Design first")

---

## Phase 2: Foundational (blocks every story)

**Purpose**: the extended row, the new channel kind and the request fields that every story
reads. Research D1, D2, D3, D8.

- [X] T003 Add `VizierChannelId::Consultation(String)` (the consulting agent id) with slug `consultation__{id}`, its `to_slug`/parse arms, `usage_activity()` → new `UsageActivity::Consultation` (label "Consultations", place `consultation:{consulting}:{topic}`), and the `"background"` grouping arm in `crates/vizier-core/src/schema/session.rs`; label the kept `Workspace(String)` variant "Workspaces (removed)" with a legacy doc comment
- [X] T004 Extend `Subagent` with `consultee_id: Option<AgentId>`, `principal_user_id: Option<String>`, `follow_ups: u32`; add `new_consultation_id()` (`c_` + 8 base36), `Subagent::is_consultation()`, and make `worker_session()` return `(consultee, Consultation(agent_id), topic)` for a consultation; add `SubagentStopReason::{ConsultingAgentDeleted, ConsulteeDeleted}` and a kind-aware `ByAgent` label ("stopped by the consulting agent") in `crates/vizier-core/src/schema/subagent.rs`
- [X] T005 Add `consultation_chain: Vec<AgentId>` to `VizierRequest` (journaled like `subagent_depth`/`project_scope`, cleared from client input in `stamp_on_receipt`) in `crates/vizier-core/src/schema/request.rs`, and copy it onto `ToolContext` where `subagent_depth` is copied in `crates/vizier-core/src/agents/tools/mod.rs`. *Built as one field: the consultation depth is the chain's length.*
- [X] T006 Add `ConsultationsConfig { max_depth: u8 }` (default 1) under `consultations` in the seed config in `crates/vizier-core/src/config/mod.rs`
- [X] T007 Add the three columns and the partial index on `consultee_id` to the SQLite `subagent` schema with `add_column_if_missing` in `crates/vizier-core/src/storage/sqlite/subagent.rs`, and write `crates/vizier-core/src/storage/sql/postgres/migrations/0012_consultations.sql` *(numbered 0012: it landed before the Workspace drop)*
- [X] T008 Make every `SubagentStorage` transition fence on the scope of the side that causes it (research D3): open, reply, follow-up, stop-by-agent, seen, claim, time-out, re-own and session/agent stops on `agent:{agent_id}`; settle, ask and the new consultee stop on `agent:{consultee_id or agent_id}`, in `crates/vizier-core/src/storage/sqlite/subagent.rs`
- [X] T009 [P] Add one row per new fenced write to the contract tests (settle and ask under the consultee's fence succeed while the consulting scope is held elsewhere; a consulting-side write under a lost fence fails `OwnershipLost`) in `crates/vizier-core/src/storage/sqlite/fence_tests.rs`
- [X] T010 Append `ToolCategory::Consultations` (the 24th, last) with name `consultations`, reserve the name for MCP/addon clashes, and extend `a_new_category_is_appended_to_the_end_of_the_list` in `crates/vizier-core/src/agents/tools/mod.rs`

**Checkpoint**: `cargo test -p vizier-core` passes, and workers behave exactly as before.

---

## Phase 3: User Story 1 - Consult another agent and get the answer back (P1) 🎯 MVP

**Goal**: A consults B with `consult_agent`, B runs as itself in its own consultation
conversation, and A gets the answer through a wait or a delivery.

**Independent Test**: quickstart §1 and §2.1.

- [X] T011 [US1] Generalise `Subagents::spawn` to take `Target::{Worker { task }, Agent { agent_id, request }}`: consultation rows get a `c_` id, `consultee_id`, `principal_user_id`, and their request goes to `worker_session()` with `consultation_chain + [A]` (its length is the depth), at `subagent_depth` 0, with no project scope, in `crates/vizier-core/src/agents/subagents.rs`
- [X] T012 [US1] Make `turn_ended` settle a `Consultation` session's row (looked up by topic) as it does a `Subagent` one, and recognise either ask tool as `TurnEnd::Asked`, in `crates/vizier-core/src/agents/subagents.rs` and the `Chat` arm of `crates/vizier-core/src/agents/process.rs`
- [X] T013 [US1] Title `render_update` by kind ("[Consultation update]", naming the consultee and its position) in `crates/vizier-core/src/agents/subagents.rs`
- [X] T014 [P] [US1] Write `consultation_md(name, id)` from the fixed text in `contracts/consultation-tools.md` in a new `crates/vizier-core/src/agents/agent/system_prompt/consultation.rs`, and register the module in `system_prompt/mod.rs`
- [X] T015 [US1] Append `consultation_md` last for a `Consultation` session in `system_prompt_layout` (the consultee keeps its own BOOT, persona and identity), and add a pure layout test beside the worker tests in `crates/vizier-core/src/agents/agent/mod.rs`
- [X] T016 [US1] Frame `SubagentTask`/`SubagentReply` requests on a `Consultation` session in `generate_frontmatter` (`sender: <A>`, `via: consultation | consultation_reply | consultation_followup`, `on_behalf_of: <person display name> | none`, `consultation: N of M`), with a test, in `crates/vizier-core/src/schema/request.rs`
- [X] T017 [US1] Exclude `Consultation` conversations in `is_interactive_turn`'s conversation test (removes `ask_question`/`ask_sensitive_input`, FR-017) in `crates/vizier-core/src/agents/tools/mod.rs`
- [X] T018 [US1] Create `crates/vizier-core/src/agents/tools/consultations.rs` with `consult_agent` (`consultations: [{agent, request}]` 1..=8, `timeout_secs?`, validate every entry before starting any) and `wait_consultations` (refusing an `s_` id as `unknown_consultation`), returning the snapshot shape from `data-model.md`
- [X] T019 [US1] Register both tools on `default_toolset` in `BaseTools::new()`, add them to the `every_tool_is_in_its_category` table, and keep them out of `DREAM_TOOL_NAMES`, in `crates/vizier-core/src/agents/tools/mod.rs`
- [X] T020 [US1] Make the subagent tools refuse a `c_` id as `unknown_subagent`, and leave the `s_` path untouched, in `crates/vizier-core/src/agents/tools/subagents.rs`
- [X] T021 [US1] Add `consult_and_wait` and `consultation_delivered_once` cases (two dummyplug agents; B scripted by the request text) in `crates/vizier-core/tests/dummyplug_e2e.rs`

**Checkpoint**: quickstart §1 and §2.1 pass. Do not release without US4.

---

## Phase 4: User Story 4 - Only agents the person can use (P1) 🎯 MVP

**Goal**: The view-right check, the owner rule for scheduled runs, the Discord/Telegram deferral,
and the consultable list. Research D5.

**Independent Test**: quickstart §5.

- [X] T022 [US4] Create `crates/vizier-core/src/consultations/mod.rs` and `access.rs`: `principal_for_turn(deps, ctx)` (`web_user()` for HTTP/Project, A's `owner_id` for Task/System, `no_owner` otherwise), `consultable(deps, ctx, agent_id)` (`can_view_agent`, not self, not in `consultation_chain`), and `may_consult(ctx, max_depth)` (depth below the max, `subagent_depth == 0`, not a dream, and the root conversation has no `platform()`, FR-011a / #116); register in `crates/vizier-core/src/lib.rs`
- [X] T023 [P] [US4] Unit tests for the three functions (view right, unknown and not-allowed agents answer the same, self, chain, owner fallback, no owner, Discord/Telegram and a detached script started there refused) in `crates/vizier-core/src/consultations/access.rs`
- [X] T024 [US4] Call `consultable` from `consult_agent` (refusals `unknown_agent`, `self`, `in_chain`, `no_owner`; `unavailable` with the `AgentUnavailable` reason read from the published health view) and gate every consultation tool's `available(ctx)` on `may_consult`, in `crates/vizier-core/src/agents/tools/consultations.rs`
- [X] T025 [US4] Add `list_consultable_agents` (`{id, name, description, available}`, never self or the chain) in `crates/vizier-core/src/agents/tools/consultations.rs`, and register it in `crates/vizier-core/src/agents/tools/mod.rs`
- [ ] T026 [US4] Add `consult_refused_for_viewer`, `scheduled_run_consults_as_owner` and `no_consultations_on_discord` cases in `crates/vizier-core/tests/dummyplug_e2e.rs`

**Checkpoint**: The MVP is complete. Quickstart §1, §2.1 and §5 pass.

---

## Phase 5: User Story 2 - Managing consultations like workers (P1)

**Goal**: Stop, list, read and the time limit.

**Independent Test**: quickstart §3.

- [X] T027 [US2] Add `stop_consultation`, `list_consultations` and `get_consultation` (marks seen; returns `conversation`) over the shared `Subagents` methods, each filtering to `c_` rows of this conversation, in `crates/vizier-core/src/agents/tools/consultations.rs`, and register them in `crates/vizier-core/src/agents/tools/mod.rs`
- [X] T028 [US2] Make `halt`/`abort_worker` abort the consultee's session (`worker_session()` already points at B), so a stop or time-out ends B's turn with a stop notice, and confirm it in `crates/vizier-core/src/agents/subagents.rs`
- [X] T029 [US2] Add a `stop_one_consultation_others_finish` case in `crates/vizier-core/tests/dummyplug_e2e.rs`

---

## Phase 6: User Story 6 - Consultations survive interruptions (P1)

**Goal**: Aborts, turn ends and restarts of either agent lose nothing, and the outcome is
delivered once.

**Independent Test**: quickstart §2.2 and §2.3.

- [ ] T030 [US6] Confirm `recover_agent` re-arms the consulting side's timers (`reown_running_subagents` on A) and pokes the consulting sessions, and that B's own `session_turn` recovery resumes the consultation turn (`claim_turn_on_receipt`, `resumed`); fix what doesn't, in `crates/vizier-core/src/agents/subagents.rs` and `crates/vizier-core/src/recovery.rs`
- [ ] T031 [US6] Add recovery tests for a consultation (A restarted: its timer is re-armed and nothing is delivered twice; B restarted: its turn is resumed and the answer settles the row) in `crates/vizier-core/src/recovery_tests.rs`
- [ ] T032 [US6] Add `abort_keeps_consultation` and `consultee_restart_resumes` cases in `crates/vizier-core/tests/dummyplug_e2e.rs`

---

## Phase 7: User Story 8 - Workspaces are removed (P1)

**Goal**: Nothing of Workspaces remains reachable, their data is deleted once, and the old names
give guidance. Research D11, D12.

**Independent Test**: quickstart §9.

### Server

- [X] T033 [US8] Point `delegate_agent`, `mention_agent`, `send_message_to_workspace`, `list_workspaces` and `read_workspace_messages` at one new hint ("Workspaces were removed: consult another agent with consult_agent, then wait_consultations for its answer."), drop `consult_agent` from `RETIRED_TOOL_NAMES`, and add a test that no hint names a retired tool, in `crates/vizier-core/src/agents/tools/mod.rs`
- [X] T034 [US8] Delete `crates/vizier-core/src/agents/tools/workspaces.rs` and its registrations in `crates/vizier-core/src/agents/tools/mod.rs`. Keep the `ToolCategory::Workspaces` variant in place as a legacy entry with no tools, so the order in `ToolCategory::ALL` doesn't shift (`a_new_category_is_appended_to_the_end_of_the_list`); a category with no registered tools is never listed. Keep `workspaces` reserved as a category name.
- [X] T035 [US8] Delete `crates/vizier-core/src/workspaces/` (all files), `crates/vizier-core/src/service/workspaces.rs` and `crates/vizier-core/src/schema/workspace.rs`, and their `mod`/`pub use` lines in `lib.rs`, `service/mod.rs` and `schema/mod.rs`
- [X] T036 [US8] Remove the Workspace arms and `Hub` uses from `crates/vizier-core/src/agents/process.rs`, `agents/steer.rs`, `agents/steps.rs`, `agents/mod.rs`, `agents/agent/mod.rs` and `agents/agent/system_prompt/{mod,context}.rs`, keeping `SilentRead` for Discord/Telegram (FR-034)
- [X] T037 [US8] Remove the Workspace catch-up before a dream from `crates/vizier-core/src/scheduler/dream/mod.rs`
- [X] T038 [US8] Remove `workspace_message` creation and the Workspace visible scope from `crates/vizier-core/src/notifications/mod.rs`, `service/notifications.rs`, `service/inbox.rs` and `schema/notification.rs`; remove Workspace rows from `service/conversations.rs` and `usage/mod.rs` (keeping the legacy label for old `usage_event` rows)
- [X] T039 [US8] Remove the `workspaces:create`/`all_workspaces:manage` top-up from `crates/vizier-core/src/storage/user.rs`
- [X] T040 [US8] Delete `crates/vizier-core/src/storage/workspace.rs` and `storage/sqlite/workspace.rs`, and remove `WorkspaceStorage` from `VizierStorageProvider` in `storage/mod.rs` and `storage/sqlite/mod.rs`
- [X] T041 [US8] Add the once-only cleanup under marker `migration.drop_workspaces_v1` in one transaction (Workspace sessions' history, `session_detail`, `session_turn`, `scratch_file`, `session_file` rows and their blobs by reference count, `message_reaction`, `platform_message_link`; `workspace_message` notifications; the two permissions from every role; `DROP TABLE` every `workspace*` table) in `crates/vizier-core/src/storage/sqlite/mod.rs`, with a test that seeds Workspace data and checks that memory, other sessions and `usage_event` survive
- [X] T042 [P] [US8] Write `crates/vizier-core/src/storage/sql/postgres/migrations/0013_drop_workspaces.sql` with the same deletions and drops
- [X] T043 [P] [US8] Delete `crates/vizier-http/src/api/v1/workspaces.rs` and `crates/vizier-http/src/workspace_topic.rs` and their router and OpenAPI lines in `api/v1/mod.rs` and `lib.rs`
- [X] T044 [P] [US8] Delete `crates/vizier-http/tests/conformance/groups/workspaces.rs`, its line in `groups/mod.rs`, its recorded baselines under `target/038-baseline/conformance/`, and the Workspace routes in `crates/vizier-http/tests/conformance/pending.txt`
- [X] T045 [P] [US8] Delete `crates/vizier-core/tests/workspaces_service.rs` and the Workspace cases in `crates/vizier-core/tests/dummyplug_e2e.rs`

### UI (after T046)

- [ ] T046 [US8] Design pass, rail without Workspaces: on the `Shell` pages of `Vizier.figcraft` (or the ASCII in `contracts/http-and-webui.md` if FigCraft is unreachable), remove the Workspace group and the create chooser, and have `+` open New agent; review it before code
- [ ] T047 [US8] Remove the Workspace entry variant from `design/src/components/Shell/Rail.tsx`; delete `design/src/components/CreateChooser/`; drop `OriginBadge`, `MentionChip` and `MentionMenu` only if nothing outside Workspaces imports them (project `@` mentions use `MentionMenu`); update stories
- [ ] T048 [US8] Delete `webui/app/routes/workspace.tsx` (and its lines in `webui/app/routes.ts`), `webui/app/screens/workspaces/`, `webui/app/stores/workspaces.ts`, `webui/app/lib/workspace*.ts` (+ tests) and `webui/app/api/endpoints/workspaces.ts`
- [ ] T049 [US8] Remove the Workspace pieces from the shell: the rail group and create chooser in `webui/app/screens/shell/`, the Workspaces list in the phone `AgentSwitcherSheet` and `MobileHome`, and Workspace notification handling in `webui/app/lib/inboxFrames.ts`/`stores/notifications.ts`; regenerate `webui/app/api/schema.d.ts` with `scripts/openapi.mjs`

**Checkpoint**: quickstart §9 passes, and `cargo test --workspace` and `npm run typecheck` pass.

---

## Phase 8: User Story 3 - Asking back and following up (P2)

**Goal**: B asks A, A replies, and A follows up on an answered consultation. Research D4.

**Independent Test**: quickstart §4.

- [X] T050 [US3] Add `follow_up_subagent(id, deadline_at, at)` (`answered → running`, consultations only, `follow_ups + 1`, `ended_at` cleared, `outcome_seq` untouched) to the trait in `crates/vizier-core/src/storage/subagent.rs` and the guarded update in `storage/sqlite/subagent.rs`, with transition tests
- [X] T051 [US3] Add `Subagents::reply_or_follow_up` (asking → reply, answered → follow-up, else `not_waiting`; re-checks `consultable`, FR-013) sending `SubagentReply` with the matching `via`, in `crates/vizier-core/src/agents/subagents.rs`
- [X] T052 [US3] Add `reply_consultation` and `ask_consulting_agent` (turn-ending, offered only in a `Consultation` session, sharing `ask_main_agent`'s implementation) in `crates/vizier-core/src/agents/tools/consultations.rs`; register both and add them to the category table in `crates/vizier-core/src/agents/tools/mod.rs`
- [X] T053 [US3] Add `consultee_asks_then_answers` and `follow_up_continues_conversation` cases in `crates/vizier-core/tests/dummyplug_e2e.rs`

---

## Phase 9: User Story 5 - A person peeks at the consultations (P2)

**Goal**: The Consultations strip item and its read-only panel in A's chat.

**Independent Test**: quickstart §8.1 and §8.3.

- [ ] T054 [US5] *(`consultee` (the id) and `follow_ups` are already on the snapshot's summary; the WebUI maps the id to a name from its agent list.)* Confirm the `SessionFrame::Subagent` snapshot in `crates/vizier-core/src/transport/events.rs` (and wherever the snapshot DTO is built), and a `kind` filter to `service::subagents` in `crates/vizier-core/src/service/subagents.rs`
- [ ] T055 [US5] Add `GET …/topic/{topic}/consultations[/{id}[/history]]` (history read from the consultee's conversation) in a new `crates/vizier-http/src/api/v1/agents/consultations.rs`; route it in `agents/mod.rs`; filter the subagent routes to `consultee == null` in `agents/subagents.rs`
- [ ] T056 [P] [US5] Add conformance vectors for the three routes (owner, other person, unknown id) in a new `crates/vizier-http/tests/conformance/groups/consultations.rs` and record baselines with `CONFORMANCE_RECORD=1`
- [ ] T057 [US5] Design pass on the `Chat` page of `Vizier.figcraft`: the Consultations strip item and panel (or approve the ASCII mockup in `contracts/http-and-webui.md`); review before code
- [ ] T058 [US5] Add an `agent` slot (consultee `Sigil` and name) and a follow-ups count to `design/src/components/SubagentWorker/SubagentWorker.tsx`, with a consultation story in `SubagentWorker.stories.tsx`
- [ ] T059 [US5] Make the grouping in `webui/app/lib/subagents.ts` kind-generic and split frames by `consultee`, with tests in `webui/app/lib/subagents.test.ts`
- [ ] T060 [US5] Add `webui/app/api/endpoints/consultations.ts` and `webui/app/screens/chat/ConsultationsPanel.tsx` (read-only; ×, Escape and an outside press close it; phone bottom sheet), and the strip item with count and amber dot beside Subagents in the chat's status strip

---

## Phase 10: User Story 7 - The consulted agent's people see what it was asked (P3)

**Goal**: Consultation rows in B's conversation list, read-only, visible to B's editors.

**Independent Test**: quickstart §1.2 and §8.2.

- [ ] T061 [US7] List `consultation__…` sessions as `ConversationKind::Consultation` with `from_agent {id, name}`, only for callers with `can_edit_agent(B)`, in `crates/vizier-core/src/service/conversations.rs` (`kind_of`)
- [ ] T062 [US7] Refuse `chat`, `client_event` and `ui_page_state` frames on a `Consultation` channel with `read_only_conversation` in `crates/vizier-http/src/topic.rs`
- [ ] T063 [US7] Draw consultation rows ("Consultation from <A>") and open them without a composer in `webui/app/screens/shell/AgentConversations.tsx` and the chat screen, after the design review in T057

---

## Phase 11: Lifecycle edges (deletion and limits, cross-story)

- [X] T064 Make `Subagents::delete_session`/`delete_agent` delete each consultation's conversation through `worker_session()` (B's history, files, scratch and turns, after aborting B's turn), and stop consultations with `ConsultingAgentDeleted` on agent delete, in `crates/vizier-core/src/agents/subagents.rs`
- [X] T065 Add `stop_consultations_of(consultee, ConsulteeDeleted)` (fenced on the consultee's scope) to `SubagentStorage` and `storage/sqlite/subagent.rs`, call it from the agent delete path in `crates/vizier-core/src/agents/mod.rs`, and poke each consulting session
- [ ] T066 Add `delete_conversation_removes_consultee_copy` and `deleted_consultee_is_delivered` cases in `crates/vizier-core/tests/dummyplug_e2e.rs`
- [ ] T067 Add an offering test (person's chat: seven tools; consultation: only `ask_consulting_agent`; worker, dream, Discord: none) in `crates/vizier-core/src/agents/tools/consultations.rs`

---

## Phase 12: Polish and cross-cutting

- [ ] T068 [P] Write `docs/src/configuration/consultations.md` (what a consultation is, access, limits, the #116 deferral, the `consultations.max_depth` setting); delete `docs/src/configuration/workspaces.md`; update `docs/src/SUMMARY.md`
- [ ] T069 [P] Write `scripts/consultations-e2e.mjs` from quickstart §1–§9 (two dummyplug agents) and delete `scripts/workspace-e2e.mjs`
- [ ] T070 [P] Update the operating doctrine in BOOT (`templates/` or the boot module) to explain subagents and consultations, and when to choose each (FR-026)
- [ ] T071 Update `CLAUDE.md`: replace the Workspaces section with a Consultations one, note `Consultation` in the channel list, set the category count to 24, list the retired names, and mention `consultations.max_depth`
- [ ] T072 Add #82 checklist entries on GitHub for `migration.drop_workspaces_v1` and its cleanup, the kept `VizierChannelId::Workspace` variant (and the `ToolCategory` legacy entry if kept), and the five retired names
- [ ] T073 Run `cargo clippy --workspace`, `cargo test --workspace`, `cargo test -p vizier-core --no-default-features`, and in `webui/` `npm run typecheck` and `npx vitest run` (no `cargo fmt`: the workspace isn't rustfmt-clean)
- [ ] T074 Run the whole of `quickstart.md` against `just run` with dummyplug agents, then `node scripts/consultations-e2e.mjs`; record anything that differs
- [ ] T075 Commit across repos with `just commit` in the order design → webui → vizier, with conventional messages (`feat!:` for the Workspace removal)

---

## Dependencies

```
Setup (T001–T002)
  └─ Foundational (T003–T010)
       ├─ US1 (T011–T021) ──┬─ US4 (T022–T026)  ← MVP = US1 + US4
       │                    ├─ US2 (T027–T029)
       │                    ├─ US6 (T030–T032)
       │                    ├─ US3 (T050–T053)
       │                    └─ US5 server (T054–T056) ─ US5 UI (T057–T060)
       │                                                └─ US7 (T061–T063, UI after T057)
       ├─ US8 server (T033–T045), independent of US1 except T033 needs T018's tool name
       │    └─ US8 UI (T046–T049)
       └─ Lifecycle (T064–T067) after US1, US2, US3
Polish (T068–T075) last
```

- US8's server half can be built beside US1 once Foundational is done. It touches different
  files, except `agents/tools/mod.rs` (T033/T034 vs T019/T025) and `agents/process.rs` (T036 vs
  T012). Sequence those two files.
- Every UI task waits on its design pass (T046, T057).

## Parallel examples

- After T010: T014 (prompt section), T022 (access module) and T042/T043/T044/T045 (Workspace
  deletions in other files) can run together.
- In US4: T023 (tests) can run beside T024.
- In US5: T056 (conformance) can run beside T058 (design component) once T055 is done.
- In Polish: T068, T069 and T070 can run together.

## Implementation strategy

1. **MVP**: Setup → Foundational → US1 → US4. Check with quickstart §1, §2.1 and §5. Agents can
   consult safely, and nothing else changes yet.
2. **Parity with subagents**: US2 → US6 → US3, then the lifecycle phase.
3. **Removal**: US8, server then UI. After this, Workspaces are gone (a breaking change, so ship
   it in the same release as the MVP, never before).
4. **Views**: US5 → US7.
5. **Polish**: docs, CLAUDE.md, #82, the full quickstart.
