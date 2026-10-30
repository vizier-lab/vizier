use crate::common::{Vector, v};
use serde_json::json;

pub fn vectors() -> Vec<Vector> {
    vec![
        v("GET", "/agents/agent-a/channel/web/topics", None),
        v("GET", "/agents/agent-b/channel/web/topics", None),
        v("GET", "/agents/missing/channel/web/topics", None),
        v("GET", "/agents/agent-a/channel/web/topic/t1/history", None),
        v("GET", "/agents/agent-b/channel/web/topic/t1/history", None),
        v("GET", "/agents/agent-a/channel/discord__1/topic/t1/history", None),
        v("GET", "/agents/missing/channel/web/topic/t1/history", None),
        v("GET", "/agents/agent-a/channel/web/topic/t1/files", None),
        v("GET", "/agents/agent-b/channel/web/topic/t1/files?limit=1&offset=0", None),
        v("DELETE", "/agents/agent-a/channel/web/topic/t1/files", None),
        v("DELETE", "/agents/agent-a/channel/discord__1/topic/t1/files", None),
        v("POST", "/agents/agent-a/channel/web/topic/t1/files/promote", Some(json!({"filename": "x"}))),
        v("POST", "/agents/agent-b/channel/web/topic/t1/files/promote", Some(json!({"filename": "x"}))),
        v("GET", "/agents/agent-a/channel/web/topic/t1/detail", None),
        v("GET", "/agents/agent-a/channel/telegram__1/topic/t1/detail", None),
        v("GET", "/agents/missing/channel/web/topic/t1/detail", None),
        v("DELETE", "/agents/agent-a/channel/web/topic/t1", None),
        v("DELETE", "/agents/agent-b/channel/web/topic/t1", None),
    ]
}
