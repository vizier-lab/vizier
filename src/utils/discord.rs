use std::sync::Arc;
use std::time::Duration;

use text_splitter::MarkdownSplitter;
use tokio::task::JoinHandle;
use twilight_http::Client;
use twilight_model::http::attachment::Attachment;
use twilight_model::id::Id;
use twilight_model::id::marker::{ChannelMarker, MessageMarker};

use crate::error::{VizierError, throw_vizier_error};

/// Discord's per-message content limit.
const MESSAGE_LIMIT: usize = 2000;

/// Discord's typing indicator lasts ~10s; refresh a bit before it expires.
const TYPING_REFRESH: Duration = Duration::from_secs(7);

/// Parse a raw snowflake into a typed id, rejecting 0 (which `Id::new` would panic on).
pub fn parse_id<T>(raw: u64, what: &str) -> Result<Id<T>, VizierError> {
    Id::new_checked(raw).ok_or_else(|| VizierError(format!("invalid discord {} id: {}", what, raw)))
}

/// Post `content`, split at Discord's limit, and return the id of every message that was
/// posted so a reply can be linked to the history entry it renders. A chunk Discord rejects is
/// logged and skipped, as before.
pub async fn send_message(
    http: Arc<Client>,
    channel_id: Id<ChannelMarker>,
    content: String,
) -> Result<Vec<Id<MessageMarker>>, VizierError> {
    let chunks = if content.len() < MESSAGE_LIMIT {
        vec![content]
    } else {
        MarkdownSplitter::new(MESSAGE_LIMIT)
            .chunks(&content)
            .map(|s| s.to_string())
            .collect::<Vec<String>>()
    };

    match tokio::spawn(async move {
        let mut ids = Vec::with_capacity(chunks.len());
        for msg in chunks {
            match http.create_message(channel_id).content(&msg).await {
                Ok(response) => match response.model().await {
                    Ok(message) => ids.push(message.id),
                    Err(err) => tracing::warn!("discord message sent but unreadable: {:?}", err),
                },
                Err(err) => tracing::error!("{:?}", err),
            }
        }
        ids
    })
    .await
    {
        Ok(ids) => Ok(ids),
        Err(err) => throw_vizier_error("sending message", err),
    }
}

pub async fn send_file(
    http: &Client,
    channel_id: Id<ChannelMarker>,
    filename: String,
    bytes: Vec<u8>,
) -> Result<Id<MessageMarker>, VizierError> {
    let attachments = [Attachment::from_bytes(filename, bytes, 0)];
    let response = match http.create_message(channel_id).attachments(&attachments).await {
        Ok(response) => response,
        Err(err) => return throw_vizier_error("sending attachment", err),
    };
    match response.model().await {
        Ok(message) => Ok(message.id),
        Err(err) => throw_vizier_error("reading sent attachment", err),
    }
}

/// Keeps the "is typing..." indicator alive in a channel until dropped.
pub struct Typing(JoinHandle<()>);

impl Typing {
    pub fn start(http: Arc<Client>, channel_id: Id<ChannelMarker>) -> Self {
        Self(tokio::spawn(async move {
            loop {
                if let Err(err) = http.create_typing_trigger(channel_id).await {
                    tracing::warn!("discord typing trigger failed: {:?}", err);
                }
                tokio::time::sleep(TYPING_REFRESH).await;
            }
        }))
    }
}

impl Drop for Typing {
    fn drop(&mut self) {
        self.0.abort();
    }
}
