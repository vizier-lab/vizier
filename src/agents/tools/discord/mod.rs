use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serenity::all::{ChannelId, Http, MessageId};

use crate::agents::tools::VizierTool;
use crate::error::{VizierError, throw_vizier_error};

pub fn new_discord_tools(
    discord_token: String,
) -> (SendDiscordMessage, ReactDiscordMessage, GetDiscordMessage) {
    let http = Arc::new(Http::new(&discord_token));

    (
        SendDiscordMessage { http: http.clone() },
        ReactDiscordMessage { http: http.clone() },
        GetDiscordMessage { http: http.clone() },
    )
}

pub struct SendDiscordMessage {
    http: Arc<Http>,
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
    type Output = ();

    fn name() -> String {
        "discord_send_message".to_string()
    }

    fn description(&self) -> String {
        "send a discord message to a channel, avoid using this when user interact with you directly from discord".into()
    }

    async fn call(&self, args: Self::Input) -> anyhow::Result<Self::Output, VizierError> {
        let response = crate::utils::discord::send_message(
            self.http.clone(),
            &ChannelId::new(args.channel_id),
            args.content,
        )
        .await;

        match response {
            Ok(()) => Ok(()),
            Err(err) => throw_vizier_error("discord_send_message ", err),
        }
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
    type Output = ();

    fn name() -> String {
        "discord_react_message".to_string()
    }

    fn description(&self) -> String {
        "emoji react to a discord message".into()
    }

    async fn call(&self, args: Self::Input) -> anyhow::Result<Self::Output, VizierError> {
        let channel = ChannelId::new(args.channel_id);
        let message_id = MessageId::new(args.message_id);

        let response = channel.message(self.http.clone(), message_id).await;
        if let Err(err) = response {
            return throw_vizier_error("discord_react_message ", err);
        }

        let message = response.unwrap();
        let response = message.react(self.http.clone(), args.emoji).await;

        match response {
            Ok(_) => Ok(()),
            Err(err) => throw_vizier_error("discord_react_message ", err),
        }
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

    async fn call(&self, args: Self::Input) -> anyhow::Result<Self::Output, VizierError> {
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
