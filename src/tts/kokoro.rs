use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use indicatif::{ProgressBar, ProgressStyle};
use sherpa_onnx::{
    GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsKokoroModelConfig,
    OfflineTtsModelConfig,
};

use crate::tts::VizierTtsModel;
use crate::utils::build_path;
use crate::{Result, VizierError};

const MODEL_BASE_URL: &str =
    "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models";

fn en_v0_19_voices() -> HashMap<String, i32> {
    let mut m = HashMap::new();
    m.insert("af".into(), 0);
    m.insert("af_bella".into(), 1);
    m.insert("af_nicole".into(), 2);
    m.insert("af_sarah".into(), 3);
    m.insert("af_sky".into(), 4);
    m.insert("am_adam".into(), 5);
    m.insert("am_michael".into(), 6);
    m.insert("bf_emma".into(), 7);
    m.insert("bf_isabella".into(), 8);
    m.insert("bm_george".into(), 9);
    m.insert("bm_lewis".into(), 10);
    m
}

fn multi_lang_v1_0_voices() -> HashMap<String, i32> {
    let mut m = HashMap::new();
    m.insert("af_alloy".into(), 0);
    m.insert("af_aoede".into(), 1);
    m.insert("af_bella".into(), 2);
    m.insert("af_heart".into(), 3);
    m.insert("af_jessica".into(), 4);
    m.insert("af_kore".into(), 5);
    m.insert("af_nicole".into(), 6);
    m.insert("af_nova".into(), 7);
    m.insert("af_river".into(), 8);
    m.insert("af_sarah".into(), 9);
    m.insert("af_sky".into(), 10);
    m.insert("am_adam".into(), 11);
    m.insert("am_echo".into(), 12);
    m.insert("am_eric".into(), 13);
    m.insert("am_fenrir".into(), 14);
    m.insert("am_liam".into(), 15);
    m.insert("am_michael".into(), 16);
    m.insert("am_onyx".into(), 17);
    m.insert("am_puck".into(), 18);
    m.insert("am_santa".into(), 19);
    m.insert("bf_alice".into(), 20);
    m.insert("bf_emma".into(), 21);
    m.insert("bf_isabella".into(), 22);
    m.insert("bf_lily".into(), 23);
    m.insert("bm_daniel".into(), 24);
    m.insert("bm_fable".into(), 25);
    m.insert("bm_george".into(), 26);
    m.insert("bm_lewis".into(), 27);
    m.insert("ef_dora".into(), 28);
    m.insert("em_alex".into(), 29);
    m.insert("ff_siwis".into(), 30);
    m.insert("hf_alpha".into(), 31);
    m.insert("hf_beta".into(), 32);
    m.insert("hm_omega".into(), 33);
    m.insert("hm_psi".into(), 34);
    m.insert("if_sara".into(), 35);
    m.insert("im_nicola".into(), 36);
    m.insert("jf_alpha".into(), 37);
    m.insert("jf_gongitsune".into(), 38);
    m.insert("jf_nezumi".into(), 39);
    m.insert("jf_tebukuro".into(), 40);
    m.insert("jm_kumo".into(), 41);
    m.insert("pf_dora".into(), 42);
    m.insert("pm_alex".into(), 43);
    m.insert("pm_santa".into(), 44);
    m.insert("zf_xiaobei".into(), 45);
    m.insert("zf_xiaoni".into(), 46);
    m.insert("zf_xiaoxiao".into(), 47);
    m.insert("zf_xiaoyi".into(), 48);
    m.insert("zm_yunjian".into(), 49);
    m.insert("zm_yunxi".into(), 50);
    m.insert("zm_yunxia".into(), 51);
    m.insert("zm_yunyang".into(), 52);
    m
}

fn multi_lang_v1_1_voices() -> HashMap<String, i32> {
    let mut m = HashMap::new();
    m.insert("af_maple".into(), 0);
    m.insert("af_sol".into(), 1);
    m.insert("bf_vale".into(), 2);
    m.insert("zf_001".into(), 3);
    m.insert("zf_002".into(), 4);
    m.insert("zf_003".into(), 5);
    m.insert("zf_004".into(), 6);
    m.insert("zf_005".into(), 7);
    m.insert("zf_006".into(), 8);
    m.insert("zf_007".into(), 9);
    m.insert("zf_008".into(), 10);
    m.insert("zf_017".into(), 11);
    m.insert("zf_018".into(), 12);
    m.insert("zf_019".into(), 13);
    m.insert("zf_021".into(), 14);
    m.insert("zf_022".into(), 15);
    m.insert("zf_023".into(), 16);
    m.insert("zf_024".into(), 17);
    m.insert("zf_026".into(), 18);
    m.insert("zf_027".into(), 19);
    m.insert("zf_028".into(), 20);
    m.insert("zf_032".into(), 21);
    m.insert("zf_036".into(), 22);
    m.insert("zf_038".into(), 23);
    m.insert("zf_039".into(), 24);
    m.insert("zf_040".into(), 25);
    m.insert("zf_042".into(), 26);
    m.insert("zf_043".into(), 27);
    m.insert("zf_044".into(), 28);
    m.insert("zf_046".into(), 29);
    m.insert("zf_047".into(), 30);
    m.insert("zf_048".into(), 31);
    m.insert("zf_049".into(), 32);
    m.insert("zf_051".into(), 33);
    m.insert("zf_059".into(), 34);
    m.insert("zf_060".into(), 35);
    m.insert("zf_067".into(), 36);
    m.insert("zf_070".into(), 37);
    m.insert("zf_071".into(), 38);
    m.insert("zf_072".into(), 39);
    m.insert("zf_073".into(), 40);
    m.insert("zf_074".into(), 41);
    m.insert("zf_075".into(), 42);
    m.insert("zf_076".into(), 43);
    m.insert("zf_077".into(), 44);
    m.insert("zf_078".into(), 45);
    m.insert("zf_079".into(), 46);
    m.insert("zf_083".into(), 47);
    m.insert("zf_084".into(), 48);
    m.insert("zf_085".into(), 49);
    m.insert("zf_086".into(), 50);
    m.insert("zf_087".into(), 51);
    m.insert("zf_088".into(), 52);
    m.insert("zf_090".into(), 53);
    m.insert("zf_092".into(), 54);
    m.insert("zf_093".into(), 55);
    m.insert("zf_094".into(), 56);
    m.insert("zf_099".into(), 57);
    m.insert("zm_009".into(), 58);
    m.insert("zm_010".into(), 59);
    m.insert("zm_011".into(), 60);
    m.insert("zm_012".into(), 61);
    m.insert("zm_013".into(), 62);
    m.insert("zm_014".into(), 63);
    m.insert("zm_015".into(), 64);
    m.insert("zm_016".into(), 65);
    m.insert("zm_020".into(), 66);
    m.insert("zm_025".into(), 67);
    m.insert("zm_029".into(), 68);
    m.insert("zm_030".into(), 69);
    m.insert("zm_031".into(), 70);
    m.insert("zm_033".into(), 71);
    m.insert("zm_034".into(), 72);
    m.insert("zm_035".into(), 73);
    m.insert("zm_037".into(), 74);
    m.insert("zm_041".into(), 75);
    m.insert("zm_045".into(), 76);
    m.insert("zm_050".into(), 77);
    m.insert("zm_052".into(), 78);
    m.insert("zm_053".into(), 79);
    m.insert("zm_054".into(), 80);
    m.insert("zm_055".into(), 81);
    m.insert("zm_056".into(), 82);
    m.insert("zm_057".into(), 83);
    m.insert("zm_058".into(), 84);
    m.insert("zm_061".into(), 85);
    m.insert("zm_062".into(), 86);
    m.insert("zm_063".into(), 87);
    m.insert("zm_064".into(), 88);
    m.insert("zm_065".into(), 89);
    m.insert("zm_066".into(), 90);
    m.insert("zm_068".into(), 91);
    m.insert("zm_069".into(), 92);
    m.insert("zm_080".into(), 93);
    m.insert("zm_081".into(), 94);
    m.insert("zm_082".into(), 95);
    m.insert("zm_089".into(), 96);
    m.insert("zm_091".into(), 97);
    m.insert("zm_095".into(), 98);
    m.insert("zm_096".into(), 99);
    m.insert("zm_097".into(), 100);
    m.insert("zm_098".into(), 101);
    m.insert("zm_100".into(), 102);
    m
}

fn resolve_voice_id(model_name: &str, voice: &str) -> i32 {
    let map = match model_name {
        "kokoro-en-v0_19" => en_v0_19_voices(),
        "kokoro-multi-lang-v1_0" => multi_lang_v1_0_voices(),
        "kokoro-multi-lang-v1_1" => multi_lang_v1_1_voices(),
        _ => en_v0_19_voices(),
    };

    if let Some(&id) = map.get(voice) {
        return id;
    }

    voice.parse::<i32>().unwrap_or(0)
}

pub struct KokoroTtsModel {
    model_name: String,
    model_dir: PathBuf,
    tts: Arc<parking_lot::Mutex<Option<OfflineTts>>>,
}

impl KokoroTtsModel {
    pub fn new(model_name: String, workspace: &str) -> Self {
        let model_dir = build_path(workspace, &[".runtime", "tts", "kokoro", &model_name]);
        Self {
            model_name,
            model_dir,
            tts: Arc::new(parking_lot::Mutex::new(None)),
        }
    }

    async fn ensure_model(&self) -> Result<()> {
        let model_onnx = self.model_dir.join("model.onnx");
        if model_onnx.exists() {
            return Ok(());
        }

        std::fs::create_dir_all(&self.model_dir)
            .map_err(|e| VizierError(format!("create kokoro model dir: {e}")))?;

        let archive_name = format!("{}.tar.bz2", self.model_name);
        let url = format!("{MODEL_BASE_URL}/{archive_name}");
        let archive_path = self.model_dir.parent().unwrap().join(&archive_name);

        tracing::info!("Downloading Kokoro model: {url}");

        let pb = ProgressBar::new(0);
        pb.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})",
            )
            .unwrap()
            .progress_chars("#>-"),
        );

        let response = reqwest::get(&url)
            .await
            .map_err(|e| VizierError(format!("download kokoro model: {e}")))?;

        let total_size = response.content_length().unwrap_or(0);
        pb.set_length(total_size);

        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        use futures::StreamExt;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| VizierError(format!("download chunk: {e}")))?;
            bytes.extend_from_slice(&chunk);
            pb.inc(chunk.len() as u64);
        }
        pb.finish_with_message("download complete");

        std::fs::write(&archive_path, &bytes)
            .map_err(|e| VizierError(format!("write archive: {e}")))?;

        tracing::info!("Extracting Kokoro model to {}", self.model_dir.display());

        let archive_file = std::fs::File::open(&archive_path)
            .map_err(|e| VizierError(format!("open archive: {e}")))?;
        let decoder = bzip2::read::BzDecoder::new(archive_file);
        let mut archive = tar::Archive::new(decoder);
        archive.set_overwrite(true);
        archive
            .unpack(self.model_dir.parent().unwrap())
            .map_err(|e| VizierError(format!("extract archive: {e}")))?;

        let _ = std::fs::remove_file(&archive_path);

        tracing::info!("Kokoro model ready at {}", self.model_dir.display());
        Ok(())
    }

    async fn ensure_tts(&self) -> Result<parking_lot::MutexGuard<'_, Option<OfflineTts>>> {
        {
            let lock = self.tts.lock();
            if lock.is_some() {
                return Ok(lock);
            }
        }

        self.ensure_model().await?;

        let model_onnx = self.model_dir.join("model.onnx");
        let voices_bin = self.model_dir.join("voices.bin");
        let tokens_txt = self.model_dir.join("tokens.txt");
        let espeak_data = self.model_dir.join("espeak-ng-data");

        let lexicon = match self.model_name.as_str() {
            "kokoro-multi-lang-v1_0" | "kokoro-multi-lang-v1_1" => Some(
                format!(
                    "{},{}",
                    self.model_dir.join("lexicon-us-en.txt").display(),
                    self.model_dir.join("lexicon-zh.txt").display()
                ),
            ),
            _ => None,
        };

        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                kokoro: OfflineTtsKokoroModelConfig {
                    model: Some(model_onnx.to_string_lossy().into_owned()),
                    voices: Some(voices_bin.to_string_lossy().into_owned()),
                    tokens: Some(tokens_txt.to_string_lossy().into_owned()),
                    data_dir: Some(espeak_data.to_string_lossy().into_owned()),
                    lexicon,
                    length_scale: 1.0,
                    ..Default::default()
                },
                num_threads: 2,
                debug: false,
                ..Default::default()
            },
            ..Default::default()
        };

        let tts = OfflineTts::create(&config)
            .ok_or_else(|| VizierError("failed to create Kokoro TTS engine".into()))?;

        let mut lock = self.tts.lock();
        *lock = Some(tts);

        Ok(lock)
    }
}

#[async_trait::async_trait]
impl VizierTtsModel for KokoroTtsModel {
    async fn generate_speech(&self, text: &str, voice: &str, speed: f32) -> Result<Vec<u8>> {
        let lock = self.ensure_tts().await?;
        let tts = lock.as_ref().ok_or_else(|| VizierError("Kokoro TTS not initialized".into()))?;

        let sid = resolve_voice_id(&self.model_name, voice);
        let num_speakers = tts.num_speakers();
        let sid = if sid >= num_speakers {
            tracing::warn!(
                "Voice ID {} exceeds max speakers ({}) for model {}, using 0",
                sid,
                num_speakers,
                self.model_name
            );
            0
        } else {
            sid
        };

        let gen_config = GenerationConfig {
            sid,
            speed,
            ..Default::default()
        };

        let audio = tts
            .generate_with_config::<fn(&[f32], f32) -> bool>(text, &gen_config, None)
            .ok_or_else(|| VizierError("Kokoro TTS generation failed".into()))?;

        let sample_rate = audio.sample_rate() as u32;
        let samples = audio.samples();

        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut wav_buf = std::io::Cursor::new(Vec::new());
        {
            let mut writer = hound::WavWriter::new(&mut wav_buf, spec)
                .map_err(|e| VizierError(format!("wav writer: {e}")))?;

            for &sample in samples {
                let clamped = sample.clamp(-1.0, 1.0);
                let pcm = (clamped * 32767.0) as i16;
                writer
                    .write_sample(pcm)
                    .map_err(|e| VizierError(format!("wav write: {e}")))?;
            }

            writer
                .finalize()
                .map_err(|e| VizierError(format!("wav finalize: {e}")))?;
        }

        Ok(wav_buf.into_inner())
    }
}
