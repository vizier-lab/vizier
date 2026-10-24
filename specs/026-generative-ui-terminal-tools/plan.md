# Implementation Plan: Generative UI and Turn-Ending Tools

**Branch**: `026-generative-ui-terminal-tools` | **Date**: 2026-10-24 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/026-generative-ui-terminal-tools/spec.md`, plus the planning brief: define the predefined components and how the agent discovers them, teach the agent without skills, and keep the system flexible enough for an interactive chess board and a low-poly 3D terrain generator.

## Summary

Replace the hand-written-HTML `render_interactive_ui` with a **generative UI** client tool, `ui_render`, whose input is a json-render spec built only from a **catalogue of ready-made WebUI components**, and add a general **`ends_turn`** flag that makes a successful call to a tool the turn's final answer. `ui_render` is the first such tool.

Five decisions carry the plan (details in [research.md](./research.md)):

1. **The turn ends from the script's report** (R1). All tools are called from `execute_python`, so the agent loop never sees the flagged call. The `RouterBridge` marks each invocation record with `ends_turn`; the loop ends the turn when an `execute_python` round has `ok` and a successful flagged call. Stateless, so a resumed turn ends the same way. The notice is added once, where a catalogue entry's definition is built.
2. **The agent learns the catalogue from the tools, never from a skill or the system prompt** (R5). A `ui` category line, then `list_tool_functions`, then `describe_tool_function("ui_render")` (a ≈1.8 KB cheat sheet), then an on-demand `ui_components` tool (index → per-component detail → complete recipes), then errors that list every problem with valid options. The text is generated from the same definitions that validate and draw, so it cannot drift.
3. **Links and media build on what exists** (R12): a scheme-restricted `Link`, and `Image`/`Audio`/`Video`/`FileCard`/`Gallery` that refer to the conversation's **session files by name**, resolved to the existing `/files/{id}` route and shown with the existing players and preview. No new endpoint or storage; no external addresses.
4. **Flexibility comes in three tiers, not a bigger catalogue** (R6): declarative components with `repeat`/`$cond`/`$computed` (enough for a chess board's look), surfaces (`Canvas2D`, `Scene3D` with a `Heightfield` and `Mesh`), and **sandboxed pure logic functions** the interface calls locally (chess rules, terrain noise). Logic runs in an opaque-origin, no-network iframe with a hard time limit (R9); `sendMessage` only follows a trusted person gesture.
5. **json-render core and react are the foundation** (R3); 3D uses `three` and `@react-three/fiber` directly with a small component subset in a lazy chunk (R8). Validation happens in the WebUI tool handler, where the zod catalogue lives (R4).

## Technical Context

**Language/Version**: Rust (edition per `Cargo.toml`, tokio multi-thread); TypeScript 5.9 / React 19 / React Router 7 (SPA) / Tailwind v4 in `webui/`

**Primary Dependencies**: Rust: none new (reuse `serde`, existing sandbox/client modules). WebUI: `@json-render/core` and `@json-render/react` (0.21.3, Apache-2.0, pinned exactly), `zod` (^4, json-render peer), `three` and `@react-three/fiber` (MIT, lazy chunk). `@vizier/design` gains layout primitives, `Slider` and `Chart` (design first, per the repo's commit order).

**Storage**: SQLite unchanged, no migration. A drawn interface is a saved client tool call (`client_tool_call`: `args` = the spec, saved state = page state). `VizierResponseStats` gains `ended_by_tool` (serde default).

**Testing**: `cargo test` (flag, notice, report detection, resume detection, declaration check); `cd webui && npm test` (catalogue ↔ registry coverage, every recipe validates, error messages, state-key filtering, trust and rate gating, runner protocol with a fake runner); Playwright (chess and terrain recipes, axe); end-to-end: dummyplug steps in [quickstart.md](./quickstart.md) §1–§3a, scripted in `scripts/client-tools-e2e.mjs` (new `ends_turn` case); live-model steps (§3b) marked.

**Target Platform**: the single `vizier` binary serving the embedded WebUI build; modern evergreen browsers (WebGL 2 for 3D; the 3D module degrades to an inline "3D is not available" note without it).

**Project Type**: web application inside a single-binary Rust service (Rust backend + `webui` submodule + `design` submodule).

**Performance Goals**: interface appears < 1 s after the call is answered; local interactions < 100 ms (SC-005, SC-009); logic call ≤ 250 ms hard; 3D at 60 fps for a 48×48 heightfield on integrated graphics; turn-ending saves one model round (SC-006).

**Constraints**: Constitution III (no CDN, no network at runtime; 3D chunk is part of the embedded build); `build.rs` already builds the WebUI; no change to the dummyplug protocol; tool description cap 2048 B and schema cap 16 KB stay (the design fits under both); the system prompt stays byte-identical across turns.

**Scale/Scope**: ≈40 catalogue components, 6 recipes, ≤1500 elements per interface; Rust change is about 250 lines in six files; WebUI change is a new `lib/generativeUi/` module plus one screen component and design-library additions.

## Constitution Check

*GATE: passes before research; re-checked after design below.*

| Principle | Assessment |
|---|---|
| **I. Lean by default** | New dependencies are justified one by one: `@json-render/core`+`react` replace ~2k lines of expression/repeat/binding/validation code we would otherwise own (R3); `three`+`@react-three/fiber` serve a stated requirement (3D terrain), are one lazy chunk, and the heavier official 3D package was *rejected* (R8). No new Rust crate. No new abstraction for a single call site: one trait default method (`ends_turn`), one report field, one stats field. See Complexity Tracking for the two non-trivial additions (logic sandbox, `three`). |
| **II. DRY via traits** | `ends_turn` is a default method on the existing `VizierTool` trait and a field on the existing declaration; **no branch on tool name or kind** anywhere. The notice is applied at the two existing places catalogue definitions are built (the router's `catalogue()` and `ClientTools::resolve`), via one helper. In the WebUI the catalogue is one source for validation, rendering, schema enum and agent reference (R5, FR-003a). New components are added by one catalogue entry plus one registry entry. |
| **III. Self-contained runtime** | PASS. Static assets only; the 3D chunk and logic runner ship in `webui/build/client/` embedded in the binary; no fetch of libraries at runtime, no CDN, no new service. Interfaces cannot use the network. |
| **IV. Portability** | PASS. No OS/arch code; no new native Rust dependency, so `Cross.toml` targets are unaffected. |
| **V. Unified errors & observability** | PASS. Rust paths return `crate::Result`; logging via `tracing`; new span attributes `vizier.tool.ends_turn` and `vizier.turn.ended_by_tool` (contract §5), no content captured outside `capture_content()`. |
| **Workflow gates** | `cargo clippy`/`cargo test` and `npm run typecheck` are in the task list; the dummyplug e2e gate is scripted (quickstart §1–§3a) and live-model steps are marked; the dummyplug protocol is not changed (the `tools` listing shows the notice through the normal definition, no protocol edit). Conventional commits; the removal of `render_interactive_ui` is `[**breaking**]`. Per CLAUDE.md, tracked-for-removal code (the legacy renderer kept for saved pages) is added to issue #82. |

**Post-design re-check**: no new violations. One watch item: the logic runner is the only place agent-written code executes on the person's device; its isolation is the same mechanism spec 019 already relies on (opaque-origin sandboxed iframe, CSP without network) and is specified and tested in [contracts/logic-sandbox.md](./contracts/logic-sandbox.md).

## Project Structure

### Documentation (this feature)

```text
specs/026-generative-ui-terminal-tools/
├── plan.md                         # this file
├── research.md                     # R1–R11 decisions
├── data-model.md                   # flag, report/stats fields, interface shape, state rules
├── quickstart.md                   # dummyplug e2e + WebUI + live-model checks
├── contracts/
│   ├── ends-turn-flag.md           # declaration field, notice, semantics, observability
│   ├── generative-ui-tools.md      # discovery path, ui_components, ui_render, errors, rendering
│   ├── ui-catalogue.md             # components, events, actions, functions, limits, recipes
│   └── logic-sandbox.md            # logic functions: isolation, limits, lifecycle, trust
├── checklists/requirements.md
└── tasks.md                        # /speckit-tasks (not created here)
```

### Source Code (repository root)

```text
src/
├── agents/
│   ├── tools/
│   │   ├── mod.rs                       # VizierTool::ends_turn(), VizierToolDyn, CatalogueEntry.ends_turn,
│   │   │                                #   ENDS_TURN_NOTICE + one helper applied in ToolRouter::catalogue()
│   │   └── python/
│   │       ├── mod.rs                   # execute_python copies ends_turn into the report records
│   │       └── bridge.rs                # RouterBridge sets ToolInvocationRecord.ends_turn from the entry
│   ├── client/
│   │   ├── declare.rs                   # ClientToolDeclaration.ends_turn + `ends_turn_type` rule
│   │   └── mod.rs                       # resolve(): notice + CatalogueEntry.ends_turn; echo in resolved frame
│   └── agent/mod.rs                     # prompt_with: end the turn on a turn-ending round; chat_turn: flush all,
│                                        #   resume detection; VizierResponseStats.ended_by_tool
├── sandbox/report.rs                    # ToolInvocationRecord.ends_turn
└── schema/                              # VizierResponseStats.ended_by_tool (serde default)
scripts/client-tools-e2e.mjs             # `ends_turn` case (quickstart §1)
docs/src/api-integration/client-tools.md # field, notice, semantics; document ui_render pointers
templates/skills/interactive-ui/         # removed (replaced by ui_components); CLAUDE.md + docs updated

design/                                  # (submodule; committed first)
└── src/components/                      # Stack/Row/Grid, Slider, Chart (line/area/pie) + stories

webui/                                   # (submodule; committed second)
├── package.json                         # + @json-render/core, @json-render/react, zod, three, @react-three/fiber
└── app/
    ├── lib/
    │   ├── clientTools.ts               # registration gains `declare?: boolean`; legacy tool draws but is not declared
    │   ├── clientTools/
    │   │   ├── builtin.tsx              # register ui category, ui_components, ui_render (ends_turn, not deferrable);
    │   │   │                            #   legacy render_interactive_ui kept render-only
    │   │   └── interactiveUi.ts         # legacy description removed (kept only if still imported by tests)
    │   └── generativeUi/
    │       ├── catalog.ts               # zod catalogue (descriptions, examples, groups) — the single source
    │       ├── registry.tsx             # catalogue → @vizier/design components
    │       ├── media.tsx                # Link, Image/Audio/Video/FileCard/Gallery; file name → /files/{id}
    │       ├── surfaces/                # Canvas2D.tsx; Scene3D.tsx + shapes (lazy chunk)
    │       ├── functions.ts             # $computed functions
    │       ├── actions.ts               # sendMessage (trust + rate), run, state saving filter (_ keys)
    │       ├── logic/                   # runner.html builder, host.ts (queue, timeout, recreate)
    │       ├── validate.ts              # zod + validateSpec/autoFixSpec → model-facing error text
    │       ├── reference.ts             # index / detail / recipe generation from the catalogue
    │       ├── recipes/                 # dashboard, form, tabs_and_filter, board_game, scene_3d, canvas_2d (.ts specs)
    │       └── *.test.ts
    ├── lib/groupHistory (or frames.ts)  # a reply with empty text and a visible client call still renders its bubble
    └── screens/chat/client/
        ├── GenerativeUi.tsx             # Renderer + StateProvider + ActionProvider for one call
        └── InteractivePage.tsx          # unchanged (legacy saved pages)
```

**Structure Decision**: web application inside the existing single binary. The Rust change is a small, generic mechanism (`ends_turn`) and lives where tools, client tools and the loop already live; everything specific to generative UI is a WebUI client tool (spec 019's extension mechanism), so no Rust code knows what a component is. The `design/` → `webui/` → `vizier` commit order from CLAUDE.md applies.

## Phases

### Phase A — Turn-ending flag (Rust, independent of the UI; ships and tests alone)
1. `VizierTool::ends_turn` (+ dyn forwarding), `CatalogueEntry.ends_turn`, notice helper applied in `ToolRouter::catalogue()`; unit tests (flag false everywhere today; one test tool true).
2. `ClientToolDeclaration.ends_turn` and its `ends_turn_type` rule; `ClientTools::resolve` applies the notice and sets the entry flag; echo in `client_tools_resolved`; size check ignores the notice.
3. `ToolInvocationRecord.ends_turn` set by `RouterBridge`; `execute_python` unchanged otherwise.
4. `prompt_with`: after a round, detect a turn-ending report, record steps, skip the closing completion, return `ended_by_tool`; `chat_turn`: flush every new message in that case; `VizierResponseStats.ended_by_tool`.
5. Resume: `chat_turn` finalises a resumed turn whose last recorded round was turn-ending.
6. Span attributes; docs (`client-tools.md`); `scripts/client-tools-e2e.mjs` `ends_turn` case.

### Phase B — Generative UI core (WebUI + design)
1. `design/`: layout primitives, `Slider`, `Chart`; stories.
2. `lib/generativeUi/`: catalogue (Layout, Text, Data, Input, Action; links and media follow in B2), registry, `$computed` functions, state store with `_` filtering and the 019 save/sync path, `validate.ts` with model-facing errors, `GenerativeUi.tsx`.
3. `reference.ts` + `ui_components` and the `ui_render` description/schema (byte-cap test); recipes `dashboard`, `form`, `tabs_and_filter`; the empty-text reply bubble fix.
4. Register `ui` category, `ui_components`, `ui_render` (`ends_turn`, not deferrable); make `render_interactive_ui` render-only; remove the skill template; retired-name hint; update CLAUDE.md and docs.

### Phase B2 — Links and media
1. `Link` (scheme allow-list, host disclosure, markdown links in `Text`).
2. `media.tsx`: file-name resolution against the topic file list, `Image`/`Audio`/`Video`/`FileCard`/`Gallery` on the existing design players and `FilePreviewDialog`; inline placeholder; submit-time unknown-file error.
3. `Canvas2D` `image` shape; `media` group in `ui_components`; tests (R11).

### Phase C — Flexibility
1. Logic runner (iframe, CSP, queue, timeout, recreate), `run` action, `watch` guard and chain-depth cap, `Effect` (`load`, `tick`, visibility and idle stops), trust-gated `sendMessage`.
2. `Canvas2D`; `Scene3D` and shapes, `Heightfield`, `Mesh` as a lazy chunk with a no-WebGL fallback.
3. Recipes `board_game`, `scene_3d`, `canvas_2d` (validated in CI); Playwright chess and terrain scenarios; axe.

### Phase D — Verify against the spec
Quickstart §1–§3a on dummyplug, §3b with a live model and **no skill installed** (SC-009, SC-010), size/time comparison against the old tool (SC-001, SC-002), `cargo clippy`/`cargo test`/`npm run typecheck`, a note in #82 for the legacy renderer.

## Complexity Tracking

| Addition | Why needed | Simpler alternative rejected because |
|---|---|---|
| **Logic sandbox** (iframe runner, queue, timeout) | The request asks for interactive chess and a terrain generator. Rules and noise are not expressible as props or `$computed`, and a round trip to the agent per move is slow and costly. | *Only `sendMessage`*: every move costs a model turn (seconds, tokens). *More `$computed` functions*: a rules engine per game is unbounded. *Reuse the legacy HTML iframe for everything*: that is the slow path this feature removes. The runner is ~150 lines and reuses the 019 isolation mechanism. |
| **`three` + `@react-three/fiber` dependency** | A stated example needs real 3D with orbit controls and flat-shaded meshes. | *The official json-render 3D package*: pulls drei and postprocessing, more weight for components we do not need (R8). *Canvas2D pseudo-3D*: cannot orbit, poor fidelity. Lazy chunk limits the cost to interfaces that use it. |
| **Second tool `ui_components`** | Teaching without a skill requires on-demand reference; the description (2048 B) and schema (16 KB) caps cannot hold the catalogue, and the system prompt must stay byte-stable and small. | *Catalogue in the system prompt*: paid on every turn of every conversation. *Skill*: ruled out by the request. *Everything in `ui_render`'s description*: does not fit. |
| **`ends_turn` carried in the execution report** | Code mode hides tool calls from the loop; a side channel is lost on resume. | *Tool-name checks in the loop*: violates Constitution II. *A `ToolContext` flag*: not durable across restart. |
