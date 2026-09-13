# Phase 1 Data Model: Discord Guild, Channel & Member Info Tools

No new persisted entities — everything below is read directly from the Discord API per call (via `serenity`) and returned as a formatted string; nothing is written to `VizierStorage`. This document describes the *shape* of what each tool reads and formats, for reference during implementation and tasks.

## Guild (Server) Info

Sourced from `serenity::model::guild::PartialGuild` (via `Http::get_guild_with_counts`).

| Field | Source | Notes |
|---|---|---|
| Name | `PartialGuild.name` | |
| Owner | `PartialGuild.owner_id` | Rendered as the raw user ID; resolving to a display name would require an extra `Http::get_user` call — out of scope unless tasks phase decides it's cheap enough to include |
| Member count | `PartialGuild.approximate_member_count` | `Option<u64>`; render "unknown" if `None` |
| Creation date | `PartialGuild.id.created_at()` | Snowflake-derived timestamp, no API call needed |
| Channels | `Http::get_channels(guild_id)` → `Vec<GuildChannel>` | Capped to first 50 (Decision 3), with a trailing count if truncated |
| Roles | `PartialGuild.roles` (`HashMap<RoleId, Role>`) | Capped to first 50 (Decision 3), with a trailing count if truncated |

**Validation / error behavior** (FR-006): any `Http::get_guild_with_counts` / `get_channels` error (403 not-a-member, 404 unknown guild) is mapped to `VizierError` via `throw_vizier_error`, not partially rendered.

## Channel Info

Sourced from `serenity::model::channel::Channel` (via `Http::get_channel`), branching on variant.

### Guild channel (`Channel::Guild(GuildChannel)`)

| Field | Source |
|---|---|
| Name | `GuildChannel.name` |
| Type | `GuildChannel.kind` (`ChannelType`: Text/Voice/Category/News/Thread variants) |
| Topic | `GuildChannel.topic` (`Option<String>`) |
| Parent category | `GuildChannel.parent_id` (`Option<ChannelId>`) — rendered as the raw ID; resolving to the parent's name would require a second lookup, left as a raw ID reference for now |
| Guild | `GuildChannel.guild_id` |

### Direct message channel (`Channel::Private(PrivateChannel)`)

| Field | Source |
|---|---|
| Type | Always "Direct Message" |
| Participant | `PrivateChannel.recipient: User` — username + display name |

Per Research Decision 1, there is no third "group chat" variant reachable by a bot token — `Channel` only ever deserializes to `Guild` or `Private` for a bot client, so no additional match arm or error path is needed for a hypothetical group case.

**Validation / error behavior** (FR-006): `Http::get_channel` errors (403/404) mapped to `VizierError`.

## Member Info

Sourced from `serenity::model::guild::Member` (via `Http::get_member(guild_id, user_id)`) for the guild case, or from the `Channel::Private.recipient: User` already fetched by Channel Info for the DM case (spec User Story 3, scenario 2).

| Field | Source (guild member) | Source (DM participant) |
|---|---|---|
| Display name | `Member.display_name()` (nick or username) | `User.name` / `User.global_name` |
| Username | `Member.user.name` | `User.name` |
| Avatar reference | `Member.avatar` or `Member.user.avatar` (`Option<ImageHash>`) | `User.avatar` |
| Join date | `Member.joined_at` (`Option<Timestamp>`) | N/A — DMs have no "joined" concept |
| Roles | `Member.roles` (`Vec<RoleId>`), resolved against the guild's role list from Guild Info if a name is desired, otherwise rendered as raw role IDs | N/A |

**Validation / error behavior** (FR-006): `Http::get_member` 404 (not a member of that guild) mapped to `VizierError`, matching User Story 3 scenario 3.

## Relationships

```text
Guild Info ──contains──> Channel Info (guild channels)
Guild Info ──contains──> Member Info (via role IDs shared with guild roles)
Channel Info (Private) ──has one participant──> Member Info (DM case)
```

No entity is persisted; every arrow above is "one API call feeds into formatting another response," not a stored relationship.
