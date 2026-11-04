//! The `gateway-http` role's bus contract (`specs/163-agent-service-seam`).
//!
//! The gateway reaches an agent only through the seam, and the seam addresses the agent and
//! never a host: a turn travels on `agent.{id}.req` and a lifecycle command on
//! `agent.{id}.cmd`, so the command reaches exactly the agent it names. Creating an agent has
//! no owner yet, so it travels on the fleet-wide `agents.cmd`.
//!
//! The gateway takes no lease: it owns no agent and starts no work.
//!
//! A cluster gateway also uses the session and integration subjects the transport already
//! carries (`session.{id}.events`, `integration.changed`). Those are the engine's own channels
//! rather than the gateway's traffic, so they are not listed here.

use super::RoleContract;

pub const CONTRACT: RoleContract = RoleContract {
    role: "gateway-http",
    subscribes: &[],
    publishes: &["agent.*.req", "agent.*.cmd", "agents.cmd"],
    leases: &[],
};
