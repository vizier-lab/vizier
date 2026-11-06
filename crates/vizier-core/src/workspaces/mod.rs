//! The Workspace channel (`specs/053-workspace-channel/`): a shared space with people, agents
//! and topics, where agents read silently and wake only when mentioned.

pub mod access;
pub mod commands;
pub mod mentions;
pub mod names;

/// The slug prefix of a Workspace session's channel (`workspace__{id}`). Every prefix check that
/// decides who may see a conversation must account for it (research R8).
pub const PREFIX: &str = "workspace__";
