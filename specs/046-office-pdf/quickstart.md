# Quickstart: Office Addon Phase 2 (PDF)

End-to-end check on a running binary with an agent on the offline `dummyplug` provider (constitution e2e gate). Setup (start dummyplug with `just dummyplug`, run vizier, obtain `$TOKEN`, create the agents) is exactly that of `specs/036-office-addon/quickstart.md`; it is not repeated here. Agent tools are called from `execute_python`. Every step below is deterministic and needs no model key.

Use an agent `dp-a` created after the upgrade (addon present) and `dp-old` created before it (addon present, one integration row, no second row after restart).

## 1. The tools are there, nothing new was seeded

- Send `tools` to `dp-a`: expect `office_pdf_read_as_md`, `office_pdf_render_page_as_png`, `office_pdf_write_from_html`, `office_pdf_write_from_docx` under category `office`.
- `GET /agents/dp-old/integrations`: exactly one `office` row, unchanged `id`; restart vizier and repeat: still one row (FR-017, SC-008).
- Turn the addon off for `dp-a`, send `tools`: no `office_*` tool at all; turn it on: all return.

## 2. Write from HTML, then read it back

1. Send `office_pdf_write_from_html`, then `{"filename":"invoice","html":"<h1>Invoice 42</h1><table><tr><th>Item</th><th>Qty</th></tr><tr><td>Widget</td><td>3</td></tr></table>","title":"Invoice"}`.
   Expect `{status:"ok", filename:"invoice.pdf", size>0}`.
2. Send `office_pdf_read_as_md`, then `{"filename":"invoice.pdf"}`.
   Expect `total_pages:1`, markdown containing `Invoice 42`, `Widget` and `3`, `truncated:false`, `scanned_likely:false`.
3. Send `send_attachment` with `invoice.pdf`: expect delivery (FR-012).

## 3. Render a page and look at it

1. `office_pdf_render_page_as_png` with `{"filename":"invoice.pdf","page":1,"scale":1}`: expect `filename:"invoice_page_1.png"`, `width`, `height` > 0, `scale:1`.
2. `read_image_file` with that name: the image is accepted.
3. `{"filename":"invoice.pdf","page":2}` : readable error stating the document has 1 page.
4. `{"filename":"invoice.pdf","scale":50}`: succeeds with the clamped `scale:4.0`, or the pixel-limit error; either is readable and ends no turn.

## 4. Word to PDF

1. `office_docx_write_from_md` with `{"filename":"report","markdown":"# Report\n\n- a\n- b\n\n| x | y |\n|---|---|\n| 1 | 2 |\n"}`.
2. `office_pdf_write_from_docx` with `{"filename":"report","docx_filename":"report.docx","title":"Report"}`: `report.pdf` is written, `report.docx` is unchanged (same size).
3. `office_pdf_read_as_md` on `report.pdf`: contains `Report`, `a`, `b`, and the cells `1` and `2`.
4. `office_pdf_write_from_docx` with `docx_filename:"invoice.pdf"`: error names `office_pdf_read_as_md`. With an `.xlsx`: error names the Excel reader.

## 5. Wrong tool in every direction (SC-007)

`office_pdf_read_as_md` on a `.docx` and on an `.xlsx`; `office_docx_read_as_md` and `office_xlsx_read_as_cells` on the `.pdf`: each error names the correct reader.

## 6. Paging and truncation

Upload `crates/vizier-core/tests/fixtures/office/long.pdf` (30 pages). `office_pdf_read_as_md` with no range: `page_end:10`, `truncated:true`, `next_page_start:11`. Follow `next_page_start` until none remains; the concatenated pages cover the document (SC-004). With `max_chars:500`: `truncated:true`.

## 7. Scanned, encrypted, corrupt

- `crates/vizier-core/tests/fixtures/office/scanned.pdf`: `scanned_likely:true`, note names the render tool.
- `crates/vizier-core/tests/fixtures/office/locked.pdf`: no password → readable error; wrong → readable error; `password:"vizier"` → content.
- `crates/vizier-core/tests/fixtures/office/corrupt.pdf`: readable "not a valid PDF" error; the next call in the same script still runs.

## 8. HTML does not reach out (SC-009)

`office_pdf_write_from_html` with `<img src="http://127.0.0.1:9/x.png"><script>fetch('http://127.0.0.1:9/')</script><link rel="stylesheet" href="file:///etc/hostname">`. Expect a PDF; with `ss -tn` or a listener on port 9, no connection arrives; the PDF text has no hostname.

## 9. Existing behaviour unchanged (FR-020)

`read_document_file` on a PDF returns what it did before this branch (compare with the 036 quickstart's output for the same file).

Live-provider steps: none. Nothing here depends on real model output.
