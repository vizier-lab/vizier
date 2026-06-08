use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    VizierError,
    agents::tools::{ToolContext, VizierTool},
    file_manager::FileManager,
    storage::{VizierStorage, session_file::SessionFileStorage},
    stt::VizierStt,
};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SttTranscribeArgs {
    #[schemars(description = "Filename of the audio file in session files to transcribe")]
    pub filename: String,
    #[schemars(description = "Language hint for transcription (e.g. 'en', 'zh', 'auto'). Optional.")]
    pub language: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SttTranscribeOutput {
    pub text: String,
}

pub struct SttTranscribe {
    pub stt: Arc<VizierStt>,
    pub storage: Arc<VizierStorage>,
    pub file_manager: FileManager,
    pub language: Option<String>,
}

#[async_trait::async_trait]
impl VizierTool for SttTranscribe {
    type Input = SttTranscribeArgs;
    type Output = SttTranscribeOutput;

    fn name() -> String {
        "stt_transcribe".to_string()
    }

    fn description(&self) -> String {
        let lang_hint = self
            .language
            .as_deref()
            .unwrap_or("auto");
        format!(
            "Transcribe audio to text using STT. Provide a filename from session files. Default language: \"{}\".",
            lang_hint
        )
    }

    async fn call(
        &self,
        args: Self::Input,
        ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let session_file = self
            .storage
            .get_session_file(&ctx.session, &args.filename)
            .await
            .map_err(|e| VizierError(e.to_string()))?
            .ok_or_else(|| {
                VizierError(format!("session file not found: '{}'", args.filename))
            })?;

        let (_stored_filename, audio_bytes) = self
            .file_manager
            .get(&session_file.file_id)
            .await
            .map_err(|e| VizierError(e.to_string()))?;

        let language = args.language.as_deref().or(self.language.as_deref());

        let text = self
            .stt
            .transcribe(&audio_bytes, &args.filename, language)
            .await?;

        Ok(SttTranscribeOutput { text })
    }
}
