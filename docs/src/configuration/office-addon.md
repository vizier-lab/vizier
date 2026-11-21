# Office addon

The **Office** addon lets an agent read and create Word (`.docx`), Excel (`.xlsx`, `.xls` for reading) and PDF (`.pdf`) files. It has no settings. It is not installed by default: add it on the agent's **Addons** (agent menu → Connections) page, and turn it off or remove it there; nothing turns it back on. Agents that existed when it first shipped were given it once, on the next start.

Files are session files: the agent reads what a person uploaded and delivers what it writes as an attachment. With the addon an agent can:

- **Read Word documents** as Markdown (headings, emphasis, nested lists, links and tables; images, shapes, text boxes and footnotes are skipped).
- **Read Excel workbooks** as typed cells per sheet (with sheet and range filters, and the formula text on request), or preview every sheet as CSV, optionally with formulas calculated (see [Calculated formulas](#calculated-formulas)).
- **Read PDFs** as Markdown, ten pages at a time by default, with a password for encrypted files and a warning when a file looks scanned.
- **Render a PDF page as an image** (scale 0.1 to 4.0, default 1.5) so the agent can look at scans, charts and diagrams.
- **Create** a new `.docx` from Markdown, a new `.xlsx` from sheets of rows (text such as `=1+1` is stored as text, never a formula), a new `.pdf` from HTML and CSS, or convert a `.docx` to `.pdf`.

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

The CSV preview with calculated formulas on (`executed_formula: true`) writes each formula cell as its calculated result; with `false` (the default) it writes the formula text, for example `=SUM(A1:A3)`. The list `formula_cells` names the cells that hold formulas (up to 200 per sheet; `note` says when there are more), so a formula and a text that starts with `=` can be told apart.

The calculation is done inside Vizier by [IronCalc](https://github.com/ironcalc/IronCalc), a pure Rust spreadsheet engine, not by Excel or LibreOffice. Nothing is installed and nothing leaves the machine. What to expect:

- **It can differ from Excel.** The result is what the engine calculates now, not the value Excel saved in the file. A function that returns the time or a random number (`NOW`, `RAND`) changes on every read.
- **A cell that cannot be calculated keeps its saved value** and is named in the result's `note`. This covers a function the engine does not know, a circular reference and a link to another file (links are never followed).
- **An error result is a result.** A formula that really returns `#DIV/0!` or `#N/A` is shown as that error and is not listed as a failure.
- **Dates stay dates.** A formula cell that the file formats as a date is written as a date.
- **Limits.** A workbook with more than 20 000 formula cells is not calculated, a calculation that takes longer than 10 seconds is given up, and at most 16 calculations exist at once. In each case, and when the engine cannot read the file, no cell is calculated: every formula cell shows its saved value and `note` gives the reason. A calculation that fails does not fail the read.
- **A file the engine refuses.** The engine needs a complete package, as Excel, LibreOffice and most libraries write. A hand-made file without `xl/styles.xml` is refused and falls back to saved values.
- **Legacy `.xls` files are never calculated.** Their formula cells show the saved value and `note` says so.

Reading typed cells never calculates, whatever the formulas option is set to: it returns the value saved in the file, and with `formulas` the formula text next to it.

## Upgrading

Agents can no longer read Word and Excel files without this addon: a skill that opened a `.docx` or `.xlsx` needs an agent that has it. Plain text and PDF are unchanged. Agents that existed when the addon first shipped were given it once.
