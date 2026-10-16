# Implementation Plan: Agent Self-Managed Integrations

**Branch**: `017-agent-self-integrations` | **Date**: 2026-10-16 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/017-agent-self-integrations/spec.md`

## Summary

Agents can manage their own MCP servers, addons and channel connections through five code-mode tools, gated by one **grant** per family. A grant can only be switched on by someone who manages that agent's integrations. Alongside this, integrations move to a strictly **per-agent** model:
- the creator-based `shared`/`owned` scope is removed;
- attach is removed;
- access is decided only by which agents a person manages;
- leftover multi-linked rows are left as they are and handled read-only.

Technical approach:
- Grants live in their own table and endpoint, not on `AgentConfig`, so changing one recomposes tools like any integration change and never restarts the agent (R1).
- All integration writes move into one `integrations::service`, used by the HTTP handlers and the agent tools alike (R2).
- `access.rs` is rewritten around "manages the owner agent" (R3).
- The tools are contributed by the integration layer, so they appear and disappear with grants (R5).
- Every change is recorded in `integration_event` (R6).

## Technical Context

**Language/Version**: Rust 2024 edition (stable); TypeScript 5 / React 19 (WebUI)

**Primary Dependencies**: existing only: axum, rusqlite (bundled), tokio, serde/schemars, utoipa; React Router v7, Tailwind v4. No new crates.

**Storage**: embedded SQLite. New tables `integration_grant` and `integration_event`, and a new nullable column `integration.creator`. No data migration (FR-023).

**Testing**: `cargo test` covers `access.rs` rules, grant/event storage round-trips, category presence and the `every_tool_is_in_its_category` table. `cargo clippy`. `npm run typecheck`. End-to-end: dummyplug agent steps in [quickstart.md](quickstart.md), with the Telegram section marked live.

**Target Platform**: the single vizier binary (Linux/macOS/Windows; musl builds unchanged)

**Project Type**: single Rust binary with an embedded web app (`src/` + `webui/`)

**Performance Goals**: a grant or integration change made by an agent is visible from its next turn. A tool call adds one storage read (the grants) before the existing write path.

**Constraints**:
- no agent restart and no aborted turn for any grant or integration change (FR-006, FR-017);
- secrets never leave redacted (FR-014);
- no new required external service.

**Scale/Scope**: about 4 integrations per agent and tens of agents; events grow by one row per change.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | Assessment |
|---|---|
| I. Lean by Default | **Pass.** Five generic tools instead of fifteen per-family ones (R5). No new crate. Grants are a 4-column table, not a config subsystem. `Requester` is reused as the actor type. The per-agent model *deletes* code (scope, attach, `SharedEditWarning`, the addon picker). |
| II. DRY via traits (non-negotiable) | **Pass.** The validate/merge/save/publish logic moves out of two HTTP handlers into one `service`, used by HTTP and the tools, which prevents the tools becoming a third copy (R2). Platform-specific behaviour (the self-lockout warning) goes through a new `IntegrationKind::platform()` default method, with no `match` on kind ids (R7). Access stays in `access.rs` only. |
| III. Self-contained runtime | **Pass.** SQLite only, and no network access added. |
| IV. Portability | **Pass.** No OS-specific code. A local MCP server's command already runs through the existing `McpServer` path. |
| V. Unified errors & observability | **Pass, with one note.** `ServiceError` is an internal enum mapped at both edges, in the same way `IntegrationConflict` already is. It is not a public error type, and storage keeps `anyhow` as it does today. Logging is `tracing` only. |
| Workflow: dummyplug e2e | **Pass.** quickstart.md scripts every story as dummyplug steps; Telegram is marked live. |
| Workflow: conventional commits | The change is breaking: attach removed, `scope` removed from API views, addon picker removed. Commit as `feat: … [**breaking**]`. |

**Post-design re-check**: still passes. The design adds no `match` over families or kinds outside `IntegrationFamily::as_str`/`parse`, which already exist.

## Project Structure

### Documentation (this feature)

```text
specs/017-agent-self-integrations/
├── plan.md              # this file
├── research.md          # R1–R8
├── data-model.md        # tables, trait changes, rules
├── quickstart.md        # dummyplug e2e script
├── contracts/
│   ├── http-api.md      # grants, events, changed 016 routes
│   ├── agent-tools.md   # the five tools
│   └── webui.md         # Config section, Integrations pages, Activity
├── checklists/requirements.md
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
src/
├── schema/
│   └── integration.rs             # drop IntegrationScope; creator: Option<Requester>; IntegrationGrant, IntegrationEvent, EventAction
├── integrations/
│   ├── access.rs                  # rewritten: owner agent, legacy, person + agent rules (R3)
│   ├── service.rs                 # NEW: Actor, ServiceError, create/update/delete/unlink/set_grant (R2)
│   ├── kind.rs                    # + platform() default method (R7)
│   ├── kinds/{discord,telegram}.rs# platform() overrides
│   └── mod.rs                     # IntegrationTools::build also takes grants → self-management tools
├── storage/
│   ├── integration.rs             # trait: EventDraft on writes, unlink, grants, events
│   └── sqlite/
│       ├── mod.rs                 # DDL: integration_grant, integration_event, ADD COLUMN creator
│       └── integration.rs         # impls; delete_agent_integrations per R4
├── agents/
│   ├── tools/
│   │   ├── mod.rs                 # ToolCategory::Integrations; compose picks up the self-management set
│   │   └── self_integrations.rs   # NEW: the five tools over integrations::service
│   └── agent/mod.rs               # refresh_integrations reads grants too
└── channels/http/api/v1/
    ├── integrations.rs            # handlers → service; IntegrationView without scope
    └── agents/
        ├── integrations.rs        # list (own only), create, unlink, DELETE, grants, events
        └── mod.rs                 # refuse `self_management` in agent bodies (422)

webui/app/
├── components/AgentForm.tsx       # Self-management section
├── components/integrations.tsx    # LegacyLinkedNotice, CreatorLine, ActivityTab; drop SharedEditWarning
├── routes/integrations-{mcp,addons,channels}.tsx
├── hooks/integrationStore.tsx     # drop attach; add grants/events
├── services/vizier.ts             # grants/events/delete calls
└── interfaces/types.ts
```

**Structure decision**: everything goes into existing modules. The only new files are `integrations/service.rs` (the shared write path) and `agents/tools/self_integrations.rs` (the tools).

## Complexity Tracking

No constitution violations to justify.
