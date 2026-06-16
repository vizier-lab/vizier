use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sherpa_onnx::{
    GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsKokoroModelConfig,
    OfflineTtsModelConfig,
};

use crate::schema::agent::TtsToolSettings;
use crate::utils::kokoro::{kokoro_model_dir, kokoro_models_dir, kokoro_prefetch_model};
use crate::{Result, VizierError};

use super::VizierTtsModel;

fn kokoro_en_v0_19_voices() -> HashMap<&'static str, i32> {
    HashMap::from([
        ("af", 0),
        ("af_bella", 1),
        ("af_nicole", 2),
        ("af_sarah", 3),
        ("af_sky", 4),
        ("am_adam", 5),
        ("am_michael", 6),
        ("bf_emma", 7),
        ("bf_isabella", 8),
        ("bm_george", 9),
        ("bm_lewis", 10),
    ])
}

fn kokoro_multi_lang_v1_0_voices() -> HashMap<&'static str, i32> {
    HashMap::from([
        ("af_alloy", 0),
        ("af_aoede", 1),
        ("af_bella", 2),
        ("af_heart", 3),
        ("af_jessica", 4),
        ("af_kore", 5),
        ("af_nicole", 6),
        ("af_nova", 7),
        ("af_river", 8),
        ("af_sarah", 9),
        ("af_sky", 10),
        ("am_adam", 11),
        ("am_echo", 12),
        ("am_eric", 13),
        ("am_fenrir", 14),
        ("am_liam", 15),
        ("am_michael", 16),
        ("am_onyx", 17),
        ("am_puck", 18),
        ("am_santa", 19),
        ("bf_alice", 20),
        ("bf_emma", 21),
        ("bf_isabella", 22),
        ("bf_lily", 23),
        ("bm_daniel", 24),
        ("bm_fable", 25),
        ("bm_george", 26),
        ("bm_lewis", 27),
        ("ef_dora", 28),
        ("em_alex", 29),
        ("ff_siwis", 30),
        ("hf_alpha", 31),
        ("hf_beta", 32),
        ("hm_omega", 33),
        ("hm_psi", 34),
        ("if_sara", 35),
        ("im_nicola", 36),
        ("jf_alpha", 37),
        ("jf_gongitsune", 38),
        ("jf_nezumi", 39),
        ("jf_tebukuro", 40),
        ("jm_kumo", 41),
        ("pf_dora", 42),
        ("pm_alex", 43),
        ("pm_santa", 44),
        ("zf_xiaobei", 45),
        ("zf_xiaoni", 46),
        ("zf_xiaoxiao", 47),
        ("zf_xiaoyi", 48),
        ("zm_yunjian", 49),
        ("zm_yunxi", 50),
        ("zm_yunxia", 51),
        ("zm_yunyang", 52),
    ])
}

fn resolve_voice_id(model_id: &str, voice: &str) -> i32 {
    let voices = if model_id.contains("multi-lang") {
        kokoro_multi_lang_v1_0_voices()
    } else {
        kokoro_en_v0_19_voices()
    };

    if let Ok(id) = voice.parse::<i32>() {
        return id;
    }

    voices
        .get(voice)
        .copied()
        .unwrap_or(0)
}

fn extract_model_id(model_dir: &Path) -> String {
    model_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("kokoro-en-v0_19")
        .to_string()
}

pub struct KokoroTtsModel {
    tts: Arc<OfflineTts>,
    sample_rate: i32,
    model_id: String,
}

impl KokoroTtsModel {
    pub async fn new(settings: &TtsToolSettings, workspace: &str) -> Result<Self> {
        let model_dir = resolve_model_dir(settings, workspace).await?;
        let model_id = extract_model_id(&model_dir);
        let config = build_config(&model_dir)?;

        let tts = tokio::task::spawn_blocking(move || {
            OfflineTts::create(&config)
                .ok_or_else(|| VizierError("failed to create kokoro TTS engine".into()))
        })
        .await
        .map_err(|e| VizierError(e.to_string()))??;

        let sample_rate = tts.sample_rate();

        tracing::info!(
            "kokoro TTS loaded from {} (model={}, sample_rate={})",
            model_dir.display(),
            model_id,
            sample_rate
        );

        Ok(Self {
            tts: Arc::new(tts),
            sample_rate,
            model_id,
        })
    }
}

#[async_trait::async_trait]
impl VizierTtsModel for KokoroTtsModel {
    async fn generate_speech(&self, text: &str, voice: &str, speed: f32) -> Result<Vec<u8>> {
        let tts = self.tts.clone();
        let text = text.to_owned();
        let sid = resolve_voice_id(&self.model_id, voice);
        let sample_rate = self.sample_rate as u32;

        tokio::task::spawn_blocking(move || {
            let gen_config = GenerationConfig {
                speed,
                sid,
                ..Default::default()
            };
            let audio = tts
                .generate_with_config(&text, &gen_config, None::<fn(&[f32], f32) -> bool>)
                .ok_or_else(|| VizierError("kokoro TTS returned no audio".into()))?;

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
    let base = kokoro_models_dir(workspace);

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
            if named.is_dir() && is_kokoro_model_dir(&named) {
                return Ok(named);
            }

            if direct.is_dir() && is_kokoro_model_dir(&direct) {
                return Ok(direct);
            }

            tracing::info!("auto-downloading kokoro model '{}'", model);
            kokoro_prefetch_model(workspace, model)
                .await
                .map_err(|e| {
                    VizierError(format!(
                        "failed to download kokoro model '{}': {}",
                        model, e
                    ))
                })
        }
        None => {
            if let Some(dir) = find_first_model_dir(&base) {
                return Ok(dir);
            }

            let default_model = "kokoro-en-v0_19";
            tracing::info!(
                "no kokoro model found, auto-downloading default: '{}'",
                default_model
            );
            kokoro_prefetch_model(workspace, default_model)
                .await
                .map_err(|e| {
                    VizierError(format!(
                        "failed to download default kokoro model '{}': {}",
                        default_model, e
                    ))
                })
        }
    }
}

fn is_kokoro_model_dir(dir: &Path) -> bool {
    has_onnx_file(dir)
        && dir.join("voices.bin").is_file()
        && dir.join("tokens.txt").is_file()
        && dir.join("espeak-ng-data").is_dir()
}

fn find_first_model_dir(base: &Path) -> Option<PathBuf> {
    if !base.is_dir() {
        return None;
    }
    for entry in std::fs::read_dir(base).ok()? {
        let entry = entry.ok()?;
        let path = entry.path();
        if path.is_dir() && is_kokoro_model_dir(&path) {
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

    let dict_dir = model_dir.join("dict");
    let dict_dir = if dict_dir.is_dir() {
        Some(dict_dir.to_string_lossy().into_owned())
    } else {
        None
    };

    let lexicon_files: Vec<String> = std::fs::read_dir(model_dir)
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.path()
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.starts_with("lexicon") && n.ends_with(".txt"))
                        .unwrap_or(false)
                })
                .map(|e| e.path().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();

    let lexicon = if lexicon_files.is_empty() {
        None
    } else {
        Some(lexicon_files.join(","))
    };

    Ok(OfflineTtsConfig {
        model: OfflineTtsModelConfig {
            kokoro: OfflineTtsKokoroModelConfig {
                model: Some(onnx_path.to_string_lossy().into_owned()),
                voices: voices_path,
                tokens: tokens_path.map(|p| p.to_string_lossy().into_owned()),
                data_dir,
                dict_dir,
                lexicon,
                lang: Some("en".into()),
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
