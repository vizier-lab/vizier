//! One `IntegrationKind` per kind, registered in `IntegrationKinds::builtin()`.

pub mod brave_search;
pub mod discord;
pub mod mcp;
pub mod telegram;

use serde_json::Value;

use crate::integrations::kind::{FieldType, KindField, string_setting};

/// The settings fields of a bot connection: its token.
pub(crate) fn token_fields() -> Vec<KindField> {
    vec![KindField::new("token", "Bot token", FieldType::Secret).required()]
}

pub(crate) fn validate_token(settings: &Value) -> Result<(), String> {
    match string_setting(settings, "token") {
        Some(_) => Ok(()),
        None => Err("a bot token is required".into()),
    }
}
