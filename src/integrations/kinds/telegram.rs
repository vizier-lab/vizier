use std::sync::Arc;

use serde_json::Value;

use crate::{
    agents::tools::{VizierToolSet, telegram::new_telegram_tools},
    channels::{readers::Reader, telegram::TelegramChannelReader},
    dependencies::VizierDependencies,
    integrations::{
        kind::{AgentToolContext, Contribution, IntegrationKind, KindListing, string_setting},
        kinds::{token_fields, validate_token},
    },
    schema::{ChannelPlatform, Integration, IntegrationFamily},
};

pub struct TelegramKind;

impl IntegrationKind for TelegramKind {
    fn id(&self) -> &'static str {
        "telegram"
    }

    fn family(&self) -> IntegrationFamily {
        IntegrationFamily::Channel
    }

    fn listing(&self) -> KindListing {
        KindListing {
            name: "Telegram",
            description: "Chat with the agent from a Telegram bot",
            icon: "telegram",
            fields: token_fields(),
        }
    }

    fn validate(&self, _name: &str, settings: &Value) -> Result<(), String> {
        validate_token(settings)
    }

    fn contribute(&self, integration: &Integration, ctx: &AgentToolContext) -> Contribution {
        let Some(token) = string_setting(&integration.settings, "token") else {
            return Contribution::default();
        };
        let (send, react, get) =
            new_telegram_tools(token.to_string(), ctx.agent_id.clone(), ctx.storage.clone());
        Contribution {
            native: VizierToolSet::new().tool(send).tool(react).tool(get),
            ..Default::default()
        }
    }

    fn reader(
        &self,
        integration: &Integration,
        agent_id: &str,
        deps: &VizierDependencies,
    ) -> Option<(ChannelPlatform, Reader)> {
        let token = string_setting(&integration.settings, "token")?;
        let reader = TelegramChannelReader::new(agent_id.into(), token.into(), deps.clone());
        Some((ChannelPlatform::Telegram, Arc::new(reader)))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn the_token_is_redacted_and_cleared_by_null() {
        let stored = json!({"token": "123456:telegram-token-wxyz"});
        assert_eq!(TelegramKind.redact(&stored), json!({"token": "••••wxyz"}));
        assert_eq!(TelegramKind.merge_secrets(&stored, json!({"token": "••••wxyz"})), stored);
        assert_eq!(TelegramKind.merge_secrets(&stored, json!({"token": null})), json!({}));
    }
}
