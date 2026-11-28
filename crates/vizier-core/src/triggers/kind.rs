//! The one trait every trigger kind implements (`specs/071-triggers/contracts/trigger-kind.md`).
//! Nothing outside a kind branches on a kind id: the webhook route calls [`TriggerKind::receive`],
//! the service asks [`TriggerKind::when_full`], and the listing builds the settings form.

use std::sync::Arc;

use serde_json::{Map, Value};

use crate::{
    config::TriggersConfig,
    integrations::{
        kind::{FieldType, KindListing},
        mask_secret,
    },
    schema::{Trigger, WhenFull},
    time::Timestamp,
};

/// A request pushed to a trigger's own address.
#[derive(Debug, Clone)]
pub struct PushRequest {
    /// Lower-cased names.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// The secret carried in the address, for senders that cannot set a header.
    pub path_secret: Option<String>,
    pub received_at: Timestamp,
}

impl PushRequest {
    pub fn header(&self, name: &str) -> Option<&str> {
        let name = name.to_ascii_lowercase();
        self.headers.iter().find(|(n, _)| *n == name).map(|(_, v)| v.as_str())
    }
}

/// What a kind makes of a pushed request.
#[derive(Debug, Clone, PartialEq)]
pub enum Intake {
    /// An event: the payload the agent and triage will see, and an optional delivery id.
    Fire { payload: Value, delivery_id: Option<String> },
    /// Authenticated, but not wanted (a filter): recorded, starts nothing.
    Ignore { reason: String, delivery_id: Option<String> },
    /// Not authenticated, or not acceptable. Answered exactly like an unknown trigger.
    Reject,
}

/// Why a pushed request was refused before it became an event, mapped to an HTTP status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushRefusal {
    TooLarge,
    UnsupportedType,
}

pub trait TriggerKind: Send + Sync {
    /// The `trigger.kind` value.
    fn id(&self) -> &'static str;

    /// The catalogue entry and settings form.
    fn listing(&self) -> KindListing;

    /// A message for the person when the settings cannot be saved.
    fn validate(&self, settings: &Value) -> Result<(), String>;

    /// Fills in what a new trigger needs, such as a generated secret. Returns what must be shown
    /// once and never again.
    fn prepare(&self, _settings: &mut Value) -> Option<String> {
        None
    }

    /// Replaces the secret and returns the new one, shown once. `None` for kinds with none.
    fn replace_secret(&self, _settings: &mut Value) -> Option<String> {
        None
    }

    /// The settings with every secret masked, for responses.
    fn redact(&self, settings: &Value) -> Value {
        mask_secret_fields(&self.listing(), settings)
    }

    /// `new` with the secrets a request left out, or sent back masked, taken from `old`.
    fn merge_secrets(&self, old: &Value, new: Value) -> Value {
        merge_secret_fields(&self.listing(), old, new)
    }

    /// Narrows what an agent wrote in the settings before they are saved, so the agent never
    /// grants itself tool access (FR-031). `previous` is what was saved before a change, and
    /// `turn_scope` is the calling turn's own scope, which nothing granted here may exceed.
    fn restrict_agent_settings(
        &self,
        _settings: &mut Value,
        _previous: Option<&Value>,
        _turn_scope: Option<&[String]>,
    ) {
    }

    /// A pushed request. Pull kinds reject everything.
    fn receive(&self, _trigger: &Trigger, _request: &PushRequest) -> Result<Intake, PushRefusal> {
        Ok(Intake::Reject)
    }

    /// Whether a request shows it comes from this trigger's sender, judged without its body:
    /// what may be told about a body too large to read (FR-022).
    fn authenticates_without_body(&self, _trigger: &Trigger, _request: &PushRequest) -> bool {
        false
    }

    /// Whether this kind receives pushed requests, and so needs the public-webhook permission.
    fn is_public(&self) -> bool {
        false
    }

    /// What a full queue does with a new event (FR-015).
    fn when_full(&self) -> WhenFull {
        WhenFull::DropOldest
    }

    /// The tool scope a trigger of this kind starts with when a person creates it. `None` is
    /// every tool. Triggers an agent creates always start with the narrow default (FR-030).
    fn default_scope(&self) -> Option<Vec<String>> {
        Some(crate::agents::tools::scope::narrow_default())
    }
}

/// Every trigger kind, registered once: the built-in ones, then those addons contribute.
pub struct TriggerKinds {
    kinds: Vec<Arc<dyn TriggerKind>>,
    /// Trigger kind id → the addon that contributed it.
    addons: std::collections::HashMap<String, String>,
}

impl TriggerKinds {
    pub fn new(kinds: Vec<Arc<dyn TriggerKind>>) -> Self {
        Self { kinds, addons: Default::default() }
    }

    /// The built-in kinds plus every kind an integration kind contributes, the addon way (FR-004).
    pub fn with_addons(
        integrations: &crate::integrations::IntegrationKinds,
        limits: &TriggersConfig,
    ) -> Self {
        let mut kinds = Self::builtin_with(limits);
        for integration in integrations.all() {
            for kind in integration.trigger_kinds() {
                kinds.add_from_addon(integration.id(), kind);
            }
        }
        kinds
    }

    /// Register a kind an addon contributes. A kind whose id is taken, or that is not named after
    /// its addon, is ignored with a warning.
    pub fn add_from_addon(&mut self, addon: &str, kind: Arc<dyn TriggerKind>) {
        let id = kind.id();
        if self.get(id).is_some() || !id.starts_with(&format!("{addon}_")) {
            tracing::warn!(addon, kind = id, "a trigger kind must be named <addon>_<name> and not taken; ignored");
            return;
        }
        self.addons.insert(id.to_string(), addon.to_string());
        self.kinds.push(kind);
    }

    /// The addon a kind needs on the agent, if it came from one (FR-005).
    pub fn addon_of(&self, kind: &str) -> Option<&str> {
        self.addons.get(kind).map(String::as_str)
    }

    /// The kinds that ship: `webhook` and `monitor` (FR-004).
    pub fn builtin() -> Self {
        Self::builtin_with(&TriggersConfig::default())
    }

    /// [`TriggerKinds::builtin`] under the operator's limits.
    pub fn builtin_with(limits: &TriggersConfig) -> Self {
        Self::new(vec![
            Arc::new(super::webhook::WebhookKind),
            Arc::new(super::monitor::MonitorKind { limits: limits.clone() }),
        ])
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn TriggerKind>> {
        self.kinds.iter().find(|k| k.id() == id).cloned()
    }

    pub fn all(&self) -> &[Arc<dyn TriggerKind>] {
        &self.kinds
    }
}

pub(crate) fn mask_secret_fields(listing: &KindListing, settings: &Value) -> Value {
    let mut out = settings.clone();
    let Some(map) = out.as_object_mut() else {
        return out;
    };
    for field in &listing.fields {
        if field.field_type == FieldType::Secret
            && let Some(Value::String(s)) = map.get(field.key)
        {
            let masked = mask_secret(s);
            map.insert(field.key.into(), Value::String(masked));
        }
    }
    out
}

pub(crate) fn merge_secret_fields(listing: &KindListing, old: &Value, new: Value) -> Value {
    let Value::Object(mut map) = new else {
        return new;
    };
    let empty = Map::new();
    let old = old.as_object().unwrap_or(&empty);
    for field in &listing.fields {
        if field.field_type != FieldType::Secret {
            continue;
        }
        match map.get(field.key) {
            None => {
                if let Some(value) = old.get(field.key) {
                    map.insert(field.key.into(), value.clone());
                }
            }
            Some(Value::Null) => {
                map.remove(field.key);
            }
            Some(Value::String(s)) if crate::integrations::kind::is_masked(s) => match old.get(field.key) {
                Some(value) => {
                    map.insert(field.key.into(), value.clone());
                }
                None => {
                    map.remove(field.key);
                }
            },
            Some(_) => {}
        }
    }
    Value::Object(map)
}

/// A new random secret: 64 hex characters from two random UUIDs (244 random bits).
pub(crate) fn new_secret() -> String {
    format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MailKind;

    impl TriggerKind for MailKind {
        fn id(&self) -> &'static str {
            "mail_new_message"
        }
        fn listing(&self) -> KindListing {
            KindListing { name: "New mail", description: "", icon: "mail", fields: vec![] }
        }
        fn validate(&self, _settings: &Value) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn an_addon_kind_is_registered_beside_the_built_in_ones() {
        let mut kinds = TriggerKinds::builtin();
        kinds.add_from_addon("mail", Arc::new(MailKind));
        assert!(kinds.get("mail_new_message").is_some());
        assert_eq!(kinds.addon_of("mail_new_message"), Some("mail"));
        assert_eq!(kinds.addon_of("webhook"), None);
        // Not named after its addon: ignored.
        let mut other = TriggerKinds::builtin();
        other.add_from_addon("office", Arc::new(MailKind));
        assert!(other.get("mail_new_message").is_none());
    }
}
