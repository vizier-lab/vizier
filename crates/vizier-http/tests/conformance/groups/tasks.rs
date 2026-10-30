//! Tasks and runs, dream, background jobs and the conversation picker (spec 038 T062–T065).
use crate::common::{Vector, v};
use serde_json::json;

fn task_body(slug: &str) -> serde_json::Value {
    json!({
        "slug": slug, "title": "Check", "instruction": "do it",
        "schedule": { "type": "OneTime", "datetime": "2099-01-01T00:00:00Z" },
    })
}

pub fn vectors() -> Vec<Vector> {
    let jobs = "/agents/agent-a/channel/webui/topic/t1/jobs";
    vec![
        // tasks
        v("POST", "/agents/agent-a/tasks", Some(task_body("t-one"))),
        v("POST", "/agents/agent-a/tasks?bad=1", Some(json!({
            "slug": "bad", "title": "x", "instruction": "x",
            "schedule": { "type": "Cron", "expression": "not a cron" },
        }))),
        v("POST", "/agents/agent-b/tasks", Some(task_body("t-two"))),
        v("POST", "/agents/missing/tasks", Some(task_body("t-three"))),
        v("GET", "/agents/agent-a/tasks", None),
        v("GET", "/agents/agent-a/tasks?is_active=true&limit=5&sort=name", None),
        v("GET", "/agents/agent-b/tasks", None),
        v("GET", "/agents/missing/tasks", None),
        v("GET", "/agents/agent-a/tasks/t-one", None),
        v("GET", "/agents/agent-a/tasks/absent", None),
        v("PUT", "/agents/agent-a/tasks/t-one", Some(task_body("t-one"))),
        v("PUT", "/agents/agent-a/tasks/absent", Some(task_body("absent"))),
        v("GET", "/agents/agent-a/tasks/t-one/runs", None),
        v("GET", "/agents/agent-a/tasks/t-one/runs?limit=1", None),
        v("GET", "/agents/agent-a/tasks/absent/runs", None),
        v("GET", "/agents/agent-b/tasks/t-one/runs", None),
        v("GET", "/agents/agent-a/tasks/t-one/runs/2020-01-01T00:00:00Z/history", None),
        v("GET", "/agents/agent-a/tasks/absent/runs/2020-01-01T00:00:00Z/history", None),
        v("DELETE", "/agents/agent-a/tasks/t-one", None),
        v("DELETE", "/agents/agent-b/tasks/t-one", None),
        // dream
        v("POST", "/agents/agent-a/dream/trigger", None),
        v("POST", "/agents/agent-b/dream/trigger", None),
        v("POST", "/agents/missing/dream/trigger", None),
        v("GET", "/agents/agent-a/dream/status", None),
        v("GET", "/agents/agent-b/dream/status", None),
        v("GET", "/agents/agent-a/dream/journal", None),
        v("GET", "/agents/agent-a/dream/journal?limit=5&offset=0", None),
        v("GET", "/agents/agent-a/dream/journal/nope", None),
        v("GET", "/agents/agent-a/dream/nights", None),
        v("GET", "/agents/agent-a/dream/nights?days=7&tz=%2B07:00", None),
        v("GET", "/agents/agent-b/dream/nights", None),
        v("GET", "/agents/agent-a/dream/cycles", None),
        v("GET", "/agents/agent-a/dream/cycles?limit=3&before=2030-01-01T00:00:00Z", None),
        v("GET", "/agents/agent-b/dream/cycles", None),
        // background jobs
        v("GET", jobs, None),
        v("GET", "/agents/agent-b/channel/webui/topic/t1/jobs", None),
        v("GET", "/agents/missing/channel/webui/topic/t1/jobs", None),
        v("GET", "/agents/agent-a/channel/webui/topic/t1/jobs/nojob", None),
        v("POST", "/agents/agent-a/channel/webui/topic/t1/jobs/nojob/cancel", Some(json!({ "reason": "because" }))),
        v("POST", "/agents/agent-a/channel/webui/topic/t1/jobs/nojob/cancel", Some(json!({}))),
        v("GET", "/agents/agent-a/channel/webui/topic/t1/jobs/nojob/pieces/0/history", None),
        // conversations
        v("GET", "/agents/agent-a/conversations", None),
        v("GET", "/agents/agent-a/conversations?limit=5&channel=webui&q=x", None),
        v("GET", "/agents/agent-a/conversations?cursor=%3F%3F%3F", None),
        v("GET", "/agents/agent-b/conversations", None),
        v("GET", "/agents/missing/conversations", None),
    ]
}
