use teloxide::Bot;
use teloxide::prelude::*;
use teloxide::types::{MessageId, Recipient};

use crate::error::VizierError;

const MAX_MESSAGE_LENGTH: usize = 4096;

fn escape_markdown_v2(text: &str) -> String {
    let reserved = [
        '_', '[', ']', '(', ')', '~', '`', '+', '-', '=', '|', '{', '}', '.', '!',
    ];
    let mut escaped = String::with_capacity(text.len() * 2);
    for c in text.chars() {
        if reserved.contains(&c) {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

/// Post `content`, split at Telegram's limit, and return the id of every message that was
/// posted so a reply can be linked to the history entry it renders. A chunk Telegram rejects
/// is logged and skipped, as before.
pub async fn send_message<C, T>(
    bot: &Bot,
    recipient: C,
    content: T,
) -> Result<Vec<MessageId>, VizierError>
where
    C: Into<Recipient>,
    T: Into<String>,
{
    let escaped_content = escape_markdown_v2(&content.into());
    let recipient = recipient.into();

    let chunks: Vec<String> = if escaped_content.len() < MAX_MESSAGE_LENGTH {
        vec![escaped_content]
    } else {
        escaped_content
            .chars()
            .collect::<Vec<char>>()
            .chunks(MAX_MESSAGE_LENGTH)
            .map(|chunk| chunk.iter().collect())
            .collect()
    };

    let mut ids = Vec::with_capacity(chunks.len());
    for msg in chunks {
        match bot
            .parse_mode(teloxide::types::ParseMode::MarkdownV2)
            .send_message(recipient.clone(), msg)
            .await
        {
            Ok(sent) => ids.push(sent.id),
            Err(err) => tracing::error!("{:?}", err),
        }
    }

    Ok(ids)
}
