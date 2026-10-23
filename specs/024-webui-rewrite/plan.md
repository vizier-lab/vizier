# Implementation Plan: WebUI Rewrite

**Branch**: `024-webui-rewrite` | **Date**: 2026-10-23 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/024-webui-rewrite/spec.md`

## Summary

Replace the WebUI with a rewrite built on the "Celestial Court" design:

- **Repository moves:** the current `webui/` moves to `legacy/`, unbuilt and unserved. The new app is the submodule `vizier-lab/webui`, at the same `webui/` path, so `build.rs`, the embed folder, the crate `include` and CI keep their paths (research R1).
- **Design library:** every screen is composed from `@vizier/design` (the `design/` submodule), consumed by `file:../design` as its README prescribes (R2). Six components are added to the library (MarkdownEditor, DateTimeField, EmojiPicker, ImageCropper, ConversationRow, LoadMore) and five get additive props (NightStrip, ActivityGrid, DreamJournalEntry, Picker, AgentStartingCard), each with a story, before any screen uses it (R6, [contracts/design-library.md](contracts/design-library.md)).
- **Server additions:** these supply what the design shows and the server lacks:
  - opt-in paging, search and counts on list endpoints (R7)
  - a Home overview, hourly activity and away feed (R8)
  - a cross-channel conversation list (R9)
  - dream-cycle attribution on revisions, with nights and cycles endpoints (R10)
  - a recalled-passage history entry (R11)
  - session-file listing, plus a frame and Range downloads (R12)
  - a public status endpoint (R13)
  - stale-save detection (R16)

  Context use and start progress are already served (R14, R15).

## Technical Context

**Language/Version**: Rust 2024 edition (server, current toolchain); TypeScript 5.9, React 19 (WebUI and design library)

**Primary Dependencies**:
- Server: axum, rusqlite (bundled), utoipa (all existing; no new crates).
- WebUI: React Router 7 (SPA, `ssr: false`), Tailwind 4, Vite 7, Zustand, react-markdown + remark-gfm + rehype-highlight, nanoid, `@vizier/design` (file dependency); dev: openapi-typescript, vitest, Playwright.
- Design library: adds Tiptap 3 for MarkdownEditor (R5).

**Storage**: embedded SQLite. Additive columns only (`core_revision.dream_cycle_id`, `memory_revision.dream_cycle_id`, `"user".last_seen_at`/`previous_visit_at`), plus JSON fields on `session_file.data` and a new `session_history` content type. No new tables ([data-model.md](data-model.md) §1).

**Testing**:
- Server: `cargo test` for pure parts (paging/`Listing`, change-kind derivation, hour bucketing, feed merge, legacy window attribution) and `cargo clippy`.
- WebUI: `npm run typecheck`, vitest for `lib/` (frame reducer, paging, client tools, trail), and the Playwright e2e/a11y suite.
- Library: `npm run typecheck && npm test && npm run lint:tokens`.
- End to end: the dummyplug agent steps in [quickstart.md](quickstart.md), per the constitution's e2e gate.

**Target Platform**: the existing single binary on Linux/macOS/Windows and the Docker image, serving the WebUI to evergreen desktop and mobile browsers.

**Project Type**: web application (Rust server + SPA), with the SPA and its component library in their own repositories as submodules.

**Performance Goals**: first content on every page under 2 s locally with 50 agents, 1,000 bundles, 600 conversations and 300 tasks (SC-004); first sign of work within 1 s of sending, and a mid-turn reopen showing the turn within 2 s (SC-005).

**Constraints**:
- No page loads more than one page of any list (FR-011/FR-022).
- The API stays backward compatible (SC-009).
- WebUI assets stay embedded in the binary (Principle III).
- No dummyplug in user-facing copy (FR-017).
- WCAG AA in both themes (SC-008).

**Scale/Scope**: 21 screens plus variants (Home first run, Chat collapsed, Skill editor, Mobile Home/Chat); about 30 routes; 9 new server endpoints and 10 changed ones; 6 new and 5 changed library components.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle / gate | Assessment |
|------------------|------------|
| **I. Lean by Default** | ✅ No new Rust crates. The WebUI drops 11 dependencies the library now covers (R3). The overview, feed and conversation list are queries over existing tables, not a new event table (R8, R9). Paging is filtered in memory at the HTTP edge instead of migrating blob tables (R7). Context ring and start progress reuse existing data (R14, R15). The one new library dependency (Tiptap) has a concrete need no existing package meets (R5). |
| **II. DRY via traits** | ✅ One `ListQuery`/`Page<T>` with a per-entity `Listing` impl, not per-endpoint paging code. One `FeedSource` impl per feed source, merged in one place. Change-kind derivation lives in one function used by nights and cycles. Revision attribution goes through the existing single write paths via `RevisionOrigin`, never in callers. |
| **III. Self-contained runtime** | ✅ The WebUI is still built at build time and embedded (`rust_embed`, unchanged folder). No runtime fetches: fonts and icons come from the library's bundled packages, and the emoji data is bundled (X78). |
| **IV. Portability** | ✅ No platform code. CI checks out submodules recursively, and cross targets are unaffected (no new system libraries). |
| **V. Unified errors & observability** | ✅ New handlers return `crate::Result` and use `err_response`, and logging is `tracing` only. The new endpoints add no content to telemetry. |
| **Gate: clippy/test/typecheck** | ✅ Planned per Testing above. The WebUI typecheck now runs in the `webui` submodule. |
| **Gate: manual verification for build-pipeline changes** | ✅ The build, CI and Docker changes (R18) are run for real in quickstart §0, including the crate package listing. |
| **Gate: dummyplug e2e** | ✅ Quickstart §2–§7 are dummyplug steps. Where the dummyplug model can't produce a behaviour (dream writes), the step says how it is driven. |
| **Convention: #82 migration tracking** | ✅ Column adds (R8, R10), the legacy dream-window attribution (R10) and `session_file` default fields (R12) each get a #82 checklist entry when added. |

**Post-design re-check (after Phase 1)**: still passes. The contracts add no type-branching dispatch: the feed's `kind` is data, rendered by one WebUI component. No external service is added, and every compatibility path is opt-in. Nothing goes in Complexity Tracking.

## Project Structure

### Documentation (this feature)

```text
specs/024-webui-rewrite/
├── plan.md              # This file
├── research.md          # R1–R19 decisions
├── data-model.md        # storage changes, server types, WebUI state
├── quickstart.md        # dummyplug + Playwright checks per user story
├── contracts/
│   ├── http-api.md      # new and changed endpoints
│   ├── websocket.md     # frames the WebUI handles (+ session_file)
│   ├── design-library.md# library additions and changes
│   └── webui-routes.md  # repo layout, routes, redirects
└── tasks.md             # /speckit-tasks output
```

### Source Code (repository root)

```text
vizier/
├── legacy/                          # git mv webui legacy (unchanged contents)
├── webui/                           # NEW submodule vizier-lab/webui (layout: contracts/webui-routes.md)
├── design/                          # submodule: + X76–X81, NightStrip/ActivityGrid/DreamJournalEntry/Picker/AgentStartingCard props
├── .gitmodules                      # + webui
├── justfile                         # install: submodule init + npm i in design/ and webui/
├── .github/workflows/{release,publish,docker-publish}.yml   # submodules: recursive; npm ci design + webui; node 22
├── scripts/
│   ├── openapi.mjs                  # writes webui/openapi.json from a running server
│   └── seed-scale.mjs               # quickstart §6 data
└── src/
    ├── channels/http/
    │   ├── models/page.rs           # ListQuery, Page<T>, Listing, paginate
    │   ├── auth/middleware.rs       # last_seen_at / previous_visit_at (throttled)
    │   └── api/v1/
    │       ├── mod.rs               # + /status, /overview nest
    │       ├── overview.rs          # NEW: overview, activity, feed (+ feed sources)
    │       ├── files.rs             # Range support
    │       ├── auth.rs              # paged users/roles/api-keys
    │       ├── skills.rs            # paged; base_hash
    │       └── agents/
    │           ├── mod.rs           # paged agents; + conversations route
    │           ├── conversations.rs # NEW
    │           ├── channel.rs       # + topic files route
    │           ├── core.rs          # base_seq
    │           ├── dream.rs         # + nights, cycles
    │           ├── task.rs          # paged; last_run on items
    │           ├── memory.rs        # bundle sorts
    │           └── integrations.rs  # paged MCP list
    ├── schema/
    │   ├── revision.rs              # RevisionOrigin.dream_cycle
    │   ├── history.rs               # SessionHistoryContent::Recall (skipped for model input)
    │   ├── session_file.rs          # sender, history_uid
    │   └── dream_journal.rs         # DreamCycle, DreamNight, DreamChange
    ├── storage/
    │   ├── sqlite/mod.rs            # column adds + indexes
    │   ├── sqlite/{core_revision,memory_revision}.rs   # write/read dream_cycle_id; cycle queries
    │   ├── sqlite/usage.rs          # hourly activity query, today's cost across agents
    │   ├── sqlite/session.rs        # conversation listing query
    │   └── sqlite/user.rs           # visit columns
    ├── agents/
    │   ├── process.rs               # record Recall{Auto} after retrieve_auto_context
    │   ├── agent/mod.rs             # drain ToolContext.recalled per round → Recall{Search}
    │   └── tools/{mod.rs, vector_memory/}  # ToolContext.recalled; memory_search pushes passages
    └── transport.rs                 # SessionFrame::File
```

**Structure Decision**: a web application split across three repositories (the vizier server, the `webui` app, and the `design` library), joined by vizier's submodule pins. Server work stays in the existing `src/channels/http/api/v1` and storage modules. The one new handler module (`overview.rs`) and one new agent sub-route module (`conversations.rs`) follow the existing per-area layout. The WebUI's internal layout is fixed in [contracts/webui-routes.md](contracts/webui-routes.md).

## Delivery order

Each step leaves vizier building and serving a working WebUI.

1. **Library first** (`design/`): X76–X81 and the five prop changes, with stories and tests. Bump the pin.
2. **Server additions** behind their opt-in shapes (R7–R13, R16), each with `cargo test` units, OpenAPI annotations and its quickstart section. Nothing in the legacy WebUI breaks, because every change is additive.
3. **WebUI skeleton** in the new repo: the app shell, auth, the API client from `openapi.json`, stores, the frame reducer (ported tests), theme, and the route table with redirects.
4. **P1 Chat**, then **P2** pages (day-to-day; create/config/integrations; onboarding/settings), then **P3** (Home and Dreams visualisations on the new endpoints; scale; mobile).
5. **Switch**: `git mv webui legacy` + add the submodule, update `just install`, CI, Docker and docs (README, CLAUDE.md, contributor docs). Run quickstart §0. The switch lands on master only once P1 and P2 pass (spec Assumptions). On this feature branch it can happen early, so the new app is developed in place at `webui/` with Vite's dev server proxying `/api` to a running vizier (`npm run dev`). Master keeps serving the old WebUI until the branch merges.

## Complexity Tracking

No constitution violations; nothing to justify.
