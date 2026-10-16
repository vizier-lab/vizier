# Research: Integrations Section

**Feature**: `016-integrations-section` · **Spec**: [spec.md](spec.md) · **Date**: 2026-10-16

What the code does today, and the decisions this plan rests on. Every Technical Context unknown is resolved here.

## What exists today (the starting point)

- **Integration config lives on `AgentConfig`** (`src/schema/agent.rs`):
  - `discord_token` and `telegram_token`;
  - `tools.discord.enabled` and `tools.telegram.enabled`;
  - `tools.brave_search` (`enabled` plus `{api_key, safesearch}`);
  - `tools.mcp_servers: HashMap<String, McpClientConfig>`. `McpClientConfig` is `Local {command, args, env}` or `Http {uri}`; there are no headers.
- **The tools are built once per instance.** `VizierTools::new` (`src/agents/tools/mod.rs`) builds every tool when the instance starts:
  - Discord/Telegram tools from the token (`new_discord_tools`, `new_telegram_tools`), each holding its own HTTP client and not depending on the reader;
  - Brave from the per-agent key;
  - one `McpServer` per entry, warmed up in the background.

  `VizierTools.categories` is computed once "so the system prompt stays byte-identical across turns" (provider prompt caching).
- **Channel readers belong to one run of the agent.** `agent_process` (`src/agents/process.rs:110`) calls `ChannelReaders::start(agent_id, config, …)`, so the readers stop and start with the instance. Each reader has its own backoff supervisor and reports `ChannelHealth` through the agent's `ChannelHealthHandle` (#50).
- **Any change to the config form restarts the agent.** `AgentCommand::Update` rebuilds the whole agent and swaps it in, and the swap aborts running turns (#52). Saving a channel token therefore costs every in-flight turn.
- **Access checks** use `user_can_view_agent` / `user_can_edit_agent` (`api/v1/agents/mod.rs`): system role, or `all_agents:*`, or owner. `agents:mcp_config` is only listed in `AVAILABLE_PERMISSIONS` and the role editor; no handler enforces it.
- **`arc-swap` is already a dependency** (`Cargo.toml`).

## R1 — Where integrations are stored

**Decision**: two new SQLite tables behind a new `IntegrationStorage` trait, added to the `VizierStorageProvider` supertrait and forwarded by `VizierStorage`, like every other store:

- `integration`: one row per channel connection, addon configuration or MCP server, with its settings as JSON;
- `integration_attachment`: `(integration_id, agent_id)`.

Schema creation is split into `init_integration_schema` (the `init_task_run_schema` pattern) so unit tests can build it on an in-memory connection.

**Rationale**: integrations are no longer owned by one agent, so they can't stay inside the `agent_config` JSON blob. Settings stay JSON because each kind's shape differs and is validated by its kind (R4), not by columns.

**Alternatives considered**:
- Keep them on `AgentConfig` and have the UI fan out edits. That can't share one definition, and every edit would still be an agent update.
- One table per kind. That means four schemas and four storage APIs for the same lifecycle (Principle II).

## R2 — Applying a change without restarting the agent

**Decision**: split `VizierTools` into a fixed base and a swappable integration layer, and let each turn take one snapshot.

1. `VizierTools::new` keeps building everything that is not an integration, unchanged: memory, CORE, scheduler, skills, background jobs, fetch, HTTP client, TTS, STT, image, shell, webui messaging. This becomes `BaseTools`. It is the expensive part with side effects, such as the shell container, the indexer and STT/TTS clients.
2. A new `IntegrationTools` holds:
   - the native tools contributed by attached channel connections and addons;
   - the agent's `McpServer`s, keyed by integration name.
3. `ToolSet::compose(&BaseTools, Arc<IntegrationTools>) -> VizierTools` builds the `ToolRouter`, `sandbox_toolset` and `categories`. All three are cheap: clones of `Arc`s plus one category pass.
4. `VizierAgent` holds `tools: ArcSwap<VizierTools>` instead of `tools: VizierTools`. `chat`, `prompt`, `dream_chat` and the handover paths call `self.tools.load_full()` once at entry and pass that snapshot down. The `ExecutePython` / `ListToolFunctions` instances inside it hold that snapshot's router.
5. `VizierAgent::refresh_integrations()` re-reads the agent's attachments from storage, builds a new `IntegrationTools`, composes and `store`s it.
   - An `McpServer` whose `(name, config)` is unchanged is carried over by `Arc`, so a refresh caused by a different integration never drops or reconnects a working MCP connection.
   - A changed or removed server is dropped, and its process ends with the last snapshot holding it.

**Rationale**: this meets the spec's requirements without a restart.
- FR-016 (no restart, no abort) holds because no instance is rebuilt.
- FR-018 (one consistent set per turn) holds because a turn holds one `Arc` snapshot from start to end.
- FR-017 (next turn) holds because the swap happens before the next `load_full`.
- The prompt-cache property is kept: `categories` change only when the integration set actually changes, so the system prompt stays byte-identical between changes.
- Per-agent isolation is kept, because each agent builds its own `McpServer`.

**Alternatives considered**:
- `ArcSwap` inside `ToolRouter`, read per call. A turn could then see two different tool sets mid-turn, which breaks FR-018.
- Rebuilding the whole `VizierTools`. That recreates the shell (a Docker container) and STT/TTS clients on every integration edit.
- Restarting the agent "gracefully" by draining turns. That's still a restart, and it delays the change until the agent is idle.

## R3 — Channel readers outlive agent restarts

**Decision**: move channel readers out of `agent_process` into a process-wide `ChannelConnections` manager (`src/channels/connections.rs`), owned by `VizierChannels`.

- **Keying.** It keeps one supervised reader per *enabled, assigned* channel integration, keyed by integration id. Each one reuses the `supervise` loop and backoff from `channels/readers.rs`, which becomes the manager's internals.
- **Reconciling.** `reconcile()` compares storage with the running readers:
  - it starts what's missing;
  - it stops what's gone, disabled or unassigned;
  - it restarts a reader whose token or assigned agent changed.

  It runs at startup and on every channel `IntegrationChanged` event (R5).
- **Routing.** Readers send to the agent id they were started for, through the existing per-agent request channel. An agent that is restarting or failed already queues or rejects requests (`AgentUnavailable`), so a reader never needs the agent instance.
- **Health.** `ChannelHealth` is stored per integration in the manager. The agent `HealthCheck` reads the entries assigned to that agent, which keeps `AgentHealthStatus.channels` and `vizier agent ps` unchanged. The Channels page reads the same entries.
- **Fatal errors.** A `FatalChannelError` marks the connection `failed` until its integration is edited. Today the fix is "update the agent"; now it's "replace the token".

**Rationale**:
- FR-010: a bad token never takes the agent down.
- FR-017: a channel change touches only that bot.
- Edge case "restarting a connection does not drop the agent's in-flight reply on other channels": readers are independent tasks.
- A side benefit: an agent *config* update no longer drops and reconnects its bots.

**Alternatives considered**: keeping the readers per agent and reconciling them inside `agent_process`. That still ties the readers to the agent instance, so every agent update would reconnect every bot, and reassigning a bot would need two agents to coordinate.

## R4 — One trait for every integration kind (Principle II)

**Decision**: an `IntegrationKind` trait in `src/integrations/kind.rs`, with one implementation per kind, all registered in one constructor (`IntegrationKinds::builtin()`):
- `DiscordKind` and `TelegramKind` (family `channel`);
- `BraveSearchKind` (family `addon`);
- `McpKind` (family `mcp`).

```rust
trait IntegrationKind: Send + Sync {
    fn id(&self) -> &'static str;                 // "discord", "telegram", "brave_search", "mcp"
    fn family(&self) -> IntegrationFamily;        // Channel | Addon | Mcp
    fn listing(&self) -> KindListing;             // name, description, icon, settings fields (addons use it as the catalogue tile)
    fn validate(&self, settings: &Value) -> Result<()>;
    fn redact(&self, settings: &Value) -> Value;  // masks secrets for API responses
    fn merge_secrets(&self, old: &Value, new: Value) -> Value; // keep secrets the request left out
    fn contribute(&self, i: &Integration, agent: &AgentToolContext) -> Contribution; // native tools and/or one McpServer
    fn reader(&self, i: &Integration, agent_id: &str, deps: &VizierDependencies) -> Option<Reader>; // channels only
}
```

**Rationale**:
- The four call sites that care about kinds would otherwise each `match` on a kind tag: the tool build, the reader start, redaction, and the catalogue listing. The constitution forbids that.
- The Addons catalogue is `kinds.filter(family == Addon).map(listing)`, so FR-013 (future addons need no agent-form change) holds by construction: a new addon is a new `impl IntegrationKind` plus one registration line.

**Alternatives considered**:
- A `match` on an enum in each place: rejected by Principle II.
- Separate traits per family: three traits for one lifecycle, and the shared parts (validate, redact, secrets) would be duplicated.

## R5 — How a change reaches agents and readers

**Decision**: the HTTP handler does three things in order.
1. Write storage in one transaction: the integration row and attachments.
2. Compute the affected agents: attached before ∪ attached after.
3. Publish one `IntegrationChanged { integration_id, family, agents }` on a new `VizierTransport` broadcast channel.

There are two subscribers:
- **`ChannelConnections`**, for `family == Channel`, runs `reconcile()`.
- **`VizierAgents`** sends `AgentCommand::RefreshIntegrations { agent_id }` to each affected agent's supervisor. If the agent is running, the supervisor calls `refresh_integrations()` on the live instance. If it is failed, starting or retrying, nothing is needed, because the next build reads storage.

Every subscriber rebuilds from storage (level-triggered), never from the event's contents. A lost or repeated event is therefore harmless, and startup is just "reconcile everything".

**Rationale**: this reuses the transport bus pattern (`SessionEvent` broadcast) and the supervisor's command path. Building from storage makes refresh idempotent.

**Alternatives considered**:
- The handler calls agents directly. That couples HTTP to agent internals and bypasses the supervisor, which is the one place that knows whether an instance is running.
- Polling storage. That adds latency and needless work.

## R6 — Permissions and scope

**Decision**:
- **Permissions.** Add `all_agents:integrations:manage` and `owned_agents:integrations:manage` to `AVAILABLE_PERMISSIONS`, and remove `agents:mcp_config`.
- **Who may act.** One helper decides every case: `integration_access(user, integration, agent) -> Access { see, edit, attach }`.
  - All-scope managers can do everything.
  - Owned-scope users may *edit* integrations they created (`created_by == user`). They may *attach* their own and `shared` integrations, and only to agents they own.
  - Anyone who can view an agent may *see*, read-only, the integrations attached to it. Secrets are always redacted.
- **Shared or owned.** An integration's `scope` is fixed when it is created: `shared` if the creator held the all-scope permission, otherwise `owned`. It does not change later, even if the creator's role changes.
- **Agent scope.** Integrations endpoints live under `/agents/{agent_id}/integrations/...` (contract), so the agent check is the one already used, `user_can_view_agent`, plus the helper above.

**Rationale**: this matches spec FR-005, FR-005a and FR-005b and the agent-scoped view (FR-002). Fixing `scope` at creation avoids integrations silently becoming private or shared when someone's role changes.

## R7 — No migration

**Decision**: no data migration. On upgrade, the per-agent fields (`discord_token`, `telegram_token`, `tools.discord`, `tools.telegram`, `tools.brave_search`, `tools.mcp_servers`) are no longer read.

- Typed `AgentConfig` deserialisation ignores the unknown keys, as it already does (no `deny_unknown_fields`). The next save of the agent re-serialises without them, so they disappear on their own.
- Operators set their bots, Brave key and MCP servers up again under Integrations.
- Roles are not rewritten. The superadmin's seed includes the new permissions. Other roles get them from an administrator in the role editor. A stored role still listing `agents:mcp_config` is harmless: nothing checks that string.
- The existing `migrate_agent_tools` startup step only copies the legacy global `mcp_servers` into the per-agent field this feature removes. It is deleted, together with the `delete_global_config("mcp_servers"/"shell")` cleanup it does.

**Rationale**: the user's call: integrations are few and quick to re-enter, and a migration (merging, dedup, ownership inference, role rewriting) is a lot of one-off code for a one-time benefit (Principle I).

**Alternatives considered**: an automatic migration that merges identical MCP definitions and rewrites roles. It was designed, then dropped as not needed.

## R8 — Removing fields from `AgentConfig` and the agent API (FR-019, FR-021)

**Decision**: delete `discord_token`, `telegram_token`, `tools.discord`, `tools.telegram`, `tools.brave_search` and `tools.mcp_servers` from the Rust types.
- `POST`/`PUT /agents` reject a body that still carries any of them with `422`, naming the field and pointing to `/agents/{id}/integrations` (FR-021: never silently overwrite).
- `AgentConfig` deserialisation from storage ignores unknown keys, which it already does (no `deny_unknown_fields`), so rows written before the upgrade still load (R7).
- This is a breaking API change and is flagged `[**breaking**]` in the commit.

## R9 — Live status for MCP servers and addons

**Decision**:
- **MCP status.** Add `McpServer::status() -> McpStatus { state: Connecting | Connected | Unreachable, tools: Option<usize>, last_error }`, read from its existing `ServerState` without connecting. The per-agent page asks the agent through `AgentCommand::IntegrationStatus { agent_id }`. The running instance answers from its current snapshot; an agent that isn't running answers `unknown`.
- **Addon status.** It is configuration-derived: `incomplete` when a required setting is missing, otherwise `ready`. A `last_error` is set by the Brave tool on an auth failure (401/403) through a small shared `IntegrationHealth` map on `VizierDependencies` (a plain `RwLock<HashMap>`) and cleared on the next success.

**Rationale**: the page is agent-scoped (FR-002), so the status shown is exactly that agent's connection, which keeps isolation honest. No new dependency.

## R10 — WebUI

**Decision**:
- **Routes.** Add `:agentId/integrations/channels`, `:agentId/integrations/addons` and `:agentId/integrations/mcp`. Rename `:agentId/settings` to `:agentId/config` and redirect the old path. `:agentId/usage` stays.
- **Sidebar** (`layout.tsx`):
  - the agent list loses Usage and Agent Config;
  - a new "Integrations" `nav-section` with Channels, Addons and MCP, disabled with no agent (FR-002);
  - a new "Misc" `nav-section` with Usage and Config.
- **Pages.**
  - Channels: cards.
  - Addons: app-store tile grid plus a details panel, driven by `GET /integrations/kinds?family=addon`.
  - MCP: a table plus an edit panel.

  The layout follows the mockup canvas. One `integrationStore.tsx` (Zustand) holds the selected agent's integrations, the visible library, and statuses.
- **`AgentForm.tsx`** loses its Channel Tokens, Discord/Telegram toggles, Brave Search and MCP Servers sections, and is retitled "Config".
- **Role editor** (`RolesSection.tsx`) lists the two new permissions and drops "Configure Agent MCP".

## R11 — Testing approach

- **Unit tests** (`cargo test`):
  - storage round-trips and the access helper;
  - each kind's `validate` / `redact` / `merge_secrets`;
  - `compose` reusing unchanged `McpServer` `Arc`s.
- **End to end:** a dummyplug agent (constitution gate). In code mode the `tools` command lists only the four sandbox tools, so the checks run `list_tool_functions {"category": "brave_search"}` / `{"category": "<mcp key>"}` through dummyplug's JSON path to see which integration tools the agent has on its *next* turn. The quickstart scripts this.
- **Live:** only the Discord/Telegram connection steps need real tokens, and they are marked as such.

## R12 — Tool naming and namespacing

**Decision**:

- **No agent or integration-id prefixes, ever.** Tool names depend only on the tool, so a shared integration produces identical names on every agent. This also keeps prompts byte-identical across agents.
- **MCP:** unchanged. Tools are `<server name>_<tool>` and the category is the server name (`<name>_mcp` when it clashes with a built-in category, *or now with an addon id*). Because tool names come from the server name:
  - `integration.name` is **not** unique install-wide (drops `UNIQUE(kind, name)`);
  - instead, an agent may not have two attached MCP integrations with the same name. The attachment and create endpoints refuse with `409 name_in_use_on_agent`, naming the integration already attached.
- **Addons:** namespaced like MCP. Each addon tool is named `<addon id>_<tool>`, and every tool an addon contributes is listed under one category named after the addon id, described by the addon's `KindListing`.
  - Brave Search: `web_search` → `brave_search_web`, `news_search` → `brave_search_news`, category `web` → `brave_search`. The `web` category keeps fetch and the HTTP client.
  - **Mechanism:** `Contribution` carries `addon: Option<&'static str>`. The router's catalogue lists those native tools under that category instead of `VizierTool::category()`. The Brave tool types return `None` from `category()`, like the direct tools, and `every_tool_is_in_its_category` lists them under their addon.
  - A unit test asserts that every tool an addon kind contributes is named `<addon id>_…`.
  - Addon ids are reserved category names, so an MCP server named `brave_search` gets category `brave_search_mcp`. Its tools can't take `brave_search_web`, because native names are reserved first in `assign_names`.
- **One configuration per addon per agent:** an agent may have at most one configuration of a given addon kind attached, or two Brave keys would produce the same tool names. Attaching another configuration of the same kind replaces the first, in one transaction.
- **Channels:** Discord and Telegram tools keep their names (`discord_*`, `telegram_*`) and categories; they are already namespaced by platform. An agent may have only one connection per platform, so the tool names can't collide. Attaching a second Discord bot to an agent that has one is refused with `409 platform_in_use_on_agent`.
- **Breaking:** the Brave rename changes dispatch keys, like 014's renames. It is flagged `[**breaking**]` and listed in the PR notes. Skills or scripts that call `web_search` / `news_search` must be updated, because there is no alias (consistent with 014: retired names are not aliased).

**Alternatives considered**:
- Prefixing tools with the integration's id or the agent: unstable, unreadable, and it differs per agent.
- Keeping the bare `web_search`: the user asked for addon namespacing, and bare names would collide as more search addons arrive.
