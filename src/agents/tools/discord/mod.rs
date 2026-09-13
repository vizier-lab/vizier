use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serenity::all::{Channel, ChannelId, GuildId, Http, MessageId, Role, UserId};

use crate::agents::tools::{ToolContext, VizierTool};
use crate::error::{VizierError, throw_vizier_error};
use crate::schema::{AgentId, TopicId, VizierChannelId, VizierResponse, VizierResponseContent, VizierSession};
use crate::storage::{VizierStorage, history::HistoryStorage, state::StateStorage};

/// Maximum number of channels/roles rendered before collapsing the rest into a trailing count.
const LIST_TRUNCATE_LIMIT: usize = 50;

fn format_truncated_list<T>(items: &[T], limit: usize, label: impl Fn(&T) -> String) -> String {
    let shown = items.iter().take(limit).map(label).collect::<Vec<_>>().join(", ");
    if items.len() > limit {
        format!("{}, ...and {} more", shown, items.len() - limit)
    } else {
        shown
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct ChannelState {
    active_topic: Option<TopicId>,
}

pub fn new_discord_tools(
    discord_token: String,
    agent_id: AgentId,
    storage: Arc<VizierStorage>,
) -> (
    SendDiscordMessage,
    ReactDiscordMessage,
    GetDiscordMessage,
    GetDiscordGuildInfo,
    GetDiscordChannelInfo,
    GetDiscordMemberInfo,
) {
    let http = Arc::new(Http::new(&discord_token));

    (
        SendDiscordMessage { http: http.clone(), agent_id: agent_id.clone(), storage: storage.clone() },
        ReactDiscordMessage { http: http.clone() },
        GetDiscordMessage { http: http.clone() },
        GetDiscordGuildInfo { http: http.clone() },
        GetDiscordChannelInfo { http: http.clone() },
        GetDiscordMemberInfo { http: http.clone() },
    )
}

pub struct SendDiscordMessage {
    http: Arc<Http>,
    agent_id: AgentId,
    storage: Arc<VizierStorage>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct SendDiscoedMessageArgs {
    #[schemars(description = "id of target discord channel")]
    channel_id: u64,

    #[schemars(description = "content of the message")]
    content: String,
}

#[async_trait::async_trait]
impl VizierTool for SendDiscordMessage {
    type Input = SendDiscoedMessageArgs;
    type Output = String;

    fn name() -> String {
        "discord_send_message".to_string()
    }

    fn description(&self) -> String {
        "send a discord message to a channel, avoid using this when user interact with you directly from discord".into()
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> anyhow::Result<Self::Output, VizierError> {
        let channel_id = args.channel_id;
        let content = args.content.clone();

        crate::utils::discord::send_message(
            self.http.clone(),
            &ChannelId::new(channel_id),
            args.content,
        )
        .await
        .map_err(|err| VizierError(err.to_string()))?;

        let channel = VizierChannelId::DiscordChanel(channel_id);
        let key = format!("{}__{}", self.agent_id, channel.to_slug());
        let topic_id = if let Ok(Some(value)) = self.storage.get_state(key).await {
            let state: ChannelState = serde_json::from_value(value).unwrap_or(ChannelState { active_topic: None });
            state.active_topic
        } else {
            None
        };

        let session = VizierSession(self.agent_id.clone(), channel, topic_id);
        let response = VizierResponse {
            timestamp: Utc::now(),
            content: VizierResponseContent::Message { content, stats: None },
            attachments: vec![],
        };
        self.storage
            .save_session_history(session, crate::schema::SessionHistoryContent::Response(response))
            .await
            .map_err(|e| VizierError(e.to_string()))?;

        Ok(format!("Message sent to channel {}", channel_id))
    }
}

pub struct ReactDiscordMessage {
    http: Arc<Http>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ReactDiscoedMessageArgs {
    #[schemars(description = "id of the target discord channel")]
    channel_id: u64,

    #[schemars(description = "id of the target discord message")]
    message_id: u64,

    #[schemars(description = "an emoji")]
    emoji: char,
}

#[async_trait::async_trait]
impl VizierTool for ReactDiscordMessage {
    type Input = ReactDiscoedMessageArgs;
    type Output = String;

    fn name() -> String {
        "discord_react_message".to_string()
    }

    fn description(&self) -> String {
        "emoji react to a discord message".into()
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> anyhow::Result<Self::Output, VizierError> {
        let channel = ChannelId::new(args.channel_id);
        let message_id = MessageId::new(args.message_id);

        let message = channel
            .message(self.http.clone(), message_id)
            .await
            .map_err(|err| VizierError(err.to_string()))?;

        message
            .react(self.http.clone(), args.emoji)
            .await
            .map_err(|err| VizierError(err.to_string()))?;

        Ok(format!("Reacted with {} to message {}", args.emoji, args.message_id))
    }
}

pub struct GetDiscordMessage {
    http: Arc<Http>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct GetDiscordMessageArgs {
    #[schemars(description = "id of the target discord channel")]
    channel_id: u64,

    #[schemars(description = "id of the target discord message")]
    message_id: u64,
}

#[async_trait::async_trait]
impl VizierTool for GetDiscordMessage {
    type Input = GetDiscordMessageArgs;
    type Output = String;

    fn name() -> String {
        "discord_get_message_by_id".to_string()
    }

    fn description(&self) -> String {
        "get message by message id".into()
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> anyhow::Result<Self::Output, VizierError> {
        let channel = ChannelId::new(args.channel_id);
        let message_id = MessageId::new(args.message_id);

        let response = channel.message(self.http.clone(), message_id).await;

        match response {
            Ok(message) => Ok(format!(
                "{}: {}",
                message.author.display_name(),
                message.content
            )),
            Err(err) => throw_vizier_error("discord_react_message ", err),
        }
    }
}

pub struct GetDiscordGuildInfo {
    http: Arc<Http>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct GetDiscordGuildInfoArgs {
    #[schemars(description = "id of the target discord server (guild)")]
    guild_id: u64,
}

#[async_trait::async_trait]
impl VizierTool for GetDiscordGuildInfo {
    type Input = GetDiscordGuildInfoArgs;
    type Output = String;

    fn name() -> String {
        "discord_get_guild_info".to_string()
    }

    fn description(&self) -> String {
        "get information about a discord server (guild), including its name, owner, member count, and its channels and roles".into()
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> anyhow::Result<Self::Output, VizierError> {
        let guild_id = GuildId::new(args.guild_id);

        let guild = match self.http.get_guild_with_counts(guild_id).await {
            Ok(guild) => guild,
            Err(err) => return throw_vizier_error(&format!("discord_get_guild_info guild {}", args.guild_id), err),
        };

        let channels = match self.http.get_channels(guild_id).await {
            Ok(channels) => channels,
            Err(err) => return throw_vizier_error(&format!("discord_get_guild_info guild {}", args.guild_id), err),
        };

        let member_count = guild
            .approximate_member_count
            .map(|count| count.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let roles: Vec<&Role> = guild.roles.values().collect();

        Ok(format!(
            "Guild \"{}\" (id {})\nOwner: user {}\nCreated: {}\nApproximate members: {}\nChannels ({}): {}\nRoles ({}): {}",
            guild.name,
            guild.id,
            guild.owner_id,
            guild.id.created_at(),
            member_count,
            channels.len(),
            format_truncated_list(&channels, LIST_TRUNCATE_LIMIT, |c| format!("#{}", c.name)),
            roles.len(),
            format_truncated_list(&roles, LIST_TRUNCATE_LIMIT, |r| r.name.clone()),
        ))
    }
}

pub struct GetDiscordChannelInfo {
    http: Arc<Http>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct GetDiscordChannelInfoArgs {
    #[schemars(description = "id of the target discord channel")]
    channel_id: u64,
}

#[async_trait::async_trait]
impl VizierTool for GetDiscordChannelInfo {
    type Input = GetDiscordChannelInfoArgs;
    type Output = String;

    fn name() -> String {
        "discord_get_channel_info".to_string()
    }

    fn description(&self) -> String {
        "get information about a discord channel or direct message, including its name/type, topic, and (for a direct message) its participant".into()
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> anyhow::Result<Self::Output, VizierError> {
        let channel = match self.http.get_channel(ChannelId::new(args.channel_id)).await {
            Ok(channel) => channel,
            Err(err) => return throw_vizier_error(&format!("discord_get_channel_info channel {}", args.channel_id), err),
        };

        match channel {
            Channel::Guild(guild_channel) => {
                let topic = guild_channel.topic.unwrap_or_else(|| "none".to_string());
                let parent = guild_channel
                    .parent_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "none".to_string());

                Ok(format!(
                    "Channel \"#{}\" (id {})\nType: {:?}\nGuild: {}\nParent category: {}\nTopic: {}",
                    guild_channel.name, guild_channel.id, guild_channel.kind, guild_channel.guild_id, parent, topic
                ))
            }
            Channel::Private(private_channel) => {
                let recipient = &private_channel.recipient;
                let display_name = recipient.global_name.as_deref().unwrap_or(&recipient.name);

                Ok(format!(
                    "Direct Message (id {})\nParticipant: {} ({})",
                    private_channel.id, recipient.name, display_name
                ))
            }
            _ => Err(VizierError(format!(
                "discord_get_channel_info channel {}: unsupported channel type",
                args.channel_id
            ))),
        }
    }
}

pub struct GetDiscordMemberInfo {
    http: Arc<Http>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct GetDiscordMemberInfoArgs {
    #[schemars(description = "id of the discord server (guild) the member belongs to")]
    guild_id: u64,

    #[schemars(description = "id of the target member/user")]
    user_id: u64,
}

#[async_trait::async_trait]
impl VizierTool for GetDiscordMemberInfo {
    type Input = GetDiscordMemberInfoArgs;
    type Output = String;

    fn name() -> String {
        "discord_get_member_info".to_string()
    }

    fn description(&self) -> String {
        "get information about a member of a discord server, including display name, username, join date, and roles".into()
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> anyhow::Result<Self::Output, VizierError> {
        let member = match self
            .http
            .get_member(GuildId::new(args.guild_id), UserId::new(args.user_id))
            .await
        {
            Ok(member) => member,
            Err(err) => {
                return throw_vizier_error(
                    &format!("discord_get_member_info user {} in guild {}", args.user_id, args.guild_id),
                    err,
                );
            }
        };

        let joined = member
            .joined_at
            .map(|ts| ts.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let roles = format_truncated_list(&member.roles, LIST_TRUNCATE_LIMIT, |role_id| role_id.to_string());

        Ok(format!(
            "Member {} (nick: {:?}) in guild {}\nUsername: {}\nJoined: {}\nRoles: {}",
            member.display_name(),
            member.nick,
            args.guild_id,
            member.user.name,
            joined,
            roles,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::format_truncated_list;

    #[test]
    fn format_truncated_list_under_limit_renders_unchanged() {
        let items = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let result = format_truncated_list(&items, 5, |s| s.clone());
        assert_eq!(result, "a, b, c");
    }

    #[test]
    fn format_truncated_list_over_limit_truncates_with_count() {
        let items: Vec<String> = (0..10).map(|n| n.to_string()).collect();
        let result = format_truncated_list(&items, 3, |s| s.clone());
        assert_eq!(result, "0, 1, 2, ...and 7 more");
    }
}
