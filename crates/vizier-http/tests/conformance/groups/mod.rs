//! One file per route group, so ports in parallel never edit the same lines. Paths use the
//! fixture ids: agents `agent-a` (owned by `olive`) and `agent-b` (owned by `root`); users
//! `root` (system), `olive` (owned_agents:view/edit/core) and `nobody` (no permissions).
use crate::common::Vector;
use serde_json::json;
use crate::common::v;

mod agents;
mod auth;
mod core;
mod memory;
mod misc;
mod projects;
mod providers;
mod tasks;
mod topics;

pub fn all() -> Vec<Vector> {
    let mut all = vec![
        v("GET", "/status", None),
        v("GET", "/ping", None),
        v("GET", "/auth/users/me", None),
        v("GET", "/agents", None),
        v("GET", "/agents/agent-a", None),
        v("GET", "/agents/agent-b", None),
        v("GET", "/agents/missing", None),
        v("GET", "/agents/agent-a/core", None),
        v("GET", "/agents/agent-b/core", None),
        v("PUT", "/agents/agent-a/core", Some(json!({ "content": "# core\n" }))),
        v("PUT", "/agents/agent-b/core", Some(json!({ "content": "# core\n" }))),
    ];
    all.extend(agents::vectors());
    all.extend(auth::vectors());
    all.extend(core::vectors());
    all.extend(memory::vectors());
    all.extend(misc::vectors());
    all.extend(projects::vectors());
    all.extend(providers::vectors());
    all.extend(tasks::vectors());
    all.extend(topics::vectors());
    all
}
