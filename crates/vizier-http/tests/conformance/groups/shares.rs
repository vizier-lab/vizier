//! Shared chat snapshots (`specs/056-shared-chat/contracts/share-api.md`).
//!
//! Vectors must not change state (the harness sends each twice), so every `POST /shares` here
//! is refused before anything is stored: `403` for a person who may not share (or an agent that
//! is not there), `400` for a document that fails its structural check.
//!
//! `GET /shares/{share_id}` has no vector on purpose: it is public, so it has no desktop twin
//! worth comparing, and it is listed in `pending.txt` with that reason. It is covered by
//! `tests/shares.rs` instead.
//!
//! List (`GET /shares`, `?all=true`), the people search and revoke (`DELETE`) of an id that is
//! not there change nothing, so they run through both the router and the dispatcher as well.

use crate::common::{Vector, v};
use serde_json::json;

fn body(agent: &str, snapshot: serde_json::Value) -> serde_json::Value {
    json!({
        "agent_id": agent, "topic_id": "t1", "title": "A chat", "audience": "public",
        "include_trail": false, "snapshot": snapshot
    })
}

pub fn vectors() -> Vec<Vector> {
    let wrong_version = json!({ "format_version": 2, "turns": [{ "parts": [] }] });
    vec![
        // 400 for the system person (who may share), 403 for the others.
        v("POST", "/shares", Some(body("agent-a", wrong_version.clone()))),
        v("POST", "/shares", Some(body("agent-b", wrong_version.clone()))),
        // An agent that is not there is refused like one the person may not share.
        v("POST", "/shares", Some(body("missing", wrong_version))),
        // Not a document at all.
        v("POST", "/shares", Some(json!({ "agent_id": "agent-a" }))),
        // The caller's own list (empty), the administrator's list (403 unless allowed), paging.
        v("GET", "/shares", None),
        v("GET", "/shares?limit=5&before=1", None),
        v("GET", "/shares?all=true", None),
        // People search: too short, then a real query.
        v("GET", "/shares/people?q=a", None),
        v("GET", "/shares/people?q=ol", None),
        // Revoking what is not there is the same 404 for everyone, whatever the id looks like.
        v("DELETE", "/shares/ZZZZZZZZZZZZZZZZZZZZZ", None),
        v("DELETE", "/shares/short", None),
    ]
}
