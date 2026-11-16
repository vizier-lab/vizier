//! Interactive pages of a topic (`specs/063-html-generative-ui` contracts/http.md). Both routes
//! are reads, so every vector is state-free. No page exists in the fixture, so these pin the
//! refusals: another person's agent, a missing agent, and an unknown page or version.
use crate::common::{Vector, v};

pub fn vectors() -> Vec<Vector> {
    vec![
        v("GET", "/agents/agent-a/channel/webui/topic/t1/pages/pg_nothere", None),
        v("GET", "/agents/agent-b/channel/webui/topic/t1/pages/pg_nothere", None),
        v("GET", "/agents/missing/channel/webui/topic/t1/pages/pg_nothere", None),
        v("GET", "/agents/agent-a/channel/webui/topic/t1/pages/pg_nothere/versions/1", None),
        v("GET", "/agents/agent-b/channel/webui/topic/t1/pages/pg_nothere/versions/1", None),
        v("GET", "/agents/agent-a/channel/webui/topic/t1/pages/pg_nothere/versions/1/frame", None),
        v("GET", "/agents/missing/channel/webui/topic/t1/pages/pg_nothere/versions/1/frame", None),
        v("GET", "/ui-frames/pg_nothere.1.9999999999999.forged", None),
    ]
}
