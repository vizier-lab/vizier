use std::sync::Arc;
use std::time::Duration;

use text_splitter::MarkdownSplitter;
use tokio::task::JoinHandle;
use twilight_http::Client;
use twilight_model::http::attachment::Attachment;
use twilight_model::id::Id;
use twilight_model::id::marker::ChannelMarker;

use crate::error::{VizierError, throw_vizier_error};

/// Discord's per-message content limit.
const MESSAGE_LIMIT: usize = 2000;

/// Discord's typing indicator lasts ~10s; refresh a bit before it expires.
const TYPING_REFRESH: Duration = Duration::from_secs(7);

/// Parse a raw snowflake into a typed id, rejecting 0 (which `Id::new` would panic on).
pub fn parse_id<T>(raw: u64, what: &str) -> Result<Id<T>, VizierError> {
    Id::new_checked(raw).ok_or_else(|| VizierError(format!("invalid discord {} id: {}", what, raw)))
}

pub async fn send_message(
    http: Arc<Client>,
    channel_id: Id<ChannelMarker>,
    content: String,
) -> Result<(), VizierError> {
    if content.len() < MESSAGE_LIMIT {
        if let Err(err) = http.create_message(channel_id).content(&content).await {
            tracing::error!("{:?}", err);
        }

        return Ok(());
    }

    let splitter = MarkdownSplitter::new(MESSAGE_LIMIT);
    let chunks = splitter
        .chunks(&content)
        .map(|s| s.to_string())
        .collect::<Vec<String>>();

    if let Err(err) = tokio::spawn(async move {
        for msg in chunks {
            if let Err(err) = http.create_message(channel_id).content(&msg).await {
                tracing::error!("{:?}", err);
            }
        }
    })
    .await
    {
        return throw_vizier_error("sending message", err);
    }

    Ok(())
}

pub async fn send_file(
    http: &Client,
    channel_id: Id<ChannelMarker>,
    filename: String,
    bytes: Vec<u8>,
) -> Result<(), VizierError> {
    let attachments = [Attachment::from_bytes(filename, bytes, 0)];
    if let Err(err) = http.create_message(channel_id).attachments(&attachments).await {
        return throw_vizier_error("sending attachment", err);
    }

    Ok(())
}

/// Keeps the "is typing..." indicator alive in a channel until dropped.
pub struct Typing(JoinHandle<()>);

impl Typing {
    pub fn start(http: Arc<Client>, channel_id: Id<ChannelMarker>) -> Self {
        Self(tokio::spawn(async move {
            loop {
                if let Err(err) = http.create_typing_trigger(channel_id).await {
                    tracing::warn!("discord typing trigger failed: {:?}", err);
                    return;
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

#[cfg(test)]
mod tests {
    // Guards against twilight picking up a rustls backend: rustls has both `ring` and
    // `aws-lc-rs` enabled in our tree, so building a client would panic at runtime.
    #[tokio::test]
    async fn twilight_clients_construct_without_panicking() {
        let _http = twilight_http::Client::new("x".into());
        let _shard = twilight_gateway::Shard::new(
            twilight_gateway::ShardId::ONE,
            "x".into(),
            twilight_gateway::Intents::empty(),
        );
    }
}
