# Research: Office Addon (DOCX and XLSX)

All unknowns from the plan's Technical Context are resolved here. Facts about the existing code were read from the repository on branch `feat/030-office-addon` (file references are to that tree).

## R1. Where the tools get a `FileManager`

- **Decision**: add `file_manager: FileManager` to `AgentToolContext` (`src/integrations/kind.rs`) and fill it in `tool_context()` (`src/agents/agent/mod.rs:2020`) from `deps.file_manager`. Update the one other constructor, the test `ctx()` in `src/integrations/mod.rs`.
- **Rationale**: an addon's `contribute()` only sees `AgentToolContext`, which carries storage but not the blob store, so it cannot build a tool that uploads bytes. `FileManager` is `Clone`; `image_generate` and `read_image_file` already hold clones.
- **Alternatives**: giving tools the whole `VizierDependencies` (too wide, couples kinds to everything); reaching the blob store through `storage` (it is not there).

## R2. Where the format logic lives; `read_document_file` stops handling Word and Excel

- **Decision**: `src/agents/tools/office/docx.rs` and `xlsx.rs` expose pure functions on `&[u8]`/`&str` returning `crate::Result<…>`: `docx::{to_markdown, from_markdown}`, `xlsx::{read_workbook, write_workbook}`; `xlsx` opens both `.xlsx` and legacy `.xls` (`application/vnd.ms-excel`). In `extract_text` (`session_files/mod.rs`), the DOCX arm and the XLSX arm (the only code that uses `docx-rs` and `calamine` today) are removed; the three Word/Excel MIME types return a message instead: "Word and Excel files are read with `office_docx_read` / `office_xlsx_read` (Office addon; install it from the Addons page if this agent does not have it)". The text arms and the PDF arm stay untouched.
- **Rationale**: the repository owner's decision (issue #100 originally kept the arms): one reader per format, no redundancy, and no plain-text conversion that loses structure. FR-012/013 follow from it. Pure functions are unit-testable with no storage (FR-022).
- **Why the message does not depend on the addon being present**: `read_document_file` is always registered, so it cannot know cheaply whether this agent has the addon; the message therefore names both the tools and how to get them. The attachment hint in `src/schema/request.rs` (line ~280, "Use `read_document_file` for documents…") and the descriptions of `read_document_file` and `read_image_file` get the same pointer (FR-014).
- **Continuity on upgrade**: the one-time backfill (R14) gives every existing agent the addon, so no agent loses the ability to read these files when it upgrades. An agent whose addon is removed loses it — by choice.
- **Breaking change**: agents and skills that relied on `read_document_file` for Word or Excel now need the addon. The commit is `feat(office)!:` with a `[**breaking**]` note (project convention), and `docs/src/configuration/tools-embedding.md` (which says `docx/xlsx` are extracted to text) is updated.
- **Alternatives**: keep the arms and delegate to the new modules (the issue's original plan; two readers, redundant); keep two copies (violates Principle II).

## R3. DOCX read

- **Decision**: read with `docx-rs` (already a dependency) and walk the document model: paragraphs (style id → heading level, run `bold`/`italic`, numbering properties → list items with level, hyperlinks), **and `DocumentChild::Table`** (rows → cells → paragraphs) rendered as GitHub pipe tables.- **Rationale**: the defect is that the current walker ignores `DocumentChild::Table`; fixing the walk is the smallest change (spec assumption: start with the existing library, revisit with real files).
- **Known limits (documented in the tool description, not hidden)**: nested tables flatten to text; images/shapes/text boxes/footnotes are skipped; list numbering is reconstructed from numbering ids and may use a generic `-`/`1.` rendering.
- **Verified by a throwaway probe** (not in the repo): a document written with `docx-rs` 0.4.22 (heading, bold/italic, external link, bullet and numbered list, 2×2 table) and read back with `read_docx`. Results: (a) **hyperlink targets are available** — the paragraph's hyperlink carries only a relationship id (`rid`), the URL is in `docx.hyperlinks` as `(rid, target, "External")`, so the reader looks the `rid` up there; an anchor link (`HyperlinkData::Anchor`) has no URL; (b) headings come through as paragraph `style: "Heading1"`; (c) bold and italic as `runProperty`; (d) a list item is a paragraph with `numberingProperty {id, level}`, and `docx.numberings` maps `id → abstractNumId → levels[].format` (`"bullet"` vs `"decimal"`), which is enough to tell bullet from numbered; (e) tables read as rows → cells → paragraphs → runs, with merge fields (`gridSpan`, `verticalMerge`) present.
- **Not verified**: the probe read a file `docx-rs` wrote itself, not one saved by Word or LibreOffice. A list that Word defines through a paragraph style (for example "List Bullet") rather than a direct `numberingProperty` may not be recognised as a list. Fixtures in the tests should include a file saved by Word or LibreOffice, and the limit goes into the tool description if it holds.
- **Alternatives**: a raw `word/document.xml` walker over `quick-xml` (more fidelity, more code; revisit only if real files show gaps — recorded as an assumption in the spec); LibreOffice conversion (violates Principle III).

## R4. DOCX write

- **Decision**: parse Markdown with `pulldown-cmark` and build the document with `docx-rs`' writer: headings (H1–H6 → built-in heading styles), paragraphs, bold/italic/inline code, ordered/unordered lists with nesting, GFM tables, hyperlinks, thematic breaks. The optional `title` becomes a Title-style first paragraph and the document core title. Unsupported constructs (images, raw HTML) degrade to their text.
- **Rationale**: Markdown is the agent's natural authoring format and the same format `office_docx_read` returns, which is what makes the round-trip requirement (SC-002) testable.
- **Writer quirk found by the probe**: `Docx::new()` already contains a default numbering with id 1 (a 9-level decimal list), so a list definition added with id 1 produces duplicates (two abstract numberings and two numberings with id 1). Number the lists the writer adds from 2 upwards.
- **Dependency note**: `pulldown-cmark 0.13.4` is already in `Cargo.lock` via `text-splitter`'s `markdown` feature, so promoting it to a direct dependency adds no new code to the build.
- **Alternatives**: a bespoke Markdown subset parser (more bugs, not leaner); `comrak` (a second Markdown crate, rejected by the dependency-weight rule).

## R5. XLSX read

- **Decision**: `calamine` for both `Xlsx` and `Xls` (open by sniffing the file's media type; `application/vnd.ms-excel` selects `Xls`). Return, per sheet, `{name, rows}` where each cell is a tagged value (`string`, `number`, `bool`, `date`, `error`, or empty/null). Dates come back as ISO-8601 strings (via calamine's `DateTime`/`as_datetime`), tagged `date`, so a date serial is never exposed raw. Formulas are read as their cached value (calamine returns values). `sheet` selects one sheet; `range` accepts A1 notation (`B2:F40`) and is applied before truncation.
- **Rationale**: calamine is already a dependency and supports `.xls`; typed cells fix the "everything is a string" defect.
- **Errors**: an unknown sheet or range returns the list of valid sheet names in the error text (spec edge case).
- **Alternatives**: parse OOXML by hand (no), a different reader crate (no).

## R6. XLSX write

- **Decision**: new dependency `rust_xlsxwriter` (pure Rust, no system libraries, musl-safe). Cell mapping from JSON: string → text, number → number, boolean → boolean, `{ "date": "<ISO-8601>" }` → real date with a date format, `null` → blank. Header (first) row bold, column widths auto-fit from content (bounded), freeze the header row.
- **Formula safety (spec FR-028)**: every string is written with the writer's *string* API (`write_string`), never `write`/`write_formula`, so `=`, `+`, `-`, `@` prefixes cannot become formulas. A test asserts that `=1+1` reads back as the string `=1+1` and the cell has no formula in the XML.
- **Rationale**: calamine cannot write; `rust_xlsxwriter` is the maintained pure-Rust writer and the issue already names it.
- **Alternatives**: hand-rolled OOXML via the `zip` crate already in the tree (shared strings, styles, content types — far more than "a few lines"); `umya-spreadsheet` (heavier, edit-oriented — we are create-only by decision).

## R7. Limits (constants in `office/mod.rs`)

- **Read output — from the issue**: text ≈ 200 KB (`max_chars` may lower it, never raise it above the cap); capped number of sheets and rows; cut output sets `truncated: true` and a note on what was cut. The issue gives ~200 KB and says "cap sheets/rows"; the sheet/row/column numbers are ours: sheets ≤ 20, rows ≤ 1 000 per sheet, columns ≤ 100 per row.
- **Nothing else is limited** — decided by the repository owner for this proof of concept: no cap on input file size, uncompressed size or write size, because validation is easy to add later and awkward to remove. Long text for a document is composed first with the scratchpad tools (`write_tmp`, `append_tmp`, `read_tmp`), then passed to the write tool; a single `execute_python` script is itself limited to 64 KiB.
- **File-format limits are not ours**: Excel's own limits (sheet name rules, cell text length, row and column counts) are enforced by the writer library and come back as ordinary tool errors (FR-010).
- **Rationale**: keep tool results small enough for the model's context (issue). The contract (FR-009) is that cutting a read is always signalled.

## R8. Input handling on read (no extra guard in v1)

- **Decision**: no input guard beyond what the libraries do. Dropped after discussion: size checks on the file and on its uncompressed size, a cap on ZIP entry count, an explicit `spawn_blocking`, and a special message for password-protected files — none is in the issue, and the owner prefers adding validation later to removing it now. A password-protected or non-OOXML file fails to open and returns the ordinary "not a valid Office file" error (FR-010).
- **Risk accepted**: a very large or hostile `.docx`/`.xlsx` is read into memory by `docx-rs`/`calamine` with nothing in front of it. Revisit if it shows up in practice.
- **Why no explicit `spawn_blocking`**: every `office_*` tool is reached from an `execute_python` script (code mode), and the sandbox already runs the whole script on a `spawn_blocking` thread (`src/sandbox/runtime.rs:75`); a script's tool calls are driven on that same thread through `Handle::block_on` (`runtime.rs:326`). So synchronous parsing in `docx-rs`/`calamine` already runs off the async worker threads. (An earlier draft of this note said parsing would run on the async executor; that was wrong.) The office tools are not in `DREAM_TOOL_NAMES`, the only path that calls tools outside the sandbox.
- **Remaining limit**: a synchronous parse cannot be interrupted by the per-tool timeout, because the timeout is checked only when the future yields. With no size caps, nothing bounds a parse's duration or memory in v1 (see the risk above).

## R9. Writing session files: same flow as the other file tools (FR-027)

- **Decision**: a shared helper in `office/mod.rs`, `write_session_file(ctx, storage, file_manager, filename, mime, bytes)`:
  1. normalise the name (basename only; append `.docx`/`.xlsx` if the extension is missing; reject path separators/control characters);
  2. `file_manager.upload(&name, bytes)`, then `storage.save_session_file(&ctx.session, &name, MIME, size, &file.id)`, passing the OOXML media type **explicitly**.
  The helper returns `{filename, size}`; the agent delivers with `send_attachment` (no auto-attach).
- **Rationale**: the issue says the write tools "follow `image_generate`" (build bytes, `file_manager.upload`, `save_session_file` with the OOXML MIME passed explicitly, return the filename). That flow does not check for an existing file: `save_session_file` (`storage/sqlite/session_file.rs`) is `INSERT OR REPLACE` keyed `session/filename`, so a write replaces a file of the same name, exactly like `image_generate`, `tts_generate` and `load_path_as_session_file` (whose description says so: "An existing session file with the same name is replaced"). FR-027 keeps that behaviour and makes it explicit; it adds no ownership check, no `overwrite` parameter and no schema or storage change.
- **Known difference from the other tools**: `filename` is required for `office_*_write` (as in the issue), whereas `image_generate`/`tts_generate` default to a UUID name. Collisions with a person's upload are therefore likelier here. Mitigation in v1 is the tool description only: it states that a file of the same name is replaced and tells the agent to choose a new name for a version derived from an existing file.
- **Deferred (separate issue)**: a policy for same-name files across *all* session-file writers, including two people uploading the same name in a shared Discord/Telegram channel (the later upload replaces the earlier one and the agent is not told). Candidate designs are listed under Alternatives. Not part of this feature.
- **MIME constants**: DOCX `application/vnd.openxmlformats-officedocument.wordprocessingml.document`; XLSX `application/vnd.openxmlformats-officedocument.spreadsheetml.sheet`; legacy `application/vnd.ms-excel` (read only).
- **Alternatives** (all deferred to a separate cross-tool issue, see "Deferred" below): refuse with a suggested free name; save under an automatically unique name; ask the person first and replace only on an explicit `overwrite` flag. Each is a better policy than silently replacing, but each changes behaviour that every session-file writer shares (uploads, `image_generate`, `tts_generate`, `load_path_as_session_file`), so it is designed once for all of them rather than only for `office_*`.

## R10. PDF (phase 2) — designed for, not built

- **Decision**: reserve the `office_pdf_*` names and keep `office/pdf.rs` out of this change. When built: PDF tools are runtime-detected and return a clear "no PDF engine installed" error until an operator enables one, so the default image stays slim and Principle III holds. Nothing in the phase 1 tool or settings contract changes when PDF lands (FR-026).
- **Rationale**: the Dockerfile has Python and Node but no LibreOffice/Chromium/poppler; every credible engine is a heavy native dependency. Out of scope here by decision of the issue.

## R11. The addon kind

- **Decision**: `src/integrations/kinds/office.rs` — `OfficeKind` implementing `IntegrationKind`: `id() = "office"`, family `Addon`, `listing()` = name "Office", short description, icon `"office"`, **`fields: vec![]`**; `validate` accepts any settings object (empty `{}` or `null`); `missing()` defaults to empty so it contributes immediately; `contribute()` returns the four tools with `addon: Some("office")`. Register in `IntegrationKinds::builtin()`; add `assets/integrations/office.svg`. All four tool names start with `office_` (enforced by `every_addon_tool_is_namespaced_by_its_addon`).
- **Rationale**: identical shape to `BraveSearchKind`; the catalogue, API, redaction and the "one per agent" rule (`409 addon_installed`) already work for a kind with no fields.
- **Display name**: "Office" (the id stays `office`; renaming to "Documents" later is presentation-only — spec assumption).
- **Also**: add the new tool names to the pinned `EXPECTED`/`ADDON` tables in `src/agents/tools/mod.rs` (FR-024). Not added to `DREAM_TOOL_NAMES` (dreams are unattended memory work; no Office files there).

## R12. `seed_default` — a system write through the integration service

- **Decision**: add `integrations::service::seed_default(deps, agent_id, owner: Option<&str>, kind_id, name) -> ServiceResult<Option<Integration>>`. It takes **no `Actor`/access check** (it is a system action — FR-018), but otherwise mirrors `create`: look up the kind, `validate` with `{}`, build the `Integration` (settings `{}`, enabled, `creator` = actor requester), call `storage.save_integration(…, Some(agent_id), &[draft(EventAction::Created)])` (which writes the `integration_event` row in the same transaction), call `notifications.integration_event(…)`, and `publish` `IntegrationChanged`. The event actor is `Requester::User(owner_id)` when the agent has an owner, else `Requester::User("system")` — no new `Requester` variant (FR-019). An `addon_installed` conflict is treated as "already there" and returns `Ok(None)`.
- **Rationale**: reuses validation, event row, notification and publish; the only thing skipped is the permission check, which is exactly the requirement. To avoid duplicating `create`'s body, the part after the permission check is factored into one private function both `create` and `seed_default` call (Principle II / DRY).

## R13. Seeding a new agent

- **Decision**: in `VizierAgents::finish_create` (`src/agents/mod.rs`), after `create_agent` and the CORE seed and before `self.start(…)`, call `seed_default(deps, agent_id, config.owner_id.as_deref(), "office", "Office")`. A failure is logged with `tracing::warn!` and never fails creation. Nothing about it is specified beyond that: the addon has no setup, so the only thing that can fail is the SQLite write in `save_integration`, which is very unlikely and not worth its own behaviour (owner's decision). After `self.start(…)`, call `self.refresh_integrations(&agent_id)` once.
- **Why the explicit refresh**: `VizierAgent::new` builds its tool snapshot during the *build* (before `finish_create`), and the `IntegrationChanged` event published by the seed fires before this agent has an entry in `self.processes`, so the manager loop's refresh would find nothing to refresh. The single explicit refresh after `start` closes that gap, so the first instance has the tools without a restart (FR-015).
- **Alternatives**: seed before the build so the first snapshot already includes it (would require the integration to exist before the agent row exists — the integration's attachment references the agent — so rejected).

## R14. Backfill of existing agents and "removal sticks"

- **Decision** (as the issue specifies): a one-time pass in `src/dependencies.rs` that, for every stored agent without an `office` integration, calls `seed_default`, then sets the `StateStorage` key `office.default_seeded` (const `OFFICE_DEFAULT_SEEDED` beside `USAGE_ACCOUNTING_SINCE`). It runs at the **end of `VizierDependencies::new`**, after `Ok(Self { … })`'s fields exist: build `let deps = Self { … };`, run `deps.seed_office_defaults().await`, return `Ok(deps)`. It runs only when the key is absent, before any subsystem or agent exists (so each agent's first build already includes the addon and no refresh is needed). Because it is keyed by a persisted flag — not by "agent lacks the addon" — a later removal is never undone by a restart. A mid-pass crash is safe: the flag is set last, and a rerun skips agents that already have it (`addon_installed` conflict = success). A failure for one agent is logged with `warn!` and does not stop the others or startup; the flag is set only when every agent was handled.
- **Why the end of `new`, not the migration block**: the seed goes through the integration service, which takes `&VizierDependencies` and needs the transport and notifications. Those are created near the end of `new` (after the migration block), so the pass must run after the struct is assembled — not inside the earlier migrations that work on the raw connection. (An earlier draft of this plan placed it in `VizierAgents::new`; checking the code showed the dependencies are available at the end of `new`, so the issue's location stands and keeps all migration-style code in one file.)
- **Update never re-seeds**: `finish_update` is untouched; only `finish_create` and the flag-guarded pass seed (FR-017, spec open question 1).
- **Verify at implementation**: publishing `IntegrationChanged` at that point has no subscribers yet; `publish_integration_change` must tolerate that (a broadcast with no receivers). If it returns an error, ignore it there. The pass is migration-style code, so it is recorded on tracker #82 (FR-025).

## R15. Testing and the e2e check

- **Unit**: docx round-trip (heading, bold/italic, nested list, table, link) and a checked-in `.docx` fixture with a table; xlsx round-trip (string, number, bool, date, multi-sheet, header bold, width) and checked-in `.xlsx` and `.xls` fixtures; formula-safety test; limit/truncation tests; corrupt-file error tests.
- **Kind/catalogue**: `every_addon_tool_is_namespaced_by_its_addon` covers the prefix; extend the `EXPECTED`/`ADDON` tables; test `OfficeKind` (no fields, `missing() == []`, validate accepts `{}`).
- **Service/state**: `seed_default` against in-memory SQLite — creates the row + event, is idempotent (second call → `Ok(None)`), works without any permission, actor falls back to `"system"`; backfill test — seeds each existing agent once, sets the flag, a removed addon stays removed across a second run.
- **Write-path rule**: `write_session_file` writes over a same-named file like the other file tools do (no existing-file check); the tool descriptions say so.
- **E2E (constitution gate)**: dummyplug steps in [quickstart.md](quickstart.md); `scripts/office-e2e.mjs` may automate them like `scripts/client-tools-e2e.mjs`.
- **Optional CI check**: open generated files with LibreOffice headless to prove OOXML validity (SC-004); not a required gate because LibreOffice is not in the default CI image.

## R16. Changelog and tracker

- **Decision**: there is no hand-edited changelog; `CHANGELOG.md` is generated by `git-cliff` from conventional commits. The "changelog note" (FR-023) is therefore a `feat(office)!:` commit flagged `[**breaking**]` (it removes Word and Excel reading from `read_document_file`), with a body line saying every agent now has the addon by default. The backfill gets a checklist entry on issue #82 when the code lands. A docs page (`docs/src/…`) describes the addon, create-only writes and the removal behaviour, linked from `SUMMARY.md`.
