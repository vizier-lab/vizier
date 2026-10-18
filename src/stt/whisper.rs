use std::path::PathBuf;
use std::sync::Arc;

use sherpa_onnx::{OfflineRecognizer, OfflineRecognizerConfig, OfflineWhisperModelConfig, Wave};

use crate::setup::{DownloadKind, SetupBoard};
use crate::stt::VizierSttModel;
use crate::utils::build_path;
use crate::{Result, VizierError};

/// How many bytes a download moves between two progress reports.
const PROGRESS_STEP: u64 = 1024 * 1024;

const MODEL_BASE_URL: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models";

pub struct WhisperSttModel {
    short_name: String,
    model_dir: PathBuf,
    recognizer: Arc<parking_lot::Mutex<Option<OfflineRecognizer>>>,
    /// Reports the one-time model download to the WebUI setup board.
    setup: SetupBoard,
    agent_id: String,
}

impl WhisperSttModel {
    pub fn new(short_name: String, workspace: &str, setup: SetupBoard, agent_id: String) -> Self {
        let archive_name = format!("sherpa-onnx-whisper-{short_name}");
        let model_dir = build_path(workspace, &[".runtime", "stt", "whisper", &archive_name]);
        Self {
            short_name,
            model_dir,
            recognizer: Arc::new(parking_lot::Mutex::new(None)),
            setup,
            agent_id,
        }
    }

    async fn ensure_model(&self) -> Result<()> {
        let encoder_int8 = self
            .model_dir
            .join(format!("{}-encoder.int8.onnx", self.short_name));
        if encoder_int8.exists() {
            return Ok(());
        }

        std::fs::create_dir_all(&self.model_dir)
            .map_err(|e| VizierError(format!("create whisper model dir: {e}")))?;

        let archive_name = format!("sherpa-onnx-whisper-{}.tar.bz2", self.short_name);
        let url = format!("{MODEL_BASE_URL}/{archive_name}");
        let archive_path = self.model_dir.parent().unwrap().join(&archive_name);

        tracing::info!("Downloading Whisper model: {url}");

        self.setup.download_started(
            &self.agent_id,
            DownloadKind::Stt,
            format!("Downloading speech model '{}'", self.short_name),
        );

        let client = reqwest::Client::builder()
            .user_agent("vizier/0.10")
            .build()
            .map_err(|e| VizierError(format!("build download client: {e}")))?;

        let max_retries = 100u32;
        let mut last_err: Option<VizierError> = None;
        let mut bytes: Vec<u8> = Vec::new();
        // Set only by an attempt whose stream ended cleanly with every byte, so a final
        // attempt cut off half way is never taken for a finished download.
        let mut complete = false;

        for attempt in 1..=max_retries {
            bytes.clear();

            let response = match client.get(&url).send().await {
                Ok(r) => r,
                Err(e) => {
                    let err = VizierError(format!("download whisper model: {e}"));
                    tracing::warn!("Download attempt {attempt}/{max_retries} failed: {e}");
                    last_err = Some(err);
                    if attempt < max_retries {
                        tokio::time::sleep(std::time::Duration::from_secs(2 * attempt as u64))
                            .await;
                    }
                    continue;
                }
            };

            let total_size = response.content_length().unwrap_or(0);
            let mut completed = 0u64;
            let mut reported = 0u64;

            let mut stream = response.bytes_stream();
            use futures::StreamExt;
            let mut download_failed = false;
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(c) => {
                        bytes.extend_from_slice(&c);
                        completed += c.len() as u64;
                        // Every chunk would take the board's lock thousands of times.
                        if completed - reported >= PROGRESS_STEP || completed == total_size {
                            reported = completed;
                            self.setup.download_progress(
                                &self.agent_id,
                                DownloadKind::Stt,
                                completed,
                                total_size,
                                "downloading",
                            );
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Download attempt {attempt}/{max_retries} failed at chunk: {e}"
                        );
                        last_err = Some(VizierError(format!("download chunk: {e}")));
                        download_failed = true;
                        break;
                    }
                }
            }

            if download_failed {
                if attempt < max_retries {
                    tokio::time::sleep(std::time::Duration::from_secs(2 * attempt as u64)).await;
                }
                continue;
            }

            if total_size > 0 && completed != total_size {
                tracing::warn!(
                    "Download attempt {attempt}/{max_retries} ended early: {completed}/{total_size} bytes"
                );
                last_err = Some(VizierError(format!(
                    "download ended early: {completed} of {total_size} bytes"
                )));
                if attempt < max_retries {
                    tokio::time::sleep(std::time::Duration::from_secs(2 * attempt as u64)).await;
                }
                continue;
            }

            complete = true;
            break;
        }

        if !complete {
            let err = last_err.unwrap_or_else(|| VizierError("download failed".into()));
            self.setup
                .download_failed(&self.agent_id, DownloadKind::Stt, err.to_string());
            return Err(err);
        }
        self.setup
            .download_finished(&self.agent_id, DownloadKind::Stt);

        std::fs::write(&archive_path, &bytes)
            .map_err(|e| VizierError(format!("write archive: {e}")))?;

        tracing::info!("Extracting Whisper model to {}", self.model_dir.display());

        let archive_file = std::fs::File::open(&archive_path)
            .map_err(|e| VizierError(format!("open archive: {e}")))?;
        let bz_decoder = bzip2::read::BzDecoder::new(archive_file);
        let mut archive = tar::Archive::new(bz_decoder);
        archive.set_overwrite(true);
        archive
            .unpack(self.model_dir.parent().unwrap())
            .map_err(|e| VizierError(format!("extract archive: {e}")))?;

        let _ = std::fs::remove_file(&archive_path);

        tracing::info!("Whisper model ready at {}", self.model_dir.display());
        Ok(())
    }

    async fn ensure_recognizer(
        &self,
    ) -> Result<parking_lot::MutexGuard<'_, Option<OfflineRecognizer>>> {
        {
            let lock = self.recognizer.lock();
            if lock.is_some() {
                return Ok(lock);
            }
        }

        self.ensure_model().await?;

        let encoder = self
            .model_dir
            .join(format!("{}-encoder.int8.onnx", self.short_name));
        let decoder = self
            .model_dir
            .join(format!("{}-decoder.int8.onnx", self.short_name));
        let tokens = self
            .model_dir
            .join(format!("{}-tokens.txt", self.short_name));

        let config = OfflineRecognizerConfig {
            model_config: sherpa_onnx::OfflineModelConfig {
                whisper: OfflineWhisperModelConfig {
                    encoder: Some(encoder.to_string_lossy().into_owned()),
                    decoder: Some(decoder.to_string_lossy().into_owned()),
                    language: None,
                    task: Some("transcribe".into()),
                    tail_paddings: 0,
                    enable_token_timestamps: false,
                    enable_segment_timestamps: false,
                },
                tokens: Some(tokens.to_string_lossy().into_owned()),
                num_threads: std::thread::available_parallelism()
                    .map(|n| n.get() as i32)
                    .unwrap_or(2)
                    .min(4),
                debug: false,
                ..Default::default()
            },
            decoding_method: Some("greedy_search".into()),
            ..Default::default()
        };

        let recognizer = OfflineRecognizer::create(&config)
            .ok_or_else(|| VizierError("failed to create Whisper STT engine".into()))?;

        let mut lock = self.recognizer.lock();
        *lock = Some(recognizer);

        Ok(lock)
    }
}

#[async_trait::async_trait]
impl VizierSttModel for WhisperSttModel {
    async fn transcribe(
        &self,
        audio: &[u8],
        filename: &str,
        _language: Option<&str>,
    ) -> Result<String> {
        let lock = self.ensure_recognizer().await?;
        let recognizer = lock
            .as_ref()
            .ok_or_else(|| VizierError("Whisper STT not initialized".into()))?;

        let temp_dir = std::env::temp_dir();
        let ext = std::path::Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("wav");
        let temp_path = temp_dir.join(format!("vizier_stt_{}.{}", nanoid::nanoid!(), ext));

        std::fs::write(&temp_path, audio)
            .map_err(|e| VizierError(format!("write temp audio: {e}")))?;

        let wave = Wave::read(temp_path.to_str().unwrap_or("audio.wav")).ok_or_else(|| {
            let _ = std::fs::remove_file(&temp_path);
            VizierError("failed to read audio file (expected WAV format)".into())
        })?;

        let stream = recognizer.create_stream();
        stream.accept_waveform(wave.sample_rate(), wave.samples());
        recognizer.decode(&stream);

        let _ = std::fs::remove_file(&temp_path);

        let result = stream
            .get_result()
            .ok_or_else(|| VizierError("whisper transcription failed".into()))?;

        Ok(result.text)
    }
}
