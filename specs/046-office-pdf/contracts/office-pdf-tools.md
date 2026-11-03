# Contract: Office PDF Tools

**Feature**: `046-office-pdf`. Extends `specs/036-office-addon/contracts/office-tools.md`; the common rules there apply (tools are functions in `execute_python`; a failure is a catchable error with a readable message; never ends the turn).

## `office_pdf_read_as_md`

| Input | Type | Required | Notes |
|---|---|---|---|
| `filename` | string | yes | a `.pdf` session file |
| `page_start` | integer | no | 1-indexed, default 1 |
| `page_limit` | integer | no | default 10 |
| `max_chars` | integer | no | lowers the cap; cannot exceed 200 000 |
| `password` | string | no | for an encrypted file; not stored |

Output: `PdfRead` (data-model.md). When pages remain: `truncated: true` and `next_page_start`. Scanned: `scanned_likely: true` and a `note` naming `office_pdf_render_page_as_png`. Skips images and drawings; says so in the description.

Errors: file not found (lists the files that exist); wrong format (names the right tool); corrupt; password needed or wrong; `page_start` beyond the last page (states the page count).

## `office_pdf_render_page_as_png`

| Input | Type | Required | Notes |
|---|---|---|---|
| `filename` | string | yes | a `.pdf` session file |
| `page` | integer | no | 1-indexed, default 1 |
| `scale` | number | no | 0.1 to 4.0, default 1.5 (72 DPI × scale); out-of-range values are clamped and the used value is returned |
| `output_filename` | string | no | default `<name>_page_<n>.png`; `.png` added if missing |

Output: `{ status, filename, size, width, height, page, scale }`. The PNG is a session file with media type `image/png`, openable with `read_image_file`.

Errors: page 0 or past the end (states the page count); a render larger than 25 000 000 pixels (states the pixels and suggests a smaller scale); corrupt or encrypted file.

## `office_pdf_write_from_html`

| Input | Type | Required | Notes |
|---|---|---|---|
| `filename` | string | yes | `.pdf` added if missing; basename only |
| `html` | string | yes | HTML with inline `<style>`; ≤ 5 MB |
| `title` | string | no | document title |

Output: `{ status, filename, size }`. Replaces a same-named file. No remote or `file://` resource is loaded and no script runs; images by URL or by session file name are not available in this version. The description says all of this and that it never edits an existing PDF.

## `office_pdf_write_from_docx`

| Input | Type | Required | Notes |
|---|---|---|---|
| `filename` | string | yes | the new PDF; `.pdf` added if missing |
| `docx_filename` | string | yes | a `.docx` session file; left unchanged |
| `title` | string | no | document title; also shown as a heading |

Output: `{ status, filename, size }`. Carries headings, bold/italic, lists, tables and links. Does not carry images, headers/footers, page numbers or tracked changes (stated in the description). A non-Word source names the right reader tool.

## Pinned tables

The four names appear in `every_tool_is_in_its_category` (category `office`, `category()` returns `None`) and in the uncategorised list in `agents/tools/mod.rs`. Not in `DREAM_TOOL_NAMES`.
