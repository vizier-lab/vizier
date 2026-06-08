use std::path::{Path, PathBuf};
use std::sync::Arc;

use sherpa_onnx::{
    OfflineRecognizer, OfflineRecognizerConfig, OfflineSenseVoiceModelConfig,
};

use crate::schema::agent::SttToolSettings;
use crate::stt::decode_to_pcm_f32;
use crate::utils::sense_voice::{sense_voice_model_dir, sense_voice_models_dir, sense_voice_prefetch_model};
use crate::{Result, VizierError};

pub struct SenseVoiceSttModel {
    recognizer: Arc<OfflineRecognizer>,
}

impl SenseVoiceSttModel {
    pub async fn new(settings: &SttToolSettings, workspace: &str) -> Result<Self> {
        let model_dir = resolve_model_dir(settings, workspace).await?;
        let config = build_config(&model_dir)?;

        let recognizer = tokio::task::spawn_blocking(move || {
            OfflineRecognizer::create(&config).ok_or_else(|| {
                VizierError("failed to create sherpa-onnx STT recognizer".into())
            })
        })
        .await
        .map_err(|e| VizierError(e.to_string()))??;

        log::info!(
            "sense_voice STT loaded from {}",
            model_dir.display()
        );

        Ok(Self {
            recognizer: Arc::new(recognizer),
        })
    }
}

#[async_trait::async_trait]
impl crate::stt::VizierSttModel for SenseVoiceSttModel {
    async fn transcribe(
        &self,
        audio: &[u8],
        filename: &str,
        language: Option<&str>,
    ) -> Result<String> {
        let recognizer = self.recognizer.clone();
        let audio = audio.to_vec();
        let filename = filename.to_owned();
        let language = language.map(|s| s.to_owned());

        tokio::task::spawn_blocking(move || {
            let (samples, sample_rate) = decode_to_pcm_f32(&audio, &filename)?;

            let stream = recognizer.create_stream();
            stream.accept_waveform(sample_rate, &samples);
            recognizer.decode(&stream);

            let result = stream
                .get_result()
                .ok_or_else(|| VizierError("STT returned no result".into()))?;

            Ok(result.text)
        })
        .await
        .map_err(|e| VizierError(e.to_string()))?
    }
}

async fn resolve_model_dir(settings: &SttToolSettings, workspace: &str) -> Result<PathBuf> {
    let base = sense_voice_models_dir(workspace);

    match &settings.model {
        Some(model) => {
            let direct = PathBuf::from(model);
            if direct.is_file() && direct.extension().is_some_and(|e| e == "onnx") {
                return Ok(direct
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .to_path_buf());
            }

            let named = base.join(model);
            if named.is_dir() && has_onnx_file(&named) {
                return Ok(named);
            }

            if direct.is_dir() && has_onnx_file(&direct) {
                return Ok(direct);
            }

            log::info!("auto-downloading sense_voice model '{}'", model);
            sense_voice_prefetch_model(workspace, model)
                .await
                .map_err(|e| {
                    VizierError(format!(
                        "failed to download sense_voice model '{}': {}",
                        model, e
                    ))
                })
        }
        None => {
            if let Some(dir) = find_first_model_dir(&base) {
                return Ok(dir);
            }

            let default_model = "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17";
            log::info!(
                "no STT model found, auto-downloading default: '{}'",
                default_model
            );
            sense_voice_prefetch_model(workspace, default_model)
                .await
                .map_err(|e| {
                    VizierError(format!(
                        "failed to download default sense_voice model '{}': {}",
                        default_model, e
                    ))
                })
        }
    }
}

fn find_first_model_dir(base: &Path) -> Option<PathBuf> {
    if !base.is_dir() {
        return None;
    }
    for entry in std::fs::read_dir(base).ok()? {
        let entry = entry.ok()?;
        let path = entry.path();
        if path.is_dir() && has_onnx_file(&path) {
            return Some(path);
        }
    }
    None
}

fn has_onnx_file(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .ok()
        .and_then(|mut entries| {
            entries
                .any(|e| {
                    e.ok()
                        .is_some_and(|e| e.path().extension().is_some_and(|ext| ext == "onnx"))
                })
                .then_some(())
        })
        .is_some()
}

fn build_config(model_dir: &Path) -> Result<OfflineRecognizerConfig> {
    let onnx_path = find_file(model_dir, "onnx")
        .ok_or_else(|| VizierError(format!("no .onnx file found in {}", model_dir.display())))?;

    let tokens_path = find_file(model_dir, "txt").or_else(|| find_file(model_dir, "tokens"));

    Ok(OfflineRecognizerConfig {
        model_config: sherpa_onnx::OfflineModelConfig {
            sense_voice: OfflineSenseVoiceModelConfig {
                model: Some(onnx_path.to_string_lossy().into_owned()),
                language: Some("auto".into()),
                use_itn: true,
            },
            tokens: tokens_path.map(|p| p.to_string_lossy().into_owned()),
            num_threads: 2,
            debug: false,
            ..Default::default()
        },
        ..Default::default()
    })
}

fn find_file(dir: &Path, extension: &str) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok().and_then(|entries| {
        entries
            .filter_map(|e| e.ok())
            .find(|e| e.path().extension().is_some_and(|ext| ext == extension))
            .map(|e| e.path())
    })
}
