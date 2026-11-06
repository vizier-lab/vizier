//! Workspace routes (`specs/053-workspace-channel/contracts/http-api.md`).
//!
//! The harness sends every vector twice as the same person, so a vector must not change state:
//! a successful create would conflict the second time. These pin the reads and the refusals (a
//! stranger sees `404` everywhere, a person without `workspaces:create` gets `403`, a bad name
//! `422`). The create, member and topic success paths are covered in
//! `crates/vizier-core/tests/workspaces_service.rs`.

use crate::common::{Vector, v};
use serde_json::json;

pub fn vectors() -> Vec<Vector> {
    vec![
        v("GET", "/workspaces", None),
        // Not a member of anything, and not an administrator: nothing leaks.
        v("GET", "/workspaces/missing", None),
        v("POST", "/workspaces/missing/topics", Some(json!({ "name": "daily" }))),
        v(
            "POST",
            "/workspaces/missing/members",
            Some(json!({ "kind": "agent", "handle": "agent-a" })),
        ),
        v("DELETE", "/workspaces/missing/members/agent/agent-a", None),
        // A name the rule refuses is refused before anything is written: 422 for someone who
        // may create, 403 for someone who may not.
        v("POST", "/workspaces", Some(json!({ "name": "Not Allowed!" }))),
    ]
}
