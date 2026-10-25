# Feature Specification: Office Addon (DOCX and XLSX)

**Feature Branch**: `feat/030-office-addon`

**Created**: 2026-10-25

**Status**: Draft

**Input**: User description: "Add a built-in `office` addon that gives agents first-class, local Office file support, enabled by default for every agent (GitHub issue vizier-lab/vizier#100). Phase 1: DOCX read+write and XLSX read+write. PDF is phase 2 and parked."

## Clarifications

### Session 2026-10-25

- Q: What happens when a write targets a filename that already exists in the conversation's files? → A: Follow the issue: the write tools use the same flow as `image_generate`, so a file of the same name is replaced, as with every other file-producing tool. The tool descriptions say so and tell the agent to choose a new name for a version derived from an existing file.
- Q: How is workbook text that starts with `=`, `+`, `-` or `@` written? → A: Always as plain text, never as a formula.
- Q: Which size limits apply, given the issue only limits what a read returns? → A: Only the issue's read-output limit (about 200 KB of text, capped sheets and rows, `truncated: true`). Decided by the repository owner for this proof of concept: no other limits (none on input file size, uncompressed size or write size), because validation is easy to add later and awkward to remove. Long text for a document is composed first with the scratchpad tools.
- Q: Should the generic document reader stop handling Word and Excel in this feature, since the repository owner dislikes a built-in tool that is always present? → A: Yes, decided by the repository owner: Word and Excel reading leaves the generic document reader in this change, so there is one reader per format and no redundancy. The generic reader keeps handling plain text and PDF, and for a Word or Excel file it points the agent to the office tools (US5). Every existing agent receives the addon through the one-time step (FR-016), so no agent loses the ability to read these files on upgrade; an agent whose addon is removed can no longer read them until it is installed again.
- Q: Is the addon on by default, or only available to install? → A: On by default for every agent, as the issue says (US3). The on/off switch and removal (US4, FR-029) let a person turn it off.
- Q: Does a password-protected file need its own error message? → A: Not now. The issue does not mention it, so it gets the ordinary "not a valid Office file" error and a dedicated message can be added later.
- Q: Does PDF belong to this feature? → A: No. Confirmed by the requester: PDF reading, rendering and creation are phase 2, a later feature.

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
4. **Given** a file the person uploaded, **When** the agent is asked to "edit" it, **Then** the tool descriptions make clear that writing produces a newly generated file rather than editing the original in place, and that it should use a different filename to keep the original, so the agent says so instead of silently losing the original's styles, headers, images or tracked changes.
5. **Given** a written file, **When** it is read back, **Then** headings, emphasis, lists, tables and links (documents) or sheets, types and values (workbooks) survive the round trip.

---

### User Story 3 - Every agent has the capability without setup (Priority: P1)

A person creating an agent, or one who already has agents, finds that the Office tools are simply there. Nothing to install, no credentials, no settings. Agents created from now on get the addon as part of creation. Agents that already exist receive it once, automatically.

**Why this priority**: A capability people must discover and install is rarely used. "On by default" is a stated decision of the issue.

**Independent Test**: Create a new agent and list its tools: the Office tools are present. Upgrade an installation that has existing agents: after first start each agent has the addon.

**Acceptance Scenarios**:

1. **Given** a new agent, **When** it finishes creation, **Then** its first running instance already has the Office tools, without a restart.
2. **Given** existing agents from before this feature, **When** vizier starts for the first time after the upgrade, **Then** each gets the addon once.
3. **Given** the person who owns an agent does not hold permission to manage integrations, **When** the addon is seeded for that agent, **Then** it is still added, and the action is recorded in the integration history under the agent's owner, or under a system actor when the agent has no owner (FR-019).
4. **Given** the addon is seeded, **When** the integration history of the agent is viewed, **Then** the addition appears with an actor, like any other integration change.

---

### User Story 4 - Turning the addon off, or removing it, sticks (Priority: P2)

A person does not want an agent to touch Office files. From that agent's Addons page they can turn the addon off with its on/off switch (and back on later), or remove it altogether. Either way it stays that way across restarts, upgrades and agent updates.

**Why this priority**: "On by default" is only acceptable if opting out is easy and permanent; otherwise it is a nuisance people cannot get rid of.

**Independent Test**: Turn the addon off for an agent, restart vizier, update the agent's configuration, restart again. It is still off and the agent has no Office tools. Turn it on: the tools are back. Remove it and repeat: it never returns.

**Acceptance Scenarios**:

1. **Given** an agent with the addon turned off, **When** the person looks at its Addons page, **Then** the addon is shown as off, with a switch to turn it on, and the agent has none of the Office tools.
2. **Given** an agent with the addon turned off, **When** vizier restarts or the agent's configuration is updated, **Then** it is still off and nothing turns it on or adds a second one.
3. **Given** an agent with the addon turned off, **When** the person turns it on, **Then** the Office tools are available again, without restarting the agent or aborting a running turn.
4. **Given** an agent with the addon removed, **When** vizier restarts or the agent's configuration is updated, **Then** the addon is not re-added.
5. **Given** an agent with the addon removed, **When** someone installs it again from the Addons page, **Then** it works like any other addon install, with only one installation per agent.

---

### User Story 5 - Word and Excel reading lives in one place (Priority: P2)

Until now the generic `read_document_file` tool also converted Word and Excel files to plain text, losing structure. Reading those files now belongs to the office tools only. The generic reader keeps handling plain text and PDF, and when it is given a Word or Excel file it answers with a clear message pointing to the office tools instead of a degraded conversion.

**Why this priority**: Two readers for the same format would be redundant, and the plain-text one is the source of the lost context this feature fixes.

**Independent Test**: Ask an agent with the addon to read a Word file through the generic reader: it is told to use the office tools. Ask it to read a text file and a PDF through the generic reader: both work as before.

**Acceptance Scenarios**:

1. **Given** a Word or Excel file (including a legacy `.xls`), **When** the agent reads it through the generic reader, **Then** it gets a clear message that these files are read with the office tools, and no plain-text conversion is produced.
2. **Given** an agent without the Office tools (addon removed or turned off), **When** it is given a Word or Excel file, **Then** the message also says the Office addon must be installed or turned on first.
3. **Given** a plain text file or a PDF, **When** the agent reads it through the generic reader, **Then** the result is the same as before this feature.
4. **Given** the generic reader's description and the hint shown with attached files, **When** the agent reads them, **Then** they point to the office tools for Word and Excel files.

---

### Edge Cases

- A filename that does not exist among the conversation's files, or that belongs to a different format than the tool expects (a `.docx` passed to the workbook reader): a clear, non-fatal error the agent can act on.
- A corrupt, truncated, password-protected or otherwise unreadable file: a clear error saying it is not a valid Office file, never a crash of the turn.
- A requested range or sheet that does not exist: an error that lists the sheets that do exist.
- A workbook with empty cells, merged cells, formulas and error values: formulas are read as their last computed value; empty cells stay empty; errors are reported as such.
- Writing a filename that already exists among the conversation's files (an upload, or something the agent wrote earlier): the existing file is replaced, as with the product's other file-producing tools. The tool descriptions state this and tell the agent to pick a new name when producing a version derived from an existing file.- Writing with a filename that lacks or has the wrong extension, or is unsafe: normalised or refused with a clear message.
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
- **FR-006**: Write tools MUST generate a brand-new file from the content they are given. No tool in this addon edits an existing Word or Excel file in place or reads one back in to modify it. The tool descriptions MUST state this and the reason (styles, headers, images and tracked changes would be lost).
- **FR-007**: Written files MUST be stored in the conversation's files with their correct Office media type set explicitly, so every reader of those files recognises the format. Delivery to the person remains the agent's explicit use of the existing attachment-sending tool; the write tools MUST NOT attach files automatically.
- **FR-008**: Files produced by the write tools MUST open without repair prompts in current Microsoft Office and LibreOffice.
- **FR-009**: Read tools MUST bound their output (about 200 KB of text; capped numbers of sheets and rows) and, when they cut output, MUST say so with a `truncated: true` marker rather than failing.
- **FR-010**: Every error (missing file, wrong format, corrupt file, bad sheet or range) MUST be returned as a result the agent can read and act on, never as a failure that ends the turn.
- **FR-011**: Legacy `.xls` workbooks MUST be readable by the workbook reader.

**Generic document reader**

- **FR-012**: The generic `read_document_file` tool MUST stop converting Word (`.docx`) and Excel (`.xlsx`, `.xls`) files to text. Given one, it MUST return a clear message naming the office tools that read it, and saying the Office addon must be installed or turned on when the agent does not have its tools.
- **FR-013**: The generic reader MUST keep handling every other format it handles today (plain text formats and PDF) without any change in behaviour.
- **FR-014**: The descriptions of the generic document reader and the image reader, and the hint shown with attached files, MUST point agents to the `office_*` tools for Word and Excel files and for creating them.

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

**File handling**

- **FR-027**: A write tool MUST behave like the product's other file-producing tools when a conversation file of the same name exists: it replaces that file. Its description MUST say so, and MUST tell the agent to choose a new name when the new file is a version derived from an existing one (so the original is kept).
- **FR-028**: The workbook write tool MUST store every text value as plain text. Text that begins with `=`, `+`, `-` or `@` MUST NOT become a formula, so opening a generated file never executes content taken from untrusted sources.

**Turning the addon off and on**

- **FR-029**: A person who can manage an agent's integrations MUST be able to turn the Office addon off and on again from the agent's Addons page without removing it, using the same on/off switch as every other installed addon. While it is off the agent has none of the Office tools; turning it on restores them. Neither change restarts the agent or aborts a running turn (FR-021), and an addon that is off is not re-added or duplicated by a restart, an agent update or the one-time step.

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
- **SC-006**: Reading a file whose content exceeds the documented output limits never fails the turn; it returns a partial result marked `truncated: true` in 100% of the oversized-output test cases.
- **SC-007**: A Word or Excel file given to the generic reader is never converted to plain text, and the agent is pointed to the office tools in 100% of test cases, while plain text and PDF files read exactly as before.
- **SC-008**: No existing agent's configuration, running turns or other integrations change when the addon is seeded.

## Assumptions

- Phase 1 delivers Word and Excel only. PDF reading, rendering and creation are explicitly phase 2 and out of scope here, as confirmed by the requester; a later feature adds them. Word and Excel handling is also the part of the repository owner's wider goal (read and write documents without external tools) that fits the product's no-external-dependency rule; PDF needs heavier engines and waits.
- This is a breaking change for agents and skills that relied on the generic reader for Word or Excel files: they now need the Office addon. The one-time step gives every existing agent the addon, so none loses the ability on upgrade, and the commit is flagged as breaking.
- A password-protected file gets the ordinary "not a valid Office file" error; a dedicated message is left for later because the issue does not mention it.
- Everything runs locally with no network service, account or credential. No new operating-system package is required.
- **Decision (the issue's open question, confirmed by the requester) — re-seeding on update.** Removal is permanent, including across agent updates (the issue's recommendation). Reversing this later is a new decision.
- **Decision (the issue's open question, confirmed by the requester) — Word reader depth.** The first version uses the document library already in the product plus table support; a lower-level reader for numbering and hyperlinks is revisited only if real files show gaps.
- **Decision (the issue's open question, which gave no recommendation; chosen by the requester) — display name.** The addon is shown as "Office" in the catalogue; the id is `office` in either case. A rename to "Documents" is a presentation-only change.
- **Decision (the issue's open question, confirmed by the requester) — auto-attach.** Written files are not attached automatically; the agent sends them with the existing attachment tool, consistent with image generation.
- Read-output limits (about 200 KB of text; capped sheets and rows) come from the issue. No other size limit is imposed in this proof of concept (owner's decision): validation is easier to add later than to remove. Cutting a read is always signalled.
- Formulas are read as their last saved value; formulas are never written in phase 1 (see FR-028).
- Charts, pivot tables, conditional formatting, images and embedded objects are not read or written in phase 1; they may be skipped silently on read.
- Existing rules for who may read and send conversation files are unchanged.
- Out of scope: editing existing Office files in place with formatting preserved; legacy binary `.doc` and `.ppt`; charts, pivots and conditional formatting; online Office services (Google Workspace, Microsoft Graph); a general-purpose filesystem tool.
- **Web interface work, separate pull requests in the design and web interface repositories** (the owner wants the on/off switch, FR-029). The Addons page already shows an on/off switch on every installed addon card and the server already supports it, so the Office addon gets one as soon as it is installed. What remains is to make sure it works for an addon with no fields and shows a sensible on/ready state instead of a "reading" state; to reserve `office` as a taken category name so an MCP server with that name is labelled correctly; and, optionally, to list the addon's tools in the interface's addon tool list. The server side does not depend on them to function.
- The backfill of existing agents is migration code and is tracked for removal before 1.0.0 on the project's tracker.
