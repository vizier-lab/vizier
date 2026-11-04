//! The `worker` role's bus contract (`specs/048-agent-service-seam`).
//!
//! The worker holds agents and answers for them: it takes the turns of `agent.{id}.req`, the
//! lifecycle commands of `agent.{id}.cmd` and the fleet-wide creates of `agents.cmd`. A worker
//! subscribes to `agent.{id}.cmd` only for the agents it holds, so a command is answered by
//! exactly one handler (the bus delivers at most once, so a command consumed by a non-owner is
//! lost).
//!
//! The lease names are the shape `lease.agent.{id}`: the holder of an agent's lease is its
//! owner. The lease-backed placement arrives with the worker and control roles (#166, #168);
//! in a single node the one local host holds everything.
//!
//! `agents.cmd` - the fleet-wide create - belongs to the control role in a cluster. In a single
//! node it stays with the local host. That handover is #166's work, not this issue's.

use super::RoleContract;

pub const CONTRACT: RoleContract = RoleContract {
    role: "worker",
    subscribes: &["agent.*.req", "agent.*.cmd", "agents.cmd"],
    publishes: &[],
    leases: &["lease.agent.*"],
};
