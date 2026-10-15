pub mod elevenlabs;
pub mod kokoro;

use std::sync::Arc;

use crate::dependencies::VizierDependencies;
use crate::llm::media::{self, hyperbolic::HyperbolicTts, openai_shaped::{OpenAiTts, XaiTts}};
use crate::schema::agent::{TtsProvider, TtsToolSettings};
use crate::{Result, VizierError};

#[async_trait::async_trait]
pub trait VizierTtsModel: Send + Sync {
    async fn generate_speech(&self, text: &str, voice: &str, speed: f32) -> Result<Vec<u8>>;
}

pub struct VizierTts(Arc<dyn VizierTtsModel>);

impl VizierTts {
    pub async fn new(
        settings: &TtsToolSettings,
        deps: &VizierDependencies,
        workspace: &str,
    ) -> Result<Self> {
        let provider = &settings.provider;
        let model = settings.model.clone().unwrap_or_else(|| {
            match provider {
                TtsProvider::Openai => "tts-1",
                TtsProvider::Openrouter => "openai/gpt-4o-mini-tts-2025-12-15",
                TtsProvider::Elevenlabs => "eleven_multilingual_v2",
                TtsProvider::Xai => "grok-2-tts",
                TtsProvider::Hyperbolic => "Melo-TTS",
                TtsProvider::Kokoro => "kokoro-en-v0_19",
            }
            .into()
        });
        let model: Arc<dyn VizierTtsModel> = match (provider, provider.credentials()) {
            (TtsProvider::Kokoro, _) => Arc::new(kokoro::KokoroTtsModel::new(model, workspace)),
            (TtsProvider::Elevenlabs, Some(id)) => Arc::new(elevenlabs::ElevenLabsTtsModel::new(
                media::api_key(deps, &id).await?,
                model,
            )),
            (TtsProvider::Xai, Some(id)) => Arc::new(XaiTts(media::endpoint(deps, &id, model).await?)),
            (TtsProvider::Hyperbolic, Some(id)) => {
                Arc::new(HyperbolicTts(media::endpoint(deps, &id, model).await?))
            }
            (TtsProvider::Openrouter, Some(id)) => Arc::new(OpenAiTts {
                endpoint: media::endpoint(deps, &id, model).await?,
                response_format: Some("mp3"),
            }),
            (_, Some(id)) => Arc::new(OpenAiTts {
                endpoint: media::endpoint(deps, &id, model).await?,
                response_format: None,
            }),
            (_, None) => unreachable!("only kokoro needs no credentials"),
        };

        Ok(Self(model))
    }

    pub async fn generate_speech(&self, text: &str, voice: &str, speed: f32) -> Result<Vec<u8>> {
        self.0.generate_speech(text, voice, speed).await
    }
}

pub fn mp3_to_wav(mp3_bytes: &[u8]) -> Result<Vec<u8>> {
    use symphonia::core::codecs::audio::AudioDecoderOptions;
    use symphonia::core::formats::probe::Hint;
    use symphonia::core::formats::{FormatOptions, TrackType};
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;

    let cursor = std::io::Cursor::new(mp3_bytes.to_vec());
    let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

    let mut format = symphonia::default::get_probe()
        .probe(
            &Hint::new(),
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|e| VizierError(format!("mp3 probe: {e}")))?;

    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| VizierError("no default track in mp3".into()))?;
    let track_id = track.id;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or_else(|| VizierError("no audio codec parameters in mp3".into()))?
        .clone();

    let sample_rate = params
        .sample_rate
        .ok_or_else(|| VizierError("unknown sample rate".into()))?;
    let channels = params.channels.as_ref().map(|c| c.count()).unwrap_or(1) as u16;

    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&params, &AudioDecoderOptions::default())
        .map_err(|e| VizierError(format!("mp3 decoder: {e}")))?;

    let mut samples: Vec<i16> = Vec::new();
    let mut chunk: Vec<i16> = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => return Err(VizierError(format!("mp3 read: {e}"))),
        };
        if packet.track_id != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => {
                chunk.resize(decoded.samples_interleaved(), 0);
                decoded.copy_to_slice_interleaved(&mut chunk);
                samples.extend_from_slice(&chunk);
            }
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => return Err(VizierError(format!("mp3 decode: {e}"))),
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
