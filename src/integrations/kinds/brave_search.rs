use serde_json::{Value, json};

use crate::{
    agents::tools::{
        VizierToolSet,
        brave_search::{BraveSearch, NewsOnlySearch, WebOnlySearch},
    },
    config::tools::BraveSearchConfig,
    integrations::kind::{
        AgentToolContext, Contribution, FieldType, IntegrationKind, KindField, KindListing,
        string_setting,
    },
    schema::{Integration, IntegrationFamily},
};

pub struct BraveSearchKind;

impl IntegrationKind for BraveSearchKind {
    fn id(&self) -> &'static str {
        "brave_search"
    }

    fn family(&self) -> IntegrationFamily {
        IntegrationFamily::Addon
    }

    fn listing(&self) -> KindListing {
        KindListing {
            name: "Brave Search",
            description: "Search the web and the news with Brave Search",
            icon: "brave_search",
            fields: vec![
                KindField::new("api_key", "API key", FieldType::Secret).required(),
                KindField::new("safesearch", "Safe search", FieldType::Bool).default(json!(true)),
            ],
        }
    }

    fn validate(&self, _name: &str, settings: &Value) -> Result<(), String> {
        match settings.get("safesearch") {
            None | Some(Value::Null) | Some(Value::Bool(_)) => Ok(()),
            Some(_) => Err("safesearch must be true or false".into()),
        }
    }

    fn contribute(&self, integration: &Integration, ctx: &AgentToolContext) -> Contribution {
        let Some(api_key) = string_setting(&integration.settings, "api_key") else {
            return Contribution::default();
        };
        let config = BraveSearchConfig {
            api_key: api_key.to_string(),
            safesearch: integration
                .settings
                .get("safesearch")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
        };
        let health = Some((ctx.health.clone(), integration.id.clone()));
        Contribution {
            native: VizierToolSet::new()
                .tool(BraveSearch::<WebOnlySearch>::new(&config, health.clone()))
                .tool(BraveSearch::<NewsOnlySearch>::new(&config, health)),
            mcp: None,
            addon: Some(self.id()),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn an_edit_without_the_key_keeps_it() {
        let stored = json!({"api_key": "BSA-secret-key-1234", "safesearch": true});
        assert_eq!(
            BraveSearchKind.redact(&stored),
            json!({"api_key": "••••1234", "safesearch": true})
        );
        assert_eq!(
            BraveSearchKind.merge_secrets(&stored, json!({"safesearch": false})),
            json!({"api_key": "BSA-secret-key-1234", "safesearch": false})
        );
        assert_eq!(BraveSearchKind.missing(&json!({"safesearch": true})), ["api_key"]);
        assert!(BraveSearchKind.validate("b", &json!({"safesearch": "yes"})).is_err());
    }
}
