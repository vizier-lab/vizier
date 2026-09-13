# Tool Contracts: Discord Guild, Channel & Member Info

These are `VizierTool` contracts (the interface an agent's LLM sees and calls), following the exact convention already used by `discord_send_message` / `discord_react_message` / `discord_get_message_by_id` in `src/agents/tools/discord/mod.rs`. Each tool's `Input` is a `schemars::JsonSchema`-derived struct exposed to the model as the tool's parameters; `Output = String` is a formatted, human-readable summary.

## `discord_get_guild_info`

**Description**: "get information about a discord server (guild), including its name, owner, member count, and its channels and roles"

**Input**:
```rust
struct GetDiscordGuildInfoArgs {
    /// id of the target discord server (guild)
    guild_id: u64,
}
```

**Output** (`String`, example):
```text
Guild "Vizier Community" (id 123456789012345678)
Owner: user 987654321098765432
Created: 2021-03-15
Approximate members: 4231
Channels (12): #general, #support, #dev-chat, ... (showing 12 of 12)
Roles (5): @everyone, Moderator, Verified, Bot, Admin
```

**Errors**: guild not found or bot not a member → `VizierError` with a message naming the guild ID and the underlying Discord API error (FR-006). No partial output on error.

---

## `discord_get_channel_info`

**Description**: "get information about a discord channel or direct message, including its name/type, topic, and (for a direct message) its participant"

**Input**:
```rust
struct GetDiscordChannelInfoArgs {
    /// id of the target discord channel
    channel_id: u64,
}
```

**Output** (`String`, examples):

Guild channel:
```text
Channel "#dev-chat" (id 111222333444555666)
Type: Text
Guild: 123456789012345678
Parent category: 999888777666555444
Topic: development discussion
```

Direct message:
```text
Direct Message (id 222333444555666777)
Participant: alice (Alice W.)
```

**Errors**: channel not found or inaccessible → `VizierError` naming the channel ID (FR-006).

---

## `discord_get_member_info`

**Description**: "get information about a member of a discord server, including display name, username, join date, and roles"

**Input**:
```rust
struct GetDiscordMemberInfoArgs {
    /// id of the discord server (guild) the member belongs to
    guild_id: u64,
    /// id of the target member/user
    user_id: u64,
}
```

**Output** (`String`, example):
```text
Member alice (nick: "Ally") in guild 123456789012345678
Username: alice#0
Joined: 2022-07-01
Roles: Moderator, Verified
```

**Errors**: user is not a member of the given guild → `VizierError` naming both IDs (FR-006, User Story 3 scenario 3). Note per data-model.md, DM-participant identity is already surfaced via `discord_get_channel_info`'s Direct Message output — this tool is guild-scoped only, consistent with Discord's own API (there is no separate "member of a DM" endpoint).

---

## Registration contract

All three tools are constructed alongside the existing three in `new_discord_tools()` (`src/agents/tools/discord/mod.rs`), sharing the same `Arc<Http>`, and added to `default_toolset` in `src/agents/tools/mod.rs` inside the existing:

```rust
if agent_config.tools.discord.enabled {
    if let Some(token) = &agent_config.discord_token {
        // existing 3 tools + these 3 new ones
    }
}
```

No config schema change — availability continues to be gated purely by `agent_config.tools.discord.enabled` + presence of `discord_token`, per FR-007.
