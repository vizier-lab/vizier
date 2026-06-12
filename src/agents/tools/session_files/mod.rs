use std::sync::Arc;

use anyhow::Result;
use calamine::Reader;
use chrono::Utc;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{ToolContext, VizierTool},
    error::VizierError,
    file_manager::FileManager,
    schema::{VizierResponse, VizierResponseContent, session_file::SessionFileRecord},
    storage::{VizierStorage, session_file::SessionFileStorage},
};

mod send_attachment;
pub use send_attachment::SendAttachment;

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
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => {
            let docx = docx_rs::read_docx(&content)
                .map_err(|e| VizierError(format!("Failed to read DOCX: {}", e)))?;
            let mut text = String::new();
            for child in docx.document.children {
                match child {
                    docx_rs::DocumentChild::Paragraph(p) => {
                        for run in p.children {
                            if let docx_rs::ParagraphChild::Run(r) = run {
                                for text_run in r.children {
                                    if let docx_rs::RunChild::Text(t) = text_run {
                                        text.push_str(&t.text);
                                    }
                                }
                            }
                        }
                        text.push('\n');
                    }
                    docx_rs::DocumentChild::Table(_) => {
                        text.push_str("[table content]\n");
                    }
                    _ => {}
                }
            }
            Ok(text)
        }
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => {
            let mut workbook = calamine::open_workbook_from_rs::<calamine::Xlsx<_>, _>(
                std::io::Cursor::new(content),
            )
            .map_err(|e| VizierError(format!("Failed to read XLSX: {}", e)))?;

            let mut csv = String::new();
            for name in workbook.sheet_names() {
                if let Ok(range) = workbook.worksheet_range(&name) {
                    csv.push_str(&format!("## {}\n\n", name));
                    for row in range.rows() {
                        let cells: Vec<String> = row
                            .iter()
                            .map(|cell| match cell {
                                calamine::Data::Empty => String::new(),
                                calamine::Data::String(s) => s.clone(),
                                calamine::Data::Float(f) => format!("{}", f),
                                calamine::Data::Int(i) => format!("{}", i),
                                calamine::Data::Bool(b) => format!("{}", b),
                                calamine::Data::Error(e) => format!("ERR:{:?}", e),
                                calamine::Data::DateTime(dt) => format!("{}", dt),
                                calamine::Data::DateTimeIso(s) => s.clone(),
                                calamine::Data::DurationIso(s) => s.clone(),
                            })
                            .collect();
                        csv.push_str(&cells.join(","));
                        csv.push('\n');
                    }
                    csv.push('\n');
                }
            }
            Ok(csv)
        }
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

    fn name() -> String {
        "read_document_file".to_string()
    }

    fn description(&self) -> String {
        "Read a textual document from the current session (plain text, JSON, YAML, CSV, Markdown, HTML, CSS, JavaScript, XML, TOML, PDF, DOCX, XLSX). Returns the extracted text content. For images, use read_image_file.".to_string()
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
        })
    }
}
