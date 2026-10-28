//! Calculating the formulas of a workbook, for `office_xlsx_read_as_csv` with
//! `executed_formula: true` (spec `034-office-addon`, FR-030; research R10).
//!
//! This is the only file that uses the formula engine crate (IronCalc), so replacing the engine
//! touches nothing else. The engine is asked for the formula cells the caller will print and
//! answers each one with a value or with "cannot say", never with an error that ends the turn:
//!
//! - a cell the engine cannot calculate (an unknown or unimplemented function, a circular
//!   reference, a link to another file) is [`Outcome::Unavailable`], and the caller prints the
//!   value the file saved for it and names the cell;
//! - a workbook the engine cannot read, has too many formulas for, or does not finish in time
//!   is a [`Calculation::failure`], and the caller does the same for every formula cell;
//! - links to other files are never followed (the engine has no way to).
//!
//! IronCalc cannot be interrupted, so the calculation runs on a worker thread that is waited for
//! a bounded time. A run that overruns is abandoned (the thread ends when the engine returns);
//! at most [`MAX_WORKERS`] exist at once, abandoned ones included, so a hostile workbook cannot
//! pile up threads.

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use ironcalc::base::Model;
use ironcalc::base::cell::CellValue;
use ironcalc::base::types::CellType;
use ironcalc::import::load_from_xlsx_bytes;

/// How long a calculation is waited for.
const CALC_TIMEOUT: Duration = Duration::from_secs(10);
/// More formulas than this in the whole workbook and nothing is calculated. A dependency chain
/// cannot be longer than the number of formulas, so this also bounds the engine's recursion.
const MAX_FORMULAS: usize = 20_000;
/// The engine evaluates dependencies recursively; the stack is only committed as it is used.
const WORKER_STACK: usize = 256 * 1024 * 1024;

/// Error codes that say "the engine cannot do this", not "the formula's result is an error".
/// `#NAME?` is here because an unknown function and a function the engine lacks look the same.
const ENGINE_LIMITS: [&str; 4] = ["#NAME?", "#N/IMPL", "#CIRC!", "#ERROR!"];

/// How many calculations may exist at once, counting ones abandoned after the time limit that
/// are still inside the engine. Past this a request is refused, so a hostile workbook cannot
/// pile up threads.
const MAX_WORKERS: usize = 16;

/// Calculations running now, abandoned ones included.
static WORKERS: AtomicUsize = AtomicUsize::new(0);

/// A value the engine calculated.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(f64),
    Text(String),
    Bool(bool),
    /// A result that is an Excel error (`#DIV/0!`, `#VALUE!`, `#REF!`, `#N/A`, `#NUM!`, ...).
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Value(Value),
    /// The engine could not calculate this cell; the saved value stands.
    Unavailable,
}

/// The formula cells of one sheet to read back: 0-based absolute `(row, column)`.
pub struct Wanted {
    pub sheet: String,
    pub cells: Vec<(u32, u32)>,
}

#[derive(Debug, Default)]
pub struct Calculation {
    /// Keyed by sheet name and 0-based absolute `(row, column)`.
    pub cells: HashMap<(String, u32, u32), Outcome>,
    /// Why nothing was calculated, when that is the case.
    pub failure: Option<String>,
}

impl Calculation {
    fn failed(reason: impl Into<String>) -> Self {
        Self { cells: HashMap::new(), failure: Some(reason.into()) }
    }

    pub fn get(&self, sheet: &str, row: u32, col: u32) -> Option<&Outcome> {
        self.cells.get(&(sheet.to_string(), row, col))
    }
}

/// Calculate the workbook in `bytes` (an `.xlsx`) and read back the `wanted` formula cells.
pub fn calculate(bytes: &[u8], wanted: Vec<Wanted>) -> Calculation {
    if WORKERS.fetch_add(1, Ordering::SeqCst) >= MAX_WORKERS {
        WORKERS.fetch_sub(1, Ordering::SeqCst);
        return Calculation::failed("too many calculations are running, try again in a moment");
    }
    let (sender, receiver) = mpsc::channel();
    let bytes = bytes.to_vec();
    let spawned = std::thread::Builder::new()
        .name("office-formula".into())
        .stack_size(WORKER_STACK)
        .spawn(move || {
            let result = catch_unwind(AssertUnwindSafe(|| run(&bytes, &wanted)));
            WORKERS.fetch_sub(1, Ordering::SeqCst);
            let _ = sender.send(
                result.unwrap_or_else(|_| Calculation::failed("the formula engine stopped unexpectedly")),
            );
        });
    if let Err(e) = spawned {
        WORKERS.fetch_sub(1, Ordering::SeqCst);
        tracing::warn!("could not start the formula worker: {e}");
        return Calculation::failed("the formula engine could not be started");
    }
    receiver.recv_timeout(CALC_TIMEOUT).unwrap_or_else(|_| {
        tracing::warn!("a formula calculation took longer than {CALC_TIMEOUT:?} and was abandoned");
        Calculation::failed(format!(
            "the calculation took longer than {} seconds",
            CALC_TIMEOUT.as_secs()
        ))
    })
}

fn run(bytes: &[u8], wanted: &[Wanted]) -> Calculation {
    let workbook = match load_from_xlsx_bytes(bytes, "workbook", "en", "UTC") {
        Ok(workbook) => workbook,
        Err(e) => {
            tracing::debug!("the formula engine could not read the workbook: {e}");
            return Calculation::failed("the formula engine could not read this file");
        }
    };
    let mut model = match Model::from_workbook(workbook, "en") {
        Ok(model) => model,
        Err(e) => {
            tracing::debug!("the formula engine could not build the workbook: {e}");
            return Calculation::failed("the formula engine could not read this file");
        }
    };
    // Count the formula cells (not `parsed_formulas`, which holds each distinct formula once).
    let formulas = model
        .workbook
        .worksheets
        .iter()
        .flat_map(|sheet| sheet.sheet_data.cells())
        .filter(|(_, _, cell)| cell.has_formula())
        .count();
    if formulas > MAX_FORMULAS {
        return Calculation::failed(format!(
            "the workbook has {formulas} formulas, more than the {MAX_FORMULAS} that are calculated"
        ));
    }
    model.evaluate();

    let names = model.workbook.get_worksheet_names();
    let mut cells = HashMap::new();
    for sheet in wanted {
        let index = names.iter().position(|name| name == &sheet.sheet);
        for &(row, col) in &sheet.cells {
            let outcome = match index {
                Some(index) => read_cell(&model, index as u32, row, col),
                None => Outcome::Unavailable,
            };
            cells.insert((sheet.sheet.clone(), row, col), outcome);
        }
    }
    Calculation { cells, failure: None }
}

/// One calculated cell. The engine counts rows and columns from 1.
fn read_cell(model: &Model, sheet: u32, row: u32, col: u32) -> Outcome {
    let (r, c) = (row as i32 + 1, col as i32 + 1);
    let Ok(value) = model.get_cell_value_by_index(sheet, r, c) else {
        return Outcome::Unavailable;
    };
    let is_error = matches!(model.get_cell_type(sheet, r, c), Ok(CellType::ErrorValue));
    match value {
        CellValue::String(code) if is_error => {
            if ENGINE_LIMITS.contains(&code.as_str()) {
                Outcome::Unavailable
            } else {
                Outcome::Value(Value::Error(code))
            }
        }
        CellValue::String(text) => Outcome::Value(Value::Text(text)),
        CellValue::Number(n) => Outcome::Value(Value::Number(n)),
        CellValue::Boolean(b) => Outcome::Value(Value::Bool(b)),
        CellValue::None => Outcome::Value(Value::Text(String::new())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_xlsxwriter::{ExcelDateTime, Format, Formula, Workbook};

    /// A complete package, as Excel writes one (the engine needs styles and document properties).
    /// Formulas are written with their saved result, like a file saved by a spreadsheet program.
    fn workbook(build: impl FnOnce(&mut Workbook)) -> Vec<u8> {
        let mut book = Workbook::new();
        build(&mut book);
        book.save_to_buffer().expect("a workbook is built")
    }

    fn wanted(sheet: &str, cells: &[(u32, u32)]) -> Vec<Wanted> {
        vec![Wanted { sheet: sheet.into(), cells: cells.to_vec() }]
    }

    fn value(calc: &Calculation, sheet: &str, row: u32, col: u32) -> Outcome {
        calc.get(sheet, row, col).cloned().expect("the cell was asked for")
    }

    #[test]
    fn it_recalculates_instead_of_echoing_the_saved_result() {
        let bytes = workbook(|book| {
            let ws = book.add_worksheet();
            ws.write_number(0, 0, 10.0).unwrap();
            ws.write_number(1, 0, 20.0).unwrap();
            // The saved result (999) is wrong on purpose; the true value is 30.
            ws.write_formula(2, 0, Formula::new("=SUM(A1:A2)").set_result("999")).unwrap();
            // A formula with no saved result at all, as a script-written file has.
            ws.write_formula(3, 0, "=A1*3").unwrap();
        });
        let calc = calculate(&bytes, wanted("Sheet1", &[(2, 0), (3, 0)]));
        assert_eq!(calc.failure, None);
        assert_eq!(value(&calc, "Sheet1", 2, 0), Outcome::Value(Value::Number(30.0)));
        assert_eq!(value(&calc, "Sheet1", 3, 0), Outcome::Value(Value::Number(30.0)));
    }

    #[test]
    fn it_calculates_text_booleans_errors_and_other_sheets() {
        let bytes = workbook(|book| {
            let first = book.add_worksheet();
            first.set_name("Data").unwrap();
            first.write_number(0, 0, 7.0).unwrap();
            first.write_formula(1, 0, r#"=IF(A1>5,"big","small")"#).unwrap();
            first.write_formula(2, 0, "=A1>5").unwrap();
            first.write_formula(3, 0, "=A1/0").unwrap();
            let second = book.add_worksheet();
            second.set_name("Summary").unwrap();
            second.write_formula(0, 0, "=Data!A1*2").unwrap();
        });
        let calc = calculate(
            &bytes,
            vec![
                Wanted { sheet: "Data".into(), cells: vec![(1, 0), (2, 0), (3, 0)] },
                Wanted { sheet: "Summary".into(), cells: vec![(0, 0)] },
            ],
        );
        assert_eq!(value(&calc, "Data", 1, 0), Outcome::Value(Value::Text("big".into())));
        assert_eq!(value(&calc, "Data", 2, 0), Outcome::Value(Value::Bool(true)));
        // A real Excel error is a result, not a failure to calculate.
        assert_eq!(value(&calc, "Data", 3, 0), Outcome::Value(Value::Error("#DIV/0!".into())));
        assert_eq!(value(&calc, "Summary", 0, 0), Outcome::Value(Value::Number(14.0)));
    }

    #[test]
    fn a_cell_the_engine_cannot_calculate_is_unavailable_and_the_others_still_are() {
        let bytes = workbook(|book| {
            let ws = book.add_worksheet();
            ws.write_number(0, 0, 4.0).unwrap();
            ws.write_formula(1, 0, "=FOOBAR(A1)").unwrap();
            ws.write_formula(2, 0, "=A4+1").unwrap();
            ws.write_formula(3, 0, "=A3+1").unwrap();
            ws.write_formula(4, 0, "=A1+1").unwrap();
        });
        let calc = calculate(&bytes, wanted("Sheet1", &[(1, 0), (2, 0), (3, 0), (4, 0)]));
        assert_eq!(calc.failure, None);
        assert_eq!(value(&calc, "Sheet1", 1, 0), Outcome::Unavailable, "unknown function");
        assert_eq!(value(&calc, "Sheet1", 2, 0), Outcome::Unavailable, "circular reference");
        assert_eq!(value(&calc, "Sheet1", 3, 0), Outcome::Unavailable, "circular reference");
        assert_eq!(value(&calc, "Sheet1", 4, 0), Outcome::Value(Value::Number(5.0)));
    }

    #[test]
    fn a_sheet_the_engine_does_not_have_is_unavailable() {
        let bytes = workbook(|book| {
            book.add_worksheet().write_formula(0, 0, "=1+1").unwrap();
        });
        let calc = calculate(&bytes, wanted("Nope", &[(0, 0)]));
        assert_eq!(value(&calc, "Nope", 0, 0), Outcome::Unavailable);
    }

    #[test]
    fn a_file_the_engine_cannot_read_is_a_failure_not_a_panic() {
        let calc = calculate(b"this is not a workbook", wanted("Sheet1", &[(0, 0)]));
        assert!(calc.cells.is_empty());
        assert!(calc.failure.unwrap().contains("could not read"));
    }

    #[test]
    fn a_workbook_with_too_many_formulas_is_not_calculated() {
        let bytes = workbook(|book| {
            let ws = book.add_worksheet();
            for row in 0..(MAX_FORMULAS as u32 + 1) {
                ws.write_formula(row, 0, "=1").unwrap();
            }
        });
        let calc = calculate(&bytes, wanted("Sheet1", &[(0, 0)]));
        assert!(calc.cells.is_empty());
        assert!(calc.failure.unwrap().contains("more than"));
    }

    #[test]
    fn a_long_dependency_chain_does_not_overflow_the_stack() {
        let depth = MAX_FORMULAS as u32;
        let bytes = workbook(|book| {
            let ws = book.add_worksheet();
            ws.write_number(0, 0, 1.0).unwrap();
            for row in 1..depth {
                ws.write_formula(row, 0, format!("=A{row}+1").as_str()).unwrap();
            }
        });
        let calc = calculate(&bytes, wanted("Sheet1", &[(depth - 1, 0)]));
        assert_eq!(calc.failure, None);
        assert_eq!(
            value(&calc, "Sheet1", depth - 1, 0),
            Outcome::Value(Value::Number(depth as f64))
        );
    }

    #[test]
    fn a_date_formula_comes_back_as_its_serial_number() {
        let bytes = workbook(|book| {
            let ws = book.add_worksheet();
            let date = Format::new().set_num_format("yyyy-mm-dd");
            ws.write_datetime_with_format(0, 0, ExcelDateTime::from_ymd(2026, 11, 1).unwrap(), &date).unwrap();
            ws.write_formula_with_format(1, 0, "=A1+1", &date).unwrap();
        });
        let calc = calculate(&bytes, wanted("Sheet1", &[(1, 0)]));
        // 2026-11-02 as an Excel serial; the caller turns it back into a date from the saved format.
        assert_eq!(value(&calc, "Sheet1", 1, 0), Outcome::Value(Value::Number(46328.0)));
    }
}
