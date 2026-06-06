use std::io::Read;

use calamine::Reader;

use crate::config::provider::ProviderVariant;

#[derive(Debug, Clone)]
pub enum ExtractionResult {
    Text(String),
    InjectContext(String),
    Redirect(String),
    Unsupported(String),
}

pub fn extract_text(
    bytes: &[u8],
    mime_type: &str,
    supports_pdf: bool,
) -> ExtractionResult {
    match mime_type {
        "text/plain" | "text/csv" | "text/markdown" | "text/xml" | "application/json"
        | "application/xml" | "text/html" | "text/css" | "text/rtf" => {
            ExtractionResult::InjectContext(String::from_utf8_lossy(bytes).to_string())
        }
        "application/pdf" if supports_pdf => {
            let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes);
            ExtractionResult::InjectContext(b64)
        }
        "application/pdf" => match extract_pdf(bytes) {
            Ok(text) => ExtractionResult::Text(text),
            Err(e) => ExtractionResult::Text(format!("PDF extraction error: {}", e)),
        },
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => {
            match extract_docx(bytes) {
                Ok(text) => ExtractionResult::Text(text),
                Err(e) => ExtractionResult::Text(format!("DOCX extraction error: {}", e)),
            }
        }
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => {
            match extract_xlsx(bytes) {
                Ok(text) => ExtractionResult::Text(text),
                Err(e) => ExtractionResult::Text(format!("XLSX extraction error: {}", e)),
            }
        }
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => {
            match extract_pptx(bytes) {
                Ok(text) => ExtractionResult::Text(text),
                Err(e) => ExtractionResult::Text(format!("PPTX extraction error: {}", e)),
            }
        }
        mt if mt.starts_with("image/") => {
            ExtractionResult::Redirect("Use read_image tool for image files".into())
        }
        _ => ExtractionResult::Unsupported(format!(
            "Unsupported file format: {}. Cannot extract text from this file.",
            mime_type
        )),
    }
}

pub fn provider_supports_pdf(provider: &ProviderVariant) -> bool {
    matches!(
        provider,
        ProviderVariant::openai | ProviderVariant::anthropic | ProviderVariant::gemini
    )
}

fn extract_pdf(bytes: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
    // pdf-extract only supports file paths, so we need to use a temp file
    let temp_dir = std::env::temp_dir();
    let temp_path = temp_dir.join(format!("vizier_pdf_{}.pdf", nanoid::nanoid!(8)));
    std::fs::write(&temp_path, bytes)?;
    let text = pdf_extract::extract_text(&temp_path)?;
    std::fs::remove_file(&temp_path).ok();
    Ok(text)
}

fn extract_docx(bytes: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
    let doc = docx_rs::read_docx(bytes)?;
    let mut text = String::new();

    for child in doc.document.children {
        if let docx_rs::DocumentChild::Paragraph(paragraph) = child {
            for run in paragraph.children {
                if let docx_rs::ParagraphChild::Run(run) = run {
                    for child in run.children {
                        if let docx_rs::RunChild::Text(t) = child {
                            text.push_str(&t.text);
                        }
                    }
                }
            }
            text.push('\n');
        }
    }

    Ok(text)
}

fn extract_xlsx(bytes: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
    let mut workbook: calamine::Xlsx<_> = calamine::open_workbook_from_rs(std::io::Cursor::new(bytes))?;
    let mut text = String::new();

    for name in workbook.sheet_names().to_vec() {
        text.push_str(&format!("## Sheet: {}\n", name));
        let range = workbook.worksheet_range(&name)?;
        for row in range.rows() {
            let cells: Vec<String> = row
                .iter()
                .map(|cell| match cell {
                    calamine::Data::Empty => String::new(),
                    calamine::Data::String(s) => s.clone(),
                    calamine::Data::Float(f) => format!("{}", f),
                    calamine::Data::Int(i) => format!("{}", i),
                    calamine::Data::Bool(b) => format!("{}", b),
                    calamine::Data::Error(e) => format!("ERROR: {:?}", e),
                    calamine::Data::DateTime(dt) => format!("{}", dt),
                    calamine::Data::DateTimeIso(s) => s.clone(),
                    calamine::Data::DurationIso(s) => s.clone(),
                })
                .collect();
            text.push_str(&cells.join("\t"));
            text.push('\n');
        }
        text.push('\n');
    }

    Ok(text)
}

fn extract_pptx(bytes: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
    let mut text = String::new();

    let slide_names: Vec<String> = zip
        .file_names()
        .filter(|name| name.starts_with("ppt/slides/slide") && name.ends_with(".xml"))
        .map(|s| s.to_string())
        .collect();

    for name in slide_names {
        text.push_str(&format!("## Slide: {}\n", name));
        let mut file = zip.by_name(&name)?;
        let mut xml_content = String::new();
        file.read_to_string(&mut xml_content)?;

        for node in xml_content.split('<') {
            if let Some(content) = node.split('>').next() {
                if content.starts_with("a:t>") || content.starts_with("a:t ") {
                    if let Some(text_content) = content.strip_prefix("a:t>") {
                        let cleaned: String = text_content
                            .chars()
                            .filter(|c| !c.is_control())
                            .collect();
                        if !cleaned.trim().is_empty() {
                            text.push_str(&cleaned);
                            text.push(' ');
                        }
                    }
                }
            }
        }
        text.push('\n');
    }

    Ok(text)
}
