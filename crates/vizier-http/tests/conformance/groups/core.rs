use crate::common::{Vector, v};
use serde_json::json;

pub fn vectors() -> Vec<Vector> {
    let mut out = vec![];
    for agent in ["agent-a", "agent-b", "missing"] {
        let p = |s: &str| -> &'static str { Box::leak(format!("/agents/{agent}/core{s}").into_boxed_str()) };
        out.push(v("GET", p("/history"), None));
        out.push(v("GET", p("/history?offset=0&limit=1"), None));
        out.push(v("GET", p("/history?seq=999999"), None));
        out.push(v("GET", p("/history?to=999999"), None));
        out.push(v("POST", p("/history"), Some(json!({ "seq": 999999 }))));
    }
    // A save from a revision that is not the current one is refused with the current text.
    out.push(v("PUT", "/agents/agent-a/core?stale=1", Some(json!({ "content": "# core\n", "base_seq": 1234567 }))));
    out.push(v("PUT", "/agents/missing/core", Some(json!({ "content": "x" }))));
    out
}
