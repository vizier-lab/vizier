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

#![allow(unused)]
#[macro_use]
extern crate tracing;

use crate::error::VizierError;

pub type Result<T> = std::result::Result<T, VizierError>;

// Stable tier.
pub mod access;
pub mod auth;
pub mod config;
pub mod error;
pub mod runtime;
pub mod schema;
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
pub mod cli;
#[doc(hidden)]
pub mod command;
#[doc(hidden)]
pub mod dependencies;
#[doc(hidden)]
pub mod integrations;
#[doc(hidden)]
pub mod llm;
#[doc(hidden)]
pub mod notifications;
#[doc(hidden)]
pub mod provider_keys;
#[doc(hidden)]
pub mod provider_migration;
#[doc(hidden)]
pub mod scheduler;
#[doc(hidden)]
pub mod secrets;
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
#[doc(hidden)]
pub mod usage;
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
