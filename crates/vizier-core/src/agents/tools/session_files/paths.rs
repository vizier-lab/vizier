use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{ToolCategory, ToolContext, VizierTool},
    error::VizierError,
    file_manager::FileManager,
    storage::{VizierStorage, session_file::SessionFileStorage},
};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct GetSessionFilePathArgs {
    #[schemars(description = "filename of the session file")]
    pub filename: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct GetSessionFilePathOutput {
    pub filename: String,
    pub path: String,
}

pub struct GetSessionFilePath {
    pub storage: Arc<VizierStorage>,
    pub file_manager: FileManager,
}

#[async_trait::async_trait]
impl VizierTool for GetSessionFilePath {
    type Input = GetSessionFilePathArgs;
    type Output = GetSessionFilePathOutput;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Files)
    }

    fn name() -> String {
        "get_session_file_path".to_string()
    }

    fn description(&self) -> String {
        "Get the actual absolute filesystem path of a session file, e.g. to pass it to a shell command. Treat the file as read-only: it is the stored copy.".to_string()
    }

    async fn call(
        &self,
        args: Self::Input,
        ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let file = self
            .storage
            .get_session_file(&ctx.session, &args.filename)
            .await
            .map_err(|e| VizierError(e.to_string()))?
            .ok_or_else(|| VizierError(format!("File not found: {}", args.filename)))?;

        let path = self
            .file_manager
            .path_of(&file.file_id)
            .await
            .map_err(|e| VizierError(e.to_string()))?;

        Ok(GetSessionFilePathOutput {
            filename: file.filename,
            path: path.to_string_lossy().to_string(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct LoadPathAsSessionFileArgs {
    #[schemars(description = "path of the file on the filesystem to add to the session")]
    pub path: String,
    #[schemars(description = "optional session filename; defaults to the file's own name")]
    pub filename: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct LoadPathAsSessionFileOutput {
    pub filename: String,
    pub mime_type: String,
    pub size: u64,
}

pub struct LoadPathAsSessionFile {
    pub storage: Arc<VizierStorage>,
    pub file_manager: FileManager,
}

#[async_trait::async_trait]
impl VizierTool for LoadPathAsSessionFile {
    type Input = LoadPathAsSessionFileArgs;
    type Output = LoadPathAsSessionFileOutput;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Files)
    }

    fn name() -> String {
        "load_path_as_session_file".to_string()
    }

    fn description(&self) -> String {
        "Copy a file from any filesystem path into the current session as a session file, so it can be read with read_document_file / read_image_file or sent with send_attachment. An existing session file with the same name is replaced.".to_string()
    }

    async fn call(
        &self,
        args: Self::Input,
        ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let path = std::path::Path::new(&args.path);
        let meta = tokio::fs::metadata(path)
            .await
            .map_err(|e| VizierError(format!("Cannot access {}: {}", args.path, e)))?;
        if !meta.is_file() {
            return Err(VizierError(format!("{} is not a regular file", args.path)));
        }

        let filename = match args.filename {
            Some(name) => name,
            None => path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .ok_or_else(|| VizierError(format!("{} has no file name", args.path)))?,
        };
        if filename.is_empty() || filename.contains('/') || filename.contains('\\') {
            return Err(VizierError(format!("Invalid filename: {}", filename)));
        }

        let content = tokio::fs::read(path)
            .await
            .map_err(|e| VizierError(format!("Failed to read {}: {}", args.path, e)))?;

        let record = self
            .file_manager
            .upload(&filename, content)
            .await
            .map_err(|e| VizierError(e.to_string()))?;

        self.storage
            .save_session_file(
                &ctx.session,
                &record.filename,
                &record.mime_type,
                record.size,
                &record.id,
            )
            .await
            .map_err(|e| VizierError(e.to_string()))?;

        Ok(LoadPathAsSessionFileOutput {
            filename: record.filename,
            mime_type: record.mime_type,
            size: record.size,
        })
    }
}
