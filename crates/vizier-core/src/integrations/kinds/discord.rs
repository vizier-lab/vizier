use std::sync::Arc;

use serde_json::Value;
use twilight_model::id::{Id, marker::ChannelMarker};

use crate::{
    agents::tools::{VizierToolSet, discord::new_discord_tools},
    channels::{discord::DiscordChannelReader, readers::Reader},
    dependencies::VizierDependencies,
    integrations::{
        kind::{
            AgentToolContext, Contribution, ConversationNames, IntegrationKind, KindListing,
            NamesFuture, string_setting,
        },
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

    fn platform(&self) -> Option<ChannelPlatform> {
        Some(ChannelPlatform::Discord)
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

    fn conversation_names(&self, integration: &Integration, chat_id: &str) -> Option<NamesFuture> {
        let token = string_setting(&integration.settings, "token")?.to_string();
        let channel_id = Id::<ChannelMarker>::new_checked(chat_id.parse().ok()?)?;
        Some(Box::pin(async move {
            let http = twilight_http::Client::new(token);
            let channel = http.channel(channel_id).await.ok()?.model().await.ok()?;
            let space = match channel.guild_id {
                Some(guild_id) => http.guild(guild_id).await.ok()?.model().await.ok().map(|g| g.name),
                None => None,
            };
            let conversation = match (&channel.name, &channel.guild_id) {
                (Some(name), Some(_)) => Some(format!("#{name}")),
                (Some(name), None) => Some(name.clone()),
                (None, _) => Some("Direct message".to_string()),
            };
            Some(ConversationNames { space, conversation })
        }))
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
