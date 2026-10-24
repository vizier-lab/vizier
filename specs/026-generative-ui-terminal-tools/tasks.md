# Tasks: Generative UI and Turn-Ending Tools

**Input**: Design documents from `/specs/026-generative-ui-terminal-tools/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/ (ends-turn-flag, generative-ui-tools, ui-catalogue, logic-sandbox), quickstart.md

**Tests**: Unit tests beside the code (`cargo test`, vitest for the catalogue, recipes, validators and runner). **End-to-end verification is out of scope for this task list and is done by the maintainer**: the Playwright scenarios, the dummyplug runs of quickstart.md and the live-model checks. This is a deliberate exception to the constitution's dummyplug e2e gate, requested by the maintainer; `scripts/client-tools-e2e.mjs` and `webui/e2e/` are not touched here.

**Organization**: Grouped by user story. Stories are ordered by priority (US1 to US3 are P1, US4, US5, US7 are P2, US6 is P3). The Rust `ends_turn` work (US3, US4) and the WebUI generative UI work (US1, US2, US5, US7) touch different code and can proceed in parallel.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: US1 to US7, from spec.md
- Paths are relative to the repository root. `webui/` and `design/` are git submodules: commit **design → webui → vizier pin bump**, in that order (CLAUDE.md).
- `webui/app/lib/generativeUi/catalog/` and `registry/` are directories (one file per group) so groups can be built in parallel; `catalog/index.ts` and `registry/index.ts` combine them.

---

## Phase 1: Setup

- [X] T001 Add pinned dependencies to `webui/package.json`: `@json-render/core@0.21.3`, `@json-render/react@0.21.3` (exact versions, no caret) and `zod@^4`; run `npm install` in `webui/`; confirm `npm run typecheck` and `npm run build` still pass (research R3)
- [X] T002 Record the baseline: run `cargo test`, `cargo clippy`, `cd webui && npm run typecheck && npm test` and note any failures that exist before this feature, so later regressions are attributable

---

## Phase 2: Foundational (blocks US1, US2, US5, US7)

**Purpose**: the design-library components the catalogue needs and the generative UI scaffolding every story fills in.

- [X] T003 [P] Add `Stack` (gap, align) to `design/src/components/Stack/` with a story and tokens-only styling (`Stack.tsx`, `Stack.stories.tsx`, `index.ts`)
- [X] T004 [P] Add `Row` (gap, align, justify, wrap) to `design/src/components/Row/` with a story
- [X] T005 [P] Add `Grid` (cols 1–12, gap, minCol responsive) to `design/src/components/Grid/` with a story
- [X] T006 [P] Add `Slider` (min, max, step, label, value, onChange, disabled; keyboard and ARIA complete) to `design/src/components/Slider/` with a story
- [X] T007 [P] Add `Chart` (kind `bar`/`line`/`area`/`pie`, data `[{label,value,series?}]`, legend, height, `onPress`) to `design/src/components/Chart/`, reusing `BarList`/series tokens, with a story per kind and an accessible text alternative
- [X] T008 Export the new components from `design/src/index.ts`, run the design Storybook build and token lint, commit in `design/`, then bump the `design` pin in `webui/` (`npm install ../design`) (depends on the five tasks above)
- [X] T009 Create the catalogue core in `webui/app/lib/generativeUi/catalog/index.ts`: a `defineComponent` helper taking `{ group, description, props (zod), children rule, events, example }`, the json-render `defineSchema`/`defineCatalog` wiring with built-in actions, group metadata, and a test `catalog.test.ts` that fails when an entry has no `description` or no `example` that validates against its own schema (contracts/ui-catalogue.md §6)
- [X] T010 [P] Create `webui/app/lib/generativeUi/functions.ts` with the pure `$computed` set (`add sub mul div mod min max abs round floor eq not and or len at concat join slice includes range`) and `functions.test.ts` (including bad-argument behaviour: returns `null`, never throws)
- [X] T011 [P] Create `webui/app/lib/generativeUi/state.ts`: a json-render `StateStore` wrapper that seeds from `spec.state` overlaid with saved state, excludes top-level `_`-prefixed keys from the saved snapshot, coalesces saves to one per 500 ms, and merges state arriving from another tab without firing `watch`; `state.test.ts` (data-model §3)
- [X] T012 Create `webui/app/lib/generativeUi/registry/index.ts` combining per-group registry maps through json-render `defineRegistry`, plus `registry.test.ts` asserting every catalogue entry has a registered component (fails today by design; each story adds its group)

**Checkpoint**: the scaffolding exists; user stories can start.

---

## Phase 3: User Story 1 - Agent answers with a generated interface, quickly (P1) 🎯 MVP

**Goal**: an agent calls `ui_render` with a catalogue-only description and the WebUI draws it in the reply, themed, persisted, validated with helpful errors; the agent finds the catalogue through `ui_components` with no skill.

**Independent Test** (verified by the maintainer): quickstart §2 steps 1–7 and 9 on a dummyplug agent: `tools` shows the `ui` category, `ui_components` returns the index and a recipe, `ui_render` of the dashboard recipe draws and survives a reload and a second tab, an invalid spec lists every problem and the turn continues, a theme switch re-themes it.

- [X] T013 [P] [US1] Add layout entries (`Stack`, `Row`, `Grid`, `Card`, `Tabs`, `Fold`, `Divider`) to `webui/app/lib/generativeUi/catalog/layout.ts` per contracts/ui-catalogue.md §1, each with a description and example
- [X] T014 [P] [US1] Add text entries (`Heading`, `Text`, `Badge`, `Callout`, `Code`) to `webui/app/lib/generativeUi/catalog/text.ts`
- [X] T015 [P] [US1] Add data entries (`Stat`, `Table`, `KeyValue`, `Progress`, `Chart`) to `webui/app/lib/generativeUi/catalog/data.ts`
- [X] T016 [P] [US1] Implement the layout components in `webui/app/lib/generativeUi/registry/layout.tsx` on `@vizier/design` (`Tabs` over `FilterTabs`/`Tab`, `Fold`, `Card`, the new `Stack`/`Row`/`Grid`)
- [X] T017 [P] [US1] Implement the text components in `webui/app/lib/generativeUi/registry/text.tsx`; `Text` renders the markdown subset through the existing sanitised `react-markdown` pipeline, rendering links as plain text until US7
- [X] T018 [P] [US1] Implement the data components in `webui/app/lib/generativeUi/registry/data.tsx` (`Table` on design `Table`/`TableRow` with `maxRows`, `Stat` on `StatGroup`, `Chart`, `Progress` on `ProgressBar`)
- [X] T019 [US1] Implement `webui/app/lib/generativeUi/validate.ts`: zod per-component props, json-render `validateSpec`/`autoFixSpec` (lossless fixes applied and reported, lossy ones are errors), the size/element/depth/logic limits of contracts/ui-catalogue.md §4, nearest-name suggestions, and the model-facing error text of contracts/generative-ui-tools.md §5 (all issues, JSON paths, valid options, capped at 20)
- [X] T020 [P] [US1] Write `webui/app/lib/generativeUi/validate.test.ts`: unknown component, wrong prop type, enum miss with suggestions, dangling child, misplaced `visible`, oversize, over 20 issues, and that no error echoes the whole spec
- [X] T021 [US1] Implement `webui/app/lib/generativeUi/reference.ts`: generate the `ui_components` index, per-component detail (compact signature from the zod schema, children rule, events, example) and recipe output from the catalogue (contracts/generative-ui-tools.md §2), capped at 24 KB; `reference.test.ts` checks every component appears and output size
- [X] T022 [P] [US1] Add the `dashboard` recipe in `webui/app/lib/generativeUi/recipes/dashboard.ts` and `recipes/index.ts` (name, intent, notes, spec), plus `recipes.test.ts` validating **every** recipe in the index against the catalogue (fails if any recipe is invalid)
- [X] T023 [US1] Create `webui/app/screens/chat/client/GenerativeUi.tsx`: json-render `Renderer` + `StateProvider` + `ActionProvider` for one saved call; a per-component error boundary showing an inline chip; accessible region named by the call's `title`; light/dark follows the app; keeps the 019 expand and download controls (download exports the spec as `.json`)
- [X] T024 [US1] Create `webui/app/lib/clientTools/generativeUi.ts`: the `ui` category description (≤512 bytes, "Start with ui_components"), the `ui_components` and `ui_render` descriptions per contracts/generative-ui-tools.md §2–§3, and the `ui_render` input schema with the `type` enum generated from the catalogue; `generativeUi.test.ts` asserts `ui_render`'s description is ≤ 2048 bytes (leaving room for the ends-turn notice) and the schema ≤ 16 KB
- [X] T025 [US1] Register the tools in `webui/app/lib/clientTools/builtin.tsx`: `registerClientCategory({name:'ui'})`, `ui_components` (read-only, handler returns the reference output), and `ui_render` (not deferrable; handler validates, enforces the 256 KB cap, returns `{ui_id, status:'rendered', fixed?}` or throws the model-facing error; `render` uses `GenerativeUi`, `renderAfterTurn`, `latestPerTurn`)
- [X] T026 [US1] Make a reply with empty text still draw its bubble when it has a visible client call: find the history-grouping code (`groupHistory` in `webui/app/lib/`, used by `trail.ts`/`frames.ts`) and the reply bubble in `webui/app/screens/chat/`, fix, and add a case to `webui/app/lib/trail.test.ts`

**Checkpoint**: US1 works end to end except the turn does not yet end by itself (US3).

---

## Phase 4: User Story 2 - Generated interfaces are interactive (P1)

**Goal**: inputs, tabs and toggles update locally; a button can send a message (with data) as the person's turn; state is saved and synced.

**Independent Test** (verified by the maintainer): quickstart §2 steps 4, 5 and 8: the Add counter increments with no turn, state survives reload and a second tab, `sendMessage` from the `form` recipe posts the person's message with data, a second press within one second is ignored.

- [X] T027 [P] [US2] Add input entries (`TextInput`, `NumberInput`, `Slider`, `Select`, `Checkbox`, `Switch`, `RadioGroup`; shared `label`, `value` bind, `disabled`, `checks`) to `webui/app/lib/generativeUi/catalog/input.ts`
- [X] T028 [P] [US2] Add the `Button` entry (variant, tone, disabled, selected; event `press`) to `webui/app/lib/generativeUi/catalog/action.ts`
- [X] T029 [P] [US2] Implement the input components in `webui/app/lib/generativeUi/registry/input.tsx` on design `TextField`, `Select`, `Checkbox`, `Switch`, `Radio`, `Slider`, using json-render bound props and `checks` validation display
- [X] T030 [P] [US2] Implement `Button` in `webui/app/lib/generativeUi/registry/action.tsx` (emits `press`, carries the originating event's `isTrusted`)
- [X] T031 [US2] Implement `webui/app/lib/generativeUi/actions.ts`: wire json-render built-ins (`setState`, `pushState`, `removeState`, `validateForm`) and `sendMessage {text,data}` through `ClientCallContext.sendEvent`, allowed only when the action chain began at a trusted DOM event, ≤ 1 per second per interface, text ≤ 4000 chars, data ≤ 64 KB (research R9, data-model §4); `actions.test.ts` covers trusted vs untrusted, rate limit, and size refusals
- [X] T032 [US2] Connect `GenerativeUi.tsx` to `state.ts`: load the call's saved state, save non-`_` changes through `ctx.saveState`, merge `client_call_state` from other tabs without firing watchers, and show the existing short "sent" notice for `sendMessage` (spec 019 FR-028a)
- [X] T033 [P] [US2] Add recipes `form` (inputs with `checks`, `validateForm`, `sendMessage` with the values) and `tabs_and_filter` (`Tabs` + `visible`, a `Select` driving a `repeat`) in `webui/app/lib/generativeUi/recipes/form.ts` and `tabs_and_filter.ts`; register in `recipes/index.ts`

**Checkpoint**: US1 and US2 give a complete, interactive catalogue-only generative UI.

---

## Phase 5: User Story 3 - A turn-ending tool ends the turn as the final answer (P1)

**Goal**: a successful call to a flagged tool ends the turn with no further model round; failures and deferrals do not.

**Independent Test** (verified by the maintainer): quickstart §1 steps 3–7 and 9 on dummyplug via `scripts/client-tools-e2e.mjs`: one answered completion on a successful flagged call, `stats.ended_by_tool` set, the turn continues on error/script failure/deferred, and a restart mid-turn ends the resumed turn.

- [X] T034 [P] [US3] Add `ends_turn: bool` (`#[serde(default, skip_serializing_if = "is_false")]`) to `ToolInvocationRecord` in `src/sandbox/report.rs` and `ExecutionReport::ended_turn(&self) -> bool` (`ok` and some record with `ok && ends_turn`); unit tests for all combinations (data-model §1)
- [X] T035 [US3] In `src/agents/tools/mod.rs` add `fn ends_turn() -> bool { false }` to `VizierTool`, forward it through the `VizierToolDyn` blanket impl, add `CatalogueEntry.ends_turn` and fill it in `ToolRouter::catalogue()` (native and addon from the tool, MCP `false`); test that every current tool reports `false` and a test tool reports `true`
- [X] T036 [US3] Add `ends_turn: bool` (`#[serde(default, skip_serializing_if = "is_false")]`) to `ClientToolDeclaration` in `src/agents/client/declare.rs`; confirm how a non-boolean value is refused (deserialisation failure path in `handle_socket`) and make it surface as `client_error` `invalid_client_tools`, adding the rule name `ends_turn_type` only if a rule is reachable; test both
- [X] T037 [US3] In `src/agents/client/mod.rs` `ClientTools::resolve` set `CatalogueEntry.ends_turn` from the declaration and echo `ends_turn` per tool in `resolved_frame()`; extend the existing tests
- [X] T038 [US3] In `src/agents/tools/python/bridge.rs` set `ToolInvocationRecord.ends_turn` from the dispatched `CatalogueEntry` (native, MCP and client targets); test with a client entry and a native entry
- [X] T039 [P] [US3] Add `ended_by_tool: Option<String>` (`#[serde(default)]`) to `VizierResponseStats` in `src/schema/response.rs` and update every construction site (`grep -rn "VizierResponseStats {" src`)
- [X] T040 [US3] In `src/agents/agent/mod.rs` `prompt_with`: after the round's tool responses are recorded, if the round's call was `execute_python` and its report `ended_turn()`, push the round's tool results to `history`/`full_history`, take the step's own text parts as `output`, set `ended_by_tool`, skip the closing completion and the checkpoint, and `break` through the existing success path (`guard.succeeded()`); extract the detection into a small tested function
- [X] T041 [US3] In `chat_turn` (same file) flush **every** new message when `stats.ended_by_tool` is set instead of dropping the last one, so the final tool result is recorded; test with a fixture history ending in a tool result
- [X] T042 [US3] Resume: in `chat_turn`/`prompt_with`, when a resumed turn's last recorded message is an `execute_python` result whose report `ended_turn()`, finalise with `ended_by_tool` instead of requesting a completion (data-model §1); unit test using `history_entries_to_messages`
- [X] T043 [P] [US3] Telemetry: `vizier.tool.ends_turn` on the `execute_tool` span (`traced_tool_call` and `ToolRouter::call` in `src/agents/tools/mod.rs`) and `vizier.turn.ended_by_tool` on `invoke_agent` (`src/agents/agent/mod.rs`); no content attributes outside `capture_content()`
- [X] T044 [P] [US3] Add `ends_turn?: boolean` to `ClientToolDeclaration` in `webui/app/api/frames.ts` and set `ends_turn: true` on the `ui_render` definition in `webui/app/lib/clientTools/builtin.tsx`

**Checkpoint**: US1 + US2 + US3 deliver the headline: a generated, interactive interface that is the turn's final answer.

---

## Phase 6: User Story 4 - The agent is told a tool is turn-ending (P2)

**Goal**: any flagged tool's description carries the standard notice exactly once; unflagged tools and the standing instructions are unchanged.

**Independent Test** (verified by the maintainer): quickstart §1 steps 1–2 and 8: `tools`/describe show the notice once for a flagged client tool and not for an unflagged one; a near-limit author description is accepted; the system prompt is byte-identical with and without a flagged tool.

- [X] T045 [US4] Add `ENDS_TURN_NOTICE` and one helper `with_ends_turn_notice(definition, ends_turn)` in `src/agents/tools/mod.rs`; apply it in `ToolRouter::catalogue()` for native and addon entries; tests: appended once, absent when unflagged, not duplicated on repeated catalogue builds
- [X] T046 [US4] Apply the same helper in `ClientTools::resolve` (`src/agents/client/mod.rs`) after the existing `(Provided by the person's client.)` prefix; test the combined text and that a 2048-byte author description is still accepted by `declare::validate` (the notice is added after the size check)
- [X] T047 [P] [US4] Test in `src/agents/agent/system_prompt/sandbox.rs` that `SANDBOX.md` output is byte-identical for catalogues with and without a flagged tool (standing instructions list categories only)
- [X] T048 [P] [US4] Check the dummyplug `tools` listing and sample-request output (`src/agents/agent/model/dummyplug.rs`) show the notice through the normal definition and need no protocol change; add a unit test, leave `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md` unchanged
- [X] T049 [US4] Update `docs/src/api-integration/client-tools.md`: the `ends_turn` field in the declaration table, the notice text, the turn-ending semantics, and the changelog line "optional `ends_turn` on declarations (version 1)" (contracts/ends-turn-flag.md §1–§3, §6)

**Checkpoint**: the flag is a documented, general mechanism.

---

## Phase 7: User Story 5 - Rich apps and simulations (P2)

**Goal**: chess and a low-poly 3D terrain generator work from the catalogue plus sandboxed logic, with no agent turn per interaction.

**Independent Test** (verified by the maintainer): quickstart §3a: the `board_game` recipe plays legal moves locally, the `scene_3d` recipe regenerates terrain on a slider within 100 ms and can be orbited, a runaway or network-touching function is stopped with an inline error.

- [X] T050 [US5] Add `three` and `@react-three/fiber` (exact versions) to `webui/package.json`; confirm the build emits the 3D module as a separate lazy chunk and the main bundle size does not grow (Vite output)
- [X] T051 [P] [US5] Add `Canvas2D` (shapes rect/circle/line/path/text, `press`/`move` events) to `webui/app/lib/generativeUi/catalog/surface.ts` and the 3D entries (`Scene3D`, `Box`, `Sphere`, `Cylinder`, `Plane`, `Group3D`, `AmbientLight`, `DirectionalLight`, `PointLight`, `OrbitControls`, `Heightfield`, `Mesh`) to `catalog/surface3d.ts`; add `Effect` (`intervalMs` ≥ 50, `running`, events `load`/`tick`) to `catalog/control.ts`
- [X] T052 [P] [US5] Implement `Canvas2D` in `webui/app/lib/generativeUi/registry/canvas2d.tsx` (SVG or canvas, scaled to fit, closed colour tokens, press reports `{x,y,id?}`)
- [X] T053 [P] [US5] Implement `Scene3D` and the primitive/light/controls components in `webui/app/lib/generativeUi/registry/scene3d.tsx` as a `React.lazy` module, with a "3D is not available" inline note when WebGL is missing and presses reporting the object `id`
- [X] T054 [P] [US5] Implement `Heightfield` (size, resolution ≤ 192, `heights` row-major, `heightScale`, `palette`, `flat` default true, wireframe, press → `{x,z,height}`) and `Mesh` in `webui/app/lib/generativeUi/registry/terrain.tsx` inside the lazy 3D module; unit-test geometry generation (vertex/index counts, flat shading normals)
- [X] T055 [P] [US5] Implement `Effect` in `webui/app/lib/generativeUi/registry/control.tsx`: `load` once per draw, `tick` at `intervalMs` (≥ 50 ms, one tick at a time), paused off screen (`IntersectionObserver`) and in a hidden tab, stopped after 10 minutes without a person interaction; `control.test.ts` with fake timers
- [X] T056 [US5] Implement the logic runner in `webui/app/lib/generativeUi/logic/runner.ts` (the fixed runner script: compile `new Function`, remove timers, `fns` object, call depth ≤ 32, JSON-only results) and `logic/host.ts` (opaque-origin `sandbox="allow-scripts"` iframe with the CSP of contracts/logic-sandbox.md §2, FIFO queue ≤ 16, 250 ms timeout that destroys and recreates the frame, 256 KB args/results, lifecycle of §4)
- [X] T057 [P] [US5] Write `webui/app/lib/generativeUi/logic/host.test.ts` against a fake runner transport: queueing, timeout rejection and recreation, oversize rejection, `logic_busy`, error message truncation
- [X] T058 [US5] Add the `run {fn,args,into}` action and the `watch`/chain rules to `webui/app/lib/generativeUi/actions.ts`: `watch`, `Effect` and logic results run untrusted (cannot `sendMessage`), a chain started by a `watch` ignores watch triggers on its own writes, chain depth ≤ 8, a failed `run` leaves state unchanged and stops the chain; extend `actions.test.ts`
- [X] T059 [US5] Show a logic failure as a small inline error under the interface in `GenerativeUi.tsx` (`<fn>: <message>`), cleared by the next success, and `console.warn` with the function name
- [X] T060 [US5] Extend `webui/app/lib/generativeUi/validate.ts`: every `run.fn` names an entry in `logic`, logic limits (64 KB total, ≤ 32 functions, ≤ 8 params), `Heightfield.resolution ≤ 192`, `repeat` ≤ 2048 items, `Canvas2D.shapes` ≤ 5000; tests added to `validate.test.ts`
- [X] T061 [P] [US5] Add the `board_game` recipe in `webui/app/lib/generativeUi/recipes/board_game.ts` with a complete chess `step(game, index)` function (move generation, check, turn passing; ~90 lines) per contracts/ui-catalogue.md §5, and a unit test of `step` on opening moves, captures and an illegal move
- [X] T062 [P] [US5] Add the `scene_3d` recipe in `webui/app/lib/generativeUi/recipes/scene_3d.ts` with a seeded value-noise `generate(seed, roughness, n)` (deterministic, n×n values in 0..1) and a unit test of determinism and range; and the `canvas_2d` recipe in `recipes/canvas_2d.ts` (shapes from state, `press` → `run`, `Effect tick` animation)
- [X] T063 [US5] Update `reference.ts` and the `ui_components` input enum for the surface and control groups and the three new recipes; extend `reference.test.ts` and `recipes.test.ts`

**Checkpoint**: the plan's flexibility goal is met.

---

## Phase 8: User Story 7 - Links and conversation files inside an interface (P2)

**Goal**: safe links, and image/audio/video/file/gallery components that show the conversation's session files by name.

**Independent Test** (verified by the maintainer): quickstart §2b: media from the conversation display and play without autoplay, an unknown name is refused with the available files, unsafe link schemes are refused, the host is shown when label and address differ.

- [X] T064 [P] [US7] Add media entries (`Link`, `Image` with `file` or `src`, `Audio`, `Video`, `FileCard`, `Gallery`) to `webui/app/lib/generativeUi/catalog/media.ts` and the `image` shape to `Canvas2D` in `catalog/surface.ts` (contracts/ui-catalogue.md "Links and media")
- [X] T065 [P] [US7] Implement the link rules in `webui/app/lib/generativeUi/links.ts` (`URL` parsing; allow only `https`, `http`, `mailto`; reject `javascript:`, `data:`, `file:`, `blob:`, protocol-relative and control-character tricks; host-disclosure decision) with `links.test.ts` covering the attack strings of quickstart §2b step 7
- [X] T066 [US7] Render `Link` and links inside `Text` through one shared anchor in `webui/app/lib/generativeUi/registry/text.tsx` and `registry/media.tsx`: `target="_blank"`, `rel="noopener noreferrer nofollow"`, `referrerpolicy="no-referrer"`, host shown beside a differing label, `file` links downloading the session file
- [X] T067 [US7] Implement file-name resolution in `webui/app/lib/generativeUi/media.ts`: read the topic's file list through the existing client used by `webui/app/screens/chat/SessionFilesPanel.tsx` (`GET …/topic/{topic}/files`), map name → record → `/files/{file_id}`, re-resolve when the list changes, expose "not available" for a missing name; `media.test.ts`
- [X] T068 [US7] Implement `Image`, `Audio`, `Video`, `FileCard` and `Gallery` in `webui/app/lib/generativeUi/registry/media.tsx` by reusing `webui/app/screens/chat/ReplyMedia.tsx` pieces and `FilePreviewDialog` (`preload="metadata"`, never autoplay, seek/speed/expand/download, inline placeholder for a missing file, `Image` also accepts `data:image/`)
- [X] T069 [US7] Add the submit-time file check: `ui_render`'s handler in `webui/app/lib/clientTools/builtin.tsx` fetches the file list and passes it to `validate.ts`, which refuses unknown `file` names with the names and mime types of the available files (contracts/generative-ui-tools.md §5); tests in `validate.test.ts` including `Image.src` external URL refusal and link-scheme refusal
- [X] T070 [US7] Draw conversation images in `Canvas2D` (`image` shape) in `registry/canvas2d.tsx` using an `<img>` from the resolved URL
- [X] T071 [US7] Add the `media` group to `reference.ts` / the `ui_components` enum and the one-line media/link mention to the `ui_render` description (re-check the 2048-byte test)

**Checkpoint**: interfaces can point at sources and show conversation files.

---

## Phase 9: User Story 6 - Existing pages keep working, the old tool is retired (P3)

**Goal**: new turns no longer offer `render_interactive_ui`; saved pages still draw.

**Independent Test** (verified by the maintainer): quickstart §2 steps 1 and 10: `tools` lists only the new tools; an old saved page still draws with its state; calling the old name returns the retired hint.

- [X] T072 [US6] Add `declare?: boolean` (default true) to `ClientToolRegistration` and filter it in `declaredClientTools()` in `webui/app/lib/clientTools.ts`; add a test that a non-declared registration still resolves in `findClientTool`
- [X] T073 [US6] In `webui/app/lib/clientTools/builtin.tsx` keep `render_interactive_ui` registered as render-only (`declare: false`, `handle` rejecting, `render` unchanged), drop `RENDER_INTERACTIVE_UI_DESCRIPTION` from `webui/app/lib/clientTools/interactiveUi.ts` and update `interactiveUi.test.ts`
- [X] T074 [P] [US6] Add `ui_render_interactive_ui` to `RETIRED_TOOL_NAMES` in `src/agents/tools/mod.rs` with the hint "render_interactive_ui was replaced by ui_render. Start with ui_components()." and a test of `retired_hint`
- [X] T075 [P] [US6] Remove the skill template `templates/skills/interactive-ui/` and any embedding or listing of it (`grep -rn "interactive-ui" src templates build.rs Cargo.toml docs`); update the tool names in the fixtures at `src/agents/agent/system_prompt/sandbox.rs` tests and the comment in `src/schema/history.rs`

**Checkpoint**: the old path is retired without losing history.

---

## Phase 10: Polish & Cross-Cutting

- [X] T076 [P] Write the user docs `docs/src/api-integration/generative-ui.md` (what an agent can build, the catalogue summary, logic functions, links and files, limits) and add it to `docs/src/SUMMARY.md`; update the `render_interactive_ui` mentions in `docs/src/api-integration/client-tools.md`
- [X] T077 [P] Update `CLAUDE.md`: the Client tools paragraph (generative UI replaces `render_interactive_ui`, `ui_components`/`ui_render`, `ends_turn`, the notice, `ended_by_tool`), the WebUI section (`lib/generativeUi/`, json-render, the logic runner), and the "Adding a new tool" note on `ends_turn`
- [ ] T078 Draft the checklist entry for issue #82 (legacy render-only registration and `declare` flag, `interactive-ui` retirement) and add it after confirming with the maintainer
- [X] T079 Regenerate the API types after the `ended_by_tool` change: `node webui/scripts/openapi.mjs` then `cd webui && npm run gen:api`; commit the diff
- [X] T080 Run `cargo clippy` and `cargo test`; fix warnings introduced by this feature
- [X] T081 Run `cd webui && npm run typecheck && npm test && npm run lint:tokens`; fix findings
- [X] T082 Commit in order: `design/` first, then `webui/`, then the vizier repo with the submodule pin bumps, using conventional commits and `[**breaking**]` for the removal of `render_interactive_ui`

---

## Dependencies & Execution Order

- **Phase 1** → **Phase 2** → user stories. Phase 2's design components block everything that renders (US1, US2, US5, US7).
- **US1** (P1) needs Phase 2. **US2** needs US1's `GenerativeUi.tsx` and state wiring. **US3** is Rust-only for most of its tasks and can start right after Phase 1; its last WebUI tasks need US1's `builtin.tsx`. **US4** needs US3's entry flag and resolve changes. **US5** needs US2 (actions, state) and US1 (validate, reference). **US7** needs US1 and, for the Canvas image and `Text` links, US5 and US1's registry. **US6** needs US1 (so the replacement exists) and can run any time after.
- **Polish** last.

```text
Phase 1 → Phase 2 ──┬─ US1 → US2 → US5 ─┐
                    │        └─ US7 ────┤→ Polish
     (Rust, early) ─┴─ US3 → US4        │
                          US6 (after US1)┘
```

## Parallel Opportunities

- Phase 2: the five design components (Stack, Row, Grid, Slider, Chart) and `functions.ts` / `state.ts` are independent files.
- US1: the three catalogue files and the three registry files (six tasks) in parallel; `validate.test.ts` and the dashboard recipe in parallel with `reference.ts`.
- US3 (Rust) runs in parallel with US1/US2 (WebUI) from the start: different languages, different files. Within US3: the `report.rs` change, the `VizierResponseStats` field and the telemetry attributes are independent.
- US5: `Canvas2D`, `Scene3D`, `Heightfield`/`Mesh`, `Effect`, and the runner host test are independent files; the three recipes are independent.
- US7: `links.ts` and the catalogue entries in parallel.

### Parallel example (US1)

```text
T013 (catalog/layout.ts)   T014 (catalog/text.ts)   T015 (catalog/data.ts)
T016 (registry/layout.tsx) T017 (registry/text.tsx) T018 (registry/data.tsx)   (each registry task follows its catalogue file)
T022 (dashboard recipe)    T020 (validate.test.ts)
```

## Implementation Strategy

**MVP = Phase 1 + Phase 2 + US1 + US3** (`ui_render` draws a catalogue-only interface, `ui_components` teaches the agent, and the call ends the turn). That is the headline of the request and is shippable alone behind the existing client tools mechanism, with the old tool still offered until US6.

**Increments**: add US2 (interaction), then US4 (flag documented for others), then US7 (links, media) and US5 (logic, surfaces) in either order, then US6 (retire the old tool), then Polish. Each increment ends at a checkpoint with its Independent Test.

**Order of landing**: retire the old tool (US6) only after the maintainer's live-model check (quickstart §3b, no skill installed) shows agents use the new one correctly, so a regression never leaves agents without any UI tool.
