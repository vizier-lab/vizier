---

description: "Task list for the Office addon (DOCX and XLSX)"
---

# Tasks: Office Addon (DOCX and XLSX)

**Input**: Design documents from `/specs/034-office-addon/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/office-tools.md, quickstart.md

**Tests**: Included. The spec requires them (FR-022 per-format tests, FR-023 dummyplug end-to-end, FR-024 pinned tool tables) and the constitution requires `cargo clippy`, `cargo test` and a dummyplug end-to-end run for agent-observable changes.

**Organization**: Tasks are grouped by user story so each can be implemented and tested on its own. The WebUI work lives in the `webui` submodule and ships as separate pull requests (design → webui → vizier), so those tasks are marked **[web]**.

**Tool set**: five tools — `office_docx_read_as_md`, `office_docx_write_from_md`, `office_xlsx_read_as_cells`, `office_xlsx_read_as_csv`, `office_xlsx_write_from_sheets`. Each tool's name, description and parameters must teach an agent how to use it with no extra tutorial: say what it returns, when to use it instead of the sibling tool, and what each parameter does.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: Which user story the task belongs to (US1 to US5)
- Every task names the file it changes

## Path Conventions

Single Rust project: `src/`, `assets/`, `docs/`, `Cargo.toml` at the repository root. New code lives in `src/agents/tools/office/` and `src/integrations/kinds/office.rs`. Test fixtures live in `src/agents/tools/office/fixtures/` and are read with `include_bytes!`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Dependencies, module skeleton and test fixtures.

- [X] T001 Add `rust_xlsxwriter` and a direct `pulldown-cmark = "0.13"` dependency to `Cargo.toml` (`pulldown-cmark` is already in `Cargo.lock` through `text-splitter`); run `cargo fetch` and confirm the lockfile changes only by `rust_xlsxwriter` and its own dependencies. The formula engine crate is added later, by T016, once it is chosen
- [X] T002 [P] Create the module skeleton `src/agents/tools/office/mod.rs`, `src/agents/tools/office/docx.rs`, `src/agents/tools/office/xlsx.rs` (module docs only, pure functions to be filled in later) and declare `mod office;` in `src/agents/tools/mod.rs`
- [ ] T003 [P] Create the fixtures in `src/agents/tools/office/fixtures/`: `table.docx` saved by Word or LibreOffice (heading, bold and italic text, nested bullet list, numbered list, an external link, a table) because research R3 notes the probe only read a file written by `docx-rs` itself; `mixed.xlsx` (two sheets, text, number, boolean, date, empty cell, a formula with a saved value, and a sheet whose data starts at `C5`); `formulas.xlsx` (a formula with a saved value, and a formula with no saved value, as a script-made file has, for quickstart step 4); `legacy.xls` (at least one sheet with text and numbers). This needs a real office application, so a person creates and commits these files

**Checkpoint**: The crate builds with the new dependencies and an empty `office` module.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: What every story needs before tools can read or write session files.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T004 Add `file_manager: FileManager` to `AgentToolContext` in `src/integrations/kind.rs`
- [X] T005 [P] Fill `file_manager` from `deps.file_manager` in `tool_context()` in `src/agents/agent/mod.rs` (about line 2020)
- [X] T006 [P] Update the test constructor `ctx()` in `src/integrations/mod.rs` so the existing tests build with the new field
- [X] T007 [P] Add the catalogue icon `assets/integrations/office.svg` and register it wherever the other addon icons (`brave_search`, `discord`, `telegram`, `mcp`) are embedded and served
- [X] T008 Implement the shared types in `src/agents/tools/office/mod.rs` per `data-model.md`: MIME constants (DOCX, XLSX, legacy XLS), read-output constants (`READ_MAX_CHARS` 200 000, sheets 20, rows 1 000, columns 100), the `Cell` enum (text, number, bool, `{"date": ISO}`, null, `{"error": ...}`, and the read-only formula form `{"value", "formula"}`) with its JSON form, and the output structs `DocxRead`, `WorkbookRead`, `WorkbookCsv`, `WriteResult`
- [X] T009 [P] Unit tests for the shared types in `src/agents/tools/office/mod.rs` (a `Cell` round-trips through JSON in every variant)

**Checkpoint**: Foundation ready. Tools can be given a `FileManager`, and the shared types exist.

---

## Phase 3: User Story 1 - Agent reads a Word or Excel file with full fidelity (Priority: P1) 🎯 MVP

**Goal**: `office_docx_read_as_md` returns Markdown with headings, emphasis, nested lists, pipe tables and links. `office_xlsx_read_as_cells` returns typed cells per sheet for `.xlsx` and legacy `.xls`, with sheet and range filters (a block or a single cell), the data area and first-cell address per sheet, an information-only mode (`info`), formula text on request (`formulas`) and a `truncated: true` marker; it never calculates. `office_xlsx_read_as_csv` previews every sheet as CSV text (FR-030) with the formulas shown as text or, with `executed_formula: true`, as calculated results.

**Independent Test**: Install the addon by hand on a dummyplug agent, upload the fixtures, call the read tools (quickstart steps 3 and 4). Every table cell and typed value appears, `[table content]` does not, one CSV read shows the whole workbook, and calculated formulas agree with the saved results.

### Tests for User Story 1

- [X] T010 [P] [US1] Tests for `docx::to_markdown` in `src/agents/tools/office/docx.rs`: the `table.docx` fixture gives the heading, bold and italic marks, nested bullet and numbered lists, `[text](url)` with the real URL (resolved from `docx.hyperlinks` by relationship id, research R3), every table row as a pipe table, no `[table content]`; `max_chars` cuts the text and sets `truncated`
- [X] T011 [P] [US1] Tests for `xlsx::read_workbook` in `src/agents/tools/office/xlsx.rs`: `mixed.xlsx` keeps number, boolean, date (as `{"date": ISO}`), empty and text cell types across two sheets; the `sheet` and `range` (A1 notation, including a single cell such as `C7`) filters; `used_range`, `dimensions` and `start` for a sheet whose data starts at `C5` (and for a `range`, `start` is the range's top-left); `info` returns only name, `used_range` and `dimensions`; a formula cell returns its saved value, `formulas` returns `{value, formula}`, and a formula cell with no saved value is named in `note` (`formulas.xlsx`); an unknown sheet errors with the list of valid sheets; the 20-sheet, 1 000-row and 100-column caps set `truncated` and report the real `dimensions`; `legacy.xls` reads like an `.xlsx`; a corrupt file errors
- [X] T012 [P] [US1] Tests for `xlsx::read_csv` in `src/agents/tools/office/xlsx.rs` (calculation off): every sheet by default and `sheet` and `range` (block and single cell) filters; RFC 4180 quoting for commas, quotes and line breaks; dates ISO, booleans `true`/`false`, blanks empty, errors as text; `used_range`, `start` and `formula_cells` (including text that merely begins with `=`); formula cells as their formula text; the caps set `truncated` with a `note`

### Implementation for User Story 1

- [X] T013 [P] [US1] Implement `docx::to_markdown(bytes, max_chars)` in `src/agents/tools/office/docx.rs` with `docx-rs`: paragraph style to heading level, bold and italic runs, list items from `numberingProperty` plus `docx.numberings` (bullet versus decimal, level for nesting), external hyperlinks looked up in `docx.hyperlinks`, tables as pipe tables; images, shapes, text boxes and footnotes skipped (R3)
- [X] T014 [P] [US1] Implement `xlsx::read_workbook(bytes, mime, sheet, range, info, formulas)` in `src/agents/tools/office/xlsx.rs` with `calamine` (`Xlsx` and `Xls` chosen by media type): typed `Cell` values, dates as ISO-8601, formulas as their saved value (never calculated) or, with `formulas`, as `{value, formula}`, a `note` for formula cells without a saved value, A1 range parsing (block or single cell), `used_range` and `start` as A1 addresses (calamine's used range does not begin at `A1`, so the offset must be carried), the `info` mode, output caps and `truncated`
- [X] T015 [US1] Implement `xlsx::read_csv(bytes, mime, sheet, range, executed_formula)` in `src/agents/tools/office/xlsx.rs` for `executed_formula` false (formula text from calamine, one CSV per sheet, `formula_cells`), sharing the range, address and cap logic of `read_workbook` (T014); `executed_formula` true returns an explicit "not available yet" note until T017 lands
- [ ] T016 [P] [US1] **Trial the formula engine before building the calculation (research R10; blocks T017).** Try `formualizer`, `IronCalc` and `logisheets-rs` on the checked-in workbook fixtures (`mixed.xlsx`, `formulas.xlsx`): does each load them and calculate every formula cell; how often does the result equal the value saved in the file; what happens with an unsupported function (error or silent wrong number); the time and binary size it adds; whether it is pure Rust and builds for a static `musl` target (`cross build`, plan Constitution Check IV); whether `.xls` is covered; and the time and size limits to use for calculating. Record the numbers in `research.md` R10, pick one, set the target for SC-011, and add the crate to `Cargo.toml`. If none qualifies, stop and report to the reviewer instead of building T017
- [ ] T017 [US1] Add `executed_formula: true` to `xlsx::read_csv` with the engine chosen in T016, kept inside `src/agents/tools/office/formula.rs` (the only file that uses the engine crate): formula cells written as calculated results; a cell that cannot be calculated (unsupported function, circular reference, time or size limit) falls back to its saved value and is named in `note`; `.xls` is never calculated and `note` says so; no link to another file is followed; calculation is bounded in time and size and any failure, including a panic, is a result and never fails the turn. Tests: results equal the saved values on the fixtures within the target set in T016, one unsupported function falls back with a note, a circular reference does not hang
- [X] T018 [US1] Implement the `OfficeDocxRead`, `OfficeXlsxRead` and `OfficeXlsxReadCsv` tools (`VizierTool`, names `office_docx_read_as_md`, `office_xlsx_read_as_cells` and `office_xlsx_read_as_csv`) in `src/agents/tools/office/mod.rs`: resolve the session file, load its bytes through the `FileManager`, call the pure modules, and return errors the way other tools do (`VizierError`, which a script sees as a catchable error and which never ends the turn); a wrong file type names the tool to use instead; descriptions state the limits and what is skipped, say when to use the typed tool and when the CSV tool, and explain each parameter so an agent can use them without a tutorial
- [X] T019 [US1] Implement `OfficeKind` in `src/integrations/kinds/office.rs` (id `office`, family Addon, listing name "Office" with a short description and icon `office`, no fields, `validate` accepts `{}` and null, `contribute` returns the read tools with `addon: Some("office")`); add `pub mod office;` to `src/integrations/kinds/mod.rs` and register it in `IntegrationKinds::builtin()` in `src/integrations/mod.rs`
- [X] T020 [US1] Add `office_docx_read_as_md`, `office_xlsx_read_as_cells` and `office_xlsx_read_as_csv` to the pinned `EXPECTED` and `ADDON` tables in `src/agents/tools/mod.rs` (about lines 1490 to 1570) and confirm `every_addon_tool_is_namespaced_by_its_addon` passes
- [X] T021 [P] [US1] Unit tests for `OfficeKind` in `src/integrations/kinds/office.rs`: no fields, `missing()` is empty, `validate` accepts `{}` and null, `contribute` returns only `office_*` tools

**Checkpoint**: US1 works on its own: with the addon installed by hand, an agent reads Word and Excel files with their structure and previews a whole workbook as CSV. Everything except `executed_formula: true` (T016 and T017) can be reviewed first.

---

## Phase 4: User Story 2 - Agent creates a Word or Excel file for the person (Priority: P1)

**Goal**: `office_docx_write_from_md` builds a `.docx` from Markdown; `office_xlsx_write_from_sheets` builds a `.xlsx` from sheets of rows. Both store the file as a conversation file with the OOXML media type and return `{filename, size}`; the agent delivers it with `send_attachment`.

**Independent Test**: Quickstart steps 2 and 4: write a document and a workbook, send the document as an attachment, open both in Word or LibreOffice, and read them back.

### Tests for User Story 2

- [x] T022 [P] [US2] Tests for `docx::from_markdown` in `src/agents/tools/office/docx.rs`: headings, bold and italic, nested lists, a table, a link and a `title` survive a write then `to_markdown` round-trip; list numbering starts at 2 so it does not collide with the writer's default numbering 1 (research R4)
- [x] T023 [P] [US2] Tests for `xlsx::write_workbook` in `src/agents/tools/office/xlsx.rs`: text, number, boolean, `{"date": ...}` and null round-trip through `read_workbook`; two sheets; the header row is bold; `=1+1`, `+x` and `@y` read back as text and the sheet XML has no formula element (FR-028); an invalid sheet name returns an ordinary error

### Implementation for User Story 2

- [x] T024 [P] [US2] Implement `docx::from_markdown(markdown, title)` in `src/agents/tools/office/docx.rs` with `pulldown-cmark` and the `docx-rs` writer: heading styles, paragraphs, bold, italic and inline code, ordered and unordered nested lists numbered from 2, pipe tables, hyperlinks; images and raw HTML degrade to their text; `title` becomes a Title paragraph and the document title
- [x] T025 [P] [US2] Implement `xlsx::write_workbook(sheets)` in `src/agents/tools/office/xlsx.rs` with `rust_xlsxwriter`: every string through the string API (never `write_formula`), numbers, booleans and dates with a date format, null as blank, bold and frozen first row, column widths fitted to content
- [x] T026 [US2] Implement `write_session_file(...)` in `src/agents/tools/office/mod.rs`: normalise the filename (basename only, append `.docx` or `.xlsx` when missing, reject path separators and control characters), `file_manager.upload`, then `storage.save_session_file` with the explicit OOXML media type, returning `{filename, size}`; no existing-file check, so a same-named file is replaced like `image_generate` (FR-027); unit tests for the normalisation and for replacement
- [x] T027 [US2] Implement the `OfficeDocxWrite` and `OfficeXlsxWrite` tools (names `office_docx_write_from_md` and `office_xlsx_write_from_sheets`) in `src/agents/tools/office/mod.rs`; each description states: it generates a new file and never edits an existing one (and why), a same-named file is replaced so use a new name for a version of an existing file, deliver the result with `send_attachment` and the returned filename, and compose long text first with the scratchpad tools (FR-006, FR-007, FR-027)
- [x] T028 [US2] Add the two write tools to `OfficeKind::contribute` in `src/integrations/kinds/office.rs` and to the pinned `EXPECTED` and `ADDON` tables in `src/agents/tools/mod.rs`

**Checkpoint**: US1 and US2 work: an agent can read, create and deliver Office files.

---

## Phase 5: User Story 3 - Every agent has the capability without setup (Priority: P1)

**Goal**: New agents get the addon at creation and their first running instance already offers its tools; existing agents get it once through a persisted flag; the seed is a system action that still goes through the integration service.

**Independent Test**: Quickstart steps 0 and 1: create an agent and list its tools; start the new binary against a database that has an older agent, which then has exactly one `office` integration and one `created` event.

### Tests for User Story 3

- [X] T029 [P] [US3] Tests for `seed_default` in `src/integrations/service.rs`: it creates the integration with its `integration_event` row, a second call returns `Ok(None)` (the `addon_installed` conflict counts as success), it needs no permission, and the actor is the agent's owner or `Requester::User("system")` when there is none
- [X] T030 [P] [US3] Tests for the startup backfill in `src/dependencies.rs`: each existing agent is seeded once and the flag is set; a second run adds nothing, so an addon removed in between stays removed; one failing agent does not stop the others and leaves the flag unset

### Implementation for User Story 3

- [X] T031 [US3] Add `seed_default(deps, agent_id, owner, kind_id, name)` to `src/integrations/service.rs`: look up the kind, validate with `{}`, then run the same save, notification and `publish` steps as `create` (move that shared tail into one private function used by both), with no access check
- [X] T032 [US3] Call `seed_default` from `finish_create` in `src/agents/mod.rs` after the CORE seed and before `self.start(...)`, logging a failure with `tracing::warn!` and never failing creation; call `self.refresh_integrations(&agent_id)` once after `start`, because the change event fires before the agent has a process entry (research R13)
- [X] T033 [US3] Add the `OFFICE_DEFAULT_SEEDED` key and `seed_office_defaults()` to `src/dependencies.rs`: build `deps` first, run the pass at the end of `VizierDependencies::new` only when the key is absent, set the key only after every agent was handled, log per-agent failures; confirm that publishing the change event with no subscribers is harmless (research R14)
- [ ] T034 [US3] Run the real binary against a workspace that already has an agent from the previous version and confirm the quickstart step 0 and 1 results, including that no backfill runs on the second start

**Checkpoint**: US1 to US3 give the full P1 value: every agent can read and create Office files without setup.

---

## Phase 6: User Story 4 - Turning the addon off, or removing it, sticks (Priority: P2)

**Goal**: The addon can be turned off and on from the Addons page, or removed, and nothing turns it back on; neither change restarts the agent or aborts a turn.

**Independent Test**: Quickstart steps 6a and 6b: turn it off, restart, update the agent, turn it on; remove it and repeat.

### Tests for User Story 4

- [X] T035 [P] [US4] Test that a disabled `office` integration contributes no tools and an enabled one does, in `src/integrations/mod.rs` (or the closest existing composition test) (written, not yet compiled)
- [ ] T036 [P] [US4] Test that flipping `enabled` through the service publishes `IntegrationChanged` and the agent's tools recompose without a restart (extend the existing update tests in `src/integrations/service.rs` with an `office` case)
- [X] T037 [P] [US4] Test that an agent update (`finish_update` in `src/agents/mod.rs`) and a second run of the startup pass never add or duplicate the addon when it is disabled or removed (written, not yet compiled; the `finish_update` half holds because `seed_default` is called only from `finish_create` and the startup pass)

### Implementation for User Story 4

- [X] T038 [US4] Verify, and fix if needed, that disabling and removing leave the agent running and that nothing in `src/agents/mod.rs` or `src/dependencies.rs` re-adds the addon; no new feature code is expected because the server already supports `enabled` (verified by reading: the only callers of `seed_default` are `finish_create` and `seed_office_defaults`; no code change needed)
- [ ] T039 [US4] **[web]** In the `webui` submodule, check that the installed-addon card in `webui/app/routes/integrations-addons.tsx` shows the on/off switch and a sensible on or ready state for an addon with no fields (the "reading" state from `cardState` in `webui/app/lib/integrations.ts` is wrong for an offline addon); separate PR
- [ ] T040 [P] [US4] **[web]** Add `office` to `TAKEN_CATEGORIES` in `webui/app/lib/integrations.ts` and, optionally, the addon's tools to `ADDON_TOOLS`; separate PR
- [ ] T041 [US4] **[web]** After the `design` and `webui` pull requests merge, bump their pins in vizier in the order design → webui → vizier (see CLAUDE.md)

**Checkpoint**: A person can turn the addon off and on, and removal stays removed.

---

## Phase 7: User Story 5 - Word and Excel reading lives in one place (Priority: P2)

**Goal**: `read_document_file` stops converting Word and Excel to text and points to the office tools instead; text and PDF are unchanged.

**Independent Test**: Quickstart step 5: give `read_document_file` a `.docx`, an `.xlsx` and an `.xls` and get the pointer message; give it a text file and a PDF and get the same result as before.

### Tests for User Story 5

- [X] T042 [P] [US5] Tests for `extract_text` in `src/agents/tools/session_files/mod.rs`: the three Word and Excel media types return the message naming `office_docx_read_as_md` and `office_xlsx_read_as_cells` and the Office addon (install or turn on); a plain text file and a PDF read exactly as before

### Implementation for User Story 5

- [X] T043 [US5] In `src/agents/tools/session_files/mod.rs`, remove the DOCX and XLSX arms of `extract_text` (and their `docx_rs` and `calamine` use), return the pointer message for `.docx`, `.xlsx` and `.xls`, and update the tool's `description()` to say Word and Excel files are read with the `office_*` tools
- [X] T044 [P] [US5] Update the other places that mention the old behaviour: the description of `read_image_file` in `src/agents/tools/read_image/mod.rs` (about lines 58 to 63), the attached-files hint in `src/schema/request.rs` (about line 280), and the `docx/xlsx` claim in `docs/src/configuration/tools-embedding.md` (line 19)

**Checkpoint**: One reader per format, and the removal of Word and Excel from the built-in reader is explicit.

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Documentation, end-to-end verification, build gates and the commit.

- [X] T045 [P] Add a docs page for the addon in `docs/src/configuration/office-addon.md` (the five tools and when to use each, cell and range reads, the CSV preview and what `executed_formula` does and where it falls back, create-only writes, same-named files are replaced, on by default and how to turn it off) and link it from `docs/src/SUMMARY.md`
- [X] T046 [P] Reconcile `specs/034-office-addon/contracts/office-tools.md` with the implemented error and output shape: the contract describes errors as `status: "error"` objects and outputs with a `status` field, while tools following `image_generate` return `{filename, size}` and raise a catchable error on failure
- [ ] T047 Run the whole dummyplug script in `specs/034-office-addon/quickstart.md` (steps 0 to 8 including 6a and 6b) against the real binary and the standalone dummyplug server (`just dummyplug`; this branch already contains vizier-lab/vizier#103) and fix what it finds; optionally automate it as `scripts/office-e2e.mjs` in the style of `scripts/dummyplug-vizier-e2e.mjs`
- [ ] T048 Open the generated `.docx` and `.xlsx` in Microsoft Office and in LibreOffice and confirm there is no repair prompt (SC-004); needs a person with those applications
- [ ] T049 Run `cargo clippy` and `cargo test` (including `every_addon_tool_is_namespaced_by_its_addon` and `every_tool_is_in_its_category`) and fix every finding
- [ ] T050 Build one static target from `Cross.toml` (for example `cross build --target x86_64-unknown-linux-musl`) with the final dependencies, including the formula engine chosen in T016, to confirm cross-compilation still works (constitution Principle IV)
- [X] T051 Prepare the text of a checklist entry for tracker issue #82 covering the startup backfill (`seed_office_defaults` and its flag) and what depends on it (FR-025); the maintainer posts it, since it is public (text in `issue-82-entry.md`)
- [ ] T052 Commit with a conventional message `feat(office)!:` and a `[**breaking**]` note that `read_document_file` no longer reads Word and Excel and that every agent now has the Office addon by default (research R16); the changelog is generated from it

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies.
- **Foundational (Phase 2)**: needs Phase 1; blocks every story.
- **US1 (Phase 3)**: needs Phase 2. It creates `OfficeKind` (T019), which US2 to US4 build on.
- **US2 (Phase 4)**: needs Phase 2 and `docx.rs` and `xlsx.rs` from US1 (same files); its tests round-trip through the US1 readers.
- **US3 (Phase 5)**: needs T019 (the kind exists) so there is something to seed; independent of the tools themselves.
- **US4 (Phase 6)**: needs T019; the web tasks need no Rust.
- **US5 (Phase 7)**: needs only Phase 2; it is independent of the addon code and can run in parallel with US1 to US4.
- **Polish (Phase 8)**: needs the stories you want shipped. T047 needs the `dummyplug/` submodule checked out (`just install`; dummyplug is already on this branch, #103).

### User Story Dependencies

- **US1**: first, no story dependency.
- **US2**: after US1 (shared files and round-trip tests).
- **US3**: after T019.
- **US4**: after T019, ideally after US3 so seeding exists to test against.
- **US5**: independent.

### Within Each Story

Tests first (they fail), then the pure format modules, then the tools, then registration, then the checkpoint check. Inside US1: T015 follows T014 (it shares the range and cap logic and the file); T017 needs both T015 and T016 and is the only task that depends on the engine, so T018 to T021 can finish without it; T018 follows T014, T015 and T017 for its calculation parameter.

### Parallel Opportunities

- Setup: T002 and T003.
- Foundational: T005, T006, T007 after T004; T009 with T008 done.
- US1: T010, T011 and T012 together; then T013 and T014 together (different files); T016 (the engine trial) can run at any time after T003 and does not touch the code; T015 after T014; T017 after T015 and T016; T021 with T019 done.
- US2: T022 and T023 together, then T024 and T025 together.
- US3: T029 and T030 together.
- US4: T035, T036 and T037 together; T039 and T040 in the web repository.
- US5 runs alongside any other story.
- Polish: T045 and T046.

---

## Parallel Example: User Story 1

```text
# Tests together:
Task: "T010 Tests for docx::to_markdown in src/agents/tools/office/docx.rs"
Task: "T011 Tests for xlsx::read_workbook in src/agents/tools/office/xlsx.rs"
Task: "T012 Tests for xlsx::read_csv in src/agents/tools/office/xlsx.rs"

# Then the pure modules, and the engine trial alongside:
Task: "T013 Implement docx::to_markdown in src/agents/tools/office/docx.rs"
Task: "T014 Implement xlsx::read_workbook in src/agents/tools/office/xlsx.rs"
Task: "T016 Trial the formula engine candidates on the fixtures (research.md R10)"
```

---

## Implementation Strategy

### MVP First (User Story 1)

1. Phase 1 and Phase 2.
2. Phase 3 (US1) without the calculation: T010 to T015 and T018 to T021.
3. **Stop and check**: install the addon by hand on a dummyplug agent and read the fixtures (quickstart steps 3 and 4). `executed_formula: true` says it is not available yet.
4. Add the calculation (T016 then T017) when the engine trial has chosen a crate; if no candidate qualifies, report it to the reviewer and keep the setting out.

### Incremental Delivery

1. US1 gives reading with structure and a whole-workbook preview.
2. US2 adds writing, so an agent can read and create Office files.
3. US3 makes both available to every agent with no setup. US1 to US3 are all P1 and together form what the issue asks for.
4. US4 and US5 follow as P2; US5 is a breaking change and can be reviewed on its own.
5. Polish, then the single `feat(office)!:` commit.

### Parallel Team Strategy

After Phase 2: one person on US1 then US2 (they share `docx.rs` and `xlsx.rs`), one on the formula engine trial (T016), one on US3, one on US5, and the web tasks in the `design` and `webui` repositories.

---

## Notes

- [P] tasks touch different files and have no dependency on an unfinished task.
- A task that edits `src/agents/tools/office/mod.rs` is never parallel with another that edits the same file.
- Tests are written to fail before the code they cover.
- No size limits are implemented beyond the read-output caps (owner's decision) and the calculation limits set in T016; do not add input or write limits.
- Do not add any behaviour for a failed seed beyond a `warn!` log (owner's decision).
