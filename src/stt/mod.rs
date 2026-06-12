pub mod elevenlabs;
pub mod openai;
pub mod sense_voice;

use std::sync::Arc;

use crate::config::provider::ProviderVariant;
use crate::schema::agent::{SttProvider, SttToolSettings};
use crate::storage::VizierStorage;
use crate::{Result, VizierError};

#[async_trait::async_trait]
pub trait VizierSttModel: Send + Sync {
    async fn transcribe(
        &self,
        audio: &[u8],
        filename: &str,
        language: Option<&str>,
    ) -> Result<String>;
}

pub struct VizierStt(Arc<dyn VizierSttModel>);

impl VizierStt {
    pub async fn new(
        settings: &SttToolSettings,
        storage: &Arc<VizierStorage>,
        workspace: &str,
    ) -> Result<Self> {
        let model: Arc<dyn VizierSttModel> = match &settings.provider {
            SttProvider::SenseVoice => {
                Arc::new(sense_voice::SenseVoiceSttModel::new(settings, workspace).await?)
            }
            SttProvider::Openai => {
                let resolved = crate::provider_keys::resolve_provider_key(
                    storage,
                    ProviderVariant::openai,
                    "OPENAI_API_KEY",
                )
                .await?;
                let model = settings
                    .model
                    .clone()
                    .unwrap_or_else(|| SttProvider::Openai.default_model().into());
                Arc::new(openai::OpenAiSttModel::new(
                    resolved.api_key,
                    model,
                    resolved.base_url,
                ))
            }
            SttProvider::Elevenlabs => {
                let resolved = crate::provider_keys::resolve_provider_key(
                    storage,
                    ProviderVariant::elevenlabs,
                    "ELEVENLABS_API_KEY",
                )
                .await?;
                let model = settings
                    .model
                    .clone()
                    .unwrap_or_else(|| SttProvider::Elevenlabs.default_model().into());
                Arc::new(elevenlabs::ElevenLabsSttModel::new(resolved.api_key, model))
            }
        };

        Ok(Self(model))
    }

    pub async fn transcribe(
        &self,
        audio: &[u8],
        filename: &str,
        language: Option<&str>,
    ) -> Result<String> {
        self.0.transcribe(audio, filename, language).await
    }
}

/// Decode audio bytes (WAV, MP3, OGG, etc.) to mono f32 PCM samples at 16kHz
/// for sherpa-onnx consumption.
pub fn decode_to_pcm_f32(audio: &[u8], filename: &str) -> Result<(Vec<f32>, i32)> {
    use symphonia::core::audio::{AudioBufferRef, Signal};
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let cursor = std::io::Cursor::new(audio.to_vec());
    let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = std::path::Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
    {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| VizierError(format!("audio probe: {e}")))?;

    let mut format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| VizierError("no default track in audio".into()))?;

    let sample_rate = track
        .codec_params
        .sample_rate
        .ok_or_else(|| VizierError("unknown sample rate".into()))? as i32;
    let channels = track
        .codec_params
        .channels
        .map(|c| c.count())
        .unwrap_or(1);

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| VizierError(format!("audio decoder: {e}")))?;

    let mut all_samples: Vec<f32> = Vec::new();
    loop {
        match format.next_packet() {
            Ok(packet) => match decoder.decode(&packet) {
                Ok(decoded) => match decoded {
                    AudioBufferRef::F32(buf) => {
                        if channels == 1 {
                            all_samples.extend_from_slice(buf.chan(0));
                        } else {
                            // Mix down to mono
                            let num_frames = buf.chan(0).len();
                            for i in 0..num_frames {
                                let mut sum = 0.0f32;
                                for ch in 0..channels {
                                    sum += buf.chan(ch)[i];
                                }
                                all_samples.push(sum / channels as f32);
                            }
                        }
                    }
                    AudioBufferRef::S16(buf) => {
                        if channels == 1 {
                            for &s in buf.chan(0) {
                                all_samples.push(s as f32 / 32768.0);
                            }
                        } else {
                            let num_frames = buf.chan(0).len();
                            for i in 0..num_frames {
                                let mut sum = 0.0f32;
                                for ch in 0..channels {
                                    sum += buf.chan(ch)[i] as f32;
                                }
                                all_samples.push(sum / channels as f32 / 32768.0);
                            }
                        }
                    }
                    AudioBufferRef::U8(buf) => {
                        if channels == 1 {
                            for &s in buf.chan(0) {
                                all_samples.push((s as f32 - 128.0) / 128.0);
                            }
                        } else {
                            let num_frames = buf.chan(0).len();
                            for i in 0..num_frames {
                                let mut sum = 0.0f32;
                                for ch in 0..channels {
                                    sum += buf.chan(ch)[i] as f32 - 128.0;
                                }
                                all_samples.push(sum / channels as f32 / 128.0);
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
                Err(e) => return Err(VizierError(format!("audio decode: {e}"))),
            },
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => return Err(VizierError(format!("audio read: {e}"))),
        }
    }

    // Resample to 16kHz if needed
    if sample_rate != 16000 {
        let resampler =
            sherpa_onnx::LinearResampler::create(sample_rate, 16000).ok_or_else(|| {
                VizierError(format!(
                    "failed to create resampler {}->16000",
                    sample_rate
                ))
            })?;
        let resampled = resampler.resample(&all_samples, true);
        Ok((resampled, 16000))
    } else {
        Ok((all_samples, sample_rate))
    }
}
