# Data Model: Integrations Section

**Feature**: `016-integrations-section` · **Spec**: [spec.md](spec.md) · **Research**: [research.md](research.md)

## 1. `integration` (SQLite)

One row per channel connection, addon configuration or MCP server.

| Column | Type | Notes |
|---|---|---|
| `id` | TEXT PK | UUID v4 |
| `kind` | TEXT NOT NULL | An `IntegrationKind::id()`: `discord`, `telegram`, `brave_search`, `mcp` |
| `name` | TEXT NOT NULL | Display name. For `mcp` it is also the server key in tool names (`<name>_<tool>`), so it must match `^[a-z0-9][a-z0-9_-]{0,47}$` |
| `settings` | TEXT NOT NULL | JSON, shape set by the kind (§4), secrets in clear like `provider_config` |
| `enabled` | INTEGER NOT NULL | 1/0. A disabled integration contributes nothing and has no reader |
| `scope` | TEXT NOT NULL | `shared` or `owned`; fixed at creation (research R6) |
| `created_by` | TEXT NULL | user id; NULL only if the creating user was deleted |
| `created_at` / `updated_at` | TEXT NOT NULL | RFC 3339 |

Constraints:
- `name` is **not** unique install-wide. Two MCP integrations may share a name, and tool names come only from it (research R12). Per-agent rules, enforced in `IntegrationStorage::set_attachment` / create inside the transaction:
  - an agent may not have two attached `mcp` integrations with the same `name`;
  - an agent may have at most one attached integration of each addon kind (attaching another replaces it);
  - an agent may have at most one attached connection per channel platform.
- Channel tokens are unique across integrations of the same kind (FR-009). This is checked in `IntegrationStorage::save` inside the write transaction, by comparing the stored token with the token being saved; an index can't express this over a JSON field.

## 2. `integration_attachment` (SQLite)

| Column | Type | Notes |
|---|---|---|
| `integration_id` | TEXT NOT NULL | FK → `integration.id` `ON DELETE CASCADE` |
| `agent_id` | TEXT NOT NULL | the agent's id (no FK: `agent_config` rows are deleted through `AgentStorage`) |
| `created_at` | TEXT NOT NULL | |

- The primary key is `(integration_id, agent_id)`, plus an index on `agent_id`.
- **One agent per channel:** an integration of the `channel` family may have at most one attachment. Attaching to a second agent within one transaction is a *move*: delete the old row, insert the new one. A channel connection with no attachment is *unassigned*.
- `AgentStorage::delete_agent` also deletes the agent's attachments (edge case: agent deleted).

## 3. Rust types (`src/schema/integration.rs`)

```rust
pub enum IntegrationFamily { Channel, Addon, Mcp }
pub enum IntegrationScope  { Shared, Owned }

pub struct Integration {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub settings: serde_json::Value,
    pub enabled: bool,
    pub scope: IntegrationScope,
    pub created_by: Option<String>,
    pub agents: Vec<AgentId>,          // filled from integration_attachment
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub enum IntegrationStatus {
    Disabled,
    Unassigned,                                           // channel with no agent
    Incomplete { missing: Vec<String> },                  // addon missing a required setting
    Connecting,
    Running,                                              // channel reader up / MCP connected / addon ready
    Reconnecting { last_error: String, next_retry_at: DateTime<Utc> },
    Failed { last_error: String },                        // fatal channel error, or addon auth error
    Unreachable { last_error: String },                   // MCP server for this agent
    Unknown,                                              // agent not running, status not available
}
```

`IntegrationStorage` (`src/storage/integration.rs`, implemented in `src/storage/sqlite/integration.rs`):

```rust
async fn list_integrations(&self) -> Result<Vec<Integration>>;
async fn list_agent_integrations(&self, agent_id: &str) -> Result<Vec<Integration>>;
async fn get_integration(&self, id: &str) -> Result<Option<Integration>>;
async fn save_integration(&self, i: &Integration) -> Result<()>;          // upsert; enforces token uniqueness
async fn delete_integration(&self, id: &str) -> Result<Vec<AgentId>>;    // returns the agents it was attached to
async fn set_attachment(&self, id: &str, agent_id: &str, attached: bool) -> Result<Vec<AgentId>>; // returns the affected agents (a channel move returns both)
async fn delete_agent_attachments(&self, agent_id: &str) -> Result<()>;
```

## 4. Settings per kind

| Kind | Family | Settings JSON | Secrets (redacted to `••••<last 4>`) | Required |
|---|---|---|---|---|
| `discord` | channel | `{ "token": "…" }` | `token` | `token` |
| `telegram` | channel | `{ "token": "…" }` | `token` | `token` |
| `brave_search` | addon | `{ "api_key": "…", "safesearch": true }` | `api_key` | `api_key` |
| `mcp` | mcp | `{ "transport": "local", "command": "…", "args": [..], "env": {..} }` or `{ "transport": "http", "uri": "…" }` (today's `McpClientConfig`) | every `env` value | `command` or `uri` |

**Redaction round-trip** (`merge_secrets`): when a request leaves out a secret field, or sends the redacted string back unchanged, the stored value is kept. For example, an MCP edit that only changes `args` keeps every `env` value. Sending `null` for a secret clears it, which for a required secret leaves the integration `incomplete`.

## 5. What each kind contributes to an agent

| Kind | Native tools added to `IntegrationTools` | MCP | Reader |
|---|---|---|---|
| `discord` | the six Discord tools (`new_discord_tools`) | — | `DiscordChannelReader` |
| `telegram` | the three Telegram tools | — | `TelegramChannelReader` |
| `brave_search` | `brave_search_web`, `brave_search_news` (category `brave_search`) | — | — |
| `mcp` | — | one `McpServer` keyed by `name` | — |

Tool names and categories are unchanged except for the addon namespacing (research R12): Brave's `web_search` / `news_search` become `brave_search_web` / `brave_search_news` under category `brave_search`. `DREAM_TOOL_NAMES` doesn't include any integration tool and is unchanged. The tools simply move from `VizierTools::new` into each kind's `contribute`.

## 6. Runtime state (not persisted)

- **`VizierAgent.tools: ArcSwap<VizierTools>`**: the composed snapshot. A turn holds one `Arc<VizierTools>` from entry to end.
- **`BaseTools`**: built once per instance; everything that isn't an integration.
- **`IntegrationTools { native: VizierToolSet, mcp: HashMap<String, Arc<McpServer>> }`**: rebuilt by `refresh_integrations()`. An `McpServer` whose `(name, settings)` didn't change is reused.
- **`ChannelConnections`**: `HashMap<integration_id, RunningReader { agent_id, token_fingerprint, task, health }>`, owned by `VizierChannels`.
- **`IntegrationHealth`** on `VizierDependencies`: `RwLock<HashMap<integration_id, last_error>>`, written by addon tools on an auth failure and cleared on success.

## 7. State transitions

**Channel connection** (computed by `ChannelConnections`):

```
disabled ──enable──▶ (no agent) unassigned ──assign──▶ connecting ──ok──▶ running
running ──run ends / error──▶ reconnecting ──backoff──▶ connecting
connecting/running ──FatalChannelError──▶ failed ──token replaced──▶ connecting
any ──disable──▶ disabled          any ──unassign──▶ unassigned
running ──reassign to agent B──▶ connecting (as B)    token replaced ──▶ connecting
```

**MCP server for one agent** (`McpServer::status`): `connecting → connected`, `connected → unreachable` (crash or hang) `→ connecting` (backoff), as today.

**Addon**:
- `incomplete` (missing required setting) `→ ready` when the setting is saved;
- `ready → failed` on an auth error from the provider `→ ready` on the next successful call or a key change.

## 8. Permissions (`AVAILABLE_PERMISSIONS`)

- **Added:** `all_agents:integrations:manage` and `owned_agents:integrations:manage`.
- **Removed:** `agents:mcp_config`.

The access rules are in research R6.
