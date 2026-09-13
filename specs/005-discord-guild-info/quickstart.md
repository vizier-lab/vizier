# Quickstart: Discord Guild, Channel & Member Info Tools

## Prerequisites

- An agent already configured with `tools.discord.enabled = true` and a valid `discord_token` (bot token), same as for `discord_send_message` today.
- The bot must actually be a member of the target guild for guild/channel/member lookups to succeed (Discord enforces this, not this feature).

## Try it

1. Start the dev server: `just dev` (or `just run`).
2. From the WebUI or a Discord message the agent receives, prompt the agent to look something up, e.g.:
   - "What server is this and how many members does it have?" → agent calls `discord_get_guild_info` with the guild ID from the current session.
   - "What's this channel's topic?" → agent calls `discord_get_channel_info` with the current channel ID.
   - "Who is user 123456789012345678 in this server?" → agent calls `discord_get_member_info` with the guild ID and that user ID.
3. Verify the tool call appears in the agent's tool-call log/trace and that the response text matches the live Discord state (cross-check against the Discord client itself).

## Verifying error handling

- Call `discord_get_guild_info` with a guild ID the bot is not in → expect a clear "not found / inaccessible" error surfaced back through the agent, not a crash or fabricated data (FR-006).
- Call `discord_get_member_info` with a valid guild ID but a `user_id` that never joined it → expect a "member not found" error.

## Manual verification checklist (per CLAUDE.md: sparse test suite, verify by running)

- [ ] `cargo build` succeeds
- [ ] `cargo clippy` clean
- [ ] `cargo test` passes (including any new unit tests for truncation/formatting logic)
- [ ] Running `just dev` with a real Discord bot token: all three tools return correct live data for a real guild/channel/member
- [ ] An agent with `tools.discord.enabled = false` (or no `discord_token`) does not see these tools at all (toolset introspection or simply confirming the model never has them available)
