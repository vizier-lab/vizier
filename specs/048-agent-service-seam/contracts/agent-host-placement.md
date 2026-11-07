# Contract: the agent host and the placement

**Feature**: #163 | **Date**: 2026-11-03

The split of today's `VizierAgents` into the worker side and the control side.

---

## AgentHost (worker side)

Builds and runs **only** the agents it has been told to hold. It is the only component allowed to
read a running agent's state.

```rust
impl AgentHost {
    /// Start the host with the agents it is told to hold.
    pub async fn new(deps: VizierDependencies, held: Vec<AgentId>) -> Result<Self>;

    /// Take an agent on. Subscribes to its subjects and builds it.
    pub async fn take(&mut self, agent_id: AgentId, config: AgentConfig) -> Result<()>;

    /// Let an agent go. The running turns end the way a delete ends them.
    pub async fn release(&mut self, agent_id: &AgentId) -> Result<()>;

    pub async fn run(&mut self) -> Result<()>;
}
```

**Rules**

1. The host runs exactly the agents it was given. It never reads the agent list itself (FR-004).
2. For an agent it does not hold, it refuses with *agent unavailable* - it never builds the agent on
   the spot (US3 scenario 4).
3. The host subscribes to `agent.{id}.req` and `agent.{id}.cmd` for each agent it holds, and to
   `agents.cmd` for the fleet-wide commands.
4. The guarantees that hold today hold here (FR-006):
   - an agent is built **before** it is persisted or swapped;
   - requests queued while an agent restarts are **not lost** - the request channel is handed from
     one supervisor to the next across an update;
   - an **update resumes** the running turns;
   - a **delete drops** them.
5. A build in flight refuses other creates, updates and deletes of that agent, so two swaps of one
   agent never race.

**Testable alone** (FR-016): built from a bus handle, a storage handle and a list of agents, it runs
with no gateway and no other role. The test drives it with a scripted request source and asserts on
the frames it produces.

## Placement (control side)

Decides which host owns which agent.

```rust
#[async_trait]
pub trait Placement: Send + Sync {
    /// The agents this node's host should hold, and who holds each.
    async fn assign(&self) -> Result<Vec<PlacedAgent>>;
}

pub struct PlacedAgent {
    pub agent_id: AgentId,
    pub owner: OwnerRef,
}
```

**Rules**

1. In a single node, `SingleNodePlacement` returns every stored agent with the one local host as
   its owner (FR-005). The single node therefore behaves exactly as before.
2. One owner per agent. Sharding an agent across hosts is out of scope.
3. Placement is the only component that may answer "which node owns this agent", and it answers only
   to the control role - never to a gateway (FR-007).
4. The owner of an agent is the holder of the lease `lease.agent.{id}`. A host that loses the lease
   stops the agent's work at once (the epic's fencing rule); the ownership columns and the fencing
   check on the write path are #164's work.

**In a single node today**, placement is a fixed answer: the local host holds everything. The
lease-backed placement arrives with the worker and control roles (#166, #168).

---

## What moves where

| Today (`VizierAgents`) | Goes to |
|---|---|
| Listing the stored agents and registering each one's request channel | `Placement` decides; `AgentHost` takes what it is told |
| Building an agent, keeping the supervisor, resuming pending turns | `AgentHost` |
| Create / update / delete / restart / refresh / integration status / health | `AgentHost` (the handlers); the seam is how a gateway reaches them |
| Deciding which agents to run | `Placement` |

The command loop stays one loop in the host. The change is *what it is told to hold*, not how it
runs an agent.
