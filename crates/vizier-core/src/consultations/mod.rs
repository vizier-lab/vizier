//! Consulting other agents (`specs/064-consult-agents`).
//!
//! A consultation is a `subagent` row with a consultee: the engine, the state machine, delivery
//! and recovery are the subagent ones (`agents::subagents`). What is consultation-only lives
//! here: who a consultation is for and which agents that person may use (`access`).

pub mod access;
