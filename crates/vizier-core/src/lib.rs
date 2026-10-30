//! vizier: a runtime for AI agents with their own tools, memory and providers.
//!
//! The crate has two tiers of public items:
//!
//! - **Stable**: [`runtime::VizierRuntime`] and its options, events and health, [`auth`],
//!   [`config`], [`error`], [`schema`] (the request/response types a host sends and reads),
//!   [`transport::VizierTransport`] and [`access`]. These are documented and checked
//!   for breaking changes.
//! - **Adapter surface** (`#[doc(hidden)]`): the modules the HTTP adapter and the command-line
//!   binary reach into today (`dependencies`, `storage`, `agents`, …). They are public only so
//!   those crates can be built on top of this one, are **not** part of the API, and will be
//!   replaced by application services (phase 5 of vizier-lab/vizier#102).
//!
//! Everything else is private to the crate.
//!
//! # Starting an engine
//!
//! ```no_run
//! use vizier_core::prelude::*;
//!
//! # async fn run() -> vizier_core::Result<()> {
//! let config = VizierConfig::with_workspace("/path/I/choose".into());
//! let mut runtime = VizierRuntime::start(RuntimeOptions { config }).await?;
//!
//! match runtime.wait().await {
//!     RuntimeExit::Requested => {}
//!     RuntimeExit::SubsystemStopped { name, outcome } => {
//!         eprintln!("{name} stopped: {outcome}"); // the host decides what to do
//!     }
//! }
//! runtime.shutdown().await
//! # }
//! ```
//!
//! Requests and agent commands go through [`transport::VizierTransport`], reached with
//! [`runtime::VizierRuntime::transport`]: `send_request` for a chat turn and `send_agent_command`
//! to create, update or delete an agent. See `examples/headless.rs`.

#![allow(unused)]
#[macro_use]
extern crate tracing;

use crate::error::VizierError;

pub type Result<T> = std::result::Result<T, VizierError>;

/// The items a host needs to start an engine, create an agent and send it a message.
pub mod prelude {
    pub use crate::Result;
    pub use crate::access::Caller;
    pub use crate::auth::{AuthMethod, Principal, Role};
    pub use crate::config::VizierConfig;
    pub use crate::error::VizierError;
    pub use crate::runtime::{
        HealthReport, RuntimeEvent, RuntimeEvents, RuntimeExit, RuntimeOptions, SubsystemState,
        VizierRuntime,
    };
    pub use crate::schema::{
        AgentCommand, AgentCommandResult, AgentConfig, VizierChannelId, VizierRequest,
        VizierRequestContent, VizierResponse, VizierResponseContent, VizierSession,
    };
    pub use crate::transport::{SessionEvent, VizierTransport};
}

// Stable tier.
pub mod access;
pub mod auth;
pub mod config;
pub mod error;
pub mod runtime;
pub mod schema;
pub mod service;
pub mod transport;

// Adapter surface: used by the HTTP adapter and the binary, unstable.
#[doc(hidden)]
pub mod agents;
#[doc(hidden)]
pub mod artifacts;
#[doc(hidden)]
pub mod catalogue;
#[doc(hidden)]
pub mod channels;
#[doc(hidden)]
pub mod dependencies;
#[doc(hidden)]
pub mod integrations;
#[doc(hidden)]
pub mod llm;
pub(crate) mod notifications;
pub(crate) mod provider_keys;
pub(crate) mod provider_migration;
pub(crate) mod scheduler;
pub(crate) mod secrets;
#[doc(hidden)]
pub mod session_files;
#[doc(hidden)]
pub mod setup;
#[doc(hidden)]
pub mod skill;
#[doc(hidden)]
pub mod storage;
#[doc(hidden)]
pub mod telemetry;
pub(crate) mod usage;
#[doc(hidden)]
pub mod utils;

// Private to the engine.
pub(crate) mod constant;
pub(crate) mod embedding;
pub(crate) mod encryption;
pub(crate) mod file_manager;
pub(crate) mod image_generation;
pub(crate) mod indexer;
pub(crate) mod sandbox;
pub(crate) mod stt;
pub(crate) mod tts;

#[cfg(test)]
mod tests_support;
