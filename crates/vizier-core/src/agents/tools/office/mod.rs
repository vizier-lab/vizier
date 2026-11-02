//! The Office addon's tools (spec `036-office-addon`): read Word and Excel files as Markdown,
//! typed cells or CSV. The format logic lives in the pure `docx` and `xlsx` modules; the tools
//! here only resolve a session file, call them and shape the result.

use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Number;

use crate::{
    agents::tools::{ToolCategory, ToolContext, VizierTool},
    error::VizierError,
    file_manager::FileManager,
    schema::VizierSession,
    storage::{VizierStorage, session_file::SessionFileStorage},
};

pub mod docx;
mod formula;
pub mod pdf;
pub mod xlsx;
#[cfg(test)]
mod fixture_tests;
#[cfg(test)]
mod pdf_tests;

pub use crate::agents::tools::session_files::{DOCX_MIME, PDF_MIME, PNG_MIME, XLS_MIME, XLSX_MIME};

/// Reads return at most this much text (about 200 KB), then say they were cut.
pub const READ_MAX_CHARS: usize = 200_000;
pub const MAX_SHEETS: usize = 20;
pub const MAX_ROWS: usize = 1_000;
pub const MAX_COLS: usize = 100;
/// Addresses of formula cells listed in a CSV read.
pub const MAX_FORMULA_CELLS: usize = 200;

/// One workbook cell in its JSON form: text `"…"`, number, boolean, date `{"date": ISO}`,
/// blank `null`, error `{"error": "#DIV/0!"}`, and, only when formulas were asked for,
/// `{"value": …, "formula": "=…"}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Cell {
    Null,
    Bool(bool),
    Number(Number),
    Text(String),
    Date { date: String },
    Error { error: String },
    Formula { value: Option<Box<Cell>>, formula: String },
}

impl Cell {
    /// A whole number stays an integer (`12`, not `12.0`).
    pub fn number(value: f64) -> Cell {
        if value.is_finite() && value.fract() == 0.0 && value.abs() < 1e15 {
            return Cell::Number(Number::from(value as i64));
        }
        Number::from_f64(value).map(Cell::Number).unwrap_or(Cell::Null)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Dimensions {
    pub rows: usize,
    pub cols: usize,
}

/// A Word document read as Markdown.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DocxRead {
    pub markdown: String,
    pub truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SheetRead {
    pub name: String,
    /// The area of the sheet that holds data, as an A1 range such as `C5:H400`; null for an
    /// empty sheet.
    pub used_range: Option<String>,
    /// The size of that area.
    pub dimensions: Dimensions,
    /// The A1 address of the first cell in `rows`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rows: Option<Vec<Vec<Cell>>>,
}

/// A workbook read as typed cells.
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct WorkbookRead {
    pub sheets: Vec<SheetRead>,
    pub truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SheetCsv {
    pub name: String,
    pub used_range: Option<String>,
    /// The A1 address of the first cell in `csv`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    /// The A1 addresses of the cells that hold formulas.
    pub formula_cells: Vec<String>,
    pub csv: String,
}

/// A workbook read as CSV text.
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct WorkbookCsv {
    pub sheets: Vec<SheetCsv>,
    pub truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// One sheet of a workbook to write.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct SheetInput {
    #[schemars(description = "Sheet name: 1 to 31 characters, none of []:*?/\\, unique in the workbook")]
    pub name: String,
    #[schemars(
        description = "Rows of cells. A cell is text, a number, true/false, {\"date\": \"2026-11-01\"} (or with a time, \"2026-11-01T09:30:00\") or null for blank. The first row is the header: it is written bold and frozen. Text is always stored as text, even when it starts with =."
    )]
    pub rows: Vec<Vec<Cell>>,
}

/// What kind of Office file a session file is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Docx,
    Xlsx,
    Xls,
    Pdf,
    Other,
}

fn format_of(mime: &str, filename: &str) -> Format {
    let base = mime.split(';').next().unwrap_or(mime).trim();
    match base {
        DOCX_MIME => return Format::Docx,
        XLSX_MIME => return Format::Xlsx,
        XLS_MIME => return Format::Xls,
        PDF_MIME => return Format::Pdf,
        _ => {}
    }
    match filename.rsplit('.').next().map(str::to_ascii_lowercase).as_deref() {
        Some("docx") => Format::Docx,
        Some("xlsx") => Format::Xlsx,
        Some("xls") => Format::Xls,
        Some("pdf") => Format::Pdf,
        _ => Format::Other,
    }
}

/// The tool an agent should use for a file it gave to the wrong one.
fn wrong_tool(filename: &str, format: Format, wanted: &str) -> VizierError {
    let hint = match format {
        Format::Docx => "it is a Word document: read it with office_docx_read_as_md".to_string(),
        Format::Xlsx | Format::Xls => {
            "it is an Excel workbook: read it with office_xlsx_read_as_cells (typed cells) or office_xlsx_read_as_csv (a preview of every sheet)".to_string()
        }
        Format::Pdf => {
            "it is a PDF document: read it with office_pdf_read_as_md, or render a page with office_pdf_render_page_as_png".to_string()
        }
        Format::Other => {
            "it is not a Word, Excel or PDF file: use read_document_file for text, or read_image_file for images".to_string()
        }
    };
    VizierError(format!("{wanted} cannot read '{filename}': {hint}."))
}

/// Runs CPU-bound work (rendering, layout, parsing a large PDF) off the async runtime.
async fn blocking<T, F>(work: F) -> Result<T, VizierError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, VizierError> + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|e| VizierError(format!("The PDF operation could not finish: {e}")))?
}

/// A session file's media type and bytes, or an error that names the files that do exist.
async fn load(
    storage: &Arc<VizierStorage>,
    file_manager: &FileManager,
    ctx: &ToolContext,
    filename: &str,
) -> Result<(String, Vec<u8>), VizierError> {
    let file = storage
        .get_session_file(&ctx.session, filename)
        .await
        .map_err(|e| VizierError(e.to_string()))?;
    let Some(file) = file else {
        let names: Vec<String> = storage
            .list_session_files(&ctx.session)
            .await
            .map(|files| files.into_iter().map(|f| f.filename).collect())
            .unwrap_or_default();
        let known = if names.is_empty() {
            "This conversation has no files.".to_string()
        } else {
            format!("Files in this conversation: {}.", names.join(", "))
        };
        return Err(VizierError(format!("File not found: {filename}. {known}")));
    };
    let (_, bytes) = file_manager
        .get(&file.file_id)
        .await
        .map_err(|e| VizierError(e.to_string()))?;
    Ok((file.mime_type, bytes))
}

fn flavour(format: Format) -> Option<xlsx::Flavour> {
    match format {
        Format::Xlsx => Some(xlsx::Flavour::Xlsx),
        Format::Xls => Some(xlsx::Flavour::Xls),
        _ => None,
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct DocxReadArgs {
    #[schemars(description = "Filename of a .docx file in this conversation's files")]
    pub filename: String,
    #[schemars(
        description = "Most characters to return (default and maximum 200000). The result says when it was cut."
    )]
    pub max_chars: Option<usize>,
}

pub struct OfficeDocxRead {
    pub storage: Arc<VizierStorage>,
    pub file_manager: FileManager,
}

#[async_trait::async_trait]
impl VizierTool for OfficeDocxRead {
    type Input = DocxReadArgs;
    type Output = DocxRead;

    /// Listed under the `office` addon's category, not a built-in one.
    fn category() -> Option<ToolCategory> {
        None
    }

    fn name() -> String {
        "office_docx_read_as_md".to_string()
    }

    fn description(&self) -> String {
        "Read a Word document (.docx) as Markdown: headings, bold and italic, nested lists, tables as pipe tables and links. Images, shapes, text boxes and footnotes are skipped. Long documents are cut at max_chars (at most 200000) and the result says so."
            .to_string()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let (mime, bytes) = load(&self.storage, &self.file_manager, ctx, &args.filename).await?;
        let format = format_of(&mime, &args.filename);
        if format != Format::Docx {
            return Err(wrong_tool(&args.filename, format, Self::name().as_str()));
        }
        let max = args.max_chars.unwrap_or(READ_MAX_CHARS).clamp(1, READ_MAX_CHARS);
        docx::to_markdown(&bytes, max)
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct XlsxReadArgs {
    #[schemars(description = "Filename of a .xlsx or .xls file in this conversation's files")]
    pub filename: String,
    #[schemars(description = "Read only this sheet, by name. Default: every sheet.")]
    pub sheet: Option<String>,
    #[schemars(
        description = "Read only these cells, in A1 notation: a block such as \"B2:F40\" or one cell such as \"C7\". Default: the whole sheet."
    )]
    pub range: Option<String>,
    #[schemars(
        description = "true: return only each sheet's name, data area (used_range) and size (dimensions), without any cell. Use it first on a large workbook, then read a range. Default false."
    )]
    pub info: Option<bool>,
    #[schemars(
        description = "true: show each formula cell as {\"value\": the saved result, \"formula\": \"=...\"}. Default false: formula cells show only their saved result. Formulas are never calculated by this tool."
    )]
    pub formulas: Option<bool>,
}

pub struct OfficeXlsxRead {
    pub storage: Arc<VizierStorage>,
    pub file_manager: FileManager,
}

#[async_trait::async_trait]
impl VizierTool for OfficeXlsxRead {
    type Input = XlsxReadArgs;
    type Output = WorkbookRead;

    fn category() -> Option<ToolCategory> {
        None
    }

    fn name() -> String {
        "office_xlsx_read_as_cells".to_string()
    }

    fn description(&self) -> String {
        "Read an Excel workbook (.xlsx or legacy .xls) as sheets of typed cells: text, number, boolean, date ({\"date\": ISO-8601}), blank (null) and error ({\"error\": \"#DIV/0!\"}). Each sheet reports used_range (where its data is, such as C5:H400), dimensions and start (the A1 address of the first returned cell, so you know which cell each value came from). Read one cell or a block with `range`, one sheet with `sheet`, or only the sheet sizes with `info`. Formula cells return their last saved result; add formulas=true for the formula text too. This tool never calculates formulas. For a quick look at a whole workbook as text, use office_xlsx_read_as_csv. Output is cut at 20 sheets, 1000 rows, 100 columns or about 200 KB, and the result says so."
            .to_string()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let (mime, bytes) = load(&self.storage, &self.file_manager, ctx, &args.filename).await?;
        let format = format_of(&mime, &args.filename);
        let Some(flavour) = flavour(format) else {
            return Err(wrong_tool(&args.filename, format, Self::name().as_str()));
        };
        xlsx::read_workbook(
            &bytes,
            flavour,
            args.sheet.as_deref(),
            args.range.as_deref(),
            args.info.unwrap_or(false),
            args.formulas.unwrap_or(false),
        )
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct XlsxCsvArgs {
    #[schemars(description = "Filename of a .xlsx or .xls file in this conversation's files")]
    pub filename: String,
    #[schemars(description = "Read only this sheet, by name. Default: every sheet.")]
    pub sheet: Option<String>,
    #[schemars(
        description = "Read only these cells, in A1 notation: a block such as \"B2:F40\" or one cell such as \"C7\". Default: the whole sheet."
    )]
    pub range: Option<String>,
    #[schemars(
        description = "false (default): a formula cell is written as its formula text, for example =SUM(A1:A3). true: a formula cell is written as its calculated result; a cell that cannot be calculated shows its saved value and is named in note."
    )]
    pub executed_formula: Option<bool>,
}

pub struct OfficeXlsxReadCsv {
    pub storage: Arc<VizierStorage>,
    pub file_manager: FileManager,
}

#[async_trait::async_trait]
impl VizierTool for OfficeXlsxReadCsv {
    type Input = XlsxCsvArgs;
    type Output = WorkbookCsv;

    fn category() -> Option<ToolCategory> {
        None
    }

    fn name() -> String {
        "office_xlsx_read_as_csv".to_string()
    }

    fn description(&self) -> String {
        "Preview an Excel workbook (.xlsx or legacy .xls) as CSV text, every sheet in one call, so you can see the whole picture. Each sheet returns its name, used_range, start (the A1 address of the first CSV cell), formula_cells (the addresses of cells that hold formulas) and the csv. With executed_formula=false (default) formula cells show their formula text; with true they show calculated results. The calculation is done here, not by Excel, so it can differ from what Excel shows (and NOW or RAND change on every read). A cell that cannot be calculated (an unsupported function, a circular reference, a link to another file), and every cell of a legacy .xls, shows the value the file saved, and the result names those cells in note. CSV loses cell types (a date or a number looks like text); use office_xlsx_read_as_cells when types matter. Output is cut at 20 sheets, 1000 rows, 100 columns or about 200 KB, and the result says so."
            .to_string()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let (mime, bytes) = load(&self.storage, &self.file_manager, ctx, &args.filename).await?;
        let format = format_of(&mime, &args.filename);
        let Some(flavour) = flavour(format) else {
            return Err(wrong_tool(&args.filename, format, Self::name().as_str()));
        };
        let executed_formula = args.executed_formula.unwrap_or(false);
        if executed_formula {
            // Calculating waits on a worker thread for up to its time limit; keep that off the
            // async runtime. A calculation that fails is data in the result, not an error.
            let (sheet, range) = (args.sheet, args.range);
            return tokio::task::spawn_blocking(move || {
                xlsx::read_csv(&bytes, flavour, sheet.as_deref(), range.as_deref(), true)
            })
            .await
            .map_err(|e| VizierError(format!("The calculation could not finish: {e}")))?;
        }
        xlsx::read_csv(&bytes, flavour, args.sheet.as_deref(), args.range.as_deref(), false)
    }
}

/// A file the tool stored in the conversation.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct FileWritten {
    pub status: String,
    pub filename: String,
    pub size: u64,
}

/// A name for a file the agent asked for: the basename only, ending in `extension`.
fn normalise_filename(raw: &str, extension: &str) -> Result<String, VizierError> {
    let name = raw.trim();
    if name.is_empty() {
        return Err(VizierError("The filename is empty.".into()));
    }
    if name.chars().any(char::is_control) {
        return Err(VizierError("The filename must not contain control characters.".into()));
    }
    if name.contains('/') || name.contains('\\') {
        return Err(VizierError(
            "The filename must be a name only, without a folder: files are kept per conversation, not in folders.".into(),
        ));
    }
    if name == "." || name == ".." {
        return Err(VizierError(format!("'{name}' is not a usable filename.")));
    }
    let suffix = format!(".{extension}");
    if name.to_ascii_lowercase().ends_with(&suffix) && name.len() > suffix.len() {
        Ok(name.to_string())
    } else {
        Ok(format!("{name}{suffix}"))
    }
}

/// Stores generated bytes as a conversation file. A file of the same name is replaced, like
/// `image_generate` does (FR-027).
async fn write_session_file(
    storage: &Arc<VizierStorage>,
    file_manager: &FileManager,
    session: &VizierSession,
    filename: &str,
    extension: &str,
    mime: &str,
    bytes: Vec<u8>,
) -> Result<FileWritten, VizierError> {
    let filename = normalise_filename(filename, extension)?;
    let record = file_manager
        .upload(&filename, bytes)
        .await
        .map_err(|e| VizierError(e.to_string()))?;
    storage
        .save_session_file(session, &filename, mime, record.size, &record.id)
        .await
        .map_err(|e| VizierError(e.to_string()))?;
    Ok(FileWritten { status: "ok".into(), filename, size: record.size })
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct DocxWriteArgs {
    #[schemars(description = "Name for the new file, such as report.docx (.docx is added if missing). A name only, no folder.")]
    pub filename: String,
    #[schemars(
        description = "The document as Markdown: # headings, **bold**, *italic*, `code`, nested - and 1. lists, | pipe | tables, [links](url), > quotes and --- rules. Images and raw HTML become their text."
    )]
    pub markdown: String,
    #[schemars(description = "A title shown at the top of the document, in the Title style")]
    pub title: Option<String>,
}

pub struct OfficeDocxWrite {
    pub storage: Arc<VizierStorage>,
    pub file_manager: FileManager,
}

#[async_trait::async_trait]
impl VizierTool for OfficeDocxWrite {
    type Input = DocxWriteArgs;
    type Output = FileWritten;

    fn category() -> Option<ToolCategory> {
        None
    }

    fn name() -> String {
        "office_docx_write_from_md".to_string()
    }

    fn description(&self) -> String {
        "Create a new Word document (.docx) from Markdown and save it as a file in this conversation. It always generates a new file and never edits an existing one: editing in place would silently lose the original's styles, headers, images and tracked changes. If a file with the same name already exists in this conversation it is replaced, so to make a version of an existing file, use a new filename and the original is kept. To deliver the result, call send_attachment with the returned filename. For a long document, compose the text first with the scratchpad tools (write_tmp, append_tmp), then pass it here."
            .to_string()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let bytes = docx::from_markdown(&args.markdown, args.title.as_deref())?;
        write_session_file(&self.storage, &self.file_manager, &ctx.session, &args.filename, "docx", DOCX_MIME, bytes).await
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct XlsxWriteArgs {
    #[schemars(description = "Name for the new file, such as budget.xlsx (.xlsx is added if missing). A name only, no folder.")]
    pub filename: String,
    #[schemars(description = "The sheets, in order: each has a name and rows of cells")]
    pub sheets: Vec<SheetInput>,
}

pub struct OfficeXlsxWrite {
    pub storage: Arc<VizierStorage>,
    pub file_manager: FileManager,
}

#[async_trait::async_trait]
impl VizierTool for OfficeXlsxWrite {
    type Input = XlsxWriteArgs;
    type Output = FileWritten;

    fn category() -> Option<ToolCategory> {
        None
    }

    fn name() -> String {
        "office_xlsx_write_from_sheets".to_string()
    }

    fn description(&self) -> String {
        "Create a new Excel workbook (.xlsx) from sheets of rows and save it as a file in this conversation. It always generates a new file and never edits an existing one: editing in place would silently lose the original's formulas, formatting, charts and macros. If a file with the same name already exists in this conversation it is replaced, so to make a version of an existing file, use a new filename and the original is kept. Cells are text, numbers, booleans, {\"date\": \"YYYY-MM-DD\"} or null. The first row of each sheet is written bold and frozen as the header. Text is always stored as text and formulas are not written: a value like \"=SUM(A1:A9)\" stays that literal text. To deliver the result, call send_attachment with the returned filename. For a large table, compose the data first with the scratchpad tools."
            .to_string()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let bytes = xlsx::write_workbook(&args.sheets)?;
        write_session_file(&self.storage, &self.file_manager, &ctx.session, &args.filename, "xlsx", XLSX_MIME, bytes).await
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct PdfReadArgs {
    #[schemars(description = "Filename of a .pdf file in this conversation's files")]
    pub filename: String,
    #[schemars(description = "1-indexed page number to start reading from. Default: 1")]
    pub page_start: Option<usize>,
    #[schemars(description = "Number of pages to read from page_start. Default: 10")]
    pub page_limit: Option<usize>,
    #[schemars(description = "Maximum character budget (default 100000, max 200000). The result says when it was cut.")]
    pub max_chars: Option<usize>,
    #[schemars(description = "Optional password if the PDF file is encrypted/password-protected")]
    pub password: Option<String>,
}

pub struct OfficePdfRead {
    pub storage: Arc<VizierStorage>,
    pub file_manager: FileManager,
}

#[async_trait::async_trait]
impl VizierTool for OfficePdfRead {
    type Input = PdfReadArgs;
    type Output = pdf::PdfRead;

    fn category() -> Option<ToolCategory> {
        None
    }

    fn name() -> String {
        "office_pdf_read_as_md".to_string()
    }

    fn description(&self) -> String {
        "Read a PDF document (.pdf) as Markdown: headings, paragraphs, lists and tables. It reads page_limit pages (default 10) from page_start (default 1) and returns total_pages, page_start, page_end and, when pages remain, next_page_start: call again with that page to continue. Output is also cut at max_chars (default 100000, at most 200000); the result says truncated when it was cut. Images and drawings are skipped. A document with no selectable text sets scanned_likely: render its pages with office_pdf_render_page_as_png and look at them with read_image_file. Pages that could not be parsed are listed in failed_pages while the others are returned. For an encrypted file pass password. This only reads: no tool here edits, merges or fills in an existing PDF."
            .to_string()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let (mime, bytes) = load(&self.storage, &self.file_manager, ctx, &args.filename).await?;
        let format = format_of(&mime, &args.filename);
        if format != Format::Pdf {
            return Err(wrong_tool(&args.filename, format, Self::name().as_str()));
        }
        let max_chars = args.max_chars.map(|c| c.clamp(1, READ_MAX_CHARS));
        blocking(move || {
            pdf::to_markdown(&bytes, args.page_start, args.page_limit, max_chars, args.password.as_deref())
        })
        .await
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct PdfRenderPageArgs {
    #[schemars(description = "Filename of a .pdf file in this conversation's files")]
    pub filename: String,
    #[schemars(description = "1-indexed page number to render as PNG image. Default: 1")]
    pub page: Option<usize>,
    #[schemars(description = "Rendering scale factor (0.1 to 4.0). Default: 1.5 (~108 DPI). 1.0 is 72 DPI, 2.0 is 144 DPI.")]
    pub scale: Option<f32>,
    #[schemars(description = "Optional name for the output PNG file. If omitted, defaults to <filename>_page_<page>.png")]
    pub output_filename: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PdfPageRendered {
    pub status: String,
    pub filename: String,
    pub size: u64,
    pub width: u32,
    pub height: u32,
    pub page: usize,
    /// The scale the page was rendered at, after it was held to the supported range.
    pub scale: f32,
}

pub struct OfficePdfRenderPage {
    pub storage: Arc<VizierStorage>,
    pub file_manager: FileManager,
}

#[async_trait::async_trait]
impl VizierTool for OfficePdfRenderPage {
    type Input = PdfRenderPageArgs;
    type Output = PdfPageRendered;

    fn category() -> Option<ToolCategory> {
        None
    }

    fn name() -> String {
        "office_pdf_render_page_as_png".to_string()
    }

    fn description(&self) -> String {
        "Render one page of a PDF (.pdf) to a PNG image saved as a file in this conversation, then look at it with read_image_file. Use it for scanned pages, charts, diagrams and layouts that text cannot show. page counts from 1 (default 1). scale is 0.1 to 4.0 (default 1.5; 1.0 is 72 DPI): a value outside that range is held to it, and the result returns the scale used with the image width and height in pixels. A page too large to render at that scale is an error: try a smaller scale. The file is named <pdf name>_page_<n>.png unless output_filename is given; a file of the same name is replaced."
            .to_string()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let (mime, bytes) = load(&self.storage, &self.file_manager, ctx, &args.filename).await?;
        let format = format_of(&mime, &args.filename);
        if format != Format::Pdf {
            return Err(wrong_tool(&args.filename, format, Self::name().as_str()));
        }
        let page = args.page.unwrap_or(1);
        let scale = pdf::effective_scale(args.scale);
        let (png_bytes, width, height) = blocking(move || pdf::render_page(&bytes, page, Some(scale))).await?;

        let default_name = {
            let base = args
                .filename
                .strip_suffix(".pdf")
                .or_else(|| args.filename.strip_suffix(".PDF"))
                .unwrap_or(&args.filename);
            format!("{base}_page_{page}.png")
        };
        let out_name = args.output_filename.as_deref().unwrap_or(&default_name);

        let written = write_session_file(
            &self.storage,
            &self.file_manager,
            &ctx.session,
            out_name,
            "png",
            PNG_MIME,
            png_bytes,
        )
        .await?;

        Ok(PdfPageRendered {
            status: "ok".into(),
            filename: written.filename,
            size: written.size,
            width,
            height,
            page,
            scale,
        })
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct PdfWriteHtmlArgs {
    #[schemars(description = "Name for the new PDF file, such as document.pdf (.pdf is added if missing). A name only, no folder.")]
    pub filename: String,
    #[schemars(description = "The document as HTML/CSS content to render into PDF.")]
    pub html: String,
    #[schemars(description = "Optional title for the document")]
    pub title: Option<String>,
}

pub struct OfficePdfWriteFromHtml {
    pub storage: Arc<VizierStorage>,
    pub file_manager: FileManager,
}

#[async_trait::async_trait]
impl VizierTool for OfficePdfWriteFromHtml {
    type Input = PdfWriteHtmlArgs;
    type Output = FileWritten;

    fn category() -> Option<ToolCategory> {
        None
    }

    fn name() -> String {
        "office_pdf_write_from_html".to_string()
    }

    fn description(&self) -> String {
        "Create a new PDF file from HTML and CSS and save it as a file in this conversation. Put the styling in a <style> element: layout, typography, tables, borders and colours are supported (HTML up to 5 MB). Nothing outside the HTML is loaded: images and stylesheets by URL or from other files in the conversation are not available, and scripts do not run, so describe visuals with CSS and text. It always generates a new file and never edits an existing PDF. If a file with the same name already exists in this conversation it is replaced, so to make a version of an existing file use a new filename and the original is kept. .pdf is added to the name if missing. To deliver the result, call send_attachment with the returned filename."
            .to_string()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let (html, title) = (args.html, args.title);
        let pdf_bytes = blocking(move || pdf::from_html(&html, title.as_deref())).await?;
        write_session_file(
            &self.storage,
            &self.file_manager,
            &ctx.session,
            &args.filename,
            "pdf",
            PDF_MIME,
            pdf_bytes,
        )
        .await
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct PdfWriteDocxArgs {
    #[schemars(description = "Name for the new PDF file, such as report.pdf (.pdf is added if missing). A name only, no folder.")]
    pub filename: String,
    #[schemars(description = "Filename of an existing Word document (.docx) in this conversation's files to convert to PDF")]
    pub docx_filename: String,
    #[schemars(description = "Optional title for the document")]
    pub title: Option<String>,
}

pub struct OfficePdfWriteFromDocx {
    pub storage: Arc<VizierStorage>,
    pub file_manager: FileManager,
}

#[async_trait::async_trait]
impl VizierTool for OfficePdfWriteFromDocx {
    type Input = PdfWriteDocxArgs;
    type Output = FileWritten;

    fn category() -> Option<ToolCategory> {
        None
    }

    fn name() -> String {
        "office_pdf_write_from_docx".to_string()
    }

    fn description(&self) -> String {
        "Convert a Word document (.docx) in this conversation to a new PDF file. Headings, bold and italic, lists, tables and links are carried over from the document's text; images, headers and footers, page numbers and tracked changes are not, and the layout is a clean document layout, not Word's own. The Word file is left unchanged. title is optional and is also shown as the first heading. It always generates a new file and never edits an existing PDF. If a file with the same name already exists in this conversation it is replaced, so use a new filename to keep the original. .pdf is added to the name if missing. To deliver the result, call send_attachment with the returned filename."
            .to_string()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let (mime, docx_bytes) = load(&self.storage, &self.file_manager, ctx, &args.docx_filename).await?;
        let format = format_of(&mime, &args.docx_filename);
        if format != Format::Docx {
            return Err(wrong_tool(&args.docx_filename, format, Self::name().as_str()));
        }
        let title = args.title;
        let pdf_bytes = blocking(move || pdf::from_docx(&docx_bytes, title.as_deref())).await?;
        write_session_file(
            &self.storage,
            &self.file_manager,
            &ctx.session,
            &args.filename,
            "pdf",
            PDF_MIME,
            pdf_bytes,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_cell_round_trips_through_json_in_every_form() {
        let cells = [
            (Cell::Null, json!(null)),
            (Cell::Bool(true), json!(true)),
            (Cell::number(12.0), json!(12)),
            (Cell::number(2.5), json!(2.5)),
            (Cell::Text("a".into()), json!("a")),
            (Cell::Date { date: "2026-11-01".into() }, json!({"date": "2026-11-01"})),
            (Cell::Error { error: "#DIV/0!".into() }, json!({"error": "#DIV/0!"})),
            (
                Cell::Formula { value: Some(Box::new(Cell::number(3.0))), formula: "=1+2".into() },
                json!({"value": 3, "formula": "=1+2"}),
            ),
            (
                Cell::Formula { value: None, formula: "=A1".into() },
                json!({"value": null, "formula": "=A1"}),
            ),
        ];
        for (cell, expected) in cells {
            let value = serde_json::to_value(&cell).unwrap();
            assert_eq!(value, expected);
            assert_eq!(serde_json::from_value::<Cell>(value).unwrap(), cell);
        }
    }

    #[test]
    fn the_format_comes_from_the_media_type_then_the_extension() {
        assert_eq!(format_of(DOCX_MIME, "x"), Format::Docx);
        assert_eq!(format_of(XLSX_MIME, "x"), Format::Xlsx);
        assert_eq!(format_of(XLS_MIME, "x"), Format::Xls);
        assert_eq!(format_of("application/octet-stream", "Budget.XLSX"), Format::Xlsx);
        assert_eq!(format_of(PDF_MIME, "x"), Format::Pdf);
        assert_eq!(format_of("application/octet-stream", "report.PDF"), Format::Pdf);
        assert_eq!(format_of("text/plain", "a.txt"), Format::Other);
    }

    #[test]
    fn a_wrong_file_type_names_the_tool_to_use() {
        let docx = wrong_tool("a.docx", Format::Docx, "office_xlsx_read_as_cells");
        assert!(docx.0.contains("office_docx_read_as_md"));
        let xlsx = wrong_tool("a.xlsx", Format::Xlsx, "office_docx_read_as_md");
        assert!(xlsx.0.contains("office_xlsx_read_as_cells") && xlsx.0.contains("office_xlsx_read_as_csv"));
        let pdf = wrong_tool("a.pdf", Format::Pdf, "office_docx_read_as_md");
        assert!(pdf.0.contains("office_pdf_read_as_md") && pdf.0.contains("office_pdf_render_page_as_png"));
        let other = wrong_tool("a.txt", Format::Other, "office_docx_read_as_md");
        assert!(other.0.contains("read_document_file"));
    }

    #[test]
    fn a_filename_is_a_name_ending_in_the_extension() {
        assert_eq!(normalise_filename("report", "docx").unwrap(), "report.docx");
        assert_eq!(normalise_filename("  report.docx ", "docx").unwrap(), "report.docx");
        assert_eq!(normalise_filename("Report.DOCX", "docx").unwrap(), "Report.DOCX");
        assert_eq!(normalise_filename("my.budget", "xlsx").unwrap(), "my.budget.xlsx");
        assert_eq!(normalise_filename(".xlsx", "xlsx").unwrap(), ".xlsx.xlsx");
        for bad in ["", "  ", "a/b.docx", "..\\x", "../x", "a\nb", "a\0b", ".", ".."] {
            assert!(normalise_filename(bad, "docx").is_err(), "{bad:?}");
        }
    }

    fn files() -> (Arc<VizierStorage>, FileManager, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE session_file (id TEXT PRIMARY KEY, session_slug TEXT NOT NULL, agent_id TEXT NOT NULL, data TEXT NOT NULL);",
        )
        .unwrap();
        let storage = VizierStorage::new(crate::storage::sqlite::SqliteStorage::new(
            Arc::new(parking_lot::Mutex::new(conn)),
            Arc::new(crate::storage::document::LocalDocumentStore::new(dir.path().to_path_buf())),
        ));
        let manager = FileManager::new(dir.path().to_string_lossy().to_string());
        (Arc::new(storage), manager, dir)
    }

    #[tokio::test]
    async fn a_written_file_is_stored_with_its_media_type_and_replaced_by_the_same_name() {
        let (storage, manager, _dir) = files();
        let session = VizierSession("agent".into(), crate::schema::VizierChannelId::HTTP("u".into(), "c".into()), None);
        let first = write_session_file(&storage, &manager, &session, "report", "docx", DOCX_MIME, b"one".to_vec())
            .await
            .unwrap();
        assert_eq!((first.status.as_str(), first.filename.as_str(), first.size), ("ok", "report.docx", 3));
        let second = write_session_file(&storage, &manager, &session, "report.docx", "docx", DOCX_MIME, b"second".to_vec())
            .await
            .unwrap();
        assert_eq!(second.size, 6);

        let listed = storage.list_session_files(&session).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].mime_type, DOCX_MIME);
        let (_, bytes) = manager.get(&listed[0].file_id).await.unwrap();
        assert_eq!(bytes, b"second");
    }

    #[tokio::test]
    async fn all_office_tools_user_journey_and_anomalies() {
        let (storage, manager, _dir) = files();
        let session = VizierSession("agent-1".into(), crate::schema::VizierChannelId::HTTP("u".into(), "c".into()), None);
        let ctx = ToolContext {
            session: session.clone(),
            pending_attachments: Default::default(),
            hooks: None,
            background_depth: 0,
            deadline: None,
            loaded_images: Default::default(),
            recalled: Default::default(),
            client: None,
            questions: crate::questions::QuestionsHandle::new(Default::default()),
            interactive: false,
            dream_sources: vec![],
            secret_redactor: Arc::new(crate::secrets::SecretRedactor::default()),
            project: None,
        };

        // Instantiate tools
        let docx_write = OfficeDocxWrite { storage: storage.clone(), file_manager: manager.clone() };
        let docx_read = OfficeDocxRead { storage: storage.clone(), file_manager: manager.clone() };
        let xlsx_write = OfficeXlsxWrite { storage: storage.clone(), file_manager: manager.clone() };
        let xlsx_read = OfficeXlsxRead { storage: storage.clone(), file_manager: manager.clone() };
        let xlsx_read_csv = OfficeXlsxReadCsv { storage: storage.clone(), file_manager: manager.clone() };
        let pdf_write_html = OfficePdfWriteFromHtml { storage: storage.clone(), file_manager: manager.clone() };
        let pdf_write_docx = OfficePdfWriteFromDocx { storage: storage.clone(), file_manager: manager.clone() };
        let pdf_read = OfficePdfRead { storage: storage.clone(), file_manager: manager.clone() };
        let pdf_render = OfficePdfRenderPage { storage: storage.clone(), file_manager: manager.clone() };

        // === 1. DOCX: Write & Read ===
        let docx_out = docx_write.call(DocxWriteArgs {
            filename: "spec.docx".into(),
            markdown: "# Office Specification\n\n**Important** note.\n\n| Feature | Status |\n|---|---|\n| DOCX | OK |\n| PDF | Phase 2 |".into(),
            title: Some("Office Addon Spec".into()),
        }, &ctx).await.expect("write docx");
        assert_eq!(docx_out.filename, "spec.docx");
        assert!(docx_out.size > 0);

        let docx_content = docx_read.call(DocxReadArgs {
            filename: "spec.docx".into(),
            max_chars: None,
        }, &ctx).await.expect("read docx");
        assert!(docx_content.markdown.contains("Office Addon Spec") || docx_content.markdown.contains("Specification"));

        // === 2. PDF: Write From DOCX & Read ===
        let pdf_from_docx_out = pdf_write_docx.call(PdfWriteDocxArgs {
            filename: "from_word.pdf".into(),
            docx_filename: "spec.docx".into(),
            title: Some("Converted PDF".into()),
        }, &ctx).await.expect("docx to pdf");
        assert_eq!(pdf_from_docx_out.filename, "from_word.pdf");

        let pdf_from_docx_read = pdf_read.call(PdfReadArgs {
            filename: "from_word.pdf".into(),
            page_start: Some(1),
            page_limit: Some(10),
            max_chars: None,
            password: None,
        }, &ctx).await.expect("read pdf from docx");
        assert!(pdf_from_docx_read.total_pages >= 1);
        assert!(!pdf_from_docx_read.markdown.is_empty());

        // === 3. PDF: Write From HTML, Render Page as PNG & Read ===
        let html_content = r#"
            <!DOCTYPE html>
            <html>
            <head><style>h1 { color: #0284c7; } table { border-collapse: collapse; } td, th { border: 1px solid #ccc; padding: 4px; }</style></head>
            <body>
                <h1>Invoice #1001</h1>
                <p>Status: <strong>Paid</strong></p>
                <table>
                    <tr><th>Item</th><th>Price</th></tr>
                    <tr><td>Consulting</td><td>$150.00</td></tr>
                </table>
            </body>
            </html>
        "#;
        let pdf_html_out = pdf_write_html.call(PdfWriteHtmlArgs {
            filename: "invoice.pdf".into(),
            html: html_content.into(),
            title: Some("Invoice #1001".into()),
        }, &ctx).await.expect("write pdf from html");
        assert_eq!(pdf_html_out.filename, "invoice.pdf");

        // Render page 1 of invoice.pdf to PNG
        let rendered_page = pdf_render.call(PdfRenderPageArgs {
            filename: "invoice.pdf".into(),
            page: Some(1),
            scale: Some(1.5),
            output_filename: Some("invoice_preview.png".into()),
        }, &ctx).await.expect("render pdf page to png");
        assert_eq!(rendered_page.filename, "invoice_preview.png");
        assert!(rendered_page.width > 0 && rendered_page.height > 0);
        assert!(rendered_page.size > 0);

        // Read invoice.pdf as markdown
        let read_invoice = pdf_read.call(PdfReadArgs {
            filename: "invoice.pdf".into(),
            page_start: None,
            page_limit: None,
            max_chars: None,
            password: None,
        }, &ctx).await.expect("read invoice pdf");
        assert!(read_invoice.markdown.contains("Invoice") || read_invoice.markdown.contains("1001") || read_invoice.markdown.contains("Consulting"));

        // === 4. XLSX: Write & Read (Cells + CSV) ===
        let xlsx_out = xlsx_write.call(XlsxWriteArgs {
            filename: "finance.xlsx".into(),
            sheets: vec![
                SheetInput {
                    name: "Revenue".into(),
                    rows: vec![
                        vec![Cell::Text("Quarter".into()), Cell::Text("Amount".into())],
                        vec![Cell::Text("Q1".into()), Cell::number(2500.0)],
                        vec![Cell::Text("Q2".into()), Cell::number(3500.0)],
                    ],
                },
            ],
        }, &ctx).await.expect("write xlsx");
        assert_eq!(xlsx_out.filename, "finance.xlsx");

        let cells_read = xlsx_read.call(XlsxReadArgs {
            filename: "finance.xlsx".into(),
            sheet: Some("Revenue".into()),
            range: None,
            info: Some(false),
            formulas: Some(false),
        }, &ctx).await.expect("read xlsx cells");
        assert_eq!(cells_read.sheets.len(), 1);
        assert_eq!(cells_read.sheets[0].name, "Revenue");

        let csv_read = xlsx_read_csv.call(XlsxCsvArgs {
            filename: "finance.xlsx".into(),
            sheet: None,
            range: None,
            executed_formula: Some(false),
        }, &ctx).await.expect("read xlsx csv");
        assert!(csv_read.sheets[0].csv.contains("Quarter,Amount"));

        // === 5. ANOMALY CASES ===

        // Anomaly A: Tool mismatch - Passing .docx file to office_pdf_read_as_md
        let err_wrong_tool_pdf = pdf_read.call(PdfReadArgs {
            filename: "spec.docx".into(),
            page_start: None,
            page_limit: None,
            max_chars: None,
            password: None,
        }, &ctx).await.unwrap_err();
        assert!(err_wrong_tool_pdf.0.contains("it is a Word document: read it with office_docx_read_as_md"), "Error message should guide user to correct tool: {}", err_wrong_tool_pdf.0);

        // Anomaly B: Tool mismatch - Passing .pdf file to office_docx_read_as_md
        let err_wrong_tool_docx = docx_read.call(DocxReadArgs {
            filename: "invoice.pdf".into(),
            max_chars: None,
        }, &ctx).await.unwrap_err();
        assert!(err_wrong_tool_docx.0.contains("it is a PDF document: read it with office_pdf_read_as_md"), "Error message should guide user: {}", err_wrong_tool_docx.0);

        // Anomaly C: Tool mismatch - Passing .xlsx to office_pdf_render_page_as_png
        let err_wrong_tool_render = pdf_render.call(PdfRenderPageArgs {
            filename: "finance.xlsx".into(),
            page: Some(1),
            scale: None,
            output_filename: None,
        }, &ctx).await.unwrap_err();
        assert!(err_wrong_tool_render.0.contains("it is an Excel workbook"), "Error message: {}", err_wrong_tool_render.0);

        // Anomaly D: File Not Found - Lists files in conversation
        let err_fnf = pdf_read.call(PdfReadArgs {
            filename: "ghost_file.pdf".into(),
            page_start: None,
            page_limit: None,
            max_chars: None,
            password: None,
        }, &ctx).await.unwrap_err();
        assert!(err_fnf.0.contains("File not found: ghost_file.pdf"), "Error should clearly state missing file");
        assert!(err_fnf.0.contains("Files in this conversation:"), "Error should list available files to help agent/user");

        // Anomaly E: PDF Render out-of-bounds page
        let err_page_oob = pdf_render.call(PdfRenderPageArgs {
            filename: "invoice.pdf".into(),
            page: Some(999),
            scale: None,
            output_filename: None,
        }, &ctx).await.unwrap_err();
        assert!(err_page_oob.0.contains("Failed to render PDF page 999"), "Should fail gracefully on OOB page");

        // Anomaly F: Path traversal filename rejected
        let err_traversal = pdf_write_html.call(PdfWriteHtmlArgs {
            filename: "../hack.pdf".into(),
            html: "<h1>Hacked</h1>".into(),
            title: None,
        }, &ctx).await.unwrap_err();
        assert!(err_traversal.0.contains("without a folder") || err_traversal.0.contains("name only"), "Path traversal in filename must be rejected: {}", err_traversal.0);

        // Anomaly G: Conversion from invalid docx filename
        let err_docx_convert_mismatch = pdf_write_docx.call(PdfWriteDocxArgs {
            filename: "out.pdf".into(),
            docx_filename: "invoice.pdf".into(), // PDF is not a DOCX
            title: None,
        }, &ctx).await.unwrap_err();
        assert!(err_docx_convert_mismatch.0.contains("cannot read 'invoice.pdf'"), "Should reject non-docx source: {}", err_docx_convert_mismatch.0);
    }
}
