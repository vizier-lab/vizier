//! Custom tools (`specs/061-agent-custom-tools/contracts/http-api.md`). The fixture agents have no
//! custom tools, so every change is aimed at a tool or category that does not exist: the
//! vectors check who may reach a route (view, edit, or `404`) without changing state between
//! the harness's two sends.
use serde_json::json;

use crate::common::{Vector, v};

pub fn vectors() -> Vec<Vector> {
    vec![
        v("GET", "/agents/agent-a/custom-tools", None),
        v("GET", "/agents/agent-b/custom-tools", None),
        v("GET", "/agents/missing/custom-tools", None),
        v("GET", "/agents/agent-a/custom-tools/events", None),
        v("GET", "/agents/agent-b/custom-tools/events?limit=5", None),
        v("GET", "/agents/agent-a/custom-tools/nothere", None),
        v("GET", "/agents/agent-a/custom-tools/nothere?version=1", None),
        v("PUT", "/agents/agent-a/custom-tools/nothere", Some(json!({ "expected_version": 1, "script": "1" }))),
        v("PUT", "/agents/agent-b/custom-tools/nothere", Some(json!({ "expected_version": 1 }))),
        v("DELETE", "/agents/agent-a/custom-tools/nothere", None),
        v("PUT", "/agents/agent-a/custom-tools/nothere/enabled", Some(json!({ "enabled": false }))),
        v("GET", "/agents/agent-a/custom-tools/nothere/versions", None),
        v("GET", "/agents/agent-a/custom-tools/nothere/diff?from=1", None),
        v("POST", "/agents/agent-a/custom-tools/nothere/rollback", Some(json!({ "to_version": 1, "expected_version": 1 }))),
        v("PUT", "/agents/agent-a/custom-tool-categories/nothere", Some(json!({ "description": "x" }))),
        v("PUT", "/agents/agent-b/custom-tool-categories/nothere", Some(json!({ "new_name": "y" }))),
    ]
}
