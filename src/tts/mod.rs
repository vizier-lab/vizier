pub mod elevenlabs;
#[cfg(feature = "media")]
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

/// The model a setting resolves to: what it names, or the provider's documented default.
fn resolved_model(settings: &TtsToolSettings) -> String {
    settings
        .model
        .clone()
        .unwrap_or_else(|| settings.provider.default_model().into())
}

impl VizierTts {
    pub async fn new(
        settings: &TtsToolSettings,
        deps: &VizierDependencies,
        workspace: &str,
        agent_id: &str,
    ) -> Result<Self> {
        let provider = &settings.provider;
        let model = resolved_model(settings);
        let model: Arc<dyn VizierTtsModel> = match (provider, provider.credentials()) {
            #[cfg(feature = "media")]
            (TtsProvider::Kokoro, _) => Arc::new(kokoro::KokoroTtsModel::new(
                model,
                workspace,
                deps.setup.clone(),
                agent_id.to_string(),
            )),
            #[cfg(not(feature = "media"))]
            (TtsProvider::Kokoro, _) => {
                return Err(VizierError(
                    "local speech (kokoro) is not compiled into this build".into(),
                ));
            }
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

#[cfg(feature = "media")]
mod mp3 {
    use crate::{Result, VizierError};

    /// A minimal MPEG Layer III frame header, enough to find its Xing/Info tag.
    pub(super) struct FrameHeader {
        /// MPEG version bits: `3` is MPEG1, `2` MPEG2, `0` MPEG2.5.
        version: u8,
        mono: bool,
    }

    pub(super) fn frame_header(bytes: &[u8]) -> Option<FrameHeader> {
        if bytes.len() < 4 {
            return None;
        }
        let (b1, b2, b3) = (bytes[1], bytes[2], bytes[3]);
        // Sync, then Layer III with a valid MPEG version, bitrate and sample rate.
        if bytes[0] != 0xFF || (b1 & 0xE0) != 0xE0 {
            return None;
        }
        let version = (b1 >> 3) & 0x3;
        let layer = (b1 >> 1) & 0x3;
        if layer != 0b01 || version == 0b01 || (b2 >> 4) & 0xF == 0xF || (b2 >> 2) & 0x3 == 0x3 {
            return None;
        }
        Some(FrameHeader {
            version,
            mono: (b3 >> 6) == 0b11,
        })
    }

    /// How many bytes a stream that starts with a Xing/Info header records itself as. Providers
    /// that stream speech one sentence at a time concatenate independent MP3 streams, each with
    /// its own header; the byte count is the only thing that tells them apart.
    pub(super) fn xing_stream_len(bytes: &[u8]) -> Option<usize> {
        let header = frame_header(bytes)?;
        // The Xing tag follows the frame header and the side information.
        let side_info = match (header.version, header.mono) {
            (0b11, true) => 17,
            (0b11, false) => 32,
            (_, true) => 9,
            (_, false) => 17,
        };
        let xing = 4 + side_info;
        if !matches!(bytes.get(xing..xing + 4)?, b"Xing" | b"Info") {
            return None;
        }
        let flags = u32::from_be_bytes(bytes.get(xing + 4..xing + 8)?.try_into().ok()?);
        let mut at = xing + 8;
        if flags & 0x1 != 0 {
            at += 4; // frame count
        }
        if flags & 0x2 == 0 {
            return None; // no byte count
        }
        let len = u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?) as usize;
        (len > 4 && len <= bytes.len()).then_some(len)
    }

    /// Decode one MP3 stream, appending its samples.
    pub(super) fn decode_mp3_stream(
        mp3_bytes: &[u8],
        samples: &mut Vec<i16>,
        spec: &mut Option<hound::WavSpec>,
    ) -> Result<()> {
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
        match spec {
            Some(current) if current.sample_rate != sample_rate || current.channels != channels => {
                return Err(VizierError(
                    "concatenated mp3 streams disagree on sample rate or channels".into(),
                ));
            }
            Some(_) => {}
            None => {
                *spec = Some(hound::WavSpec {
                    channels,
                    sample_rate,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                })
            }
        }

        let mut decoder = symphonia::default::get_codecs()
            .make_audio_decoder(&params, &AudioDecoderOptions::default())
            .map_err(|e| VizierError(format!("mp3 decoder: {e}")))?;

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
        Ok(())
    }

    pub fn mp3_to_wav(mp3_bytes: &[u8]) -> Result<Vec<u8>> {
        let mut samples: Vec<i16> = Vec::new();
        let mut spec: Option<hound::WavSpec> = None;

        // A speech provider may split one answer into a sentence per independently encoded MP3
        // stream and concatenate them (OpenRouter/Kokoro does). Decode each stream on its own:
        // one reader over the concatenation stops at the end of the first. A plain MP3 is the
        // single-stream case of this loop.
        let mut offset = 0;
        while offset < mp3_bytes.len() {
            let at_frame = frame_header(&mp3_bytes[offset..]).is_some();
            let stream_len = if at_frame || offset == 0 {
                xing_stream_len(&mp3_bytes[offset..])
            } else {
                None
            };
            let end = match stream_len {
                Some(len) if offset + len <= mp3_bytes.len() => offset + len,
                _ if at_frame || offset == 0 => mp3_bytes.len(),
                _ => break, // trailing tag or padding after the last stream
            };
            decode_mp3_stream(&mp3_bytes[offset..end], &mut samples, &mut spec)?;
            if end >= mp3_bytes.len() {
                break;
            }
            offset = end;
        }

        let spec = spec.ok_or_else(|| VizierError("mp3 has no decodable audio".into()))?;
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
}

#[cfg(feature = "media")]
pub use mp3::mp3_to_wav;

/// Without the `media` feature there is no MP3 decoder: a provider that answers in MP3 fails
/// with this, and one that answers in WAV is unaffected.
#[cfg(not(feature = "media"))]
pub fn mp3_to_wav(_mp3_bytes: &[u8]) -> Result<Vec<u8>> {
    Err(VizierError(
        "MP3 audio decoding is not compiled into this build".into(),
    ))
}

#[cfg(all(test, feature = "media"))]
mod tests {
    use super::mp3::*;
    use super::*;
    use base64::Engine as _;

    /// The oldest fixture: a single-stream MP3 with no Xing header. Its first frame is MPEG1
    /// Layer III, 128 kbps, 44.1 kHz, joint stereo, so a Xing tag would sit 36 bytes in.
    fn fixture_mp3() -> Vec<u8> {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../llm/media/fixtures/golden/tts_openai.json"))
                .unwrap();
        base64::engine::general_purpose::STANDARD
            .decode(fixture["rig_output"].as_str().unwrap())
            .unwrap()
    }

    /// The fixture wrapped in a Xing frame that records its own length, as a provider that
    /// returns one stream per sentence does.
    fn xing_wrapped(mp3: &[u8]) -> Vec<u8> {
        const FRAME_LEN: usize = 417; // 128 kbps at 44.1 kHz
        let mut frame = vec![0u8; FRAME_LEN];
        frame[..4].copy_from_slice(&mp3[..4]);
        let at = 4 + 32; // MPEG1 joint-stereo side information
        frame[at..at + 4].copy_from_slice(b"Xing");
        frame[at + 4..at + 8].copy_from_slice(&3u32.to_be_bytes()); // frames | bytes
        // The count includes the Xing frame itself.
        let frames = 1 + mp3.len().div_ceil(FRAME_LEN) as u32;
        frame[at + 8..at + 12].copy_from_slice(&frames.to_be_bytes());
        let len = (FRAME_LEN + mp3.len()) as u32;
        frame[at + 12..at + 16].copy_from_slice(&len.to_be_bytes());
        [frame, mp3.to_vec()].concat()
    }

    fn wav_frames(wav: &[u8]) -> usize {
        let r = hound::WavReader::new(std::io::Cursor::new(wav.to_vec())).unwrap();
        r.len() as usize / r.spec().channels as usize
    }

    #[test]
    fn a_xing_stream_reports_its_own_length() {
        let wrapped = xing_wrapped(&fixture_mp3());
        assert_eq!(xing_stream_len(&wrapped), Some(wrapped.len()));
        // A stream without the tag is decoded whole by the caller.
        assert_eq!(xing_stream_len(&fixture_mp3()), None);
    }

    /// OpenRouter/Kokoro returns each sentence as its own MP3 stream, concatenated. One
    /// reader over the concatenation stops after the first, which cut every reply off at its
    /// first punctuation.
    #[test]
    fn concatenated_mp3_streams_are_all_decoded() {
        let once = mp3_to_wav(&fixture_mp3()).unwrap();
        let wrapped = xing_wrapped(&fixture_mp3());
        let twice = mp3_to_wav(&[wrapped.clone(), wrapped].concat()).unwrap();
        let single = wav_frames(&once);
        assert!(
            wav_frames(&twice) > single * 3 / 2,
            "decoded {} frames for two streams, {} for one",
            wav_frames(&twice),
            single
        );
    }

    /// Spec 028 FR-008: the OpenRouter default pair must be a live model and one of its
    /// published voices — the retired OpenAI id and `alloy` must not come back.
    #[test]
    fn openrouter_resolves_its_documented_model_and_voice() {
        let settings = TtsToolSettings {
            provider: TtsProvider::Openrouter,
            model: None,
            voice: None,
            speed: None,
        };
        assert_eq!(resolved_model(&settings), "hexgrad/kokoro-82m");
        assert_eq!(TtsProvider::Openrouter.default_voice(), "af_alloy");

        // OpenAI keeps its own voice.
        assert_eq!(TtsProvider::Openai.default_voice(), "alloy");
        assert_eq!(TtsProvider::Openai.default_model(), "tts-1");
    }

    #[test]
    fn an_explicit_tts_model_wins_over_the_default() {
        let settings = TtsToolSettings {
            provider: TtsProvider::Openrouter,
            model: Some("someone/other".into()),
            voice: None,
            speed: None,
        };
        assert_eq!(resolved_model(&settings), "someone/other");
    }
}
