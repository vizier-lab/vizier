//! Readers against golden files written by other tools (python-docx, openpyxl, xlwt and
//! hand-written OOXML), not by this crate's own writers.

use super::docx::to_markdown;
use super::xlsx::{read_csv, read_workbook, Flavour};
use super::{Cell, Dimensions, SheetRead, WorkbookRead};

/// The files are generated outside the repository and not committed, so a test is
/// skipped (with a message) when its file has not been generated.
fn fixture(name: &str) -> Option<Vec<u8>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/agents/tools/office/fixtures")
        .join(name);
    match std::fs::read(&path) {
        Ok(bytes) => Some(bytes),
        Err(_) => {
            eprintln!("skipped: {} is missing; generate the fixtures first (see the Office addon PR notes)", path.display());
            None
        }
    }
}

macro_rules! fixture {
    ($name:expr) => {
        match fixture($name) {
            Some(bytes) => bytes,
            None => return,
        }
    };
}

fn rows(sheet: &SheetRead) -> &Vec<Vec<Cell>> {
    sheet.rows.as_ref().unwrap()
}

fn sheet<'a>(read: &'a WorkbookRead, name: &str) -> &'a SheetRead {
    read.sheets.iter().find(|s| s.name == name).unwrap()
}

#[test]
fn a_word_table_is_a_markdown_pipe_table() {
    let table = fixture!("table.docx");
    let md = to_markdown(&table, 100_000).unwrap().markdown;
    assert!(!md.contains("[table content]"), "{md}");
    assert!(md.contains("# Quarterly Report"), "{md}");
    assert!(md.contains("**bold**") && md.contains("*italic*"), "{md}");
    assert!(md.contains("| Name | Note |\n|---|---|\n| alpha | a\|b |\n| beta | plain |"), "{md}");
    assert!(md.contains("[the docs](https://example.com/docs)"), "{md}");
    assert!(md.contains("Top item") && md.contains("Nested item"), "{md}");
}

#[test]
fn an_xlsx_from_another_tool_keeps_its_types() {
    let mixed = fixture!("mixed.xlsx");
    let read = read_workbook(&mixed, Flavour::Xlsx, None, None, false, false).unwrap();
    assert_eq!(read.sheets.len(), 2);

    let types = sheet(&read, "Types");
    let r = rows(types);
    assert_eq!(r[0], vec![Cell::Text("Name".into()), Cell::Text("Value".into())]);
    assert_eq!(r[1][1], Cell::Text("hello".into()));
    assert_eq!(r[2][1], Cell::number(12.0));
    assert_eq!(r[3][1], Cell::number(2.5));
    assert_eq!(r[4][1], Cell::Bool(true));
    assert_eq!(r[5][1], Cell::Date { date: "2026-11-01".into() });
    assert_eq!(r[6][1], Cell::Null);
    assert_eq!(r[7][1], Cell::Error { error: "#DIV/0!".into() });
    assert_eq!(types.dimensions, Dimensions { rows: 8, cols: 2 });

    let offset = sheet(&read, "Offset");
    assert_eq!(offset.start.as_deref(), Some("C5"));
    assert_eq!(offset.used_range.as_deref(), Some("C5:D6"));
    assert_eq!(rows(offset)[0][0], Cell::Text("x".into()));
    assert_eq!(rows(offset)[1][1], Cell::number(7.0));
}

#[test]
fn a_formula_shows_its_saved_value_and_a_missing_one_is_named() {
    let formulas = fixture!("formulas.xlsx");
    let read = read_workbook(&formulas, Flavour::Xlsx, Some("Data"), None, false, false).unwrap();
    let r = rows(&read.sheets[0]);
    assert_eq!(r[3][1], Cell::number(42.0));
    assert_eq!(r[4][1], Cell::Text("big".into()));
    assert_eq!(r[5][1], Cell::Null);
    assert_eq!(r[6][1], Cell::Error { error: "#DIV/0!".into() });
    let note = read.note.unwrap();
    assert!(note.contains("no saved value") && note.contains("B6"), "{note}");
    assert!(!note.contains("B4"), "{note}");

    let with = read_workbook(&formulas, Flavour::Xlsx, Some("Data"), None, false, true).unwrap();
    assert_eq!(
        rows(&with.sheets[0])[3][1],
        Cell::Formula { value: Some(Box::new(Cell::number(42.0))), formula: "=SUM(B2:B3)".into() }
    );
    assert_eq!(
        rows(&with.sheets[0])[5][1],
        Cell::Formula { value: None, formula: "=B2*2".into() }
    );

    let cross = read_workbook(&formulas, Flavour::Xlsx, Some("Summary"), None, false, false).unwrap();
    assert_eq!(rows(&cross.sheets[0])[0][1], Cell::number(42.0));
}

#[test]
fn csv_lists_the_formula_cells_of_a_real_file() {
    let formulas = fixture!("formulas.xlsx");
    let csv = read_csv(&formulas, Flavour::Xlsx, Some("Data"), None, false).unwrap();
    let s = &csv.sheets[0];
    assert_eq!(s.formula_cells, vec!["B4", "B5", "B6", "B7"]);
    assert!(s.csv.contains("sum,=SUM(B2:B3)"), "{}", s.csv);
}

#[test]
fn a_legacy_xls_reads_through_the_xls_flavour() {
    let legacy = fixture!("legacy.xls");
    let read = read_workbook(&legacy, Flavour::Xls, None, None, false, false).unwrap();
    let r = rows(&read.sheets[0]);
    assert_eq!(read.sheets[0].name, "Old");
    assert_eq!(r[0], vec![Cell::Text("Item".into()), Cell::Text("Qty".into())]);
    assert_eq!(r[1], vec![Cell::Text("pens".into()), Cell::number(12.0)]);
    assert_eq!(r[2], vec![Cell::Text("ink".into()), Cell::number(2.5)]);
}
