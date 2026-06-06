use std::sync::Arc;

use chrono::Utc;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::agents::agent::image_processor::VizierImageProcessor;
use crate::agents::context_files::{ContextFileSummary, ContextFiles, ExtractionResult, provider_supports_pdf};
use crate::agents::tools::VizierTool;
use crate::config::provider::ProviderVariant;
use crate::error::VizierError;
use crate::schema::{VizierAttachment, VizierAttachmentContent, VizierResponse, VizierResponseContent};

// ListContextFiles

pub struct ListContextFiles {
    context_files: ContextFiles,
}

impl ListContextFiles {
    pub fn new(context_files: ContextFiles) -> Self {
        Self { context_files }
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ListContextFilesArgs {}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ListContextFilesOutput {
    files: Vec<ContextFileSummary>,
}

#[async_trait::async_trait]
impl VizierTool for ListContextFiles {
    type Input = ListContextFilesArgs;
    type Output = ListContextFilesOutput;

    fn name() -> String {
        "list_context_files".to_string()
    }

    fn description(&self) -> String {
        "List all files available in your context files. Returns filenames, sizes, and types.".to_string()
    }

    async fn call(&self, _args: Self::Input) -> Result<Self::Output, VizierError> {
        let files = self.context_files.list();
        let summaries = files.iter().map(ContextFileSummary::from).collect();
        Ok(ListContextFilesOutput { files: summaries })
    }
}

// ReadContextFile

pub struct ReadContextFile {
    context_files: ContextFiles,
    provider: ProviderVariant,
}

impl ReadContextFile {
    pub fn new(context_files: ContextFiles, provider: ProviderVariant) -> Self {
        Self { context_files, provider }
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ReadContextFileArgs {
    #[schemars(description = "The filename to read from context files")]
    filename: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ReadContextFileOutput {
    content: String,
}

#[async_trait::async_trait]
impl VizierTool for ReadContextFile {
    type Input = ReadContextFileArgs;
    type Output = ReadContextFileOutput;

    fn name() -> String {
        "read_context_file".to_string()
    }

    fn description(&self) -> String {
        "Read a file from context files. Supports text, PDF (on supported providers), DOCX, XLSX, PPTX. Images should be read with read_image instead.".to_string()
    }

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
        let supports_pdf = provider_supports_pdf(&self.provider);
        let result = self.context_files.extract(&args.filename, supports_pdf)?;

        let content = match result {
            ExtractionResult::Text(text) => text,
            ExtractionResult::InjectContext(b64) => {
                format!("[PDF file content - base64 encoded]\n{}", b64)
            }
            ExtractionResult::Redirect(msg) => msg,
            ExtractionResult::Unsupported(msg) => msg,
        };

        Ok(ReadContextFileOutput { content })
    }
}

// AddContextFile

pub struct AddContextFile {
    context_files: ContextFiles,
}

impl AddContextFile {
    pub fn new(context_files: ContextFiles) -> Self {
        Self { context_files }
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct AddContextFileArgs {
    #[schemars(description = "The filename to store the file as")]
    filename: String,
    #[schemars(description = "The content of the file (use base64 for binary files)")]
    content: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct AddContextFileOutput {
    file: ContextFileSummary,
}

#[async_trait::async_trait]
impl VizierTool for AddContextFile {
    type Input = AddContextFileArgs;
    type Output = AddContextFileOutput;

    fn name() -> String {
        "add_context_file".to_string()
    }

    fn description(&self) -> String {
        "Add a file to your context files. For binary files, provide content as base64.".to_string()
    }

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
        let is_binary = !args.content.is_ascii() || args.content.len() > 1000 && args.content.contains('=');

        let bytes = if is_binary {
            base64::Engine::decode(
                &base64::engine::general_purpose::STANDARD,
                &args.content,
            )
            .map_err(|e| VizierError(format!("Invalid base64: {}", e)))?
        } else {
            args.content.into_bytes()
        };

        let file = self.context_files.add(&args.filename, bytes)?;
        Ok(AddContextFileOutput {
            file: ContextFileSummary::from(&file),
        })
    }
}

// ReadImage

pub struct ReadImage {
    context_files: ContextFiles,
    processor: Arc<VizierImageProcessor>,
}

impl ReadImage {
    pub fn new(context_files: ContextFiles, processor: Arc<VizierImageProcessor>) -> Self {
        Self { context_files, processor }
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ReadImageArgs {
    #[schemars(description = "The image filename to read from context files")]
    filename: String,
    #[schemars(description = "The instruction/prompt for how the image model should process this image")]
    prompt: String,
}

#[async_trait::async_trait]
impl VizierTool for ReadImage {
    type Input = ReadImageArgs;
    type Output = VizierResponse;

    fn name() -> String {
        "read_image".to_string()
    }

    fn description(&self) -> String {
        "Read an image from context files. If the image model is the same as your main model, the image will be added to the conversation context. Otherwise, the image model will describe the image based on your prompt.".to_string()
    }

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
        let (base64, mime_type) = self.context_files.read_image_base64(&args.filename)?;

        if self.processor.is_same_as_main_model() {
            Ok(VizierResponse {
                timestamp: Utc::now(),
                content: VizierResponseContent::ToolResponse {
                    response: serde_json::Value::String(format!(
                        "Image '{}' loaded into context. Prompt: {}",
                        args.filename, args.prompt
                    )),
                },
                attachments: vec![VizierAttachment {
                    filename: args.filename.clone(),
                    content: VizierAttachmentContent::Base64(base64),
                }],
            })
        } else {
            let description = self.processor.describe(&base64, &mime_type, &args.prompt)
                .await
                .map_err(|e| VizierError(e.to_string()))?;
            Ok(VizierResponse {
                timestamp: Utc::now(),
                content: VizierResponseContent::ToolResponse {
                    response: serde_json::Value::String(description),
                },
                attachments: vec![],
            })
        }
    }
}
