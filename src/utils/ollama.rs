use anyhow::Result;
use futures::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};

pub async fn ollama_pull_model(base_url: &str, model: &str) -> Result<()> {
    let pull_url = format!("{}/api/pull", base_url);
    let http_client = reqwest::Client::new();

    log::info!("Pulling Ollama model '{}'...", model);

    let resp = http_client
        .post(&pull_url)
        .json(&serde_json::json!({
            "name": model,
            "stream": true
        }))
        .send()
        .await;

    let resp = resp?;

    if !resp.status().is_success() {
        anyhow::bail!(
            "Failed to pull Ollama model '{}': {}",
            model,
            resp.text().await.unwrap_or_default()
        );
    }

    let pb = ProgressBar::new(0);
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta}) {msg}",
        )
        .unwrap()
        .progress_chars("#>-"),
    );
    pb.set_message(format!("Pulling '{}'", model));

    let mut stream = resp.bytes_stream();
    let mut buffer = Vec::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
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
                if let Some(status) = obj.get("status").and_then(|s| s.as_str()) {
                    pb.set_message(status.to_string());
                }

                if let Some(total) = obj.get("total").and_then(|t| t.as_u64()) {
                    if total > 0 && pb.length() != Some(total) {
                        pb.set_length(total);
                    }
                }

                if let Some(completed) = obj.get("completed").and_then(|c| c.as_u64()) {
                    pb.set_position(completed);
                }

                // Check for errors in the stream
                if let Some(error) = obj.get("error").and_then(|e| e.as_str()) {
                    pb.finish_and_clear();
                    anyhow::bail!("Failed to pull Ollama model '{}': {}", model, error);
                }
            }
        }
    }

    // Process any remaining data in the buffer
    if !buffer.is_empty() {
        let line_str = String::from_utf8_lossy(&buffer);
        let line_str = line_str.trim();
        if !line_str.is_empty() {
            if let Ok(obj) = serde_json::from_str::<serde_json::Value>(line_str) {
                if let Some(error) = obj.get("error").and_then(|e| e.as_str()) {
                    pb.finish_and_clear();
                    anyhow::bail!("Failed to pull Ollama model '{}': {}", model, error);
                }
            }
        }
    }

    pb.finish_with_message(format!("Ollama model '{}' is ready", model));
    log::info!("Ollama model '{}' is ready", model);
    Ok(())
}
