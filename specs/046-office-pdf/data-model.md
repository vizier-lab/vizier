# Data Model: Office Addon Phase 2 (PDF)

No table, column, migration, marker or integration row is added or changed. The addon (`office`) keeps no settings.

## Conversation file (existing)

A session file: name, media type, bytes, in the `session_file` table with the blob under `uploads/<file_id>/`. New media types written by this feature: `application/pdf` (`PDF_MIME`) and `image/png` (`PNG_MIME`), set explicitly (FR-012).

## PdfRead (tool output, new)

| Field | Type | Meaning |
|---|---|---|
| `markdown` | string | the extracted slice |
| `total_pages` | integer | pages in the document |
| `page_start`, `page_end` | integer | 1-indexed slice returned |
| `next_page_start` | integer, optional | continue here when pages remain |
| `truncated` | boolean | cut by the page limit or the character budget |
| `failed_pages` | integer[], omitted when empty | pages that could not be parsed |
| `scanned_likely` | boolean | no selectable text layer |
| `note` | string, optional | what to do next |

## PdfPageRendered (tool output, new)

`status` ("ok"), `filename`, `size` (bytes), `width`, `height` (pixels), `page`. The agent also needs the scale actually used (G9): add `scale`.

## FileWritten (existing)

`status`, `filename`, `size`: returned by both write tools, the same shape the Word and Excel writers return.

## State transitions

None. Every call is stateless: read inputs from a session file, write at most one session file, replacing a same-named one.
