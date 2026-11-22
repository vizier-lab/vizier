//! One utterance's audio, and the WAV a recogniser is handed (`specs/055-live-voice` T028).
//!
//! The 44-byte header is written by hand rather than with `hound`, which sits behind
//! `provider-media`: using it would couple this flag to that one for four constants and twelve
//! integers (`review.md` D7).

use crate::live_voice::frame::{CHUNK_BYTES, SAMPLE_RATE};

/// Below this an utterance is a door slam, not a question (FR-024, `analysis.md` M11).
pub const MIN_UTTERANCE_MS: u32 = 250;

/// The audio of one utterance: PCM16 mono at 16 kHz, accumulated, capped, and never stored.
#[derive(Debug, Clone)]
pub struct Utterance {
    pub id: String,
    pcm: Vec<i16>,
    /// The cap, from the agent's `max_utterance_seconds`.
    max_samples: usize,
    cancelled: bool,
}

impl Utterance {
    /// A new utterance, capped at `max_seconds` of audio (FR-010).
    pub fn new(id: impl Into<String>, max_seconds: u32) -> Self {
        Self {
            id: id.into(),
            pcm: Vec::new(),
            max_samples: SAMPLE_RATE as usize * max_seconds.max(1) as usize,
            cancelled: false,
        }
    }

    /// Append little-endian PCM16 bytes; returns how many samples were kept, which is fewer than
    /// offered once the cap is reached. Bytes that do not pair up are dropped, not guessed at.
    pub fn push_pcm16_le(&mut self, bytes: &[u8]) -> usize {
        let room = self.max_samples.saturating_sub(self.pcm.len());
        let take = bytes.len().min(room * 2) / 2;
        self.pcm.extend(
            bytes
                .chunks_exact(2)
                .take(take)
                .map(|pair| i16::from_le_bytes([pair[0], pair[1]])),
        );
        take
    }

    pub fn len(&self) -> usize {
        self.pcm.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pcm.is_empty()
    }

    /// Whether the cap stopped the utterance, which is what the client is told once.
    pub fn is_full(&self) -> bool {
        self.pcm.len() >= self.max_samples
    }

    pub fn duration_ms(&self) -> u32 {
        (self.pcm.len() as u64 * 1000 / SAMPLE_RATE as u64) as u32
    }

    /// Too short to be worth a recogniser call (FR-024).
    pub fn too_short(&self) -> bool {
        self.duration_ms() < MIN_UTTERANCE_MS
    }

    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    /// The clip as a WAV a provider accepts: header plus the samples kept so far.
    pub fn wav(&self) -> Vec<u8> {
        let mut out = wav_header(self.pcm.len()).to_vec();
        out.reserve(self.pcm.len() * 2);
        for sample in &self.pcm {
            out.extend_from_slice(&sample.to_le_bytes());
        }
        out
    }

    pub fn filename(&self) -> String {
        format!("{}.wav", self.id)
    }
}

/// The 44-byte canonical WAV header for mono PCM16 at 16 kHz.
pub fn wav_header(samples: usize) -> [u8; 44] {
    const CHANNELS: u16 = 1;
    const BITS: u16 = 16;
    let data = (samples * 2) as u32;
    let mut h = [0u8; 44];
    h[0..4].copy_from_slice(b"RIFF");
    h[4..8].copy_from_slice(&(36 + data).to_le_bytes());
    h[8..12].copy_from_slice(b"WAVE");
    h[12..16].copy_from_slice(b"fmt ");
    h[16..20].copy_from_slice(&16u32.to_le_bytes());
    h[20..22].copy_from_slice(&1u16.to_le_bytes()); // PCM
    h[22..24].copy_from_slice(&CHANNELS.to_le_bytes());
    h[24..28].copy_from_slice(&SAMPLE_RATE.to_le_bytes());
    h[28..32].copy_from_slice(&(SAMPLE_RATE * CHANNELS as u32 * BITS as u32 / 8).to_le_bytes());
    h[32..34].copy_from_slice(&(CHANNELS * BITS / 8).to_le_bytes());
    h[34..36].copy_from_slice(&BITS.to_le_bytes());
    h[36..40].copy_from_slice(b"data");
    h[40..44].copy_from_slice(&data.to_le_bytes());
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    fn silent(ms: u32) -> Vec<u8> {
        vec![0u8; (SAMPLE_RATE as usize * ms as usize / 1000) * 2]
    }

    /// The header is the one a recogniser expects: 16 kHz, mono, PCM16, and the sizes agree.
    #[test]
    fn the_wav_header_is_canonical() {
        let mut u = Utterance::new("u-1", 60);
        u.push_pcm16_le(&silent(500));
        let wav = u.wav();
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 16_000);
        assert_eq!(u16::from_le_bytes(wav[22..24].try_into().unwrap()), 1);
        assert_eq!(u16::from_le_bytes(wav[34..36].try_into().unwrap()), 16);
        let data = u32::from_le_bytes(wav[40..44].try_into().unwrap()) as usize;
        assert_eq!(data, wav.len() - 44);
        assert_eq!(u32::from_le_bytes(wav[4..8].try_into().unwrap()) as usize, 36 + data);
        assert_eq!(u.filename(), "u-1.wav");
    }

    /// 100 ms chunks accumulate into the duration the hangover decided.
    #[test]
    fn chunks_accumulate_into_a_duration() {
        let mut u = Utterance::new("u-1", 60);
        for _ in 0..5 {
            assert_eq!(u.push_pcm16_le(&vec![0u8; CHUNK_BYTES]), 1600);
        }
        assert_eq!(u.duration_ms(), 500);
        assert!(!u.too_short());
    }

    /// A cough is not a question (FR-024).
    #[test]
    fn a_short_burst_is_too_short() {
        let mut u = Utterance::new("u-1", 60);
        u.push_pcm16_le(&silent(200));
        assert!(u.too_short());
        let mut ok = Utterance::new("u-2", 60);
        ok.push_pcm16_le(&silent(MIN_UTTERANCE_MS));
        assert!(!ok.too_short(), "exactly the threshold is long enough");
    }

    /// The cap is enforced where the samples arrive, and says so once (FR-010).
    #[test]
    fn the_cap_stops_the_utterance() {
        let mut u = Utterance::new("u-1", 1);
        let kept = u.push_pcm16_le(&vec![0u8; CHUNK_BYTES * 20]);
        assert_eq!(kept, SAMPLE_RATE as usize);
        assert!(u.is_full());
        assert_eq!(u.duration_ms(), 1000);
        // Further audio is dropped, not grown.
        assert_eq!(u.push_pcm16_le(&silent(100)), 0);
        assert_eq!(u.len(), SAMPLE_RATE as usize);
    }

    /// An odd byte cannot become a sample.
    #[test]
    fn an_unpaired_byte_is_dropped() {
        let mut u = Utterance::new("u-1", 60);
        assert_eq!(u.push_pcm16_le(&[1, 0, 2]), 1);
        assert_eq!(u.len(), 1);
    }
}
