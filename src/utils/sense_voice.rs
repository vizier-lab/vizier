use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use bzip2::read::BzDecoder;
use futures::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use tar::Archive;

use crate::utils::build_path;

const SENSE_VOICE_RELEASE_BASE: &str =
    "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models";

pub fn sense_voice_models_dir(workspace: &str) -> PathBuf {
    build_path(workspace, &[".runtime", "models", "stt"])
}

pub fn sense_voice_model_dir(workspace: &str, model_id: &str) -> PathBuf {
    sense_voice_models_dir(workspace).join(model_id)
}

fn is_model_cached(model_dir: &std::path::Path) -> bool {
    if !model_dir.is_dir() {
        return false;
    }
    let has_onnx = std::fs::read_dir(model_dir)
        .ok()
        .and_then(|mut entries| {
            entries
                .any(|e| {
                    e.ok()
                        .is_some_and(|e| e.path().extension().is_some_and(|ext| ext == "onnx"))
                })
                .then_some(())
        })
        .is_some();
    let has_tokens = model_dir.join("tokens.txt").is_file();
    has_onnx && has_tokens
}

pub async fn sense_voice_prefetch_model(workspace: &str, model_id: &str) -> Result<PathBuf> {
    let model_dir = sense_voice_model_dir(workspace, model_id);

    if is_model_cached(&model_dir) {
        tracing::info!("sense_voice model already cached: {}", model_dir.display());
        return Ok(model_dir);
    }

    let url = format!("{}/{}.tar.bz2", SENSE_VOICE_RELEASE_BASE, model_id);
    tracing::info!("downloading sense_voice model from {}", url);

    let models_dir = sense_voice_models_dir(workspace);
    std::fs::create_dir_all(&models_dir)?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let response = client.get(&url).send().await.map_err(|e| {
        anyhow::anyhow!("failed to download sense_voice model '{}': {}", model_id, e)
    })?;

    if !response.status().is_success() {
        anyhow::bail!(
            "failed to download sense_voice model '{}': HTTP {}",
            model_id,
            response.status()
        );
    }

    let total_size = response.content_length().unwrap_or(0);

    let pb = ProgressBar::new(total_size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta}) {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );
    pb.set_message(format!("downloading {}", model_id));

    let archive_path = models_dir.join(format!("{}.tar.bz2", model_id));
    let mut file = std::fs::File::create(&archive_path)?;
    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| {
            anyhow::anyhow!("failed to download sense_voice model '{}': {}", model_id, e)
        })?;
        std::io::Write::write_all(&mut file, &chunk)?;
        downloaded += chunk.len() as u64;
        pb.set_position(downloaded);
    }

    pb.set_message("extracting...");
    tracing::info!("download complete, extracting to {}", models_dir.display());

    let file = std::fs::File::open(&archive_path)?;
    let decoder = BzDecoder::new(file);
    let mut archive = Archive::new(decoder);

    archive
        .unpack(&models_dir)
        .map_err(|e| anyhow::anyhow!("failed to extract sense_voice model: {}", e))?;

    // Clean up the archive file
    let _ = std::fs::remove_file(&archive_path);

    pb.finish_with_message(format!("sense_voice model {} ready", model_id));
    tracing::info!("sense_voice model extracted to {}", model_dir.display());

    Ok(model_dir)
}
