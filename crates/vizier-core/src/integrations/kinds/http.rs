//! The built-in `http` addon (the addons redesign): fetch a webpage and call HTTP APIs. Both tools were
//! per-agent switches before; they now travel together as one addon with no settings.

use serde_json::Value;

use crate::{
    agents::tools::{VizierToolSet, fetch::FetchWebpage, http_client::HttpClient},
    integrations::kind::{AgentToolContext, Contribution, IntegrationKind, KindListing},
    schema::{Integration, IntegrationFamily},
};

pub const ID: &str = "http";
/// The name of the integration created for an agent.
pub const DEFAULT_NAME: &str = "HTTP";

pub struct HttpKind;

impl IntegrationKind for HttpKind {
    fn id(&self) -> &'static str {
        ID
    }

    fn family(&self) -> IntegrationFamily {
        IntegrationFamily::Addon
    }

    fn listing(&self) -> KindListing {
        KindListing {
            name: "HTTP",
            description: "Fetch webpages and call HTTP APIs, so the agent can read and post to the web",
            icon: "http",
            fields: vec![],
        }
    }

    /// No settings: `{}` and null are both fine.
    fn validate(&self, _name: &str, settings: &Value) -> Result<(), String> {
        match settings {
            Value::Null => Ok(()),
            Value::Object(map) if map.is_empty() => Ok(()),
            _ => Err("the HTTP addon has no settings".into()),
        }
    }

    fn contribute(&self, _integration: &Integration, _ctx: &AgentToolContext) -> Contribution {
        Contribution {
            native: VizierToolSet::new().tool(FetchWebpage).tool(HttpClient),
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
    fn it_has_no_settings() {
        assert!(HttpKind.listing().fields.is_empty());
        assert!(HttpKind.validate("http", &json!({})).is_ok());
        assert!(HttpKind.validate("http", &Value::Null).is_ok());
        assert!(HttpKind.validate("http", &json!({"x": 1})).is_err());
    }
}
