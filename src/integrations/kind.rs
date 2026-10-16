//! The one trait every integration kind implements (research R4). Tool build, reader start,
//! redaction and the Addons catalogue all go through it; none of them branch on a kind id.

use std::sync::Arc;

use serde::Serialize;
use serde_json::{Map, Value};

use crate::{
    agents::{mcp::McpServer, tools::VizierToolSet},
    channels::readers::Reader,
    dependencies::VizierDependencies,
    integrations::{IntegrationHealth, mask_secret},
    schema::{AgentId, ChannelPlatform, Integration, IntegrationFamily},
    storage::VizierStorage,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    /// Redacted in every response and kept when a request leaves it out or sends it back
    /// redacted.
    Secret,
    String,
    Bool,
    StringList,
    /// A map whose every value is a secret (MCP `env`).
    SecretMap,
    /// One of `options`.
    Select,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct KindField {
    pub key: &'static str,
    pub label: &'static str,
    #[serde(rename = "type")]
    pub field_type: FieldType,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Object>)]
    pub default: Option<Value>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<&'static str>,
}

impl KindField {
    pub fn new(key: &'static str, label: &'static str, field_type: FieldType) -> Self {
        Self { key, label, field_type, required: false, default: None, options: vec![] }
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    pub fn default(mut self, value: Value) -> Self {
        self.default = Some(value);
        self
    }

    pub fn options(mut self, options: Vec<&'static str>) -> Self {
        self.options = options;
        self
    }
}

/// What the catalogue shows for a kind; for an addon, its tile. The description also
/// describes an addon's tool category in the system prompt.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct KindListing {
    pub name: &'static str,
    pub description: &'static str,
    /// An embedded icon, served from `/integrations/kinds/{id}/icon`.
    pub icon: &'static str,
    pub fields: Vec<KindField>,
}

/// What one integration adds to one agent's tools.
#[derive(Clone, Default)]
pub struct Contribution {
    pub native: VizierToolSet,
    /// One MCP server, keyed by the integration's name.
    pub mcp: Option<(String, Arc<McpServer>)>,
    /// Set for an addon: its tools are listed under this category instead of their own.
    pub addon: Option<&'static str>,
}

/// What a kind needs to build tools for one agent.
#[derive(Clone)]
pub struct AgentToolContext {
    pub agent_id: AgentId,
    pub storage: Arc<VizierStorage>,
    pub health: IntegrationHealth,
}

pub trait IntegrationKind: Send + Sync {
    /// The `integration.kind` value: `discord`, `telegram`, `brave_search`, `mcp`.
    fn id(&self) -> &'static str;

    fn family(&self) -> IntegrationFamily;

    fn listing(&self) -> KindListing;

    /// A message for the person when `name`/`settings` can't be saved.
    fn validate(&self, name: &str, settings: &Value) -> Result<(), String>;

    /// Required settings that are missing or empty. An integration with any is `incomplete`
    /// and contributes nothing.
    fn missing(&self, settings: &Value) -> Vec<String> {
        self.listing()
            .fields
            .iter()
            .filter(|field| field.required && is_empty(settings.get(field.key)))
            .map(|field| field.key.to_string())
            .collect()
    }

    /// The settings with every secret masked, for API responses.
    fn redact(&self, settings: &Value) -> Value {
        let mut out = settings.clone();
        let Some(map) = out.as_object_mut() else {
            return out;
        };
        for field in self.listing().fields {
            match field.field_type {
                FieldType::Secret => {
                    if let Some(Value::String(s)) = map.get(field.key) {
                        let masked = mask_secret(s);
                        map.insert(field.key.into(), Value::String(masked));
                    }
                }
                FieldType::SecretMap => {
                    if let Some(Value::Object(secrets)) = map.get_mut(field.key) {
                        for value in secrets.values_mut() {
                            if let Value::String(s) = value {
                                *value = Value::String(mask_secret(s));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// `new` with the secrets a request left out, or sent back redacted, taken from `old`.
    /// A secret sent as `null` is cleared.
    fn merge_secrets(&self, old: &Value, new: Value) -> Value {
        let Value::Object(mut map) = new else {
            return new;
        };
        let empty = Map::new();
        let old = old.as_object().unwrap_or(&empty);
        for field in self.listing().fields {
            match field.field_type {
                FieldType::Secret => match map.get(field.key) {
                    None => {
                        if let Some(value) = old.get(field.key) {
                            map.insert(field.key.into(), value.clone());
                        }
                    }
                    Some(Value::Null) => {
                        map.remove(field.key);
                    }
                    Some(Value::String(s)) if is_masked(s) => match old.get(field.key) {
                        Some(value) => {
                            map.insert(field.key.into(), value.clone());
                        }
                        None => {
                            map.remove(field.key);
                        }
                    },
                    Some(_) => {}
                },
                FieldType::SecretMap => match map.get_mut(field.key) {
                    None => {
                        if let Some(value) = old.get(field.key) {
                            map.insert(field.key.into(), value.clone());
                        }
                    }
                    Some(Value::Object(secrets)) => {
                        let previous = old.get(field.key).and_then(|v| v.as_object());
                        secrets.retain(|key, value| match value {
                            Value::String(s) if is_masked(s) => {
                                match previous.and_then(|p| p.get(key)) {
                                    Some(kept) => {
                                        *value = kept.clone();
                                        true
                                    }
                                    None => false,
                                }
                            }
                            Value::Null => false,
                            _ => true,
                        });
                    }
                    Some(_) => {}
                },
                _ => {}
            }
        }
        Value::Object(map)
    }

    /// The tools this integration adds to one agent. Called only for an enabled integration
    /// with nothing `missing`.
    fn contribute(&self, integration: &Integration, ctx: &AgentToolContext) -> Contribution;

    /// The reader for a channel connection attached to `agent_id`, and the platform its
    /// health is reported as.
    fn reader(
        &self,
        _integration: &Integration,
        _agent_id: &str,
        _deps: &VizierDependencies,
    ) -> Option<(ChannelPlatform, Reader)> {
        None
    }
}

fn is_empty(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => true,
        Some(Value::String(s)) => s.trim().is_empty(),
        Some(_) => false,
    }
}

/// A value `redact` produced, which a client sends back to mean "unchanged".
pub fn is_masked(s: &str) -> bool {
    s.starts_with(crate::integrations::MASK)
}

/// A string setting, if present and not empty.
pub fn string_setting<'a>(settings: &'a Value, key: &str) -> Option<&'a str> {
    settings
        .get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
}
