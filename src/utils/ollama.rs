use anyhow::Result;
use futures::StreamExt;

use crate::setup::{DownloadKind, SetupBoard};

/// Pull `model` on the Ollama server at `base_url`, reporting progress to the setup board
/// (the WebUI shows it; nothing is written to the console). The download only appears on
/// the board once bytes actually move, so a model the server already has is never labelled
/// as downloading.
///
/// `kind` is `Model` for an agent's chat model and `Embedding` for an embedding model, so
/// the UI can label the download.
pub async fn ollama_pull_model(
    base_url: &str,
    model: &str,
    setup: &SetupBoard,
    agent_id: &str,
    kind: DownloadKind,
) -> Result<()> {
    let pull_url = format!("{}/api/pull", base_url);
    let http_client = reqwest::Client::new();

    tracing::info!("Pulling Ollama model '{}'...", model);

    let resp = http_client
        .post(&pull_url)
        .json(&serde_json::json!({
            "name": model,
            "stream": true
        }))
        .send()
        .await;

    let resp = match resp {
        Ok(resp) => resp,
        Err(e) => {
            let error = format!("Failed to pull Ollama model '{model}': {e}");
            setup.download_failed(agent_id, kind, error.clone());
            anyhow::bail!(error);
        }
    };

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        let error = format!("Failed to pull Ollama model '{model}': {body}");
        setup.download_failed(agent_id, kind, error.clone());
        anyhow::bail!(error);
    }

    let mut stream = resp.bytes_stream();
    let mut buffer = Vec::new();
    let mut completed = 0u64;
    let mut total = 0u64;
    let mut started = false;

    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(chunk) => chunk,
            Err(e) => {
                let error = format!("Failed to pull Ollama model '{model}': {e}");
                setup.download_failed(agent_id, kind, error.clone());
                anyhow::bail!(error);
            }
        };
        buffer.extend_from_slice(&chunk);

        // Process all complete lines in the buffer
        while let Some(newline_pos) = buffer.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = buffer.drain(..=newline_pos).collect();
            let line_str = String::from_utf8_lossy(&line);
            let line_str = line_str.trim();

            if line_str.is_empty() {
                continue;
            }

            if let Ok(obj) = serde_json::from_str::<serde_json::Value>(line_str) {
                if let Some(t) = obj.get("total").and_then(|t| t.as_u64()) {
                    total = t;
                }
                if let Some(c) = obj.get("completed").and_then(|c| c.as_u64()) {
                    completed = c;
                }
                if !started && total > 0 && completed < total {
                    started = true;
                    setup.download_started(agent_id, kind, model);
                }
                if started && let Some(status) = obj.get("status").and_then(|s| s.as_str()) {
                    setup.download_progress(agent_id, kind, completed, total, status);
                }

                // Check for errors in the stream
                if let Some(error) = obj.get("error").and_then(|e| e.as_str()) {
                    let error = format!("Failed to pull Ollama model '{model}': {error}");
                    setup.download_failed(agent_id, kind, error.clone());
                    anyhow::bail!(error);
                }
            }
        }
    }

    // Process any remaining data in the buffer
    if !buffer.is_empty() {
        let line_str = String::from_utf8_lossy(&buffer);
        let line_str = line_str.trim();
        if !line_str.is_empty()
            && let Ok(obj) = serde_json::from_str::<serde_json::Value>(line_str)
            && let Some(error) = obj.get("error").and_then(|e| e.as_str())
        {
            let error = format!("Failed to pull Ollama model '{model}': {error}");
            setup.download_failed(agent_id, kind, error.clone());
            anyhow::bail!(error);
        }
    }

    setup.download_finished(agent_id, kind);
    tracing::info!("Ollama model '{}' is ready", model);
    Ok(())
}
