//! Platform account linking (`specs/069-platform-account-linking/contracts/http-connections.md`).
//!
//! Vectors must not change state (each is sent twice), so no code here can be redeemed and every
//! delete finds nothing. The full flow is in `tests/platform_links.rs`.

use crate::common::{Vector, v};
use serde_json::json;

pub fn vectors() -> Vec<Vector> {
    vec![
        v("GET", "/auth/users/me/connections", None),
        // A well-formed code nobody was issued.
        v("POST", "/auth/users/me/connections/preview", Some(json!({ "code": "ZZZZ-ZZZZ" }))),
        v("POST", "/auth/users/me/connections", Some(json!({ "code": "ZZZZ-ZZZZ" }))),
        v("DELETE", "/auth/users/me/connections/discord", None),
        v("DELETE", "/auth/users/me/connections/telegram/unverified", None),
        // A hand-typed id is refused before anything is stored.
        v("PUT", "/auth/users/me/profile", Some(json!({ "discord_id": "1" }))),
    ]
}
