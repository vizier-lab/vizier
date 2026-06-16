use std::path::PathBuf;

use anyhow::Result;
use bzip2::read::BzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use tar::Archive;

use crate::utils::build_path;

const KOKORO_RELEASE_BASE: &str =
    "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models";

pub fn kokoro_models_dir(workspace: &str) -> PathBuf {
    build_path(workspace, &["tts_models"])
}

pub fn kokoro_model_dir(workspace: &str, model_id: &str) -> PathBuf {
    kokoro_models_dir(workspace).join(model_id)
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
    let has_voices = model_dir.join("voices.bin").is_file();
    let has_data_dir = model_dir.join("espeak-ng-data").is_dir();
    has_onnx && has_tokens && has_voices && has_data_dir
}

pub async fn kokoro_prefetch_model(workspace: &str, model_id: &str) -> Result<PathBuf> {
    let model_dir = kokoro_model_dir(workspace, model_id);

    if is_model_cached(&model_dir) {
        tracing::info!("kokoro model already cached: {}", model_dir.display());
        return Ok(model_dir);
    }

    let url = format!("{}/{}.tar.bz2", KOKORO_RELEASE_BASE, model_id);
    tracing::info!("downloading kokoro model from {}", url);

    std::fs::create_dir_all(&model_dir)?;

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} [{elapsed}] {msg}")
            .unwrap(),
    );
    pb.set_message(format!("downloading {}", model_id));

    let response = reqwest::get(&url).await.map_err(|e| {
        pb.finish_and_clear();
        anyhow::anyhow!("failed to download kokoro model '{}': {}", model_id, e)
    })?;

    if !response.status().is_success() {
        pb.finish_and_clear();
        anyhow::bail!(
            "failed to download kokoro model '{}': HTTP {}",
            model_id,
            response.status()
        );
    }

    let bytes = response.bytes().await.map_err(|e| {
        pb.finish_and_clear();
        anyhow::anyhow!("failed to read kokoro model response: {}", e)
    })?;

    pb.set_message("extracting...");

    let models_dir = kokoro_models_dir(workspace);
    let cursor = std::io::Cursor::new(bytes);
    let decoder = BzDecoder::new(cursor);
    let mut archive = Archive::new(decoder);

    archive
        .unpack(&models_dir)
        .map_err(|e| anyhow::anyhow!("failed to extract kokoro model: {}", e))?;

    pb.finish_with_message(format!("kokoro model {} ready", model_id));
    tracing::info!("kokoro model extracted to {}", model_dir.display());

    Ok(model_dir)
}
