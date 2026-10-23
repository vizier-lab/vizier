# Tasks: WebUI Rewrite

**Input**: Design documents from `/specs/024-webui-rewrite/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/](contracts/), [quickstart.md](quickstart.md)

**Tests**: The spec does not ask for TDD. Tests included here are the ones the plan and constitution require:
- `cargo test` units for the pure server parts
- vitest for the WebUI's `lib/`
- the design library's own story tests for every library change
- one Playwright spec per user story, which makes the quickstart steps executable
- the dummyplug e2e gate

**Organization**: tasks are grouped by user story (spec.md) so each story can be built and checked on its own.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: the spec user story (US1…US7) the task belongs to

## Path conventions (three repositories)

- `src/…`, `scripts/…`, `.github/…`, `justfile`, `CLAUDE.md`: **vizier** (this repository)
- `webui/…`: the **vizier-lab/webui** submodule, the new app ([contracts/webui-routes.md](contracts/webui-routes.md) for its layout)
- `design/…`: the **vizier-lab/design** submodule, the component library

**Rules for every `design/` task** ([contracts/design-library.md](contracts/design-library.md)):
- The component gets a story (`*.stories.tsx`), an export from `design/src/index.ts`, an inventory id in its story metadata, and a `design/CHANGELOG.md` line.
- It must pass `npm run typecheck && npm test && npm run lint:tokens` in `design/`.
- Commit it in `design/` first. The vizier commit that uses it bumps the `design` pin.

**Rules for every `webui/` screen task**:
- Compose library components only, using token utilities and no arbitrary values.
- Data comes from `webui/app/api/` (typed from `webui/app/api/schema.d.ts`).
- No user-facing mention of dummyplug (FR-017).
- Match the FigCraft page named in the task, in both themes.

---

## Phase 1: Setup (shared infrastructure)

**Purpose**: the repository switch and an empty, building new app.

- [X] T001 Create the first commit of the `vizier-lab/webui` repository (README stub, MIT `LICENSE`, `.gitignore` for `node_modules/`, `build/`, `.react-router/`, `test-results/`) and push it to `git@github.com:vizier-lab/webui.git` (confirm with the maintainer before pushing)
- [X] T002 Move the old app: `git mv webui legacy` in vizier. Leave its contents unchanged and don't touch `legacy/` again in any later task
- [X] T003 Add the submodule: `git submodule add https://github.com/vizier-lab/webui.git webui`, then `git -C webui remote set-url --push origin git@github.com:vizier-lab/webui.git`, which updates `.gitmodules`
- [X] T004 Scaffold the app in `webui/`:
  - `webui/package.json`: React 19, react-router 7.12 + `@react-router/dev`, Tailwind 4 + `@tailwindcss/vite`, Vite 7, zustand, react-markdown, remark-gfm, rehype-highlight, nanoid, `"@vizier/design": "file:../design"`; dev: typescript 5.9, vitest, `@playwright/test`, openapi-typescript. Scripts: `dev`, `build`, `typecheck` (`react-router typegen && tsc`), `test`, `e2e`, `gen:api`.
  - `webui/react-router.config.ts` with `ssr: false`.
  - `webui/vite.config.ts` with `resolve.dedupe: ["react","react-dom"]` and a `/api` proxy to `VIZIER_URL` (default `http://localhost:9999`, ws included).
  - `webui/tsconfig.json` with the React types `paths` from `design/README.md`.
- [X] T005 Create `webui/app/root.tsx` (html shell, `<html data-theme>` from the stored choice, or no attribute to follow the OS) and `webui/app/app.css` with `@import "@vizier/design/styles.css";` and `@source "../../design/src";`. Then check that `npm run build` writes `webui/build/client/index.html`
- [X] T006 [P] Add ESLint and Prettier configs in `webui/eslint.config.mjs` and `webui/prettier.config.js`, carrying over the rules from `legacy/eslint.config.mjs` and `legacy/prettier.config.js`
- [X] T007 [P] Update `justfile`: `install` runs `git submodule update --init --recursive`, then `npm i` in `design/` and `webui/`, then `cargo fetch`. `build` runs `cd webui && npm run build`
- [X] T008 [P] Add `scripts/openapi.mjs` (Node ≥ 22, no dependencies). It fetches `${VIZIER_URL}/openapi.json`, writes it to `webui/openapi.json`, and runs `npm run gen:api` in `webui/`. `gen:api` runs openapi-typescript into `webui/app/api/schema.d.ts`
- [X] T009 [P] Add the test harness:
  - `webui/vitest.config.ts`
  - `webui/playwright.config.ts` (baseURL from `VIZIER_URL`, projects for desktop 1440×900 and mobile 390×844, in both color schemes)
  - `webui/e2e/fixtures.ts`: an admin login helper; `createPlugAgent(id)`, which creates a dummyplug agent through `POST /api/v1/agents`; `send(text)`; `HOLD(n)` from quickstart.md

**Checkpoint**: `just install && just run` builds and serves the empty new app at `:9999`. `legacy/` is not built.

---

## Phase 2: Foundational (blocking prerequisites)

**Purpose**: what every story needs: paging, the API client, stores, shell, routes and shared screen parts.

**⚠️ CRITICAL**: no story phase starts until this phase is complete.

- [X] T010 Add `ListQuery`, `SortOrder`, `Page<T>`, the `Listing<T>` trait and `paginate()` to `src/channels/http/models/page.rs`, and register the module in `src/channels/http/models/mod.rs`. Behaviour:
  - clamp `limit` to 1–100
  - `counts` per filter is computed after `q` and before `filter`
  - an unknown filter returns `400 unknown_filter`
  - an unknown sort uses the default

  Add `#[cfg(test)]` units for each of these (data-model §2.1)
- [X] T011 [P] Add the LoadMore component (X81) in `design/src/components/LoadMore/` (`LoadMore.tsx`, `LoadMore.stories.tsx`, `index.ts`): `onLoad`, `loading`, `label`, `remaining`, and `look: "button" | "auto"` (`auto` uses an IntersectionObserver)
- [X] T012 [P] Extend the Picker (X41) in `design/src/components/Picker/Picker.tsx`: add `onEndReached`, `loading`, and `search.mode: "client" | "server"` (default `client`, which keeps today's filtering). Add a "Server paged" story in `design/src/components/Picker/Picker.stories.tsx`
- [X] T013 Implement the typed client in `webui/app/api/client.ts`:
  - `request<T>(method, path, body?)` unwraps the `APIResponse` envelope, adds `Authorization: Bearer`, and maps the error body to `ApiError { status, code, message, data }`
  - a 401 clears the session and redirects to `/login`
  - `paged<T>(path, ListQuery)` builds the query string

  Put domain wrappers in `webui/app/api/endpoints/*.ts`, typed from `schema.d.ts`
- [X] T014 [P] Implement `webui/app/lib/paging.ts` (query-string ⇄ `ListQuery`, page math, "1–25 of 312") and the `usePagedList(path, defaults)` hook in `webui/app/lib/usePagedList.ts` (URL-synced `q`/`filter`/`sort`/`page`, debounced search). Test in `webui/app/lib/paging.test.ts`
- [X] T015 [P] Implement `webui/app/lib/format.ts`: middle truncation (re-exporting the library's helper), counts with separators, chart-only abbreviation (61,880 → 61.9k), relative and absolute times, `~$0.03` cost formatting, durations ("2m 14s"). Test in `webui/app/lib/format.test.ts`
- [X] T016 Implement `webui/app/stores/session.ts`: token, current user (`GET /auth/users/me`), the permission list, `can(permission)`, `canEditAgent(agent)`, and the theme (`system | dark | light`) applied to `<html data-theme>`. Every localStorage access goes in try/catch
- [X] T017 [P] Implement `webui/app/stores/toasts.ts` and the `webui/app/layouts/ToastRegion.tsx` stack, using the library's Toast
- [X] T018 Implement `webui/app/stores/agents.ts`:
  - visible agents from `GET /agents`
  - health from `GET /agents/health`, polled every 10 s while the tab is visible
  - setup progress from `GET /utils/setup-state`, polled every 1 s only while some agent is starting, keeping the last 5 samples for the ETA (research R15)
- [X] T019 Write the full route table in `webui/app/routes.ts` per [contracts/webui-routes.md](contracts/webui-routes.md):
  - placeholder modules in `webui/app/routes/*.tsx` that render PageHeader + EmptyState
  - redirects in `webui/app/routes/redirects.tsx`: `/settings` → `/settings/profile`, `/:agentId/settings` → `/:agentId/config`, and a catch-all to `/`
- [X] T020 Implement `webui/app/layouts/AuthLayout.tsx` (Login/Onboarding split panel, without the LED board, which comes in US5) and the auth guard in `webui/app/layouts/RequireAuth.tsx` (no token → `/login`; `setup_required` → `/onboarding`)
- [X] T021 Implement `webui/app/layouts/AppShell.tsx` with the library Sidebar:
  - AgentSwitcher opens the agent Picker: search with count, Recent, All A–Z, an Owner badge, "Create agent"
  - NavSections App / Integrations / Misc, filtered by permission
  - UserMenu with the theme toggle and sign out
  - collapse to the rail, remembered per device
  - the active item comes from the route
- [X] T022 [P] Implement `webui/app/layouts/SettingsLayout.tsx` with the library SettingsNav (Account: Profile, Password, API keys; Administration: Providers, Users, Roles, each gated by permission)
- [X] T023 [P] Implement `webui/app/lib/access.ts` (permission names, `canManageIntegrations`, `canViewUsage`) and `webui/app/layouts/RequirePermission.tsx` (EmptyState "You don't have access to this page", FR-012)
- [X] T024 [P] Implement `webui/app/screens/shared/ConfirmDialog.tsx`: the library Dialog with tone Danger, a list of what will be removed, and an optional type-to-confirm field (States page, FR spec US4-4)
- [X] T025 [P] Implement `webui/app/screens/shared/PagedTable.tsx`: SearchField with a total count, FilterTabs with server counts, SortSelect, Table, Pagination (25 per page), and EmptyState for empty or no results, all bound to `usePagedList`
- [X] T026 [P] Implement `webui/app/screens/shared/Markdown.tsx`: react-markdown + remark-gfm + rehype-highlight inside `<Prose>`, with code blocks through the library CodeBlock (copy button) and links opening in a new tab
- [X] T027 [P] Implement `webui/app/screens/shared/PageFrame.tsx`, the standard page body (PageHeader with mono meta, a right slot, a content area at the design's widths) used by every agent page

**Checkpoint**: signed in, the shell renders every route as a placeholder with correct navigation and permission gating. `cargo test` passes for `page.rs`.

---

## Phase 3: User Story 1 - Chat with an agent (Priority: P1) 🎯 MVP

**Goal**: the redesigned Chat with every turn state, the side panels (recalled memory, background jobs, session files), the context ring, client tools and actionable errors.

**Independent Test**: quickstart §2 and §3, plus `webui/e2e/chat.spec.ts`.

### Server for User Story 1

- [X] T028 [US1] Add `RecallOrigin`, `RecalledPassage` and `SessionHistoryContent::Recall { origin, passages }` in `src/schema/history.rs` (serialized `content_type: "recall"`), and make `history_entries_to_messages` skip `Recall`. Unit tests: model input is unchanged with Recall entries present, and duplicate passages keep the highest score (data-model §2.5)
- [X] T029 [US1] Add `recalled: Arc<Mutex<Vec<RecalledPassage>>>` to `ToolContext` in `src/agents/tools/mod.rs` and initialise it at every `ToolContext` construction site (dreams, tasks, handovers, background pieces included)
- [X] T030 [US1] Make `memory_search` in `src/agents/tools/vector_memory/` push each returned passage (bundle, path, ordinal, score, title) into `ToolContext.recalled`
- [X] T031 [US1] In `src/agents/process.rs`, record `Recall { origin: Auto }` through the turn's `StepRecorder` right after `retrieve_auto_context` returns a non-empty result, before the first model call
- [X] T032 [US1] In `src/agents/agent/mod.rs`, drain `ToolContext.recalled` after each tool round (next to the existing `loaded_images` drain) and record one `Recall { origin: Search }` when it is non-empty
- [X] T033 [P] [US1] Add `sender: Option<String>` and `history_uid: Option<String>` (`#[serde(default)]`) to `SessionFileRecord` in `src/schema/session_file.rs`. Set them in every `save_session_file` caller: `src/agents/tools/{tts,stt,image_gen,read_image,session_files}/` and the upload path in `src/channels/http/api/v1/files.rs`
- [X] T034 [US1] Add `SessionFrame::File(SessionFileRecord)` in `src/transport.rs`, publish it after each successful `save_session_file`, and emit it on the socket as `{ "type": "session_file", "file": … }` in `src/channels/http/api/v1/agents/channel.rs`'s socket writer
- [X] T035 [US1] Add `GET /agents/{id}/channel/{channel}/topic/{topic}/files?offset&limit` in `src/channels/http/api/v1/agents/channel.rs` (newest first, `Page<SessionFileRecord>`, view check as in `get_topic_history`, `#[utoipa::path]`, registered in `ApiDoc`)
- [X] T036 [P] [US1] Support a single `Range: bytes=a-b` on `GET /files/{file_id}` in `src/channels/http/api/v1/files.rs`: `206` + `Content-Range` + `Accept-Ranges: bytes`, `416` when out of bounds, unchanged without the header
- [X] T037 [P] [US1] Add `POST /agents/{id}/restart` in `src/channels/http/api/v1/agents/mod.rs`: `user_can_edit_agent`, send `AgentCommand::Restart`, answer `202` with the health status, `409 busy` while a build is in flight ([contracts/http-api.md](contracts/http-api.md))
- [X] T038 [US1] Run `scripts/openapi.mjs` against the updated server and commit `webui/openapi.json` + `webui/app/api/schema.d.ts`

### Design library for User Story 1

- [X] T039 [P] [US1] Add the EmojiPicker component (X78) in `design/src/components/EmojiPicker/`: search, a recent row, and a grid from a bundled emoji list in `design/src/components/EmojiPicker/emoji.ts` (no network), built on Picker's popover and search
- [X] T040 [P] [US1] Add an `eta` prop to AgentStartingCard (X58) in `design/src/components/AgentStartingCard/AgentStartingCard.tsx`, with a story

### WebUI for User Story 1

- [X] T041 [US1] Type every frame in [contracts/websocket.md](contracts/websocket.md) in `webui/app/api/frames.ts`, `session_file` included
- [X] T042 [US1] Implement `webui/app/api/socket.ts`: one WebSocket per open topic, backoff 1 s doubling to 30 s, `offline` after 3 failed attempts, and on reopen a history re-read after the last known uid (no duplicates)
- [X] T043 [P] [US1] Port `legacy/app/lib/clientTools.ts`, `legacy/app/lib/clientTools/builtin.tsx`, `legacy/app/lib/pageDocument.ts` and `legacy/app/lib/trail.ts` with their tests into `webui/app/lib/`, changing only imports and UI bindings
- [X] T044 [US1] Implement the pure turn reducer `reduce(topic, frame)` in `webui/app/lib/frames.ts`:
  - history merged by uid
  - live labels from `thinking`/`tool_choice`
  - queued messages
  - jobs
  - recalls
  - files
  - context use from `response.stats`
  - error kinds
  - the state machine in data-model §3

  Vitest in `webui/app/lib/frames.test.ts`, covering a reopen mid-turn with no duplicates, the queue being dropped on abort, and a resumed reply
- [X] T045 [US1] Implement `webui/app/stores/connection.ts` (socket per topic, connection state, client-tool declarations on every request from `lib/clientTools.ts`) and `webui/app/stores/turns.ts` (per-topic state from history plus the reducer; upward history paging)
- [X] T046 [US1] Implement the Chat page frame in `webui/app/routes/chat.tsx`:
  - three columns: conversation, composer, and a collapsible side panel
  - TopicHeader with a topic Picker (from the existing `GET …/channel/{channel}/topics`) and "New topic"
  - JobsPill
  - the agent-state banners on top
- [X] T047 [US1] Implement `webui/app/screens/chat/MessageList.tsx`:
  - UserMessage, with SkillChip, attachments, and the queued look
  - AgentHeader + AgentReply through `screens/shared/Markdown.tsx`, with a meta row (model · in · out · cost, copy, reactions)
  - ChatGreeting for an empty topic
  - CheckpointDivider + HandoverSummary (with and without a handover)
  - StoppedNote, BackgroundReportCard, JumpToLatest ("N new messages")
  - upward history loading with LoadMore `auto`
- [X] T048 [US1] Implement the trail in `webui/app/screens/chat/Trail.tsx`:
  - ActivityTrail, TrailHead, TrailCall (done/running/failed/limit; lookup look), ScriptCall + RunDetail, Thought/Fold
  - the live trail while working, then folding into the summary row ("Worked 18.4s · 3 python runs · 4 passages recalled", counting failed runs) when the reply lands (FR-008b)
- [X] T049 [US1] Implement `webui/app/screens/chat/Composer.tsx`:
  - the library Composer (idle/busy/focus/filled/recording/disabled), and Send turning into Stop (sends `/abort`)
  - attachments through `POST /files/upload` with Attachment chips and a DropZone overlay
  - VoiceRecorder recording and playback
  - SkillMenu on `/` with built-in commands plus the agent's skills (`GET /agents/{id}/skills` + global), ↑↓/Tab/Enter
  - ContextRing from `stats.current_context_size`/`context_window` (FR-008c)
- [X] T050 [P] [US1] Implement `webui/app/screens/chat/ErrorNotice.tsx`, mapping each error kind to the Notice tone, text and actions from the Chat states sheet C1–C5: Retry, Switch model (→ config), Open Providers, Open agent health; and a compact in-trail Notice for a tool timeout
- [X] T051 [P] [US1] Implement `webui/app/screens/chat/Reactions.tsx`: Reaction badges, the add-reaction button opening EmojiPicker, and sending and acknowledging through the existing reactions frame and endpoint
- [X] T052 [P] [US1] Implement `webui/app/screens/chat/RecalledMemoryPanel.tsx`: PanelHeader with a count, the newest 5 RecalledMemoryRow for the selected turn (auto and search, grouped), "Show all", and rows linking to `/:agentId/memory?bundle=&doc=` (FR-008a)
- [X] T053 [P] [US1] Implement `webui/app/screens/chat/BackgroundJobsPanel.tsx`: BackgroundJob rows from `job` frames + `GET …/jobs`, pieces progress, "Jump to start", Cancel through `POST …/jobs/{id}/cancel`, and newest 5 then "Show all"
- [X] T054 [US1] Implement `webui/app/screens/chat/SessionFilesPanel.tsx` + `webui/app/screens/chat/FilePreviewDialog.tsx`:
  - files from `GET …/topic/{topic}/files` plus `session_file` frames, shown as FileRow
  - FilePreview in a Dialog: CSV as a sortable Table (first 256 KB through `Range`, with a "showing the first 256 KB" note) or Raw; images; markdown through Markdown.tsx
  - Download, Open in new tab, previous/next across the topic's files, and keyboard ← → Esc
- [X] T055 [P] [US1] Implement reply media in `webui/app/screens/chat/ReplyMedia.tsx`: FilePeek for files in a reply, ImageGallery, AudioPlayer (with transcript), VideoPlayer, and FileTypeTag
- [X] T056 [P] [US1] Implement client tools in replies in `webui/app/screens/chat/ClientCalls.tsx`: CallChip for each call (filtered by `visibleClientCalls`), and the library InteractivePage frame hosting the sandboxed iframe with the `window.vizier` bridge, carried over from `legacy/app/components/client/InteractivePage.tsx`
- [X] T057 [US1] Implement `webui/app/screens/chat/AgentStateBanner.tsx`:
  - starting: AgentStartingCard with the ETA from the agents store
  - failed: Banner with the cause, failure count, retry time, "Retry now" (→ `POST /agents/{id}/restart`) and "Open config"
  - offline/reconnected: Banner from the connection state

  Also the "messages wait in line" note
- [X] T058 [US1] Write `webui/e2e/chat.spec.ts` covering quickstart §2 steps 1–8 and §3 steps 2–7 against a dummyplug agent
- [ ] T059 [US1] Walk quickstart §2 and §3 on a running binary (constitution e2e gate), then step §2.10 by hand: compare every Chat state with the design in both themes

**Checkpoint**: Chat is usable end to end. This is the MVP.

---

## Phase 4: User Story 2 - Replace the current WebUI cleanly (Priority: P1)

**Goal**: release builds, the Docker image, the crate and the docs all use the new submodule. Nothing references `legacy/`.

**Independent Test**: quickstart §0.

- [X] T060 [P] [US2] In `.github/workflows/release.yml`, `.github/workflows/publish.yml` and `.github/workflows/docker-publish.yml`:
  - every `actions/checkout` gets `submodules: recursive`
  - Node 22
  - the WebUI step runs `npm ci` in `design/` then `npm ci && npm run build` in `webui/`
  - `cache-dependency-path` lists `design/package-lock.json` and `webui/package-lock.json`
- [X] T061 [P] [US2] Update the panic message in `build.rs` (submodules + `just install`). Check that `Cargo.toml`'s `include` ships `webui/build/client/**` and nothing under `legacy/` or `design/`
- [X] T062 [P] [US2] Check the `Dockerfile` build path: if it builds from source, init submodules and install `design/` + `webui/`. If it copies release binaries, confirm no change is needed and record which it is in the task's commit message
- [X] T063 [P] [US2] Write `webui/README.md`: what it is, and that it builds inside a vizier checkout (needs `../design`); `npm run dev` with the `/api` proxy; `gen:api`, `test`, `e2e`; and the commit order for library changes (design → webui → vizier pins)
- [X] T064 [US2] Update `README.md` and `CLAUDE.md`:
  - in the WebUI section and the build gotcha: the `webui` and `design` submodules, `legacy/` as reference only, `just install`, and the new stack (no recharts or MDX editor; the library's components; Zustand stores in `app/stores/`)
  - fix the stale line in the `design/` paragraph
  - add `docs/src/getting-started/` contributor notes for submodules
- [X] T065 [US2] Run quickstart §0 steps 1–4, including building with `legacy/` moved away, and `cargo package --list`

**Checkpoint**: a clean clone with submodules builds and serves the new app; release and Docker pipelines are ready.

---

## Phase 5: User Story 3 - Day-to-day agent pages (Priority: P2)

**Goal**: Home (court and first run), Core, Memory, Tasks, Skills, Dreams (with the night timeline and change attribution) and Usage.

**Independent Test**: quickstart §5 and §7 steps 1–3, plus `webui/e2e/pages.spec.ts`.

### Server for User Story 3

- [X] T066 [US3] Add an `AgentListing` (`Listing` impl: filters `mine`/`live`/`failed`, sorts `last_active`/`name`) and opt-in paging on `GET /agents` in `src/channels/http/api/v1/agents/mod.rs` (unpaged response unchanged)
- [X] T067 [P] [US3] Add a `TaskListing` (`active`, `last_run_failed`; `next_run`, `name`, `last_run`), opt-in paging, and `last_run: {id, ran_at, outcome} | null` on task items (both shapes) in `src/channels/http/api/v1/agents/task.rs`, using `TaskRunStorage`
- [X] T068 [P] [US3] Add a `SkillListing` (`agent`/`global`; `name`/`updated`), opt-in paging, and `base_hash` handling (`409 stale_content` with the current content) on update in `src/channels/http/api/v1/skills.rs` and `src/channels/http/api/v1/agents/skills.rs`
- [X] T069 [P] [US3] Accept `base_seq` on `PUT /agents/{id}/core` in `src/channels/http/api/v1/agents/core.rs`, answering `409 stale_revision` with `latest_seq` and the content on a mismatch
- [X] T070 [P] [US3] Add bundle sorts `documents`/`updated`/`links` and `total` on `GET /agents/{id}/memory/bundles` in `src/channels/http/api/v1/agents/memory.rs` (existing offset/limit kept)
- [X] T071 [US3] Add `dream_cycle: Option<String>` to `RevisionOrigin` in `src/schema/revision.rs`, filled by `from_session` from a dream session's topic. Extend the existing unit test with a dream session
- [X] T072 [US3] Add the `dream_cycle_id` column (`add_column_if_missing`) and the index `(agent_id, dream_cycle_id)` on `core_revision` and `memory_revision` in `src/storage/sqlite/mod.rs`. Write it from `RevisionOrigin` in `src/storage/sqlite/core_revision.rs` and `src/storage/sqlite/memory_revision.rs` (the single write paths)
- [X] T073 [US3] Add `DreamChange`, `ChangeKind`, `DreamCycle`, `DreamCycleState`, `DreamNight` and `DreamNights` in `src/schema/dream_journal.rs`, plus a pure `change_kind(prev, rev)` and the legacy window attribution as pure functions with `#[cfg(test)]` units (data-model §2.4: wrote/revised/removed/core, windows ±1 min, unattributed)
- [X] T074 [US3] Add storage queries for cycles and their changes (by `dream_cycle_id`, with the legacy-window fallback) and per-night summaries in `src/storage/sqlite/dream_journal.rs`, exposed through `DreamJournalStorage` in `src/storage/dream_journal.rs` and forwarded in `src/storage/mod.rs`
- [X] T075 [US3] Add `GET /agents/{id}/dream/nights?days&tz` and `GET /agents/{id}/dream/cycles?before&limit` in `src/channels/http/api/v1/agents/dream.rs`:
  - the state comes from `DreamStatus`
  - the cost comes from `usage_event` rows with the cycle's topic
  - `#[utoipa::path]`

  See [contracts/http-api.md](contracts/http-api.md)
- [X] T076 [US3] Regenerate `webui/openapi.json` + `webui/app/api/schema.d.ts` with `scripts/openapi.mjs`

### Design library for User Story 3

- [X] T077 [P] [US3] Add the MarkdownEditor component (X76) in `design/src/components/MarkdownEditor/` on Tiptap 3 with markdown in and out (research R5: check for a markdown extension maintained by Tiptap, else `tiptap-markdown`):
  - rich, markdown and split modes
  - compact and full looks, composing EditorToolbar, OutlineNav and `Prose size="document"`
  - Ctrl/Cmd+S → `onSaveShortcut`
  - read-only
  - a round-trip story test with a fixture document of headings, lists, a table, code, links and a quote
- [X] T078 [P] [US3] Add the DateTimeField component (X77) in `design/src/components/DateTimeField/`: a TextField-styled trigger, a month grid + time row popover, keyboard (arrows, PgUp/PgDn, Enter, Esc), `min`, `invalid`, `disabled`
- [X] T079 [P] [US3] Extend the NightStrip (X69) in `design/src/components/NightStrip/NightStrip.tsx`: `Night.live`, `Night.hint` (hover/focus label), `selected`, and `onSelectNight` (nights become buttons). Add stories for selected and running nights
- [X] T080 [P] [US3] Extend the DreamJournalEntry (X56) in `design/src/components/DreamJournalEntry/DreamJournalEntry.tsx`: a `"removed"` change kind and `DreamChange.href`, with a story showing all four kinds

### WebUI for User Story 3

- [X] T081 [P] [US3] Implement `webui/app/stores/drafts.ts`: unsaved text per document key (`core:<agent>`, `skill:<agent>:<slug>`, `memory:<agent>:<bundle>/<path>`) in localStorage with try/catch, plus a restore/discard prompt helper
- [X] T082 [US3] Implement Home (court and first run) in `webui/app/routes/home.tsx`:
  - greeting PageHeader, with the counts taken from the agents store for now
  - the court: the 6 most recently active AgentCard, then "Show all N agents", which opens the paged agent list (SearchField + FilterTabs All/Mine/Live/Failed on `GET /agents?limit`)
  - the first-run variant: SetupStepCard ×3 and the Ollama Callout
  - (design page: Home)
- [X] T083 [US3] Implement Core in `webui/app/routes/agent-core.tsx`:
  - PageHeader meta "CORE.md · rev N · last written by …"
  - MarkdownEditor compact, with UnsavedIndicator and the drafts store
  - Save sends `base_seq`; a `409 stale_revision` opens "Reload theirs / Keep mine"
  - History: a RevisionRow list (newest 20 + LoadMore), DiffView between revisions, and Restore through the existing rollback
- [X] T084 [US3] Implement Memory in `webui/app/routes/memory.tsx` and `webui/app/screens/memory/`:
  - passage search (SearchField → existing query endpoint), with "Nothing matched" EmptyState
  - inside a bundle: Breadcrumb, MemoryGraph neighbourhood (1 hop / 2 hops / Whole bundle, at most 40 stars) and DocumentDetail (links to/from, provenance, Edit through MarkdownEditor, History through memory revisions)
  - "All bundles": the bundle-level MemoryGraph of the 24 largest (Largest/Recently changed/Most linked) + a paged bundle Table + the selected-bundle card (Open, Export .zip, Delete with ConfirmDialog)
  - Import / Export / New memory / New bundle
  - a bundle Picker in server mode
- [X] T085 [US3] Implement Tasks in `webui/app/routes/tasks.tsx` and `webui/app/screens/tasks/`:
  - PagedTable (TASK, SCHEDULE with cron, REQUESTED BY, LATEST RUN Status, ACTIVE Switch)
  - a detail panel (KeyValueList, RunRow list newest 20 + LoadMore, the selected run's reply in Markdown, "Open run conversation")
  - a create/edit dialog (cron or one-time with DateTimeField; requester own/person)
  - Delete with ConfirmDialog
  - the EmptyState "No duties scheduled"
- [X] T086 [US3] Implement Skills in `webui/app/routes/skills.tsx` and the editor in `webui/app/routes/skill-edit.tsx`:
  - a PagedTable of skills (All/Agent/Global), with a detail panel (keywords as Badge, SKILL.md CodeBlock, resources FileRow, invoke hint)
  - the editor: ActionBar with Breadcrumb, UnsavedIndicator, Discard and Save (Kbd Ctrl S); MarkdownEditor full with outline and modes; KeywordInput; resources with DropZone
  - drafts restore
  - `base_hash` on save, with the 409 dialog
  - New skill, and Delete with ConfirmDialog
- [X] T087 [US3] Implement Dreams in `webui/app/routes/agent-dream.tsx`:
  - PageHeader meta (schedule, last and next dream) and "Dream now" (existing trigger)
  - NightStrip from `GET …/dream/nights?days=30&tz`, with per-night hints, a live running night, and selection scrolling to the cycle
  - a DreamJournalEntry feed from `GET …/dream/cycles` (newest 20 + LoadMore), with change lines linking to memory documents and Core revisions (FR-018d)
  - a status card (KeyValueList: state, schedule, dream model, last cost, "reflects on") and the restricted-tools note
- [X] T088 [US3] Implement Usage in `webui/app/routes/usage.tsx` and `webui/app/screens/usage/`:
  - Segment 7 days/30 days/Custom and SortSelect "Break down by" (Activity/Channel/Model)
  - StatGroup (cost, tokens, calls, cache hit rate, spent on failed turns)
  - StackedBarChart (top 6 + Other) with Legend and a dated marker
  - "Where it went" Table with BarList share
  - costliest conversations, opening a SlideOver drawer (conversation usage)
  - the cost-estimate footnote
- [X] T089 [US3] Write `webui/e2e/pages.spec.ts` covering quickstart §5 (UI part) and §7 steps 1–3
- [X] T090 [US3] Walk quickstart §5 and §7 steps 1–3 on a running binary (dummyplug), including the legacy-row checks in §5.4

**Checkpoint**: every day-to-day page works, with CORE and skill saves protected from overwriting newer versions.

---

## Phase 6: User Story 4 - Create, configure and connect agents (Priority: P2)

**Goal**: the New agent wizard, Config, and the Channels/Addons/MCP integration pages.

**Independent Test**: quickstart §7 steps 4–6, plus `webui/e2e/agents.spec.ts`.

- [X] T091 [US4] Add an `McpListing` (filter `down` from live server status; sorts `name`/`tools`) and opt-in paging on `GET /agents/{id}/integrations?family=mcp` in `src/channels/http/api/v1/agents/integrations.rs`
- [X] T092 [P] [US4] Add the ImageCropper component (X79) in `design/src/components/ImageCropper/`: square crop, drag, zoom slider + wheel, keyboard nudging, `onCrop(blob)`, `onCancel`, in a Dialog. Wire SigilPicker's upload story to it in `design/src/components/SigilPicker/SigilPicker.stories.tsx`
- [X] T093 [US4] Implement the model and provider selection composition in `webui/app/screens/agents/ModelSelect.tsx`: Select with a leading ProviderLogo, the catalogue and custom providers from `GET /catalogue/*` and `GET /providers`, the capability line "Text, image and tool input · 200k context · $3 / $15 per M tokens", and a search past ~8 items
- [X] T094 [US4] Implement the New agent wizard in `webui/app/routes/agent-new.tsx` and `webui/app/screens/agents/wizard/`:
  - Stepper Config → System Prompt → Tools → Review, synced to `?step=`
  - Tools: ToggleChip "Always on", SwitchRow (fetch, HTTP, WebUI messaging), ToggleSection for Text-to-Speech, Speech-to-Text, Read Image (InlineSelect vision model), Image Generation and Shell (Local/Docker FilterTabs, image, Dockerfile, environment Textarea)
  - AgentPreviewCard
  - ActionBar Back/Continue
  - nothing is persisted until "Create agent" (`POST /agents`); refused configs show the server's reason in a Notice
- [X] T095 [US4] Implement Config in `webui/app/routes/agent-config.tsx` and `webui/app/screens/agents/config/`:
  - Tabs Identity / System Prompt / Tools / Sharing / Danger Zone
  - Identity: SigilPicker + ImageCropper upload, name, locked Agent ID, description
  - Model: ModelSelect, context window and max output with unit suffixes, thinking depth
  - Behaviour: timeouts, checkpoint %, dream schedule + dream model, silent read
  - Memory index: indexer, embedding provider and model
  - Sharing: ShareList on `GET/PUT /agents/{id}/sharing`
  - the "Saving rebuilds … and stops its running turns" warning before save
  - Danger zone: ConfirmDialog with typed agent ID
- [X] T096 [US4] Implement the shared integration parts in `webui/app/screens/integrations/`:
  - `SelfManagementCard.tsx`: SwitchRow for this page's family only, using `PUT …/integrations/grants`, disabled with a helper for people without permission
  - `ActivityCard.tsx`: an ActivityItem list from `GET …/integrations/events`, newest 20 + LoadMore
- [X] T097 [P] [US4] Implement Channels in `webui/app/routes/integrations-channels.tsx`:
  - an IntegrationCard per Discord/Telegram connection: Status, Switch, SecretField + Replace token, slash command sync, silent reading, provenance, Delete
  - the failed state shows a Notice with the cause and "Replace token" (from `AgentHealthStatus.channels`)
  - add connection; SelfManagementCard (channel) and ActivityCard
- [X] T098 [P] [US4] Implement Addons in `webui/app/routes/integrations-addons.tsx`: a search over `GET /integrations/kinds` addons; an IntegrationCard with SecretField and ToolList; install and unlink; the "That's every addon" Callout; SelfManagementCard (addon) and ActivityCard
- [X] T099 [P] [US4] Implement MCP in `webui/app/routes/integrations-mcp.tsx`:
  - PagedTable of ServerRow (SERVER, STATUS with detail, CATEGORY with the `_mcp` note, TOOLS, ON Switch), All/Down filters
  - the Add/Edit server dialog: Remote URL / Local command FilterTabs, name, command, arguments Textarea mono, KeyValueEditor env with SecretField values, Enabled
  - Delete with ConfirmDialog; SelfManagementCard (mcp) and ActivityCard
- [X] T100 [US4] Write `webui/e2e/agents.spec.ts` covering quickstart §7 steps 4–6
- [X] T101 [US4] Walk quickstart §7 steps 4–6 on a running binary

**Checkpoint**: an agent can be created, configured, shared, connected and deleted from the new UI.

---

## Phase 7: User Story 5 - Sign in, set up and administer (Priority: P2)

**Goal**: Login, Onboarding, and every Settings page.

**Independent Test**: quickstart §1, plus `webui/e2e/admin.spec.ts`.

- [ ] T102 [US5] Add public `GET /status` → `{ online, version: env!("CARGO_PKG_VERSION"), setup_required }` in `src/channels/http/api/v1/mod.rs`, outside the auth layer, with `#[utoipa::path]`
- [ ] T103 [P] [US5] Add a `UserListing` (`role:<name>`; `created`/`username`), a `RoleListing` (`builtin`/`custom`; `people`/`name`, with people counts) and an `ApiKeyListing` (`created`/`last_used`), with opt-in paging on `GET /auth/users`, `GET /auth/roles` and `GET /auth/api-keys` in `src/channels/http/api/v1/auth.rs`
- [ ] T104 [US5] Regenerate `webui/openapi.json` + `webui/app/api/schema.d.ts`
- [ ] T105 [US5] Implement Login in `webui/app/routes/login.tsx`:
  - AuthLayout left panel with LedBoard, Wordmark, the tagline and three capability lines
  - the form (TextField username/password, full-width Button, invalid state)
  - the footer "Server online · <origin> · v<version>" from `GET /status`
  - the invite hint
- [ ] T106 [US5] Implement Onboarding in `webui/app/routes/onboarding.tsx`:
  - compact Stepper Account → Provider → First agent
  - Account: `POST /auth/setup`, with the 8-character minimum and confirm checks
  - Provider: provider connect, reusing T108's composition; the Ollama Callout ("No API key? Ollama runs open models on this machine")
  - First agent: a minimal agent form, then into Chat
  - never lists or mentions dummyplug in copy (FR-017)
- [ ] T107 [P] [US5] Implement Profile and Password in `webui/app/routes/settings-profile.tsx` and `webui/app/routes/settings-password.tsx`: ProfileHeader; Discord/Telegram IDs, usernames and aliases through `GET/PUT /auth/users/me/profile`; change password through `POST /auth/change-password`
- [ ] T108 [P] [US5] Implement Providers in `webui/app/routes/settings-providers.tsx` and `webui/app/screens/providers/`:
  - CONNECTED Eyebrow + ProviderRow list (saved key or URL, usage, Models drawer as a SlideOver, ⋯ menu with edit/delete and a warning about the agents using it)
  - "Add a provider": a search over the catalogue + ProviderTile grid (Popular)
  - ConnectForm (key, OAuth flows for ChatGPT and Copilot as in legacy) and CustomProviderForm (protocol, base URL, model list)
  - "Refresh catalogue"
- [ ] T109 [P] [US5] Implement API keys in `webui/app/routes/settings-api-keys.tsx`: a PagedTable (NAME, PREFIX, LAST USED), a New key dialog, SecretReveal showing the full key once with Copy, and Delete with ConfirmDialog
- [ ] T110 [P] [US5] Implement Users in `webui/app/routes/settings-users.tsx`: a PagedTable with role filter chips and counts, an inline role Select (compact badge value), an Add user dialog (username, password, role), and Delete with ConfirmDialog. "(you)" can't be deleted
- [ ] T111 [P] [US5] Implement Roles in `webui/app/routes/settings-roles.tsx`:
  - a paged RoleRow list (search, All/Built-in/Custom, sorted by people)
  - a one-role editor: PermissionGroup/PermissionRow from `GET /auth/roles/available-permissions`, "Show granted only", built-ins read-only
  - Save role ("Applies to all N people on their next request"), Duplicate, Delete with ConfirmDialog, and "See them in Users →"
- [ ] T112 [US5] Write `webui/e2e/admin.spec.ts` covering quickstart §1 on an empty data dir, and creating an API key, user and role
- [ ] T113 [US5] Walk quickstart §1 on a running binary, timing onboarding to the first reply (SC-003, under 5 min)

**Checkpoint**: a fresh install can be set up and administered entirely from the new UI.

---

## Phase 8: User Story 6 - New capabilities the design adds (Priority: P3)

**Goal**: the Home overview, the "Last 24 hours" dot grid, "Since you were away", and the cross-channel conversation picker, all at scale.

**Independent Test**: quickstart §4 and §6, plus `webui/e2e/scale.spec.ts`.

### Server for User Story 6

- [ ] T114 [US6] Add the `last_seen_at` and `previous_visit_at` columns to `"user"` (`add_column_if_missing`) in `src/storage/sqlite/mod.rs`, with read and write methods in `src/storage/sqlite/user.rs` behind `UserStorage` (`src/storage/user.rs`, forwarded in `src/storage/mod.rs`)
- [ ] T115 [US6] Update the visit on JWT-authenticated requests in `src/channels/http/auth/middleware.rs`:
  - at most once a minute per person, using an in-memory throttle map on `HTTPState` in `src/channels/http/state.rs`
  - when the gap is over 30 min, copy `last_seen_at` into `previous_visit_at` first
  - never for API-key requests
- [ ] T116 [US6] Add the hourly activity query to `src/storage/sqlite/usage.rs` behind `UsageStorage`:
  - `COUNT(DISTINCT turn_id)` per agent per hour bucket over the last 24 h, a turn counted in the hour of its first event, plus per-activity counts for today
  - today's priced groups across a set of agents since a timestamp
  - units on an in-memory DB (the existing usage test setup)
- [ ] T117 [US6] Add `src/channels/http/api/v1/overview.rs` with `GET /overview?tz` (counts from `AgentCommand::HealthCheck` filtered by `user_can_view_agent`; cost through `metrics::estimate_cost`) and `GET /overview/activity?rank`:
  - the top 5 + `all` when there are 7 or more agents, otherwise every agent up to 6
  - `live` from thinking sessions
  - `today_by_kind` when there is exactly one agent
  - nested in `src/channels/http/api/v1/mod.rs`, with `#[utoipa::path]`
- [ ] T118 [US6] Add the `FeedSource` trait and one impl per source (`task_run`, dream cycles via T074, `background_job`, `integration_event`, Discord/Telegram channel replies collapsed per conversation-hour, live start failures pinned first) in `src/channels/http/api/v1/overview/feed.rs`, and `GET /overview/feed?before&limit`:
  - key-set cursor
  - the lower bound is `previous_visit_at`, else 24 h
  - text from stored fields only
  - unit tests for merge order and the cursor
- [ ] T119 [US6] Make `is_non_user_channel` `pub(crate)` and share one copy from `src/storage/sqlite/history.rs`. Add the conversation listing query to `src/storage/sqlite/session.rs` behind `SessionStorage`:
  - grouped by channel, with topic count, latest title and `MAX(session_history.timestamp)`
  - per-kind counts
  - title `LIKE` + message `LIKE` over the agent's newest 5,000 history rows
  - key-set cursor on `(last_activity, channel)`
- [ ] T120 [US6] Add `GET /agents/{id}/conversations?q&channel&cursor&limit` in `src/channels/http/api/v1/agents/conversations.rs`, registered in `src/channels/http/api/v1/agents/mod.rs`: view check, a WebUI person sees only their own WebUI channel, and the 5,000-message bound stated in the `#[utoipa::path]` description
- [ ] T121 [P] [US6] Add `scripts/seed-scale.mjs` (Node ≥ 22, no dependencies), which seeds 50 agents, 300 tasks, 1,000 bundles, 600 conversations and 1,000 roles through the API, for quickstart §6
- [ ] T122 [US6] Regenerate `webui/openapi.json` + `webui/app/api/schema.d.ts`

### Design library for User Story 6

- [ ] T123 [P] [US6] Extend the ActivityGrid (X68) in `design/src/components/ActivityGrid/ActivityGrid.tsx`: `ActivityRow.hints` (24 per-dot tooltips, with dots focusable when set). Add a story with hints for each variant (none, one, four, a thousand)
- [ ] T124 [P] [US6] Add the ConversationRow component (X80) in `design/src/components/ConversationRow/`, built on ListItem: channel kind icon, title with match highlight, place, topic count and time. Add a Picker story with the Today/Yesterday/Older sections

### WebUI for User Story 6

- [ ] T125 [US6] On Home in `webui/app/routes/home.tsx` and `webui/app/screens/home/`:
  - the greeting meta from `GET /overview?tz` ("48 agents at court · 2 live · 1 failed · ~$4.12 today")
  - ActivityGrid from `GET /overview/activity` in every variant (none, one with the summary line from `today_by_kind`, 2–6, 7+ with the "All N" row), with per-dot hints, and the Most active · Mine · Live · Failed switch through `rank`
  - the panel height fixed at 7 rows (FR-018b)
- [ ] T126 [US6] Implement "Since you were away" in `webui/app/screens/home/AwayFeed.tsx`: ActivityItem with sigils from `GET /overview/feed`, failed tone, `href` links, pinned failures first, newest 20 + LoadMore, and EmptyState when nothing happened
- [ ] T127 [US6] Implement the conversation Picker in `webui/app/screens/chat/ConversationPicker.tsx`:
  - server mode with `onEndReached`, on `GET /agents/{id}/conversations`
  - FilterTabs All/WebUI/Discord/Telegram with counts, ConversationRow, Today/Yesterday/Older sections, and "New conversation"

  Wire it into TopicHeader in `webui/app/routes/chat.tsx`. Discord and Telegram conversations open read-only (history, no composer)
- [ ] T128 [US6] Write `webui/e2e/scale.spec.ts` against seeded data: every listed page shows first content in under 2 s, and no response body carries more than one page (network assertions)
- [ ] T129 [US6] Walk quickstart §4 and §6 on a running binary, including `scripts/client-tools-e2e.mjs` and `scripts/durable-turns-e2e.mjs` passing unchanged (SC-009)

**Checkpoint**: Home shows live court activity and what happened while away; every list stays fast at the seeded scale.

---

## Phase 9: User Story 7 - Phone-sized screens and both themes (Priority: P3)

**Goal**: the Mobile Home and Chat designs, plus theme switching on every screen.

**Independent Test**: quickstart §8, plus `webui/e2e/mobile.spec.ts` and `webui/e2e/a11y.spec.ts`.

- [ ] T130 [US7] Make `webui/app/layouts/AppShell.tsx` responsive: below the design's breakpoint, the Sidebar is replaced by the library MobileTopBar (☰ opens the nav in a SlideOver; wordmark on Home, sigil + topic + status in Chat), with no horizontal scroll
- [ ] T131 [P] [US7] Implement Mobile Home in `webui/app/screens/home/MobileHome.tsx`: Wordmark, greeting, compact ActivityGrid, and an AgentRow list per the Mobile design
- [ ] T132 [P] [US7] Implement Mobile Chat in `webui/app/routes/chat.tsx` and `webui/app/screens/chat/`: side panels move into a SlideOver opened from the JobsPill/icon form, the trail uses the mini-timeline summary, and the composer shows "21k / 200k"
- [ ] T133 [P] [US7] Check theme handling across screens: the UserMenu toggle switches without a reload and is remembered per device, and the first paint applies the stored theme before render to avoid a flash (inline script in `webui/app/root.tsx`)
- [ ] T134 [US7] Write `webui/e2e/mobile.spec.ts` (every route at 390 px: `scrollWidth <= clientWidth`; Home and Chat match the Mobile layout) and `webui/e2e/a11y.spec.ts` (axe on every route in both themes, failing on serious or critical; a keyboard-only pass through Login, Chat, Tasks and Roles)
- [ ] T135 [US7] Review §8.1 by hand: every screen in dark and light against the design frames, noting and fixing differences

**Checkpoint**: every screen works at phone width and in both themes.

---

## Phase 10: Polish & cross-cutting concerns

- [ ] T136 [P] Run the library's token lint over the app: make `design/scripts/lint-tokens.mjs` take a path argument, add `"lint:tokens": "node ../design/scripts/lint-tokens.mjs app"` to `webui/package.json`, and fix any findings
- [ ] T137 [P] Add #82 checklist entries for the backward-compat code added here:
  - the `dream_cycle_id` column adds and the legacy time-window attribution (T072–T074)
  - the `last_seen_at`/`previous_visit_at` column adds (T114)
  - the `serde(default)` `sender`/`history_uid` on `SessionFileRecord` (T033)
  - the unpaged-response path kept for old clients on each paged list (T066–T070, T091, T103)

  Use `gh issue view 82` / `gh issue edit` (confirm before editing)
- [ ] T138 [P] Document the new and changed endpoints and the `session_file` frame in `docs/src/api-integration/rest-api.md`, and note the frame in `docs/src/api-integration/client-tools.md`'s frame list where frames are enumerated
- [ ] T139 [P] Add a legacy-link check to `webui/e2e/redirects.spec.ts` (quickstart §9)
- [ ] T140 Confirm no user-facing copy mentions dummyplug: `grep -rni dummyplug webui/app` returns nothing outside dev-only code paths (FR-017)
- [ ] T141 Run `cargo clippy`, `cargo test`, `npm run typecheck && npm test && npm run lint:tokens` in `webui/`, and `npm run typecheck && npm test && npm run lint:tokens` in `design/`. Fix everything
- [ ] T142 End-to-end check with a `dummyplug` agent on a running binary (constitution e2e gate): walk every quickstart.md section (§0–§9) and run the full `npm run e2e` in `webui/`
- [ ] T143 Commit order and pins: commit and push `design/`, then `webui/` (confirm before pushing). Bump both submodule pins in vizier in one commit (`chore: pin design and webui for the rewrite`), and check that `git submodule status` is clean

---

## Dependencies & Execution Order

### Phase dependencies

- **Setup (Phase 1)**: none. T001 must precede T003.
- **Foundational (Phase 2)**: needs Setup. Blocks every story.
- **US1 (Phase 3)** and **US2 (Phase 4)**: both P1, after Foundational. US2 is independent of US1 and can run in parallel.
- **US3, US4, US5 (Phases 5–7)**: P2, after Foundational. They are independent of each other and of US1. US5's Onboarding (T106) reuses T108's provider forms within the same phase.
- **US6 (Phase 8)**: needs T074 (dream cycles for the feed's dream source) from US3, and T082 (Home page) from US3. Everything else in it is independent.
- **US7 (Phase 9)**: needs the screens it adapts: Home (T082/T125) and Chat (T046–T057).
- **Polish (Phase 10)**: after the stories chosen for the release. The spec says vizier serves the new UI only once every P1 and P2 story passes.

### Within each story

Server types → storage → handlers → `openapi.mjs` regeneration → library components → WebUI screens → the e2e spec → the quickstart walk.

### Parallel opportunities

- Phase 1: T006–T009 together after T005.
- Phase 2: T011, T012, T014, T015, T017, T022–T027 together. The server T010 runs alongside.
- US1: the server tasks T033, T036 and T037 alongside T028–T032; the library tasks T039 and T040 alongside the server; and the WebUI panels T050–T053, T055 and T056 together once T045 is done.
- US3: the server tasks T067–T070 together; the library tasks T077–T080 together; screens T083–T088 are independent files.
- US4: T097–T099 together after T096.
- US5: T107–T111 together.
- US6: T121, T123 and T124 alongside the server tasks.
- Across stories: once Foundational is done, US1, US2, US3, US4 and US5 can be split among people.

---

## Parallel Example: User Story 1

```text
# Server, at once:
T033 SessionFileRecord sender/history_uid
T036 Range support on /files/{id}
T037 POST /agents/{id}/restart

# Library, at once:
T039 EmojiPicker (X78)
T040 AgentStartingCard eta

# WebUI panels, after T045:
T050 ErrorNotice   T051 Reactions   T052 RecalledMemoryPanel
T053 BackgroundJobsPanel   T055 ReplyMedia   T056 ClientCalls
```

## Parallel Example: User Story 3

```text
T077 MarkdownEditor   T078 DateTimeField   T079 NightStrip props   T080 DreamJournalEntry props
T067 TaskListing      T068 SkillListing    T069 CORE base_seq       T070 bundle sorts
```

---

## Implementation Strategy

### MVP first (User Story 1)

1. Phase 1 Setup, then Phase 2 Foundational.
2. Phase 3 (US1 Chat). **Stop and check**: quickstart §2–§3 and `e2e/chat.spec.ts`.
3. Phase 4 (US2) so the branch builds and packages cleanly with the new app.

### Incremental delivery

1. Setup + Foundational → the shell with every route.
2. US1 → chat (MVP). US2 → release-ready packaging.
3. US3 → day-to-day pages. US4 → agent lifecycle. US5 → onboarding and admin. **Merge to master once P1 and P2 pass** (spec Assumptions).
4. US6 → Home intelligence and scale. US7 → mobile and themes. Then Polish.

### Notes

- `[P]` tasks touch different files and have no dependency on an unfinished task.
- Each server change is additive and keeps `legacy/`'s callers working until the switch merges, so the server tasks can land on master early if wanted.
- Every library change is committed in `design/` before the WebUI task that uses it, and the vizier commit bumps the pin.
- Pushes to `vizier-lab/webui` or `vizier-lab/design`, and edits to issue #82, are outward-facing: confirm first.
- Commit after each task or logical group with conventional commits (`feat:`, `fix:`, `doc:`, `chore:`).
