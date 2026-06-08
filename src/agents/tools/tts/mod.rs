use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    VizierError,
    agents::tools::{ToolContext, VizierTool},
    file_manager::FileManager,
    storage::{VizierStorage, session_file::SessionFileStorage},
    tts::VizierTts,
};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct TtsGenerateArgs {
    #[schemars(
        description = "The text to convert to speech. send only a plain text with no heading. only use common punctuation (? . , !)."
    )]
    pub text: String,
    #[schemars(description = "Output filename (optional, defaults to tts_{uuid}.wav)")]
    pub filename: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct TtsGenerateOutput {
    pub filename: String,
    pub size: u64,
}

pub struct TtsGenerate {
    pub tts: Arc<VizierTts>,
    pub storage: Arc<VizierStorage>,
    pub file_manager: FileManager,
    pub voice: String,
    pub speed: f32,
}

#[async_trait::async_trait]
impl VizierTool for TtsGenerate {
    type Input = TtsGenerateArgs;
    type Output = TtsGenerateOutput;

    fn name() -> String {
        "tts_generate".to_string()
    }

    fn description(&self) -> String {
        format!(
            "Generate speech audio from text using TTS. The audio file is saved to the session files. Default voice: \"{}\", speed: {}.",
            self.voice, self.speed
        )
    }

    async fn call(
        &self,
        args: Self::Input,
        ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let voice = &self.voice;
        let speed = self.speed;

        let audio_bytes = self.tts.generate_speech(&args.text, voice, speed).await?;

        let filename = args
            .filename
            .unwrap_or_else(|| format!("tts_{}.wav", uuid::Uuid::new_v4()));

        let file_record = self
            .file_manager
            .upload(&filename, audio_bytes)
            .await
            .map_err(|e| VizierError(e.to_string()))?;

        let mime_type = "audio/wav".to_string();
        self.storage
            .save_session_file(
                &ctx.session,
                &filename,
                &mime_type,
                file_record.size,
                &file_record.id,
            )
            .await
            .map_err(|e| VizierError(e.to_string()))?;

        Ok(TtsGenerateOutput {
            filename,
            size: file_record.size,
        })
    }
}
