//! Shared chat snapshots (`specs/055-shared-chat/contracts/share-api.md`).
//!
//! Vectors must not change state (the harness sends each twice), so every `POST /shares` here
//! is refused before anything is stored: `403` for a person who may not share (or an agent that
//! is not there), `400` for a document that fails its structural check.
//!
//! `GET /shares/{share_id}` has no vector on purpose: it is public, so it has no desktop twin
//! worth comparing, and it is listed in `pending.txt` with that reason. It is covered by
//! `tests/shares.rs` instead.
//!
//! The list (`GET /shares`) and revoke (`DELETE /shares/{share_id}`) vectors arrive with those
//! routes (T064).

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
    ]
}
