use std::path::PathBuf;
use std::sync::Arc;

use any_tts::{load_model, ModelType, SynthesisRequest, TtsConfig, TtsModel};

use crate::setup::{DownloadKind, SetupBoard};
use crate::tts::VizierTtsModel;
use crate::utils::build_path;
use crate::{Result, VizierError};

/// Where the Kokoro weights and voice packs are published.
const DEFAULT_REPO: &str = "hexgrad/Kokoro-82M";

/// The voice any-tts uses when none is chosen; `af` was sherpa's name for it.
const DEFAULT_VOICE: &str = "af_heart";

/// Voice packs shipped by the Kokoro-82M release.
const VOICES: &[&str] = &[
    "af_alloy",
    "af_aoede",
    "af_bella",
    "af_heart",
    "af_jessica",
    "af_kore",
    "af_nicole",
    "af_nova",
    "af_river",
    "af_sarah",
    "af_sky",
    "am_adam",
    "am_echo",
    "am_eric",
    "am_fenrir",
    "am_liam",
    "am_michael",
    "am_onyx",
    "am_puck",
    "am_santa",
    "bf_alice",
    "bf_emma",
    "bf_isabella",
    "bf_lily",
    "bm_daniel",
    "bm_fable",
    "bm_george",
    "bm_lewis",
    "ef_dora",
    "em_alex",
    "em_santa",
    "ff_siwis",
    "hf_alpha",
    "hf_beta",
    "hm_omega",
    "hm_psi",
    "if_sara",
    "im_nicola",
    "jf_alpha",
    "jf_gongitsune",
    "jf_nezumi",
    "jf_tebukuro",
    "jm_kumo",
    "pf_dora",
    "pm_alex",
    "pm_santa",
    "zf_xiaobei",
    "zf_xiaoni",
    "zf_xiaoxiao",
    "zf_xiaoyi",
    "zm_yunjian",
    "zm_yunxi",
    "zm_yunxia",
    "zm_yunyang",
];

/// A saved agent may still name a sherpa artifact; all of them are the same Kokoro-82M
/// checkpoint under any-tts, so map the old names onto it.
fn resolve_repo(model_name: &str) -> (String, String) {
    let name = model_name.trim();
    let name = match name {
        "" | "kokoro-en-v0_19" | "kokoro-multi-lang-v1_0" | "kokoro-multi-lang-v1_1" => {
            "Kokoro-82M"
        }
        other => other,
    };

    if name.contains('/') {
        let dir = name.rsplit('/').next().unwrap_or(name).to_string();
        (name.to_string(), dir)
    } else {
        (format!("hexgrad/{name}"), name.to_string())
    }
}

/// The name any-tts will look for in its `voices/` directory. Unknown names (including the
/// numeric ids sherpa accepted) fall back to the default voice with a warning.
fn resolve_voice(voice: &str) -> String {
    let voice = voice.trim();
    if voice.is_empty() || voice == "af" {
        return DEFAULT_VOICE.to_string();
    }
    if VOICES.contains(&voice) {
        return voice.to_string();
    }

    tracing::warn!("Unknown Kokoro voice '{voice}', using {DEFAULT_VOICE}");
    DEFAULT_VOICE.to_string()
}

pub struct KokoroTtsModel {
    model_name: String,
    repo: String,
    model_dir: PathBuf,
    tts: Arc<parking_lot::Mutex<Option<Box<dyn TtsModel>>>>,
    /// Reports the one-time model download to the WebUI setup board.
    setup: SetupBoard,
    agent_id: String,
}

impl KokoroTtsModel {
    pub fn new(model_name: String, workspace: &str, setup: SetupBoard, agent_id: String) -> Self {
        let (repo, dir_name) = resolve_repo(&model_name);
        let model_dir = build_path(workspace, &[".runtime", "tts", "kokoro", &dir_name]);
        Self {
            model_name,
            repo,
            model_dir,
            tts: Arc::new(parking_lot::Mutex::new(None)),
            setup,
            agent_id,
        }
    }

    fn file_url(&self, file: &str) -> String {
        format!("https://huggingface.co/{}/resolve/main/{file}?download=true", self.repo)
    }

    fn ready_marker(&self) -> PathBuf {
        self.model_dir.join(".ready")
    }

    async fn ensure_model(&self) -> Result<()> {
        if self.ready_marker().exists() {
            return Ok(());
        }

        std::fs::create_dir_all(self.model_dir.join("voices"))
            .map_err(|e| VizierError(format!("create kokoro model dir: {e}")))?;

        let client = reqwest::Client::builder()
            .user_agent("vizier/0.10")
            .build()
            .map_err(|e| VizierError(format!("build download client: {e}")))?;

        tracing::info!(
            "Downloading Kokoro model '{}' from {}",
            self.model_name,
            self.repo
        );

        self.setup.download_started(
            &self.agent_id,
            DownloadKind::Tts,
            format!("Downloading speech model '{}'", self.model_name),
        );

        let config = self.model_dir.join("config.json");
        self.download(&client, "config.json", &config, false)
            .await?;

        // Kokoro releases publish either safetensors or the original `.pth`.
        let safetensors = self.model_dir.join("model.safetensors");
        if !safetensors.exists() {
            let found = self
                .download(&client, "model.safetensors", &safetensors, true)
                .await?;
            if !found {
                let pth = self.model_dir.join("kokoro-v1_0.pth");
                self.download(&client, "kokoro-v1_0.pth", &pth, false)
                    .await?;
            }
        }

        // any-tts only exposes a voices directory that exists when the model loads.
        let starter_voice = self.model_dir.join("voices").join(format!("{DEFAULT_VOICE}.pt"));
        if !starter_voice.exists() {
            let file = format!("voices/{DEFAULT_VOICE}.pt");
            self.download(&client, &file, &starter_voice, true).await?;
        }

        self.setup
            .download_finished(&self.agent_id, DownloadKind::Tts);

        std::fs::write(self.ready_marker(), b"ok")
            .map_err(|e| VizierError(format!("write kokoro ready marker: {e}")))?;

        tracing::info!("Kokoro model ready at {}", self.model_dir.display());
        Ok(())
    }

    async fn download(
        &self,
        client: &reqwest::Client,
        file: &str,
        dest: &std::path::Path,
        optional: bool,
    ) -> Result<bool> {
        let url = self.file_url(file);
        self.setup
            .download_file(
                client,
                &url,
                dest,
                &self.agent_id,
                DownloadKind::Tts,
                file,
                optional,
            )
            .await
            .inspect_err(|e| {
                self.setup
                    .download_failed(&self.agent_id, DownloadKind::Tts, e.to_string());
            })
    }

    /// Voice packs are downloaded on first use, so the initial model fetch stays small.
    async fn ensure_voice(&self, voice: &str) -> Result<()> {
        let path = self.model_dir.join("voices").join(format!("{voice}.pt"));
        if path.exists() {
            return Ok(());
        }

        std::fs::create_dir_all(self.model_dir.join("voices"))
            .map_err(|e| VizierError(format!("create kokoro voices dir: {e}")))?;

        self.setup.download_started(
            &self.agent_id,
            DownloadKind::Tts,
            format!("Downloading speech model '{}'", self.model_name),
        );
        let client = reqwest::Client::builder()
            .user_agent("vizier/0.10")
            .build()
            .map_err(|e| VizierError(format!("build download client: {e}")))?;

        let result = self
            .download(&client, &format!("voices/{voice}.pt"), &path, false)
            .await;

        match &result {
            Ok(_) => self.setup.download_finished(&self.agent_id, DownloadKind::Tts),
            Err(e) => self
                .setup
                .download_failed(&self.agent_id, DownloadKind::Tts, e.to_string()),
        }
        result.map(|_| ())
    }

}

#[async_trait::async_trait]
impl VizierTtsModel for KokoroTtsModel {
    async fn generate_speech(&self, text: &str, voice: &str, speed: f32) -> Result<Vec<u8>> {
        let voice = resolve_voice(voice);
        // The model dir and its starter voice must exist before the engine loads; the voice
        // packs are fetched on first use.
        self.ensure_model().await?;
        self.ensure_voice(&voice).await?;

        let request = SynthesisRequest::new(text)
            .with_voice(&voice)
            // any-tts accepts 0.1..=5.0; the trait's speed is a plain multiplier.
            .with_speed(speed.clamp(0.1, 5.0) as f64);

        // Loading the weights and synthesising are CPU-bound and take seconds; keep them off
        // the async workers so other turns keep running. The lock makes concurrent requests
        // share one loaded engine and take turns on it.
        let slot = self.tts.clone();
        let model_dir = self.model_dir.clone();
        tokio::task::spawn_blocking(move || {
            let mut slot = slot.lock();
            if slot.is_none() {
                let tts = load_model(
                    TtsConfig::new(ModelType::Kokoro)
                        .with_model_path(model_dir.to_string_lossy().into_owned()),
                )
                .map_err(|e| VizierError(format!("load Kokoro TTS engine: {e}")))?;
                *slot = Some(tts);
            }

            let tts = slot
                .as_ref()
                .ok_or_else(|| VizierError("Kokoro TTS not initialized".into()))?;
            let audio = tts
                .synthesize(&request)
                .map_err(|e| VizierError(format!("Kokoro TTS generation failed: {e}")))?;
            Ok(audio.get_wav())
        })
        .await
        .map_err(|e| VizierError(format!("Kokoro TTS task: {e}")))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_sherpa_models_map_to_the_kokoro_release() {
        for old in ["kokoro-en-v0_19", "kokoro-multi-lang-v1_0", "kokoro-multi-lang-v1_1"] {
            assert_eq!(resolve_repo(old), ("hexgrad/Kokoro-82M".into(), "Kokoro-82M".into()));
        }
        assert_eq!(
            resolve_repo("Kokoro-82M"),
            ("hexgrad/Kokoro-82M".into(), "Kokoro-82M".into())
        );
        assert_eq!(
            resolve_repo("someone/Kokoro-Tiny"),
            ("someone/Kokoro-Tiny".into(), "Kokoro-Tiny".into())
        );
    }

    #[test]
    fn sherpa_voice_names_keep_their_kokoro_packs() {
        assert_eq!(resolve_voice("af"), "af_heart");
        assert_eq!(resolve_voice("af_bella"), "af_bella");
        // Numeric ids and unknown names fall back to the default.
        assert_eq!(resolve_voice("3"), "af_heart");
        assert_eq!(resolve_voice("nope"), "af_heart");
    }
}
