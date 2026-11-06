# Implementation Plan: Workspace Channel

**Branch**: `053-workspace-channel` | **Date**: 2026-11-06 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/053-workspace-channel/spec.md`

## Summary

A Workspace is a shared space with people, agents and topics. Agents read along for free, and wake only on `@<agent-id>`. This replaces `consult_agent` and `delegate_agent` with `mention_agent` inside a Workspace, and `send_message_to_workspace`, `list_workspaces` and `read_workspace_messages` from anywhere. Separately, every WebUI chat begins drawing mid-turn text as messages.

**Approach**: As on Discord, each agent keeps its own record of the Workspace, one session per topic on a new `Workspace` channel, holding its own turns plus what was shown as messages: never another agent's tool calls. One new table holds only those shown messages, in the shared order, written inside the existing history save. Records fill lazily: an activated agent copies what it missed just before its turn, which is what makes silent reading free ([research R1 to R3](research.md)). Activation is an ordinary `Chat` request through `AgentsClient`. Skills and commands use the format `/<command> @<agent-id>` (R19).

## Technical Context

**Language/Version**: Rust (workspace edition, as existing); TypeScript with React 19 in `design/` and `webui/`

**Primary Dependencies**: Existing only: axum, tokio, serde, rusqlite/SQL layer, React Router, Zustand, Tailwind v4. No new crate or npm package

**Storage**: SQLite and PostgreSQL through the shared `SqlStorage` (new `WorkspaceStorage`, `0004_workspaces.sql`); files through the existing `BlobStore`. See [data-model.md](data-model.md)

**Testing**: `cargo test --workspace`, `cargo clippy --workspace`, conformance vectors for every new route, vitest for `trail.ts` and `workspaces.ts`, `npm run typecheck`; end-to-end: dummyplug steps in [quickstart.md](quickstart.md), scripted in `scripts/workspace-e2e.mjs`

**Target Platform**: The server (Linux, macOS, Docker) and the desktop app (macOS, Linux, Windows)

**Project Type**: Rust web service with an embedded WebUI and a desktop host

**Performance Goals**: A new entry reaches members within 1 second, with 20 members and 10 agents (SC-003). Posting costs one insert plus mentioned agents only. Stop all settles within 5 seconds (SC-005)

**Constraints**: Silent reading makes no model call and no queue entry (SC-001). A person who is not a member cannot tell a Workspace exists (SC-006). No new required external service

**Scale/Scope**: Hundreds of people and agents on an installation (SC-012), 42 requirements, 7 user stories, roughly 8 new design components, 1 new screen family

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Gate | Status | Notes |
|---|---|---|
| I. Lean by Default | Pass | No crate, flag or trait beyond `WorkspaceStorage`. Uses existing queue, journal, abort and blob store |
| II. Trait-based extensibility | Justified | A new `VizierChannelId` variant extends existing matches. See Complexity Tracking |
| III. Self-contained default | Pass | SQLite and local files by default. Nothing external |
| IV. Portability | Pass | No OS-specific code |
| V. Unified errors and observability | Pass | `crate::Result`, `tracing`. Tool refusals are data. Spans follow existing turns |
| Design before UI code | Pass | [mockup.html](mockup.html), reviewed by the owner. FigCraft was unreachable (v2.2.0) |
| Dummyplug end-to-end | Pass | [quickstart.md](quickstart.md). An additive `say` key is added to the protocol (R16) |
| Feature flag for optional capability | Pass, not applicable | Not optional, since it replaces the only agent coordination (R17) |
| Roles stay independent | Pass | Dispatch goes through `AgentsClient`, events through the bus. Gateway never reads agent state |
| Matching submodule branches | **To do first** | `webui`, `design`, `landing`, `dummyplug` need the branch: `just checkout 053-workspace-channel`. Submodules `webui` and `landing` hold uncommitted `package-lock.json` changes, so check those first |
| Commit order | Noted | design → webui → vizier |
| Migration tracking (#82) | Planned | New compatibility items listed in [data-model.md](data-model.md). A checklist entry is a task |

**Post-design re-check**: all gates hold. The one recorded risk is slug prefix checks (R8), covered by tests on the access paths.

## Project Structure

### Documentation (this feature)

```text
specs/053-workspace-channel/
├── spec.md
├── mockup.html          # Agreed layout
├── plan.md              # This file
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── http-api.md
│   ├── socket-frames.md
│   ├── agent-tools.md
│   └── turn-rendering.md
└── tasks.md             # /speckit-tasks
```

### Source Code

```text
crates/vizier-core/src/
├── workspaces/                  # New
│   ├── mod.rs                   # Post, after_save, dispatch, stop, startup sweep
│   ├── access.rs                # The only permission matrix
│   ├── catch_up.rs              # Lazy reads into an agent's session
│   ├── mentions.rs              # Pure parser
│   ├── commands.rs              # Pure parser for /<command> @<agent-id>
│   └── names.rs                 # Name rule
├── storage/workspace.rs         # WorkspaceStorage trait, new
├── storage/sqlite/workspace.rs  # init_workspace_schema + impl, new
├── storage/sqlite/history.rs    # Entry index inside save_session_history
├── storage/sql/postgres/migrations/0004_workspaces.sql
├── service/workspaces.rs        # Operations, DTOs, per-delivery checks, new
├── agents/tools/workspaces.rs   # The four tools, new
├── agents/tools/consult/        # Removed
├── agents/tools/mod.rs          # Registration, category, retired names
├── agents/agent/mod.rs          # catch_up at turn start
├── agents/agent/system_prompt/context.rs  # ## Workspace
├── agents/steps.rs              # after_save call
├── agents/process.rs            # No titling for Workspace
├── schema/session.rs            # Workspace variant
├── notifications/               # workspace_message: one row per message
├── transport/events.rs          # Workspace event arm
└── scheduler/dream/             # catch_up before a cycle

crates/vizier-http/src/
├── api/v1/workspaces.rs         # Routes, new
├── workspace_topic.rs           # WorkspaceConnection, new
└── embedded.rs                  # Dispatcher opens it for the desktop app

crates/vizier-http/tests/conformance/groups/workspaces.rs   # New
app/src-tauri/src/               # IPC workspace_open / close

design/src/components/           # See contracts/turn-rendering.md
webui/app/
├── lib/trail.ts                 # Blocks: mid-turn messages
├── lib/workspaces.ts            # Entries to turn runs, new
├── stores/workspaces.ts         # New
├── routes/ + screens/workspaces/  # Topic screen, members, new workspace, new
├── screens/chat/MessageList.tsx, Trail.tsx
└── screens/shell/ShellRail.tsx

dummyplug/src/protocol.mjs   # say key (the dummyplug submodule)
specs/008-dummyplug-provider/contracts/dummyplug-protocol.md   # Documents say
scripts/workspace-e2e.mjs        # New
docs/src/                        # New Workspace page, tools tables, usage, observability
CLAUDE.md                        # Architecture section
```

**Structure Decision**: Engine code in `vizier-core` under a new `workspaces/` module, with authorization in core rather than handlers, as the service layer requires. Routes in `vizier-http`. No new crate.

## Delivery Order

Each step leaves `develop` working, and later steps depend on earlier ones.

1. **Mid-turn messages** (US7). WebUI only, no dependency. This could ship first as its own PR.
2. **Core**: channel variant, storage, access, mention parser, service, routes, conformance.
3. **Activation**: entry index, catch-up, dispatch, outbox, stop, socket.
4. **Tools**: the four tools, then removal of `consult_agent` and `delegate_agent` (US2, US5, US6). Removal lands with its replacement, never before.
5. **WebUI**: design components, then rail, topic screen, members, notifications (US1, US3, US4).
6. **Finish**: usage, dreams, docs, e2e script, CLAUDE.md, #82.

## Risks

| Risk | Mitigation |
|---|---|
| Chains with no limit spend budget unattended | Accepted by the owner. Stop all is guarded against races (R7), and a limit can be added later |
| A prefix check misses the new slug and exposes a session | Reviewed list in R8. Conformance vectors with three fixture users |
| Timeline rows drift from history | Written in the same transaction (R3) |
| Sharing files between agent sessions assumes `save_session_file` accepts an existing id | Confirm early (R9). Failing that, keep the reference on the entry and resolve at read |
| Every message notifies, so chatty agents can fill the inbox | Accepted by the owner. Toasts already group into bursts, and muting can follow (R20) |
| Long unseen backlog | Capped at 500 with a pointer to `read_workspace_messages` (R2) |
| Filler text at full size and in other agents' context | Accepted. To be judged with a live provider (quickstart 13.7) |
| Non-members reach a Workspace through Discord or Telegram | Accepted for this version. A separate spec tightens it |

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| A new `VizierChannelId` variant extends exhaustive matches (Principle II) | A session must identify its channel, and the enum is the existing identity, as the `Project` variant was | A trait over channel identity would rewrite every stored key and the serialized form, for one addition |
| `save_session_history` has a branch on `Workspace` | A derived index must commit with the row, the same as `artifact_reference` | Writing from callers leaves at least three paths able to miss it (R3) |
