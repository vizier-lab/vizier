use crate::common::{Vector, v};
use serde_json::json;

/// The fixture agents have no running instance, so a memory operation that reaches the
/// engine answers 500 ("no memory-op channel"); the access checks and request validation
/// before it are what these vectors pin.
pub fn vectors() -> Vec<Vector> {
    let mut out = vec![];
    for agent in ["agent-a", "agent-b", "missing"] {
        let p = |s: &str| -> &'static str { Box::leak(format!("/agents/{agent}/memory{s}").into_boxed_str()) };
        let attachment = json!([{ "filename": "f", "content": { "Local": "/x" } }]);
        out.push(v("GET", p(""), None));
        out.push(v("GET", p("?bundle=default&tags=a,b&limit=5"), None));
        out.push(v("GET", p("/query?query=hello&limit=3&threshold=0.4"), None));
        out.push(v("GET", p("/bundles"), None));
        out.push(v("GET", p("/bundles?limit=5&sort=name"), None));
        out.push(v("GET", p("/bundles/graph?search=%20x%20"), None));
        out.push(v("GET", p("/default/graph"), None));
        out.push(v("GET", p("/note"), None));
        out.push(v("GET", p("/note/related"), None));
        out.push(v("GET", p("/doc/default/dir/note"), None));
        out.push(v("GET", p("/related/default/dir/note"), None));
        out.push(v("GET", p("/bundles/default/export"), None));
        out.push(v("GET", p("/history/default/dir/note.md"), None));
        out.push(v("GET", p("/history/default/note?seq=3"), None));
        out.push(v("GET", p("/history/default/note?to=3&from=1"), None));
        out.push(v("POST", p("/history/default/note"), Some(json!({ "seq": 1 }))));
        out.push(v("PUT", p("/note"), Some(json!({ "title": "t", "content": "c", "tags": ["x"] }))));
        out.push(v("PUT", p("/doc/default/dir/note"), Some(json!({ "title": "t", "content": "c" }))));
        out.push(v("PUT", p("/note2"), Some(json!({ "title": "t", "content": "c", "attachments": attachment }))));
        out.push(v("POST", p(""), Some(json!({ "title": "t", "content": "c", "attachments": attachment }))));
        out.push(v("POST", p("?plain=1"), Some(json!({ "title": "t", "content": "c" }))));
        out.push(v("POST", p("/attachments/default/note.md"), Some(json!({ "artifact": "nothing" }))));
        out.push(v("DELETE", p("/attachments/default/note?name=f"), None));
        out.push(v("DELETE", p("/bundles/default?force=true"), None));
        out.push(v("DELETE", p("/note"), None));
        out.push(v("DELETE", p("/doc/default/dir/note"), None));
    }
    out
}
