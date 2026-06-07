use std::sync::Arc;

use anyhow::Result;
use base64::Engine;
use calamine::Reader;
use chrono::Utc;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{ToolContext, VizierTool},
    config::provider::ProviderVariant,
    error::VizierError,
    file_manager::FileManager,
    schema::{VizierResponse, VizierResponseContent, context_file::ContextFileRecord},
    storage::{VizierStorage, context_file::ContextFileStorage},
};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ListContextFilesArgs {}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ListContextFilesOutput {
    pub files: Vec<ContextFileSummary>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ContextFileSummary {
    pub filename: String,
    pub mime_type: String,
    pub size: u64,
}

pub struct ListContextFiles {
    pub storage: Arc<VizierStorage>,
}

#[async_trait::async_trait]
impl VizierTool for ListContextFiles {
    type Input = ListContextFilesArgs;
    type Output = ListContextFilesOutput;

    fn name() -> String {
        "list_context_files".to_string()
    }

    fn description(&self) -> String {
        "List files available in the current session context. Use this to see what files have been attached or uploaded before reading them with read_context_file.".to_string()
    }

    async fn call(
        &self,
        _args: Self::Input,
        ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let records = self
            .storage
            .list_context_files(&ctx.session)
            .await
            .map_err(|e| VizierError(e.to_string()))?;

        let files = records
            .into_iter()
            .map(|r| ContextFileSummary {
                filename: r.filename,
                mime_type: r.mime_type,
                size: r.size,
            })
            .collect();

        Ok(ListContextFilesOutput { files })
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ReadContextFileArgs {
    #[schemars(description = "filename of the context file to read")]
    pub filename: String,
}

pub struct ReadContextFile {
    pub storage: Arc<VizierStorage>,
    pub file_manager: FileManager,
    pub provider: ProviderVariant,
}

enum ExtractResult {
    Text(String),
    Attachment(String), // base64 encoded
}

fn extract_content(
    mime_type: &str,
    content: Vec<u8>,
    provider: &ProviderVariant,
) -> Result<ExtractResult, VizierError> {
    if mime_type.starts_with("image/") {
        let b64 = base64::engine::general_purpose::STANDARD.encode(&content);
        return Ok(ExtractResult::Attachment(b64));
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
            Ok(ExtractResult::Text(text))
        }
        "application/pdf" => {
            let text = pdf_extract::extract_text_from_mem(&content)
                .map_err(|e| VizierError(format!("Failed to extract PDF: {}", e)))?;
            Ok(ExtractResult::Text(text))
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
            Ok(ExtractResult::Text(text))
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
            Ok(ExtractResult::Text(csv))
        }
        _ => {
            // Try as UTF-8 text, fall back to error
            match String::from_utf8(content) {
                Ok(text) => Ok(ExtractResult::Text(text)),
                Err(_) => Err(VizierError(format!("Unsupported file type: {}", mime_type))),
            }
        }
    }
}

#[async_trait::async_trait]
impl VizierTool for ReadContextFile {
    type Input = ReadContextFileArgs;
    type Output = VizierResponse;

    fn name() -> String {
        "read_context_file".to_string()
    }

    fn description(&self) -> String {
        "Read a file from the current session context. Returns the file content as text, or injects images/PDFs (on supported models) into the conversation context.".to_string()
    }

    async fn call(
        &self,
        args: Self::Input,
        ctx: &ToolContext,
    ) -> Result<Self::Output, VizierError> {
        let file = self
            .storage
            .get_context_file(&ctx.session, &args.filename)
            .await
            .map_err(|e| VizierError(e.to_string()))?
            .ok_or_else(|| VizierError(format!("File not found: {}", args.filename)))?;

        let (_, content) = self
            .file_manager
            .get(&file.file_id)
            .await
            .map_err(|e| VizierError(e.to_string()))?;

        match extract_content(&file.mime_type, content, &self.provider)? {
            ExtractResult::Text(text) => Ok(VizierResponse {
                timestamp: Utc::now(),
                content: VizierResponseContent::ToolResponse {
                    response: serde_json::Value::String(text),
                },
                attachments: vec![],
            }),
            ExtractResult::Attachment(b64) => Ok(VizierResponse {
                timestamp: Utc::now(),
                content: VizierResponseContent::ToolResponse {
                    response: serde_json::Value::String(format!(
                        "Loaded {} into context.",
                        file.filename
                    )),
                },
                attachments: vec![crate::schema::VizierAttachment {
                    filename: file.filename,
                    content: crate::schema::VizierAttachmentContent::Base64(b64),
                }],
            }),
        }
    }
}
