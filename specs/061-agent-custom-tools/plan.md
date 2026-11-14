# Implementation Plan: Agent custom tools

**Branch**: `061-agent-custom-tools` | **Date**: 2026-11-15 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/061-agent-custom-tools/spec.md`

## Summary

An agent saves a Python script as a **custom tool** (name, description, category, input schema,
script) and calls it later from any script, found through the existing `list_tool_functions` /
`describe_tool_function`. A call runs the saved script through the same `run_script` and
`RouterBridge` an `execute_python` call uses, with the validated input bound to a global `input`,
inside the caller's turn, deadline and context. Custom tools live in four new SQL tables (tools,
versions, categories, events), are written through one `CustomTools` service used by the agent's
authoring tools, the dream cycle and the HTTP API alike, and appear in the prompt as one category
line per agent-owned category, fixed for the turn. The dream prompts gain a "reusable workflows"
step that turns repeated, fixed workflows into custom tools. The WebUI gets a per-agent Custom
tools page grouped by category, with history, diff, rollback, enable/disable, delete and an editor.

Before custom tools become the third per-turn catalogue source dispatched by `RouterBridge`
(after client tools and project tools), the two existing ones are collapsed behind one
`TurnToolSource` trait, as the constitution requires when a third near-identical path appears
(research R1).

## Technical Context

**Language/Version**: Rust 2024 (workspace toolchain); TypeScript 5 / React 19 for the WebUI and
design library

**Primary Dependencies**: existing only — `monty` (the sandbox), `similar` (diffs, via
`storage/diff.rs`), `axum` in `vizier-http`. Input validation is a small hand-written checker for
a documented JSON Schema subset, not a new crate (research R6). One new *direct* dependency already in the tree:
`ruff_python_parser`/`ruff_python_ast` 0.0.14 (pulled in by `monty`, same version), to find the
tools a script calls (research R4)

**Storage**: SQLite (`init_custom_tool_schema`) and PostgreSQL (migration
`0010_custom_tools.sql`); no blobs — scripts are small text rows

**Testing**: `cargo test -p vizier-core` (storage contract, validation, catalogue, cycle, dispatch,
prompt-stability tests), `vizier-http` conformance vectors for every new route, `npm run
typecheck` + vitest for the WebUI's pure helpers; end-to-end: dummyplug steps in
[quickstart.md](quickstart.md)

**Target Platform**: the server (Linux/macOS/Docker) and the desktop app (macOS/Linux/Windows);
nothing platform-specific

**Project Type**: Rust workspace (engine + HTTP + binary) with a React WebUI submodule and a
design-system submodule

**Performance Goals**: a custom tool call adds no model round and at most one indexed SQL read
(the current version) over an `execute_python` call of the same script; listing reads one query
per script run

**Constraints**: the system prompt stays byte-identical within a turn and across turns unless the
category set changes (SC-007, SC-010); no limits on count, size, depth or history (FR-015a) beyond
the sandbox's own existing 64 KiB script cap and the caller's time limit; gateways never read a
running agent's state (the #163 seam)

**Scale/Scope**: tens to low hundreds of custom tools per agent; one new built-in tool category
(`custom_tools`, the 22nd, appended); ~10 authoring tools; ~10 HTTP routes; one WebUI page

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Gate | Status | Notes |
|---|---|---|
| I. Lean by default | ✅ | No new crate beyond two already compiled in the tree; no feature flag (custom tools are core, not an optional channel/addon/provider). One service, one storage trait, one turn-source trait that removes code rather than adds a parallel path. |
| II. DRY via traits (NON-NEGOTIABLE) | ✅ with refactor | Custom tools would be the third per-turn catalogue source matched in `RouterBridge::dispatch_to` / `turn_catalogue` / `ToolRouter::catalogue` ranking. R1 collapses client and project tools behind `TurnToolSource` first; custom tools then register as an implementation, with no new `match` arm. Validation, versioning and diffs reuse `storage/diff.rs` and the revision value types. |
| III. Self-contained default | ✅ | SQLite + local, no network. PostgreSQL gets an ordered migration. |
| IV. Portability | ✅ | No OS calls; scripts run in the existing sandbox. |
| V. Unified errors & observability | ✅ | `crate::Result`, `tracing`; refusals are `status` data like the other agent tools; one `execute_tool custom_<name>` span per call with the nested calls as children. |
| Design before UI code | ⏳ required | The Custom tools page and its components are drawn (FigCraft, or a dummy `.html`/ASCII mockup if FigCraft is unreachable) and reviewed before any `design/` or `webui/` code. Tasks order this first. |
| dummyplug e2e | ✅ planned | [quickstart.md](quickstart.md) scripts create, list, describe, call, fail, update, diff, rollback, disable, categories and a dream save through dummyplug; dream *judgment* (whether a workflow becomes a tool) is marked live-provider. |
| Matching submodule branches | ⏳ | Run `just checkout 061-agent-custom-tools` before touching `design/`, `webui/` or `dummyplug/`; commit order design → webui → vizier. |
| Migration code → #82 | ✅ n/a | New tables only; nothing legacy. If PostgreSQL needs a compatibility step, add a #82 entry. |
| Fleet: roles independent / flags | ✅ | Engine work in `vizier-core`, routes in `vizier-http`; no new role or flag. |

**Post-design re-check (after Phase 1)**: still passes. The design adds `TurnToolSource` (replacing
two match arms), `CustomToolStorage` on `VizierStorageProvider`, `CustomTools` on
`VizierDependencies`, and a published view `agent.{id}.tool_names` so the gateway can validate a
person's edit without reading the running agent (research R8). No violation needs the Complexity
table.

## Project Structure

### Documentation (this feature)

```text
specs/061-agent-custom-tools/
├── spec.md
├── plan.md              # this file
├── research.md          # Phase 0
├── data-model.md        # Phase 1
├── quickstart.md        # Phase 1 (dummyplug e2e script)
├── contracts/
│   ├── agent-tools.md       # authoring tools + calling convention + discovery additions
│   ├── http-api.md          # /agents/{id}/custom-tools routes
│   ├── prompt-and-dream.md  # SANDBOX.md category lines, dream prompt changes
│   └── turn-tool-source.md  # the refactored per-turn catalogue seam
├── checklists/requirements.md
└── tasks.md             # Phase 2 (/speckit-tasks, not created here)
```

### Source Code (repository root)

```text
crates/vizier-core/src/
├── agents/tools/
│   ├── mod.rs                      # ToolCategory::CustomTools (22nd); ToolTarget::Turn; turn_catalogue /
│   │                               # turn_categories over TurnToolSource; reserved `custom_` prefix
│   ├── turn_source.rs              # NEW: TurnToolSource trait (client, project, custom implement it)
│   ├── custom_tools.rs             # NEW: the authoring tools (create/update/list/get/history/diff/
│   │                               # rollback/delete/set_enabled/update_category)
│   └── python/
│       ├── bridge.rs               # dispatch through TurnToolSource; nested call records (FR-028)
│       └── mod.rs                  # run_script gains an optional bound `input`
├── custom_tools/                   # NEW: the domain
│   ├── mod.rs                      # CustomTools service (one write path for agent, dream, person)
│   ├── validate.rs                 # schema + parse + calls + cycle checks, every problem at once
│   ├── schema.rs                   # the JSON Schema subset: check a schema, validate an input by path
│   ├── calls.rs                    # ruff-based "which functions does this script call"
│   ├── run.rs                      # call a custom tool: load current version, bind input, run_script
│   ├── catalogue.rs                # CustomTurnSource: entries + category lines for a turn
│   └── naming.rs                   # name rule, `custom_` prefix, category clash suffix
├── sandbox/runtime.rs              # execute_with_input: MontyRun input names / start inputs
├── sandbox/report.rs               # ToolInvocationRecord.calls (nested)
├── agents/views.rs                 # publish agent.{id}.tool_names
├── agents/agent/mod.rs             # snapshot custom categories into ToolContext at turn start
├── scheduler/dream/prompts.rs      # reusable workflows (extraction), tool-vs-skill rule, audit counts
├── service/custom_tools.rs         # NEW: HTTP-facing operations + authorization (can_view/can_edit_agent)
├── storage/custom_tool.rs          # NEW: CustomToolStorage trait + value types
├── storage/sqlite/custom_tool.rs   # NEW: SQL implementation + init_custom_tool_schema
└── storage/sql/postgres/migrations/0010_custom_tools.sql

crates/vizier-http/src/api/v1/agents/custom_tools.rs   # NEW routes
crates/vizier-http/tests/conformance/groups/custom_tools.rs

design/src/…                        # CustomToolGroup, CustomToolRow, version/diff pieces (+ stories), after the mockup
webui/app/
├── routes/custom-tools.tsx, custom-tool.tsx
├── screens/customTools/            # page, tool view, editor, category dialog
├── lib/customTools.ts (+ test)     # grouping, needs-attention, editor diff state
└── lib/agentDetails.ts             # phone details row

docs/src/configuration/custom-tools.md   # skills vs custom tools (FR-029)
```

**Structure Decision**: engine and domain in `vizier-core` (`custom_tools/` beside `skill/` and
`secrets/`), authorization in `core::service`, routes in `vizier-http`, UI in `design/` then
`webui/`, following the CLAUDE.md rule "new engine code in core, new route in http".

## Complexity Tracking

None. The one constitution pressure (a third `RouterBridge` match arm) is resolved by the R1
refactor rather than justified.
