use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{
        ToolCategory, ToolContext, VizierTool,
        office::{DOCX_MIME, XLS_MIME, XLSX_MIME},
    },
    error::VizierError,
    file_manager::FileManager,
    schema::{VizierResponse, VizierResponseContent, session_file::SessionFileRecord},
    storage::{VizierStorage, session_file::SessionFileStorage},
};

mod send_attachment;
pub use send_attachment::SendAttachment;

mod paths;
pub use paths::{GetSessionFilePath, LoadPathAsSessionFile};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ListSessionFilesArgs {}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ListSessionFilesOutput {
    pub files: Vec<SessionFileSummary>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SessionFileSummary {
    pub filename: String,
    pub mime_type: String,
    pub size: u64,
}

pub struct ListSessionFiles {
    pub storage: Arc<VizierStorage>,
}

#[async_trait::async_trait]
impl VizierTool for ListSessionFiles {
    type Input = ListSessionFilesArgs;
    type Output = ListSessionFilesOutput;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Files)
    }

    fn name() -> String {
        "list_session_files".to_string()
    }

    fn description(&self) -> String {
        "List files available in the current session. Use this to see what files have been attached or uploaded before reading them with read_document_file or read_image_file.".to_string()
    }

    async fn call(
        &self,
        _args: Self::Input,
        ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let records = self
            .storage
            .list_session_files(&ctx.session)
            .await
            .map_err(|e| VizierError(e.to_string()))?;

        let files = records
            .into_iter()
            .map(|r| SessionFileSummary {
                filename: r.filename,
                mime_type: r.mime_type,
                size: r.size,
            })
            .collect();

        Ok(ListSessionFilesOutput { files })
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ReadDocumentFileArgs {
    #[schemars(description = "filename of the session file to read")]
    pub filename: String,
}

pub struct ReadDocumentFile {
    pub storage: Arc<VizierStorage>,
    pub file_manager: FileManager,
}

fn extract_text(mime_type: &str, content: Vec<u8>) -> Result<String, VizierError> {
    if mime_type.starts_with("image/") {
        return Err(VizierError(format!(
            "read_document_file does not handle image/* MIME types (got '{}'); use read_image_file instead.",
            mime_type
        )));
    }

    match mime_type {
        "text/plain"
        | "text/csv"
        | "application/json"
        | "text/yaml"
        | "text/markdown"
        | "text/html"
        | "text/css"
        | "text/javascript"
        | "application/javascript"
        | "application/xml"
        | "text/xml"
        | "application/toml"
        | "application/x-yaml" => {
            let text = String::from_utf8(content)
                .map_err(|e| VizierError(format!("Invalid UTF-8: {}", e)))?;
            Ok(text)
        }
        "application/pdf" => {
            let text = pdf_extract::extract_text_from_mem(&content)
                .map_err(|e| VizierError(format!("Failed to extract PDF: {}", e)))?;
            Ok(text)
        }
        // Word and Excel are read by the Office addon (specs/034-office-addon).
        DOCX_MIME | XLSX_MIME | XLS_MIME => Err(VizierError(
            "read_document_file no longer reads Word and Excel files. Use office_docx_read_as_md for .docx, or office_xlsx_read_as_cells (typed cells) / office_xlsx_read_as_csv (whole-workbook preview) for .xlsx and .xls. These tools belong to the Office addon: install it or turn it on in the agent's integrations if they are missing.".into(),
        )),
        _ => {
            // Try as UTF-8 text, fall back to error
            match String::from_utf8(content) {
                Ok(text) => Ok(text),
                Err(_) => Err(VizierError(format!("Unsupported file type: {}", mime_type))),
            }
        }
    }
}

#[async_trait::async_trait]
impl VizierTool for ReadDocumentFile {
    type Input = ReadDocumentFileArgs;
    type Output = VizierResponse;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Files)
    }

    fn name() -> String {
        "read_document_file".to_string()
    }

    fn description(&self) -> String {
        "Read a textual document from the current session (plain text, JSON, YAML, CSV, Markdown, HTML, CSS, JavaScript, XML, TOML, PDF). Returns the extracted text content. For images, use read_image_file. For Word (.docx) and Excel (.xlsx, .xls) files use the office_* tools of the Office addon instead.".to_string()
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

        let (_, content) = self
            .file_manager
            .get(&file.file_id)
            .await
            .map_err(|e| VizierError(e.to_string()))?;

        let text = extract_text(&file.mime_type, content)?;

        Ok(VizierResponse {
            timestamp: Utc::now(),
            content: VizierResponseContent::ToolResponse {
                response: serde_json::Value::String(text),
            },
            attachments: vec![],
            ..Default::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::extract_text;
    use crate::agents::tools::office::{DOCX_MIME, XLS_MIME, XLSX_MIME};

    #[test]
    fn word_and_excel_point_to_the_office_tools() {
        for mime in [DOCX_MIME, XLSX_MIME, XLS_MIME] {
            let message = extract_text(mime, b"x".to_vec()).unwrap_err().0;
            assert!(message.contains("office_docx_read_as_md"), "{mime}: {message}");
            assert!(message.contains("office_xlsx_read_as_cells"), "{mime}: {message}");
            assert!(message.contains("Office addon"), "{mime}: {message}");
        }
    }

    #[test]
    fn text_still_reads_as_before() {
        assert_eq!(extract_text("text/plain", b"hello".to_vec()).unwrap(), "hello");
        assert!(extract_text("application/pdf", b"not a pdf".to_vec()).is_err());
    }
}
