---

description: "Task list for the Office addon Phase 2 (PDF)"
---

# Tasks: Office Addon Phase 2 (PDF)

**Input**: Design documents from `/specs/044-office-pdf/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/office-pdf-tools.md, quickstart.md

**Tests**: Included. The spec requires them (FR-023 per-operation tests, FR-024 dummyplug end-to-end, FR-019 pinned tool tables) and the constitution requires `cargo clippy`, `cargo test` and a dummyplug run.

**State on the branch**: the tool code was written before this task list (commit `4b0158e`). Tasks for work that already exists are marked `[X]` so the list is a true account; each unchecked task is a gap from plan.md (G1 to G9) or an owed piece of evidence.

**Organization**: grouped by user story. Stories 1 to 4 each own one tool and are independent of each other; Story 5 is rollout and cross-tool behaviour.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: US1 to US5, from spec.md
- Every task names the file it changes

## Path Conventions

All code is in `crates/vizier-core/src/agents/tools/office/` (`pdf.rs` pure functions, `mod.rs` the tool structs and the module's tests), `crates/vizier-core/src/integrations/kinds/office.rs`, `crates/vizier-core/src/agents/tools/mod.rs` (pinned tables). Fixtures live in `crates/vizier-core/tests/fixtures/office/` and are compiled into tests by `office/fixture_tests.rs` (a missing file fails the build). Tests run with `cargo test -p vizier-core`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: the dependency, its cost, and the fixtures every story's tests use.

- [X] T001 Add `pdf-agent = "0.1.0"` to `Cargo.toml` and `crates/vizier-core/Cargo.toml` behind the `documents` feature (`dep:pdf-agent`); `Cargo.lock` updated
- [ ] T002 Measure and record the dependency cost required by constitution Principle I: release-binary size and clean-build time with and without `pdf-agent` (`cargo build --release -p vizier`), in the PR description and in `specs/044-office-pdf/research.md` R1
- [X] T003 [P] Create the PDF fixtures in `crates/vizier-core/tests/fixtures/office/`: `table.pdf` (heading, list, a table, 1 page), `long.pdf` (30 pages, each starting with a unique `PAGE-n` marker), `scanned.pdf` (image-only page, no text layer), `locked.pdf` (encrypted, user password `vizier`), `corrupt.pdf` (valid header, truncated body). Record in a `README` line how each was made (the generator is committed or described, as for the Word and Excel fixtures)
- [X] T004 [P] Register the new fixtures with `include_bytes!` accessors in `crates/vizier-core/src/agents/tools/office/fixture_tests.rs` (same macro as the existing ones)
- [X] T005 Confirm `app/Cargo.lock` carries the same `pdf-agent` and transitive versions as the root lockfile (`cd app && cargo tree -i pdf-agent`), updating it if CI `policy` would fail *(`cargo update -p image --precise 0.25.6` in `app/`: `pdf-agent` pins `image =0.25.6`, as the root lockfile already does; only the pinned crates went down, nothing unrelated moved. The app itself was not compiled here: it needs webkit.)*

**Checkpoint**: crate builds with the dependency; fixtures exist; cost recorded.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: shared pieces every tool uses. Nothing below may start until these are in place.

- [X] T006 Add `PDF_MIME` and `PNG_MIME` constants in `crates/vizier-core/src/agents/tools/session_files/mod.rs`
- [X] T007 Add `Format::Pdf` to `format_of` and the PDF arm of `wrong_tool` in `crates/vizier-core/src/agents/tools/office/mod.rs`
- [X] T008 Create `crates/vizier-core/src/agents/tools/office/pdf.rs` with the pure functions `to_markdown`, `render_page`, `from_html`, `from_docx` and the `PdfRead` output type; declare `pub mod pdf;`
- [X] T009 Register the four tools in `OfficeKind::contribute` and update the catalogue description in `crates/vizier-core/src/integrations/kinds/office.rs`
- [X] T010 Add the four tools to the pinned `every_tool_is_in_its_category` table and the uncategorised list in `crates/vizier-core/src/agents/tools/mod.rs`
- [X] T011 Run the four CPU-bound calls (`to_markdown`, `render_page`, `from_html`, `from_docx`) through `tokio::task::spawn_blocking` in the tool `call` methods in `crates/vizier-core/src/agents/tools/office/mod.rs`, following the pattern already used at the formula calculation (`mod.rs` near line 371); a panicked or cancelled blocking task becomes a readable `VizierError` (plan G5, research R5)

**Checkpoint**: foundation ready; stories can proceed in parallel.

---

## Phase 3: User Story 1 - Agent reads a PDF as structured text (Priority: P1) 🎯 MVP

**Goal**: read a PDF as Markdown with paging, truncation, password, scanned and failed-page handling.

**Independent Test**: read `table.pdf` and `long.pdf` through the tool; every heading and table cell is present; following `next_page_start` covers the whole document.

### Tests for User Story 1

- [X] T012 [P] [US1] Test in `crates/vizier-core/src/agents/tools/office/pdf.rs` that `table.pdf` reads with its heading, list items and every table cell, `total_pages` and the page range correct (SC-001)
- [X] T013 [P] [US1] Test in `pdf.rs` that `long.pdf` read with no range returns pages 1 to 10, `truncated: true`, `next_page_start: 11`, and that following `next_page_start` until it is `None` yields every `PAGE-n` marker exactly once (SC-004)
- [X] T014 [P] [US1] Test in `pdf.rs` that an omitted `max_chars` on `long.pdf` is still bounded, and that `max_chars` of 500 gives `truncated: true` (plan G2)
- [X] T015 [P] [US1] Test in `pdf.rs` for `locked.pdf`: no password and a wrong password give distinct readable errors (not "corrupt"); `vizier` reads it (spec US1 scenario 4)
- [X] T016 [P] [US1] Test in `pdf.rs` that `scanned.pdf` sets `scanned_likely` and the note names `office_pdf_render_page_as_png`; that `corrupt.pdf` returns a readable "not a valid PDF" error; that a PDF with an unparseable page lists it in `failed_pages` while returning the others (spec US1 scenarios 5 and 6; if no fixture can produce a failed page, assert the field passes through and document the gap) *(no fixture can make a single unparseable page, so `failed_pages` is passed through by a plain read, not exercised on a bad page)*
- [X] T017 [P] [US1] Test in `pdf.rs` that `page_start` beyond the last page returns a readable result or error that states the page count (plan G8, SC-005)

### Implementation for User Story 1

- [X] T018 [US1] In `crates/vizier-core/src/agents/tools/office/mod.rs` resolve the `max_chars` default (plan G2): either pass the documented 100 000 to `pdf_agent` when the argument is absent, or correct the schema text to the library's real default; make T014 pass
- [X] T019 [US1] In `pdf.rs` map the library's password-required and wrong-password failures to messages that say which one it was, keeping the library's text otherwise (research R5); make T015 pass
- [X] T020 [US1] In `pdf.rs`, when `page_start` is past the end, return an error or `note` that states `total_pages`; make T017 pass (plan G8)
- [X] T021 [US1] Rewrite the `office_pdf_read_as_md` description in `mod.rs` per contracts/office-pdf-tools.md: defaults (page 1, 10 pages), the character cap, password, the continuation field, what is skipped (images, drawings), and that it never edits a PDF (plan G1, FR-011)

**Checkpoint**: US1 works alone; this is the MVP.

---

## Phase 4: User Story 2 - Agent looks at a PDF page as an image (Priority: P1)

**Goal**: render one page to a PNG session file the image reader accepts.

**Independent Test**: render page 2 of `long.pdf`, read the PNG with the image reader's loader; page 0 and page 999 give errors that state the page count.

### Tests for User Story 2

- [X] T022 [P] [US2] Test in `pdf.rs` that every page of `long.pdf` (loop 1..=30) renders to a PNG with the PNG signature and non-zero size, and that the dimensions grow with `scale` (SC-005)
- [X] T023 [P] [US2] Test in `pdf.rs` that page 0 and page past the end return an error containing the page count; that `corrupt.pdf` and `locked.pdf` (no password) return readable errors; that scale 50 is clamped to 4.0 and a scale that exceeds the pixel limit returns the library's size error without panicking (plan G9)

### Implementation for User Story 2

- [X] T024 [US2] In `pdf.rs` check the page against the page count before rendering and return an error that states it (the library's error may not); make T023 pass
- [X] T025 [US2] Add `scale: f32` (the value actually used after clamping) to `PdfPageRendered` in `mod.rs` and to data-model.md/contract if the shape changes (plan G9); update the round-trip test in `mod.rs`
- [X] T026 [US2] Make `output_filename` get `.png` appended when missing and refuse folders, by routing it through the same name rule `write_session_file` already applies; confirm with a test in `mod.rs` (FR-013)
- [X] T027 [US2] Rewrite the `office_pdf_render_page_as_png` description in `mod.rs`: page numbering, the scale range 0.1 to 4.0 with its default and clamp, `output_filename`, that the PNG opens with `read_image_file`, and the pixel limit (plan G1)

**Checkpoint**: US1 and US2 both work independently.

---

## Phase 5: User Story 3 - Agent creates a designed PDF from HTML (Priority: P1)

**Goal**: write a `.pdf` from HTML with styling, offline.

**Independent Test**: write an invoice with a bordered table and a coloured row; the PDF reads back with its text and its table cells, and opens in a viewer.

### Tests for User Story 3

- [X] T028 [P] [US3] Test in `pdf.rs` that styled HTML with a table round-trips: the heading and every cell are in `to_markdown` of the result, and the bytes start with `%PDF` and end with `%%EOF` (SC-002, FR-016)
- [X] T029 [P] [US3] Test in `pdf.rs` for plan G3 / SC-009: HTML containing a remote `<img>`, a remote stylesheet `<link>`, `<link href="file:///etc/hostname">` and a `<script>` produces a PDF; bind a local `TcpListener` on `127.0.0.1:0`, point the remote references at it, and assert it accepts no connection; the PDF text contains neither the script body nor the hostname
- [X] T030 [P] [US3] Test in `mod.rs` that the write tool adds `.pdf` when missing, refuses `../x.pdf` and `a/b.pdf`, and replaces a same-named session file (FR-013); that empty HTML and HTML over 5 MB give readable errors (FR-014)
- [X] T031 [P] [US3] Test in `pdf.rs` that non-Latin text and an emoji in the HTML either appear in the read-back text or come back with a `note`/error, never silently blank (edge case; record the observed behaviour in the tool description)

### Implementation for User Story 3

- [X] T032 [US3] Rewrite the `office_pdf_write_from_html` description in `mod.rs`: HTML with inline `<style>`, no remote or file resources and no scripts, images not available in this version, 5 MB limit, `.pdf` added, same-name replacement and "use a new filename to keep the original", never edits an existing PDF, deliver with `send_attachment` (plan G1; FR-009, FR-011, FR-013)
- [ ] T033 [US3] Verify fonts (research R6): run T028 inside the project's Docker image (`just docker` build) and confirm the text is present; if the text is blank, add a font to `Dockerfile` and `Cross.toml` `pre-build` in the same change and record the decision in research.md R6

**Checkpoint**: US1 to US3 work independently.

---

## Phase 6: User Story 4 - Agent turns a Word document into a PDF (Priority: P2)

**Goal**: convert a `.docx` session file to a `.pdf`, leaving the source unchanged.

**Independent Test**: write a Word document with a heading, a list, a table and a link, convert it, read the PDF back: all present.

### Tests for User Story 4

- [X] T034 [P] [US4] Test in `pdf.rs` using the existing Word fixture `table.docx` (via `fixture_tests.rs`) that the converted PDF reads back with the heading, bold text, list items and every table cell, and the link text (SC-002)
- [X] T035 [P] [US4] Test in `pdf.rs` for plan G4: a Word paragraph whose text is `<b>not bold</b> a < b <script>x</script>` appears literally in the PDF's text and is not interpreted as markup
- [X] T036 [P] [US4] Test in `mod.rs` that the source `.docx` session file is byte-identical after conversion, that `title` appears in the output, and that a non-Word source (`.xlsx`, `.pdf`, `.txt`) is refused with the right reader named (FR-010, FR-015)

### Implementation for User Story 4

- [X] T037 [US4] In `from_docx` in `pdf.rs`, turn `pulldown_cmark` `Html` and `InlineHtml` events into escaped text before `push_html` (plan G4); make T035 pass
- [X] T038 [US4] Rewrite the `office_pdf_write_from_docx` description in `mod.rs`: what is preserved (headings, emphasis, lists, tables, links), what is not (images, headers and footers, page numbers, tracked changes), the source is unchanged, same-name replacement and the new-name advice (plan G1, FR-022)

**Checkpoint**: all four tools work independently.

---

## Phase 7: User Story 5 - The PDF tools arrive with the addon, with no setup (Priority: P2)

**Goal**: rollout behaves as spec 036 decided: no new row, no backfill, off stays off, every wrong-format message names the right tool.

**Independent Test**: the quickstart sections 1 and 5 pass against a running binary.

### Tests for User Story 5

- [X] T039 [P] [US5] Test in `mod.rs` the full wrong-format matrix (FR-015, SC-007): each of the 7 office tools that take a file, given each of Word, Excel, PDF and a text file where it does not apply, returns an error naming the correct reader; assert on the tool names, not the whole sentence
- [X] T040 [P] [US5] Test in `crates/vizier-core/src/integrations/kinds/office.rs` that `contribute` returns all eleven tools, that the listing text mentions PDF, and that the kind still has no fields and one installation per agent (FR-017, FR-019) *(the listing text is tested; `contribute` needs a full agent context to build, so the eleven-tool set is pinned by `every_tool_is_in_its_category` instead)*
- [X] T041 [P] [US5] Test that the key phrases required of every description are present: for the four PDF tools "replaced"/"new filename" (write tools), "never edits" (all four), and the contract's limits; mirror the pin 036 used for the Word and Excel descriptions (plan G1)

### Implementation for User Story 5

- [X] T042 [US5] Confirm `read_document_file` and the image-reader hint are unchanged for PDF (FR-020, quickstart section 9): add a test in `crates/vizier-core/src/agents/tools/session_files/` that a PDF still reads as before, and, if the hint text lists the office tools, add the PDF tools to it
- [X] T043 [US5] Update `docs/src/configuration/office-addon.md`: a limits subsection (10 pages per read, 200 000 characters, 25 megapixels per rendered page, 5 MB of HTML, 256 MB PDF), the Word-to-PDF caveats, "no images in written PDFs", and the scale range; keep the four table rows as they are

**Checkpoint**: all five stories done.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [ ] T044 Run the dummyplug end-to-end in `specs/044-office-pdf/quickstart.md` (sections 1 to 9) against a running binary; paste the result of each section into the PR (FR-024, constitution e2e gate)
- [ ] T045 [P] *(not hand-edited: CHANGELOG.md is generated by git-cliff from commit messages, so the `feat:` commit is the entry)* Add the changelog entry (`feat: [office] read, render and write PDF files`) to `CHANGELOG.md`; it is not breaking (FR-022)
- [ ] T046 [P] Cross-build evidence (constitution Principle IV, research R6): `cross build --release --target x86_64-unknown-linux-musl` and `--target aarch64-unknown-linux-gnu`, and the CI `app` job green on Windows; record the result in the PR. Change `Cross.toml` only if a build fails
- [X] T047 [P] `cargo clippy --workspace` clean, and `cargo test -p vizier-core --no-default-features` still builds with the `documents` feature off *(clippy `-p vizier-core --features documents --all-targets` clean; `--no-default-features` builds)*
- [ ] T048 `cargo test --workspace` green, including the pinned tables
- [X] T049 Review `office/pdf.rs` and `office/mod.rs` for `unwrap`/`expect` and `println!` outside tests (constitution V); fix any found
- [X] T050 Update the stale line in `specs/036-office-addon/spec.md` Assumptions ("Phase 2, a later feature, is PDF support") to point at `specs/044-office-pdf/`

---

## Dependencies & Execution Order

- **Phase 1** first. T003 and T004 can run beside T002 and T005.
- **Phase 2**: T006 to T010 are done; T011 gates the stories' tool-level tests only if they call the tools, not the pure functions.
- **US1 to US4** need Phase 1 (fixtures) and Phase 2, and need nothing from each other. US3's writer is also used by US4 (`from_docx` calls `from_html`), so finish T028 before T034 if one person does both.
- **US5** needs the four tools; T039 needs US1 to US4's tools to exist, which they do; its description pins (T041) must run after T021, T027, T032, T038.
- **Polish** last. T044 needs everything; T046 can run any time after Phase 1.

### Within a story

Tests first and seen failing, then the fix, then the description rewrite.

## Parallel Example

```text
After Phase 2:
  Developer A: T012-T017 then T018-T021   (US1, pdf.rs read path)
  Developer B: T022-T023 then T024-T027   (US2, pdf.rs render path)
  Developer C: T028-T031 then T032-T033   (US3, html path)
Then: T034-T038 (US4), which reuses the US3 writer.
```

Several of these edit `pdf.rs` and `mod.rs`: use separate test modules per story to avoid merge conflicts.

## Implementation Strategy

- **MVP**: Phase 1 + Phase 2 + US1 (T001 to T021). A reader alone is useful and carries the least risk.
- **Next**: US3 (HTML to PDF), since PDF output is what people send; then US2, then US4.
- **Finish**: US5 and Polish. Do not merge before T002 (dependency cost), T044 (e2e) and T046 (cross builds) have recorded results; those are the constitution gates.
- Commits are conventional (`feat:`, `fix:`, `doc:`, `chore:`; there is no `test:` type here, so tests go in the `feat:` or `fix:` commit they belong to). `just commit "<message>"` handles the submodule order.

## Summary

50 tasks. Phase 1: 5; Phase 2: 6; US1: 10; US2: 6; US3: 6; US4: 5; US5: 5; Polish: 7.
