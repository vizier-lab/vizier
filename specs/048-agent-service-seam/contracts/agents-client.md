# Contract: the agent seam (`AgentsClient`)

**Feature**: #163 | **Date**: 2026-11-03

The one way the gateway side reaches an agent. `vizier-http` and `core::service` call this trait;
they never touch a running agent's memory. A local implementation ships now; a bus implementation
lands in #168/#169 against this document, without reading the local one.

This document is the seam's contract for **FR-001** (the operations it promises), **FR-002** (the two
implementations behind one contract), **FR-007** (it addresses the agent, never a host) and
**FR-017** (it is written down so a later implementation can be built from it alone).

---

## The trait

```rust
#[async_trait]
pub trait AgentsClient: Send + Sync {
    /// Send a request to an agent and receive its frames until the turn ends.
    ///
    /// The session travels with the request: the turn belongs to a conversation, and the
    /// agent answers in it.
    async fn request(
        &self,
        agent_id: &str,
        session: &VizierSession,
        request: VizierRequest,
    ) -> Result<AgentTurn>;

    /// Abort the turn running in a session.
    async fn abort(&self, agent_id: &str, session: &VizierSession) -> Result<()>;

    /// Create an agent. It has no owner yet: the fleet places it.
    async fn create(&self, agent_id: &str, config: AgentConfig) -> Result<AgentSummary>;

    async fn update(&self, agent_id: &str, config: AgentConfig) -> Result<AgentSummary>;
    async fn delete(&self, agent_id: &str, delete_workspace: bool) -> Result<AgentSummary>;
    async fn restart(&self, agent_id: &str) -> Result<AgentSummary>;

    /// That one agent's health.
    async fn health(&self, agent_id: &str) -> Result<AgentHealthStatus>;

    /// That agent's view of its attached integrations.
    async fn integration_status(&self, agent_id: &str) -> Result<HashMap<String, IntegrationStatus>>;

    /// Ask the agent to rebuild its integration tools, without a restart.
    async fn refresh_integrations(&self, agent_id: &str) -> Result<()>;
}
```

Every operation returns `crate::Result<T>` (`VizierError`). There is no bespoke error type
(Principle V).

**The trait is object-safe** and is held as `Arc<dyn AgentsClient>` on `VizierDependencies`, so a
test can put a fake there without touching the engine.

---

## Addressing

| Operation | Subject | Who subscribes |
|---|---|---|
| `request`, `abort` | `agent.{id}.req` | The agent's owner. |
| `update`, `delete`, `restart`, `health`, `integration_status`, `refresh_integrations` | `agent.{id}.cmd` | The agent's owner. |
| `create` | `agents.cmd` | The control role (in one node: the local host). The agent has no owner yet. |

Rules that hold for every implementation:

1. **The seam addresses the agent, never a host.** No operation takes a node, a host or a lease, and
   no operation reveals which node owns the agent (FR-007).
2. **One subscriber per agent.** Only the owner subscribes to `agent.{id}.cmd`, so a command reaches
   exactly one handler. The bus delivers at most once, so a command consumed by a non-owner is
   lost.
3. **The id is one token.** The subject's id is percent-encoded (`Subject::encode_token`), so an id
   containing `.` cannot widen a subscription.

---

## Errors

| Situation | What the caller sees | Why |
|---|---|---|
| Nobody is subscribed to the agent's subject | *agent unavailable* - **no owner** - at once | The bus answers `NoResponders`. This is FR-014: never a timeout. |
| A responder existed but the reply never came | *agent unavailable* - **owner lost** - within the request deadline | The bus answers `Timeout`. This is FR-015: never a hang. |
| The agent refused the operation | The refusal's own message | Today a refusal is a `400`; the seam carries the message, the service layer maps it to the status. A refusal carries the marker `agent_refused`, so the service layer can tell it from a failure to reach the agent. |
| The agent does not exist | *not found* | Checked against storage before the command is sent, as today. |

**The deadlines.** Two, and the operation decides:

| Deadline | How long | Which operations |
|---|---|---|
| `REPLY_TIMEOUT` | 5 s | the operations that answer from the agent's *current* state: `health`, `integration_status`, `refresh_integrations` |
| `CHANGE_TIMEOUT` | 1 h | the operations that *change* the agent: `create`, `update`, `delete`, `restart` |

The five seconds are what FR-015 and SC-006 ask for: a lost owner is noticed at once. The
operations that change the agent cannot take that deadline, because a build can pull a model and
the caller waits for the result today; shortening it would refuse an update that is still on its
way (FR-021). The turn itself is not on a deadline at all: it ends when the agent's frames end,
and an agent that goes away mid-turn ends the turn with the unavailable error, as it does today.

Both availability reasons ride on the existing `ErrorKind::AgentUnavailable`
(`crates/vizier-core/src/schema/response.rs:69-70`). The reason is carried in the error's message
detail, not as a new kind, so the WebUI and the channels keep reading the kind they read today.

**The reason must be distinguishable.** An operator has to be able to tell "the fleet has not placed
this agent yet" from "the worker died" (FR-013). The contract therefore requires the distinction to
be *in the error*, not merely in the log.

---

## The frames of a turn

`request` returns a turn: a stream of `VizierResponse` frames that ends when the turn ends. The
stream carries the same frames as today, in the same order:

- the response stream is the same one the transport already produces
  (`crates/vizier-core/src/transport/mod.rs:257`, `:329`);
- an aborted turn ends with the frames the abort produces, and the abort's own notification reaches
  the aborted turn's sender (`agents/process.rs:82-100`);
- a turn that cannot be started (the agent is gone) ends with *agent unavailable*, not a silent
  close.

**Abort is a request, not a new message.** `abort` sends the request that aborts a turn today: a
`VizierRequest` whose content is `Command("abort")`, on `agent.{id}.req`
(`agents/process.rs:193-215`). A networked implementation needs no new mechanism.

---

## The two implementations

### Local (ships now)

`LocalAgentsClient` reaches the agent manager **over the in-process bus**, on the subjects above. It
is a typed wrapper over `VizierTransport`; it holds no reference to `VizierAgents` and reads
nothing from it. This is what makes the contract worth writing down: the local implementation
exercises exactly the path a networked one will.

### Bus (ships in #168/#169)

The same trait, over a networked bus. It publishes to the same subjects; the bus routes a subject to
the worker that subscribed to it. Its errors come from the same two bus cases (`NoResponders`,
`Timeout`), so the availability vocabulary is identical.

### Fake (ships now, for the test)

`FakeAgentsClient` records what it was asked for and answers from a script. The gateway-side test
runs with **no agent host running** (FR-016).

---

## What the seam does NOT cover

- **A fleet-wide health list.** A gateway cannot ask "which node owns agent X", so it cannot
  assemble the list by asking owners. It reads the published views instead
  ([contracts/shared-state-views.md](shared-state-views.md)).
- **The person's client socket.** It lives in the gateway process by definition.
- **Process-wide registries** (the questions registry, the open-connections registry, the
  notification inbox, the rate limiters). They are not per-agent state; the audit records why
  ([contracts/agent-state-audit.md](agent-state-audit.md)).
