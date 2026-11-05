# Office test fixtures

Sample Word and Excel files that the Office addon tests read (`src/agents/tools/office/fixture_tests.rs`).
They are written by tools other than this crate's own writers, so the readers are checked against files
this crate did not produce. They are committed and read at compile time with `include_bytes!`, so a
missing file fails the build instead of skipping a test.

| File | What it holds | Made with |
|------|---------------|-----------|
| `table.docx` | heading, bold and italic text, nested bullet list, numbered list, an external link, a table | python-docx |
| `mixed.xlsx` | two sheets: text, number, boolean, date, an empty cell, a formula with a saved value, and a sheet whose data starts at `C5` | openpyxl |
| `formulas.xlsx` | formulas for `executed_formula`: SUM, IF, a cross-sheet reference, a cell with no saved result, an error cell. Complete package (styles and document properties) | hand-written OOXML |
| `formulas-minimal.xlsx` | the same sheets without `xl/styles.xml` and `docProps`; calamine reads it, the formula engine refuses it | hand-written OOXML |
| `legacy.xls` | one sheet of text and numbers (the old binary format) | xlwt |
| `gen-office-fixtures.py` | the script that writes the five files above | |

All content is made-up sample data. No office application was available when these were made, so a file
saved by Word or LibreOffice would still be a better check of `table.docx`.

## Rebuilding

```sh
pip install python-docx openpyxl xlwt
python3 fixtures/office/gen-office-fixtures.py
```

It rewrites the five sample files in this folder. Only commit the result if a sample really changed.

## PDF samples (spec 044)

Read by `src/agents/tools/office/pdf_tests.rs`. Made by `gen-pdf-fixtures.py` with reportlab and pypdf, not by this
crate's PDF writer.

| File | What it holds |
|------|---------------|
| `table.pdf` | one page: heading, paragraph, bullet list, a bordered table |
| `long.pdf` | 30 pages, each opening with a unique `PAGE-n` marker |
| `scanned.pdf` | one page that is only a raster image, no text layer |
| `locked.pdf` | encrypted, user password `vizier`, text "Locked Report" |
| `corrupt.pdf` | the first 120 bytes of a real file: a header and nothing a page can be recovered from |

```sh
pip install reportlab pypdf pillow cryptography
python3 gen-pdf-fixtures.py
```
