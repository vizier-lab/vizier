//! Platform account linking (`specs/069-platform-account-linking`).
//!
//! A person proves that a Discord or Telegram account is theirs: the bot gives that account a
//! one-time code ([`connect`]), and a signed-in person redeems it. Every request built from a
//! platform person is then stamped by [`sender::resolve`], the only place that decides who the
//! agent is told sent it.

pub mod code;
pub mod connect;
pub mod sender;
pub mod text;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::schema::ChannelPlatform;

/// The sender an agent reads for a platform account nobody has linked.
pub const UNREGISTERED_USER: &str = "Unregistered User";

/// Who a platform says sent something, built by a reader from the native event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformPerson {
    pub platform: ChannelPlatform,
    /// The platform's own user id. The only thing that identifies the account.
    pub id: String,
    /// The handle (`rin_dev`); a Telegram account may have none.
    pub username: Option<String>,
    /// The name shown beside messages (`Rin`).
    pub display_name: Option<String>,
}

impl PlatformPerson {
    /// What a per-account limit is keyed by. Never the sender label: every unlinked person shares
    /// that one.
    pub fn key(&self) -> String {
        format!("{}:{}", self.platform, self.id)
    }

    /// The platform identity carried in a request's metadata, linked or not.
    pub fn metadata_fields(&self) -> Map<String, Value> {
        let mut fields = Map::new();
        fields.insert("platform".into(), Value::String(self.platform.to_string()));
        fields.insert("platform_user_id".into(), Value::String(self.id.clone()));
        if let Some(username) = &self.username {
            fields.insert("platform_username".into(), Value::String(username.clone()));
        }
        if let Some(name) = &self.display_name {
            fields.insert("platform_display_name".into(), Value::String(name.clone()));
        }
        fields
    }
}

/// What a platform request is stamped with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformSender {
    /// The Vizier username of the linked person, or [`UNREGISTERED_USER`].
    pub label: String,
    pub verified: bool,
    pub person: PlatformPerson,
}
