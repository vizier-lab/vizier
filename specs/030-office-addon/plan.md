# Implementation Plan: Office Addon (DOCX and XLSX)

**Branch**: `feat/030-office-addon` | **Date**: 2026-10-25 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/030-office-addon/spec.md` (GitHub issue vizier-lab/vizier#100)

## Summary

Add a built-in `office` addon (an `IntegrationKind` in the Addon family) that contributes four tools — `office_docx_read`, `office_docx_write`, `office_xlsx_read`, `office_xlsx_write` — enabled by default on every agent. DOCX and XLSX parsing/writing live in pure per-format modules (`src/agents/tools/office/{docx,xlsx}.rs`, no `ToolContext`), the tools are thin wrappers over session files, and `read_document_file` stops handling Word and Excel (one reader per format): given such a file it returns a message pointing to the office tools, while text and PDF are unchanged. "On by default" is one internal `integrations::service::seed_default` used by `finish_create` (new agents) and a state-flag-guarded one-time backfill (existing agents); removal is an ordinary integration delete and nothing re-seeds. Technical approach and rejected alternatives are in [research.md](research.md).

## Technical Context

**Language/Version**: Rust (edition of the existing crate); no toolchain change

**Primary Dependencies**: existing `docx-rs 0.4` (read + write), `calamine 0.36` (read; add `Xls`), `pulldown-cmark 0.13` (already in the tree through `text-splitter`'s markdown feature; promoted to a direct dependency to parse `docx_write` input). **One new crate**: `rust_xlsxwriter` (pure Rust XLSX writer; calamine cannot write and hand-writing OOXML is not "a few lines" — see Constitution Check I). No new system library.

**Storage**: existing SQLite only. Reuses `integration` / `integration_event` tables, `session_file` table, `FileManager` blob store, and the `state` key/value table (`office.default_seeded`). No schema change.

**Testing**: `cargo test` (per-format unit tests with checked-in fixtures and round-trips; kind and catalogue-table tests; seed/backfill tests against in-memory SQLite); `cargo clippy`; end-to-end: dummyplug agent steps scripted in [quickstart.md](quickstart.md) (constitution e2e gate — default tool list of every agent changes).

**Target Platform**: Linux/macOS/Windows single binary, incl. static `musl` cross targets in `Cross.toml` (all new code is pure Rust).

**Project Type**: single Rust binary (backend only); WebUI follow-up lives in the `design`/`webui` submodules as separate PRs.

**Performance Goals**: reading/writing a typical (<5 MB) office file within a normal tool-call time.

**Constraints**: read output ≈ 200 KB of text and capped sheets/rows with `truncated: true` (FR-009, from the issue); no other size limits in this proof of concept (owner's decision, research R7); all failures are tool result data, never turn-ending errors (FR-010).

**Scale/Scope**: 4 tools, 1 integration kind, 1 seed helper, 1 one-time backfill, ~6 touched existing files, 0 new tables.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Assessment |
|---|---|
| **I. Lean by Default** | Pass with one justified dependency. `rust_xlsxwriter` is the only new crate; reason: no writer exists in the tree and a hand-rolled OOXML spreadsheet writer (shared strings, styles, content types, relationships) is not "a few lines". `pulldown-cmark` is already in `Cargo.lock`. No new trait/config flag; no second implementation introduced. |
| **II. Trait-based extensibility (NON-NEGOTIABLE)** | Pass. Delivered as one `IntegrationKind` impl registered in `IntegrationKinds::builtin()` and four `VizierTool` impls; no `match` over kind ids added. Seeding goes through the shared integration service rather than a bespoke storage write. The two `read_document_file` arms are *collapsed into* the shared per-format modules (removing duplication, not adding it). |
| **III. Self-contained, zero-dependency runtime** | Pass. Pure Rust, no sidecar, no network, no operator step. PDF engines (which would violate this) are explicitly parked. |
| **IV. Portability** | Pass. No platform code; musl targets unaffected (pure-Rust crates). Verified by `cross build` of an existing static target in the polish phase. |
| **V. Unified errors & observability** | Pass. Pure modules return `crate::Result<T>`; conversions via `throw_vizier_error`; no `unwrap/expect` outside tests; `tracing` only. Seeding logs a `warn!` and never blocks agent creation or startup. |
| **Quality gates** | `cargo clippy`/`cargo test` required; **dummyplug e2e required** (agent-observable change) → quickstart is scripted as dummyplug steps; startup backfill touches process startup → run the real binary against a pre-existing database (quickstart US3/US4). Conventional commit `feat:` drives the changelog (`git-cliff`; there is no hand-edited changelog file). |

**Result: PASS** (no Complexity Tracking entries needed). Re-checked after Phase 1 design: unchanged — the design adds no abstraction beyond the kind, the four tools and `seed_default`.

## Project Structure

### Documentation (this feature)

```text
specs/030-office-addon/
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
│       │   ├── mod.rs               # shared: limits, MIME consts, write_session_file helper (FR-027), output structs, the four VizierTool impls
│       │   ├── docx.rs              # pure: to_markdown / from_markdown
│       │   └── xlsx.rs              # pure: read_workbook / write_workbook
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
scripts/office-e2e.mjs               # optional scripted dummyplug run (mirrors scripts/client-tools-e2e.mjs)
docs/src/…                           # a short docs page for the office addon + SUMMARY.md entry; configuration/tools-embedding.md no longer says docx/xlsx are extracted to text
src/schema/request.rs                # the attached-files hint also points Word/Excel files at the office tools
```

**Structure Decision**: single-project Rust layout, following the existing addon pattern (`kinds/brave_search.rs` + `tools/brave_search`). Format logic is deliberately isolated in `tools/office/{docx,xlsx}.rs` as pure functions so it is unit-testable without storage. `mod.rs` holds the thin tool wrappers to avoid a fourth file per tool.

## Out of scope for this plan (tracked, not built)

- PDF tools (phase 2): the tool names `office_pdf_*` and the runtime-detected-engine approach are reserved in research R10; nothing is implemented.
- WebUI (the owner wants the on/off switch, FR-029): the Addons page already has an on/off switch on installed addon cards (`webui/app/routes/integrations-addons.tsx`, calling `PUT /integrations/{id}` with `enabled`), and the server already supports enable/disable, so no backend work is needed for it. Remaining: check the card for an addon with no fields and its on/ready state, `office` in `TAKEN_CATEGORIES`, optional `ADDON_TOOLS` entry → separate PRs in `design` → `webui` → vizier order.
- Issue #82 checklist entry for the one-time backfill (FR-025), added when the code lands.
