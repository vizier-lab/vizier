# Implementation Plan: Pluggable Storage

**Branch**: `045-pluggable-storage` | **Date**: 2026-11-02 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/045-pluggable-storage/spec.md` (GitHub issue #123)

## Summary

Make the database and file backends selectable without forking storage logic. One shared SQL implementation of all 29 storage traits runs over a small synchronous connection trait with SQLite (today) and PostgreSQL + pgvector implementations; dialect differences live in a short `Dialect` list. All file bytes (uploads, artifacts, memory documents) move behind one `BlobStore` trait with local (today's paths) and hand-rolled S3 implementations reusing the existing SigV4 signer, and HTTP file serving becomes streaming. A resumable `vizier storage migrate` command moves a stopped workspace. One storage test suite runs on every backend. Defaults and the desktop app are unchanged.

## Technical Context

**Language/Version**: Rust edition 2024, toolchain 1.96

**Primary Dependencies**: existing `rusqlite` (bundled) + `sqlite-vec`, `reqwest`, `rustls`, `sha2`, `bytes`, `futures-util`, `llm::sigv4`. New: `postgres`, `r2d2`, `r2d2_postgres`, `tokio-postgres-rustls` (pure Rust). No OpenDAL, no AWS SDK, no sqlx (research R1, R2, R8).

**Storage**: SQLite + local files (default); PostgreSQL ≥ 15 with `vector` extension + S3-compatible bucket (opt-in).

**Testing**: `cargo test --workspace`; backend-parametrised storage and blob suites, PostgreSQL/MinIO enabled by `VIZIER_TEST_DATABASE_URL` / `VIZIER_TEST_S3_ENDPOINT`; CI job with `pgvector/pgvector` + `minio` services; end-to-end: dummyplug steps in [quickstart.md](quickstart.md)

**Target Platform**: existing server targets (Linux/macOS, musl via `cross`); desktop app unchanged (SQLite + local)

**Project Type**: Rust library (`vizier-core`) + HTTP adapter (`vizier-http`) + CLI (`vizier`)

**Performance Goals**: no measurable regression on SQLite (same statements, same single connection); range preview of a 1 GB file adds < 64 MB RSS; PostgreSQL pool default 8

**Constraints**: zero required external services by default (Principle III); one implementation per trait (Principle II); credentials env/file only; same encryption AAD on both backends

**Scale/Scope**: 29 traits / ≈252 methods / ≈18k lines converted to `SqlConn`; 4 byte trees onto `BlobStore`; 2 HTTP file endpoints made streaming; 1 new CLI command

## Constitution Check

*GATE: Passed before Phase 0 research. Re-checked after Phase 1 design.*

| Principle / gate | Assessment | Status |
|---|---|---|
| I. Lean by Default | `SqlConn`, `Dialect` and `BlobStore` each have two real implementations now. S3 is hand-rolled on the existing signer instead of OpenDAL/AWS SDK. PostgreSQL crates are the minimum for a sync pooled client with rustls. | ✅ |
| II. Trait-based extensibility | One implementation per storage trait over `SqlConn`; dialect differences are trait methods, never `match backend` at call sites. Byte storage is one trait. | ✅ |
| III. Self-contained runtime | Default stays bundled SQLite + local files with config-less mode; PostgreSQL/S3 are strictly opt-in and additive, as the Distribution constraint allows. | ✅ |
| IV. Portability | New crates are pure Rust; `cross`/musl unaffected. Local keys map through `BlobKey` validation (keeps today's Windows-safe segment rules). | ✅ |
| V. Unified errors & observability | Storage keeps `anyhow` internally (as today) and `VizierError` at the boundary; startup errors are fixed credential-free strings; `tracing` only. | ✅ |
| Conventional commits / matching branches | `045-pluggable-storage` exists in workspace and all submodules (`just checkout`). | ✅ |
| clippy + test; manual run; dummyplug e2e | quickstart §1–§6. | ✅ |
| #82 tracking | migration command, `storage_migration` table and the SQLite-only legacy migrations' `as_sqlite()` gate get #82 entries. | ✅ |

**Post-design re-check**: all gates pass. The only new external services are the opt-in PostgreSQL and S3 backends.

## Project Structure

### Documentation (this feature)

```text
specs/045-pluggable-storage/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── sql-layer.md
│   ├── blob-store.md
│   └── configuration-and-cli.md
├── checklists/requirements.md
└── tasks.md
```

### Source Code

```text
crates/vizier-core/src/
├── config/storage.rs            # StorageConfig::{Sqlite, Postgres}, ObjectsConfig::{Local, S3}
├── storage/
│   ├── sql/                     # NEW: SqlValue, SqlConn, SqlStore, Dialect
│   │   ├── mod.rs
│   │   ├── sqlite.rs            # SqliteConn / SqlitePool over rusqlite
│   │   ├── postgres.rs          # PgConn / PgPool over postgres + r2d2
│   │   └── postgres/migrations/NNNN_*.sql
│   ├── sqlite/ → sql_impl/      # the 29 trait impls, converted to SqlConn (module keeps its files)
│   ├── memory_bundle.rs         # over SqlStore
│   ├── blob/                    # NEW: BlobStore, BlobKey, local.rs, s3.rs
│   └── document/mod.rs          # DocumentStore over BlobStore
├── indexer/{sqlite,postgres}.rs # PgIndexer (pgvector)
├── file_manager/mod.rs          # uploads via BlobStore
├── artifacts/{mod,paths}.rs     # versions via BlobStore
├── service/files.rs             # streaming locate
├── dependencies.rs              # backend construction, as_sqlite() legacy gate
└── storage/migrate/             # NEW: sqlite+local → postgres+objects copier
crates/vizier-http/src/api/v1/{files.rs, agents/artifacts.rs}   # Body::from_stream
crates/vizier/src/cli/{run.rs, storage.rs}                       # --storage/--objects, storage migrate
app/src-tauri/src/engine.rs                                      # pin Sqlite + Local
.github/workflows/ci.yml                                         # postgres + minio job
docs/src/configuration/storage.md                                # NEW
```

**Structure Decision**: No new crate. The shared implementation keeps its per-domain files; only the connection type underneath changes.

## Phase Sequencing

1. **SQL layer** (`refactor: run storage over a backend-neutral connection`): `storage/sql` with `SqliteConn`; convert every `storage/sqlite/*.rs`, `memory_bundle.rs`, the indexer and outside-storage SQL to `SqlConn`. SQLite behavior identical; existing tests green. Parallelisable per file once `sql/mod.rs` exists.
2. **PostgreSQL backend** (`feat: add a PostgreSQL storage backend`): `PgConn`, migrations, dialect, `PgIndexer`, tsvector search, identity `seq`, config/CLI, startup validation, parametrised tests.
3. **BlobStore** (`feat: store files through a pluggable blob store`): trait, local, S3, consumers, streaming HTTP.
4. **Migration command** (`feat: migrate a workspace to PostgreSQL and object storage`).
5. **CI + docs** (`doc: document storage backends`).
