use std::path::{Path, PathBuf};
use std::sync::Arc;

use sherpa_onnx::{
    GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsKittenModelConfig,
    OfflineTtsModelConfig,
};

use crate::schema::agent::TtsToolSettings;
use crate::utils::kitten::{kitten_model_dir, kitten_models_dir, kitten_prefetch_model};
use crate::{Result, VizierError};

use super::VizierTtsModel;

pub struct KittenTtsModel {
    tts: Arc<OfflineTts>,
    sample_rate: i32,
}

impl KittenTtsModel {
    pub async fn new(settings: &TtsToolSettings, workspace: &str) -> Result<Self> {
        let model_dir = resolve_model_dir(settings, workspace).await?;
        let config = build_config(&model_dir)?;

        let tts = tokio::task::spawn_blocking(move || {
            OfflineTts::create(&config)
                .ok_or_else(|| VizierError("failed to create kitten TTS engine".into()))
        })
        .await
        .map_err(|e| VizierError(e.to_string()))??;

        let sample_rate = tts.sample_rate();

        tracing::info!(
            "kitten TTS loaded from {} (sample_rate={})",
            model_dir.display(),
            sample_rate
        );

        Ok(Self {
            tts: Arc::new(tts),
            sample_rate,
        })
    }
}

#[async_trait::async_trait]
impl VizierTtsModel for KittenTtsModel {
    async fn generate_speech(&self, text: &str, voice: &str, speed: f32) -> Result<Vec<u8>> {
        let tts = self.tts.clone();
        let text = text.to_owned();
        let sid: i32 = voice.parse().unwrap_or(0);
        let sample_rate = self.sample_rate as u32;

        tokio::task::spawn_blocking(move || {
            let gen_config = GenerationConfig {
                speed,
                sid,
                ..Default::default()
            };
            let audio = tts
                .generate_with_config(&text, &gen_config, None::<fn(&[f32], f32) -> bool>)
                .ok_or_else(|| VizierError("kitten TTS returned no audio".into()))?;

            let samples = audio.samples();
            let spec = hound::WavSpec {
                channels: 1,
                sample_rate,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            };

            let mut buf = std::io::Cursor::new(Vec::new());
            {
                let mut writer = hound::WavWriter::new(&mut buf, spec)
                    .map_err(|e| VizierError(format!("wav writer: {e}")))?;
                for &s in samples {
                    let clamped = s.clamp(-1.0, 1.0);
                    let sample = (clamped * 32767.0) as i16;
                    writer
                        .write_sample(sample)
                        .map_err(|e| VizierError(format!("wav write: {e}")))?;
                }
                writer
                    .finalize()
                    .map_err(|e| VizierError(format!("wav finalize: {e}")))?;
            }

            Ok(buf.into_inner())
        })
        .await
        .map_err(|e| VizierError(e.to_string()))?
    }
}

async fn resolve_model_dir(settings: &TtsToolSettings, workspace: &str) -> Result<PathBuf> {
    let base = kitten_models_dir(workspace);

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
            if named.is_dir() && is_kitten_model_dir(&named) {
                return Ok(named);
            }

            if direct.is_dir() && is_kitten_model_dir(&direct) {
                return Ok(direct);
            }

            tracing::info!("auto-downloading kitten model '{}'", model);
            kitten_prefetch_model(workspace, model).await.map_err(|e| {
                VizierError(format!(
                    "failed to download kitten model '{}': {}",
                    model, e
                ))
            })
        }
        None => {
            if let Some(dir) = find_first_model_dir(&base) {
                return Ok(dir);
            }

            let default_model = "kitten-nano-en-v0_1-fp16";
            tracing::info!(
                "no kitten model found, auto-downloading default: '{}'",
                default_model
            );
            kitten_prefetch_model(workspace, default_model)
                .await
                .map_err(|e| {
                    VizierError(format!(
                        "failed to download default kitten model '{}': {}",
                        default_model, e
                    ))
                })
        }
    }
}

fn is_kitten_model_dir(dir: &Path) -> bool {
    has_onnx_file(dir) && dir.join("voices.bin").is_file() && dir.join("tokens.txt").is_file()
}

fn find_first_model_dir(base: &Path) -> Option<PathBuf> {
    if !base.is_dir() {
        return None;
    }
    for entry in std::fs::read_dir(base).ok()? {
        let entry = entry.ok()?;
        let path = entry.path();
        if path.is_dir() && is_kitten_model_dir(&path) {
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

fn build_config(model_dir: &Path) -> Result<OfflineTtsConfig> {
    let onnx_path = find_file(model_dir, "onnx")
        .ok_or_else(|| VizierError(format!("no .onnx file found in {}", model_dir.display())))?;

    let voices_path = model_dir.join("voices.bin");
    let voices_path = if voices_path.is_file() {
        Some(voices_path.to_string_lossy().into_owned())
    } else {
        None
    };

    let tokens_path = find_file(model_dir, "txt").or_else(|| find_file(model_dir, "tokens"));
    let data_dir = model_dir.join("espeak-ng-data");
    let data_dir = if data_dir.is_dir() {
        Some(data_dir.to_string_lossy().into_owned())
    } else {
        None
    };

    Ok(OfflineTtsConfig {
        model: OfflineTtsModelConfig {
            kitten: OfflineTtsKittenModelConfig {
                model: Some(onnx_path.to_string_lossy().into_owned()),
                voices: voices_path,
                tokens: tokens_path.map(|p| p.to_string_lossy().into_owned()),
                data_dir,
                ..Default::default()
            },
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
