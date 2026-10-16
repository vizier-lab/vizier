use std::sync::Arc;

use serde_json::Value;

use crate::{
    agents::tools::{VizierToolSet, discord::new_discord_tools},
    channels::{discord::DiscordChannelReader, readers::Reader},
    dependencies::VizierDependencies,
    integrations::{
        kind::{AgentToolContext, Contribution, IntegrationKind, KindListing, string_setting},
        kinds::{token_fields, validate_token},
    },
    schema::{ChannelPlatform, Integration, IntegrationFamily},
};

pub struct DiscordKind;

impl IntegrationKind for DiscordKind {
    fn id(&self) -> &'static str {
        "discord"
    }

    fn family(&self) -> IntegrationFamily {
        IntegrationFamily::Channel
    }

    fn listing(&self) -> KindListing {
        KindListing {
            name: "Discord",
            description: "Chat with the agent from a Discord bot",
            icon: "discord",
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
        let (send, react, get, guild, channel, member) =
            new_discord_tools(token.to_string(), ctx.agent_id.clone(), ctx.storage.clone());
        Contribution {
            native: VizierToolSet::new()
                .tool(send)
                .tool(react)
                .tool(get)
                .tool(guild)
                .tool(channel)
                .tool(member),
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
        let reader = DiscordChannelReader::new(agent_id.into(), token.into(), deps.clone());
        Some((ChannelPlatform::Discord, Arc::new(reader)))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn the_token_is_redacted_and_kept_when_sent_back() {
        let stored = json!({"token": "MTIz.discord-token-abcd"});
        let redacted = DiscordKind.redact(&stored);
        assert_eq!(redacted, json!({"token": "••••abcd"}));
        assert_eq!(DiscordKind.merge_secrets(&stored, redacted), stored);
        assert_eq!(DiscordKind.merge_secrets(&stored, json!({})), stored);
        assert_eq!(
            DiscordKind.merge_secrets(&stored, json!({"token": "new-token-1234"})),
            json!({"token": "new-token-1234"})
        );
        assert_eq!(DiscordKind.missing(&json!({"token": ""})), ["token"]);
        assert!(DiscordKind.validate("bot", &json!({})).is_err());
    }
}
