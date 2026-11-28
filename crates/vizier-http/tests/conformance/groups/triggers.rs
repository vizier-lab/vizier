//! Triggers (`specs/071-triggers/contracts/http-api.md`). The fixture agents have no triggers, so
//! every change is aimed at a trigger that does not exist, or is refused before it is made: the
//! vectors check who may reach a route without changing state between the harness's two sends.
use serde_json::json;

use crate::common::{Vector, v};

pub fn vectors() -> Vec<Vector> {
    vec![
        v("GET", "/trigger-kinds", None),
        v("GET", "/agents/agent-a/triggers", None),
        v("GET", "/agents/agent-a/trigger-summary", None),
        v("GET", "/agents/missing/trigger-summary", None),
        v("GET", "/agents/agent-b/triggers", None),
        v("GET", "/agents/missing/triggers", None),
        // No kind: refused before anything is saved, whoever asks.
        v("POST", "/agents/agent-a/triggers", Some(json!({ "slug": "ci", "title": "CI", "instruction": "x" }))),
        v("POST", "/agents/agent-b/triggers", Some(json!({ "slug": "ci", "kind": "nope", "title": "CI", "instruction": "x" }))),
        v("GET", "/agents/agent-a/triggers/absent", None),
        v("PUT", "/agents/agent-a/triggers/absent", Some(json!({ "title": "x" }))),
        v("DELETE", "/agents/agent-a/triggers/absent", None),
        v("POST", "/agents/agent-a/triggers/absent/approve", Some(json!({}))),
        v("POST", "/agents/agent-b/triggers/absent/secret", None),
        v("POST", "/agents/agent-a/triggers/absent/test", None),
        v("GET", "/agents/agent-a/triggers/absent/events", None),
        v("GET", "/agents/agent-a/triggers/absent/runs?limit=5", None),
        v("GET", "/agents/agent-a/triggers/absent/runs/1", None),
        // The public address: an unknown trigger is a 404 for everyone, signed in or not.
        v("POST", "/hooks/trg_nothere", Some(json!({ "hello": "world" }))),
    ]
}
