//! Detached scripts of a topic (spec 058 contracts/http-and-frames.md). Both routes are
//! reads, so every vector is state-free.
use crate::common::{Vector, v};

pub fn vectors() -> Vec<Vector> {
    let list = "/agents/agent-a/channel/webui/topic/t1/detached-scripts";
    vec![
        v("GET", list, None),
        v("GET", "/agents/agent-b/channel/webui/topic/t1/detached-scripts", None),
        v("GET", "/agents/missing/channel/webui/topic/t1/detached-scripts", None),
        v("GET", "/agents/agent-a/channel/webui/topic/t1/detached-scripts/d_nothere", None),
        v("GET", "/agents/agent-b/channel/webui/topic/t1/detached-scripts/d_nothere", None),
    ]
}
