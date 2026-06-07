use std::path::PathBuf;

use anyhow::Result;
use hf_hub::{api::tokio::ApiBuilder, Repo, RepoType};
use indicatif::{ProgressBar, ProgressStyle};

use crate::utils::build_path;

fn model_dir(model_id: &str) -> String {
    model_id.replace('/', "--")
}

pub fn mistralrs_cache_dir(workspace: &str) -> PathBuf {
    build_path(workspace, &[".runtime", "models"])
}

pub fn mistralrs_model_dir(workspace: &str, model_id: &str) -> PathBuf {
    mistralrs_cache_dir(workspace).join(model_dir(model_id))
}

pub async fn mistralrs_prefetch_model(workspace: &str, model_id: &str) -> Result<()> {
    let cache_dir = mistralrs_model_dir(workspace, model_id);
    std::fs::create_dir_all(&cache_dir)?;

    let api = ApiBuilder::new()
        .with_cache_dir(cache_dir)
        .build()?;

    let repo = api.repo(Repo::new(model_id.to_string(), RepoType::Model));

    let info = repo.info().await?;
    let files: Vec<&str> = info
        .siblings
        .iter()
        .filter(|s| {
            let name = &s.rfilename;
            name.ends_with(".safetensors")
                || name == "tokenizer.json"
                || name == "config.json"
                || name == "tokenizer_config.json"
        })
        .map(|s| s.rfilename.as_str())
        .collect();

    if files.is_empty() {
        anyhow::bail!("no model files found for {}", model_id);
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
        repo.get(file).await?;
        pb.inc(1);
    }

    pb.finish_with_message(format!("model {} ready", model_id));
    Ok(())
}
