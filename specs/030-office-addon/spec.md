# Feature Specification: Office Addon (DOCX and XLSX)

**Feature Branch**: `feat/030-office-addon`

**Created**: 2026-10-25

**Status**: Draft

**Input**: User description: "Add a built-in `office` addon that gives agents first-class, local Office file support, enabled by default for every agent (GitHub issue vizier-lab/vizier#100). Phase 1: DOCX read+write and XLSX read+write. PDF is phase 2 and parked."

## Clarifications

### Session 2026-10-25

- Q: What happens when a write targets a filename that already exists in the conversation's files? → A: The agent may replace a file it created itself, but never a file a person uploaded; in that case the tool returns an error and the agent chooses another name.
- Q: How is workbook text that starts with `=`, `+`, `-` or `@` written? → A: Always as plain text, never as a formula.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Agent reads a Word or Excel file with full fidelity (Priority: P1)

A person attaches a Word document or a spreadsheet to a conversation and asks the agent about it. The agent reads the file and sees its real structure: headings, emphasis, lists, links and **tables** for a document; sheets and typed cells (text, numbers, booleans, dates) for a workbook. Today tables in a Word document come back as the placeholder `[table content]` and spreadsheet cells are flattened to plain text, so the agent answers from incomplete information.

**Why this priority**: Reading is the most common need and the lowest risk. It fixes a visible defect (lost tables) and is useful with no write path at all.

**Independent Test**: Attach a Word file containing a heading, bold text, a nested list, a link and a table, and a workbook with several sheets and mixed cell types. Ask the agent to describe each. Every table row and typed value appears in the agent's view of the file.

**Acceptance Scenarios**:

1. **Given** a Word document with a table, **When** the agent reads it, **Then** the table's rows and cells are present in the result and the text `[table content]` does not appear.
2. **Given** a workbook with two sheets, **When** the agent reads it without naming a sheet, **Then** it receives both sheets and each cell keeps its type (a number is a number, a date is a date, a true/false value is a boolean).
3. **Given** a workbook, **When** the agent asks for one sheet and a cell range, **Then** only that range is returned.
4. **Given** a very large document or workbook, **When** the agent reads it, **Then** the result is cut at a documented limit and carries a `truncated: true` marker instead of failing.
5. **Given** a legacy `.xls` workbook, **When** the agent reads it, **Then** it reads like a modern workbook.

---

### User Story 2 - Agent creates a Word or Excel file for the person (Priority: P1)

A person asks the agent for a report, a letter, a budget or a data export. The agent produces a new `.docx` or `.xlsx` file, stores it with the conversation's files and hands it to the person as an attachment. The file opens normally in Microsoft Office, LibreOffice and similar programs.

**Why this priority**: There is no write path for any Office format today, so this is the headline capability. It shares priority with reading because people ask for both in the same breath ("read this, then give me a cleaned-up version").

**Independent Test**: Ask the agent for a one-page document with a heading, a list and a table, and for a two-sheet workbook with a bold header row. Open both files in an Office program. Content and structure match what was asked.

**Acceptance Scenarios**:

1. **Given** the agent has drafted content as Markdown, **When** it writes a document, **Then** a `.docx` file is saved to the conversation's files and the agent receives its filename and size.
2. **Given** the agent has rows of data, **When** it writes a workbook with named sheets, **Then** a `.xlsx` file is saved with those sheets, typed cells, a bold header row and readable column widths.
3. **Given** a file was just written, **When** the agent sends it as an attachment, **Then** the person receives it.
4. **Given** a file the person uploaded, **When** the agent is asked to "edit" it, **Then** the tool descriptions make clear that writing always creates a new file and never changes the original, so the agent says so instead of silently losing the original's styles, headers, images or tracked changes.
5. **Given** a written file, **When** it is read back, **Then** headings, emphasis, lists, tables and links (documents) or sheets, types and values (workbooks) survive the round trip.

---

### User Story 3 - Every agent has the capability without setup (Priority: P1)

A person creating an agent, or one who already has agents, finds that the Office tools are simply there. Nothing to install, no credentials, no settings. Agents created from now on get the addon as part of creation. Agents that already exist receive it once, automatically.

**Why this priority**: A capability people must discover and install is rarely used. "On by default" is a stated decision of the issue.

**Independent Test**: Create a new agent and list its tools: the Office tools are present. Upgrade an installation that has existing agents: after first start each agent has the addon.

**Acceptance Scenarios**:

1. **Given** a new agent, **When** it finishes creation, **Then** its first running instance already has the Office tools, without a restart.
2. **Given** existing agents from before this feature, **When** vizier starts for the first time after the upgrade, **Then** each gets the addon once.
3. **Given** the person who owns an agent does not hold permission to manage integrations, **When** the addon is seeded for that agent, **Then** it is still added, and the action is recorded as a system action.
4. **Given** the addon is seeded, **When** the integration history of the agent is viewed, **Then** the addition appears with an actor, like any other integration change.

---

### User Story 4 - Removing the addon sticks (Priority: P2)

An operator does not want an agent to touch Office files and removes the addon from that agent's Addons page. It stays removed across restarts, upgrades and agent updates.

**Why this priority**: "On by default" is only acceptable if opting out is permanent; otherwise it is a nuisance people cannot get rid of.

**Independent Test**: Remove the addon from an agent, restart vizier, update the agent's configuration, restart again. The addon never returns.

**Acceptance Scenarios**:

1. **Given** an agent with the addon removed, **When** vizier restarts, **Then** the addon is not re-added.
2. **Given** an agent with the addon removed, **When** the agent's configuration is updated, **Then** the addon is not re-added.
3. **Given** an agent with the addon removed, **When** someone installs it again from the Addons page, **Then** it works like any other addon install, with only one installation per agent.

---

### User Story 5 - The general document reader keeps working and gets better (Priority: P2)

Existing agents and skills call the generic `read_document_file` tool for Word and Excel files. That must keep working whether or not the addon is installed. It also benefits from the improved reading (tables, legacy `.xls`), and its description tells the agent that richer tools exist.

**Why this priority**: Prevents regressions for current users and avoids two diverging implementations.

**Independent Test**: Remove the addon from an agent, then ask it to read a Word file with a table through the generic reader. The table content is present.

**Acceptance Scenarios**:

1. **Given** an agent without the addon, **When** it reads a Word document with a table through the generic reader, **Then** the table content appears instead of `[table content]`.
2. **Given** an agent without the addon, **When** it reads a legacy `.xls` file through the generic reader, **Then** it returns the sheets as text.
3. **Given** the generic reader's description, **When** the agent reads it, **Then** it points to the Office tools for structured reading and for creating files.

---

### Edge Cases

- A filename that does not exist among the conversation's files, or that belongs to a different format than the tool expects (a `.docx` passed to the workbook reader): a clear, non-fatal error the agent can act on.
- A corrupt, truncated or password-protected file: a clear error naming the problem, never a crash of the turn.
- A requested range or sheet that does not exist: an error that lists the sheets that do exist.
- A workbook with empty cells, merged cells, formulas and error values: formulas are read as their last computed value; empty cells stay empty; errors are reported as such.
- Writing a filename that already exists among the conversation's files: if the existing file was created by the agent, it is replaced (so a draft can be revised); if a person uploaded it, the write is refused with an error telling the agent to choose another name, and the upload is left untouched.
- Writing with a filename that lacks or has the wrong extension, or is unsafe: normalised or refused with a clear message.
- Very large write requests (huge Markdown, enormous row counts): refused or capped with a clear message rather than exhausting memory.
- Dates and numbers that spreadsheets store ambiguously (date serials, 1900 versus 1904 date systems, numbers stored as text): reported by their displayed meaning, not as raw serials.
- An agent that has the addon but a person with no access to files in this conversation channel: governed by the existing session-file rules, no new rule.

## Requirements *(mandatory)*

### Functional Requirements

**Tools**

- **FR-001**: The system MUST provide a built-in addon with the id `office`. All its tools MUST be named with the prefix `office_` and listed under the category `office`.
- **FR-002**: The system MUST provide a tool that reads a Word document from the conversation's files and returns Markdown preserving headings, bold, italic, ordered and unordered (nested) lists, tables as pipe tables, and hyperlinks. It MUST accept an optional maximum number of characters.
- **FR-003**: The system MUST provide a tool that creates a new Word document from Markdown and an optional title, saves it as a conversation file, and returns its filename and size.
- **FR-004**: The system MUST provide a tool that reads a workbook and returns, per sheet, its name and rows with typed cells (text, number, boolean, date, empty, error). It MUST accept an optional sheet and an optional cell range.
- **FR-005**: The system MUST provide a tool that creates a new workbook from named sheets of rows, saves it as a conversation file, and returns its filename and size. Cell types MUST be preserved; the header row MUST be bold; column widths MUST be set so content is readable.
- **FR-006**: Write tools MUST only create new files. No tool in this addon edits an existing Word or Excel file in place. The tool descriptions MUST state this and the reason (styles, headers, images and tracked changes would be lost).
- **FR-007**: Written files MUST be stored in the conversation's files with their correct Office media type set explicitly, so every reader of those files recognises the format. Delivery to the person remains the agent's explicit use of the existing attachment-sending tool; the write tools MUST NOT attach files automatically.
- **FR-008**: Files produced by the write tools MUST open without repair prompts in current Microsoft Office and LibreOffice.
- **FR-009**: Read tools MUST bound their output (about 200 KB of text; capped numbers of sheets and rows) and, when they cut output, MUST say so with a `truncated: true` marker rather than failing.
- **FR-010**: Every error (missing file, wrong format, corrupt file, bad sheet or range, oversized request) MUST be returned as a result the agent can read and act on, never as a failure that ends the turn.
- **FR-011**: Legacy `.xls` workbooks MUST be readable by the workbook reader and by the generic document reader.
- **FR-028**: The workbook write tool MUST store every text value as plain text. Text that begins with `=`, `+`, `-` or `@` MUST NOT become a formula, so opening a generated file never executes content taken from untrusted sources.
- **FR-027**: A write tool MUST replace an existing conversation file of the same name only if that file was created by the agent. If the existing file was uploaded by a person, the write MUST be refused with an error that tells the agent to choose another name, and the upload MUST remain unchanged.

**Generic document reader**

- **FR-012**: The generic `read_document_file` tool MUST keep supporting Word and Excel files whether or not the addon is installed.
- **FR-013**: The generic reader MUST use the same reading logic as the addon (one implementation per format), so Word tables, and legacy `.xls` files are handled there too. Its plain-text output for these formats MUST remain a plain-text rendering, not Markdown or typed cells.
- **FR-014**: The descriptions of the generic document reader and the image reader MUST point agents to the `office_*` tools for richer reading and for creating files.

**On by default**

- **FR-015**: Every newly created agent MUST receive the `office` addon as part of creation, and its first running instance MUST already offer the tools, without needing a restart.
- **FR-016**: Every agent that existed before this feature MUST receive the addon exactly once, through a one-time step recorded by a persisted marker so it never runs again.
- **FR-017**: Removing the addon from an agent MUST be permanent. Nothing re-adds it on restart, upgrade, agent update or the one-time step.
- **FR-018**: Seeding MUST be a system action that does not require the agent's owner to hold permission to manage integrations, yet MUST still pass through the same integration management path as any other change: validation, a recorded history entry naming who made the change, and a change notification to the running agent.
- **FR-019**: The history entry's actor MUST be the agent's owner when one is known, otherwise a system actor. No new kind of actor is introduced for this.
- **FR-020**: The addon MUST have no required settings and MUST be valid with an empty configuration. At most one installation per agent MUST still hold.
- **FR-021**: Installing the addon MUST NOT restart the agent or abort a running turn.

**Quality and rollout**

- **FR-022**: Each format MUST be tested on its own, without storage or an agent: round-trip (write then read) and reading of fixed sample files, including a Word table and each spreadsheet cell type across more than one sheet.
- **FR-023**: Because every agent's default tool list changes, the change MUST be verified end to end on an agent using the offline test provider (write a document, receive the filename, send it as an attachment) and MUST be noted in the changelog.
- **FR-024**: The pinned tables that list every tool and its category MUST include the new tools, so adding or renaming a tool without updating them fails the build.
- **FR-025**: Code that exists only to support upgrades from earlier versions (the one-time backfill) MUST be recorded on the project's tracker for pre-1.0 migration cleanup.

**Parked (phase 2, designed for, not built)**

- **FR-026**: The design MUST leave room for PDF tools (read as Markdown, render pages as images, create from HTML, create from a Word document) without changing the tools or settings defined here. No PDF capability is delivered by this feature.

### Key Entities *(include if feature involves data)*

- **Office addon**: A built-in integration of kind `office`, owned by exactly one agent, with no settings. It contributes the four tools. Present or absent per agent.
- **Conversation file**: A named file stored with a conversation, with a media type. The addon's tools read from and write to these.
- **Seeding marker**: A persisted flag recording that the one-time backfill of existing agents has run.
- **Integration history entry**: The existing record of who changed which integration and when. Seeding adds one per agent.
- **Workbook / Sheet / Cell**: What the spreadsheet tools exchange: a workbook has named sheets; a sheet has rows; a cell has a value and a type.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: In a fixed sample set of Word documents containing tables, 100% of table cells are present in what the agent sees, and the placeholder `[table content]` appears in none of them.
- **SC-002**: For a fixed sample of documents and workbooks, writing and then reading back preserves every heading, list, table, link, sheet name and cell value and type (100% in the round-trip tests).
- **SC-003**: A person can ask an agent for a report or a spreadsheet and have the file in hand in a single request, with no setup step and no credentials.
- **SC-004**: Files produced by the addon open without a repair prompt in both Microsoft Office and LibreOffice in the verification set.
- **SC-005**: 100% of agents (new and pre-existing) have the addon after their first start on the new version, except those where it was removed; zero agents that removed it get it back across at least two restarts and one configuration update.
- **SC-006**: Reading a file larger than the documented limits never fails the turn; it returns a partial result marked `truncated: true` in 100% of the oversized test cases.
- **SC-007**: Agents without the addon read Word (including tables) and legacy `.xls` files through the generic reader with no regression in the existing document-reading cases.
- **SC-008**: No existing agent's configuration, running turns or other integrations change when the addon is seeded.

## Assumptions

- Phase 1 delivers Word and Excel only. PDF reading, rendering and creation are explicitly phase 2 and out of scope here; a later feature adds them.
- Everything runs locally with no network service, account or credential. No new operating-system package is required.
- **Open question: re-seeding on update.** Removal is permanent, including across agent updates (the issue's recommendation). Reversing this later is a new decision.
- **Open question: Word reader depth.** The first version uses the document library already in the product plus table support; a lower-level reader for numbering and hyperlinks is revisited only if real files show gaps.
- **Open question: display name.** The addon is shown as "Office" in the catalogue; the id is `office` in either case. A rename to "Documents" is a presentation-only change.
- **Open question: auto-attach.** Written files are not attached automatically; the agent sends them with the existing attachment tool, consistent with image generation.
- Output limits (about 200 KB of text; capped sheets and rows) are starting values and can be tuned; the contract is that cutting is always signalled.
- Formulas are read as their last saved value; formulas are never written in phase 1 (see FR-028).
- Charts, pivot tables, conditional formatting, images and embedded objects are not read or written in phase 1; they may be skipped silently on read.
- Existing rules for who may read and send conversation files are unchanged.
- Out of scope: editing existing Office files in place with formatting preserved; legacy binary `.doc` and `.ppt`; charts, pivots and conditional formatting; online Office services (Google Workspace, Microsoft Graph); a general-purpose filesystem tool.
- **Follow-up work, separate pull requests in the design and web interface repositories:** reserve `office` as a taken category name so an MCP server with that name is labelled correctly in the interface; make sure the addon card renders well for an addon with no fields and shows a sensible installed/ready state instead of a "reading" state. The core feature does not depend on them to function.
- The backfill of existing agents is migration code and is tracked for removal before 1.0.0 on the project's tracker.
