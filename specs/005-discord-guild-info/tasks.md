# Tasks: Discord Guild, Channel & Member Info Tools

**Input**: Design documents from `/specs/005-discord-guild-info/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/discord-info-tools.md, quickstart.md

**Tests**: Not explicitly requested in spec.md (no TDD ask). One unit test task is included for the pure list-truncation helper introduced by this feature, per plan.md's Testing decision — it is a normal implementation task, not a red/green TDD gate.

**Organization**: Tasks are grouped by user story (P1 guild info → P2 channel info → P3 member info), matching spec.md's priority order. All work lands in two existing files: `src/agents/tools/discord/mod.rs` and `src/agents/tools/mod.rs` — this is a small, additive change to an established pattern (see `SendDiscordMessage`/`ReactDiscordMessage`/`GetDiscordMessage` already in that file).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

Single Rust project (existing `vizier` binary crate) — all paths are relative to the repository root.

---

## Phase 1: Setup

**Purpose**: Confirm a clean baseline before adding new code. No new dependencies, scaffolding, or config are needed (per plan.md — `serenity` 0.12.5 is already a dependency and already exposes every endpoint this feature needs).

- [X] T001 Confirm the working tree builds and lints clean before starting: run `cargo build` and `cargo clippy` from the repository root and resolve any pre-existing failures first (this feature's diff must not be blamed for unrelated breakage)

---

## Phase 2: Foundational

**Purpose**: N/A for this feature — there is no shared schema, auth layer, or cross-cutting infrastructure to stand up first. The existing `Arc<Http>` construction in `new_discord_tools()` (`src/agents/tools/discord/mod.rs:17-29`) and the existing `VizierTool` trait already provide everything all three user stories build on. Each user story phase below is self-contained and independently shippable — proceed directly to Phase 3.

**Checkpoint**: N/A — no blocking foundational work for this feature.

---

## Phase 3: User Story 1 - Look up server (guild) details (Priority: P1) 🎯 MVP

**Goal**: An agent can call `discord_get_guild_info(guild_id)` and receive the guild's name, owner, approximate member count, creation date, and its channel and role lists.

**Independent Test**: Call the new tool with a guild ID the configured bot is a member of and confirm all fields render correctly; call it with a guild ID the bot cannot see and confirm a clear error (no partial/fabricated data) comes back instead.

### Implementation for User Story 1

- [X] T002 [US1] Add `GetDiscordGuildInfoArgs` (`guild_id: u64`, `#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]`, matching the style of `GetDiscordMessageArgs`) in `src/agents/tools/discord/mod.rs`
- [X] T003 [US1] Add a private helper `fn format_truncated_list<T>(items: &[T], limit: usize, label: impl Fn(&T) -> String) -> String` in `src/agents/tools/discord/mod.rs` that joins up to `limit` formatted items and appends `"...and N more"` when `items.len() > limit` (research.md Decision 3 — cap at 50)
- [X] T004 [US1] Implement `pub struct GetDiscordGuildInfo { http: Arc<Http> }` and its `VizierTool` impl in `src/agents/tools/discord/mod.rs`: `name()` returns `"discord_get_guild_info"`, `description()` matches contracts/discord-info-tools.md, `call()` invokes `self.http.get_guild_with_counts(GuildId::new(args.guild_id))` then `self.http.get_channels(guild_id)`, formats the output string per contracts/discord-info-tools.md (name, owner_id, `guild_id.created_at()`, `approximate_member_count`, truncated channel list via T003, truncated role list from `PartialGuild.roles` via T003), and maps any `Http` error to `VizierError` via `throw_vizier_error` (FR-006)
- [X] T005 [US1] Extend `new_discord_tools()`'s signature and body in `src/agents/tools/discord/mod.rs` to also construct and return `GetDiscordGuildInfo { http: http.clone() }`, reusing the same `Arc<Http>` already built in that function
- [X] T006 [US1] Update the discord wiring block (`if agent_config.tools.discord.enabled { ... }`) in `src/agents/tools/mod.rs` (~line 456) to destructure the new tuple element and add `.tool(guild_info)` to `default_toolset`
- [X] T007 [US1] Add a `#[cfg(test)] mod tests` unit test in `src/agents/tools/discord/mod.rs` for `format_truncated_list`: verify a list under the limit renders unchanged and a list over the limit truncates with the correct `"...and N more"` count
- [X] T008 [US1] Manually verify per `quickstart.md`'s guild-lookup scenario against a real Discord bot token/guild: run `just dev`, prompt the agent to describe the current server, and confirm the response matches the live guild's name/member count/channels/roles; also verify the inaccessible-guild error path

**Checkpoint**: User Story 1 (guild info) is fully functional and independently testable/shippable as an MVP.

---

## Phase 4: User Story 2 - Look up channel or group-chat details (Priority: P2)

**Goal**: An agent can call `discord_get_channel_info(channel_id)` and receive channel name/type/topic/parent (guild channel) or participant info (1:1 DM) — per research.md Decision 1, true multi-user Group DMs are not reachable by a bot token and are out of scope.

**Independent Test**: Call the tool with a known guild text channel ID and confirm name/type/topic/parent render; call it with a 1:1 DM channel ID and confirm the participant's username/display name render; call it with an inaccessible channel ID and confirm a clear error.

### Implementation for User Story 2

- [X] T009 [US2] Add `GetDiscordChannelInfoArgs` (`channel_id: u64`) in `src/agents/tools/discord/mod.rs`, following the same derive/style as T002
- [X] T010 [US2] Implement `pub struct GetDiscordChannelInfo { http: Arc<Http> }` and its `VizierTool` impl in `src/agents/tools/discord/mod.rs`: `name()` returns `"discord_get_channel_info"`, `description()` matches contracts/discord-info-tools.md, `call()` invokes `self.http.get_channel(ChannelId::new(args.channel_id))` and matches the result: `Channel::Guild(gc)` → format name/kind/topic/parent_id/guild_id; `Channel::Private(pc)` → format as "Direct Message" plus `pc.recipient` username/display name (data-model.md Channel Info); any `Http` error maps to `VizierError` via `throw_vizier_error` (FR-006)
- [X] T011 [US2] Extend `new_discord_tools()` in `src/agents/tools/discord/mod.rs` to also construct and return `GetDiscordChannelInfo { http: http.clone() }`
- [X] T012 [US2] Update the discord wiring block in `src/agents/tools/mod.rs` to destructure the new tuple element and add `.tool(channel_info)` to `default_toolset`
- [X] T013 [US2] Manually verify per `quickstart.md`: ask the agent about the current guild channel's topic (confirm correct name/type/topic/parent) and, separately, about a 1:1 DM channel (confirm the participant is correctly identified); also verify the inaccessible-channel error path

**Checkpoint**: User Stories 1 AND 2 both work independently; channel lookups are available alongside guild lookups.

---

## Phase 5: User Story 3 - Look up member details (Priority: P3)

**Goal**: An agent can call `discord_get_member_info(guild_id, user_id)` and receive that member's display name, username, avatar reference, join date, and roles within that guild.

**Independent Test**: Call the tool with a known guild ID and a member's user ID and confirm display name/username/join date/roles render; call it with a user ID that is not a member of that guild and confirm a clear "not found" error.

### Implementation for User Story 3

- [X] T014 [US3] Add `GetDiscordMemberInfoArgs` (`guild_id: u64`, `user_id: u64`) in `src/agents/tools/discord/mod.rs`, following the same derive/style as T002
- [X] T015 [US3] Implement `pub struct GetDiscordMemberInfo { http: Arc<Http> }` and its `VizierTool` impl in `src/agents/tools/discord/mod.rs`: `name()` returns `"discord_get_member_info"`, `description()` matches contracts/discord-info-tools.md, `call()` invokes `self.http.get_member(GuildId::new(args.guild_id), UserId::new(args.user_id))`, formats `Member::display_name()`, `user.name`, `avatar`, `joined_at`, and `roles` (raw role IDs, per data-model.md) into the contract's output shape, and maps a 404/"not a member" `Http` error to a `VizierError` naming both IDs (FR-006, User Story 3 acceptance scenario 3)
- [X] T016 [US3] Extend `new_discord_tools()` in `src/agents/tools/discord/mod.rs` to also construct and return `GetDiscordMemberInfo { http: http.clone() }` (final tuple element)
- [X] T017 [US3] Update the discord wiring block in `src/agents/tools/mod.rs` to destructure the final tuple element and add `.tool(member_info)` to `default_toolset`
- [X] T018 [US3] Manually verify per `quickstart.md`: ask the agent for a known member's roles/join date (confirm correctness) and ask about a user ID that never joined the guild (confirm the clear not-found error)

**Checkpoint**: All three user stories are independently functional — guild, channel, and member lookups are all available to Discord-configured agents.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Final verification across all three tools together.

- [X] T019 [P] Run `cargo clippy` from the repository root and resolve any warnings introduced by T002-T017
- [X] T020 Run `cargo test` from the repository root and confirm the full suite passes, including the new truncation unit test (T007)
- [X] T021 [P] Re-check against plan.md's Constitution Check: confirm no global/static `Http` client was introduced (per-agent `Arc<Http>` only, Principle IV per-agent isolation) and confirm no `match`/`if` branching over a type tag was added outside the `Channel::Guild`/`Channel::Private` match required by the Discord API shape itself (Principle II)
- [X] T022 Confirm an agent with `tools.discord.enabled = false` (or no `discord_token`) does not have any of the three new tools available (per FR-007 / quickstart.md's last checklist item)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately.
- **Foundational (Phase 2)**: N/A — nothing blocks Phase 3.
- **User Story 1 (Phase 3)**: Depends only on Phase 1. No dependency on US2/US3.
- **User Story 2 (Phase 4)**: Depends only on Phase 1. Touches the same two files as US1 (`discord/mod.rs`, `tools/mod.rs`) but adds a distinct tool/struct/tuple-slot — implement after US1 to avoid tuple/merge churn, not because of a logical dependency.
- **User Story 3 (Phase 5)**: Same relationship — sequenced after US2 for the same file-locality reason, not a logical dependency.
- **Polish (Phase 6)**: Depends on whichever user stories were implemented (all three for full completion).

### Within Each User Story

- Args struct → tool struct/impl → tuple extension → toolset wiring → manual verification (matches T002→T008, T009→T013, T014→T018 ordering above).

### Parallel Opportunities

Limited by design: this is a small, single-file-pattern feature, and every implementation task within a story touches `src/agents/tools/discord/mod.rs` and/or `src/agents/tools/mod.rs` in sequence. The only true parallel opportunities are in Phase 6 (T019 and T021 are independent verification passes with no shared state) and, if desired, doing Phase 3/4/5's manual-verification tasks (T008/T013/T018) out of band from a different session once their preceding implementation tasks land.

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1 (T001).
2. Complete Phase 3 / User Story 1 (T002-T008) — this alone ships `discord_get_guild_info` as a usable, independently valuable tool.
3. **STOP and VALIDATE**: run T008's manual verification against a real guild.
4. Optionally stop here — guild info is the MVP per spec.md's priority ordering.

### Incremental Delivery

1. Setup → User Story 1 → validate → ship (MVP).
2. Add User Story 2 (channel info) → validate → ship.
3. Add User Story 3 (member info) → validate → ship.
4. Phase 6 polish once all three (or however many are in scope for a given release) are in.
