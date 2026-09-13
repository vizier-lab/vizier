# Phase 0 Research: Discord Guild, Channel & Member Info Tools

## Decision 1: "Group chat" scope — bots cannot see Discord Group DMs

**Decision**: Descope true multi-user Group DM support. "Group chat/DM" in the spec's User Story 2 and User Story 3 is implemented as an ordinary 1:1 DM (`serenity::model::channel::PrivateChannel`), which the spec's own Assumptions section already treats as "a degenerate case of Channel Info with a single other participant."

**Rationale**: Discord removed bot-account access to Group DMs years ago — they are a user-account-only feature at the API level. Confirmed directly in the `serenity` 0.12.5 source used by this project:
- `serenity::model::channel::Channel` (`src/model/channel/mod.rs`) is a two-variant enum: `Guild(GuildChannel)` and `Private(PrivateChannel)`. There is no `Group` variant.
- `PrivateChannel` (`src/model/channel/private_channel.rs`) models exactly one `recipient: User` — structurally a 1:1 DM, not a multi-party one.
- `ChannelType::GroupDm = 3` still exists as an enum discriminant (for deserializing historical/user-API payloads) but no bot-reachable HTTP route in `serenity::http::Http` returns a channel of that shape populated with members.

There is no way for a bot token to enumerate or fetch a Group DM's participant list via the Discord Bot API, so FR-003 and the "group chat" acceptance scenarios (User Story 2 scenario 2, User Story 3 scenario 2) cannot be implemented as literally written.

**Alternatives considered**:
- *Use a user token instead of a bot token to access Group DMs*: rejected — this is a documented Discord Terms of Service violation ("self-botting") and is out of scope for a legitimate agent framework; it would also require a second, incompatible auth path alongside the existing bot-token-based `discord_token` config.
- *Silently drop Group DM scenarios with no notice*: rejected — recorded here explicitly so the spec/plan/tasks stay honest about what ships; flagged back to the requester in the plan Summary.

**Resulting scope**: `GetDiscordChannelInfo` handles both `Channel::Guild` and `Channel::Private` variants; a channel ID that resolves to neither (i.e., would have been a Group DM had the bot been able to see it) simply cannot occur for a bot-scoped client, so no special-case error path is needed for it.

## Decision 2: Which serenity `Http` endpoints to use

**Decision**:
- Guild info → `Http::get_guild_with_counts(guild_id)` (returns `PartialGuild` populated with `approximate_member_count`/`approximate_presence_count`) + `Http::get_channels(guild_id)` for the channel list + `Http::get_guild_roles(guild_id)` for the role list (roles are also present on `PartialGuild.roles` as a `HashMap<RoleId, Role>`, so a separate roles call is unnecessary — use the field already on the fetched `PartialGuild`).
- Channel info → `Http::get_channel(channel_id)`, matched on the returned `Channel::Guild(GuildChannel)` / `Channel::Private(PrivateChannel)` variant.
- Member info → `Http::get_member(guild_id, user_id)` for guild members; for the 1:1-DM case, member identity comes from the `PrivateChannel.recipient: User` already returned by the channel-info lookup (no separate "member of a DM" endpoint exists on Discord's API — a DM has exactly one other recipient).

**Rationale**: These are the same `Http`-level, single-request calls the three existing tools already use (`channel.message(...)`, `message.react(...)`) — no gateway cache, no new client type, one round trip per tool call (matches SC-002).

**Alternatives considered**:
- *`Http::get_guild(guild_id)` without counts*: rejected — omits `approximate_member_count`, which FR-001 explicitly requires.
- *Maintaining a local gateway cache (`serenity::cache`) of guilds/members*: rejected by Principle I (Lean by Default) — none of the three existing Discord tools run a gateway shard/cache today; adding one only for these read tools would be a large new subsystem for a feature that Discord's REST API already serves directly.

## Decision 3: Large member list truncation (FR-008)

**Decision**: Cap the member list returned by `GetDiscordMemberInfo`'s guild-wide listing behavior... — **not needed**: per the spec's Key Entities and User Story 3 acceptance scenarios, the member-info tool takes a single member ID and returns that one member's data; it does not enumerate a guild's full member list. The only "list" surfaces are:
- Guild info's channel list and role list (from Decision 2) — typically small (tens, not thousands) even in large guilds.
- No tool in this feature returns a raw member roster.

Given that, truncate the guild-info tool's channel list and role list to the first 50 entries each (Discord's own UI and API defaults commonly use limits in this range) with a trailing `"...and N more"` summary when exceeded, satisfying FR-008 without needing member-list pagination at all.

**Rationale**: Matches the spec's actual shape (member lookups are single-ID, not roster dumps) while still giving FR-008 a concrete, bounded behavior for the one place unbounded output could realistically occur (a guild with hundreds of channels/roles).

**Alternatives considered**: Paginating via `after`/`limit` params exposed to the agent as tool arguments — rejected as unnecessary complexity (Principle I) until an actual need for paging through channels/roles is demonstrated; a flat cap with a count is enough for "which channels/roles exist here" style questions.

## Decision 4: Output shape

**Decision**: Return a formatted human-readable `String` (not structured JSON), matching every existing tool in this module (`SendDiscordMessage`, `ReactDiscordMessage`, `GetDiscordMessage` all return `Output = String`).

**Rationale**: Consistency with the established `VizierTool` convention in this codebase — tool output is model-facing text, not an API response the caller parses.

**Alternatives considered**: A structured `serde`-serializable output type — rejected; no other tool in `src/agents/tools/` does this for Discord, and introducing one now would be an unjustified divergence (Principle I/II).
