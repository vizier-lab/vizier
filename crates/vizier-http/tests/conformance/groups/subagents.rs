//! The workers a topic hired (spec 059 US7). Every route is a read, so every vector is
//! state-free.
use crate::common::{Vector, v};

pub fn vectors() -> Vec<Vector> {
    vec![
        v("GET", "/agents/agent-a/channel/webui/topic/t1/subagents", None),
        v("GET", "/agents/agent-b/channel/webui/topic/t1/subagents", None),
        v("GET", "/agents/missing/channel/webui/topic/t1/subagents", None),
        v("GET", "/agents/agent-a/channel/webui/topic/t1/subagents/s_nothere", None),
        v("GET", "/agents/agent-b/channel/webui/topic/t1/subagents/s_nothere", None),
        v("GET", "/agents/agent-a/channel/webui/topic/t1/subagents/s_nothere/history", None),
        v("GET", "/agents/agent-b/channel/webui/topic/t1/subagents/s_nothere/history", None),
    ]
}
