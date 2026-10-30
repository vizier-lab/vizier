use std::path::{Path, PathBuf};
use std::sync::Arc;

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::setup::{DownloadKind, SetupBoard};
use crate::stt::VizierSttModel;
use crate::utils::build_path;
use crate::{Result, VizierError};

/// GGML Whisper models published for whisper.cpp.
const MODEL_BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

/// whisper.cpp only accepts 16 kHz mono f32 PCM.
const WHISPER_SAMPLE_RATE: u32 = 16_000;

/// Map the artifact name a setting names to the GGML file whisper.cpp expects. A saved agent
/// may still name a sherpa artifact (`large-v3`), which is the same model under its GGML name.
fn ggml_artifact(short_name: &str) -> String {
    let name = short_name.trim().rsplit('/').next().unwrap_or(short_name);
    let name = name.strip_suffix(".bin").unwrap_or(name);
    if name.starts_with("ggml-") {
        name.to_string()
    } else {
        format!("ggml-{name}")
    }
}

pub struct WhisperSttModel {
    short_name: String,
    model_dir: PathBuf,
    model_file: PathBuf,
    ctx: Arc<parking_lot::Mutex<Option<Arc<WhisperContext>>>>,
    /// Reports the one-time model download to the WebUI setup board.
    setup: SetupBoard,
    agent_id: String,
}

impl WhisperSttModel {
    pub fn new(short_name: String, workspace: &str, setup: SetupBoard, agent_id: String) -> Self {
        let artifact = ggml_artifact(&short_name);
        let model_dir = build_path(workspace, &[".runtime", "stt", "whisper", &artifact]);
        let model_file = model_dir.join(format!("{artifact}.bin"));
        Self {
            short_name,
            model_dir,
            model_file,
            ctx: Arc::new(parking_lot::Mutex::new(None)),
            setup,
            agent_id,
        }
    }

    async fn ensure_model(&self) -> Result<()> {
        if self.model_file.exists() {
            return Ok(());
        }

        std::fs::create_dir_all(&self.model_dir)
            .map_err(|e| VizierError(format!("create whisper model dir: {e}")))?;

        let artifact = ggml_artifact(&self.short_name);
        let url = format!("{MODEL_BASE_URL}/{artifact}.bin?download=true");

        tracing::info!(
            "Downloading Whisper model '{}' from {url}",
            self.short_name
        );

        self.setup.download_started(
            &self.agent_id,
            DownloadKind::Stt,
            format!("Downloading speech model '{}'", self.short_name),
        );

        let client = reqwest::Client::builder()
            .user_agent("vizier/0.10")
            .build()
            .map_err(|e| VizierError(format!("build download client: {e}")))?;

        if let Err(e) = self
            .setup
            .download_file(
                &client,
                &url,
                &self.model_file,
                &self.agent_id,
                DownloadKind::Stt,
                &format!("{artifact}.bin"),
                false,
            )
            .await
        {
            self.setup
                .download_failed(&self.agent_id, DownloadKind::Stt, e.to_string());
            return Err(e);
        }

        self.setup
            .download_finished(&self.agent_id, DownloadKind::Stt);

        tracing::info!("Whisper model ready at {}", self.model_file.display());
        Ok(())
    }

    /// The loaded model, loading it on first use. Loading reads the whole GGML file, so it
    /// runs on a blocking thread and only the first caller pays for it.
    async fn context(&self) -> Result<Arc<WhisperContext>> {
        if let Some(ctx) = self.ctx.lock().as_ref() {
            return Ok(ctx.clone());
        }

        self.ensure_model().await?;

        let slot = self.ctx.clone();
        let model_file = self.model_file.clone();
        tokio::task::spawn_blocking(move || {
            let mut slot = slot.lock();
            if let Some(ctx) = slot.as_ref() {
                return Ok(ctx.clone());
            }

            // whisper.cpp logs to stderr by default; route it through tracing instead.
            static LOGGING: std::sync::Once = std::sync::Once::new();
            LOGGING.call_once(whisper_rs::install_logging_hooks);

            let ctx = Arc::new(
                WhisperContext::new_with_params(&model_file, WhisperContextParameters::default())
                    .map_err(|e| VizierError(format!("load whisper model: {e}")))?,
            );
            *slot = Some(ctx.clone());
            Ok(ctx)
        })
        .await
        .map_err(|e| VizierError(format!("whisper model load task: {e}")))?
    }
}

/// symphonia's codecs plus Opus, which it does not ship.
static CODECS: std::sync::LazyLock<symphonia::core::codecs::registry::CodecRegistry> =
    std::sync::LazyLock::new(|| {
        let mut registry = symphonia::core::codecs::registry::CodecRegistry::new();
        symphonia::default::register_enabled_codecs(&mut registry);
        registry.register_audio_decoder::<symphonia_adapter_opus_rs::OpusAdapter>();
        registry
    });

/// Decode any format symphonia understands into mono f32 samples at 16 kHz.
fn decode_to_whisper_pcm(audio: &[u8], filename: &str) -> Result<Vec<f32>> {
    use symphonia::core::codecs::audio::AudioDecoderOptions;
    use symphonia::core::formats::probe::Hint;
    use symphonia::core::formats::{FormatOptions, TrackType};
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;

    if audio.is_empty() {
        return Err(VizierError("audio file is empty".into()));
    }

    let cursor = std::io::Cursor::new(audio.to_vec());
    let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = Path::new(filename).extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|e| VizierError(format!("probe audio: {e}")))?;

    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| VizierError("audio has no default track".into()))?;
    let track_id = track.id;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or_else(|| VizierError("audio has no codec parameters".into()))?
        .clone();

    let sample_rate = params
        .sample_rate
        .ok_or_else(|| VizierError("unknown audio sample rate".into()))?;
    let channels = params.channels.as_ref().map(|c| c.count()).unwrap_or(1);

    let mut decoder = CODECS
        .make_audio_decoder(&params, &AudioDecoderOptions::default())
        .map_err(|e| VizierError(format!("audio decoder: {e}")))?;

    let mut interleaved: Vec<f32> = Vec::new();
    let mut chunk: Vec<f32> = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => return Err(VizierError(format!("audio read: {e}"))),
        };
        if packet.track_id != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => {
                chunk.resize(decoded.samples_interleaved(), 0.0);
                decoded.copy_to_slice_interleaved(&mut chunk);
                interleaved.extend_from_slice(&chunk);
            }
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => return Err(VizierError(format!("audio decode: {e}"))),
        }
    }

    if interleaved.is_empty() {
        return Err(VizierError("audio has no decodable samples".into()));
    }

    let mono: Vec<f32> = if channels > 1 {
        interleaved
            .chunks_exact(channels)
            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
            .collect()
    } else {
        interleaved
    };

    Ok(resample_linear(&mono, sample_rate, WHISPER_SAMPLE_RATE))
}

fn resample_linear(samples: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || samples.is_empty() {
        return samples.to_vec();
    }
    let ratio = from as f64 / to as f64;
    let out_len = ((samples.len() as f64) / ratio).round() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let pos = i as f64 * ratio;
        let left = pos.floor() as usize;
        let right = (left + 1).min(samples.len() - 1);
        let t = (pos - left as f64) as f32;
        let value = samples[left] * (1.0 - t) + samples[right] * t;
        out.push(value);
    }
    out
}

#[async_trait::async_trait]
impl VizierSttModel for WhisperSttModel {
    async fn transcribe(
        &self,
        audio: &[u8],
        filename: &str,
        language: Option<&str>,
    ) -> Result<String> {
        // Decoding, inference and model loading are CPU-bound and can take seconds; keep them
        // off the async workers so other turns keep running.
        let (audio, filename) = (audio.to_vec(), filename.to_string());
        let pcm = tokio::task::spawn_blocking(move || decode_to_whisper_pcm(&audio, &filename))
            .await
            .map_err(|e| VizierError(format!("audio decode task: {e}")))??;

        let ctx = self.context().await?;
        let language = language.map(str::to_string);

        tokio::task::spawn_blocking(move || run_whisper(&ctx, &pcm, language.as_deref()))
            .await
            .map_err(|e| VizierError(format!("whisper task: {e}")))?
    }
}

fn run_whisper(ctx: &WhisperContext, pcm: &[f32], language: Option<&str>) -> Result<String> {
    let mut state = ctx
        .create_state()
        .map_err(|e| VizierError(format!("create whisper state: {e}")))?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    // Empty, `auto` and `detect` all mean "let Whisper decide".
    let language = language
        .map(|l| l.trim().to_ascii_lowercase())
        .filter(|l| !l.is_empty() && l != "auto" && l != "detect");
    let language = language
        .as_deref()
        .and_then(|l| l.split(['-', '_']).next())
        .filter(|l| !l.is_empty());
    params.set_language(language);
    params.set_translate(false);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_n_threads(
        std::thread::available_parallelism()
            .map(|n| n.get() as i32)
            .unwrap_or(2)
            .min(4),
    );

    state
        .full(params, pcm)
        .map_err(|e| VizierError(format!("whisper transcription failed: {e}")))?;

    let mut text = String::new();
    for segment in state.as_iter() {
        let part = segment
            .to_str_lossy()
            .map_err(|e| VizierError(format!("whisper segment: {e}")))?;
        text.push_str(&part);
    }

    Ok(text.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_sherpa_names_map_to_ggml_artifacts() {
        assert_eq!(ggml_artifact("large-v3"), "ggml-large-v3");
        assert_eq!(ggml_artifact("ggml-large-v3"), "ggml-large-v3");
        assert_eq!(ggml_artifact("ggml-small.en.bin"), "ggml-small.en");
        assert_eq!(ggml_artifact("tiny"), "ggml-tiny");
    }

    fn wav(rate: u32, channels: u16, seconds: u32) -> Vec<u8> {
        let spec = hound::WavSpec {
            channels,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut out = std::io::Cursor::new(Vec::new());
        let mut writer = hound::WavWriter::new(&mut out, spec).unwrap();
        for i in 0..rate * seconds {
            let s = ((i as f32 * 0.05).sin() * 8_000.0) as i16;
            for _ in 0..channels {
                writer.write_sample(s).unwrap();
            }
        }
        writer.finalize().unwrap();
        out.into_inner()
    }

    /// The WebUI records 16 kHz mono WAV; other clients may send any rate or channel count.
    #[test]
    fn wav_uploads_decode_to_16khz_mono() {
        let browser = decode_to_whisper_pcm(&wav(16_000, 1, 1), "voice-message.wav").unwrap();
        assert_eq!(browser.len(), 16_000);

        let stereo = decode_to_whisper_pcm(&wav(44_100, 2, 1), "voice-message.wav").unwrap();
        assert_eq!(stereo.len(), 16_000);
        assert!(stereo.iter().any(|s| s.abs() > 0.1));
    }

    /// What Chrome records: Opus in WebM. One second of a 440 Hz tone, 48 kHz mono.
    #[test]
    fn webm_opus_uploads_decode_to_16khz_mono() {
        let audio = include_bytes!("fixtures/tone.webm");
        let pcm = decode_to_whisper_pcm(audio, "voice-message.webm").unwrap();
        assert!(
            (15_000..=17_500).contains(&pcm.len()),
            "{} samples",
            pcm.len()
        );
        let rms = (pcm.iter().map(|s| s * s).sum::<f32>() / pcm.len() as f32).sqrt();
        assert!(rms > 0.05, "decoded silence (rms {rms})");
    }

    #[test]
    fn an_undecodable_upload_is_an_error_not_a_panic() {
        assert!(
            decode_to_whisper_pcm(b"\x1a\x45\xdf\xa3 not really webm", "voice-message.webm")
                .is_err()
        );
    }

    #[test]
    fn resampling_keeps_the_duration() {
        let samples = vec![0.0f32; 44_100];
        let out = resample_linear(&samples, 44_100, WHISPER_SAMPLE_RATE);
        assert_eq!(out.len(), WHISPER_SAMPLE_RATE as usize);
    }
}
