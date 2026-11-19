# Implementation Plan: Consulting other agents (and removing Workspaces)

**Branch**: `064-consult-agents` | **Date**: 2026-11-20 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/064-consult-agents/spec.md`

## Summary

Remove Workspaces (spec 053) end to end, including their data. Agents collaborate through
**consultations** instead: `consult_agent` sends a request to a real, separate agent the person
may use, and the call returns a handle at once. The consultee works as itself in a conversation
of its own, and its answer comes back through the subagent machinery of spec 059: wait, stop,
asking back, delivery exactly once, survival across aborts and restarts, and a read-only peek
panel.

The technical approach is to **extend the subagent row, not copy it** (research D1). A
consultation is a `subagent` row with a `consultee_id`. Its turns run on the consultee under a
new `Consultation` channel (D2). Each side writes under its own agent's fence (D3), and the only
new transition is the follow-up, `answered → running` (D4). On top sit an access check in core
(the person's view right, D5), one appended prompt section (D6) and an eight-tool `consultations`
category (D7).

## Technical Context

**Language/Version**: Rust 2024 (workspace toolchain); TypeScript 5 + React 19 (WebUI, design)

**Primary Dependencies**: existing only (tokio, axum, rusqlite/sqlx, serde, schemars; React
Router 7, Zustand, Tailwind 4). No new crates or npm packages.

**Storage**: SQLite (default) and PostgreSQL. The `subagent` table gains three columns. Every
`workspace*` table is dropped, and Workspace sessions and notifications are deleted once.

**Testing**: `cargo test --workspace` (unit tests for transitions, fencing, access, framing and
retired hints; `tests/dummyplug_e2e.rs`; conformance vectors for the new routes). `vitest` for the
WebUI's consultation store and grouping. End to end: dummyplug steps in
[quickstart.md](quickstart.md), automated in `scripts/consultations-e2e.mjs`.

**Target Platform**: the server (Linux, macOS, Docker) and the desktop app (macOS, Linux,
Windows), all through the same core.

**Project Type**: a Rust workspace (engine library + HTTP crate + binary) with a WebUI submodule
and a design submodule.

**Performance Goals**: no new hot path. A consultation costs one guarded update per transition
and one access check per call (one user read and one agent-config read).

**Constraints**: two-sided fencing must never refuse under a scope the writing process does not
hold (spec 057). The consultee's prompt prefix must stay byte-identical to its other turns (the
section goes last). No change to the dummyplug protocol.

**Scale/Scope**: about 5.6k lines of Workspace code removed from core and HTTP, plus about 15
WebUI files. About 1.5k lines added, mostly tools, tests and UI.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design: still passes.*

| Gate | Verdict |
|---|---|
| **I. Lean by default** | Pass. The consultation adds no table, service or trait: it reuses the subagent row and engine. No new dependencies. Workspaces' removal is a net reduction of thousands of lines. |
| **II. DRY via traits** | Pass. One `Subagents` engine and one storage implementation serve both kinds. Delivery stays one `DeliverySource`. The new channel kind is selected the way existing ones are (`system_prompt_layout`, `usage_activity`, `kind_of`), with no new dispatch scattered elsewhere. The `ask_*` tools share one implementation. |
| **III. Self-contained default** | Pass. Nothing needs a new service. Consultations are in-process (or on the bus in a Fleet). |
| **IV. Portability** | Pass. There is no platform-specific code. |
| **V. Unified errors and observability** | Pass. Refusals are data, and failures are `crate::Result`. Consultation turns adopt the caller's trace through `VizierRequest.trace` (as worker turns do), so `invoke_agent {B}` nests under A's `execute_tool consult_agent`. |
| **Migration code tracked (#82)** | Required, so these are in the tasks: the `migration.drop_workspaces_v1` cleanup, the kept `VizierChannelId::Workspace` variant, and the five retired names get #82 entries. |
| **Design before UI code** | Required. The ASCII mockups in [contracts/http-and-webui.md](contracts/http-and-webui.md) are the fallback form. The FigCraft pass (Chat page strip and panel, the conversation list row, the rail without Workspaces) is the first UI task, before `design/` changes. |
| **Matching submodule branches** | Required: run `just checkout 064-consult-agents` before UI work, because the `speckit.git.feature` hook branched only the workspace. Commit order is design → webui → vizier. |
| **dummyplug e2e** | Pass. [quickstart.md](quickstart.md) scripts every story as dummyplug steps, and none needs a live model. |
| **Feature flags / roles** | Pass. No new optional capability. Consultations ride `AgentsClient`/the transport like any request, so a networked role needs nothing extra (D3). |
| **Verify what a change touches** | Pass. The CI paths are core, http, webui and design, which run their usual jobs. |

No violations, so Complexity Tracking is empty.

## Project Structure

### Documentation (this feature)

```text
specs/064-consult-agents/
├── spec.md
├── plan.md                       # this file
├── research.md                   # D1–D13
├── data-model.md                 # the extended subagent row, the channel kind, removals
├── quickstart.md                 # dummyplug checks
├── contracts/
│   ├── consultation-tools.md     # the eight tools, refusals, offering, framing text
│   └── http-and-webui.md         # routes, frames, read-only rule, ASCII mockups
├── checklists/requirements.md
└── tasks.md                      # /speckit-tasks
```

### Source Code (repository root)

```text
crates/vizier-core/src/
├── agents/
│   ├── subagents.rs              # Target::{Worker, Agent}; follow-up; kind-aware render_update; consultee deletion
│   ├── tools/
│   │   ├── consultations.rs      # NEW: the eight tools
│   │   ├── subagents.rs          # refuse c_ handles; ask tool shared
│   │   ├── workspaces.rs         # DELETED
│   │   └── mod.rs                # ToolCategory::Consultations (appended); RETIRED_TOOL_NAMES; is_interactive_turn
│   ├── agent/mod.rs              # system_prompt_layout: consultation_md; frontmatter via:/on_behalf_of
│   ├── agent/system_prompt/consultation.rs   # NEW: consultation_md
│   ├── process.rs                # turn_ended for Consultation sessions; Workspace arms removed
│   ├── steer.rs, steps.rs        # Workspace arms removed
│   └── mod.rs                    # AgentHost delete → stop_consultations_of(B)
├── consultations/                # NEW, small: access.rs (principal_for_turn, consultable)
├── schema/{session,subagent,request}.rs  # Consultation channel; columns; consultation_depth/chain
├── storage/{subagent.rs, sqlite/subagent.rs, sqlite/fence_tests.rs}  # columns, follow-up, two-sided fence
├── storage/sql/postgres/migrations/0012_drop_workspaces.sql, 0013_consultations.sql   # NEW
├── storage/sqlite/{mod,workspace}.rs     # cleanup once; workspace.rs DELETED
├── workspaces/, service/workspaces.rs, schema/workspace.rs, storage/workspace.rs      # DELETED
├── service/{subagents,conversations,topics,inbox,notifications}.rs  # kind filter; consultation rows; removals
├── notifications/, usage/, scheduler/dream/, storage/user.rs        # Workspace pieces removed
└── config/mod.rs                 # ConsultationsConfig { max_depth }

crates/vizier-http/src/
├── api/v1/agents/…/consultations.rs  # NEW: three routes (mirrors subagents)
├── api/v1/workspaces.rs, workspace_topic.rs   # DELETED
├── topic.rs                      # read_only_conversation on Consultation channels
└── tests/conformance/groups/{consultations.rs NEW, workspaces.rs DELETED}

design/src/components/            # SubagentWorker: agent slot; CreateChooser DELETED; Rail: Workspace entry removed
webui/app/
├── screens/chat/ConsultationsPanel.tsx   # NEW (shares grouping with SubagentsPanel)
├── lib/subagents.ts              # grouping made kind-generic; consultations split by `consultee`
├── stores/…                      # consultations in the chat store
├── api/endpoints/consultations.ts        # NEW
├── screens/…/conversation list   # consultation rows, read-only chat
└── routes/workspace.tsx, screens/workspaces/, stores/workspaces.ts, lib/workspace*.ts, api/endpoints/workspaces.ts   # DELETED

docs/src/configuration/{consultations.md NEW, workspaces.md DELETED}, docs/src/SUMMARY.md
scripts/{consultations-e2e.mjs NEW, workspace-e2e.mjs DELETED}
CLAUDE.md                         # Workspaces section → Consultations; category count; retired names
```

**Structure Decision**: the existing workspace layout. Engine code goes in `vizier-core`, routes
in `vizier-http`, components in `design/` (first), screens in `webui/`. Nothing goes in the
binary crate except the seed config passing through `VizierConfig`.

## Order of work

1. **Core engine** (no UI): the row columns, the follow-up, two-sided fencing, `Target`, the
   channel kind, framing, access, depth/chain, the tools, retired names. Unit tests and
   `dummyplug_e2e.rs` cases. This delivers Stories 1, 2, 3, 4 and 6 to agents.
2. **Workspace removal, core and HTTP**: the deletions, the once-only cleanup, the PostgreSQL
   migration, the conformance vectors, and the #82 entries (Story 8, server half). It is
   independent of step 1 except for `RETIRED_TOOL_NAMES`, so do it on the same branch after
   step 1.
3. **HTTP routes and frames** for consultations, plus the read-only rule (the backend for
   Stories 5 and 7).
4. **Design pass** (FigCraft, or the mockups as written), then `design/` components with stories.
5. **WebUI**: the Consultations panel, consultation rows, the read-only chat, the Workspace UI
   removal (Stories 5, 7 and 8, UI half).
6. **Docs, CLAUDE.md, scripts**, then the full quickstart run.
