# Feature Specification: Agent memory in the database

**Feature Branch**: `070-memory-in-database`

**Created**: 2026-11-28

**Status**: Draft

**Input**: User description: "ok let's move it to db, as for user access, user can see the agent memory on webui, so doesnt really matter what format it is being saved for user. as this is pretty much a core feature, we may need a migration for existing memory in user's instances and note it in #82 for removal later"

## Context

Today an agent's memory documents (bundles of markdown notes, including the identity document `core/soul`) are stored as individual files in the instance's file store: the local workspace folder or S3-compatible object storage. The database keeps only a derived copy (titles, tags, links, passage positions, revisions). Because the files are the source of truth, ordinary reads must check the file store for documents added or removed behind Vizier's back. Each search result fetches a whole file, every turn reads the identity file, and a read can even rewrite a file. On local disk this is cheap. On object storage, each of these is a network round trip, and the cost grows with memory size and traffic.

People already view and edit memory through the WebUI and the HTTP API, never by opening files, so the storage format is not part of the product. This feature makes the database the single home of memory content and migrates every existing instance's memory into it once.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Existing memory survives the upgrade unchanged (Priority: P1)

An operator upgrades a Vizier instance that already has agents with months of memory, stored on local disk or object storage. On first start after the upgrade, every agent still has every memory document, bundle, identity, link, tag, attachment and revision history it had before. Nothing in the WebUI or in agent behaviour looks different.

**Why this priority**: Memory is what an agent knows about its people and itself. Losing or corrupting any of it on upgrade is the worst outcome this feature can have, and it applies to every existing instance.

**Independent Test**: Take a workspace with several agents and bundles (local files, and separately an S3-backed one). Upgrade, start, and compare every document's content, metadata, links, attachments and revision list with a snapshot taken before.

**Acceptance Scenarios**:

1. **Given** an instance whose memory lives in local files, **When** it starts for the first time on the new version, **Then** every document is available with identical content and metadata, and the agent's identity is unchanged.
2. **Given** an instance whose memory lives in object storage, **When** it starts for the first time on the new version, **Then** the same holds, and no memory read afterwards contacts object storage.
3. **Given** the migration was interrupted partway (crash, power loss), **When** the instance starts again, **Then** the migration finishes without duplicating or losing documents.
4. **Given** the migration already completed, **When** the instance restarts, **Then** it does not run again.
5. **Given** a file that cannot be read or parsed during migration, **When** the migration runs, **Then** the remaining documents still migrate, the failure is logged with the agent, bundle and path, and the original file is left in place for recovery.

---

### User Story 2 - Memory reads are fast regardless of where files are stored (Priority: P1)

An agent searches its memory, reads a document, follows related documents and starts a turn, which loads its identity. A person browses an agent's memory list and graph in the WebUI. None of these wait on the file store, so they are as fast on an object-storage deployment as on a local one.

**Why this priority**: This is why the feature exists. Memory is read on almost every turn and on every memory page view.

**Independent Test**: On an object-storage deployment, run memory search, document read, related-documents, memory list and a chat turn. Confirm that no request reaches object storage and that response times match a local-disk deployment of the same data.

**Acceptance Scenarios**:

1. **Given** an agent with memory, **When** it runs a memory search, **Then** the passages are returned without fetching any document from the file store.
2. **Given** a person opens an agent's memory list or graph, **When** the page loads, **Then** no file-store listing happens.
3. **Given** a document is read, **When** the read completes, **Then** nothing was written as a side effect of the read (apart from read counters).

---

### User Story 3 - Writes are all-or-nothing (Priority: P2)

When an agent or a person writes, deletes, rolls back or imports memory, the document, its links, its search passages and its revision either all change together or none of them do. A crash mid-write never leaves a document whose search index or history disagrees with its content.

**Why this priority**: Today the file write and the database update are separate steps. Keeping them consistent is what the reconciliation work on reads exists for. Making writes atomic removes that work and a class of drift bugs.

**Independent Test**: Force a failure between the content change and the index update (fault injection in tests). Confirm the document and its derived data are either both old or both new.

**Acceptance Scenarios**:

1. **Given** a memory write fails partway, **When** the document is read, **Then** it shows either the old version with its old links and passages, or the new version with new ones, never a mix.
2. **Given** a bundle import of many documents, **When** it completes, **Then** every imported document is searchable and listed immediately.

---

### User Story 4 - Bundles can still be exported and imported as markdown (Priority: P3)

A person exports a bundle from the WebUI and gets the same markdown archive they get today. They can import that archive into another agent or another instance, including archives exported by earlier versions.

**Why this priority**: Export/import is the portable, human-readable form of memory now that there are no files on disk. It already exists and must keep working, but it is not new work.

**Independent Test**: Export a bundle before the upgrade and another after. Import both into a fresh agent and compare the contents.

**Acceptance Scenarios**:

1. **Given** a bundle in the database, **When** it is exported, **Then** the archive has the same structure and content as an export from the previous version.
2. **Given** an archive exported by a previous version, **When** it is imported, **Then** every document is created as before.

---

### User Story 5 - Moving an instance to PostgreSQL carries memory with it (Priority: P3)

An operator moves an instance from SQLite to PostgreSQL with the existing offline migration command. Memory content moves with the rest of the database. It no longer needs to be copied as files.

**Why this priority**: Only a few operators do this, but the command must not silently leave memory behind once memory is no longer a set of files.

**Independent Test**: Run the offline migration on a workspace with memory (migrated or not yet migrated to the database) and verify every document on the PostgreSQL side.

**Acceptance Scenarios**:

1. **Given** a SQLite instance whose memory is already in the database, **When** it is migrated to PostgreSQL, **Then** every document arrives with its content, metadata, passages and revisions.
2. **Given** a SQLite instance from before this feature, whose memory is still in files, **When** it is migrated to PostgreSQL, **Then** the memory still arrives, either by first running the file-to-database migration or by carrying the files across so the new instance migrates them on first start.

### Edge Cases

- A document over today's size ceiling (written before limits existed): it migrates as-is. Limits apply only to later writes, as today.
- Two files that normalize to the same address (e.g. differing only in a trailing slash or separator): the migration keeps one, logs the other, and leaves both files in place.
- Bundle-level files that are not concept documents (a bundle's `index` and `log`): they migrate too if they carry content people or agents read today. Otherwise they are skipped and named in the log.
- A document whose frontmatter cannot be parsed: logged and skipped (Story 1, scenario 5). Its file is kept.
- An agent deleted while its memory was still in files, leaving orphan files with no agent row: not migrated. The files are left alone.
- Someone hand-edits a memory file on disk after the upgrade: the edit is not picked up. The database is the only source.
- An agent whose identity document was deleted: it still renders from the template with the existing note, as today.
- The file store is unreachable at first start (object storage down): the migration does not mark itself complete. The instance reports the problem clearly, and the migration retries on the next start. Memory it has not migrated yet is not presented as empty without a warning.
- Memory attachments (links to artifacts, legacy session files): preserved exactly, still counted in the artifact reference index.

## Requirements *(mandatory)*

### Functional Requirements

**Storage**

- **FR-001**: The database MUST be the single source of truth for memory document content (body and frontmatter fields) on both supported database backends.
- **FR-002**: Memory reads (list, detail, related, graph, search, identity load, bundle list) MUST NOT read from or list the file store.
- **FR-003**: Search results MUST return passage text without loading whole documents from outside the database.
- **FR-004**: A memory read MUST NOT rewrite the document or its links. Only access counters may change on read.
- **FR-005**: A write, delete, rollback, bundle delete or import MUST update the content, its derived data (metadata, links, broken-link marks, passages, artifact references) and its revision in one atomic operation. Semantic-index updates may still follow the commit, as today, and their failure MUST NOT fail the write.
- **FR-006**: Every existing behaviour of memory MUST be preserved: addressing by `(bundle, path)`, the reserved `core` bundle and `core/soul`, size limits and warnings, links and broken-link detection, tags, attachments, version history and rollback, read counts, the eight `memory_*` tools, the HTTP routes and their response shapes, the deprecated `/core` projection, and dream-cycle access.
- **FR-007**: Memory MUST NOT be written to the file store any more. Bundle export MUST still produce the same markdown archive format, and import MUST still accept archives from previous versions.
- **FR-008**: Deleting an agent MUST delete all its memory content from the database in the same operation that deletes its other memory data.
- **FR-009**: The reconciliation of documents added or removed in the file store, and the per-read repair of links, MUST be removed from the read paths. The only path that turns markdown files into memory is the migration, plus import.

**Migration**

- **FR-010**: On first start after the upgrade, the system MUST migrate every agent's memory documents from the configured file store (local or object storage) into the database, on both database backends.
- **FR-011**: The migration MUST be resumable and idempotent. A document already migrated MUST NOT be duplicated or overwritten by a rerun, and completion MUST be recorded once, with a marker, so it never runs again.
- **FR-012**: The migration MUST preserve each document's content, title, tags, timestamps, links, attachments and read count exactly. Existing revision history and passages MUST stay attached to the same document.
- **FR-013**: The migration MUST NOT create new revisions. Moving storage is not an edit.
- **FR-014**: The migration MUST finish before any agent is started, so no turn reads memory that is half-migrated.
- **FR-015**: A document that fails to migrate MUST be logged with agent, bundle, path and reason. It MUST NOT stop the others, and its file MUST be kept. The migration's summary (migrated, skipped, failed counts per agent) MUST be logged.
- **FR-016**: If the file store cannot be reached, the migration MUST NOT record completion and MUST be retried on the next start.
- **FR-017**: The migration MUST NOT delete the source files. Removing them is left to the operator, and the documentation MUST say when that is safe.
- **FR-018**: The offline SQLite→PostgreSQL migration command MUST carry memory content across (User Story 5).
- **FR-019**: The migration, its completion marker, the source-file reading it needs, and any compatibility code it keeps MUST be added to the pre-1.0 removal checklist in issue #82, naming the code and what depends on it.

**Documentation**

- **FR-020**: Project and user documentation that describes memory as "markdown files on disk" MUST be updated to say memory lives in the database, can be exported as markdown, and that hand-edited files are no longer picked up.

### Key Entities

- **Memory document**: one note of one agent, addressed by `(agent, bundle, path)`. It has a body, a title, tags, links to other documents, attachments, timestamps and a read count. It now also holds its content, which used to live in a separate file.
- **Bundle**: a named group of an agent's documents. `core` is reserved and holds only the identity document `soul`. A bundle now exists because it has documents in the database, not because a folder exists.
- **Passage**: a searchable slice of a document, already stored as coordinates. Its text can now be served from the stored content.
- **Memory revision**: an existing record of every change to a document. Unchanged, and untouched by the migration.
- **Migration marker**: a one-time record that this instance's memory has been fully moved into the database.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: After upgrading, 100% of readable memory documents on a test corpus (local and object storage, at least 3 agents and 1,000 documents) are present with byte-identical content and identical metadata, links, attachments and revision lists.
- **SC-002**: A memory search, a document read, a memory list and a turn's identity load make zero requests to the file store.
- **SC-003**: On an object-storage deployment, a memory search over 1,000 documents returns at least 5× faster than before the change (median of 20 runs), and memory list and detail pages open with no wait attributable to storage.
- **SC-004**: A migration interrupted at any point and restarted ends with the same result as an uninterrupted one (no duplicates, nothing missing).
- **SC-005**: The migration of 1,000 documents from local disk finishes within 30 seconds on typical hardware, so startup after the upgrade is not noticeably delayed.
- **SC-006**: No existing memory tool, HTTP response shape or WebUI memory page changes behaviour (existing memory tests and the offline end-to-end check pass unchanged).

## Assumptions

- People access memory only through the WebUI, the HTTP API and the agent's tools, so giving up live hand-editing of files on disk is acceptable (stated by the requester).
- Memory documents are small (capped at 8 KiB for new writes), so storing them as database rows has no meaningful size or cost impact.
- The semantic vector index stays where it is (already in the database). Only the source of passage text changes.
- Source files are kept after migration as a safety net. Deleting them is a manual, documented step, not automatic.
- Other bytes in the file store (uploads, artifacts, UI pages) are out of scope and stay in the file store.
- The desktop app (SQLite + local files) gets the same migration with no extra work.
- A rollback to the previous version after the migration is not supported for memory written after the upgrade. Pre-upgrade files remain, so a downgrade sees memory as it was at upgrade time. The release notes will say so.
