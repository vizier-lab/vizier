//! Project routes (`specs/040-projects-mode/contracts/http-api.md`).
//!
//! `POST /agents/{agent_id}/projects` has no vector here on purpose: creation is desktop-only, so
//! the network router always answers `403 desktop_only` while the desktop dispatcher succeeds or
//! reports folder validation — a divergence the parity harness cannot express. It is covered by
//! the app's smoke test instead and is listed in `pending.txt` with the reason.

use crate::common::{Vector, v};
use serde_json::json;

pub fn vectors() -> Vec<Vector> {
    vec![
        v("GET", "/agents/agent-a/projects", None),
        v("GET", "/agents/agent-b/projects", None),
        v("GET", "/agents/missing/projects", None),
        v("GET", "/agents/agent-a/projects/p3xY9aQ2mZ", None),
        v("GET", "/agents/missing/projects/p3xY9aQ2mZ", None),
        v(
            "PATCH",
            "/agents/agent-a/projects/p3xY9aQ2mZ",
            Some(json!({ "name": "renamed" })),
        ),
        v("DELETE", "/agents/agent-a/projects/p3xY9aQ2mZ", None),
        v("GET", "/agents/agent-a/projects/p3xY9aQ2mZ/sessions", None),
        v("POST", "/agents/agent-a/projects/p3xY9aQ2mZ/sessions", None),
        // 041: mode, file suggestions, git and worktrees (contracts/http-api.md). The fixture
        // has no project row, so these pin the authorization/not-found envelope; the handlers'
        // own service tests cover the happy paths.
        v(
            "PUT",
            "/agents/agent-a/projects/p3xY9aQ2mZ/sessions/t1/mode",
            Some(json!({ "mode": "build" })),
        ),
        v("GET", "/agents/agent-a/projects/p3xY9aQ2mZ/files?q=age", None),
        v("GET", "/agents/agent-a/projects/p3xY9aQ2mZ/skills", None),
        v("GET", "/agents/agent-a/projects/p3xY9aQ2mZ/skills?topic=t1", None),
        v("GET", "/agents/agent-a/projects/p3xY9aQ2mZ/git", None),
        v("GET", "/agents/agent-a/projects/p3xY9aQ2mZ/git/changes", None),
        v("GET", "/agents/agent-a/projects/p3xY9aQ2mZ/git/diff?path=src%2Flib.rs", None),
        v("GET", "/agents/agent-a/projects/p3xY9aQ2mZ/git/branches", None),
        v(
            "POST",
            "/agents/agent-a/projects/p3xY9aQ2mZ/git/switch",
            Some(json!({ "branch": "main" })),
        ),
        v("GET", "/agents/agent-a/projects/p3xY9aQ2mZ/worktrees", None),
        v(
            "POST",
            "/agents/agent-a/projects/p3xY9aQ2mZ/worktrees",
            Some(json!({ "name": "fix", "base": "main" })),
        ),
        v("GET", "/agents/agent-a/projects/p3xY9aQ2mZ/worktrees/w1", None),
        v("DELETE", "/agents/agent-a/projects/p3xY9aQ2mZ/worktrees/w1", None),
    ]
}
