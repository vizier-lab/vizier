# Implementation Plan: HTML Generative UI

**Branch**: `063-html-generative-ui` | **Date**: 2026-11-16 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/063-html-generative-ui/spec.md`

## Summary

This replaces the json-render client tool `ui_render` with a **native** turn-ending tool of the same name. It takes a complete HTML page, or a set of exact find/replace edits against the page's latest version.

**Server.**
- Pages and their versions are stored: metadata in SQL, HTML in the blob store, fenced on the agent's scope.
- Each version is drawn from a `UiPage` history entry.
- When a tab watches the conversation, the version is checked in that tab over a bus request (`NoResponders` means no tab, so the turn never waits for nobody).

**Browser.**
- Pages run in one sandbox host: an opaque-origin iframe whose CSP blocks script network access and allows `https` media.
- The host loads the **Vizier kit**: tokens, base CSS, `<vz-*>` custom elements that render the existing `@vizier/design` components, and bundled libraries such as `three`.
- Pages get a versioned bridge, `window.vizier` (state, send, files, worker). Heavy work goes to `vizier.worker`, so a page cannot freeze the WebUI even where frames share its thread.

**Retirement.** The tool is offered only in WebUI and project conversations, by channel kind. Both old tools are retired, and their saved pages still draw.

## Technical Context

**Language/Version**: Rust (workspace edition, as `Cargo.toml`); TypeScript 5 + React 19 (webui, design)

**Primary Dependencies**:
- Existing only: axum (`vizier-http`), the `vizier-schema` bus, `BlobStore`, React Router 7, Vite, `@vizier/design`, zod (kit attribute schemas moved from `lib/generativeUi/catalog`), `three` (already a webui dependency).
- No new crates or npm packages.

**Storage**:
- SQL tables `ui_page` and `ui_page_version` (SQLite `init_ui_page_schema`; PostgreSQL `0011_ui_pages.sql`).
- HTML in the `BlobStore` under `ui-pages/`.

**Testing**:
- `cargo test --workspace`: edit application, the version race, availability, fence rows, `fill_ui_pages`.
- `cd webui && npm test` (vitest: document builder, CSP, bridge, kit attributes) and `npm run typecheck`.
- `just test-scripts`: the kit reference sync.
- End to end: dummyplug steps in [quickstart.md](quickstart.md), `cargo test --test dummyplug_e2e ui_pages`, `node scripts/ui-pages-e2e.mjs`, and the browser checks with the preview tools.

**Target Platform**:
- The server on Linux, macOS and Docker.
- The desktop app on macOS, Windows and Linux (Tauri: WebView2 on Windows, WebKit on macOS and Linux).
- Browsers: Chromium, Firefox, Safari.

**Project Type**: a web service with an embedded SPA, plus a desktop shell.

**Performance Goals**:
- A small edit is applied and saved in under 50 ms on the server.
- Verification adds at most about 2.5 s when a tab is open, and nothing when none is.
- The kit loads from cache in under 100 ms.

**Constraints**:
- No script network access from pages.
- Offline-capable kit.
- 1 MiB per version.
- Tool description fixed, so the prompt cache holds.
- Fleet-correct: verification over the bus, and writes fenced.

**Scale/Scope**:
- Expected tens of pages per conversation and hundreds of versions per agent.
- About 12 kit elements and one bundled library (`three`) at first.

## Constitution Check

| Gate | Status |
|---|---|
| **I. Lean** | Pass. No new dependency. One new optional trait method, `VizierTool::guide`, with a real caller (`ui_render`) and an obvious second one: any tool whose reference outgrows its description. The kit reuses the design components instead of re-implementing them. |
| **II. Trait-based** | Pass. `ui_render` is a `VizierTool` registered in `BaseTools::new()`. Storage is one `UiPageStorage` on `SqlStorage`, with no PostgreSQL copy. Availability is the tool's `available(ctx)`, with no branching in dispatch. The kit elements are data-driven from one manifest. |
| **III. Self-contained** | Pass. Kit assets are embedded in the binary through the WebUI build. Pages never need the internet (`https` media is optional content). |
| **IV. Portable** | Pass. Hang isolation differs per browser engine; FR-016a covers WebKit with the worker helper, the guide and the crash-loop marker (research R7). Blob keys are ASCII, which passes `windows_segment_problem`. |
| **V. Errors / tracing** | Pass. `crate::Result` and `tracing` throughout. Refusals are `VizierError`s carrying a JSON message. |
| Feature flags | None needed. Generative UI is part of the HTTP surface, not an optional capability. |
| Roles independent | Pass. Verification goes through `Messaging`. The tool does not know which gateway answers. |
| Design before UI code | **Required**: `mockups/page-frame.html` is the first UI task (FigCraft MCP unreachable, so an HTML mockup). |
| dummyplug e2e | Pass. quickstart.md scripts steps 0–6 as dummyplug steps. |
| #82 | Add these entries: the WebUI renderers kept for `render_interactive_ui` and json-render `render`; `lib/generativeUi/` (read-only); the `ui_components`/`render_interactive_ui` retired names; dropping stale client `ui` declarations. |

Re-checked after Phase 1 design: no new violations.

## Project Structure

### Documentation (this feature)

```text
specs/063-html-generative-ui/
├── spec.md
├── plan.md               # this file
├── research.md           # R1–R15
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── ui-render-tool.md
│   ├── verify-protocol.md
│   ├── bridge.md
│   └── http.md
├── mockups/page-frame.html   # first UI task (tasks.md)
└── checklists/requirements.md
```

### Source Code

```text
crates/vizier-core/
├── assets/ui-kit/REFERENCE.md                # the tool's guide (R2, R11)
├── src/ui_pages/                             # NEW: the service
│   ├── mod.rs        # UiPages: create / replace / apply_edits / verify / state / delete
│   ├── edits.rs      # pure: apply edits in order, exactly-once matching (unit-tested)
│   ├── limits.rs
│   └── verify.rs     # bus request on ui.verify.{session}, outcome mapping
├── src/agents/tools/ui_pages.rs              # NEW: the ui_render VizierTool (+ guide, available)
├── src/agents/tools/mod.rs                   # ToolCategory::Ui (23rd); VizierTool::guide; RETIRED_TOOL_NAMES
├── src/agents/tools/python/docs_tools.rs     # describe_tool_function adds `guide`
├── src/agents/client/declare.rs              # drop stale client `ui` declarations (#82)
├── src/schema/history.rs                     # SessionHistoryContent::UiPage; skipped for the model
├── src/schema/request.rs                     # ClientEvent.page source
├── src/transport/events.rs                   # SessionFrame::UiPageVerified / UiPageState
├── src/storage/ui_page.rs                    # NEW: UiPageStorage trait + records
├── src/storage/sqlite/ui_page.rs             # NEW: SQL impl (shared dialect) + init_ui_page_schema
├── src/storage/sql/postgres/migrations/0011_ui_pages.sql
├── src/storage/sqlite/fence_tests.rs         # + one test per fenced write
├── src/recovery.rs                           # sweep pending verifications on claim
└── src/dependencies.rs                       # UiPages on VizierDependencies

crates/vizier-http/
├── src/api/v1/agents/channel.rs              # inbound ui_verify_report / ui_page_state; bus responder per socket
├── src/api/v1/agents/ui_pages.rs             # NEW: GET page, GET version
├── src/webui/                                # ACAO * for /ui-kit/*
└── tests/conformance/groups/ui_pages.rs      # NEW vectors

crates/vizier-core/src/service/ui_pages.rs    # NEW: authorization + DTOs

app/src-tauri/                                # asset path: ACAO * for /ui-kit/* (if served there)

webui/ (submodule)
├── app/kit/                                  # NEW: separate Vite entry → build/client/ui-kit/
│   ├── index.ts      # defines <vz-*> custom elements (React roots, @vizier/design)
│   ├── base.css      # tokens + base element styles + layout classes
│   ├── schema/       # zod attribute schemas (moved from lib/generativeUi/catalog)
│   ├── manifest.ts   # elements, attributes, libraries → manifest.json
│   └── libraries.ts  # three → lib/three-<ver>.js, import map
├── app/lib/uiPage/
│   ├── document.ts   # srcdoc: CSP, import map, kit links, bridge, error hooks, heartbeat
│   ├── bridge.ts     # in-frame window.vizier (string source) + host rules
│   └── verify.ts     # off-screen verifier: mount, observe, report
├── app/screens/chat/client/SandboxedPage.tsx # NEW: one host (chat + verifier modes)
├── app/screens/chat/UiPageEntry.tsx          # NEW: draws a UiPage history entry; failed versions collapsed
├── app/api/frames.ts, app/lib/frames.ts      # new frames
├── app/lib/clientTools/builtin.tsx           # remove components/render; keep declare:false renderers
└── app/test/fixtures/hostile-pages/          # isolation suite

design/ (submodule)                           # only if the frame header needs a new variant (after mockup review)

scripts/
├── ui-pages-e2e.mjs                          # fake tab (quickstart §3–5)
└── ui-kit-reference.test.mjs                 # kit manifest ⇔ REFERENCE.md
```

**Structure Decision**:
- Engine logic goes in a new `vizier-core::ui_pages` module, beside `custom_tools` and `artifacts`, with its tool under `agents/tools/`.
- Routes and the socket go in `vizier-http`.
- The browser half goes in `webui`: the host, the verifier and the kit build entry.
- `design/` changes only if the reviewed mockup needs a new component variant.
- Commit order: design → webui → vizier.
- Submodule branches are created with `just checkout 063-html-generative-ui`.

## Complexity Tracking

| Deviation | Why needed | Simpler alternative rejected because |
|---|---|---|
| New optional `VizierTool::guide` method | One generative UI tool (the person's requirement), with an on-demand reference (FR-004) | A second reference tool (`ui_components`) was explicitly not wanted; a reference mode would end the turn (R2) |
| Kit bundles React and the design components inside each frame (~60–80 KB gz, cached) | One source for the look (Principle II); attribute schemas reused | Vanilla re-implementations of every block would drift from `design/` |
