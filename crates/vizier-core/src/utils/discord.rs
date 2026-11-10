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

fn message_chunks(content: String) -> Vec<String> {
    if content.chars().count() <= MESSAGE_LIMIT {
        return vec![content];
    }

    const FENCE: &str = "```";
    // A continuation can need an opening and closing fence, each with a newline.
    const FENCE_OVERHEAD: usize = 8;
    let contains_fence = content
        .lines()
        .any(|line| line.trim_start().starts_with(FENCE));
    let chunk_limit = if contains_fence {
        MESSAGE_LIMIT - FENCE_OVERHEAD
    } else {
        MESSAGE_LIMIT
    };
    let mut inside_fence = false;

    MarkdownSplitter::new(chunk_limit)
        .chunks(&content)
        .map(|raw| {
            let started_inside = inside_fence;
            for line in raw.lines() {
                if line.trim_start().starts_with(FENCE) {
                    inside_fence = !inside_fence;
                }
            }

            if !started_inside && !inside_fence {
                return raw.to_string();
            }

            let mut chunk = String::with_capacity(raw.len() + FENCE_OVERHEAD);
            if started_inside {
                chunk.push_str(FENCE);
                chunk.push('\n');
            }
            chunk.push_str(raw);
            if inside_fence {
                if !raw.ends_with('\n') {
                    chunk.push('\n');
                }
                chunk.push_str(FENCE);
            }
            chunk
        })
        .collect()
}

/// Post `content`, split at Discord's limit, and return the id of every message that was
/// posted so a reply can be linked to the history entry it renders. A chunk Discord rejects is
/// logged and skipped, as before.
pub async fn send_message(
    http: Arc<Client>,
    channel_id: Id<ChannelMarker>,
    content: String,
) -> Result<Vec<Id<MessageMarker>>, VizierError> {
    let chunks = message_chunks(content);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_fenced_messages_keep_every_chunk_balanced() {
        let code = "x".repeat(MESSAGE_LIMIT * 2);
        let content = format!("use execute_python\n```js\n{{\"code\":\"{code}\"}}\n```");

        let chunks = message_chunks(content);

        assert!(chunks.len() > 1);
        assert!(chunks.iter().all(|chunk| chunk.chars().count() <= MESSAGE_LIMIT));
        assert!(
            chunks
                .iter()
                .all(|chunk| chunk.lines().filter(|line| line.trim_start().starts_with("```")).count() % 2 == 0),
            "each Discord message must contain a complete code fence: {chunks:#?}"
        );
    }
}
