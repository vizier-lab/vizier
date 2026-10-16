# Implementation Plan: Integrations Section

**Branch**: `016-integrations-section` | **Date**: 2026-10-16 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/016-integrations-section/spec.md`

## Summary

Discord/Telegram bots, the Brave Search addon and MCP servers move off `AgentConfig` into stored **integrations**. Each integration is defined once and attached to agents. Changing one never restarts an agent:

- **Agents.** Each agent's tools are split into a fixed `BaseTools` and a swappable `IntegrationTools` layer, composed into an `ArcSwap<VizierTools>`. A turn takes one snapshot. An integration change rebuilds only that layer, reusing MCP connections that didn't change.
- **Channel readers.** Readers leave the agent's process loop and become a process-wide `ChannelConnections` manager, reconciled against storage. A bot reconnects on its own, and an agent restart no longer touches it.
- **One kind trait.** Every kind (Discord, Telegram, Brave Search, MCP) implements a single `IntegrationKind` trait, which also drives the Addons catalogue.
- **Naming.** Tool names never carry an agent or integration prefix. MCP keeps `<server>_<tool>`. Addon tools are namespaced the same way (`brave_search_web`, `brave_search_news`, category `brave_search`).
- **WebUI.**
  - The sidebar gains an agent-scoped **Integrations** section (Channels, Addons as an app-store grid, MCP).
  - A **Misc** section takes Usage and Config; Config is the former Agent Config, now built-in tools only.
- **Permissions.** Two permissions (`all_agents:integrations:manage`, `owned_agents:integrations:manage`) replace `agents:mcp_config`.
- **No migration.** Upgrading doesn't carry the old per-agent settings over; operators re-enter them under Integrations (research R7).

## Technical Context

**Language/Version**: Rust 2024 edition (stable toolchain as in `Cargo.toml`). TypeScript 5 / React 19 for the WebUI.

**Primary Dependencies**:
- Existing crates only: `tokio` (broadcast, `JoinSet`), `arc-swap` (already present), `axum`, `rusqlite`, `serde_json`, `twilight` / `teloxide` (unchanged readers), `rmcp` (unchanged `McpServer`).
- WebUI: React Router v7, Zustand-style stores, Tailwind v4.
- **No new dependencies.**

**Storage**: Embedded SQLite. Two new tables, `integration` and `integration_attachment`, behind a new `IntegrationStorage` trait in `VizierStorageProvider`. Old `agent_config` rows keep the moved keys until the agent is next saved; they are ignored (no migration).

**Testing**: `cargo test` covers:
- storage round-trips;
- the access helper;
- each kind's `validate` / `redact` / `merge_secrets`;
- `compose` reusing unchanged `McpServer`s.

`cargo clippy` and `npm run typecheck` must also pass.

End-to-end: dummyplug agent steps in [quickstart.md](quickstart.md), as the constitution's e2e gate requires. Tools are observed through `list_tool_functions` on the next turn. Channel steps that need real bot tokens are marked.

**Target Platform**: Linux, macOS and Windows server binary, plus the Docker image. Static musl cross targets in `Cross.toml` are unaffected (no new native deps).

**Project Type**: Single Rust binary (web service plus channels) with an embedded React WebUI (`webui/`).

**Performance Goals**:
- An integration change is in effect for the next turn that starts after the `PUT` returns. The refresh is an in-memory rebuild, well under 100 ms excluding MCP connects, which run in the background.
- A channel change reconnects within 30 s (SC-002).

**Constraints**:
- **No restarts:** zero agent restarts and zero aborted turns from integration changes (SC-001).
- **One snapshot per turn:** a turn sees one consistent tool set.
- **Prompt cache:** the system prompt stays byte-identical between changes, so provider prompt caching keeps working.
- **Isolation:** connections are per agent even when a definition is shared.
- **Lean:** no new crates.

**Scale/Scope**:
- Tens of agents and tens of integrations per install.
- Three new WebUI pages plus sidebar and AgentForm changes.
- About 4 integration kinds; Addons has 1 kind today.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle / gate | Assessment |
|---|---|
| **I. Lean by Default** | No new crates (`arc-swap` is already used). One new trait (`IntegrationKind`) with four real implementations from day one, not a speculative abstraction. `readers.rs` is reused as the manager's internals, not duplicated. **Pass.** |
| **II. DRY via trait-based extensibility (NON-NEGOTIABLE)** | Kind-specific behaviour (tools, reader, validation, redaction, catalogue listing) lives behind `IntegrationKind`, registered in `IntegrationKinds::builtin()`. Tool build, reader start, API redaction and the Addons catalogue don't `match` on kind. The existing Discord/Telegram/Brave/MCP wiring in `VizierTools::new` moves into the kinds rather than gaining a parallel copy. **Pass.** |
| **III. Self-contained runtime** | SQLite tables only. Addon icons are embedded like catalogue logos. No startup network access. Brave and MCP stay opt-in. **Pass.** |
| **IV. Portability** | No platform-specific code. Readers and the MCP transport are unchanged. **Pass.** |
| **V. Unified errors and observability** | `crate::Result` throughout. A failed refresh keeps the previous snapshot and logs `warn!`. **Pass.** |
| Quality gates | clippy, test and typecheck; conventional commits, with the agent API change flagged `[**breaking**]`; dummyplug e2e (quickstart §2, §3, §5). **Pass.** |

**Post-design re-check (after Phase 1):** still passes. The design adds one trait and one manager, both with several real users, and no type-tag branching. Removing the `AgentConfig` fields shrinks the agent schema.

## Project Structure

### Documentation (this feature)

```text
specs/016-integrations-section/
├── plan.md              # This file
├── research.md          # Phase 0: decisions R1–R11
├── data-model.md        # Phase 1: tables, types, settings per kind, state transitions
├── quickstart.md        # Phase 1: build checks + dummyplug e2e script
├── contracts/
│   ├── http-api.md      # agent-scoped integrations API, kinds catalogue, changed agent endpoints
│   ├── runtime.md       # IntegrationChanged event, refresh/reconcile guarantees G1–G8
│   └── webui.md         # sidebar, routes, page behaviour, store
├── checklists/requirements.md
└── tasks.md             # Phase 2 (/speckit-tasks)
```

### Source Code (repository root)

```text
src/
├── integrations/                    # NEW
│   ├── mod.rs                       # IntegrationKinds registry, compose(), refresh helpers
│   ├── kind.rs                      # IntegrationKind trait, IntegrationFamily, KindListing, Contribution
│   ├── access.rs                    # integration_access(user, integration, agent) → Access (research R6)
│   └── kinds/{discord,telegram,brave_search,mcp}.rs   # one impl each; tool construction moved from agents/tools/mod.rs
├── schema/integration.rs            # NEW: Integration, IntegrationScope, IntegrationStatus, IntegrationChanged
├── storage/integration.rs           # NEW: IntegrationStorage trait
├── storage/sqlite/integration.rs    # NEW: impl + init_integration_schema
├── storage/{mod.rs,sqlite/mod.rs}   # add to VizierStorageProvider / forwarders / init_schema
├── storage/user.rs                  # AVAILABLE_PERMISSIONS: +2, −agents:mcp_config
├── schema/agent.rs                  # remove discord_token, telegram_token, tools.{discord,telegram,brave_search,mcp_servers}
├── schema/commands.rs               # AgentCommand::{RefreshIntegrations, IntegrationStatus}
├── transport.rs                     # IntegrationChanged broadcast (publish/subscribe)
├── dependencies.rs                  # IntegrationKinds + IntegrationHealth on deps; drop migrate_agent_tools
├── agents/tools/mod.rs              # VizierTools::new → BaseTools::new; compose(); integration tool wiring removed
├── agents/agent/mod.rs              # tools: ArcSwap<VizierTools>; load_full() per turn; refresh_integrations()
├── agents/mcp/mod.rs                # McpServer::status()
├── agents/process.rs                # no longer starts ChannelReaders
├── agents/{mod.rs,supervisor.rs}    # route RefreshIntegrations / IntegrationStatus; subscribe to IntegrationChanged
├── channels/connections.rs          # NEW: ChannelConnections (reconcile, health_for); wraps readers.rs supervise loop
├── channels/readers.rs              # reduced to the per-reader supervise loop used by connections.rs
├── channels/mod.rs                  # VizierChannels owns/starts ChannelConnections
└── channels/http/api/v1/
    ├── integrations.rs              # NEW: /integrations/{id}, /integrations/kinds(+icon)
    └── agents/integrations.rs       # NEW: /agents/{id}/integrations[...]; agents/mod.rs nests it and rejects moved fields (422)

assets/integrations/                 # NEW: embedded addon/channel icons (brave_search.svg, discord.svg, telegram.svg)

webui/app/
├── routes.ts                        # + integrations/{channels,addons,mcp}, settings→config redirect
├── layout.tsx                       # sidebar: Integrations + Misc sections; Usage/Config moved
├── hooks/integrationStore.tsx       # NEW
├── routes/integrations-channels.tsx # NEW
├── routes/integrations-addons.tsx   # NEW (app-store grid + details panel)
├── routes/integrations-mcp.tsx      # NEW
├── routes/agent-settings.tsx        # served at :agentId/config, titled "Config"
├── components/AgentForm.tsx         # remove Channel Tokens, Discord/Telegram toggles, Brave, MCP sections
└── components/RolesSection.tsx      # permission list update
```

**Structure decision**: integrations get their own top-level module, `src/integrations/`. They span storage, agents, channels and HTTP, and no existing module owns all of that. Per-kind tool *types* stay where they are (`agents/tools/discord`, `telegram`, `brave_search`). Only their construction moves into the kinds.

## Phasing (for /speckit-tasks)

1. **Foundation**:
   - schema types, `IntegrationStorage` and sqlite tables;
   - the `IntegrationKind` trait and the four kinds;
   - the access helper and permission constants.
2. **Live tools** (US1/US3 backend):
   - `BaseTools` / `compose` / `ArcSwap` on `VizierAgent`;
   - the `refresh_integrations` and `IntegrationChanged` plumbing;
   - `McpServer::status`.
3. **Channel connections** (US2 backend): the `ChannelConnections` manager; remove readers from `agent_process`; health plumbing.
4. **Agent API**: reject the moved fields with 422 (FR-021).
5. **HTTP API**: agent-scoped and integration-level endpoints, the kinds catalogue and icons.
6. **WebUI**:
   - sidebar sections and routes;
   - Channels, Addons (app store) and MCP pages;
   - Config trim;
   - the role editor.
7. **Polish**: quickstart run end to end on dummyplug; `CLAUDE.md` architecture notes (tools, channels, storage, permissions); PR notes on behaviour changes.
   - Behaviour changes: a token without the tools toggle now gets the tools; Brave tools renamed `web_search`/`news_search` → `brave_search_web`/`brave_search_news` (`[**breaking**]`). MCP tool names never change.

US1 (MCP) is the MVP slice: phases 1–2 plus the MCP endpoints and page.

## Complexity Tracking

No constitution violations to justify.
