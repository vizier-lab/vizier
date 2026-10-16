# Contract: Applying Integration Changes at Runtime

**Feature**: `016-integrations-section`. This is the in-process contract behind FR-016 to FR-018. It is not exposed outside the binary, but tasks and reviews check against it.

## Event

```rust
pub struct IntegrationChanged {
    pub integration_id: String,
    pub family: IntegrationFamily,
    pub agents: Vec<AgentId>,   // attached before ∪ attached after the change
}
```

- **Published** on a `tokio::sync::broadcast` held by `VizierTransport` (`publish_integration_change` / `subscribe_integration_changes`), only after the storage transaction has committed.
- **Carries no settings.** Subscribers re-read storage.
- **Delivery may fail.** A lagged subscriber treats `RecvError::Lagged` as "reconcile everything it owns".

## Subscribers and their guarantees

### `VizierAgents`

For every `agent_id` in `agents`, it sends `AgentCommand::RefreshIntegrations { agent_id }` to that agent's supervisor. This never goes through the agent's request queue, so it never waits behind turns.

| Supervisor sees agent… | Action |
|---|---|
| running | `instance.refresh_integrations().await` |
| building / failed / backing off | nothing; the next build reads storage |
| deleted | nothing |

`VizierAgent::refresh_integrations()`:
1. Reads `list_agent_integrations(agent_id)` and keeps those that are `enabled` and not `incomplete`.
2. Builds `IntegrationTools` through each kind's `contribute`. It reuses the current snapshot's `Arc<McpServer>` when `(name, settings)` are equal, and warms up new ones.
3. Runs `compose(base, integrations)` and calls `self.tools.store(Arc::new(..))`.
4. On error, it keeps the previous snapshot and logs `warn!`. A refresh never takes the agent down.

**Guarantees**:
- **G1** No instance is rebuilt and no turn is aborted by an integration change.
- **G2** A turn uses exactly one `Arc<VizierTools>` snapshot from entry to end (FR-018). This covers `chat`, `prompt`, `dream_chat`, handover, and every tool call and script inside them.
- **G3** The next turn to *start* after `store` sees the change (FR-017).
- **G4** `categories`, and so the system prompt, change only when the composed tool set changes. Between changes the prompt stays byte-identical.
- **G5** Each agent has its own `McpServer` instances. A shared integration definition is never shared as a connection.

### `ChannelConnections`

On a `Channel`-family event, or `Lagged`, it runs `reconcile()`. The desired state is:

> every `enabled` channel integration with exactly one attachment runs one reader for that agent

Compared with the running readers:

| Desired vs running | Action |
|---|---|
| desired, not running | start (`kind.reader(..)`) |
| running, not desired (disabled / unassigned / deleted) | shutdown (`STOP_TIMEOUT`, then abort) |
| running, but agent or token fingerprint changed | shutdown, then start |
| running, unchanged | nothing (a `failed` reader stays failed until its token changes) |

**Guarantees**:
- **G6** Readers are independent of agent instances. Agent create/update/restart/delete never stops a reader, except delete, which removes the attachment and therefore the reader through `reconcile`.
- **G7** Reconciling one connection never touches another.
- **G8** A `FatalChannelError` marks only that connection `failed`. The agent stays running.

## Startup order (`run.rs`)

1. `VizierAgents` builds agents. `BaseTools::new` then reads the integrations from storage when each instance is built, so there is no refresh step at startup.
2. `VizierChannels::run` starts `ChannelConnections` with a full `reconcile()`.

## Health

- `AgentCommand::HealthCheck` keeps the shape of `AgentHealthStatus.channels`. It is filled from `ChannelConnections::health_for(agent_id)`.
- `AgentCommand::IntegrationStatus { agent_id }` returns `HashMap<integration_id, IntegrationStatus>`. MCP entries come from the instance's snapshot (`McpServer::status()`). Addon entries come from settings plus `IntegrationHealth`. Channel entries come from `ChannelConnections`.
