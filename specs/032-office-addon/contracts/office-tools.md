# Contract: Office Tools

**Feature**: `032-office-addon`

The agent-facing contract of the `office` addon. Like every tool, these are reached from `execute_python` scripts as plain functions (code mode); the inputs/outputs below are the functions' keyword arguments and returned JSON. Category: `office`. Tool names start with `office_` (enforced by `every_addon_tool_is_namespaced_by_its_addon`).

All failures are **data**: the function returns an object with `status: "error"` and a `message` the agent can act on, never an exception that ends the turn (FR-010). A successful call returns `status: "ok"` fields as listed.

Common error messages (non-exhaustive): file not found (lists nearby filenames), wrong format for the tool (names the tool to use instead), not a valid Office file (corrupt, password-protected or wrong type), unknown sheet (lists valid sheets), invalid range, an Excel format rule broken (FR-010).

## `office_docx_read`

Read a Word document from this conversation's files as Markdown.

| Input | Type | Required | Notes |
|---|---|---|---|
| `filename` | string | yes | a `.docx` session file |
| `max_chars` | integer | no | lowers the output cap; cannot exceed 200 000 |

Output:

```json
{ "status": "ok", "markdown": "# Title\n\n| a | b |\n|---|---|\n| 1 | 2 |\n", "truncated": false }
```

- Renders headings (`#`…), **bold**, *italic*, nested lists, pipe tables, `[links](url)`.
- When cut: `"truncated": true` plus `"note": "…cut at N characters of M"`.
- Skips (silently) images, shapes, text boxes, footnotes; nested tables flatten. Stated in the tool description.

## `office_docx_write`

Create a **new** Word document from Markdown. Never edits an existing document: editing in place would silently lose its styles, headers, images and tracked changes — the description says so.

| Input | Type | Required | Notes |
|---|---|---|---|
| `filename` | string | yes | `.docx` appended if missing; basename only |
| `markdown` | string | yes | headings, emphasis, lists (nested), GFM tables, links, code spans, rules |
| `title` | string | no | becomes a Title paragraph and the document title |

Output:

```json
{ "status": "ok", "filename": "report.docx", "size": 8123 }
```

- Stored with media type `application/vnd.openxmlformats-officedocument.wordprocessingml.document`.
- If a file named `filename` already exists in the conversation, it is **replaced** (same as `image_generate`, `tts_generate` and `load_path_as_session_file`). The description says so and tells the agent to choose a new name when producing a version of an existing file.
- The description reminds the agent to call `send_attachment` with the returned filename to deliver it.

## `office_xlsx_read`

Read a workbook (`.xlsx`, or legacy `.xls`) as sheets of typed cells.

| Input | Type | Required | Notes |
|---|---|---|---|
| `filename` | string | yes | `.xlsx` or `.xls` session file |
| `sheet` | string | no | one sheet by name; default all |
| `range` | string | no | A1 notation, e.g. `B2:F40`; applied before truncation |

Output:

```json
{
  "status": "ok",
  "sheets": [
    {
      "name": "Budget",
      "dimensions": { "rows": 3, "cols": 3 },
      "rows": [
        ["Item", "Qty", "Due"],
        ["Pens", 12, { "date": "2026-11-01" }],
        ["Notes", null, true]
      ]
    }
  ],
  "truncated": false
}
```

- Cell forms: text `"…"`, number, boolean, date `{"date": ISO}`, blank `null`, error `{"error": "#DIV/0!"}`.
- Formulas return their last saved value. Charts/pivots/images are skipped.
- Caps: 20 sheets, 1 000 rows, 100 columns, ≈200 KB; when exceeded `truncated: true` and `dimensions` still shows the real size.

## `office_xlsx_write`

Create a **new** workbook from named sheets. Never edits an existing workbook (same reason as DOCX).

| Input | Type | Required | Notes |
|---|---|---|---|
| `filename` | string | yes | `.xlsx` appended if missing |
| `sheets` | array | yes | `[{ "name": string, "rows": Cell[][] }]` |

Constraints: Excel's own rules apply (sheet names 1–31 chars without `[]:*?/\`, unique case-insensitively; cell text ≤ 32 767 chars); a violation comes back as an ordinary error. No other size limits.

Output:

```json
{ "status": "ok", "filename": "budget.xlsx", "size": 6340 }
```

- First row of each sheet is written bold and frozen; column widths fit content (bounded).
- **Every string is stored as text.** A value like `"=SUM(A1:A9)"` stays the literal text; formulas are never produced (FR-028).
- Numbers → numbers, booleans → booleans, `{"date": …}` → real dates, `null` → blank.
- Stored with media type `application/vnd.openxmlformats-officedocument.spreadsheetml.sheet`; same existing-file rule as `office_docx_write`.

## `read_document_file` (existing tool, changed)

Unchanged signature `{ filename }`. Changes (FR-012/013/014):

- **Word and Excel are no longer handled.** For `.docx`, `.xlsx` and `.xls` it returns an error-style message: "Word and Excel files are read with `office_docx_read` / `office_xlsx_read` (Office addon; install it from the Addons page if this agent does not have it)". No plain-text conversion is produced.
- **Everything else is unchanged:** plain text formats and PDF read exactly as before.
- The description says: Word and Excel files are read with the `office_*` tools, which also create them. `read_image_file`'s description and the attached-files hint in the request prompt get the same pointer.

## Addon catalogue entry (`GET /integrations/kinds`)

```json
{ "id": "office", "family": "addon", "name": "Office",
  "description": "Read and create Word (.docx) and Excel (.xlsx) files, locally",
  "icon": "office", "fields": [] }
```

Installing it on an agent that has it already → `409 addon_installed` (existing rule).
