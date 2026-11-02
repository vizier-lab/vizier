# Office addon

The **Office** addon lets an agent read and create Word (`.docx`), Excel (`.xlsx`, `.xls` for reading) and PDF (`.pdf`) files. It has no settings and is on by default: every new agent gets it, and agents that existed when it shipped were given it once, on first start. Turn it off or remove it from the agent's **Integrations › Addons** page; nothing turns it back on.

Files are session files: the agent reads what a person uploaded and delivers what it writes with `send_attachment`.

| Tool | Use it to |
|------|-----------|
| `office_docx_read_as_md` | Read a Word document as Markdown: headings, bold and italic, nested lists, links and tables as pipe tables. Images, shapes, text boxes and footnotes are skipped. |
| `office_xlsx_read_as_cells` | Read typed cells (text, number, boolean, date, empty, error) per sheet, with `sheet` and `range` filters, `info` for sheet sizes only, and `formulas` for formula text. It never calculates. |
| `office_xlsx_read_as_csv` | Preview every sheet of a workbook as CSV in one call. Formulas show as text, or as calculated results with `executed_formula`. The calculation is done by the addon, not by Excel, so it can differ from what Excel shows; a cell it cannot calculate (an unknown function, a circular reference, a link to another file) keeps the value saved in the file and is named in `note`. Legacy `.xls` files are never calculated. |
| `office_docx_write_from_md` | Create a new `.docx` from Markdown, with an optional title. |
| `office_xlsx_write_from_sheets` | Create a new `.xlsx` from sheets of rows. Text such as `=1+1` is stored as text, never as a formula. |
| `office_pdf_read_as_md` | Read a PDF as Markdown (headings, paragraphs, lists, tables). Reads `page_limit` pages (default 10) from `page_start`; the result gives `total_pages`, the range returned and, when pages remain, `next_page_start`. Add `password` for an encrypted file. `scanned_likely` flags a document with no text layer, and `failed_pages` lists pages that could not be parsed. |
| `office_pdf_render_page_as_png` | Render one page (counted from 1) to a PNG file in the conversation, to look at with `read_image_file`: scans, charts, diagrams. `scale` is 0.1 to 4.0 (default 1.5; values outside are held to the range and the result reports the one used). |
| `office_pdf_write_from_html` | Create a new `.pdf` from HTML and CSS (put the styles in a `<style>` element). Nothing outside the HTML is loaded: no images or stylesheets by URL or file, and no scripts. |
| `office_pdf_write_from_docx` | Convert a `.docx` in the conversation to a new `.pdf`. Headings, emphasis, lists, tables and links are carried over from the document text; images, headers and footers, page numbers and tracked changes are not. The Word file is left unchanged. |

Reads are capped (20 sheets, 1 000 rows and 100 columns per sheet, 200 000 characters of Markdown); a capped result says `truncated` and reports the real size. A PDF read is also limited to 10 pages by default and 100 000 characters unless `max_chars` (at most 200 000) says otherwise.

The write tools only generate **new** files; they never edit an existing one. A file with the same name is replaced, so give a new version a new name.

## PDF limits

The PDF tools run entirely inside Vizier (pure Rust; no PDF viewer, browser or other program is installed or called) and never fetch anything.

- A PDF larger than 256 MB, a page that would render above 25 million pixels (lower `scale`) and HTML above 5 MB are refused with a message saying so.
- A PDF read returns text, not pictures: images and drawings are skipped, and a document with no text layer is reported as `scanned_likely`. There is no OCR; render the page and look at it.
- Written PDFs have no images: HTML cannot load them from a URL or from the conversation's files, and scripts do not run. Text in a script the renderer has no font for may be missing from the output.
- A Word-to-PDF conversion lays the document's text out afresh. It does not reproduce Word's own layout, and images, headers and footers, page numbers and tracked changes are left out.
- No tool edits, merges, splits, annotates or fills in an existing PDF.

## Calculated formulas

`office_xlsx_read_as_csv` with `executed_formula: true` writes each formula cell as its calculated result; with `false` (the default) it writes the formula text, for example `=SUM(A1:A3)`. The list `formula_cells` names the cells that hold formulas (up to 200 per sheet; `note` says when there are more), so a formula and a text that starts with `=` can be told apart.

The calculation is done inside Vizier by [IronCalc](https://github.com/ironcalc/IronCalc), a pure Rust spreadsheet engine, not by Excel or LibreOffice. Nothing is installed and nothing leaves the machine. What to expect:

- **It can differ from Excel.** The result is what the engine calculates now, not the value Excel saved in the file. A function that returns the time or a random number (`NOW`, `RAND`) changes on every read.
- **A cell that cannot be calculated keeps its saved value** and is named in the result's `note`. This covers a function the engine does not know, a circular reference and a link to another file (links are never followed).
- **An error result is a result.** A formula that really returns `#DIV/0!` or `#N/A` is shown as that error and is not listed as a failure.
- **Dates stay dates.** A formula cell that the file formats as a date is written as a date.
- **Limits.** A workbook with more than 20 000 formula cells is not calculated, a calculation that takes longer than 10 seconds is given up, and at most 16 calculations exist at once. In each case, and when the engine cannot read the file, no cell is calculated: every formula cell shows its saved value and `note` gives the reason. A calculation that fails does not fail the read.
- **A file the engine refuses.** The engine needs a complete package, as Excel, LibreOffice and most libraries write. A hand-made file without `xl/styles.xml` is refused and falls back to saved values.
- **Legacy `.xls` files are never calculated.** Their formula cells show the saved value and `note` says so.

`office_xlsx_read_as_cells` never calculates, whatever `formulas` is set to: it returns the value saved in the file, and with `formulas` the formula text next to it.

## Upgrading

`read_document_file` no longer reads Word and Excel files: it replies with a message that points to the tools above, so a skill that called it for a `.docx` or `.xlsx` file now needs an agent with this addon. Plain text and PDF are unchanged. Every existing agent is given the addon once on its first start after the upgrade; an agent from which the addon is removed later does not get it back.

## For contributors: test fixtures

The tests read sample Word and Excel files written by other tools (python-docx, openpyxl, xlwt and hand-written OOXML), so the readers are checked against files this crate did not produce. They live in `fixtures/office/` at the repository root, are committed, and are compiled into the tests with `include_bytes!`, so a missing file fails the build instead of skipping a test. `fixtures/office/README.md` says what each file holds. To rebuild them:

```sh
pip install python-docx openpyxl xlwt
python3 fixtures/office/gen-office-fixtures.py
```

Commit the result only if a sample really changed. Run the Office tests with `cargo test office`.
