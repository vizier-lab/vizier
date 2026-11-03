# Research: Office Addon Phase 2 (PDF)

## R1 — Which library, and what does it cost

**Decision**: `pdf-agent` 0.1.0 (requested by the owner), as an optional dependency of `vizier-core` behind the existing `documents` feature.

**Rationale**: it is one pure-Rust facade over `pdf_oxide` (structured Markdown with pages and passwords), `hayro` (rasteriser) and `fulgur` (HTML/CSS to PDF), with panic guards (`sandbox::guarded`) and input caps. Nothing already in the tree can rasterise a PDF or produce one from HTML; `pdf-extract` stays for the generic reader's plain text.

**Cost found**: `Cargo.lock` gains 189 packages (about 2 300 lines). Principle I and the dependency-weight rule require this to be measured: record the release-binary size before and after, and the clean-build time delta, in the PR (T002). `app/Cargo.lock` must carry the same versions (CI `policy`).

**Alternatives considered**: `pdfium-render` (needs the native pdfium library, breaks Principle III and the static builds); `lopdf`/`printpdf` (low-level, no HTML layout, no rasteriser); shelling out to `wkhtmltopdf`/`pdftoppm` (external binaries, breaks Principle III); headless Chromium (same).

## R2 — Does HTML-to-PDF touch the network or run scripts

**Finding**: `fulgur`'s network provider is *offline-first*: only `file://` URLs inside a configured base directory are loaded; `http://`, `https://` and `data:` are silently dropped. The crate has no script engine. `pdf-agent` calls `fulgur::convert_html(html)` with no base directory configured.

**Decision**: rely on this, and prove it with a test (G3) instead of trusting the crate's comments: remote image, stylesheet link, `file:///etc/hostname` and `<script>` in one HTML input; the PDF is produced, contains none of them, and the test makes no socket connection.

**Consequence for authors**: an agent cannot embed an image by URL or from a session file in this version. Tool description says so; an inline `data:` image is also dropped (finding above), so images in written PDFs are out of scope.

## R3 — Read defaults and limits

**Finding**: `PdfReadOptions` has `page_start`, `page_limit` (library default 10), `max_chars`, `password`. The tool clamps `max_chars` to the addon-wide `READ_MAX_CHARS` (200 000) but passes nothing when it is omitted.

**Decision**: document 10 pages and the library's own character default as the defaults; add a test that an omitted `max_chars` on a large PDF still returns `truncated: true` with `next_page_start`. If the library has no default budget, pass 100 000 explicitly (matching the schema text). See G2.

**Inherited input limits** (spec 036 imposed none; these are the library's and cannot be removed without forking): PDF ≤ 256 MB, rendered page ≤ 25 000 000 pixels (the error states the scale and size), HTML ≤ 5 MB. They are documented limits, reported as readable errors, not new validation by us.

## R4 — Word to PDF path

**Decision**: `docx::to_markdown` (the 036 reader, uncapped) → `pulldown-cmark` (tables, strikethrough) → an HTML page with an inline stylesheet → `from_html`. One writer, so a fix to HTML rendering fixes both tools.

**Rationale**: no second layout engine; reuses code already linted and tested. Fidelity is limited to what the Word reader exposes (headings, emphasis, lists, tables, links); images, headers/footers, page numbers and tracked changes are not carried over, and the spec says so.

**Risk found (G4)**: raw HTML inside the Markdown becomes markup. Fix by escaping `Html`/`InlineHtml` events before rendering.

**Alternatives considered**: convert via LibreOffice (external binary); lay out from `docx-rs` directly (large, a second renderer to maintain).

## R5 — Errors, wrong-format messages, blocking work

**Decision**: reuse `load`, `format_of`, `wrong_tool`, `write_session_file` unchanged; `Format::Pdf` added so every pair among Word, Excel, PDF names the right tool (FR-015). Library errors are wrapped once in `pdf.rs` with a short prefix; a corrupt file and a wrong password must read differently to the agent, so `pdf.rs` keeps the library's message text rather than replacing it.

CPU-bound library calls move to `spawn_blocking` unless the existing office tools do otherwise (G5). A panic inside the library is already converted to an error by `sandbox::guarded`.

## R6 — Platform evidence still owed

Pure Rust, so no `Cross.toml` `pre-build` change is expected. Evidence needed before merge: `cargo build --release` for `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-gnu` (or the CI jobs green), and the desktop app's Windows compile (CI `app` job). Fonts: `fulgur` falls back to a font stack that names DejaVu Sans; on a host with no fonts PDFs may render without glyphs. Verify in the Docker image, and add a font to the image only if the test shows empty text (decision deferred to T033, not assumed).
