# Feature Specification: Pluggable Storage

**Feature Branch**: `045-pluggable-storage`

**Created**: 2026-11-02

**Status**: Draft

**Input**: User description: "Implement GitHub issue #123: let the operator choose the database/vector backend (embedded SQLite today, or PostgreSQL with vector search) and the file backend (local filesystem today, or an S3-compatible bucket) independently, with equivalent behavior and encryption, a tested migration path, cross-backend conformance tests, and no change to the API or WebUI. It is the storage prerequisite for Vizier Fleet (#138), so it must be safe for several writers from the start even while only one process is supported."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Keep embedded storage as the default (Priority: P1)

As an operator or desktop user, I upgrade and keep running on the embedded database and local files with no new setup.

**Why this priority**: Most installs and the desktop app depend on the self-contained default; pluggable storage must not regress it.

**Independent Test**: Upgrade an existing workspace with default settings, exercise an offline agent turn, memory write/search, an upload, an artifact and a restart, and confirm everything behaves as before.

**Acceptance Scenarios**:

1. **Given** an existing workspace and no storage settings, **When** Vizier starts, **Then** it uses the embedded database and local files and all existing data is available.
2. **Given** a new install with no configuration, **When** Vizier starts, **Then** it needs no external service.
3. **Given** the desktop app, **When** it starts, **Then** it always uses embedded storage and ignores server storage settings.

---

### User Story 2 - Run on a managed database (Priority: P1)

As an operator of a server deployment, I point Vizier at a managed PostgreSQL database with vector search, and every feature works as it does on the embedded database.

**Why this priority**: Managed databases bring backups, concurrent access and capacity that an embedded file cannot, and Fleet requires one.

**Independent Test**: Start Vizier against an empty PostgreSQL database, run the full offline agent flow and the API conformance suite, and compare results with the embedded backend.

**Acceptance Scenarios**:

1. **Given** a reachable empty database with vector support, **When** Vizier starts configured for it, **Then** it creates its schema and becomes ready.
2. **Given** that database, **When** people use agents, memory search, history, tasks, jobs, notifications, usage, secrets and integrations, **Then** results are equivalent to the embedded backend.
3. **Given** protected fields (agent secrets, provider credentials, integration secrets, shell environment values), **When** they are stored, **Then** they are encrypted at rest exactly as on the embedded backend.
4. **Given** the database is unreachable or lacks vector support, **When** Vizier starts, **Then** it refuses to start with a clear error that contains no credentials.

---

### User Story 3 - Keep files in object storage (Priority: P1)

As an operator, I store uploads, session files, artifacts and memory documents in an S3-compatible bucket instead of the host disk.

**Why this priority**: Files on one host's disk block failover and Fleet; object storage is the standard answer and makes permanent artifacts practical.

**Independent Test**: Configure a bucket, upload a file, preview it with range requests, promote and replace an artifact, write and read memory, delete them, and confirm the bucket contents and the API behave as on local files.

**Acceptance Scenarios**:

1. **Given** a configured bucket, **When** a person uploads a file or an agent writes an artifact or memory document, **Then** the bytes are stored in the bucket and served through the same API.
2. **Given** a stored file, **When** a client requests a byte range, **Then** it receives exactly that range.
3. **Given** a file is deleted through any existing path, **When** no record still references it, **Then** its bytes are removed; when one still does, they are kept.
4. **Given** the bucket is unreachable, **When** an operation needs bytes, **Then** it fails clearly and no record is left pointing at missing bytes.

---

### User Story 4 - Move an existing workspace (Priority: P2)

As an operator of an existing embedded install, I move its data to the managed database and bucket with one tested command.

**Why this priority**: Without a migration path only new installs could adopt the server backends.

**Independent Test**: Populate a representative workspace, stop it, run the migration, interrupt and resume it, then start Vizier on the destination and verify every data domain and an offline agent turn.

**Acceptance Scenarios**:

1. **Given** a stopped workspace and empty destinations, **When** the operator runs the migration, **Then** all data is copied and a report of counts and checks is produced without secrets or content.
2. **Given** the migration is interrupted, **When** it is rerun, **Then** it resumes and finishes without duplicates.
3. **Given** a completed migration, **When** it is run again for the same source, **Then** it refuses unless the destination is explicitly reset.
4. **Given** any migration outcome, **When** the operator inspects the source workspace, **Then** it is unchanged and can still start.

---

### User Story 5 - Trust backend equivalence (Priority: P2)

As a maintainer, I run one storage contract test suite against every backend so behavior cannot drift.

**Why this priority**: Two backends without a shared contract diverge silently.

**Independent Test**: Run the conformance suite in CI against embedded storage, PostgreSQL and an S3 emulator; a deliberately broken backend method fails it.

**Acceptance Scenarios**:

1. **Given** every storage concern, **When** the suite runs, **Then** each backend produces the same observable results.
2. **Given** concurrent writers on the managed database, **When** they write history, revisions and guarded state changes at once, **Then** ordering and one-winner transitions hold.

### Edge Cases

- The embedding model's vector size differs from the size an existing index was created with.
- The bucket accepts writes but a later read returns a different size or checksum.
- A write commits its record but the process stops before the bytes upload, or the reverse.
- Two writers allocate the next history position or revision number at the same moment.
- A database or bucket credential is wrong, expired, or lacks a required permission.
- A stored file is larger than memory available to the process.
- The encryption key used by the destination does not match the source during migration.
- A migration source contains legacy compatibility data that earlier startup migrations would convert.
- Operators set the database backend to PostgreSQL but leave files on local disk, or the reverse.

## Requirements *(mandatory)*

### Functional Requirements

#### Selection and configuration

- **FR-001**: The default MUST remain the embedded database with local files, with no configuration required.
- **FR-002**: Operators MUST be able to choose the database/vector backend (embedded or PostgreSQL with vector search) and the file backend (local or S3-compatible) independently.
- **FR-003**: Database and object-store credentials MUST come from environment variables or credential files, never be stored in the database, and never appear in logs, errors, health output or process arguments.
- **FR-004**: Startup MUST validate the selected backends (reachability, required capabilities, permissions, compatible schema) and refuse to start with a clear, credential-free error when validation fails.
- **FR-005**: The desktop app MUST always use the embedded database and local files.
- **FR-006**: One deployment uses exactly one database backend and one file backend at a time; mixing backends for different data in one deployment is not supported.

#### Database and vector behavior

- **FR-007**: Every stored entity MUST be supported on both database backends with equivalent observable behavior through the API, the WebUI, agent tools and the CLI.
- **FR-008**: Each database backend MUST own its schema creation and upgrades, applied automatically and exactly once at startup.
- **FR-009**: Memory search MUST return equivalent passages and ranking behavior on both backends for the same documents and embedding model.
- **FR-010**: A vector index whose dimension differs from the configured embedding model MUST be refused with a clear error, as today.
- **FR-011**: History positions, revision numbers and other ordered sequences MUST stay unique and ordered under concurrent writers without relying on a process-wide lock.
- **FR-012**: State transitions that must have one winner (job reporting, task-run closing, deferred client-call claims, durable-turn updates) MUST be atomic in the database and report a lost race as such.
- **FR-013**: Field-level encryption MUST produce the same protection, envelope and record binding on both backends, and the same key MUST open data from either.

#### File behavior

- **FR-014**: Uploads, session files, artifacts (all versions) and memory documents MUST be readable and writable through one file abstraction on both file backends.
- **FR-015**: Reads MUST support streaming and byte ranges without loading whole files into memory.
- **FR-016**: Files MUST be served through Vizier's existing authenticated endpoints; direct public or pre-signed bucket links are not exposed in this feature.
- **FR-017**: A record MUST NOT reference bytes that were not verified as stored; bytes left unreferenced by a failure MUST be removable later without affecting referenced files.
- **FR-018**: Existing reference-counted deletion rules for session files and artifacts MUST hold on both file backends.

#### Migration

- **FR-019**: Operators MUST have a command that copies a stopped embedded workspace to PostgreSQL and an S3-compatible bucket, resumable after interruption, refusing a repeat import of the same source unless explicitly reset.
- **FR-020**: The migration MUST leave the source unchanged, verify counts and checksums, and produce a report free of secrets and content.
- **FR-021**: Moving data from the managed backends back to embedded storage is not required; the documented rollback is restarting the untouched source before the destination accepts writes.
- **FR-022**: Any temporary migration markers or compatibility readers MUST be tracked in issue #82.

#### Verification and documentation

- **FR-023**: One storage conformance suite MUST run unchanged against every backend combination in CI, including PostgreSQL and an S3 emulator.
- **FR-024**: Documentation MUST describe prerequisites, configuration, supported topology (one Vizier process per deployment until Fleet), backups and the migration procedure.

### Key Entities

- **Database backend**: Where relational data and vectors live; embedded or PostgreSQL with vector search.
- **File backend**: Where file bytes live; local disk or an S3-compatible bucket.
- **Stored file**: Bytes plus metadata (size, checksum, media type, state) referenced by uploads, session files, artifact versions and memory documents.
- **Schema version**: The applied schema state of a database backend, used to refuse incompatible starts.
- **Migration run**: A resumable copy from one workspace to the managed backends with source fingerprint, phase, progress and verification results.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: All existing offline end-to-end scenarios and the API conformance suite pass unchanged on the default embedded configuration.
- **SC-002**: The storage conformance suite passes 100% of its cases on embedded, PostgreSQL, local-file and S3-compatible backends.
- **SC-003**: The API conformance suite produces equivalent results on the embedded and PostgreSQL + bucket configurations for 100% of covered operations.
- **SC-004**: Under 50 concurrent writers, zero duplicate or out-of-order history positions or revision numbers, and every one-winner transition has exactly one winner across 10,000 attempts.
- **SC-005**: Memory search on the two database backends returns the same top-5 passages for at least 95% of a fixed query set over the same documents.
- **SC-006**: A representative workspace migrates with 100% of records and files verified, and an interrupted migration resumes to the same result.
- **SC-007**: A scan of logs, errors and health output during tests finds zero database or bucket credentials.
- **SC-008**: Previewing a 1 GB file by byte range keeps process memory growth under 64 MB.
- **SC-009**: The desktop app and a default install start and complete an offline agent turn with no external service.

## Assumptions

- Answers to #123's open questions follow the Vizier Fleet plan (#138): both PostgreSQL and S3-compatible files ship; credentials are env/file only; the desktop app stays embedded; the database is the vector store; only S3-compatible object storage is in scope (other clouds later).
- One Vizier process per deployment is the supported topology until Fleet lands, but every new concurrency guarantee is built to hold for several writers.
- Files are proxied through Vizier for authorization and range previews; pre-signed links can come later.
- Memory documents keep their current format and remain addressed through the existing document-store seam (also kept by spec 043-unify-core-and-memory); only where their bytes live changes.
- The legacy filesystem storage import stays as is and is not ported to the new backends.
- Migration is an offline, stopped-workspace cutover; there is no dual-write period.
- PostgreSQL with its standard vector extension and any S3-compatible service (including self-hosted emulators) are acceptable operator prerequisites.
