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
    let contains_fence = content
        .lines()
        .any(|line| line.trim_start().starts_with(FENCE));
    if !contains_fence {
        return MarkdownSplitter::new(MESSAGE_LIMIT)
            .chunks(&content)
            .map(str::to_string)
            .collect();
    }

    let mut chunks = Vec::new();
    let mut chunk = String::with_capacity(MESSAGE_LIMIT);
    let mut opening_fence: Option<&str> = None;

    for line in content.split_inclusive('\n') {
        if line.trim_start().starts_with(FENCE) {
            if let Some(opening) = opening_fence {
                if chunk.chars().count() + line.chars().count() > MESSAGE_LIMIT {
                    finish_fenced_chunk(&mut chunks, &mut chunk);
                    reopen_fence(&mut chunk, opening);
                }
                chunk.push_str(line);
                opening_fence = None;
            } else {
                let closing_len = FENCE.len() + 1;
                if !chunk.is_empty()
                    && chunk.chars().count() + line.chars().count() + closing_len > MESSAGE_LIMIT
                {
                    chunks.push(std::mem::take(&mut chunk));
                }
                chunk.push_str(line);
                opening_fence = Some(line.trim_end_matches(['\r', '\n']));
            }
            continue;
        }

        let mut rest = line;
        while !rest.is_empty() {
            let closing_len = opening_fence.map(|_| FENCE.len() + 1).unwrap_or_default();
            let available = MESSAGE_LIMIT - chunk.chars().count() - closing_len;
            if available == 0 {
                if let Some(opening) = opening_fence {
                    finish_fenced_chunk(&mut chunks, &mut chunk);
                    reopen_fence(&mut chunk, opening);
                } else {
                    chunks.push(std::mem::take(&mut chunk));
                }
                continue;
            }

            let (prefix, remaining) = split_at_char_limit(rest, available);
            chunk.push_str(prefix);
            rest = remaining;
            if !rest.is_empty() {
                if let Some(opening) = opening_fence {
                    finish_fenced_chunk(&mut chunks, &mut chunk);
                    reopen_fence(&mut chunk, opening);
                } else {
                    chunks.push(std::mem::take(&mut chunk));
                }
            }
        }
    }

    if opening_fence.is_some() {
        finish_fenced_chunk(&mut chunks, &mut chunk);
    } else if !chunk.is_empty() {
        chunks.push(chunk);
    }
    chunks
}

fn split_at_char_limit(value: &str, limit: usize) -> (&str, &str) {
    value
        .char_indices()
        .nth(limit)
        .map_or((value, ""), |(index, _)| value.split_at(index))
}

fn finish_fenced_chunk(chunks: &mut Vec<String>, chunk: &mut String) {
    if !chunk.ends_with('\n') {
        chunk.push('\n');
    }
    chunk.push_str("```");
    chunks.push(std::mem::take(chunk));
}

fn reopen_fence(chunk: &mut String, opening: &str) {
    chunk.push_str(opening);
    chunk.push('\n');
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
            chunks.iter().all(|chunk| {
                let fences = chunk
                    .lines()
                    .filter(|line| line.trim_start().starts_with("```"))
                    .collect::<Vec<_>>();
                fences.len() == 2 && fences[0] == "```js" && fences[1] == "```"
            }),
            "each Discord message must contain a complete language-tagged code fence: {chunks:#?}"
        );
    }

    #[test]
    fn a_fence_language_never_becomes_code_block_content() {
        let content = format!(
            "use execute_python\n```js\n{{\n  \"code\": \"{}\"\n}}\n```",
            "x".repeat(MESSAGE_LIMIT * 2)
        );

        let chunks = message_chunks(content);

        assert!(
            chunks.iter().all(|chunk| !chunk.lines().any(|line| line == "js")),
            "the fence language must stay attached to its opening backticks: {chunks:#?}"
        );
    }
}
