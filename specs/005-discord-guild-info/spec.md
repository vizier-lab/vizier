# Feature Specification: Discord Guild, Channel & Member Info Tools

**Feature Branch**: `005-discord-guild-info`

**Created**: 2026-09-13

**Status**: Draft

**Input**: User description: "i want to additional tool for agent to gain more information for discord. specifically regarding a server/guild, channel and group chat and its member"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Look up server (guild) details (Priority: P1)

An agent operating in a Discord server needs to answer questions or make decisions that depend on knowing details about the server itself (name, member count, owner, creation date, available channels/roles) rather than just the message it just received.

**Why this priority**: Guild-level context is the most requested and most broadly useful piece of missing information — without it the agent cannot reason about "which server am I in" or "what channels/roles exist here", which blocks almost every other Discord-aware behavior.

**Independent Test**: Can be fully tested by invoking the new guild-info tool with a known guild ID and verifying the agent receives accurate, current guild metadata (name, member count, channel list, role list) that it can reference in its reply.

**Acceptance Scenarios**:

1. **Given** the agent is handling a message from a Discord server it has access to, **When** it calls the guild-info tool with that server's ID, **Then** it receives the server's name, owner, member count, creation date, and a list of its channels and roles.
2. **Given** the agent calls the guild-info tool with a server ID it does not have access to (not a member, or lacks permission), **When** the tool runs, **Then** it returns a clear error indicating the server is inaccessible rather than partial or fabricated data.

---

### User Story 2 - Look up channel or group-chat details (Priority: P2)

An agent needs to know details about the specific channel or group chat it is operating in or referencing — its type (text, voice, thread, DM group), topic/description, and its parent category — to tailor its behavior or explain context back to a user.

**Why this priority**: Channel-level context is needed less universally than guild-level context but is essential for any behavior that differs by channel (e.g., respecting a channel's stated topic/rules, distinguishing a public channel from a private group chat).

**Independent Test**: Can be fully tested by invoking the new channel-info tool with a known channel ID and verifying the agent receives the channel's name, type, topic, and member list (for group chats/DMs) or permission-relevant metadata (for guild channels).

**Acceptance Scenarios**:

1. **Given** the agent is handling a message in a guild text channel, **When** it calls the channel-info tool with that channel's ID, **Then** it receives the channel name, type, topic/description, and parent category (if any).
2. **Given** the agent is handling a message in a group direct message (no guild), **When** it calls the channel-info tool with that channel's ID, **Then** it receives the group's name (if set), its type, and the list of participant members.
3. **Given** the agent calls the channel-info tool with a channel ID it cannot access, **When** the tool runs, **Then** it returns a clear error instead of partial or fabricated data.

---

### User Story 3 - Look up member details (Priority: P3)

An agent needs to know details about a specific member of a server, group chat, or conversation — display name, username, roles, join date, and status — to personalize responses or make role/permission-aware decisions.

**Why this priority**: Member-level detail is the most granular and situational of the three; it matters primarily once the agent already knows which guild/channel it's operating in, so it naturally builds on the other two capabilities.

**Independent Test**: Can be fully tested by invoking the new member-info tool with a known guild/channel ID and member ID, and verifying the agent receives accurate profile and role/membership data for that member.

**Acceptance Scenarios**:

1. **Given** the agent knows a guild ID and a member's user ID, **When** it calls the member-info tool, **Then** it receives that member's display name, username, roles, and join date for that guild.
2. **Given** the agent knows a group chat/DM channel ID, **When** it calls the member-info tool for a participant of that channel, **Then** it receives that participant's username and display name.
3. **Given** the agent calls the member-info tool for a user who is not a member of the specified guild/channel, **When** the tool runs, **Then** it returns a clear error indicating the member was not found rather than partial or fabricated data.

---

### Edge Cases

- What happens when the bot's Discord token/session is not currently connected or has been rate-limited by Discord when a lookup is attempted?
- How does the system handle a guild, channel, or member ID that is syntactically valid but does not exist (deleted, left, or never existed)?
- How does the system handle a very large guild (thousands of members/channels) — does the tool return a full list, a paginated/truncated summary, or a count only?
- What happens when the requesting agent is configured for a different platform (e.g., Telegram-only) and has no Discord connection at all?
- How does the system handle member lookups in a guild where the agent's bot account lacks the permission to view the full member list?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Agents MUST be able to retrieve guild (server) information — including name, owner, member count, creation date, and lists of its channels and roles — by supplying a guild ID.
- **FR-002**: Agents MUST be able to retrieve channel information — including name, type (text/voice/thread/category/group DM), topic/description, and parent category — by supplying a channel ID.
- **FR-003**: Agents MUST be able to retrieve group-chat/DM-specific information — including participant list and group name — when the supplied channel ID refers to a group direct message rather than a guild channel.
- **FR-004**: Agents MUST be able to retrieve member information — including display name, username, avatar reference, join date, and assigned roles — by supplying a guild ID (or group chat ID) and a member/user ID.
- **FR-005**: Each new information tool MUST be scoped to data the requesting agent's own Discord bot connection already has access to; it MUST NOT allow retrieval of data for guilds, channels, or members the agent's bot account cannot see.
- **FR-006**: Each new tool MUST return a clear, descriptive error (not partial or fabricated data) when the requested guild, channel, or member does not exist or is inaccessible to the agent.
- **FR-007**: Each new tool MUST behave consistently with the existing Discord tool set: it is only made available to agents configured with Discord access, following the same per-agent isolation used by current Discord tools.
- **FR-008**: When a guild, channel, or group chat has more members than can reasonably be returned in one response, the system MUST summarize (e.g., truncate the list with a total count) rather than fail or return unbounded data.

### Key Entities

- **Guild (Server) Info**: Represents a Discord server the agent's bot is a member of — name, owner, member count, creation date, and the channels and roles it contains.
- **Channel Info**: Represents a single channel or thread within a guild, or a standalone group/DM channel — name, type, topic/description, parent category (if any).
- **Group Chat Info**: A specialization of Channel Info for a group direct message without a guild — group name (if set) and its participants.
- **Member Info**: Represents a single Discord user in the context of a specific guild or group chat — display name, username, avatar reference, join date, and roles (guild context) or simply identity (group chat context).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: An agent can retrieve full guild, channel, or member information in a single tool call, with no follow-up call needed for the common case.
- **SC-002**: Information returned by the new tools reflects the live state of Discord (not stale/cached data) at the time of the call, within the latency of a single Discord API round trip.
- **SC-003**: 100% of lookups for inaccessible or nonexistent guilds/channels/members return a clear error rather than incorrect or partial data.
- **SC-004**: Agents configured without Discord access are unaffected — the new tools are simply absent from their toolset, with no behavior change to non-Discord agents.

## Assumptions

- The agent's existing Discord bot connection and token (already used by `discord_send_message` and related tools) is reused for these new lookups — no separate credential or connection is introduced.
- These tools are read-only: they retrieve information and never modify guild, channel, or member state (no kicking, banning, role editing, etc.).
- "Group chat" refers to a Discord group direct message (a DM with more than two participants and no guild); ordinary one-on-one DMs are treated as a degenerate case of Channel Info with a single other participant.
- Results reflect what the bot account can currently see via the Discord API (governed by Discord's own permission model); the feature does not introduce a new permission system on top of Discord's.
- Large-list truncation (FR-008) uses a reasonable default page/summary size consistent with typical Discord API pagination; exact limits are an implementation detail for the planning phase.
