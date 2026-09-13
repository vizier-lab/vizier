# Implementation Plan: Discord Guild, Channel & Member Info Tools

**Branch**: `005-discord-guild-info` | **Date**: 2026-09-13 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/005-discord-guild-info/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command. See `.specify/templates/plan-template.md` for the execution workflow.

## Summary

Add three new read-only `VizierTool` implementations to the existing `src/agents/tools/discord/` module — guild info, channel info, and member info — so a Discord-connected agent can look up server, channel, and member metadata on demand via the same `serenity::Http` client already used by `SendDiscordMessage`/`ReactDiscordMessage`/`GetDiscordMessage`. **Scope adjustment from spec**: Discord's bot API (and the `serenity` crate's `Channel` model) has no Group DM support — bots have not been able to see or join multi-user group DMs since Discord restricted that surface to user accounts. "Group chat" in this plan therefore means a 1:1 DM (`serenity::model::channel::PrivateChannel`), and the spec's group-DM-specific scenarios are descoped accordingly (see Research, Decision 1).

## Technical Context

**Language/Version**: Rust 1.85+ (edition 2024, per `Cargo.toml`)

**Primary Dependencies**: `serenity` 0.12.5 (already a dependency; `Http::get_guild_with_counts`, `get_channels`, `get_channel`, `get_guild_roles`, `get_guild_members`, `get_member`), `schemars`/`serde` (tool I/O schema, existing pattern), `async-trait` (existing `VizierTool` trait), `tracing` (logging)

**Storage**: N/A — these tools are read-only passthroughs to the Discord API; no new persistence

**Testing**: `cargo test` — unit tests around pure formatting/truncation logic (no live Discord API in CI, consistent with the rest of the sparse existing test suite)

**Target Platform**: Same as the rest of the binary (Linux/macOS/Windows server, embedded single binary)

**Project Type**: Single Rust project — additive change inside the existing `src/agents/tools/discord/` module

**Performance Goals**: One Discord API round trip per tool call (per SC-002); no polling or caching layer

**Constraints**: Read-only (no mutating Discord calls); per-agent isolation (reuse the `Arc<Http>` already built per-agent in `new_discord_tools`, no global/shared client); large member lists truncated with a total count rather than returned unbounded (FR-008)

**Scale/Scope**: 3 new `VizierTool` structs + arg types added to one existing file (`src/agents/tools/discord/mod.rs`), wired into `default_toolset` under the existing `agent_config.tools.discord.enabled` gate in `src/agents/tools/mod.rs`

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. Lean by Default**: PASS. No new crate — `serenity`'s `Http` client already exposes every guild/channel/member read endpoint needed. Reuses the `Arc<Http>` already constructed in `new_discord_tools`; no new abstraction layer, just three more tool structs following the exact shape of the three that already exist in the same file.
- **II. DRY via Trait-Based Extensibility**: PASS. New capabilities are added as new `VizierTool` impls (like `GetDiscordMessage`), not as branches in existing dispatch code. Registration is additive `.tool(...)` chaining in `VizierTools::new()`, inside the pre-existing `if agent_config.tools.discord.enabled` block — no new `match`/`if` over a type tag.
- **III. Self-Contained, Zero-Dependency Runtime**: PASS. No new required external service. Feature is only active when an agent already has Discord configured (opt-in, pre-existing `discord_token` config), same as today's Discord tools.
- **IV. Portability by Default**: PASS. No OS-specific code; pure API calls through the existing cross-platform `serenity`/`reqwest` stack.
- **V. Unified Errors & Observability**: PASS. All fallible paths return `crate::Result`/`VizierError` via `throw_vizier_error` or `.map_err(...)`, consistent with the three existing Discord tools in this file. No `println!`.

No violations — Complexity Tracking table not needed.

## Project Structure

### Documentation (this feature)

```text
specs/005-discord-guild-info/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
│   └── discord-info-tools.md
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
# Single project (existing Rust workspace) — additive change only

src/agents/tools/discord/
└── mod.rs                # ADD: GetDiscordGuildInfo, GetDiscordChannelInfo,
                           #      GetDiscordMemberInfo (structs, Args, VizierTool impls);
                           #      extend new_discord_tools()'s return tuple

src/agents/tools/mod.rs   # MODIFY: wire the 3 new tools into `default_toolset`
                           #         inside the existing
                           #         `if agent_config.tools.discord.enabled` block
```

**Structure Decision**: Single project, no new module. All three tools live in the existing `src/agents/tools/discord/mod.rs` alongside `SendDiscordMessage`/`ReactDiscordMessage`/`GetDiscordMessage`, following that file's established pattern (one `pub struct` + `Args` + `VizierTool` impl per tool, sharing the module's single `Arc<Http>`). `src/agents/tools/mod.rs` gets a small additive edit to register the new tools in the same `if agent_config.tools.discord.enabled { ... }` block that already wires up the other three.

## Complexity Tracking

*No Constitution Check violations — table not needed.*
