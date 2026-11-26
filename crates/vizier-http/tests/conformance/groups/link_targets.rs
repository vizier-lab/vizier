//! The entity wiki-link name index (spec 066): the bounded index, one search, and the exact
//! resolve, per person. Vectors run after the tasks group, so `t-one` exists for `task:t-one`.
use crate::common::{Vector, v};
use serde_json::json;

pub fn vectors() -> Vec<Vector> {
    vec![
        v("GET", "/agents/agent-a/link-targets", None),
        v("GET", "/agents/agent-b/link-targets", None),
        v("GET", "/agents/missing/link-targets", None),
        v("GET", "/agents/agent-a/link-targets?kind=artifact&q=report&limit=5", None),
        v("GET", "/agents/agent-a/link-targets?kind=flavour&q=x", None),
        v(
            "POST",
            "/agents/agent-a/link-targets/resolve",
            Some(json!({
                "links": [
                    { "kind": "artifact", "name": "report" },
                    { "kind": "function", "name": "fetch_weather" },
                    { "kind": "task", "name": "t-one" },
                    { "kind": "skill", "name": "summarize" },
                    { "kind": "memory", "name": "core" },
                    { "kind": "memory", "name": "core/soul" },
                    { "kind": "flavour", "name": "vanilla" }
                ]
            })),
        ),
        v("POST", "/agents/agent-b/link-targets/resolve", Some(json!({ "links": [] }))),
        v("POST", "/agents/missing/link-targets/resolve", Some(json!({ "links": [] }))),
    ]
}
