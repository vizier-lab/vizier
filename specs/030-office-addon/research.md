# Research: Office Addon (DOCX and XLSX)

All unknowns from the plan's Technical Context are resolved here. Facts about the existing code were read from the repository on branch `feat/030-office-addon` (file references are to that tree).

## R1. Where the tools get a `FileManager`

- **Decision**: add `file_manager: FileManager` to `AgentToolContext` (`src/integrations/kind.rs`) and fill it in `tool_context()` (`src/agents/agent/mod.rs:2020`) from `deps.file_manager`. Update the one other constructor, the test `ctx()` in `src/integrations/mod.rs`.
- **Rationale**: an addon's `contribute()` only sees `AgentToolContext`, which carries storage but not the blob store, so it cannot build a tool that uploads bytes. `FileManager` is `Clone`; `image_generate` and `read_image_file` already hold clones.
- **Alternatives**: giving tools the whole `VizierDependencies` (too wide, couples kinds to everything); reaching the blob store through `storage` (it is not there).

## R2. Where the format logic lives and how `read_document_file` shares it

- **Decision**: `src/agents/tools/office/docx.rs` and `xlsx.rs` expose pure functions on `&[u8]`/`&str` returning `crate::Result<…>`: `docx::{to_markdown, to_plain_text, from_markdown}`, `xlsx::{read_workbook, to_csv, write_workbook}`. `extract_text` in `session_files/mod.rs` calls `docx::to_plain_text` and `xlsx::to_csv` and adds `application/vnd.ms-excel` (legacy `.xls`) beside the OOXML XLSX arm. The PDF arm stays where it is.
- **Rationale**: removes the in-function duplication the issue complains about (one implementation per format) and satisfies FR-012/013: the generic reader keeps working when the addon is absent, and improves for free. Pure functions are unit-testable with no storage (FR-022).
- **Alternatives**: delete the generic arms and require the addon (breaks agents/skills that call `read_document_file`, and the addon is removable); keep two copies (violates Principle II).

## R3. DOCX read

- **Decision**: read with `docx-rs` (already a dependency) and walk the document model: paragraphs (style id → heading level, run `bold`/`italic`, numbering properties → list items with level, hyperlinks), **and `DocumentChild::Table`** (rows → cells → paragraphs) rendered as GitHub pipe tables. `to_plain_text` reuses the same walk and joins table cells with tabs/newlines instead of `[table content]`.
- **Rationale**: the defect is that the current walker ignores `DocumentChild::Table`; fixing the walk is the smallest change (spec assumption: start with the existing library, revisit with real files).
- **Known limits (documented in the tool description, not hidden)**: nested tables flatten to text; images/shapes/text boxes/footnotes are skipped; list numbering is reconstructed from numbering ids and may use a generic `-`/`1.` rendering.
- **Alternatives**: a raw `word/document.xml` walker over `quick-xml` (more fidelity, more code; revisit only if real files show gaps — recorded as an assumption in the spec); LibreOffice conversion (violates Principle III).

## R4. DOCX write

- **Decision**: parse Markdown with `pulldown-cmark` and build the document with `docx-rs`' writer: headings (H1–H6 → built-in heading styles), paragraphs, bold/italic/inline code, ordered/unordered lists with nesting, GFM tables, hyperlinks, thematic breaks. The optional `title` becomes a Title-style first paragraph and the document core title. Unsupported constructs (images, raw HTML) degrade to their text.
- **Rationale**: Markdown is the agent's natural authoring format and the same format `office_docx_read` returns, which is what makes the round-trip requirement (SC-002) testable.
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

## R7. Limits (starting values, tunable constants in `office/mod.rs`)

- **Read**: text output ≈ 200 KB (`max_chars` may lower, never raise above the cap); sheets ≤ 20; rows ≤ 1 000 per sheet; columns ≤ 100 per row; cut output sets `truncated: true` and a note on what was cut.
- **Write**: Markdown ≤ 1 MB; sheets ≤ 20; rows ≤ 10 000 per sheet; columns ≤ 100; cell text ≤ 32 767 characters (the XLSX limit); sheet names follow Excel rules (≤ 31 characters, no `[]:*?/\`), invalid names are rejected with a message naming the rule. Over-limit requests are refused with a clear error, not silently cut.
- **Rationale**: bound memory and keep a tool result small enough for the model's context; the contract (FR-009) is that cutting is signalled, the numbers are not part of the contract (spec assumption).

## R8. Untrusted-input safety on read

- **Decision**: run parsing inside `tokio::task::spawn_blocking`; before parsing an OOXML/ZIP container, reject files over a size cap (default 25 MB on disk) and archives whose declared uncompressed size exceeds a cap (default 200 MB) or whose entry count is implausible, returning a normal tool error (zip-bomb guard). Password-protected files (encrypted OOXML containers are not a ZIP) return a clear "password-protected or not a valid Office file" error.
- **Rationale**: files come from people in chat; a hostile `.xlsx` must not exhaust memory or stall the runtime (FR-010, edge cases).

## R9. Writing session files and the "own files only" rule (FR-027)

- **Decision**: a shared helper in `office/mod.rs`, `write_session_file(ctx, storage, file_manager, filename, mime, bytes)`:
  1. normalise the name (basename only; append `.docx`/`.xlsx` if the extension is missing; reject path separators/control characters);
  2. `storage.get_session_file(&ctx.session, &name)`: if a record exists and its `sender` differs from the agent id (`ctx.session.0`), return a refusal error naming the file and telling the agent to pick another name — the upload is not touched;
  3. otherwise `file_manager.upload(&name, bytes)`, then `storage.save_session_file(&ctx.session, &name, MIME, size, &file.id)`, passing the OOXML media type **explicitly**.
  The helper returns `{filename, size}`; the agent delivers with `send_attachment` (no auto-attach).
- **Rationale**: `save_session_file` (`storage/sqlite/session_file.rs`) is `INSERT OR REPLACE` keyed `session/filename` and stamps `sender = session.0` (the agent id), while uploads record the person's username as `sender` — so the existing field already distinguishes agent-created from person-uploaded files and **no schema change is needed**. A check-then-write race inside one conversation is not a security boundary (one agent turn at a time per session), so no extra locking.
- **MIME constants**: DOCX `application/vnd.openxmlformats-officedocument.wordprocessingml.document`; XLSX `application/vnd.openxmlformats-officedocument.spreadsheetml.sheet`; legacy `application/vnd.ms-excel` (read only).
- **Alternatives**: always overwrite like `image_generate` (can destroy an upload — rejected in clarification); never overwrite (blocks revising drafts — rejected in clarification).

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

- **Decision**: in `VizierAgents::finish_create` (`src/agents/mod.rs`), after `create_agent` and the CORE seed and before `self.start(…)`, call `seed_default(deps, agent_id, config.owner_id.as_deref(), "office", "Office")`. A failure is logged with `tracing::warn!` and never fails creation. After `self.start(…)`, call `self.refresh_integrations(&agent_id)` once.
- **Why the explicit refresh**: `VizierAgent::new` builds its tool snapshot during the *build* (before `finish_create`), and the `IntegrationChanged` event published by the seed fires before this agent has an entry in `self.processes`, so the manager loop's refresh would find nothing to refresh. The single explicit refresh after `start` closes that gap, so the first instance has the tools without a restart (FR-015).
- **Alternatives**: seed before the build so the first snapshot already includes it (would require the integration to exist before the agent row exists — the integration's attachment references the agent — so rejected).

## R14. Backfill of existing agents and "removal sticks"

- **Decision** (as the issue specifies): a one-time pass in `src/dependencies.rs` that, for every stored agent without an `office` integration, calls `seed_default`, then sets the `StateStorage` key `office.default_seeded` (const `OFFICE_DEFAULT_SEEDED` beside `USAGE_ACCOUNTING_SINCE`). It runs at the **end of `VizierDependencies::new`**, after `Ok(Self { … })`'s fields exist: build `let deps = Self { … };`, run `deps.seed_office_defaults().await`, return `Ok(deps)`. It runs only when the key is absent, before any subsystem or agent exists (so each agent's first build already includes the addon and no refresh is needed). Because it is keyed by a persisted flag — not by "agent lacks the addon" — a later removal is never undone by a restart. A mid-pass crash is safe: the flag is set last, and a rerun skips agents that already have it (`addon_installed` conflict = success). A failure for one agent is logged with `warn!` and does not stop the others or startup; the flag is set only when every agent was handled.
- **Why the end of `new`, not the migration block**: the seed goes through the integration service, which takes `&VizierDependencies` and needs the transport and notifications. Those are created near the end of `new` (after the migration block), so the pass must run after the struct is assembled — not inside the earlier migrations that work on the raw connection. (An earlier draft of this plan placed it in `VizierAgents::new`; checking the code showed the dependencies are available at the end of `new`, so the issue's location stands and keeps all migration-style code in one file.)
- **Update never re-seeds**: `finish_update` is untouched; only `finish_create` and the flag-guarded pass seed (FR-017, spec open question 1).
- **Verify at implementation**: publishing `IntegrationChanged` at that point has no subscribers yet; `publish_integration_change` must tolerate that (a broadcast with no receivers). If it returns an error, ignore it there. The pass is migration-style code, so it is recorded on tracker #82 (FR-025).

## R15. Testing and the e2e check

- **Unit**: docx round-trip (heading, bold/italic, nested list, table, link) and a checked-in `.docx` fixture with a table; xlsx round-trip (string, number, bool, date, multi-sheet, header bold, width) and checked-in `.xlsx` and `.xls` fixtures; formula-safety test; limit/truncation tests; oversize/corrupt-file error tests.
- **Kind/catalogue**: `every_addon_tool_is_namespaced_by_its_addon` covers the prefix; extend the `EXPECTED`/`ADDON` tables; test `OfficeKind` (no fields, `missing() == []`, validate accepts `{}`).
- **Service/state**: `seed_default` against in-memory SQLite — creates the row + event, is idempotent (second call → `Ok(None)`), works without any permission, actor falls back to `"system"`; backfill test — seeds each existing agent once, sets the flag, a removed addon stays removed across a second run.
- **Write-path rule**: `write_session_file` refuses to replace a person-uploaded file and replaces an agent-created one.
- **E2E (constitution gate)**: dummyplug steps in [quickstart.md](quickstart.md); `scripts/office-e2e.mjs` may automate them like `scripts/client-tools-e2e.mjs`.
- **Optional CI check**: open generated files with LibreOffice headless to prove OOXML validity (SC-004); not a required gate because LibreOffice is not in the default CI image.

## R16. Changelog and tracker

- **Decision**: there is no hand-edited changelog; `CHANGELOG.md` is generated by `git-cliff` from conventional commits. The "changelog note" (FR-023) is therefore a `feat(office):` commit (with a body line saying every agent now has the addon by default). The backfill gets a checklist entry on issue #82 when the code lands. A docs page (`docs/src/…`) describes the addon, create-only writes and the removal behaviour, linked from `SUMMARY.md`.
