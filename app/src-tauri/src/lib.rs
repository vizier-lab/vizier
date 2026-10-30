//! The Vizier desktop app: an embedded `VizierRuntime` plus the glue that lets a window talk
//! to it. Everything but [`shell`] is plain Rust and tested without a webview.

pub mod desktop;
pub mod engine;
pub mod ipc;
pub mod settings;
pub mod web_access;

#[cfg(feature = "shell")]
pub mod shell;
