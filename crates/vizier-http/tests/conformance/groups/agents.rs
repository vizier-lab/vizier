use crate::common::{Vector, v};
use serde_json::json;

pub fn vectors() -> Vec<Vector> {
    let valid_agent = json!({"agent_id": "missing", "name": "M", "provider": "dummyplug", "model": "dummyplug"});
    vec![
        v("GET", "/agents?limit=10", None),
        v("GET", "/agents?limit=10&filter=nope", None),
        v("GET", "/agents/health", None),
        v("GET", "/agents/agent-a/ping", None),
        v("GET", "/agents/missing/ping", None),
        v("GET", "/agents/agent-a/sharing", None),
        v("GET", "/agents/agent-b/sharing", None),
        v("GET", "/agents/missing/sharing", None),
        v("PATCH", "/agents/agent-a/sharing", Some(json!({"add": [], "remove": []}))),
        v("PATCH", "/agents/agent-b/sharing", Some(json!({"add": [], "remove": []}))),
        v("PATCH", "/agents/missing/sharing", Some(json!({"add": [], "remove": []}))),
        v("GET", "/agents/agent-a/usage?start_date=2026-10-01T00:00:00Z&end_date=2026-10-10T00:00:00Z", None),
        v("GET", "/agents/agent-a/usage?start_date=bad", None),
        v("GET", "/agents/agent-a/usage?activity=nope", None),
        v("GET", "/agents/missing/usage", None),
        v("GET", "/agents/agent-a/usage/conversation", None),
        v("GET", "/agents/agent-a/usage/conversation?channel=x&limit=0", None),
        v("GET", "/agents/agent-a/usage/conversation?channel=x&start_date=2026-10-01T00:00:00Z&end_date=2026-10-10T00:00:00Z", None),
        v("GET", "/agents/missing/usage/conversation?channel=x", None),
        v("POST", "/agents/missing/restart", None),
        v("POST", "/agents/missing/chat", Some(json!({"channel_id": "c", "topic_id": "t", "content": "hi"}))),
        v("POST", "/agents/missing/btw", Some(json!({"channel_id": "c", "topic_id": "t", "question": "hi"}))),
        v("POST", "/agents", Some(json!({"nonsense": true}))),
        v("POST", "/agents", Some(json!({"agent_id": "x", "name": "X", "provider": "dummyplug", "model": "dummyplug", "discord_token": "t"}))),
        v("POST", "/agents", Some(json!({"agent_id": "x", "name": "X", "provider": "dummyplug", "model": "dummyplug", "self_management": {}}))),
        v("POST", "/agents", Some(json!({"agent_id": "x", "name": "X", "provider": "dummyplug", "model": "dummyplug", "embedding": {"provider": "openai", "model": " "}}))),
        v("PUT", "/agents/agent-a", Some(json!({"nonsense": true}))),
        v("PUT", "/agents/missing", Some(valid_agent)),
        v("DELETE", "/agents/missing", None),
    ]
}
