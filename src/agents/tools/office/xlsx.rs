//! Reading Excel workbooks (`.xlsx`, and legacy `.xls`) as typed cells or as CSV text.
//!
//! Pure functions on bytes: no storage, no `ToolContext`, so the format logic is unit-testable
//! on its own (spec `034-office-addon`, FR-004 and FR-030). Formulas are never calculated here:
//! a formula cell is read as the value the spreadsheet application last saved, optionally
//! together with its formula text.

use std::io::Cursor;

use calamine::{Data, Range, Reader, Xls, Xlsx};

use super::{
    Cell, Dimensions, MAX_COLS, MAX_FORMULA_CELLS, MAX_ROWS, MAX_SHEETS, READ_MAX_CHARS, SheetCsv,
    SheetRead, WorkbookCsv, WorkbookRead,
};
use crate::error::VizierError;

/// Which Excel format the bytes are, decided from the media type or file name by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flavour {
    Xlsx,
    Xls,
}

enum Book {
    Xlsx(Xlsx<Cursor<Vec<u8>>>),
    Xls(Xls<Cursor<Vec<u8>>>),
}

impl Book {
    fn open(bytes: &[u8], flavour: Flavour) -> Result<Self, VizierError> {
        let not_valid = |e: String| {
            VizierError(format!(
                "This is not a valid Excel file (it may be corrupt or password-protected): {e}"
            ))
        };
        let cursor = Cursor::new(bytes.to_vec());
        match flavour {
            Flavour::Xlsx => calamine::open_workbook_from_rs::<Xlsx<_>, _>(cursor)
                .map(Book::Xlsx)
                .map_err(|e| not_valid(e.to_string())),
            Flavour::Xls => calamine::open_workbook_from_rs::<Xls<_>, _>(cursor)
                .map(Book::Xls)
                .map_err(|e| not_valid(e.to_string())),
        }
    }

    fn sheet_names(&self) -> Vec<String> {
        match self {
            Book::Xlsx(b) => b.sheet_names(),
            Book::Xls(b) => b.sheet_names(),
        }
    }

    fn range(&mut self, name: &str) -> Result<Range<Data>, VizierError> {
        let read = |e: String| VizierError(format!("Could not read sheet '{name}': {e}"));
        match self {
            Book::Xlsx(b) => b.worksheet_range(name).map_err(|e| read(e.to_string())),
            Book::Xls(b) => b.worksheet_range(name).map_err(|e| read(e.to_string())),
        }
    }

    /// The formula text of each formula cell; empty when there are none, or for a format that
    /// does not expose them.
    fn formulas(&mut self, name: &str) -> Range<String> {
        let found = match self {
            Book::Xlsx(b) => b.worksheet_formula(name).ok(),
            Book::Xls(b) => b.worksheet_formula(name).ok(),
        };
        found.unwrap_or_default()
    }

    fn is_xls(&self) -> bool {
        matches!(self, Book::Xls(_))
    }
}

/// A requested block of cells, zero-based and inclusive, as absolute sheet positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub first: (u32, u32),
    pub last: (u32, u32),
}

/// The letters of a zero-based column index: 0 is `A`, 26 is `AA`.
pub fn col_letters(mut col: u32) -> String {
    let mut out = Vec::new();
    loop {
        out.push(b'A' + (col % 26) as u8);
        if col < 26 {
            break;
        }
        col = col / 26 - 1;
    }
    out.reverse();
    String::from_utf8_lossy(&out).into_owned()
}

/// The A1 address of a zero-based `(row, col)`.
pub fn address(position: (u32, u32)) -> String {
    format!("{}{}", col_letters(position.1), position.0 + 1)
}

fn parse_cell(text: &str) -> Result<(u32, u32), VizierError> {
    let bad = || {
        VizierError(format!(
            "'{text}' is not a cell address. Use A1 notation such as 'C7' or 'B2:F40'."
        ))
    };
    let text = text.trim().replace('$', "");
    let split = text.find(|c: char| c.is_ascii_digit()).ok_or_else(bad)?;
    let (letters, digits) = text.split_at(split);
    if letters.is_empty() || !letters.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(bad());
    }
    let mut col: u64 = 0;
    for c in letters.chars() {
        col = col * 26 + (c.to_ascii_uppercase() as u64 - 'A' as u64 + 1);
        if col > 16_384 {
            return Err(bad());
        }
    }
    let row: u32 = digits.parse().map_err(|_| bad())?;
    if row == 0 {
        return Err(bad());
    }
    Ok((row - 1, (col - 1) as u32))
}

/// `B2:F40` or a single cell `C7`.
pub fn parse_range(text: &str) -> Result<Window, VizierError> {
    let (a, b) = match text.split_once(':') {
        Some((a, b)) => (parse_cell(a)?, parse_cell(b)?),
        None => {
            let cell = parse_cell(text)?;
            (cell, cell)
        }
    };
    Ok(Window {
        first: (a.0.min(b.0), a.1.min(b.1)),
        last: (a.0.max(b.0), a.1.max(b.1)),
    })
}

fn range_text(first: (u32, u32), last: (u32, u32)) -> String {
    format!("{}:{}", address(first), address(last))
}

/// A sheet's cells for one read, with the addresses they came from.
struct Block {
    name: String,
    /// The area of the sheet that holds data, as its first and last cell.
    used: Option<((u32, u32), (u32, u32))>,
    /// The sheet position of `cells[0][0]`.
    start: (u32, u32),
    cells: Vec<Vec<Data>>,
    formulas: Vec<Vec<Option<String>>>,
    /// Something was cut: a cap on rows, columns or size.
    cut: bool,
}

impl Block {
    fn dimensions(&self) -> Dimensions {
        match self.used {
            Some((first, last)) => Dimensions {
                rows: (last.0 - first.0 + 1) as usize,
                cols: (last.1 - first.1 + 1) as usize,
            },
            None => Dimensions { rows: 0, cols: 0 },
        }
    }

    fn used_range(&self) -> Option<String> {
        self.used.map(|(first, last)| range_text(first, last))
    }
}

/// How many characters of output a read may still produce.
struct Budget(usize);

impl Budget {
    fn take(&mut self, n: usize) -> bool {
        if n > self.0 {
            self.0 = 0;
            false
        } else {
            self.0 -= n;
            true
        }
    }
}

fn sheet_block(
    name: &str,
    data: &Range<Data>,
    formulas: &Range<String>,
    window: Option<Window>,
    budget: &mut Budget,
) -> Block {
    let used = data.start().zip(data.end());
    let (first, last) = match (window, used) {
        (Some(w), _) => (w.first, w.last),
        (None, Some((first, last))) => (first, last),
        (None, None) => {
            return Block {
                name: name.to_string(),
                used,
                start: (0, 0),
                cells: vec![],
                formulas: vec![],
                cut: false,
            };
        }
    };
    let wanted_rows = (last.0 - first.0 + 1) as usize;
    let wanted_cols = (last.1 - first.1 + 1) as usize;
    let rows = wanted_rows.min(MAX_ROWS);
    let cols = wanted_cols.min(MAX_COLS);
    let mut cut = rows < wanted_rows || cols < wanted_cols;

    let mut cells = Vec::with_capacity(rows);
    let mut formula_rows = Vec::with_capacity(rows);
    for r in 0..rows {
        let mut row = Vec::with_capacity(cols);
        let mut formula_row = Vec::with_capacity(cols);
        let mut size = 2;
        for c in 0..cols {
            let position = (first.0 + r as u32, first.1 + c as u32);
            let value = data.get_value(position).cloned().unwrap_or(Data::Empty);
            size += cell_size(&value) + 1;
            row.push(value);
            formula_row.push(
                formulas
                    .get_value(position)
                    .filter(|f| !f.is_empty())
                    .cloned(),
            );
        }
        if !budget.take(size) {
            cut = true;
            break;
        }
        cells.push(row);
        formula_rows.push(formula_row);
    }
    Block {
        name: name.to_string(),
        used,
        start: first,
        cells,
        formulas: formula_rows,
        cut,
    }
}

/// A rough size of a cell in the output, enough to bound it.
fn cell_size(data: &Data) -> usize {
    match data {
        Data::Empty => 4,
        Data::String(s) | Data::DateTimeIso(s) | Data::DurationIso(s) => s.len() + 2,
        _ => 12,
    }
}

fn to_cell(data: &Data) -> Cell {
    match data {
        Data::Empty => Cell::Null,
        Data::String(s) => Cell::Text(s.clone()),
        Data::Float(f) => Cell::number(*f),
        Data::Int(i) => Cell::Number((*i).into()),
        Data::Bool(b) => Cell::Bool(*b),
        Data::Error(e) => Cell::Error { error: e.to_string() },
        Data::DateTime(dt) if dt.is_datetime() => Cell::Date { date: serial_to_iso(dt.as_f64()) },
        Data::DateTime(dt) => Cell::number(dt.as_f64()),
        Data::DateTimeIso(s) => Cell::Date { date: s.clone() },
        Data::DurationIso(s) => Cell::Text(s.clone()),
    }
}

/// An Excel serial date as ISO-8601: a date alone at midnight, a date and time otherwise.
fn serial_to_iso(serial: f64) -> String {
    let days = serial.floor();
    let millis = ((serial - days) * 86_400_000.0).round() as i64;
    let moment = chrono::NaiveDate::from_ymd_opt(1899, 12, 30)
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .and_then(|epoch| {
            let delta = chrono::TimeDelta::try_days(days as i64)?
                .checked_add(&chrono::TimeDelta::try_milliseconds(millis)?)?;
            epoch.checked_add_signed(delta)
        });
    match moment {
        Some(m) if m.time() == chrono::NaiveTime::MIN => m.format("%Y-%m-%d").to_string(),
        Some(m) => m.format("%Y-%m-%dT%H:%M:%S").to_string(),
        None => serial.to_string(),
    }
}

/// The sheets a read covers: the named one, or all of them.
fn select_sheets(book: &Book, sheet: Option<&str>) -> Result<Vec<String>, VizierError> {
    let all = book.sheet_names();
    let Some(wanted) = sheet else {
        return Ok(all);
    };
    let found = all
        .iter()
        .find(|name| name.as_str() == wanted)
        .or_else(|| all.iter().find(|name| name.eq_ignore_ascii_case(wanted)));
    match found {
        Some(name) => Ok(vec![name.clone()]),
        None => Err(VizierError(format!(
            "There is no sheet named '{wanted}'. The sheets in this workbook are: {}.",
            all.join(", ")
        ))),
    }
}

/// Read a workbook as typed cells (FR-004). `info` returns only each sheet's name, data area
/// and size. `formulas` shows each formula cell as `{value, formula}`.
pub fn read_workbook(
    bytes: &[u8],
    flavour: Flavour,
    sheet: Option<&str>,
    range: Option<&str>,
    info: bool,
    formulas: bool,
) -> Result<WorkbookRead, VizierError> {
    let window = range.map(parse_range).transpose()?;
    let mut book = Book::open(bytes, flavour)?;
    let names = select_sheets(&book, sheet)?;

    let mut out = WorkbookRead::default();
    let mut budget = Budget(READ_MAX_CHARS);
    let mut notes: Vec<String> = Vec::new();

    for (index, name) in names.iter().enumerate() {
        let data = book.range(name)?;
        if info {
            let block = Block {
                name: name.clone(),
                used: data.start().zip(data.end()),
                start: (0, 0),
                cells: vec![],
                formulas: vec![],
                cut: false,
            };
            out.sheets.push(SheetRead {
                name: block.name.clone(),
                used_range: block.used_range(),
                dimensions: block.dimensions(),
                start: None,
                rows: None,
            });
            continue;
        }
        if index >= MAX_SHEETS {
            out.truncated = true;
            notes.push(format!(
                "Only the first {MAX_SHEETS} of {} sheets are returned; name a sheet to read another.",
                names.len()
            ));
            break;
        }
        let formula_range = book.formulas(name);
        let block = sheet_block(name, &data, &formula_range, window, &mut budget);
        out.truncated |= block.cut;

        let mut missing = Vec::new();
        let mut rows = Vec::with_capacity(block.cells.len());
        for (r, row) in block.cells.iter().enumerate() {
            let mut cells = Vec::with_capacity(row.len());
            for (c, data) in row.iter().enumerate() {
                let formula = block.formulas[r][c].as_ref();
                let saved = to_cell(data);
                cells.push(match formula {
                    Some(text) => {
                        if matches!(data, Data::Empty) {
                            missing.push(address((block.start.0 + r as u32, block.start.1 + c as u32)));
                        }
                        if formulas {
                            Cell::Formula {
                                value: (!matches!(saved, Cell::Null)).then(|| Box::new(saved)),
                                formula: format!("={text}"),
                            }
                        } else {
                            saved
                        }
                    }
                    None => saved,
                });
            }
            rows.push(cells);
        }
        if !missing.is_empty() {
            notes.push(no_saved_value_note(name, &missing));
        }
        out.sheets.push(SheetRead {
            name: block.name.clone(),
            used_range: block.used_range(),
            dimensions: block.dimensions(),
            start: Some(address(block.start)),
            rows: Some(rows),
        });
    }
    if formulas && book.is_xls() {
        notes.push("Formula text is not available for legacy .xls files; formula cells show their saved value.".into());
    }
    if out.truncated {
        notes.push("The result was cut at the output limits (sheets, rows, columns or size); `dimensions` shows the real size. Read a smaller `range` to see the rest.".into());
    }
    out.note = (!notes.is_empty()).then(|| notes.join(" "));
    Ok(out)
}

fn no_saved_value_note(sheet: &str, missing: &[String]) -> String {
    let shown: Vec<&str> = missing.iter().take(20).map(String::as_str).collect();
    let more = if missing.len() > shown.len() {
        format!(" and {} more", missing.len() - shown.len())
    } else {
        String::new()
    };
    format!(
        "Sheet '{sheet}': formula cells with no saved value (shown as empty, they are not blank): {}{more}.",
        shown.join(", ")
    )
}

/// One CSV field (RFC 4180): quoted when it holds a comma, a quote or a line break.
fn csv_field(text: &str) -> String {
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_string()
    }
}

fn csv_text(data: &Data) -> String {
    match to_cell(data) {
        Cell::Null => String::new(),
        Cell::Text(s) => s,
        Cell::Number(n) => n.to_string(),
        Cell::Bool(b) => b.to_string(),
        Cell::Date { date } => date,
        Cell::Error { error } => error,
        Cell::Formula { .. } => String::new(),
    }
}

/// Read a workbook as CSV text (FR-030), every sheet by default. With `executed_formula` false
/// a formula cell is its formula text; calculating formulas is not available yet, so with it
/// true a formula cell shows its saved value and the result says so.
pub fn read_csv(
    bytes: &[u8],
    flavour: Flavour,
    sheet: Option<&str>,
    range: Option<&str>,
    executed_formula: bool,
) -> Result<WorkbookCsv, VizierError> {
    let window = range.map(parse_range).transpose()?;
    let mut book = Book::open(bytes, flavour)?;
    let names = select_sheets(&book, sheet)?;

    let mut out = WorkbookCsv::default();
    let mut budget = Budget(READ_MAX_CHARS);
    let mut notes: Vec<String> = Vec::new();

    for (index, name) in names.iter().enumerate() {
        if index >= MAX_SHEETS {
            out.truncated = true;
            notes.push(format!(
                "Only the first {MAX_SHEETS} of {} sheets are returned; name a sheet to read another.",
                names.len()
            ));
            break;
        }
        let data = book.range(name)?;
        let formula_range = book.formulas(name);
        let block = sheet_block(name, &data, &formula_range, window, &mut budget);
        out.truncated |= block.cut;

        let mut csv = String::new();
        let mut formula_cells = Vec::new();
        let mut uncounted = 0usize;
        for (r, row) in block.cells.iter().enumerate() {
            let fields: Vec<String> = row
                .iter()
                .enumerate()
                .map(|(c, data)| match &block.formulas[r][c] {
                    Some(text) => {
                        let at = address((block.start.0 + r as u32, block.start.1 + c as u32));
                        if formula_cells.len() < MAX_FORMULA_CELLS {
                            formula_cells.push(at);
                        } else {
                            uncounted += 1;
                        }
                        if executed_formula {
                            csv_field(&csv_text(data))
                        } else {
                            csv_field(&format!("={text}"))
                        }
                    }
                    None => csv_field(&csv_text(data)),
                })
                .collect();
            csv.push_str(&fields.join(","));
            csv.push('\n');
        }
        if uncounted > 0 {
            notes.push(format!(
                "Sheet '{name}': {uncounted} more formula cells than the {MAX_FORMULA_CELLS} addresses listed."
            ));
        }
        out.sheets.push(SheetCsv {
            name: block.name.clone(),
            used_range: block.used_range(),
            start: block.used.is_some().then(|| address(block.start)),
            formula_cells,
            csv,
        });
    }
    if executed_formula {
        notes.push("executed_formula is not available yet: formula cells show their last saved value, not a calculated result.".into());
    }
    if out.truncated {
        notes.push("The result was cut at the output limits (sheets, rows, columns or size). Read a smaller `range` to see the rest.".into());
    }
    out.note = (!notes.is_empty()).then(|| notes.join(" "));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use rust_xlsxwriter::{ExcelDateTime, Format, Formula, Workbook};

    use super::*;

    fn mixed() -> Vec<u8> {
        let mut book = Workbook::new();
        let date = Format::new().set_num_format("yyyy-mm-dd");
        let budget = book.add_worksheet();
        budget.set_name("Budget").unwrap();
        for (c, h) in ["Item", "Qty", "Due"].iter().enumerate() {
            budget.write_string(0, c as u16, *h).unwrap();
        }
        budget.write_string(1, 0, "Pens").unwrap();
        budget.write_number(1, 1, 12).unwrap();
        budget
            .write_datetime_with_format(1, 2, ExcelDateTime::from_ymd(2026, 11, 1).unwrap(), &date)
            .unwrap();
        budget.write_string(2, 0, "Done").unwrap();
        budget.write_boolean(2, 1, true).unwrap();
        budget.write_string(3, 0, "Total").unwrap();
        budget
            .write_formula(3, 1, Formula::new("=SUM(B2:B2)").set_result("12"))
            .unwrap();
        budget.write_string(4, 0, "a, \"b\"").unwrap();
        // A sheet whose data starts at C5.
        let offset = book.add_worksheet();
        offset.set_name("Offset").unwrap();
        offset.write_string(4, 2, "x").unwrap();
        offset.write_number(5, 3, 2.5).unwrap();
        book.save_to_buffer().unwrap()
    }

    fn cells(read: &WorkbookRead, sheet: usize) -> &Vec<Vec<Cell>> {
        read.sheets[sheet].rows.as_ref().unwrap()
    }

    #[test]
    fn typed_cells_keep_their_types() {
        let read = read_workbook(&mixed(), Flavour::Xlsx, Some("Budget"), None, false, false).unwrap();
        let rows = cells(&read, 0);
        assert_eq!(rows[0][0], Cell::Text("Item".into()));
        assert_eq!(rows[1][1], Cell::number(12.0));
        assert_eq!(rows[1][2], Cell::Date { date: "2026-11-01".into() });
        assert_eq!(rows[2][1], Cell::Bool(true));
        assert_eq!(rows[2][2], Cell::Null);
        assert_eq!(read.sheets[0].start.as_deref(), Some("A1"));
        assert_eq!(read.sheets[0].used_range.as_deref(), Some("A1:C5"));
        assert_eq!(read.sheets[0].dimensions, Dimensions { rows: 5, cols: 3 });
    }

    #[test]
    fn a_range_can_be_a_block_or_one_cell() {
        let block = read_workbook(&mixed(), Flavour::Xlsx, Some("Budget"), Some("A1:B2"), false, false).unwrap();
        assert_eq!(cells(&block, 0).len(), 2);
        assert_eq!(cells(&block, 0)[0].len(), 2);
        assert_eq!(block.sheets[0].start.as_deref(), Some("A1"));

        let one = read_workbook(&mixed(), Flavour::Xlsx, Some("Budget"), Some("B2"), false, false).unwrap();
        assert_eq!(cells(&one, 0), &vec![vec![Cell::number(12.0)]]);
        assert_eq!(one.sheets[0].start.as_deref(), Some("B2"));
    }

    #[test]
    fn the_first_cell_address_follows_where_the_data_starts() {
        let read = read_workbook(&mixed(), Flavour::Xlsx, Some("Offset"), None, false, false).unwrap();
        let sheet = &read.sheets[0];
        assert_eq!(sheet.used_range.as_deref(), Some("C5:D6"));
        assert_eq!(sheet.start.as_deref(), Some("C5"));
        assert_eq!(cells(&read, 0)[0][0], Cell::Text("x".into()));
        let picked = read_workbook(&mixed(), Flavour::Xlsx, Some("Offset"), Some("D6"), false, false).unwrap();
        assert_eq!(cells(&picked, 0), &vec![vec![Cell::number(2.5)]]);
    }

    #[test]
    fn info_returns_only_the_sheet_facts() {
        let read = read_workbook(&mixed(), Flavour::Xlsx, None, None, true, false).unwrap();
        assert_eq!(read.sheets.len(), 2);
        assert!(read.sheets.iter().all(|s| s.rows.is_none() && s.start.is_none()));
        assert_eq!(read.sheets[0].name, "Budget");
        assert_eq!(read.sheets[0].dimensions, Dimensions { rows: 5, cols: 3 });
        assert_eq!(read.sheets[1].used_range.as_deref(), Some("C5:D6"));
        assert!(!read.truncated);
    }

    #[test]
    fn formulas_show_their_saved_value_and_on_request_their_text() {
        let saved = read_workbook(&mixed(), Flavour::Xlsx, Some("Budget"), Some("B4"), false, false).unwrap();
        assert_eq!(cells(&saved, 0), &vec![vec![Cell::number(12.0)]]);

        let text = read_workbook(&mixed(), Flavour::Xlsx, Some("Budget"), Some("B4"), false, true).unwrap();
        assert_eq!(
            cells(&text, 0)[0][0],
            Cell::Formula { value: Some(Box::new(Cell::number(12.0))), formula: "=SUM(B2:B2)".into() }
        );
    }

    #[test]
    fn an_unknown_sheet_lists_the_real_ones() {
        let err = read_workbook(&mixed(), Flavour::Xlsx, Some("Nope"), None, false, false).unwrap_err();
        assert!(err.0.contains("Budget") && err.0.contains("Offset"), "{}", err.0);
        assert!(read_workbook(&mixed(), Flavour::Xlsx, Some("budget"), None, false, false).is_ok());
    }

    #[test]
    fn a_bad_range_or_file_is_an_error_not_a_panic() {
        assert!(read_workbook(&mixed(), Flavour::Xlsx, None, Some("zz"), false, false).is_err());
        assert!(read_workbook(&mixed(), Flavour::Xlsx, None, Some("A0"), false, false).is_err());
        assert!(read_workbook(b"not a workbook", Flavour::Xlsx, None, None, false, false).is_err());
        assert!(read_workbook(b"not a workbook", Flavour::Xls, None, None, true, false).is_err());
    }

    #[test]
    fn big_sheets_are_capped_and_report_their_real_size() {
        let mut book = Workbook::new();
        let sheet = book.add_worksheet();
        for r in 0..(MAX_ROWS as u32 + 500) {
            sheet.write_number(r, 0, r).unwrap();
        }
        let read = read_workbook(&book.save_to_buffer().unwrap(), Flavour::Xlsx, None, None, false, false).unwrap();
        assert!(read.truncated);
        assert_eq!(cells(&read, 0).len(), MAX_ROWS);
        assert_eq!(read.sheets[0].dimensions.rows, MAX_ROWS + 500);
        assert!(read.note.is_some());
    }

    #[test]
    fn addresses_and_ranges_parse() {
        assert_eq!(address((0, 0)), "A1");
        assert_eq!(address((6, 26)), "AA7");
        assert_eq!(parse_cell("c7").unwrap(), (6, 2));
        assert_eq!(parse_range("F40:B2").unwrap(), Window { first: (1, 1), last: (39, 5) });
        assert!(parse_cell("1A").is_err());
    }

    #[test]
    fn csv_has_every_sheet_with_quoting_and_addresses() {
        let read = read_csv(&mixed(), Flavour::Xlsx, None, None, false).unwrap();
        assert_eq!(read.sheets.len(), 2);
        let budget = &read.sheets[0];
        let lines: Vec<&str> = budget.csv.lines().collect();
        assert_eq!(lines[0], "Item,Qty,Due");
        assert_eq!(lines[1], "Pens,12,2026-11-01");
        assert_eq!(lines[2], "Done,true,");
        assert_eq!(lines[3], "Total,=SUM(B2:B2),");
        assert_eq!(lines[4], "\"a, \"\"b\"\"\",,");
        assert_eq!(budget.start.as_deref(), Some("A1"));
        assert_eq!(budget.formula_cells, ["B4"]);
        assert_eq!(read.sheets[1].start.as_deref(), Some("C5"));
        assert!(read.note.is_none());
    }

    #[test]
    fn csv_honours_sheet_and_range() {
        let read = read_csv(&mixed(), Flavour::Xlsx, Some("Budget"), Some("B2"), false).unwrap();
        assert_eq!(read.sheets.len(), 1);
        assert_eq!(read.sheets[0].csv, "12\n");
        assert_eq!(read.sheets[0].start.as_deref(), Some("B2"));
    }

    #[test]
    fn csv_shows_saved_values_and_says_calculation_is_not_available() {
        let read = read_csv(&mixed(), Flavour::Xlsx, Some("Budget"), Some("A4:B4"), true).unwrap();
        assert_eq!(read.sheets[0].csv, "Total,12\n");
        assert!(read.note.as_deref().unwrap().contains("not available yet"));
    }
}
