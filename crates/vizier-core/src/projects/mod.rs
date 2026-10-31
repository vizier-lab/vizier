//! Projects (`specs/040-projects-mode`): a host folder mounted to exactly one agent.
//!
//! - [`channel`] owns the `project__<id>` session identity.
//! - [`access`] is the permission matrix.
//! - [`context`] resolves the per-turn folder, shell state and instruction document.
//! - [`instructions`] reads the bounded `AGENTS.md` / `CLAUDE.md`.
//! - [`shell`] is the per-turn `workspace_shell` tool.

pub mod access;
pub mod channel;
pub mod context;
pub mod instructions;
pub mod shell;

pub use context::ProjectTurnContext;
