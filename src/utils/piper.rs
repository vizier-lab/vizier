use std::path::PathBuf;

use anyhow::Result;
use hf_hub::{api::tokio::ApiBuilder, Repo, RepoType};
use indicatif::{ProgressBar, ProgressStyle};

use crate::utils::build_path;

pub fn piper_models_dir(workspace: &str) -> PathBuf {
    build_path(workspace, &["tts_models"])
}

pub fn piper_model_dir(workspace: &str, model_id: &str) -> PathBuf {
    piper_models_dir(workspace).join(model_id)
}

fn hf_cache_dir(workspace: &str) -> PathBuf {
    build_path(workspace, &[".runtime", "hf_cache"])
}

fn hf_repo_for_model(model_id: &str) -> String {
    format!("csukuangfj/vits-piper-{}", model_id)
}

fn is_model_cached(model_dir: &PathBuf) -> bool {
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

pub async fn piper_prefetch_model(workspace: &str, model_id: &str) -> Result<PathBuf> {
    let model_dir = piper_model_dir(workspace, model_id);

    if is_model_cached(&model_dir) {
        log::info!("piper model already cached: {}", model_dir.display());
        return Ok(model_dir);
    }

    let repo_id = hf_repo_for_model(model_id);
    log::info!("downloading piper model from {}", repo_id);

    std::fs::create_dir_all(&model_dir)?;

    let cache_dir = hf_cache_dir(workspace);
    std::fs::create_dir_all(&cache_dir)?;

    let api = ApiBuilder::new().with_cache_dir(cache_dir).build()?;

    let repo = api.repo(Repo::new(repo_id.clone(), RepoType::Model));

    let info = repo.info().await.map_err(|e| {
        anyhow::anyhow!(
            "failed to fetch model info from {}: {}",
            repo_id,
            e
        )
    })?;

    let files: Vec<String> = info
        .siblings
        .iter()
        .filter(|s| {
            let name = &s.rfilename;
            name.ends_with(".onnx")
                || name.ends_with(".onnx.json")
                || name == "tokens.txt"
                || name.starts_with("espeak-ng-data/")
        })
        .map(|s| s.rfilename.clone())
        .collect();

    if files.is_empty() {
        anyhow::bail!("no model files found in {}", repo_id);
    }

    let pb = ProgressBar::new(files.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} files {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );
    pb.set_message(format!("downloading {}", model_id));

    for file in &files {
        pb.set_message(format!("downloading {}", file));
        let cached_path = repo.get(file).await.map_err(|e| {
            pb.finish_and_clear();
            anyhow::anyhow!("failed to download {}: {}", file, e)
        })?;

        let dest = model_dir.join(file);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::copy(&cached_path, &dest).map_err(|e| {
            anyhow::anyhow!(
                "failed to copy {} -> {}: {}",
                cached_path.display(),
                dest.display(),
                e
            )
        })?;

        pb.inc(1);
    }

    pb.finish_with_message(format!("piper model {} ready", model_id));
    log::info!("piper model downloaded to {}", model_dir.display());

    Ok(model_dir)
}
