# feat: Integrations section (016)

Discord/Telegram bots, the Brave Search addon and MCP servers move off the agent config into
stored **integrations**, defined once and attached to agents from a new sidebar section,
Integrations › Channels / Addons / MCP. Changing one never restarts an agent.

## What changes

- **Storage.** `IntegrationStorage` with tables `integration` and `integration_attachment`.
  Per-agent rules are enforced in the write transaction (one bot per platform per agent, a token
  used once, one configuration per addon per agent, MCP names unique per agent).
- **One kind trait.** `IntegrationKind` (`src/integrations/`) with four implementations
  (Discord, Telegram, Brave Search, MCP). Tool build, readers, redaction and the Addons catalogue
  go through it.
- **Live tools.** An agent's tools are a fixed `BaseTools` plus a swappable integration layer,
  composed into an `ArcSwap<VizierTools>`. A turn uses one snapshot from start to end; an
  integration change swaps in a new one for the next turn and keeps MCP connections that didn't
  change.
- **Channel readers** are process-wide (`ChannelConnections`), reconciled against storage. A bot
  no longer reconnects when its agent is updated, and a bad token fails only that bot.
- **HTTP.** `/agents/{id}/integrations` (list, create, attach/detach) and `/integrations/{id}`
  (edit, delete), `/integrations/kinds` and kind icons. Secrets are always redacted.
- **WebUI.** Integrations pages (MCP table, Addons app-store grid, Channels cards); a Misc
  section with Usage and Config; Config (the former Agent Config) loses the moved sections;
  `/:agentId/settings` redirects to `/:agentId/config`; the role editor lists the new permissions.

## Breaking changes [**breaking**]

- `POST`/`PUT /agents` refuse `discord_token`, `telegram_token`, `tools.discord`,
  `tools.telegram`, `tools.brave_search` and `tools.mcp_servers` with
  `422 {"error":"moved_to_integrations","fields":[…]}`; `GET /agents/{id}` no longer returns them.
- Brave tools are renamed `web_search` → `brave_search_web` and `news_search` →
  `brave_search_news`, under category `brave_search`. There is no alias; skills or scripts calling
  the old names must be updated.
- The `agents:mcp_config` permission is removed, replaced by `all_agents:integrations:manage` and
  `owned_agents:integrations:manage`.
- **No upgrade migration.** Bots, Brave keys and MCP servers must be set up again under
  Integrations. The superadmin role gets the new permissions; other roles need them granted in
  the role editor.

## Behaviour changes

- A bot token whose Discord/Telegram tools were switched off now gets the tools: an attached
  connection always brings its tools.
- Updating an agent no longer reconnects its bots.

## Testing

- `cargo test` (storage rules, access, kinds' validate/redact/merge, MCP reuse in the integration
  layer, addon tool namespacing, reconcile planning), `npm run typecheck`.
- End-to-end on dummyplug agents: see `specs/016-integrations-section/pr-notes.md`.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
