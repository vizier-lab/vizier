# Implementation Plan: Office Addon Phase 2 (PDF)

**Branch**: `feat/office-pdf` | **Date**: 2026-11-02 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/044-office-pdf/spec.md`

## Summary

Add four tools to the existing `office` addon: `office_pdf_read_as_md`, `office_pdf_render_page_as_png`, `office_pdf_write_from_html`, `office_pdf_write_from_docx`. They ride on the `pdf-agent` 0.1.0 crate (pure Rust: `pdf_oxide` reader, `hayro` rasteriser, `fulgur` HTML-to-PDF). Word-to-PDF reuses the existing Word-to-Markdown reader, renders the Markdown to HTML with `pulldown-cmark` (already a dependency) and hands it to the same HTML writer.

**State on the branch**: the code is already written (commit `4b0158e`): `office/pdf.rs` (pure functions), four tool structs in `office/mod.rs`, registration in `integrations/kinds/office.rs`, the pinned tool tables in `agents/tools/mod.rs`, `PDF_MIME`/`PNG_MIME`, the `documents` feature gaining `dep:pdf-agent`, and a docs paragraph. This plan therefore describes the design the code is held to and lists the **gaps found between the code and the spec** (see Gap list), which become the tasks.

## Technical Context

**Language/Version**: Rust 2024 workspace (as the rest of `vizier-core`)

**Primary Dependencies**: `pdf-agent = 0.1.0` (new, optional, behind the existing `documents` feature; brings 189 new locked packages, see research R1); existing `pulldown-cmark`, `docx-rs`, `image` (transitive)

**Storage**: none new. Inputs and outputs are session files through the existing `load` / `write_session_file` helpers; no table, migration or marker

**Testing**: `cargo test -p vizier-core` (unit tests in `office/pdf.rs`, round-trip test in `office/mod.rs`, the pinned tool tables in `agents/tools/mod.rs`); `cargo test -p vizier-core --no-default-features` must still build. End-to-end: dummyplug agent steps in [quickstart.md](quickstart.md)

**Target Platform**: Linux, macOS, Docker (server) and Linux/macOS/Windows (desktop app), same as the `documents` feature today

**Project Type**: library crate inside a Rust workspace (agent tools)

**Performance Goals**: a read, render or write of a typical (≤ 50 page) document completes well inside the 600 s default tool timeout; no tool blocks the async runtime for the duration of a render (see Gap G5)

**Constraints**: offline; no new service or credential; output bounded (200 000 chars); library-imposed input limits are inherited (256 MB PDF, 25 megapixels per rendered page, 5 MB of HTML)

**Scale/Scope**: 4 tools, 1 new module, 0 schema changes

## Constitution Check

*Gate evaluated before research; re-checked after design.*

| Principle | Verdict | Notes |
|---|---|---|
| I. Lean by default | **Pass, with a recorded cost** | One new dependency, justified: nothing in the tree rasterises a PDF or lays out HTML into one; `pdf-extract` only extracts plain text. The cost is real (189 locked packages, larger binary), so it is measured and recorded (R1, task T020) rather than assumed small. No new trait, flag or config |
| II. Trait-based extensibility | **Pass** | Four `VizierTool` impls contributed by the existing `OfficeKind::contribute`; no `match` added to dispatch. The `Format` enum in `office/mod.rs` gains a `Pdf` variant used only for the wrong-tool message, which is the existing pattern for Docx/Xlsx |
| III. Zero-dependency runtime | **Pass** | Pure Rust, no sidecar, no network, nothing fetched at runtime (R2) |
| IV. Portability | **Gate, verify** | Pure Rust should cross-compile, but the constitution makes a broken `Cross.toml` target blocking. Verify musl x86_64, aarch64 gnu, and the Windows desktop leg (T021). `app/` has its own lockfile, which must stay in step (CLAUDE.md, CI `policy`) |
| V. Errors and observability | **Pass** | Every failure is `VizierError`; `unwrap`/`expect` only in tests; no `println!` |
| E2E gate (dummyplug) | **Required** | Agent-observable tools. [quickstart.md](quickstart.md) scripts the steps |
| Conventional commits | **Note** | Work commits use `feat:`; the Spec Kit auto-commit message the hook produced (`doc: add specification`) already complies |

**Post-design re-check**: unchanged. The only open gate is IV's cross-build evidence.

## Project Structure

### Documentation (this feature)

```text
specs/044-office-pdf/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   └── office-pdf-tools.md
├── checklists/requirements.md
└── tasks.md              # /speckit-tasks, not created here
```

### Source Code (repository root)

```text
Cargo.toml                                              # workspace dep pdf-agent (done)
crates/vizier-core/Cargo.toml                           # documents feature gains dep:pdf-agent (done)
crates/vizier-core/src/agents/tools/office/pdf.rs       # pure functions: to_markdown, render_page, from_html, from_docx (done; hardening below)
crates/vizier-core/src/agents/tools/office/mod.rs       # four tool structs, Format::Pdf, wrong_tool (done; descriptions below)
crates/vizier-core/src/agents/tools/session_files/mod.rs # PDF_MIME, PNG_MIME (done)
crates/vizier-core/src/integrations/kinds/office.rs     # listing text + contribute() (done)
crates/vizier-core/src/agents/tools/mod.rs              # pinned tables (done)
docs/src/configuration/office-addon.md                  # table rows (done; limits and caveats below)
CHANGELOG.md                                            # entry (todo)
fixtures/office/                                        # sample PDFs (todo, FR-023)
```

**Structure Decision**: extend the existing `office` module in place. A second addon or a separate `pdf` integration kind was rejected: the spec requires one addon, one switch, no backfill (FR-017), and `OfficeKind::contribute` already returns the whole toolset.

## Gap list (code on the branch versus the spec)

| # | Gap | Spec | Resolution |
|---|---|---|---|
| G1 | Tool descriptions omit what the spec requires: that nothing edits an existing PDF, the render tool's scale default/range and `output_filename`, the Word-to-PDF caveats, `.pdf` extension being added, "choose a new name to keep the original" | FR-011, FR-013, FR-022 | Rewrite the four descriptions; pin their key phrases in a test as 036 did |
| G2 | The read schema says `max_chars` "default 100000" but the code passes `None` to the library when absent, so the real default is whatever `pdf-agent` uses | FR-003, FR-004 | Resolve in R3: either pass the documented default explicitly or document the library's. Test that the omitted case is bounded |
| G3 | HTML-to-PDF "no outside resource, no script" is true by the library's design (R2) but unproven here | FR-009, SC-009 | Add a test: HTML with a remote `<img>`, a `<link>`, a `file:///` reference and a `<script>` produces a PDF with none of them resolved, and no connection is attempted |
| G4 | `from_docx` feeds Markdown through `pulldown_cmark`, which passes raw HTML found in the document's text through to the page as markup | FR-010, SC-002 | Escape or drop raw HTML events in `from_docx` so Word text such as `<b>` or `a < b` renders literally; test it |
| G5 | `pdf::*` run synchronously inside `async fn call`; rasterising or laying out a large document holds a runtime worker | Performance | Run the four calls in `spawn_blocking` unless the existing docx/xlsx tools already do something equivalent (check; match it) |
| G6 | Fixtures and tests: only generated PDFs are exercised; no committed sample for tables, scanned, encrypted, corrupt-in-the-middle or multi-page input; the password path is untested | FR-023, SC-001, SC-005, SC-006 | Add `fixtures/office/*.pdf`, compiled into the tests like 036's |
| G7 | No changelog entry, no limits section in the docs, no quickstart | FR-022, FR-024 | Write them |
| G8 | Out-of-range page and `page_start` beyond the end are inconsistent: render errors, read may return an empty slice | FR-014, SC-005 | Make both state the page count in a readable error or note; test both |
| G9 | Spec FR-006 says "a bounded range" for scale; code clamps silently to 0.1–4.0 | FR-006 | Keep the clamp, say so in the description and in the result (`scale` echoed), so the agent knows what it got |

## Complexity Tracking

No constitution violation to justify. The one cost worth a line is the dependency weight under Principle I, handled as a measured and recorded cost (R1), not a violation.
