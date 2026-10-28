# Implementation Plan: Office Addon (DOCX and XLSX)

**Branch**: `feat/030-office-addon` | **Date**: 2026-10-25 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/032-office-addon/spec.md` (GitHub issue vizier-lab/vizier#100)

## Summary

Add a built-in `office` addon (an `IntegrationKind` in the Addon family) that contributes five tools — `office_docx_read_as_md`, `office_docx_write_from_md`, `office_xlsx_read_as_cells`, `office_xlsx_read_as_csv`, `office_xlsx_write_from_sheets` — enabled by default on every agent. DOCX and XLSX parsing/writing live in pure per-format modules (`src/agents/tools/office/{docx,xlsx}.rs`, no `ToolContext`), the tools are thin wrappers over session files, and `read_document_file` stops handling Word and Excel (one reader per format): given such a file it returns a message pointing to the office tools, while text and PDF are unchanged. Tool names state the file type and the shape they produce or take (FR-001). `office_xlsx_read_as_cells` is built so an agent never has to probe: a `range` can be a block or one cell, every sheet reports `used_range`, `dimensions` and the `start` address of the returned cells (calamine's range begins at the first non-empty cell, not `A1`), `info: true` returns only those sheet facts without reading cells, and `formulas: true` adds formula text next to the saved value. `office_xlsx_read_as_csv` (FR-030) lets an agent preview every sheet in one call; its `executed_formula` setting writes formula cells as their text (false) or as calculated results (true). Only that tool calculates, which needs a formula engine crate chosen by a trial ([research.md](research.md) R10, task T049). "On by default" is one internal `integrations::service::seed_default` used by `finish_create` (new agents) and a state-flag-guarded one-time backfill (existing agents); removal is an ordinary integration delete and nothing re-seeds. Technical approach and rejected alternatives are in [research.md](research.md).

## Technical Context

**Language/Version**: Rust (edition of the existing crate); no toolchain change

**Primary Dependencies**: existing `docx-rs 0.4` (read + write), `calamine 0.36` (read; add `Xls`), `pulldown-cmark 0.13` (already in the tree through `text-splitter`'s markdown feature; promoted to a direct dependency to parse `docx_write` input). **Two new crates**: `rust_xlsxwriter` (pure Rust XLSX writer; calamine cannot write and hand-writing OOXML is not "a few lines" — see Constitution Check I) and **a formula engine for `executed_formula`**, one of `formualizer`, `IronCalc` or `logisheets-rs`, chosen by the trial in task T049 (research R10; all pure Rust, MIT or Apache-2.0, none verified yet). `calamine` covers the saved values and the formula text (formula text for legacy `.xls` is unverified, R5); `.xls` is never calculated. No new system library.

**Storage**: existing SQLite only. Reuses `integration` / `integration_event` tables, `session_file` table, `FileManager` blob store, and the `state` key/value table (`office.default_seeded`). No schema change.

**Testing**: `cargo test` (per-format unit tests with checked-in fixtures and round-trips; kind and catalogue-table tests; seed/backfill tests against in-memory SQLite); `cargo clippy`; end-to-end: agent steps scripted in [quickstart.md](quickstart.md) against the standalone dummyplug server (`just dummyplug`, spec 030-standalone-dummyplug-server; constitution e2e gate — default tool list of every agent changes). The branch needs the merged dummyplug change (vizier-lab/vizier#103) from `master` first.

**Target Platform**: Linux/macOS/Windows single binary, incl. static `musl` cross targets in `Cross.toml` (all new code is pure Rust).

**Project Type**: single Rust binary (backend only); WebUI follow-up lives in the `design`/`webui` submodules as separate PRs.

**Performance Goals**: reading/writing a typical (<5 MB) office file within a normal tool-call time.

**Constraints**: read output ≈ 200 KB of text and capped sheets/rows with `truncated: true` (FR-009, from the issue); no other size limits in this proof of concept (owner's decision, research R7); all failures are tool result data, never turn-ending errors (FR-010).

**Scale/Scope**: 5 tools, 1 integration kind, 1 seed helper, 1 one-time backfill, ~6 touched existing files, 0 new tables.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Assessment |
|---|---|
| **I. Lean by Default** | Pass with one justified dependency. `rust_xlsxwriter` is the only new crate; reason: no writer exists in the tree and a hand-rolled OOXML spreadsheet writer (shared strings, styles, content types, relationships) is not "a few lines". `pulldown-cmark` is already in `Cargo.lock`. No new trait/config flag; no second implementation introduced. The second new crate, a formula engine, exists only for `executed_formula` (reviewer's request); it is justified because evaluating formulas is not "a few lines" and is chosen by a trial against the repo's fixtures (R10, T049). If no candidate passes, `executed_formula: true` goes back to the reviewer rather than shipping with an unreliable engine. |
| **II. Trait-based extensibility (NON-NEGOTIABLE)** | Pass. Delivered as one `IntegrationKind` impl registered in `IntegrationKinds::builtin()` and five `VizierTool` impls; no `match` over kind ids added. Seeding goes through the shared integration service rather than a bespoke storage write. The two `read_document_file` arms are *collapsed into* the shared per-format modules (removing duplication, not adding it). |
| **III. Self-contained, zero-dependency runtime** | Pass. Pure Rust, no sidecar, no network, no operator step. |
| **IV. Portability** | Pass. No platform code; musl targets unaffected (pure-Rust crates). Verified by `cross build` of an existing static target in the polish phase. |
| **V. Unified errors & observability** | Pass. Pure modules return `crate::Result<T>`; conversions via `throw_vizier_error`; no `unwrap/expect` outside tests; `tracing` only. Seeding logs a `warn!` and never blocks agent creation or startup. |
| **Quality gates** | `cargo clippy`/`cargo test` required; **dummyplug e2e required** (agent-observable change) → quickstart is scripted as dummyplug steps; startup backfill touches process startup → run the real binary against a pre-existing database (quickstart US3/US4). Conventional commit `feat:` drives the changelog (`git-cliff`; there is no hand-edited changelog file). |

**Result: PASS** (no Complexity Tracking entries needed). Re-checked after Phase 1 design: unchanged — the design adds no abstraction beyond the kind, the five tools and `seed_default`.

## Project Structure

### Documentation (this feature)

```text
specs/032-office-addon/
├── plan.md              # This file
├── research.md          # Phase 0 decisions
├── data-model.md        # Phase 1: entities, state, tool I/O shapes
├── quickstart.md        # Phase 1: dummyplug e2e script
├── contracts/
│   └── office-tools.md  # Tool input/output contracts
├── checklists/requirements.md
└── tasks.md             # Phase 2 (/speckit-tasks — not created here)
```

### Source Code (repository root)

```text
src/
├── agents/
│   ├── mod.rs                       # finish_create: seed_default + refresh once after start
│   ├── agent/mod.rs                 # tool_context(): add file_manager
│   └── tools/
│       ├── mod.rs                   # EXPECTED / ADDON catalogue tables gain office_* ; `mod office;`
│       ├── office/
│       │   ├── mod.rs               # shared: limits, MIME consts, write_session_file helper (FR-027), output structs, the five VizierTool impls
│       │   ├── docx.rs              # pure: to_markdown / from_markdown
│       │   └── xlsx.rs              # pure: read_workbook(bytes, mime, sheet, range, info, formulas) / read_csv(bytes, mime, sheet, range, executed_formula) / write_workbook
│       └── session_files/mod.rs     # read_document_file: DOCX/XLSX/XLS arms removed, replaced by a message pointing at office_*; text and PDF unchanged
│       └── read_image/mod.rs        # description points at office_* (FR-014)
├── integrations/
│   ├── kind.rs                      # AgentToolContext gains file_manager
│   ├── mod.rs                       # builtin() registers OfficeKind; test ctx() gains file_manager
│   ├── service.rs                   # seed_default(deps, agent_id, owner, kind, name)
│   └── kinds/
│       ├── mod.rs                   # pub mod office;
│       └── office.rs                # IntegrationKind impl (no fields)
├── dependencies.rs                  # OFFICE_DEFAULT_SEEDED key + one-time seed_office_defaults() at the end of VizierDependencies::new
assets/integrations/office.svg       # catalogue icon
Cargo.toml                           # + rust_xlsxwriter, pulldown-cmark (direct)
tests/ or src/**/tests               # fixtures: tests/fixtures/office/{table.docx,mixed.xlsx,legacy.xls}
scripts/office-e2e.mjs               # optional scripted run against standalone dummyplug (mirrors scripts/dummyplug-vizier-e2e.mjs, which starts and stops the server)
docs/src/…                           # a short docs page for the office addon + SUMMARY.md entry; configuration/tools-embedding.md no longer says docx/xlsx are extracted to text
src/schema/request.rs                # the attached-files hint also points Word/Excel files at the office tools
```

**Structure Decision**: single-project Rust layout, following the existing addon pattern (`kinds/brave_search.rs` + `tools/brave_search`). Format logic is deliberately isolated in `tools/office/{docx,xlsx}.rs` as pure functions so it is unit-testable without storage. `mod.rs` holds the thin tool wrappers to avoid a fourth file per tool.

## Out of scope for this plan (tracked, not built)

- WebUI (the owner wants the on/off switch, FR-029): the Addons page already has an on/off switch on installed addon cards (`webui/app/routes/integrations-addons.tsx`, calling `PUT /integrations/{id}` with `enabled`), and the server already supports enable/disable, so no backend work is needed for it. Remaining: check the card for an addon with no fields and its on/ready state, `office` in `TAKEN_CATEGORIES`, optional `ADDON_TOOLS` entry → separate PRs in `design` → `webui` → vizier order.
- The target accuracy of calculated results (SC-011): set after the engine trial (T049), not before.
- Issue #82 checklist entry for the one-time backfill (FR-025), added when the code lands.
