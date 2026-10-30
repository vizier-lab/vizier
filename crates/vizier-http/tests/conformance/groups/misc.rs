//! Artifacts, secrets, integrations (agent and global), skills (agent and global) and session
//! files. Success paths that write are left to unit tests (a second run would see the first's
//! write); everything here answers the same on every run.
use crate::common::{Vector, v};
#[allow(unused_imports)]
use serde_json::json;

pub fn vectors() -> Vec<Vector> {
    vec![
        // artifacts
        v("GET", "/agents/agent-a/artifacts", None),
        v("GET", "/agents/agent-b/artifacts", None),
        v("GET", "/agents/missing/artifacts", None),
        v("GET", "/agents/agent-a/artifacts?kind=bogus", None),
        v("GET", "/agents/agent-a/artifacts?cursor=%25", None),
        v("GET", "/agents/agent-a/artifacts?from=nope", None),
        v("GET", "/agents/agent-a/artifacts/activity", None),
        v("GET", "/agents/agent-a/artifacts/activity?cursor=%25", None),
        v("GET", "/agents/agent-a/artifacts/none", None),
        v("GET", "/agents/agent-a/artifacts/none/content", None),
        v("GET", "/agents/agent-b/artifacts/none/content", None),
        v("DELETE", "/agents/agent-a/artifacts/none", None),
        v("DELETE", "/agents/agent-b/artifacts/none", None),
        // secrets
        v("GET", "/agents/agent-a/secrets", None),
        v("GET", "/agents/agent-b/secrets", None),
        v("GET", "/agents/missing/secrets", None),
        v("GET", "/agents/agent-a/secrets?limit=5", None),
        v("GET", "/agents/agent-a/secrets?limit=5&sort=bogus", None),
        v("GET", "/agents/agent-a/secrets/events", None),
        v("GET", "/agents/agent-b/secrets/events?limit=1000", None),
        v("PUT", "/agents/agent-a/secrets/bad%20name", Some(json!({ "value": "value-00000001" }))),
        v("PUT", "/agents/agent-b/secrets/TOKEN", Some(json!({ "value": "" }))),
        v("DELETE", "/agents/agent-a/secrets/NO_SUCH", None),
        v("DELETE", "/agents/agent-b/secrets/NO_SUCH", None),
        // agent integrations
        v("GET", "/agents/agent-a/integrations", None),
        v("GET", "/agents/agent-b/integrations", None),
        v("GET", "/agents/missing/integrations", None),
        v("GET", "/agents/agent-a/integrations?family=mcp&limit=5", None),
        v("GET", "/agents/agent-a/integrations?family=mcp&limit=5&sort=bogus", None),
        v("GET", "/agents/agent-a/integrations/grants", None),
        v("GET", "/agents/agent-b/integrations/grants", None),
        v("PUT", "/agents/agent-b/integrations/grants", Some(json!({}))),
        v("PUT", "/agents/agent-a/integrations/grants", Some(json!({}))),
        v("GET", "/agents/agent-a/integrations/events", None),
        v("GET", "/agents/agent-a/integrations/events?before=garbage", None),
        v("POST", "/agents/agent-a/integrations", Some(json!({ "kind": "no-such-kind", "name": "x" }))),
        v("POST", "/agents/agent-b/integrations", Some(json!({ "kind": "no-such-kind", "name": "x" }))),
        v("DELETE", "/agents/agent-a/integrations/none", None),
        v("PUT", "/agents/agent-a/integrations/none/attachment", Some(json!({ "attached": true }))),
        v("PUT", "/agents/agent-a/integrations/none/attachment?detach=1", Some(json!({ "attached": false }))),
        // global integrations
        v("GET", "/integrations/kinds", None),
        v("GET", "/integrations/kinds?family=mcp", None),
        v("GET", "/integrations/none", None),
        v("PUT", "/integrations/none", Some(json!({ "name": "x" }))),
        v("DELETE", "/integrations/none", None),
        // global skills
        v("GET", "/skills", None),
        v("GET", "/skills/none", None),
        v("GET", "/skills/none/resources", None),
        v("GET", "/skills/none/resources/a.md", None),
        v("PUT", "/skills/none", Some(json!({ "description": "d" }))),
        v("DELETE", "/skills/none", None),
        v("POST", "/skills", Some(json!({ "name": "conf-skill", "description": "d", "content": "c", "keywords": ["k"] }))),
        v("GET", "/skills/conf-skill", None),
        v("GET", "/skills?limit=5&filter=global", None),
        v("PUT", "/skills/conf-skill", Some(json!({ "description": "d", "base_hash": "stale" }))),
        // agent skills
        v("GET", "/agents/agent-a/skills", None),
        v("GET", "/agents/agent-b/skills?scope=all", None),
        v("GET", "/agents/missing/skills", None),
        v("GET", "/agents/agent-a/skills/available", None),
        v("GET", "/agents/agent-b/skills/available", None),
        v("GET", "/agents/agent-a/skills/none", None),
        v("PUT", "/agents/agent-a/skills/none", Some(json!({ "description": "d" }))),
        v("DELETE", "/agents/agent-a/skills/none", None),
        v("DELETE", "/agents/agent-b/skills/none", None),
        v("POST", "/agents/agent-a/skills", Some(json!({ "name": "agent-conf-skill", "description": "d", "content": "c" }))),
        v("POST", "/agents/agent-b/skills", Some(json!({ "name": "agent-conf-skill", "description": "d", "content": "c" }))),
        v("GET", "/agents/agent-a/skills/agent-conf-skill", None),
        v("PUT", "/agents/agent-a/skills/agent-conf-skill", Some(json!({ "base_hash": "stale" }))),
        // session files
        v("POST", "/files/upload", Some(json!({ "file": "", "filename": "a.txt" }))),
        v("POST", "/files/upload?bad64=1", Some(json!({ "file": "!!!", "filename": "a.txt" }))),
        v("GET", "/files/missing", None),
    ]
}
