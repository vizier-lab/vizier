use crate::common::{Vector, v};
use serde_json::json;

/// Sign-in, setup, profile, password, API keys, roles, users, notifications, overview, utils.
/// Only requests that leave the same state behind in both passes (refusals, reads, no-op
/// writes), because each vector is sent through the router and the dispatcher in turn.
pub fn vectors() -> Vec<Vector> {
    vec![
        v("GET", "/auth/setup-status", None),
        v("POST", "/auth/setup", Some(json!({ "username": "x", "password": "y" }))),
        v("POST", "/auth/login", Some(json!({ "username": "root", "password": "wrong" }))),
        v("POST", "/auth/login", Some(json!({ "username": "ghost", "password": "wrong" }))),
        v("GET", "/auth/users/me/profile", None),
        v("PUT", "/auth/users/me/profile", Some(json!({ "alias": ["al"] }))),
        v("POST", "/auth/change-password", Some(json!({ "current_password": "wrong", "new_password": "new" }))),
        v("GET", "/auth/api-keys", None),
        v("DELETE", "/auth/api-keys/nope", None),
        v("GET", "/auth/roles", None),
        v("GET", "/auth/roles/available-permissions", None),
        v("GET", "/auth/roles/nope", None),
        v("POST", "/auth/roles", Some(json!({ "name": "bad", "permissions": ["not:a:permission"] }))),
        v("PUT", "/auth/roles/nope", Some(json!({ "name": "n", "permissions": [] }))),
        v("DELETE", "/auth/roles/nope", None),
        v("GET", "/auth/users", None),
        v("POST", "/auth/users", Some(json!({ "username": "z", "password": "p", "role_id": "nope" }))),
        v("PUT", "/auth/users/nope", Some(json!({ "username": "z" }))),
        v("DELETE", "/auth/users/nope", None),
        v("GET", "/notifications", None),
        v("GET", "/notifications/count", None),
        v("POST", "/notifications/read-all", None),
        v("PUT", "/notifications/nope/read", None),
        v("GET", "/overview", None),
        v("GET", "/overview/activity", None),
        v("GET", "/utils/setup-state", None),
    ]
}
