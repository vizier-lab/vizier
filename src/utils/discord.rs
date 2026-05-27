use std::sync::Arc;

use serenity::all::{ChannelId, Http};
use text_splitter::MarkdownSplitter;

use crate::error::{VizierError, throw_vizier_error};

pub async fn send_message(
    http: Arc<Http>,
    channel_id: &ChannelId,
    content: String,
) -> Result<(), VizierError> {
    if content.len() < 2000 {
        let channel_id = channel_id.clone();
        let content = content.clone();
        if let Err(err) = channel_id.say(&http, content.clone()).await {
            tracing::error!("{:?}", err);
        }

        return Ok(());
    }

    let splitter = MarkdownSplitter::new(2000);
    let content = content.clone();
    let chunks = splitter
        .chunks(&content)
        .into_iter()
        .map(|s| s.to_string())
        .collect::<Vec<String>>();

    let channel_id = channel_id.clone();
    if let Err(err) = tokio::spawn(async move {
        for msg in chunks.clone() {
            if let Err(err) = channel_id.say(&http, msg).await {
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
