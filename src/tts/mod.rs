pub mod elevenlabs;
pub mod hyperbolic;
pub mod kokoro;
pub mod openai;
pub mod openrouter;
pub mod xai;

use std::sync::Arc;

use crate::config::provider::ProviderVariant;
use crate::schema::agent::{TtsProvider, TtsToolSettings};
use crate::storage::VizierStorage;
use crate::{Result, VizierError};

#[async_trait::async_trait]
pub trait VizierTtsModel: Send + Sync {
    async fn generate_speech(&self, text: &str, voice: &str, speed: f32) -> Result<Vec<u8>>;
}

pub struct VizierTts(Arc<dyn VizierTtsModel>);

impl VizierTts {
    pub async fn new(
        settings: &TtsToolSettings,
        storage: &Arc<VizierStorage>,
        _workspace: &str,
    ) -> Result<Self> {
        let model: Arc<dyn VizierTtsModel> = match &settings.provider {
            TtsProvider::Openai => {
                let resolved = crate::provider_keys::resolve_provider_key(
                    storage,
                    ProviderVariant::openai,
                    "OPENAI_API_KEY",
                )
                .await?;
                let model = settings.model.clone().unwrap_or_else(|| "tts-1".into());
                Arc::new(openai::OpenAiTtsModel::new(resolved.api_key, model))
            }
            TtsProvider::Openrouter => {
                let resolved = crate::provider_keys::resolve_provider_key(
                    storage,
                    ProviderVariant::openrouter,
                    "OPENROUTER_API_KEY",
                )
                .await?;
                let model = settings
                    .model
                    .clone()
                    .unwrap_or_else(|| "openai/gpt-4o-mini-tts-2025-12-15".into());
                Arc::new(openrouter::OpenRouterTtsModel::new(resolved.api_key, model))
            }
            TtsProvider::Elevenlabs => {
                let resolved = crate::provider_keys::resolve_provider_key(
                    storage,
                    ProviderVariant::elevenlabs,
                    "ELEVENLABS_API_KEY",
                )
                .await?;
                let model = settings
                    .model
                    .clone()
                    .unwrap_or_else(|| "eleven_multilingual_v2".into());
                Arc::new(elevenlabs::ElevenLabsTtsModel::new(resolved.api_key, model))
            }
            TtsProvider::Xai => {
                let resolved = crate::provider_keys::resolve_provider_key(
                    storage,
                    ProviderVariant::xai,
                    "XAI_API_KEY",
                )
                .await?;
                let model = settings
                    .model
                    .clone()
                    .unwrap_or_else(|| "grok-2-tts".into());
                Arc::new(xai::XaiTtsModel::new(resolved.api_key, model))
            }
            TtsProvider::Hyperbolic => {
                let resolved = crate::provider_keys::resolve_provider_key(
                    storage,
                    ProviderVariant::hyperbolic,
                    "HYPERBOLIC_API_KEY",
                )
                .await?;
                let model = settings
                    .model
                    .clone()
                    .unwrap_or_else(|| "Melo-TTS".into());
                Arc::new(hyperbolic::HyperbolicTtsModel::new(resolved.api_key, model))
            }
            TtsProvider::Kokoro => {
                let model = settings
                    .model
                    .clone()
                    .unwrap_or_else(|| "kokoro-en-v0_19".into());
                Arc::new(kokoro::KokoroTtsModel::new(model, _workspace))
            }
        };

        Ok(Self(model))
    }

    pub async fn generate_speech(&self, text: &str, voice: &str, speed: f32) -> Result<Vec<u8>> {
        self.0.generate_speech(text, voice, speed).await
    }
}

pub fn mp3_to_wav(mp3_bytes: &[u8]) -> Result<Vec<u8>> {
    use symphonia::core::audio::{AudioBufferRef, Signal};
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let cursor = std::io::Cursor::new(mp3_bytes.to_vec());
    let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

    let probed = symphonia::default::get_probe()
        .format(
            &Hint::new(),
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| VizierError(format!("mp3 probe: {e}")))?;

    let mut format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| VizierError("no default track in mp3".into()))?;

    let sample_rate = track
        .codec_params
        .sample_rate
        .ok_or_else(|| VizierError("unknown sample rate".into()))?;
    let channels = track
        .codec_params
        .channels
        .map(|c| c.count())
        .unwrap_or(1) as u16;

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| VizierError(format!("mp3 decoder: {e}")))?;

    let mut samples: Vec<i16> = Vec::new();
    loop {
        match format.next_packet() {
            Ok(packet) => match decoder.decode(&packet) {
                Ok(decoded) => match decoded {
                    AudioBufferRef::F32(buf) => {
                        for ch in 0..buf.spec().channels.count() {
                            let channel = buf.chan(ch);
                            for &s in channel.iter() {
                                let clamped = s.clamp(-1.0, 1.0);
                                samples.push((clamped * 32767.0) as i16);
                            }
                        }
                    }
                    AudioBufferRef::S16(buf) => {
                        for ch in 0..buf.spec().channels.count() {
                            samples.extend_from_slice(buf.chan(ch));
                        }
                    }
                    AudioBufferRef::U8(buf) => {
                        for ch in 0..buf.spec().channels.count() {
                            for &s in buf.chan(ch).iter() {
                                samples.push(((s as i16) - 128) * 256);
                            }
                        }
                    }
                    _ => {
                        return Err(VizierError("unsupported audio sample format".into()));
                    }
                },
                Err(symphonia::core::errors::Error::IoError(e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break;
                }
                Err(e) => return Err(VizierError(format!("mp3 decode: {e}"))),
            },
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => return Err(VizierError(format!("mp3 read: {e}"))),
        }
    }

    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut wav_buf = std::io::Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut wav_buf, spec)
            .map_err(|e| VizierError(format!("wav writer: {e}")))?;
        for s in samples {
            writer
                .write_sample(s)
                .map_err(|e| VizierError(format!("wav write: {e}")))?;
        }
        writer
            .finalize()
            .map_err(|e| VizierError(format!("wav finalize: {e}")))?;
    }

    Ok(wav_buf.into_inner())
}
