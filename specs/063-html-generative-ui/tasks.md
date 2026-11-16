---
description: "Task list for 063 HTML Generative UI"
---

# Tasks: HTML Generative UI

**Input**: Design documents from `specs/063-html-generative-ui/` (plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md)

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: The plan and the constitution require tests here:
- unit tests for the pure parts (edit application, document builder, bridge rules);
- one fence test per fenced write;
- conformance vectors for the new routes;
- dummyplug end-to-end steps (quickstart.md).

These are included as tasks. A browser check is a manual quickstart step run with the preview tools.

**Organization**: Tasks are grouped by user story. Paths are repo-relative:
- `crates/` is the Rust workspace;
- `webui/` and `design/` are submodules (commit order design → webui → vizier);
- `scripts/` is the workspace scripts folder.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: US1–US7, from spec.md

---

## Phase 1: Setup

**Purpose**: branches, the reviewed mockup, the empty modules.

- [x] T001 Create the `063-html-generative-ui` branch in every submodule with `just checkout 063-html-generative-ui` (constitution: matching submodule branches)
- [x] T002 Draw the HTML mockup `specs/063-html-generative-ui/mockups/page-frame.html` (FigCraft unreachable, so HTML). Show the frame header (title, `v{n}`, "unchecked" mark, error indicator, full screen, download), a failed version collapsed in the trail ("v1 · 2 problems"), the error popover, the "paused · Run anyway" state, light and dark, and 375 px width. Use the existing chat look from `webui/app/screens/chat/client/InteractivePage.tsx`. **Get the person's review before T029 (the first UI code).**
- [x] T003 [P] Create the empty module `crates/vizier-core/src/ui_pages/mod.rs` with `edits.rs`, `limits.rs` and `verify.rs`, and register `mod ui_pages;` in `crates/vizier-core/src/lib.rs`
- [x] T004 [P] Add the kit build entry: create `webui/app/kit/index.ts` (empty custom-element registry) and `webui/app/kit/base.css`. Add a second Vite build (library mode, IIFE, output `build/client/ui-kit/kit-[hash].{js,css}` plus `manifest.json`) to `webui/vite.config.ts` or `webui/vite.kit.config.ts`, and run it from `npm run build`.

---

## Phase 2: Foundational (blocks every story)

**Purpose**: the tool's category and guide hook, storage, the service skeleton, the history entry, the routes, deletion and the sandbox host.

- [x] T005 Add `ToolCategory::Ui` (`"ui"`, description "Interactive pages drawn in the reply") as the **last** entry of `ToolCategory::ALL` in `crates/vizier-core/src/agents/tools/mod.rs`. Extend `a_new_category_is_appended_to_the_end_of_the_list`. Add `ui` to the reserved category names, so an MCP server keyed `ui` becomes `ui_mcp`.
- [x] T006 Add the optional method `fn guide(&self) -> Option<&'static str> { None }` to `VizierTool`, carry it through the `VizierToolDyn` blanket impl, and expose it on `CatalogueEntry` in `crates/vizier-core/src/agents/tools/mod.rs`
- [x] T007 Make `describe_tool_function` add `guide` to the doc it returns when the entry has one, in `crates/vizier-core/src/agents/tools/python/docs_tools.rs`, with a unit test
- [x] T008 [P] Define the `UiPageStorage` trait and its records (`UiPageRecord`, `UiPageVersionRecord`, `VersionMeta`, `Verification`, `Problem`, `StaleBase { latest }`) in `crates/vizier-core/src/storage/ui_page.rs`, per data-model.md. Add it to the `VizierStorageProvider` supertrait in `crates/vizier-core/src/storage/mod.rs`.
- [x] T009 Implement `UiPageStorage` on `SqlStorage` in `crates/vizier-core/src/storage/sqlite/ui_page.rs`:
  - `init_ui_page_schema` creates both tables and their indexes (data-model.md).
  - Every write calls `check_fence` on `agent:{agent_id}` in its own transaction.
  - `add_ui_page_version` is one guarded `UPDATE … WHERE latest_version = ?` plus an insert.
  - `time::stored` for the text times.
  - Register the schema init beside the other `init_*_schema` calls.
- [x] T010 [P] Add the PostgreSQL migration `crates/vizier-core/src/storage/sql/postgres/migrations/0011_ui_pages.sql` with the same tables and indexes
- [x] T011 Add one fence test per fenced write (`create_ui_page`, `add_ui_page_version`, `set_ui_page_verification`, `set_ui_page_history_uid`, `save_ui_page_state`, `sweep_pending_ui_verifications`) to `crates/vizier-core/src/storage/sqlite/fence_tests.rs`. Add a test that two `add_ui_page_version` calls with the same base yield one success and one `StaleBase`.
- [x] T012 Delete a conversation's and an agent's pages inside `delete_session` and `delete_agent`, returning `(agent, page)` pairs. After commit, remove the blobs under `ui-pages/{agent}/{page}/` in `crates/vizier-core/src/storage/sqlite/session.rs` and `crates/vizier-core/src/storage/sqlite/agent.rs` (or the files that hold those methods), through the service in T014.
- [x] T013 [P] Add `SessionHistoryContent::UiPage { page_id, version, title, verification }` (`verification` is filled on read, never stored) in `crates/vizier-core/src/schema/history.rs`. Skip it in `history_entries_to_messages`, add its name to the kind list, and add `fill_ui_pages` to the history read path beside `fill_client_calls`.
- [x] T014 Implement the `UiPages` service in `crates/vizier-core/src/ui_pages/mod.rs`:
  - `create`, `replace` and `get_version_html`, which write the blob under its final key (`ui-pages/{agent}/{page}/{sha256}.html`, content-named) before the row and check `MAX_VERSION_BYTES`;
  - `delete_blobs`;
  - id generation (`pg_` + 22 base62 characters).
  Put it on `VizierDependencies` in `crates/vizier-core/src/dependencies.rs`.
- [x] T015 [P] Put the limits (`MAX_VERSION_BYTES = 1_048_576`, `MAX_EDITS = 100`, `MAX_TITLE = 120`, `VERIFY_OBSERVE_MS = 2000`, `VERIFY_DEADLINE = 10s`) in `crates/vizier-core/src/ui_pages/limits.rs`
- [x] T016 [P] Add `SessionFrame::UiPageVerified { page_id, version, verification }` and `SessionFrame::UiPageState { page_id, state, state_seq }` in `crates/vizier-core/src/transport/events.rs`, and their socket encodings (`ui_page_verified`, `ui_page_state`) in `crates/vizier-http/src/topic.rs`
- [x] T017 Add `service::ui_pages` with `get_page` and `get_version`, authorized like reading the conversation's history (`404` outside it), in `crates/vizier-core/src/service/ui_pages.rs`
- [x] T018 Add the routes `GET …/topic/{topic}/pages/{page_id}` and `GET …/pages/{page_id}/versions/{version}` (`text/html`, immutable cache) in `crates/vizier-http/src/api/v1/agents/ui_pages.rs`. Mount them in `agents/mod.rs` and add their OpenAPI entries (contracts/http.md).
- [x] T019 [P] Add conformance vectors for both routes (owner, another user, no access, unknown page) in `crates/vizier-http/tests/conformance/groups/ui_pages.rs`, and remove the routes from `tests/conformance/pending.txt` if they were listed
- [x] T020 Serve `/ui-kit/*` with `Access-Control-Allow-Origin: *` and immutable caching in `crates/vizier-http/src/webui/` (the embedded asset handler). Do the same in the desktop app's asset path in `app/src-tauri/` if the WebUI's static files are served there.
- [x] T021 Build the page document in `webui/app/lib/uiPage/document.ts`:
  - `srcdoc(html, {kitBase, state, theme, libraries})` emits the CSP of research R8 with `{KIT}` computed from `location`, the import map, the kit `<link>`/`<script>`, the in-frame bridge source and the error hooks with a 250 ms heartbeat;
  - `standaloneDocument(...)` inlines the kit for download.
  Add `document.test.ts` (CSP string, import map, no `allow-same-origin`, http media refused by policy).
- [x] T022 Implement the host `webui/app/screens/chat/client/SandboxedPage.tsx` (`mode: "chat" | "verify"`), built on `InteractivePage.tsx`:
  - `sandbox="allow-scripts"`;
  - messages accepted only from its own `contentWindow`;
  - the problem and heartbeat listeners;
  - auto height up to 80% of the viewport;
  - full screen through `useChatAreaFullscreen`.
- [x] T023 [P] Add the new frame types (`ui_page_verified`, `ui_page_state`, `ui_verify`, inbound `ui_verify_report`, inbound `ui_page_state`) to `webui/app/api/frames.ts`, and handle the outbound ones in `webui/app/lib/frames.ts`, with tests
- [x] T024 [P] Add the API client calls `getUiPage` and `getUiPageVersion` (through the transport seam only) in `webui/app/api/endpoints/uiPages.ts`

**Checkpoint**: storage, routes and the host exist. Stories can start.

> **Changed by research R7a (2026-11-16):**
> - T021's document is built **server-side** in `crates/vizier-http/src/ui_frame.rs` and served at `/api/v1/ui-frames/{ticket}`. A ticket comes from `…/versions/{n}/frame`.
> - The in-frame runtime is `webui/app/kit/runtime.ts`, built by the kit build as `runtime-*.js`.
> - `webui/app/lib/uiPage/document.ts` is not needed.
> - `webui/app/lib/uiPage/host.ts` holds the host's pure rules.
> - T062's bridge is in the runtime already; T062 keeps its tests and the element side.

---

## Phase 3: User Story 1 — The agent builds anything a web page can be (P1) 🎯 MVP

**Goal**: `ui_render {title, html}` creates a page, draws it in the reply, isolated, and ends the turn.

**Independent test**:
- quickstart §1: no tab, the version is saved, `verified: false, reason: "no_tab"`, the turn ends.
- quickstart §7.1 and §7.2: a real browser draws a chess page, and the hostile pages are all blocked.

- [x] T025 [US1] Implement the `ui_render` tool in `crates/vizier-core/src/agents/tools/ui_pages.rs`:
  - the input as a serde enum of the three shapes (contracts/ui-render-tool.md, with `deny_unknown_fields`);
  - the fixed description;
  - `ends_turn() → true`, `category() → Ui`;
  - `guide()` → `include_str!("../../../assets/ui-kit/REFERENCE.md")`;
  - `available(ctx)` per research R9 (`HTTP`/`Project` channel and `!ctx.detached`).
  For US1, the new-page shape calls `UiPages::create`, records the entry with `agents::steps::record_step` and returns `{status: "rendered", page, version: 1, verified: false, reason: "no_tab"}`. Verification comes in US3.
- [x] T026 [US1] Register `ui_render` on `default_toolset` in `BaseTools::new()`, add it to the `every_tool_is_in_its_category` table, and keep it out of `DREAM_TOOL_NAMES`, in `crates/vizier-core/src/agents/tools/mod.rs`
- [x] T027 [US1] Map refusals to `Err(VizierError(json))` with the `code`s of contracts/ui-render-tool.md (`invalid_input`, `too_large`) in `crates/vizier-core/src/agents/tools/ui_pages.rs`. Add unit tests for each shape's validation.
- [x] T028 [P] [US1] Write the first version of the guide `crates/vizier-core/assets/ui-kit/REFERENCE.md`:
  - the page model;
  - the rules (no script network; `https` media; `requestAnimationFrame` loops; `vizier.worker` for work over about 50 ms; limits);
  - bridge v1;
  - the canvas game-loop example.
  US4 adds the kit section.
- [x] T029 [US1] Draw a `UiPage` history entry in the reply with `SandboxedPage`, fetching the version's HTML with `getUiPageVersion`, in `webui/app/screens/chat/UiPageEntry.tsx`. Wire it into the turn grouping in `webui/app/lib/trail.ts` (and `MessageList.tsx`): the last `UiPage` of a turn is the reply, and earlier ones are collapsed trail steps. Follow the T002 mockup.
- [x] T030 [US1] Add the frame header (title, `v{n}`, "unchecked" when `verified: false`, full screen, Download through `standaloneDocument`) in `webui/app/screens/chat/UiPageEntry.tsx`. If the mockup needs a new component variant, add it in `design/` first, with a story.
- [x] T031 [P] [US1] Write the hostile-page fixtures, one file per probe (`localStorage`, `cookie`, `parent.document`, `top.location`, `fetch`, `WebSocket`, `<script src=https://…>`, `window.open`), each reporting its result through `console.error`, in `webui/app/test/fixtures/hostile-pages/`
- [x] T032 [US1] Add dummyplug e2e quickstart §1 (create with no tab; GET page; history entry) as `ui_pages_create` in `crates/vizier-core/tests/dummyplug_e2e.rs`
- [ ] T033 [US1] Browser check, quickstart §7.1–7.3 (game, isolation suite, `https`/`http` media), with the preview tools. Record the result in the PR description.

**Checkpoint**: an agent can draw any HTML page in a WebUI chat (MVP).

> **Notes (2026-11-16):**
> - T031 is one combined fixture, `escape-probe.html`. It reports each probe as a page problem; all were blocked in the WebUI, and the https image loaded.
> - T033 is done except the game with a live provider. The isolation suite and http/https media were checked through `crates/vizier-http/tests/dev_serve.rs`, an ignored dev server, because the `vizier` binary does not build on Windows.
> - T032 and T038 are the same dummyplug test, `ui_pages_are_drawn_versioned_and_refused_through_the_loop`.
> - T035's replace and edit logic lives in the tool (`UiRender::add`) over `UiPages::add_version`.
> - T039 and T040 landed early: the request side of the check. With no responder yet (T041), every version is honestly `unverified(no_tab)`.

---

## Phase 4: User Story 2 — Changing a page is fast (P1)

**Goal**: full replacement and edit sets create the next version, all or nothing, refused when stale.

**Independent test**: quickstart §2.

- [x] T034 [P] [US2] Implement the pure `apply_edits(base: &str, edits: &[Edit]) -> Result<String, EditError { index, kind: NotFound | Ambiguous(count) | EmptyFind }>` (in order, exactly once at the point of application) in `crates/vizier-core/src/ui_pages/edits.rs`, with unit tests: a match after a previous edit, overlapping text, multi-byte characters, all or nothing.
- [x] T035 [US2] Add `UiPages::replace` and `UiPages::edit` (load the latest version's HTML, check `base_version`, apply, size check, `add_ui_page_version`, map `StaleBase` → `stale_base { latest }`) in `crates/vizier-core/src/ui_pages/mod.rs`
- [x] T036 [US2] Handle the full-replacement and edit-set shapes in `ui_render`, including `unknown_page`, `other_conversation` (the page's `(agent, channel, topic)` differs from `ctx.session`), `stale_base`, `edit_not_found` and `edit_ambiguous`, in `crates/vizier-core/src/agents/tools/ui_pages.rs`
- [x] T037 [US2] Make each reply keep drawing its own version: `UiPageEntry` fetches the entry's `version`, not the latest, in `webui/app/screens/chat/UiPageEntry.tsx`
- [x] T038 [US2] Add dummyplug e2e quickstart §2 (every row) as `ui_pages_edits` in `crates/vizier-core/tests/dummyplug_e2e.rs`

**Checkpoint**: an agent can make a small change to a page without rewriting it.

---

## Phase 5: User Story 3 — A broken page is repaired before the person sees it (P2)

**Goal**: with a tab open, a version is checked, problems come back as an error, and the turn continues.

**Independent test**: quickstart §3 through `scripts/ui-pages-e2e.mjs`.

- [x] T039 [US3] Implement `ui_pages::verify::verify(bus, session, page, version) -> Verification`: a bus `request` on `ui.verify.{session_key}` with `{request_id, page_id, version, observe_ms}`. Map `NoResponders`→`unverified(no_tab)`, `Timeout`→`unverified(timeout)`, a `closed` reply→`unverified(closed)`, a report→`clean`/`problems` (cut to 20 problems and 500 characters). Write the result with `set_ui_page_verification` and publish `UiPageVerified`. In `crates/vizier-core/src/ui_pages/verify.rs`.
- [x] T040 [US3] Call `verify` after every saved version in `ui_render`. On `problems`, return `Err` with `code: "problems"`, `page`, `version` and `problems`. Otherwise return `verified` and `reason`. In `crates/vizier-core/src/agents/tools/ui_pages.rs`.
- [ ] T041 [US3] Make each topic socket on an `HTTP`/`Project` session subscribe as responder to `ui.verify.{session_key}` while open. On a request, send that socket `ui_verify`, await its `ui_verify_report` (matched by `request_id`, ignored otherwise) and reply on the bus; reply `{closed: true}` if the socket closes first. In `crates/vizier-http/src/api/v1/agents/channel.rs` (and the desktop `TopicConnection`, `crates/vizier-http/src/topic.rs`).
- [ ] T042 [US3] Sweep `pending` verifications of the claimed agent to `unverified(closed)` in `recovery::recover_agent` (rows under an older fence only), in `crates/vizier-core/src/recovery.rs`
- [ ] T043 [US3] Implement the off-screen verifier: on `ui_verify`, fetch the version, mount `SandboxedPage mode="verify"` (1024×768, hidden, outside the viewport) with the page's state, collect problems for `observe_ms`, report `unresponsive` when no heartbeat arrives for 1 s, send `ui_verify_report` and unmount. In `webui/app/lib/uiPage/verify.ts` and `webui/app/screens/chat/client/Verifier.tsx`, mounted once by the chat screen.
- [ ] T044 [US3] Add the error indicator: problems after mount in chat mode light the header indicator, and a popover lists them (design `Floating` + `useDismiss`, per the dismissal convention). Show "v{n} · N problems" on collapsed failed versions. In `webui/app/screens/chat/UiPageEntry.tsx`.
- [ ] T045 [US3] Add the crash-loop marker: set `vz.mounting = {page, version}` in `localStorage` (inside try/catch) before mounting, and clear it after three heartbeats. If a load finds the marker set, draw that version "paused · Run anyway". In `webui/app/screens/chat/client/SandboxedPage.tsx`.
- [ ] T046 [US3] Tell the agent about a paused page: the WebUI sends the paused `{page, version}` with the next request's context, and the context section of `crates/vizier-core/src/agents/agent/system_prompt/context.rs` gets one line, *"Page pg_… v3 froze the person's tab and is paused; move heavy work into `vizier.worker`."*, per FR-016a.
- [ ] T047 [P] [US3] Write the fake-tab script `scripts/ui-pages-e2e.mjs` (`--no-tab`, a scripted `ui_verify_report`: clean, problems, silence, two tabs) that asserts quickstart §3's results, frames and saved versions
- [ ] T048 [US3] Browser check, quickstart §7.6 (error indicator) and §7.8 (hang on Chromium and WebKit, paused after reload), with the preview tools

**Checkpoint**: a broken first draft is fixed in the same turn while a tab is open.

---

## Phase 6: User Story 4 — Plain pages look like Vizier; ready-made pieces (P2)

**Goal**: theme tokens and base CSS on every page; `<vz-*>` elements; the bundled `three`; the reference kept in sync.

**Independent test**: quickstart §7.4 and §7.5.

- [ ] T049 [P] [US4] Write the kit CSS in `webui/app/kit/base.css`:
  - every `--vz-*` color token with light and dark values;
  - typography and spacing;
  - base styles for `body`, headings, links, `button`, `input`, `select`, `textarea`, `table`, `code`, scrollbars;
  - the layout classes `vz-stack`, `vz-row`, `vz-grid`.
  Derive the values from `design/` tokens; do not copy them by hand.
- [ ] T050 [US4] Apply the theme live: the host posts `{type: "vz:theme", tokens, dark}` on load and on every `useAppearanceKey` change (`SandboxedPage.tsx`). The kit sets the tokens on `:root` and fires `vizier:theme` (`webui/app/kit/theme.ts`).
- [ ] T051 [US4] Move the zod attribute schemas from `webui/app/lib/generativeUi/catalog/` to `webui/app/kit/schema/` for `card`, `stat`, `table`, `chart`, `tabs`, `field` (text, number, select, checkbox, slider), `badge`, `callout`, `progress` and `image`. Leave re-exports for the read-only renderer.
- [ ] T052 [US4] Implement the custom elements in `webui/app/kit/elements/*.tsx`, registered in `webui/app/kit/index.ts`. Each is a React 19 root rendering the matching `@vizier/design` component, parses its attributes with its schema, draws an inline problem box and posts a `kit` problem on invalid input, and fires DOM events for input and change. `<vz-image file>` uses `vizier.files.url`.
- [ ] T053 [P] [US4] Write the element manifest (names, attributes with types and enums, events, an example each, libraries) in `webui/app/kit/manifest.ts`, emitted as `ui-kit/manifest.json` by the kit build
- [ ] T054 [P] [US4] Add the bundled libraries as data: `three` → `ui-kit/lib/three-<version>.js` (ES module), plus the import map entry used by `document.ts`, in `webui/app/kit/libraries.ts`
- [ ] T055 [US4] Load the kit lazily: always link the CSS; include `kit-*.js` only when the HTML contains a `<vz-` tag or the page asks for it. In `webui/app/lib/uiPage/document.ts`.
- [ ] T056 [US4] Complete the guide: kit CSS, every element with attributes, events and an example, the libraries and import names, and the dashboard and form examples, in `crates/vizier-core/assets/ui-kit/REFERENCE.md`
- [ ] T057 [P] [US4] Add the sync check `scripts/ui-kit-reference.test.mjs` (`node --test`, picked up by `just test-scripts`). It fails on an element, attribute or library that is in `webui/build/client/ui-kit/manifest.json` (or `webui/app/kit/manifest.ts`) but not in `REFERENCE.md`, or the other way round.
- [ ] T058 [P] [US4] Add vitest tests for attribute parsing and problem reporting per element, in `webui/app/kit/elements/elements.test.tsx` (use `app/test/oneReact.ts`)
- [ ] T059 [US4] Browser check, quickstart §7.4–7.5 (theme in both schemes and switching live; stat and chart; an invalid attribute), with the preview tools

**Checkpoint**: unstyled pages look like Vizier, and dashboards take a few tags.

---

## Phase 7: User Story 5 — Pages remember and talk back (P2)

**Goal**: page state across versions and tabs; `vizier.send`; `vizier.files`; `vizier.worker`.

**Independent test**: quickstart §4, §5, §7.7 and §8a.

- [ ] T060 [US5] Handle the inbound socket frame `ui_page_state {page_id, state, expected_seq?}` with the `client_call_state` checks (`MAX_STATE_BYTES`, a `RateLimiter` on `HTTPState` at 2/s per page, the page in this session) → `save_ui_page_state` → publish `UiPageState`, in `crates/vizier-http/src/api/v1/agents/channel.rs`
- [ ] T061 [US5] Add `page: Option<PageSource { page_id, version }>` to `VizierRequestContent::ClientEvent` (exclusive with the source call) in `crates/vizier-core/src/schema/request.rs`. Accept it on the inbound `client_event` when the page belongs to the session, and add `page: <id> v<n>` to the frontmatter in `generate_frontmatter`.
- [ ] T062 [US5] Write the in-frame bridge v1 in `webui/app/lib/uiPage/bridge.ts`, per contracts/bridge.md:
  - `version`, `abilities`, `has`;
  - `state.get`/`set`/`subscribe`;
  - `send`;
  - `files.url`;
  - `worker(fn | "#id")` with `run(input, {timeoutMs})` (default 10 s, maximum 60 s, terminate and reject `timeout`, one run at a time) and `terminate`;
  - `<script type="vz-worker">` support.
- [ ] T063 [US5] Enforce the host rules in `SandboxedPage.tsx`:
  - `send` only within 10 s of a trusted interaction and at most 1/s;
  - state coalesced to 500 ms and sent as `ui_page_state`;
  - incoming `ui_page_state` frames forwarded to the frame;
  - `files.url` resolved through the transport as a `data:` URL;
  - a new version starts from the page's state.
- [ ] T064 [P] [US5] Add bridge and host-rule tests (no interaction → `no_interaction`; rate → `rate_limited`; coalescing; a worker timeout terminates; state handed to a new version) in `webui/app/lib/uiPage/bridge.test.ts`
- [ ] T065 [US5] Extend `scripts/ui-pages-e2e.mjs` with quickstart §4 (state, rate limit, another conversation's page refused) and §5 (`client_event` with `page` starts a turn, and the `context` frontmatter shows `page: P v3`)
- [ ] T066 [US5] Browser check, quickstart §7.7 (send rules) and §8a (worker keeps the page animating, on Chromium and WebKit), with the preview tools

**Checkpoint**: a chess game survives reloads and plays against the agent.

---

## Phase 8: User Story 6 — Offered only where a page can be shown (P3)

**Goal**: present in `HTTP`/`Project` conversations with or without a tab, absent everywhere else.

**Independent test**: quickstart §0.

- [ ] T067 [P] [US6] Add unit tests for `UiRender::available` across every `VizierChannelId` kind and `detached: true`/`false`, in `crates/vizier-core/src/agents/tools/ui_pages.rs`
- [ ] T068 [US6] Add dummyplug e2e quickstart §0 (the `context` category list in a WebUI chat with no tab; a task run without `ui`; `describe_tool_function` has `guide`) as `ui_pages_availability` in `crates/vizier-core/tests/dummyplug_e2e.rs`

---

## Phase 9: User Story 7 — Existing pages keep working (P3)

**Goal**: the old tools are retired, and their saved pages still draw.

**Independent test**: quickstart §7.10 and §0's last row.

- [X] T069 [US7] Remove the `components` and `render` client tool registrations and the `ui` client category in `webui/app/lib/clientTools/builtin.tsx`. Re-register `render` (json-render) with `declare: false` and keep `render_interactive_ui` with `declare: false`, so saved calls draw through `GenerativeUi.tsx` and `InteractivePage.tsx`.
- [X] T070 [US7] Add `ui_components` and `render_interactive_ui` to `RETIRED_TOOL_NAMES`, with guidance naming `ui_render` and its guide, in `crates/vizier-core/src/agents/tools/mod.rs`
- [X] T071 [US7] Drop client declarations of category `ui` (`components`, `render`) from stale tabs with a `warn!` instead of renaming them, in `crates/vizier-core/src/agents/client/declare.rs`, with a unit test
- [X] T072 [US7] Trim `webui/app/lib/generativeUi/` to what the read-only renderer needs: drop `reference.ts`, `recipes/` and validation-only code once nothing imports them, and leave a header comment pointing at #82
- [ ] T073 [US7] Browser check, quickstart §7.10 (a workspace with saved pages of both old tools draws them, with state)

---

## Phase 10: Polish & cross-cutting

- [ ] T074 [P] Add the #82 checklist entries (the two old WebUI renderers, the read-only `lib/generativeUi/`, the two retired names, dropping stale client `ui` declarations) to GitHub issue #82 (`gh issue edit` or a comment). This needs the person's go-ahead, because it posts to GitHub.
- [ ] T075 [P] Update the docs: rewrite `docs/src/api-integration/generative-ui.md` for HTML pages (kit, bridge, worker, limits, security); update `docs/src/api-integration/client-tools.md` (no built-in `ui` client tools)
- [ ] T076 [P] Update CLAUDE.md's **Generative UI** paragraph (and the tool categories list: `ui` is the 23rd) to describe 063
- [ ] T077 Run `cargo clippy --workspace`, `cargo test --workspace`, `cd webui && npm run typecheck && npm test`, and `just test-scripts`
- [ ] T078 Run the full quickstart (§0–§7) against `just run` with a dummyplug agent, plus §7 in the preview browser at desktop and phone width (§7.9) and the download check (§7.11)
- [ ] T079 Commit in order design → webui → vizier with `just commit`, as conventional commits

---

## Dependencies & Execution Order

### Phases

- **Setup (1)**: first. T002's mockup review gates every UI task (T029 onwards).
- **Foundational (2)**: after Setup. Blocks every story.
- **US1 (3)**: after Foundational. The MVP.
- **US2 (4)**: after US1 (it shares the tool and the entry).
- **US3 (5)**: after US1. Independent of US2, except that quickstart §3's fix step uses an edit.
- **US4 (6)**: after Foundational (kit build, T004, and host, T022). It can run beside US2 and US3.
- **US5 (7)**: after US1 (host); the worker part needs only `document.ts`.
- **US6 (8)**: after US1 (T025 holds `available`).
- **US7 (9)**: any time after US1. It must land in the same release as US1, so agents never see both tools.
- **Polish (10)**: last.

### Inside the stories

- Storage → service → tool → socket → WebUI → e2e → browser check.
- A WebUI task after its mockup; a `design/` variant before the WebUI use.

## Parallel opportunities

- Setup: T003 and T004.
- Foundational: T008, T010, T013, T015 and T016 in parallel; T019, T023 and T024 once T018 and T022 are underway.
- US1: T028 and T031 beside T025–T027.
- US4: T049, T053, T054, T057 and T058 in parallel.
- Across stories, after US1: US2 (core), US4 (webui kit) and US3's socket work (vizier-http) touch different files.

### Example: US4 in parallel

```text
T049 kit CSS            (webui/app/kit/base.css)
T053 manifest           (webui/app/kit/manifest.ts)
T054 libraries          (webui/app/kit/libraries.ts)
T057 reference sync     (scripts/ui-kit-reference.test.mjs)
T058 element tests      (webui/app/kit/elements/elements.test.tsx)
```

## Implementation Strategy

1. **MVP = Phases 1–3 + US7's T069–T071.** An agent draws any HTML page, isolated, and the old tools are gone, so there is never a second UI tool. Ship it behind nothing: it is the generative UI.
2. **+US2** (edits) for fast iteration, the person's main concern. Treat it as part of the first release if possible.
3. **+US3** (self-repair) and **+US4** (kit look), then **+US5** (state, talk back, worker).
4. **US6** is mostly tests on what T025 already does.

Each step ends with its quickstart section passing on a dummyplug agent.

## Notes

- Every refusal from `ui_render` is an `Err` carrying a JSON message (research R3). This is deliberate, so the turn continues.
- No new Rust crate or npm package (plan.md).
- Browser checks use the built-in preview tools. Never ask the person to check by hand.
