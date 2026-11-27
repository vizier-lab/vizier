# Implementation Plan: Agent memory in the database

**Branch**: `070-memory-in-database` | **Date**: 2026-11-28 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/070-memory-in-database/spec.md`

## Summary

Memory document bodies move from the file store (local workspace or S3) into one new nullable `content` column on the existing `memory_node` table, on SQLite and PostgreSQL. `BundleMemoryStore` reads and writes rows only: the reconcile-on-read logic, the read-time relation repair, the per-read document rewrite for read counts, and the stored `index.md`/`log.md` all go. Writes run their content, graph, edge and revision steps in one transaction, and passages are indexed after commit as today. A one-time startup migration, `migration.memory_in_database_v1`, copies every agent's documents from the file store into the column before any agent starts. It can resume, never adds revisions, and keeps the source files. It goes on #82 with the code it needs. Bundle export generates the same markdown archive from the rows.

## Technical Context

**Language/Version**: Rust (workspace edition and toolchain as in the root `Cargo.toml`); no WebUI change

**Primary Dependencies**: existing only: `storage::sql` (`SqlStore`, `Connection::transaction`, `Dialect`), `rusqlite` (bundled), the PostgreSQL pool, `serde_yaml` (frontmatter parse, migration and export only), `zip` (export/import). **No new crate.**

**Storage**: SQLite (default) and PostgreSQL with pgvector. One new column `memory_node.content TEXT NULL`: SQLite through `add_column_if_missing` in its schema init, PostgreSQL through migration `0019_memory_content.sql`. The file store (`BlobStore`: local or S3) is read once, by the migration.

**Testing**: `cargo test -p vizier-core` (unit tests in `memory_bundle.rs` are rewritten to the row model; new tests for the migration, read-writes-nothing and write atomicity); `cargo test -p vizier-core --no-default-features`; `cargo clippy --workspace`. End to end: dummyplug agent steps in [quickstart.md](quickstart.md) (memory is agent-observable, so the constitution's e2e gate applies), plus a real-binary upgrade run of an existing workspace (startup migration, so the manual-verification gate applies).

**Target Platform**: Linux/macOS server and Docker, the desktop app (Windows, macOS, Linux; SQLite + local only)

**Project Type**: library engine (`vizier-core`) + server binary; storage-layer change

**Performance Goals**: memory search, read, list, bundle list and the per-turn identity load make **zero** file-store calls (SC-002); on S3, `memory_search` over 1,000 documents is ≥ 5× faster than the baseline (SC-003, research Decision 9); migration of 1,000 local documents takes < 30 s (SC-005)

**Constraints**: no lost or altered memory on upgrade (SC-001); migration resumable and idempotent; no API, tool or WebUI change (SC-006); the default path stays SQLite + local with no new service (Principle III); writes never hold a transaction across an embedding call

**Scale/Scope**: a typical instance has tens of agents and hundreds to a few thousand documents each, ≤ 8 KiB per new document. One file is rewritten (`storage/memory_bundle.rs`). Small edits go to `storage/sqlite/mod.rs`, one PostgreSQL migration, `dependencies.rs`, `SqlStorage` constructors and docs.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle / gate | Status | Notes |
|---|---|---|
| I. Lean by Default | ✅ | One column, no new table, no new trait, no new crate. Code is *removed* (reconcile, read-repair, index/log writes). |
| II. DRY via traits | ✅ | `BundleMemoryStore` stays the single implementation for both SQL backends over `SqlConn`/`Dialect`; no backend branching added. The one dialect difference (SQLite `add_column_if_missing` vs a PostgreSQL migration file) is the existing schema mechanism. |
| III. Self-contained default | ✅ | Removes a runtime dependency on the file store for memory; SQLite + local stays the default. |
| IV. Portability | ✅ | No OS-specific code. Fewer filesystem paths (the Windows blob-name rules no longer apply to memory). |
| V. Unified errors & observability | ✅ | Existing `anyhow`-at-storage / `VizierError`-above boundary unchanged; migration reports through `tracing`. |
| Distribution constraint text | ⚠️ amend | The constitution says *"human-readable memory documents now use the shared `DocumentStore`/`BlobStore` abstraction on either local or S3 storage."* This feature makes that sentence false. **A PATCH amendment** (v2.3.1) rewords it to say memory documents live in the database and export as markdown. This is a factual update, not a change of principle. It is made in the same PR, with the Sync Impact Report updated. |
| Manual verification (storage migration) | ✅ planned | quickstart §1 runs the binary on a copied pre-upgrade workspace, local and S3. |
| E2E through dummyplug | ✅ planned | quickstart §2–§4. |
| Migration code tracked on #82 | ✅ planned | FR-019; contract [migration.md](contracts/migration.md) §6 lists the entries. |
| Clippy + tests | ✅ planned | |
| Matching submodule branches | ✅ | No change in `landing` or `dummyplug`; run `just checkout 070-memory-in-database` so the branch exists in both (constitution TODO: the spec-kit hook does not). |
| Design before UI | n/a | No UI change. |

**Post-design re-check**: still passes. Design added no table, trait or dependency beyond the above. The amendment is the only open item, and it is a task.

## Project Structure

### Documentation (this feature)

```text
specs/070-memory-in-database/
├── plan.md              # this file
├── research.md          # Phase 0
├── data-model.md        # Phase 1
├── quickstart.md        # Phase 1
├── contracts/
│   ├── memory-store.md  # Phase 1: behaviour every MemoryStorage operation keeps or changes
│   └── migration.md     # Phase 1: the one-time file→database migration
└── tasks.md             # Phase 2 (/speckit-tasks)
```

### Source Code (repository root)

```text
crates/vizier-core/src/
├── storage/
│   ├── memory_bundle.rs           # rewritten: rows instead of DocumentStore; one-transaction writes;
│   │                              #   no reconcile/read-repair; export generates index.md + log.md
│   ├── sqlite/mod.rs              # memory_node.content via add_column_if_missing (#82: none — permanent column)
│   ├── sqlite/memory.rs           # trait impl: unchanged signatures
│   ├── sql/postgres/migrations/
│   │   └── 0019_memory_content.sql  # ALTER TABLE memory_node ADD COLUMN content TEXT
│   ├── sqlite/storage.rs (or wherever SqlStorage::new/with_secrets live)
│   │                              # constructors drop the DocumentStore parameter
│   ├── document/mod.rs            # unchanged; now used by the migrations only (#82)
│   └── migrate.rs                 # unchanged (copies the column generically); a test asserts it
├── dependencies.rs                # migrate_memory_into_database + marker; BlobDocumentStore
│                                  #   built for the migrations only
└── (tests) storage/memory_bundle.rs tests, dependencies.rs migration tests

docs/src/configuration/            # memory/storage pages: "memory lives in the database"
CLAUDE.md                          # Storage + Agent identity paragraphs
.specify/memory/constitution.md    # PATCH amendment (Distribution constraint)
```

**Structure Decision**: single engine crate. All change is inside `vizier-core`'s storage layer and startup. `vizier-http`, `vizier`, the WebUI and the desktop app are untouched apart from compile-through constructor changes, if any reach them.

## Phases

- **Phase 0 (done)**: [research.md](research.md) covers the call-by-call inventory, the column-not-table choice, transaction scope, index/log, removed read paths, migration design, `DocumentStore` fate, `storage migrate`, and the baseline.
- **Phase 1 (done)**: [data-model.md](data-model.md), [contracts/memory-store.md](contracts/memory-store.md), [contracts/migration.md](contracts/migration.md), [quickstart.md](quickstart.md).
- **Phase 2**: `/speckit-tasks`. Expected order:
  1. Baseline measurement (research Decision 9).
  2. Schema column, both backends.
  3. Helpers onto `&Connection`, plus transactional writes.
  4. Reads from rows, with reconcile and read-repair removed.
  5. Export/import.
  6. Drop `DocumentStore` from `BundleMemoryStore` and the `SqlStorage` constructors.
  7. Migration and marker.
  8. `write_migrated_memory` onto rows.
  9. Tests.
  10. Docs, CLAUDE.md and the constitution amendment.
  11. #82 entries.
  12. Quickstart runs and the after-measurement.

## Risks

- **Data loss on upgrade** is the one unacceptable outcome. Mitigations:
  - Source files are never deleted.
  - Rows are only filled where `content IS NULL`.
  - Unparseable documents keep their row and revisions.
  - The quickstart compares a full pre/post snapshot (SC-001).
- **S3 down at first start**: memory read from the database would look partial. The migration logs a clear error, leaves the marker unset and retries on the next start. Startup is not refused (research Decision 6).
- **Hidden readers of memory files** outside `BundleMemoryStore`: a task greps for `agents/*/memory` / `DocumentStore` users before the constructor change.
- **Unknown frontmatter keys** (`attachments` in old files): a verification task first, then decide (research Decision 2).

## Complexity Tracking

No violations to justify. The constitution text amendment is listed in the Constitution Check above.
