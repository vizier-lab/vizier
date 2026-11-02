# Feature Specification: Office Addon Phase 2 (PDF)

**Feature Branch**: `feat/office-pdf`

**Created**: 2026-11-02

**Status**: Draft

**Input**: User description: "Phase 2 Office Addon: integrate pdf-agent v0.1.0 to support PDF read as markdown, render page as png, write from html, and write from docx into the office addon (issue #100)"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Agent reads a PDF as structured text (Priority: P1)

A person attaches a PDF (a report, contract, paper or invoice) and asks the agent about it. The agent reads the file and sees headings, paragraphs, lists and tables as Markdown, with page information, so it can answer from the real content and continue through a long document page by page.

**Why this priority**: Reading is the most common need and has no write-side risk. It is the foundation the other stories build on (reading back what was written, checking a conversion).

**Independent Test**: Attach a multi-page PDF containing headings, a list and a table. Ask the agent to summarise it and to quote one table row. The answer is correct, and a document longer than one read is covered by following the "next page" hint.

**Acceptance Scenarios**:

1. **Given** a text PDF, **When** the agent reads it, **Then** it receives Markdown with headings, paragraphs, lists and tables, plus the total page count and the range of pages returned.
2. **Given** a PDF longer than one read returns, **When** the agent reads it without a page range, **Then** the result is cut at a documented limit, says `truncated: true`, and names the page to continue from.
3. **Given** a long PDF, **When** the agent asks for a start page and a number of pages, **Then** only those pages are returned.
4. **Given** a password-protected PDF, **When** the agent reads it with the password the person supplied, **Then** it is read; **and when** the password is missing or wrong, **Then** the agent gets a clear error saying so.
5. **Given** a scanned PDF with no selectable text, **When** the agent reads it, **Then** the result says the document is likely scanned and points to the page-rendering tool.
6. **Given** a PDF in which some pages cannot be parsed, **When** the agent reads it, **Then** the readable pages are returned and the failed page numbers are listed.
7. **Given** a PDF the agent has just written, **When** it reads it back, **Then** the text it wrote is present.

---

### User Story 2 - Agent looks at a PDF page as an image (Priority: P1)

Some pages cannot be understood as text: diagrams, charts, scans, complex layouts, signatures. The agent renders one page to a PNG saved with the conversation's files and then looks at it with the existing image reader.

**Why this priority**: It is the only route into scanned documents and visual content, and it lets the agent check the layout of a PDF it produced.

**Independent Test**: Render page 2 of a PDF containing a chart, then read the PNG with the image reader. The agent can describe the chart.

**Acceptance Scenarios**:

1. **Given** a PDF, **When** the agent renders page N (counted from 1), **Then** a PNG is saved to the conversation's files and the agent receives its filename, size and pixel dimensions.
2. **Given** a render request with a scale, **When** it is made, **Then** the image is larger or smaller accordingly; without a scale a sensible default is used.
3. **Given** a page number below 1 or beyond the last page, **When** the agent renders it, **Then** the error states how many pages the PDF has.
4. **Given** a rendered PNG, **When** the agent opens it with the image reader, **Then** it can see the page.

---

### User Story 3 - Agent creates a designed PDF from HTML (Priority: P1)

A person asks for an invoice, a report, a certificate or a one-page summary as a PDF. The agent composes the document as HTML with styling and produces a `.pdf` saved with the conversation's files, then sends it to the person.

**Why this priority**: PDF is the format people actually send and print. Producing it from HTML gives the agent full control of layout, typography, tables and colour.

**Independent Test**: Ask the agent for an invoice with a company header, a line-item table with borders and a coloured total row. Open the PDF in a viewer: layout and styling match.

**Acceptance Scenarios**:

1. **Given** HTML with styling, **When** the agent writes a PDF, **Then** a `.pdf` file is saved to the conversation's files and the agent receives its filename and size.
2. **Given** a written PDF, **When** the agent sends it as an attachment, **Then** the person receives it and it opens in common PDF viewers without a repair prompt.
3. **Given** a filename without `.pdf`, **When** the agent writes, **Then** the extension is added; **and given** an unsafe name (a folder path), **Then** it is refused with a clear message.
4. **Given** a filename that already exists among the conversation's files, **When** the agent writes, **Then** the file is replaced, as with the other file-producing tools, and the description says to pick a new name to keep the original.
5. **Given** HTML that is empty or cannot be rendered, **When** the agent writes, **Then** it gets a readable error, not a failed turn.
6. **Given** HTML that refers to outside resources (remote images, scripts), **When** the PDF is produced, **Then** no outside resource is fetched and no script is run.

---

### User Story 4 - Agent turns a Word document into a PDF (Priority: P2)

A person has a Word document (uploaded, or just written by the agent) and wants a PDF of it. The agent converts it, keeping headings, lists, tables, emphasis and links.

**Why this priority**: A common closing step ("write the report as Word, then give me the PDF"). It builds on the Word reader already in the addon, so it is a smaller slice than Story 3.

**Independent Test**: Write a document with a heading, a list and a table using the Word tool, convert it, read the PDF back: the same content is present.

**Acceptance Scenarios**:

1. **Given** a `.docx` among the conversation's files, **When** the agent converts it, **Then** a `.pdf` is saved and the agent receives its filename and size.
2. **Given** a document with headings, lists, a table and a link, **When** converted, **Then** reading the PDF back shows each of them.
3. **Given** an optional title, **When** supplied, **Then** it is used for the PDF's title.
4. **Given** a file that is not a Word document (a workbook, a text file), **When** the agent converts it, **Then** the error says which tool reads that format.
5. **Given** the original Word file, **When** it is converted, **Then** the original is unchanged.

---

### User Story 5 - The PDF tools arrive with the addon, with no setup (Priority: P2)

An agent that already has the Office addon gets the PDF tools without anything to install or configure. Turning the addon off or removing it still removes every tool, PDF ones included, and the choice stays.

**Why this priority**: Phase 1 decided the addon is on by default and removal is permanent. Phase 2 must extend that without a second switch or a second backfill.

**Independent Test**: On an agent with the addon, list its tools: the four PDF tools are present. Turn the addon off: all Office tools disappear. Turn it back on: they return.

**Acceptance Scenarios**:

1. **Given** an agent with the Office addon, **When** this version starts, **Then** the PDF tools are present with no restart of the agent and no new integration row.
2. **Given** an agent with the addon off or removed, **When** this version starts, **Then** it still has no Office tools and nothing re-adds it.
3. **Given** the addon's description in the catalogue and in the docs, **When** read, **Then** they mention PDF alongside Word and Excel.
4. **Given** the Word or Excel reader is given a PDF, **When** it answers, **Then** the error names the PDF tools; **and given** a PDF tool is given another format, **Then** the error names the right tool for that format.

---

### Edge Cases

- A filename that does not exist among the conversation's files: a clear, non-fatal error.
- A corrupt or truncated PDF: a clear error saying it is not a valid PDF, never a crash of the turn.
- An encrypted PDF that cannot be opened: the error says a password is needed or wrong, and does not reveal anything about the content.
- A very large PDF (hundreds of pages): a read returns a bounded slice with a continuation hint; rendering one page does not load every page's text or images into the result.
- A page with an extreme size or a scale that would produce an enormous image: the scale is bounded and the error or clamp is stated.
- HTML or Word content in non-Latin scripts, emoji or right-to-left text: either rendered or reported, never silently dropped to blanks.
- Word documents with images, headers and footers, tracked changes or complex floating layouts: converted on a best-effort basis from the document's text structure; what is not carried over is stated in the tool description.
- Writing a PDF with the filename of the source `.docx`'s sibling that already exists: replaced, per the existing rule.
- An agent in a conversation channel where the person cannot receive files: governed by the existing session-file rules.

## Requirements *(mandatory)*

### Functional Requirements

**Tools**

- **FR-001**: The Office addon MUST gain four tools, named with the existing `office_` prefix and listed in the category `office`: `office_pdf_read_as_md`, `office_pdf_render_page_as_png`, `office_pdf_write_from_html` and `office_pdf_write_from_docx`. Names MUST state the file type and the shape of what they produce or take, as the Word and Excel tools do.
- **FR-002**: The read tool MUST return a PDF's content as Markdown preserving headings, paragraphs, lists and tables, together with the total page count, the first and last page returned, and, when more pages remain, the page to continue from.
- **FR-003**: The read tool MUST accept an optional start page, an optional number of pages, an optional maximum number of characters and an optional password. Defaults MUST be documented in the tool description.
- **FR-004**: The read tool MUST bound its output and, when it cuts, MUST say `truncated: true` and give the continuation page rather than fail.
- **FR-005**: The read tool MUST report when a document appears to be scanned (no selectable text) and say which tool to use next, and MUST list pages that could not be parsed while still returning the pages that could.
- **FR-006**: The render tool MUST render one page (counted from 1) to a PNG saved among the conversation's files and return its filename, size and pixel dimensions. It MUST accept an optional scale with a documented default and a bounded range.
- **FR-007**: The render tool's output MUST be readable by the existing image reader without further conversion.
- **FR-008**: The HTML tool MUST create a new `.pdf` from HTML with styling, save it among the conversation's files and return its filename and size. It MUST accept an optional title.
- **FR-009**: The HTML tool MUST NOT fetch remote resources or run scripts while producing the PDF.
- **FR-010**: The Word conversion tool MUST create a new `.pdf` from a `.docx` among the conversation's files, preserving headings, emphasis, lists, tables and links, and return its filename and size. It MUST accept an optional title. The source file MUST be left unchanged.
- **FR-011**: Both write tools MUST generate a brand-new file. No tool in this addon edits an existing PDF in place, merges, splits, annotates or fills forms; the descriptions MUST say so.
- **FR-012**: Written files and rendered images MUST be stored among the conversation's files with their correct media type set explicitly. The write tools MUST NOT attach files automatically; delivery remains the agent's explicit use of the existing attachment tool.
- **FR-013**: A write tool MUST replace a conversation file of the same name, as the other file-producing tools do, and its description MUST say so and tell the agent to choose a new name to keep the original. Names MUST get the `.pdf` (or `.png`) extension when missing and MUST be refused when unsafe (folders, traversal).
- **FR-014**: Every error (missing file, wrong format, corrupt or encrypted file, bad page number, unrenderable HTML) MUST be returned as a result the agent can read and act on, never a failure that ends the turn.
- **FR-015**: When a tool is given a file of another supported format, its error MUST name the tool that reads that format. This covers every pair among Word, Excel and PDF, in both directions.
- **FR-016**: Files produced by the write tools MUST open without repair prompts in current common PDF viewers.

**Rollout and compatibility**

- **FR-017**: The PDF tools MUST belong to the existing `office` addon: same id, no new settings, no new integration, no second on/off switch, no new backfill. Agents that have the addon get the tools on the next instance; agents with the addon off or removed do not (spec 036 FR-017, FR-029).
- **FR-018**: Adding the tools MUST NOT restart any agent or abort a running turn.
- **FR-019**: The addon's catalogue description, the user documentation page for the addon and the tool tables pinned in the tests MUST include the four tools, so adding or renaming one without updating them fails the build.
- **FR-020**: The generic document reader's behaviour for PDF MUST NOT change. The PDF tools are an addition for agents that want structure, pagination or images; the generic reader keeps working for plain text and PDF as before.
- **FR-021**: The change MUST add no network service, account or credential and no operating-system package requirement; everything runs locally.
- **FR-022**: The change MUST be recorded in the changelog and in the addon's documentation, including the limits and what is not carried over in a Word-to-PDF conversion.

**Quality**

- **FR-023**: Each PDF operation MUST be tested on its own, without storage or an agent: read (pagination, truncation, password, scanned, failed pages), render (valid PNG, page bounds, scale), write from HTML and from Word (valid PDF that reads back), and a corrupt input.
- **FR-024**: The change MUST be verified end to end on an agent using the offline test provider: write a PDF from HTML, render page 1, read it back as Markdown, convert a written Word document to PDF, and send a result as an attachment.

### Key Entities *(include if feature involves data)*

- **Office addon**: The existing built-in integration of kind `office`, owned by one agent, with no settings. It now contributes the Word, Excel and PDF tools.
- **Conversation file**: A named file with a media type stored with a conversation; the tools read from and write to these.
- **PDF read result**: Markdown text for a slice of pages with total pages, page range, continuation page, truncation flag, failed pages, a scanned-document flag and an explanatory note.
- **Rendered page**: A PNG conversation file produced from one page, with its pixel dimensions.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: In a fixed sample of text PDFs (headings, lists, tables, multiple pages), every heading and every table cell appears in what the agent sees, in 100% of the sample.
- **SC-002**: For a fixed sample of HTML inputs and Word documents, writing a PDF and reading it back preserves the text, headings, list items and table cells in 100% of the round-trip tests.
- **SC-003**: A person can ask for an invoice or a report and have the PDF in hand in a single request, with no setup step and no credentials.
- **SC-004**: Reading a PDF whose content exceeds the output limit never fails the turn; it returns a partial result marked `truncated: true` with a continuation page in 100% of oversized test cases, and following the continuation pages covers the whole document.
- **SC-005**: For every page of a sample PDF, rendering returns an image the image reader opens, in 100% of the sample; an out-of-range page returns an error naming the page count in 100% of cases.
- **SC-006**: A corrupt, truncated or wrongly protected PDF produces a readable error and the turn continues in 100% of test cases.
- **SC-007**: In all test cases across Word, Excel and PDF files given to a tool of the wrong format, the error names the right tool.
- **SC-008**: 100% of agents that have the addon offer the four PDF tools after upgrading; zero agents that turned the addon off or removed it get any Office tool back.
- **SC-009**: Producing a PDF from HTML never makes a network request or runs a script in the verification set.
- **SC-010**: Files produced by the write tools open without a repair prompt in the verification set of common PDF viewers.

## Assumptions

- This is Phase 2 of the Office addon (spec `036-office-addon`, issue #100); the Word and Excel behaviour, the on-by-default rules, permanent removal and the on/off switch decided there are unchanged and not restated here.
- The implementation uses the `pdf-agent` library at v0.1.0, as the requester specified. Its capabilities (structured Markdown extraction with pagination and password support, page rasterisation, HTML-to-PDF generation) define what the four tools can offer; limits it has surface as documented limits.
- Word-to-PDF conversion works from the document's text structure as the existing Word reader exposes it (headings, emphasis, lists, tables, links), then lays it out; it is not a pixel-faithful render of Word's own layout. Images, headers and footers, page numbering and tracked changes are not guaranteed to carry over.
- Read defaults follow the library's: ten pages per read unless a count is given. A character limit applies in addition and is signalled by `truncated: true`.
- No limit on input PDF size is added beyond the bounded output (consistent with the proof-of-concept decision in spec 036); validation is easier to add later than to remove.
- No OCR is added. A scanned PDF is flagged and the agent can look at pages as images instead.
- Out of scope: editing, merging, splitting, rotating, watermarking, annotating or form-filling existing PDFs; digital signatures; OCR; PDF/A or accessibility tagging; converting Excel to PDF; HTML-to-PDF with remote assets; any web interface change (the addon card already exists and now shows the updated description).
- A password is supplied by the person in conversation and passed by the agent as a tool argument; it is not stored by the addon.
- Existing rules for who may read and send conversation files are unchanged.
- Part of the implementation was already written and verified on the branch before this specification; this document defines the behaviour to which that work and its remaining tests, docs and changelog entry are held.
