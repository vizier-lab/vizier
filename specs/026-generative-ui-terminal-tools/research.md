# Research: Generative UI and Turn-Ending Tools

All unknowns from the Technical Context are resolved below. Sources: the json-render packages (`@json-render/core`, `react`, `react-three-fiber` 0.21.3, read from the published tarballs), and vizier's own code at `src/agents/agent/mod.rs`, `src/agents/client/`, `src/agents/tools/`, `src/sandbox/`, `webui/app/lib/clientTools*`.

---

## R1. Where does the turn end? (the turn-ending flag)

**Finding that shapes everything**: every agent is always in code mode. The model is offered only `think`, `execute_python`, `list_tool_functions` and `describe_tool_function`; every other tool, client tools included, is called from inside an `execute_python` script through `RouterBridge`. So the agent loop (`prompt_with`) never sees the turn-ending call itself, only an `execute_python` result.

**Decision**: the signal travels in the script's `ExecutionReport`, not in a side channel.

- `ToolInvocationRecord` (`src/sandbox/report.rs`) gains `ends_turn: bool` (skipped when false), set by `RouterBridge` from the called `CatalogueEntry`.
- After each round, `prompt_with` looks at the `execute_python` tool response: if `report.ok` and some record has `ok && ends_turn`, the round was a turn-ending one. It pushes the round's tool results into the history as usual, skips the next completion, and returns with the step's own text as the answer.
- A script that fails after a turn-ending call has `ok == false`, so the turn does not end (spec FR-017). A call that failed has `ok == false` on its record, so it does not count (FR-015). A `{"status": "deferred"}` result is not a result, so the generative UI tool is declared non-deferrable and a deferred answer never ends a turn.

**Why not a flag on `ToolContext` (like `recalled`)**: a side channel is lost on resume. The report is saved in history, so the same check on the last recorded round makes a resumed turn end instead of asking the model for another round (FR-022, spec edge case).

**Alternatives rejected**
- *Detect by tool name in the loop*: branching on names, breaks Constitution II.
- *End the turn inside the bridge* (return a sentinel error): conflates success with failure and loses the report.
- *Make the call a direct model tool*: code mode forbids it, and the client tools only exist per request.

**Where the final reply is built**: `chat_turn` currently flushes `new_messages[..len-1]` and treats the last message as the final assistant message. A turn-ending round's last message is a tool result, so `prompt_with` reports the end through `VizierResponseStats.ended_by_tool: Option<String>` (tool name; precedent: `depth_exhausted`), and `chat_turn` flushes every new message when it is set. The reply is a `Message` whose content is the text the model wrote in that step (possibly empty). `answer_without_tool_calls` already shows the "keep text, drop calls" pattern.

**Empty reply text**: the WebUI keys client-call rendering on the reply bubble; an empty-content reply must still produce a bubble when a visible client call exists. A WebUI task covers `groupHistory` (see tasks).

---

## R2. Flag name and the notice

**Decision**: the wire and code name is **`ends_turn`** (boolean). "terminal" is rejected because it already means the shell terminal in this product (`shell` tools, Docker shell); "final" reads as "last tool of all". `ends_turn` says what it does.

- Rust: `VizierTool::ends_turn() -> bool` (default `false`, one default method, no branching), `CatalogueEntry.ends_turn`, `ClientToolDeclaration.ends_turn` (`#[serde(default)]`, absent means false, so existing clients are byte-for-byte unchanged).
- The standard notice is added **where a `ToolDefinition` is built for a catalogue entry**: `ToolRouter::catalogue()` for native and addon tools, `ClientTools::resolve` for client tools (which already prefixes `(Provided by the person's client.)` the same way). One constant, `ENDS_TURN_NOTICE`, appended once:

  > `Calling this ends your turn: its result is your final answer, so finish all other work first and expect no further round. If the call fails you get the error and the turn continues.`

- The 2048-byte description limit in `client::declare` checks the author's text only; the notice is added after the check (spec US4.4).
- The standing instructions (`SANDBOX.md`) are untouched: they list categories only, so they stay byte-identical across turns (US4.5).
- MCP tools cannot carry the flag in this release (the MCP spec has no such field; out of scope).

---

## R3. Depend on json-render, or implement the format?

**Decision**: depend on `@json-render/core` and `@json-render/react` (both Apache-2.0, 0.21.3, peers only `react` and `zod`). Pin exact versions (the project is pre-1.0 and its own README says to).

What we take: `defineSchema`/`defineCatalog` (zod props per component, `description`), `catalog.prompt()` as a starting point for the reference, `validateSpec`/`autoFixSpec`/`formatSpecIssues` (structural checks: dangling children, bad `visible`, repeat misuse), `Renderer`, `StateProvider`/`ActionProvider`/`defineRegistry`, the expression language (`$state`, `$bindState`, `$cond`, `$template`, `$computed`, `$item`, `$index`), `repeat`, `visible`, `watch`, built-in actions (`setState`, `pushState`, `removeState`, `validateForm`), and the `StateStore` interface.

Why not hand-roll: the expression language, repeat scopes, two-way binding and structural validation are ~2k lines of subtle code we would own forever; the format is the one the request names, and models are already familiar with it.

Why it is not heavy: core+react unpacked ≈ 1.8 MB source, but tree-shaken client code is small, and zod is the only transitive dependency (also needed for the catalogue). No new Rust crate.

**Not taken**: `@json-render/shadcn` (36 shadcn components) because the WebUI composes only `@vizier/design`; the registry maps catalogue entries to design components instead. `@json-render/react-three-fiber` is evaluated in R8.

**Revisit trigger**: if a json-render release breaks the spec format, the catalogue and validators are isolated in `webui/app/lib/generativeUi/` and saved specs carry a `format` version (data-model), so a migration is local.

---

## R4. Where is the spec validated?

**Decision**: in the WebUI client tool handler, with the catalogue's zod schemas plus `validateSpec`. The Rust side cannot run zod, and a second hand-written JSON-schema validator in Rust would duplicate the catalogue (Constitution II).

- A bad spec makes the handler throw; the client answers the call with an error; the script receives a catchable tool failure; `ok == false` on the record, so the turn does **not** end and the agent can correct and call again in the same script or the next round.
- Error text is written for the model: every issue as `path: problem. Valid: …` with nearest-name suggestions (`Chart.kind "pie2" → valid: bar, line, pie`), all issues in one message (not first-only), capped at 20.
- Lossless `autoFixSpec` fixes (e.g. `visible` placed inside `props`) are applied silently and reported as `fixed` in the result; lossy ones are never applied silently, they are errors.
- Server-side, vizier only checks what it already checks for any client tool call (answer size, timeout). Third-party clients that do not declare the tool never see it.

Limits (enforced here): spec ≤ 256 KB serialized, ≤ 1500 elements, depth ≤ 16, logic source ≤ 64 KB total, `repeat` ≤ 2048 rendered items, Heightfield ≤ 192×192.

---

## R5. How the agent discovers the components, with no skill

Today the agent learns the HTML kit from the `interactive-ui` skill plus a 2 KB tool description. The replacement uses only mechanisms every agent already has, in three depths (progressive disclosure, so the standing prompt never grows):

1. **Category line** (system prompt, from `registerClientCategory`, ≤ 512 bytes): `ui — Show interfaces made of ready-made components. Start with ui_components.` It is the only always-present text and is byte-stable.
2. **`list_tool_functions(category="ui")`** returns `ui_components` and `ui_render` with their one-line summaries (the first sentence of each description), so the order of use is stated twice.
3. **`describe_tool_function("ui_render")`** returns its description (≈1.6 KB, kept under the 2048-byte cap) and input schema. The description is a *cheat sheet*, not a catalogue: the spec shape in one example, the five expression forms, the rule "only catalogue components, call `ui_components` for props", the single-final-call rule, and the limits.
4. **`ui_components`** (a second client tool, read-only, **not** turn-ending, cheap) is the reference. `ui_components()` with no arguments returns the index: every component's name, one-line purpose and group (≈1.5 KB). `ui_components(names=["Table","Chart"])` returns full prop signatures (type, required, enum values, defaults, events it emits, children rules) and a minimal example per component. `ui_components(recipe="dashboard")` returns a complete, valid, runnable spec. Recipes: `dashboard`, `form`, `tabs_and_filter`, `board_game` (list-driven grid + logic), `scene_3d` (scene + heightfield + sliders + logic), `canvas_2d`.
5. **Corrective errors** (R4) teach what the docs missed.

Why a tool and not the description: the 2048-byte cap and the 16 KB schema cap cannot hold ~35 components with props, and loading 15 KB of reference for a one-stat answer wastes tokens every turn. Why not the system prompt: it must stay byte-identical across turns and is paid on every turn of every conversation, including ones with no UI.

**Single source** (spec FR-003a): the reference text is generated from the catalogue definitions (`description` + zod → compact signature formatter) and from the recipes, which are real specs. A vitest test validates every recipe against the catalogue, so a catalogue change that breaks a recipe fails CI. json-render's `catalog.prompt()` is used as a base for the index, but its output is long and verbose (it lists every prop of every component), so the on-demand formatter is ours.

**Alternative rejected**: shipping a skill (what the request rules out) or putting the catalogue in the tool's input schema (16 KB cap; also nothing for recipes).

---

## R6. Flexibility: chess and low-poly 3D terrain

A fixed catalogue covers dashboards and forms. Two things it cannot express are *surfaces* (a 3D view, free drawing) and *rules* (is this chess move legal; heights from a seed). Three tiers cover both without opening a hole:

**Tier 1, declarative components** (R7): enough for a chess board's *look*: `Grid` + `repeat` over a 64-item list in state, each cell a `Button` styled by `$cond` on selection/legal-move membership, piece glyphs as text.

**Tier 2, surfaces**: `Canvas2D` (shape list, press reports `{x, y, id}`) and `Scene3D` with `Box/Sphere/Cylinder/Plane/Group/lights/OrbitControls` plus two we add: `Heightfield` (size, resolution, `heights` from state, colour ramp, `flat: true` for low-poly shading) and `Mesh` (vertices/indices/colors arrays). Presses on 3D objects report the object id (raycast), so clicking a terrain tile or a chess piece in 3D works.

**Tier 3, logic**: top-level `logic: { name: { params: [...], body: "js source" } }`. Pure functions, called by an action `run {fn, args, into}` that writes the result into state. They run in a sandbox (R9). Triggers: a button press, a `watch` on a state path (slider → regenerate terrain), the `Effect` component's `load` (compute the initial height map) and `tick` (simulation step) events. This is what makes chess playable without an agent round trip: one `step(game, index)` function (select a piece, or move it and pass the turn) is ~100 lines the agent writes once.

Walk-throughs (full text in `contracts/ui-catalogue.md` recipes and `quickstart.md`):
- **Chess**: state `{game:{board:[64 glyphs], turn:"w", selected:null, legal:[]}}`; `Grid cols=8` with `repeat` over `/game/board`; each cell a `Button` with `$cond` highlighting, `on.press → run step(game, index)` writing `/game`; a status `Text` with `$template`. Pressing a cell is local. A "Ask the agent for a move" `Button` uses `sendMessage` with the FEN in `data`, which starts a turn (the agent replies with a *new* interface or text).
- **Terrain**: state `{seed:7, roughness:0.5, height:3, _heights:[…]}`; `Scene3D` with `Heightfield heights={$state /_heights}`, lights, `OrbitControls`; three `Slider`s `$bindState`; each with a `watch` that runs `generate(seed, roughness, N)` → `/_heights`; an `Effect` `load` runs it once. Low-poly look = `flat: true` + coarse resolution 32–64.

**What stays out**: no rule engines or physics libraries are shipped; no network; no DOM access. If an agent needs something the catalogue and logic cannot express it can still send a message and replace the interface.

**State that is not saved** (FR-030): top-level state keys starting with `_` are excluded from the saved page state and recomputed on draw (by `Effect load`). The 64 KB saved-state limit from 019 stays; a 129×129 height map is ~130 KB and would not fit.

---

## R7. The predefined component catalogue (v1)

Mapped to `@vizier/design` where a component exists; gaps are added in `design/` first (design → webui → vizier order). Full props in `contracts/ui-catalogue.md`.

| Group | Components |
|---|---|
| Layout | `Stack`, `Row`, `Grid`, `Card`, `Tabs`, `Fold`, `Divider` |
| Text | `Heading`, `Text` (markdown subset via the existing sanitised pipeline), `Badge`, `Callout`, `Code` |
| Data | `Stat`, `Table`, `KeyValue`, `Progress`, `Chart` (`bar`, `line`, `area`, `pie`) |
| Links & media | `Link`, `Image`, `Audio`, `Video`, `FileCard`, `Gallery` (session files, R12) |
| Input | `TextInput`, `NumberInput`, `Slider`, `Select`, `Checkbox`, `Switch`, `RadioGroup` |
| Action | `Button` |
| Surface | `Canvas2D`, `Scene3D`, `Box`, `Sphere`, `Cylinder`, `Plane`, `Group3D`, `AmbientLight`, `DirectionalLight`, `PointLight`, `OrbitControls`, `Heightfield`, `Mesh` |
| Control | `Effect` (`load`, `tick`) |

Events: `press` (Button, Canvas2D, 3D objects, Table rows as `select`), `change` (inputs), `load`/`tick` (Effect). Actions: `setState`, `pushState`, `removeState`, `validateForm` (json-render built-ins) plus `sendMessage {text, data?}` and `run {fn, args?, into}`.

`$computed` functions (pure, registered once): `add sub mul div mod min max abs round floor eq not and or len at concat join slice includes range`. Enough for parity-coloured boards and indexes without custom code.

Links are limited to `https`/`http`/`mailto` (R12); no raw HTML, no style props beyond closed enums (`gap`, `align`, `tone`, `size`): nothing the agent writes can restyle or script the WebUI itself.

Design gaps to build in `design/` (audit confirmed `Stack/Row/Grid`, `Slider`, `LineChart`/`PieChart` are not exported): layout primitives, `Slider`, `Chart` (line/area/pie; bar exists as `BarList`/`StackedBarChart`).

---

## R8. 3D: reuse `@json-render/react-three-fiber` or define our own subset?

The package gives 19 components, but its dependency list adds `@react-three/postprocessing` and peers `@react-three/drei` (large), for components we do not need (`GaussianSplat`, `Model`, `Text3D`, `ExtrudedText`, environments). We still need our own `Heightfield` and `Mesh`.

**Decision**: depend on `three` and `@react-three/fiber` (both MIT) only, and define a small subset of definitions in our catalogue (`Scene3D`, shapes, lights, `OrbitControls` from `three/examples/jsm`, `Heightfield`, `Mesh`) following the package's component shapes so they stay recognisable. The whole 3D module is a **lazy chunk** (`React.lazy`), loaded only when an interface contains a `Scene3D`, and is embedded in the build output like every other WebUI asset (Constitution III: no CDN).

**Revisit**: if the catalogue's 3D set grows past ~15 components, switch to the package.

---

## R9. Running agent-written logic safely

Requirements: synchronous-looking results are not needed (actions are async), no network, no DOM, hard timeout, bounded output, works offline and in the static build.

**Decision**: a **sandboxed iframe runner**, reusing the isolation the 019 interactive page already proved: `sandbox="allow-scripts"` (opaque origin, no cookies/tokens/parent DOM) with a CSP of `default-src 'none'` (no network). The runner is a ~60-line fixed script; the logic bodies are compiled inside it with `new Function`, called over `postMessage` with `{id, fn, args}`, and answer `{id, result}` or `{id, error}`.

- **Timeout**: the host starts a 250 ms timer per call; on expiry the iframe is destroyed and recreated (the only reliable way to stop an infinite loop in a same-thread frame), the call fails with a `logic_timeout` error. Concurrent calls queue (one at a time, FIFO, max 16 queued).
- **Data**: arguments and results are JSON only (structured clone of plain data), results ≤ 256 KB, args ≤ 256 KB.
- **No capabilities**: the frame has no `allow-same-origin`, no forms, no popups; `fetch`/`XMLHttpRequest`/`WebSocket` are blocked by CSP; `import()` is blocked by CSP.
- **One runner per drawn interface**, created on first call, destroyed when the interface unmounts or goes off screen.
- **Why not a Web Worker**: `new Worker(blobUrl)` inherits the page origin and the WebUI's own CSP; the worker could `fetch` the vizier API with the person's cookies. The sandboxed iframe has an opaque origin and cannot.
- **Why not QuickJS-WASM** (`quickjs-emscripten`): a fine alternative with synchronous calls and instruction limits, but ~500 KB of WASM and a new dependency; the iframe needs nothing new. Revisit if sync evaluation inside `$computed` becomes necessary.

`Effect.tick` minimum interval is 50 ms; ticks pause when the interface is off screen (`IntersectionObserver`) or the tab is hidden, and stop entirely after 10 minutes without a person interaction, to keep a forgotten simulation from running forever.

**`sendMessage` gating**: a message to the agent is sent only from an action chain that started at a *trusted* DOM event (`event.isTrusted` on the press/change that fired it). `watch`, `Effect` and logic results can never send. Rate ≤ 1 per second per interface (019 rule, now enforced by trust of origin rather than by iframe focus).

---

## R10. Rendering, persistence and the old tool

- The new tool renders as **native React** in the reply bubble (not an iframe): it matches theme, is accessible (DOM, not a sealed frame), and re-themes live. The old `InteractivePage` (iframe) remains for *saved* `render_interactive_ui` calls (US6).
- `ClientToolRegistration` gains `declare?: boolean` (default true): the legacy registration keeps its `render` so history draws, but is not declared on new requests, so agents stop seeing it (FR-023). A call to the old name from a script fails with the existing "unknown function" hint extended through `retired_hint` to point at `ui_render`.
- Saved form: the call's `args` hold the spec (`{title, spec}`), the existing per-call saved state holds the page state (without `_` keys), both already stored in `client_tool_call`. No schema change, no migration.
- **Not progressive**: a script makes the call, so the spec arrives whole; json-render's `SpecStream` patch streaming does not apply here. The speed gain is the much shorter output (R6 sizes), and the removal of the closing model round (R1). The spec's earlier "renders as the agent writes it" wording was removed.

---

## R12. Links and media (built on session files)

**Links** (`Link`, and links inside `Text`): `https`, `http`, `mailto` only (parsed with `URL`, so `javascript:`, `data:`, `file:`, `blob:` and scheme tricks fail), `target="_blank"` with `rel="noopener noreferrer nofollow"`, `referrerpolicy="no-referrer"`. When the label differs from the address the host is shown beside it (`Docs (example.com)`), so a label cannot hide the destination. The renderer is the only place an `<a>` is made for the catalogue; markdown in `Text` goes through the same sanitiser as chat replies with the same link rule. A `Link` with `file` downloads a session file instead.

**Media from session files, not from addresses.** Everything the agent can show already lives in the conversation's session files: what the person attached, what tools produced (image generation, TTS, `send_attachment`, files loaded from a path), and what other tools saved. The record gives name, mime type and a `file_id`; the HTTP channel already serves `GET /files/{file_id}` with `Range` support, and the WebUI already has `AudioPlayer`, `VideoPlayer`, `ImageGallery` and `FilePreviewDialog` (`screens/chat/ReplyMedia.tsx`). So:

- A component refers to a file by **name** (`"file": "forecast.png"`). The spec stays small, portable and free of ids and URLs.
- The WebUI resolves names to records from the topic's file list (`GET …/topic/{topic}/files`, already used by the session files panel), builds the `/files/{file_id}` URL, and re-resolves when that list changes (a file generated later appears without redrawing).
- **At submit time** `ui_render` fetches the same list and refuses unknown names, answering with the available names and mime types. This teaches the workflow (make or find the file, then show it) and lets a freshly generated image be shown in the same script that generated it.
- External addresses are refused by schema: `file` is a name pattern, `src` accepts only `data:image/`. This keeps FR-008 (no network from an interface) true for media, and avoids hotlinking, tracking pixels and mixed-content problems.
- Audio and video use `preload="metadata"` and never autoplay (FR-037); the standard players give seek, speed and expand; `FileCard` opens `FilePreviewDialog` (PDF, CSV, text, images) and downloads.
- `Canvas2D` gets an `image` shape from a session file (drawn through an `<img>`, same origin, so the canvas is not tainted). Textures on 3D objects are **not** in v1; the same `file` reference can be added to `Box/Plane/Heightfield` later without a contract change.
- No server change: no new endpoint, no new storage. Access control is the existing one (the person must already be able to open the topic and its files); an interface only names files in its own conversation.

**Lifetime**: session files last as long as the session and are deleted with it; scratchpad files are text and temporary and are **not** eligible. A replaced or removed file shows the inline placeholder (FR-035).

**Discovery**: the `ui_components` index has a `media` group, and the `ui_render` cheat sheet gains one line: "Show files of this conversation with Image/Audio/Video/FileCard/Gallery by file name (see list_session_files); make the file first." Errors list the available files.

---

## R11. Testing

- **Rust unit**: `ends_turn` on the trait default, notice appended once for native and client entries and not for others, declaration check ignores the notice, report detection (`ok && record.ends_turn && record.ok`), resume detection, `answer` flush when ended.
- **Links/media tests**: scheme allow-list (`javascript:`, `data:`, `file:`, `//host`, whitespace/case tricks), host disclosure, unknown-file error lists available files, name → URL resolution and re-resolution, placeholder on removal, no autoplay.
- **WebUI vitest**: catalogue schema ↔ registry coverage (every catalogue name has a component), every recipe validates, error messages list valid options, saved-state key filtering, logic runner timeout and size limits (jsdom iframe replaced by a fake runner interface for unit; real runner in Playwright), `sendMessage` trust and rate gating.
- **Playwright** (existing `e2e`): render a chess recipe and play two moves; render the terrain recipe and drag a slider; axe check on a rendered interface.
- **dummyplug end to end** (constitution gate): `scripts/client-tools-e2e.mjs` gains an `ends_turn` case: declare a tool with `ends_turn: true`, have a dummyplug agent run an `execute_python` script that calls it, assert the turn ends with no further model round, the reply carries the step's text, and that a failing call or a failing script keeps the turn open. Quickstart scripts the same as dummyplug steps.
- **Live model** (marked): US5 prompts (chess, terrain) and SC-003/SC-010 samples.
