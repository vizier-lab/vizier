# Implementation Plan: Code Mode by Default, Tool Categories, Simpler MCP Tool Names

**Branch**: `014-tool-categories-code-mode` | **Date**: 2026-10-14 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/014-tool-categories-code-mode/spec.md`

## Summary

Code mode becomes the only way an agent works. The `tools.python` switches and `ToolExposure` are deleted, and every agent is offered just `think`, `execute_python`, `list_tool_functions` and `describe_tool_function`. Existing agents switch on upgrade with no migration, because serde drops the old key.

To keep discovery cheap, every built-in tool declares a `ToolCategory` (13 variants) through a required `VizierTool::category()` method. MCP tools are categorised by their server key. The system prompt's `SANDBOX.md` lists the agent's categories plus short discovery steps, and stays byte-stable across turns. `list_tool_functions` gains an optional `category` parameter; leaving it out lists everything.

MCP tools are renamed from `mcp_<server>__<tool>` to `<server>_<tool>`, and `READ_CORE`/`WRITE_CORE` to lower case. A single router catalogue names, categorises and dispatches every tool. Native names keep priority, and collisions get predictable `_2`, `_3` suffixes. Old names are not supported: they are neither listed nor dispatched.

## Technical Context

**Language/Version**: Rust (edition 2024, as in `Cargo.toml`); TypeScript + React 19 for the WebUI

**Primary Dependencies**: existing only: `rig-core` (`ToolDefinition`), `rmcp` (MCP client), `monty` (sandbox), `serde`/`schemars`, `utoipa`. No new crates.

**Storage**: none new. The `AgentToolsConfig.python` field is removed, and serde ignores the stored key (research R1).

**Testing**: `cargo test` unit tests:
- every router tool has a category;
- catalogue naming and collision order;
- old names do not resolve;
- `list_tool_functions` category filtering and not-found;
- stable `sandbox_md` output.

End-to-end: dummyplug agent steps in `quickstart.md`, with the offline `fixtures/echo_mcp.py` stdio MCP server.

**Target Platform**: unchanged (Linux/macOS/Windows, the musl and cross targets in `Cross.toml`)

**Project Type**: single Rust binary with an embedded WebUI

**Performance Goals**: at least 80% less tool-definition text per turn for agents that used direct tools (SC-001). A category listing is under ¼ of the full listing for 50+ tools (SC-003). Building the catalogue adds no MCP round-trips beyond today's cached `tools()`.

**Constraints**:
- The system-prompt section must stay byte-identical across turns for an unchanged configuration, so it can be cached (FR-010b).
- Native tool names must never change because of MCP servers (FR-014).
- No startup migration.

**Scale/Scope**: about 55 built-in tools across 13 categories, plus any number of MCP servers per agent. The changes touch about 25 Rust files (mostly one `category()` line per tool) and 6 WebUI files.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Assessment |
|-----------|------------|
| **I. Lean by Default** | ✅ Net removal: `PythonSandboxConfig`, `ToolExposure`, `CODE_MODE_DIRECT_TOOLS`/`exposed_directly`, the "tools not callable" description variant, the sandbox-additive prompt, `PythonSandboxSection.tsx`. Additions: one enum, one trait method and one catalogue struct, all with real current uses. No operator-editable categories and no MCP description config (R5). |
| **II. DRY via Trait-Based Extensibility** | ✅ The category is declared by each tool through the existing `VizierTool` trait; there is no central `match` over tool names (R3). Naming, listing and dispatch share one `ToolRouter::catalogue()` instead of today's separate naming (`docs::named`) and dispatch (prefix parsing) (R4). |
| **III. Self-Contained Runtime** | ✅ No new dependency or service. The `python3` MCP fixture is quickstart-only and is not used by vizier at runtime. |
| **IV. Portability** | ✅ No OS-specific code. The fixture uses only the Python standard library. |
| **V. Unified Errors & Observability** | ✅ Listing "not found" and unreachable-server results are data, not errors. Script-side unknown categories raise a catchable `ValueError`. Dispatch errors keep `VizierError`, and logging stays on `tracing`. |
| **Gate: dummyplug e2e** | ✅ The quickstart scripts every story as dummyplug steps. The system-prompt check (§4) is marked as needing a live provider or the debug log, because dummyplug cannot echo the system prompt. |
| **Gate: conventional commits** | ✅ The MCP and CORE renames and the removed config are breaking. Commit as `feat!:` with `[**breaking**]`. |

**Result**: PASS. Nothing goes in Complexity Tracking.

**Post-design re-check**: PASS. The design adds no abstraction beyond the three listed, and moving `think` into the direct set (R2) removes a special case rather than adding one.

## Project Structure

### Documentation (this feature)

```text
specs/014-tool-categories-code-mode/
├── plan.md              # This file
├── research.md          # Phase 0: decisions R1–R10
├── data-model.md        # Phase 1: ToolCategory, CategorySummary, CatalogueEntry
├── quickstart.md        # Phase 1: dummyplug e2e script
├── contracts/
│   ├── system-prompt.md     # SANDBOX.md text and its stability rules
│   ├── tool-definitions.md  # direct tools, list/describe I/O, in-script helpers, naming
│   └── http-api-webui.md    # removed python config; WebUI and channel rendering
├── fixtures/
│   └── echo_mcp.py      # offline stdio MCP server for the quickstart
├── checklists/requirements.md
└── tasks.md             # Phase 2 (/speckit-tasks), not created here
```

### Source Code (repository root)

```text
src/
├── agents/
│   ├── tools/
│   │   ├── mod.rs              # ToolCategory enum; VizierTool::category(); VizierToolDyn forward;
│   │   │                       # ToolRouter::catalogue() + CatalogueEntry;
│   │   │                       # ToolRouter::call resolves via catalogue; VizierTools.categories;
│   │   │                       # ThinkTool → sandbox_toolset; delete ToolExposure & exposure gating;
│   │   │                       # DREAM_TOOL_NAMES: read_core/write_core
│   │   ├── workspace/mod.rs    # READ_CORE/WRITE_CORE → read_core/write_core
│   │   ├── <every tool>.rs     # one `fn category()` each (≈55 tools)
│   │   └── python/
│   │       ├── mod.rs          # execute_python description: code-mode text only, mentions categories
│   │       ├── docs_tools.rs   # list_tool_functions {category?}; describe adds category
│   │       └── bridge.rs       # catalogue from router; resolve via catalogue
│   ├── agent/
│   │   ├── mod.rs              # always insert sandbox_md(categories, timeout)
│   │   └── system_prompt/sandbox.rs  # single code-mode text + category list (contracts/system-prompt.md)
│   └── mod.rs                  # drop python validation
├── sandbox/
│   ├── docs.rs                 # ToolFunctionDoc gains `category`; naming moves to the router
│   ├── runtime.rs              # list_tools(category=…) filter + ValueError on unknown
│   └── mod.rs                  # SandboxBridge unchanged (catalogue entries now carry category)
├── schema/agent.rs, schema/mod.rs   # remove PythonSandboxConfig / AgentToolsConfig.python
├── channels/http/api/v1/agents/mod.rs  # remove python DTO fields + schema
├── scheduler/dream/prompts.rs  # WRITE_CORE → write_core
└── utils/mod.rs                # format_thinking: READ_CORE/WRITE_CORE arms → read_core/write_core; drop mcp_ arms

webui/app/
├── components/PythonSandboxSection.tsx   # delete
├── components/AgentForm.tsx              # remove python state + badge
├── routes/agent-settings.tsx             # remove sandbox section
├── routes/agent-core.tsx                 # write_core in copy
├── routes/chat.tsx                       # READ_CORE/WRITE_CORE labels → read_core/write_core; drop mcp_ branch
└── interfaces/types.ts                   # remove PythonSandboxConfig

CLAUDE.md                                 # Tools section: code mode always on, categories, MCP naming
```

**Structure Decision**: single-project layout, unchanged. The work sits in the existing tools, sandbox, system-prompt, schema and WebUI modules. No new modules apart from the spec's fixture.

## Implementation Order

The steps are ordered so the tree builds after each one:

1. **Categories**: add `ToolCategory`, `VizierTool::category()` and the per-tool declarations, plus a coverage test. No behaviour change yet.
2. **CORE rename**: tool names, DREAM_TOOL_NAMES, the dream prompt and the renderer labels.
3. **Catalogue**: `ToolRouter::catalogue()`, `<server>_<tool>` naming, and collision suffixes. Delete the `mcp_` prefix parsing. Rewire `RouterBridge`, `ToolRouter::call` and the docs tools. Add naming unit tests.
4. **Listing by category**: `list_tool_functions {category?}`, in-script `list_tools(category=)`, and `category` on `ToolFunctionDoc`.
5. **Code mode only**: delete `PythonSandboxConfig`/`ToolExposure`, move `think` into the direct set, and switch to the new `sandbox_md` with categories. Remove the HTTP DTO fields.
6. **WebUI**: remove the sandbox section and types, and update the CORE copy and chat labels.
7. **Docs**: update CLAUDE.md (Tools section and memory/MCP naming notes), then run the quickstart e2e.

## Complexity Tracking

No constitution violations.
