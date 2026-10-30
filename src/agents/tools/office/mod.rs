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
pub mod xlsx;
#[cfg(test)]
mod fixture_tests;

pub use crate::agents::tools::session_files::{DOCX_MIME, XLS_MIME, XLSX_MIME};

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
    Other,
}

fn format_of(mime: &str, filename: &str) -> Format {
    let base = mime.split(';').next().unwrap_or(mime).trim();
    match base {
        DOCX_MIME => return Format::Docx,
        XLSX_MIME => return Format::Xlsx,
        XLS_MIME => return Format::Xls,
        _ => {}
    }
    match filename.rsplit('.').next().map(str::to_ascii_lowercase).as_deref() {
        Some("docx") => Format::Docx,
        Some("xlsx") => Format::Xlsx,
        Some("xls") => Format::Xls,
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
        Format::Other => {
            "it is not a Word or Excel file: use read_document_file for text and PDF, or read_image_file for images".to_string()
        }
    };
    VizierError(format!("{wanted} cannot read '{filename}': {hint}."))
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
        assert_eq!(format_of("application/pdf", "a.pdf"), Format::Other);
    }

    #[test]
    fn a_wrong_file_type_names_the_tool_to_use() {
        let docx = wrong_tool("a.docx", Format::Docx, "office_xlsx_read_as_cells");
        assert!(docx.0.contains("office_docx_read_as_md"));
        let xlsx = wrong_tool("a.xlsx", Format::Xlsx, "office_docx_read_as_md");
        assert!(xlsx.0.contains("office_xlsx_read_as_cells") && xlsx.0.contains("office_xlsx_read_as_csv"));
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
}
