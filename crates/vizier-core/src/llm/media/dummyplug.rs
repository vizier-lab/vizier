//! The offline media provider (`specs/065-media-usage-accounting/` research R11): no network, no
//! credentials, deterministic output, so the media paths and their usage records can be driven
//! end to end with a dummyplug agent.

use crate::image_generation::{ReferenceImage, VizierImageGenModel};
use crate::stt::{Transcript, VizierSttModel};
use crate::tts::VizierTtsModel;
use crate::embedding::VizierEmbeddingModel;
use crate::usage::record::{Reported, estimate_tokens};

/// What dummyplug's transcription always hears.
pub const TRANSCRIPT: &str = "dummyplug transcript";

/// The 1x1 transparent PNG dummyplug draws.
const PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
    0x42, 0x60, 0x82,
];

/// 100 ms of 16 kHz, 16-bit mono silence.
pub fn silent_wav() -> Vec<u8> {
    const RATE: u32 = 16_000;
    let data = (RATE / 10 * 2) as usize;
    let mut b = Vec::with_capacity(44 + data);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&((36 + data) as u32).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&RATE.to_le_bytes());
    b.extend_from_slice(&(RATE * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&(data as u32).to_le_bytes());
    b.resize(44 + data, 0);
    b
}

pub struct DummyplugImages;

#[async_trait::async_trait]
impl VizierImageGenModel for DummyplugImages {
    /// Reports nothing, so the image count is measured.
    async fn generate(
        &self,
        _prompt: &str,
        _size: Option<&str>,
        _references: &[ReferenceImage],
    ) -> crate::Result<(Vec<u8>, String)> {
        Ok((PNG.to_vec(), "image/png".into()))
    }
}

pub struct DummyplugTts;

#[async_trait::async_trait]
impl VizierTtsModel for DummyplugTts {
    async fn generate_speech(&self, _text: &str, _voice: &str, _speed: f32) -> crate::Result<Vec<u8>> {
        Ok(silent_wav())
    }
}

pub struct DummyplugStt;

#[async_trait::async_trait]
impl VizierSttModel for DummyplugStt {
    async fn transcribe(
        &self,
        _audio: &[u8],
        _filename: &str,
        _language: Option<&str>,
    ) -> crate::Result<String> {
        Ok(TRANSCRIPT.into())
    }

    /// Reports its tokens, so a transcription exercises the reported path.
    async fn transcribe_reported(
        &self,
        audio: &[u8],
        filename: &str,
        language: Option<&str>,
    ) -> crate::Result<Transcript> {
        Ok(Transcript {
            text: self.transcribe(audio, filename, language).await?,
            reported: Reported {
                output_tokens: Some(estimate_tokens(&[TRANSCRIPT])),
                ..Default::default()
            },
        })
    }
}

pub struct DummyplugEmbeddings;

/// 8 dimensions, from the text's bytes: the same text always lands on the same vector.
fn vector(text: &str) -> Vec<f64> {
    let mut v = [0f64; 8];
    for (i, b) in text.bytes().enumerate() {
        v[i % 8] += f64::from(b);
    }
    let norm = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm == 0.0 {
        v[0] = 1.0;
        return v.to_vec();
    }
    v.iter().map(|x| x / norm).collect()
}

#[async_trait::async_trait]
impl VizierEmbeddingModel for DummyplugEmbeddings {
    async fn embed_text(&self, text: &str) -> anyhow::Result<Vec<f64>> {
        Ok(vector(text))
    }

    async fn embed_texts(&self, documents: Vec<String>) -> anyhow::Result<Vec<Vec<f64>>> {
        Ok(documents.iter().map(|d| vector(d)).collect())
    }

    /// Reports its tokens, so embeddings exercise the reported path.
    async fn embed_text_reported(&self, text: &str) -> anyhow::Result<(Vec<f64>, Reported)> {
        let reported = Reported {
            input_tokens: Some(estimate_tokens(&[text])),
            ..Default::default()
        };
        Ok((vector(text), reported))
    }

    async fn embed_texts_reported(
        &self,
        documents: Vec<String>,
    ) -> anyhow::Result<(Vec<Vec<f64>>, Reported)> {
        let reported = Reported {
            input_tokens: Some(estimate_tokens(&documents)),
            ..Default::default()
        };
        Ok((documents.iter().map(|d| vector(d)).collect(), reported))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_silent_wav_is_a_tenth_of_a_second() {
        assert_eq!(crate::usage::record::wav_duration_ms(&silent_wav()), Some(100));
    }

    #[test]
    fn vectors_are_unit_length_and_stable() {
        let a = vector("fox");
        assert_eq!(a, vector("fox"));
        let n: f64 = a.iter().map(|x| x * x).sum();
        assert!((n - 1.0).abs() < 1e-9);
    }
}
