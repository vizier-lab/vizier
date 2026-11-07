//! Reading PDF documents as Markdown, rendering pages as PNG, and writing them from HTML or DOCX.
//!
//! Pure functions on bytes (Phase 2 Office addon, issue #100):
//! - `to_markdown`: structured markdown extraction with pagination, character budget guard, and password support via `pdf-agent` (`PdfReader`).
//! - `render_page`: rasterizes a single 1-indexed page to PNG image bytes via `pdf-agent` (`PdfRasterizer`).
//! - `from_html`: renders HTML/CSS into a valid PDF document via `pdf-agent` (`PdfGenerator`).
//! - `from_docx`: converts Word (.docx) to Markdown via `docx::to_markdown`, wraps with HTML/CSS, and generates PDF.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::VizierError;

/// A read returns at most this many characters when the caller names no budget.
pub const DEFAULT_MAX_CHARS: usize = 100_000;
/// Rendering scale bounds; 1.0 is 72 DPI.
pub const MIN_SCALE: f32 = 0.1;
pub const MAX_SCALE: f32 = 4.0;
pub const DEFAULT_SCALE: f32 = 1.5;

/// The scale a render will really use: the default when none (or a non-number) is given,
/// otherwise the request held to the supported range.
pub fn effective_scale(scale: Option<f32>) -> f32 {
    match scale {
        Some(s) if s.is_finite() => s.clamp(MIN_SCALE, MAX_SCALE),
        _ => DEFAULT_SCALE,
    }
}

/// Structured result of reading a PDF document as Markdown.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct PdfRead {
    /// Extracted markdown text (or preview if cut by budget/page limits).
    pub markdown: String,
    /// Total number of pages in the PDF document.
    pub total_pages: usize,
    /// 1-indexed start page of the extracted slice.
    pub page_start: usize,
    /// 1-indexed end page of the extracted slice.
    pub page_end: usize,
    /// Suggested 1-indexed start page for the next turn if more pages remain.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page_start: Option<usize>,
    /// True if the output was cut by character budget or page limit.
    pub truncated: bool,
    /// List of 1-indexed page numbers that failed to parse (empty if all succeeded).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub failed_pages: Vec<usize>,
    /// True if document appears to consist of scanned images without a selectable text layer.
    pub scanned_likely: bool,
    /// Explanatory note or instructions for the agent (e.g. truncation or scanned advice).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Extract structured Markdown from PDF bytes.
pub fn to_markdown(
    bytes: &[u8],
    page_start: Option<usize>,
    page_limit: Option<usize>,
    max_chars: Option<usize>,
    password: Option<&str>,
) -> Result<PdfRead, VizierError> {
    let mut opts = pdf_agent::PdfReadOptions::new();
    if let (Some(start), Some(limit)) = (page_start, page_limit) {
        opts = opts.with_pages(start, limit);
    } else if let Some(start) = page_start {
        opts = opts.with_pages(start, 10);
    } else if let Some(limit) = page_limit {
        opts = opts.with_pages(1, limit);
    }

    opts = opts.with_max_chars(max_chars.unwrap_or(DEFAULT_MAX_CHARS));

    if let Some(pwd) = password {
        opts = opts.with_password(pwd);
    }

    let res = pdf_agent::PdfReader::read(bytes, Some(opts)).map_err(|e| {
        // The library names its own option; the agent's argument is called `password`.
        let message = e.to_string().replace("`password` in PdfReadOptions", "the `password` argument");
        VizierError(format!("Failed to read PDF: {message}"))
    })?;

    Ok(PdfRead {
        markdown: res.markdown,
        total_pages: res.total_pages,
        page_start: res.page_start,
        page_end: res.page_end,
        next_page_start: res.next_page_start,
        truncated: res.truncated,
        failed_pages: res.failed_pages,
        scanned_likely: res.scanned_likely,
        // The library points at its own render function; the agent has a tool for it.
        note: res.note.map(|n| n.replace("`pdf_render`", "office_pdf_render_page_as_png")),
    })
}

/// Render a single 1-indexed page of a PDF document to PNG bytes at the specified scale.
/// Returns `(png_bytes, width, height)`.
pub fn render_page(
    bytes: &[u8],
    page_number: usize,
    scale: Option<f32>,
) -> Result<(Vec<u8>, u32, u32), VizierError> {
    let s = effective_scale(scale);
    let page_img = pdf_agent::PdfRasterizer::render_page(bytes, page_number, s)
        .map_err(|e| VizierError(format!("Failed to render PDF page {page_number}: {e}")))?;

    let width = page_img.width();
    let height = page_img.height();
    let png_bytes = page_img
        .to_png_bytes()
        .map_err(|e| VizierError(format!("Failed to encode rendered page as PNG: {e}")))?;

    Ok((png_bytes, width, height))
}

/// Render HTML content (with optional CSS and title) to PDF bytes.
pub fn from_html(html: &str, title: Option<&str>) -> Result<Vec<u8>, VizierError> {
    let processed_html = if let Some(t) = title {
        if !html.contains("<title>") {
            if let Some(pos) = html.find("<head>") {
                let (before, after) = html.split_at(pos + 6);
                format!("{}<title>{}</title>{}", before, html_escape(t), after)
            } else if let Some(pos) = html.find("<html>") {
                let (before, after) = html.split_at(pos + 6);
                format!("{}<head><title>{}</title></head>{}", before, html_escape(t), after)
            } else {
                format!("<title>{}</title>{}", html_escape(t), html)
            }
        } else {
            html.to_string()
        }
    } else {
        html.to_string()
    };

    pdf_agent::PdfGenerator::from_html(&processed_html)
        .map_err(|e| VizierError(format!("Failed to generate PDF from HTML: {e}")))
}

/// Convert Word (.docx) bytes to a PDF document, preserving headings, tables, lists, and formatting.
pub fn from_docx(docx_bytes: &[u8], title: Option<&str>) -> Result<Vec<u8>, VizierError> {
    let docx_read = super::docx::to_markdown(docx_bytes, usize::MAX)?;

    let mut parser_opts = pulldown_cmark::Options::empty();
    parser_opts.insert(pulldown_cmark::Options::ENABLE_TABLES);
    parser_opts.insert(pulldown_cmark::Options::ENABLE_STRIKETHROUGH);
    // Word text is text: a paragraph that reads `<b>x</b>` must print that, not become markup.
    let parser = pulldown_cmark::Parser::new_ext(&docx_read.markdown, parser_opts).map(|event| match event {
        pulldown_cmark::Event::Html(raw) | pulldown_cmark::Event::InlineHtml(raw) => {
            pulldown_cmark::Event::Text(raw)
        }
        other => other,
    });

    let mut body_html = String::new();
    pulldown_cmark::html::push_html(&mut body_html, parser);

    let title_tag = title
        .map(|t| format!("<title>{}</title>", html_escape(t)))
        .unwrap_or_default();
    let title_heading = title
        .map(|t| format!("<h1 class=\"doc-title\">{}</h1>", html_escape(t)))
        .unwrap_or_default();

    let full_html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
{title_tag}
<style>
body {{
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
    color: #111827;
    line-height: 1.6;
    padding: 32px;
    max-width: 800px;
    margin: 0 auto;
}}
.doc-title {{
    font-size: 2em;
    font-weight: bold;
    margin-bottom: 24px;
    border-bottom: 2px solid #e5e7eb;
    padding-bottom: 8px;
}}
h1, h2, h3, h4, h5, h6 {{
    color: #1f2937;
    margin-top: 1.5em;
    margin-bottom: 0.5em;
}}
table {{
    border-collapse: collapse;
    width: 100%;
    margin: 16px 0;
}}
th, td {{
    border: 1px solid #d1d5db;
    padding: 8px 12px;
    text-align: left;
}}
th {{
    background-color: #f3f4f6;
    font-weight: 600;
}}
tr:nth-child(even) {{
    background-color: #f9fafb;
}}
code {{
    background-color: #f3f4f6;
    padding: 2px 4px;
    border-radius: 4px;
    font-size: 0.9em;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
}}
pre {{
    background-color: #f3f4f6;
    padding: 12px;
    border-radius: 6px;
    overflow-x: auto;
}}
blockquote {{
    border-left: 4px solid #d1d5db;
    padding-left: 16px;
    color: #4b5563;
    margin: 16px 0;
}}
hr {{
    border: none;
    border-top: 1px solid #e5e7eb;
    margin: 24px 0;
}}
</style>
</head>
<body>
{title_heading}
{body_html}
</body>
</html>"#
    );

    from_html(&full_html, None)
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_to_pdf_and_read_roundtrip() {
        let html = "<h1>Weekly Report</h1><p>All tasks completed successfully.</p>";
        let pdf_bytes = from_html(html, Some("Weekly Report")).expect("should generate PDF");
        assert!(!pdf_bytes.is_empty());
        assert!(pdf_bytes.starts_with(b"%PDF"));

        let read = to_markdown(&pdf_bytes, None, None, None, None).expect("should read PDF");
        assert!(read.total_pages >= 1);
        assert!(read.markdown.contains("Weekly Report") || read.markdown.contains("tasks"));
    }

    #[test]
    fn html_escape_handles_special_characters() {
        assert_eq!(html_escape("a & b < c > d \" e ' f"), "a &amp; b &lt; c &gt; d &quot; e &#39; f");
    }

    #[test]
    fn render_page_valid_and_anomalies() {
        let html = "<h1>Page 1</h1><p>Hello Vizier!</p>";
        let pdf_bytes = from_html(html, None).expect("generate PDF");

        // 1. Valid render
        let (png, w, h) = render_page(&pdf_bytes, 1, Some(1.0)).expect("render page 1");
        assert!(!png.is_empty());
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"), "must be valid PNG header");
        assert!(w > 0 && h > 0);

        // 2. Anomaly: page 0 (1-indexed page underflow)
        let err_zero = render_page(&pdf_bytes, 0, None);
        assert!(err_zero.is_err(), "page 0 should fail");

        // 3. Anomaly: page 999 (out of bounds)
        let err_oob = render_page(&pdf_bytes, 999, None);
        assert!(err_oob.is_err(), "page 999 should fail");

        // 4. Anomaly: corrupted PDF bytes
        let err_corrupt = render_page(b"not a pdf file", 1, None);
        assert!(err_corrupt.is_err(), "corrupted PDF should fail to render");
    }

    #[test]
    fn read_markdown_anomalies() {
        let html = "<h1>Title</h1><p>A long paragraph with multiple sentences to test truncation.</p>";
        let pdf_bytes = from_html(html, None).expect("generate PDF");

        // 1. Anomaly: max_chars budget guard truncation
        let read_budget = to_markdown(&pdf_bytes, None, None, Some(10), None).expect("read budget");
        assert!(read_budget.truncated || read_budget.markdown.len() <= 30);

        // 2. Anomaly: page_start beyond total pages
        let read_oob = to_markdown(&pdf_bytes, Some(999), Some(5), None, None);
        // Does it return empty markdown or error?
        if let Ok(res) = read_oob {
            assert!(res.markdown.is_empty() || res.page_start > res.total_pages);
        }

        // 3. Anomaly: corrupted PDF bytes
        let err_corrupt = to_markdown(b"broken corrupted pdf stream", None, None, None, None);
        assert!(err_corrupt.is_err(), "corrupted PDF should return VizierError");
    }

    #[test]
    fn docx_to_pdf_valid_and_corrupt() {
        // 1. Anomaly: corrupt docx bytes
        let err_corrupt = from_docx(b"not a valid docx zip archive", Some("Docx Title"));
        assert!(err_corrupt.is_err(), "corrupt docx should fail");

        // 2. Valid docx fixture conversion if fixture is available
        let docx_bytes = super::super::docx::from_markdown("# Test Document\n\n- item 1\n- item 2", Some("Test"))
            .expect("create test docx");
        let pdf = from_docx(&docx_bytes, Some("Docx To Pdf")).expect("convert docx to pdf");
        assert!(!pdf.is_empty());
        assert!(pdf.starts_with(b"%PDF"));

        let read = to_markdown(&pdf, None, None, None, None).expect("read converted pdf");
        assert!(read.markdown.contains("Test Document") || read.markdown.contains("item 1"));
    }
}
