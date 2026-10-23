# Research: WebUI Rewrite

Each decision resolves an unknown from the plan's Technical Context. Findings come from reading the current code (paths given), the design library (`design/`, spec 022), and the FigCraft screens.

---

## R1. Where the old and new WebUI live

**Decision**: `git mv webui legacy`. Then `git submodule add git@github.com:vizier-lab/webui.git webui`. The new app takes the `webui/` path.

**Rationale**: `build.rs`, `#[folder = "webui/build/client"]` in `src/channels/http/webui/mod.rs`, `Cargo.toml`'s `include`, `just build`, and both release workflows already point at `webui/`. If the new app takes that path, none of them change. `legacy/` drops out of every build, because nothing references it.

**Alternatives**:
- `webui-legacy/`, or `legacy/webui/`. The user said "rename it into legacy", so the folder is `legacy`.
- New app at `webui-next/`. That would mean editing the build, the embed path, CI and docs twice: once now, and again when legacy is deleted.

## R2. How the new WebUI uses the design library

**Decision**: `"@vizier/design": "file:../design"`, exactly as the library's README prescribes. Add `resolve.dedupe: ["react", "react-dom"]` in Vite and `paths` for the React types in tsconfig. In the CSS entry, add `@import "@vizier/design/styles.css"` and `@source "../../design/src"`.

**Rationale**:
- The library ships TypeScript source with no build step, and documents this consumption path.
- Inside vizier, the two submodule pins are the compatibility contract: a vizier commit pins a `webui` commit and a `design` commit that build together.
- A change to a component shows up in the WebUI on save (Vite HMR follows the symlink), which is what "add or adjust components as needed" requires day to day.

**Consequences**:
- The `webui` repository builds only inside a vizier checkout, or next to a `design` checkout at `../design`. Its README says so, and its CI checks out vizier with submodules, so `../design` exists.
- When the WebUI needs a component change, the order is: commit to `design`, then `webui`, then bump both pins in vizier, in one vizier PR.

**Alternatives**:
- npm git dependency (`github:vizier-lab/design#sha`). It works on its own, but the pin is duplicated: once in `webui/package.json` and once in vizier's submodule pin, and the two drift. It also needs a push before every local test of a component change.
- Nested submodule `webui/design`. vizier would hold two copies of the library at possibly different commits.
- A published npm package. The library is `private` and has no release process, and setting one up is out of scope (Lean).

## R3. WebUI stack

**Decision**:
- **Kept:** React 19, React Router 7 in SPA mode (`ssr: false`), TypeScript 5.9, Tailwind 4, Vite 7, and Zustand for client state (same as `legacy/` and the library).
- **Dropped:** axios, `react-use-websocket`, recharts, react-icons, boring-avatars, `@floating-ui/react`, motion, html2canvas-pro, jspdf, `@uidotdev/usehooks` and the IBM Plex fonts.
- **Replaced:** HTTP and WebSocket go through a small typed client on `fetch` and `WebSocket`. Charts, icons, avatars, sigils, fonts and popovers come from `@vizier/design`.

**Rationale**: Principle I (every dependency must justify itself). The library already covers charts (StackedBarChart, BarList, ActivityGrid, NightStrip), the graph (MemoryGraph on d3-force), icons (lucide-react), sigils, fonts and tooltips. What `legacy/` used those packages for is now a library component.

**Kept from legacy on purpose**: `react-markdown` + `remark-gfm` + `rehype-highlight` (rendering markdown in replies, wrapped in the library's `Prose`), and `nanoid`. The tested pure modules `lib/clientTools.ts`, `lib/trail.ts` and `lib/pageDocument.ts` are copied over with their tests. They hold protocol logic (specs 019 and 023), not UI.

## R4. API types

**Decision**: generate TypeScript types from the server's OpenAPI document (`/openapi.json`, already served through utoipa) with `openapi-typescript` (a dev dependency). The output goes to `webui/app/api/schema.d.ts`, from a checked-in copy of `openapi.json` that `scripts/openapi.mjs` in vizier writes. The WebUI's `api/` client is typed from it. The WebSocket frame types, which aren't in OpenAPI, are written by hand in `app/api/frames.ts` from `contracts/websocket.md`.

**Rationale**: `legacy/app/interfaces/types.ts` is ~1,000 lines of hand-written mirrors of the Rust types, and they drift. Every new endpoint in this feature carries `#[utoipa::path]` anyway (FR-024).

**Alternatives**: hand-written types (today's drift); `ts-rs` on the Rust side (a new crate, and it doesn't cover request and response wrappers).

## R5. Rich-text editor (Core, skill editor, memory edit)

**Decision**: add a **`MarkdownEditor`** component to the design library (new entry X76). It is built on Tiptap 3, which is headless ProseMirror, with markdown in and out. Its `rich | markdown | split` modes drive the library's existing `EditorToolbar`, `OutlineNav` and `Prose` styling. API: `value`/`onChange` as a markdown string, `mode`, `look: "compact" | "full"`, and `onSaveShortcut`. Before adding the markdown extension, check the latest Tiptap release for one maintained by Tiptap; if there isn't one, use the community `tiptap-markdown`.

**Rationale**:
- The design draws its own toolbar (EditorToolbar X50), outline (X51) and document typography (Prose X49).
- `@mdxeditor/editor` (used in legacy) brings its own toolbar and theme. Matching it to the design would mean fighting its styles.
- A headless engine renders through the library's tokens, so it lives in the library with its catalogue entry, per FR-003.

**Alternatives**:
- MDXEditor with its UI hidden. Its imperative API doesn't cover the outline or split mode.
- CodeMirror only. That gives no rich mode, which the design requires.
- Lexical directly. More code for the same result.

## R6. Design library: what's added and what's changed

The inventory from spec 022 lists 139 components, all built and approved. Comparing it with every screen and with the server work in this plan:

**Added** (each gets a story, an inventory id and tests, like the existing ones):

| Id | Component | Why |
|----|-----------|-----|
| X76 | MarkdownEditor | R5 |
| X77 | DateTimeField | One-time task date and time. This is an inventory "gap" (`DatePicker.tsx`) that parity needs (FR-007) |
| X78 | EmojiPicker | What the add-reaction button opens. Inventory gap (`EmojiPickerPopup.tsx`) |
| X79 | ImageCropper | The crop step of the sigil upload in Config (SigilPicker X22 opens it). Inventory gap (`AvatarCropModal.tsx`) |
| X80 | ConversationRow | A conversation-picker row (channel icon, name, place, topic count, time). The Scaling sheet draws it, but the inventory folded it into Picker rows without its own entry. Built on ListItem |
| X81 | LoadMore | The "Load older" / "Show all" control and its loading state, used by every feed, history and side panel (FR-011) |

**Changed** (props added; nothing existing changes meaning):

| Component | Change | Why |
|-----------|--------|-----|
| NightStrip (X69) | `selected`, `onSelectNight(i)`, `Night.live`, a per-night label on hover/focus ("Oct 19 · 2 written · 2 revised · CORE"), and nights rendered as buttons | FR-018d: a night opens its journal entry, and the running cycle shows as live |
| ActivityGrid (X68) | A per-dot tooltip with the exact count and hour; a `byKind` line source for the one-agent summary | FR-018b: hover shows the exact count |
| DreamJournalEntry (X56) | `DreamChange.kind` gains `"removed"`; changes take an `href` | FR-018c lists deletions too; each change links to its document or revision |
| Picker (X41) | `onEndReached` + `loading` for server-paged lists; `search.mode: "server"` (filtering doesn't happen client-side) | Conversation and bundle pickers page from the server (FR-020, FR-022) |
| AgentStartingCard (X58) | `eta` text prop | FR-021: time left, computed by the WebUI |
| Status (D24) | No change | Its widened state table already covers every state the new endpoints return |

**Kept in the WebUI as screen code, not library** (these are compositions of library parts, with no new visual element): page layouts (the shell, the Settings two-column layout, the Chat three-column layout, the Login split), provider connect and custom-provider forms, the models drawer, the usage conversation drawer, and the embedding-index section. This matches the library's rule that screen layouts aren't components (inventory, "Excluded").

## R7. Paging, search and counts for lists (FR-022)

**Decision**: one shared query shape and one response shape for every list in the Scaling rules:

```
?q=&filter=&sort=&order=asc|desc&offset=&limit=
→ { items: T[], total: number, counts: { [filter]: number } }
```

Implementation:
- A generic `Page<T>` plus `ListQuery` in `src/channels/http/models/page.rs`.
- A `paginate(items, query, &dyn Listing<T>)` helper, where a small `Listing` trait per entity supplies the text search fields, the filter predicates and the sort keys (Principle II: one implementation, no per-endpoint copies).
- Agents, tasks, skills, users, roles, MCP servers and API keys are stored as JSON blobs and read whole today. They are filtered in Rust after the existing storage read.
- Bundles and conversations page in SQL (R9, and the existing `list_bundles` offset/limit).

**Compatibility**: an endpoint gets the paged shape only when `limit` is present. Without it, it returns exactly today's response (SC-009, FR-022).

**Rationale**: even 1,000 roles or 300 tasks is a single-digit-millisecond read from embedded SQLite. The cost the Scaling rules care about is the WebUI rendering and transferring the whole set, and paging at the HTTP edge removes that. Moving each blob table to column-backed SQL queries would mean schema migrations for no measurable gain (Lean). If a list ever grows past that, its `Listing` impl moves to SQL without changing the contract.

## R8. Home overview, activity grid and away feed

**Overview** (FR-018): `GET /api/v1/overview`.
- **Counts:** from the existing `AgentCommand::HealthCheck` (the same path as `GET /agents/health`), filtered with `user_can_view_agent`.
- **Today's cost:** from `UsageStorage`, grouped by `(provider, model)` across the visible agents since local midnight, priced with the one existing `metrics::estimate_cost`. The client sends its timezone offset (`?tz=+07:00`), because "today" is the person's day.

**Activity** (FR-018a): `GET /api/v1/overview/activity`.
- **Query:** one SQL over `usage_event` (`at >= now-24h`, index `usage_event_agent_at`): `COUNT(DISTINCT turn_id)` grouped by `agent_id` and hour bucket, plus `activity` for the per-kind split.
- **Rows:** a turn counts in the hour of its first event. The server ranks agents by turns, returns the top 5 (or all of them when there are 6 or fewer) plus an `all` row, and marks `live` from the agents' `is_thinking` sessions.
- **Response size:** at most 7 rows × 24 numbers, whatever the agent count.

**Turn kinds**: from `usage_event.activity` (`UsageActivity`: chat, scheduled, dreams, background, compaction, vision). The one-agent line shows chat turns, scheduled tasks and dream stages, as drawn.

**Away feed** (FR-019): `GET /api/v1/overview/feed?before=&limit=20`. Rather than a new event table, it is a merge of what is already stored:

| Event | Source |
|-------|--------|
| Task answered / no response / interrupted | `task_run` |
| Dream cycle finished, with its change counts | `dream_journal` + R10 |
| Background job reported / cancelled / interrupted | `background_job` |
| Integration changed, or a channel reader failed | `integration_event` |
| Agent failing to start now | live `HealthCheck` (pinned first, not historical) |
| Agent replied to someone in a channel | `session_history` responses on Discord/Telegram channels, collapsed to one item per conversation |

Each source maps to `FeedEvent` through a small `FeedSource` trait (one impl per source; Principle II), and they are merged newest first with a key-set cursor. The feed's lower bound is the person's previous visit (below), or the last 24 hours if there isn't one.

**Last visit**:
- **Storage:** two columns added to `"user"` with `add_column_if_missing`: `last_seen_at` and `previous_visit_at`.
- **Update:** the auth middleware updates `last_seen_at` on JWT-authenticated requests (not API keys), at most once a minute per person (an in-memory throttle on `HTTPState`). When the gap since the previous `last_seen_at` is over 30 minutes, it first copies that value into `previous_visit_at`.
- **Meaning:** "Since you were away" therefore means since the last gap of 30 minutes or more.
- **Compatibility:** the column add is backward-compat code, so it gets a #82 entry.

**Alternatives**:
- A new `activity_event` table written by every subsystem. That means a write on every path and a second record of facts that are already stored.
- Client-side "last visit" in localStorage. It's per device, and the spec's entity is per person.

## R9. Conversation listing (FR-020)

**Decision**: `GET /api/v1/agents/{id}/conversations?q=&channel=&cursor=&limit=`.
- **Rows:** one row per channel conversation (`session_detail` grouped by `channel`), with its topic count, the latest title, a place label, and the last activity time from `MAX(session_history.timestamp)` on `idx_sh_session`.
- **Scope:** non-person channels are excluded with the existing `is_non_user_channel`, made `pub(crate)` and shared.
- **Channel kind:** derived from the channel slug prefix through `VizierChannelId`'s existing parser. The `counts` per channel kind come from a `GROUP BY`.
- **Search:** `q` matches the title (`LIKE`), and message text through a `LIKE` over `session_history.data`, limited to the agent (`idx_sh_agent_time`) and to the newest 5,000 rows.
- **Paging:** a key-set cursor on `(last_activity, channel)`.

**Rationale**: the scale the spec gives (600 conversations) is well within a bounded `LIKE`. FTS5 over history would be a new derived index to keep in step, and that waits until measured (Lean). The bound is documented in the API description so the result is honest ("searches the latest 5,000 messages").

## R10. Dream cycle attribution (FR-018c)

**Finding**: a dream session is `VizierSession(agent, VizierChannelId::Dream(..), Some(cycle_id))` (`scheduler/dream/mod.rs:183`), so every tool call inside a dream already knows its cycle id through `ToolContext.session.2`. `RevisionOrigin::from_session` maps a dream session to `RevisionTrigger::Dream`, but drops the id.

**Decision**:
- `RevisionOrigin` gains `dream_cycle: Option<String>`, filled by `from_session` from the dream session's topic.
- `core_revision` and `memory_revision` gain a nullable `dream_cycle_id` column (`add_column_if_missing` + index `(agent_id, dream_cycle_id)`), written by the existing single write paths (`set_agent_core`, `BundleMemoryStore::{write_memory, delete_memory, …}`).
- **Change kind:** a memory revision whose previous revision is absent or deleted is `wrote`; `deleted = 1` is `removed`; anything else is `revised`. A CORE revision is `core`.
- **Legacy rows** (`trigger = 'dream'`, no cycle id) are attributed by time window: the revision's `created_at` falls between the cycle's first and last `dream_journal.timestamp`, plus the cycle's `Extracting.started_at` when it is known. A legacy row that falls in no window is reported in an `unattributed` count on the nights endpoint, never added to a night (spec FR-018c).
- The time-window fallback and the column adds are backward-compat code, so they get a #82 entry.

**Endpoints**:
- `GET /agents/{id}/dream/nights?days=30&tz=`: one entry per local night with the cycle (if any), its `written`, `revised`, `removed` and `core` counts, and its state.
- `GET /agents/{id}/dream/cycles?before=&limit=20`: one card per cycle (stages, sessions read, sources, changes with links). The journal cards are drawn per cycle, while `dream_journal` rows are per stage and session.

## R11. Recalled memory in a turn (FR-018e)

**Finding**: `retrieve_auto_context` (`agents/process.rs:1048`) returns `Vec<MemoryPassageResult>` (bundle, path, ordinal, score, text) and only logs a count. `memory_search` results reach the model only inside the script's output.

**Decision**: a new history entry, `SessionHistoryContent::Recall { origin: Auto | Search, passages: Vec<RecalledPassage> }`. `RecalledPassage` holds `bundle`, `path`, `ordinal`, `score` and `title`, never the text, matching how passages are stored (spec 009).
- **Auto:** recorded through the turn's `StepRecorder` right after retrieval, before the first model call.
- **Search:** `memory_search` pushes its passages into a new turn-scoped collector, `ToolContext.recalled: Arc<Mutex<Vec<RecalledPassage>>>`, which works the same way as the existing `loaded_images`. The agent loop drains it after each round and records one `Recall { origin: Search }`.
- **Sending:** both reach the WebUI as `history_entry` frames (spec 023) and come back on reload through the history endpoint.
- **Model input:** `history_entries_to_messages` skips `Recall`, as it skips `ClientToolCall`, so nothing changes in what the model sees.

**Alternatives**: a field on `VizierResponse`. It would only appear when the reply lands, which loses the panel during the turn and is wrong for resumed turns.

## R12. Session files (FR-018f)

**Finding**:
- **Storage:** `SessionFileStorage::list_session_files(session)` exists. `SessionFileRecord` has name, mime, size, `file_id` and `added_at`, but no sender or turn.
- **Writers:** eight writers (TTS, STT, image generation, `read_image`, session-file tools, uploads).
- **HTTP:** none lists files. `/files/{file_id}` downloads one.

**Decision**:
- `SessionFileRecord` gains `sender: Option<String>` and `history_uid: Option<String>`. The record is a JSON blob, so this needs `#[serde(default)]` and no schema change. Each writer sets them; records written before show "unknown sender".
- New route `GET …/channel/{channel}/topic/{topic}/files?offset=&limit=`, newest first, with view access checked like `get_topic_history`.
- A new `SessionFrame::File(SessionFileRecord)`, published by `save_session_file`'s callers after the write. It goes out as socket frame `session_file`.
- **Previews:** text and CSV are fetched with an HTTP `Range` header (first 256 KB) so a large file isn't loaded whole, and the WebUI says when it is truncated. `/files/{file_id}` gains `Range` support (`206 Partial Content`) if it doesn't have it.

## R13. Server status on sign-in (FR-023)

**Decision**: `GET /api/v1/status` (public): `{ online: true, version, setup_required }`. The address shown is the page's own origin. `setup_required` mirrors `/auth/setup-status`, so Login and Onboarding make one call.

**Alternative**: changing `/ping` from `"pong"` to an object. That breaks existing callers.

## R14. Context ring (FR-018g)

**Finding**: `VizierResponseStats` already carries `current_context_size` and `context_window` on every reply.

**Decision**: no server change. The WebUI reads the latest reply's stats from history (and from each new `response` frame). Checkpoint dividers use the stats of the reply before the checkpoint. FR-018g is met by existing data.

## R15. Start progress (FR-021)

**Finding**: `SetupBoard` (`src/setup.rs`) already records per-agent build phases and downloads (`completed`, `total`, `status`) for Ollama pulls and local model downloads. `GET /api/v1/utils/setup-state` serves it, filtered by view access. Failure state, error and next retry come from health (#52).

**Decision**: no server change. The WebUI polls `setup-state` every second while an agent is starting, and works out time left from the change in `completed` over the last 5 samples. FR-021 is met by existing data.

## R16. Stale saves (FR-025)

**Decision**:
- `PUT /agents/{id}/core` accepts an optional `base_seq`: the revision the editor loaded. If it is present and isn't the latest `core_revision.seq`, the server returns `409 stale_revision` with the latest seq and content.
- Skill updates accept an optional `base_hash` (sha256 of the content the editor loaded) and return `409 stale_content` the same way.
- Without the field, today's behaviour is unchanged.
- The WebUI offers "Reload theirs" or "Keep mine" (overwrite, by sending the new base).

## R17. Links from the legacy WebUI (FR-016)

**Decision**: the new route table keeps legacy paths where the page still exists: `/:agentId/chat[/:topicId]`, `/:agentId/core`, `/:agentId/memory`, `/:agentId/tasks`, `/:agentId/skills`, `/:agentId/config`, `/:agentId/integrations/*`, `/:agentId/usage`, `/:agentId/dream`, `/agents/new`, `/login` and `/onboarding`. Redirects cover the rest: `/settings` → `/settings/profile`, and `/:agentId/settings` → `/:agentId/config` (kept from legacy). An unknown path goes to Home.

## R18. Release builds and CI

**Decision**:
- Every `actions/checkout` in `release.yml`, `publish.yml` and `docker-publish.yml` gets `submodules: recursive`.
- The "Build WebUI" step runs `npm ci` in `design/` first (for its fonts and lucide), then in `webui/`. Node becomes 22, to match the library and Dockerfile.
- `Cargo.toml`'s `include` stays `webui/build/client/**/*`, so the crate still ships only the built files.
- `build.rs` doesn't change: it builds `webui/` when `webui/node_modules` exists. The `design` install is a documented step in `just install` (`git submodule update --init` + `npm i` in `design/` and `webui/`).

## R19. Testing the WebUI

**Decision**:
- **Unit:** vitest for pure modules (the API client's paging helpers, the trail and client-tools modules carried over, the frame reducer).
- **Types:** `npm run typecheck`.
- **End to end:** `npm run e2e`, a Playwright smoke suite in the `webui` repository. It drives a running vizier with a dummyplug agent: onboarding on an empty data dir, login, chat with a tool call, queued message, Stop, a checkpoint, reopening mid-turn, and the Home, Dreams and Memory pages loading with seeded data.
- **Server:** additions get `cargo test` units where they are pure (paging, change-kind derivation, hour bucketing, feed merge order), and quickstart steps against dummyplug (constitution e2e gate).

**Rationale**: the constitution requires dummyplug e2e for agent-observable and HTTP/WebUI changes. Playwright is already a dependency of the design library's test setup, so it adds no new kind of tool to the project.
