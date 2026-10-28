# Office addon

The **Office** addon lets an agent read and create Word (`.docx`) and Excel (`.xlsx`, `.xls` for reading) files. It has no settings and is on by default: every new agent gets it, and agents that existed when it shipped were given it once, on first start. Turn it off or remove it from the agent's **Integrations › Addons** page; nothing turns it back on.

Files are session files: the agent reads what a person uploaded and delivers what it writes with `send_attachment`.

| Tool | Use it to |
|------|-----------|
| `office_docx_read_as_md` | Read a Word document as Markdown: headings, bold and italic, nested lists, links and tables as pipe tables. Images, shapes, text boxes and footnotes are skipped. |
| `office_xlsx_read_as_cells` | Read typed cells (text, number, boolean, date, empty, error) per sheet, with `sheet` and `range` filters, `info` for sheet sizes only, and `formulas` for formula text. It never calculates. |
| `office_xlsx_read_as_csv` | Preview every sheet of a workbook as CSV in one call. Formulas show as text, or as calculated results with `executed_formula` (not available yet: the call says so). |
| `office_docx_write_from_md` | Create a new `.docx` from Markdown, with an optional title. |
| `office_xlsx_write_from_sheets` | Create a new `.xlsx` from sheets of rows. Text such as `=1+1` is stored as text, never as a formula. |

Reads are capped (20 sheets, 1 000 rows and 100 columns per sheet, 200 000 characters of Markdown); a capped result says `truncated` and reports the real size.

The write tools only generate **new** files; they never edit an existing one. A file with the same name is replaced, so give a new version a new name.

`read_document_file` no longer reads Word and Excel files: it points to these tools.
