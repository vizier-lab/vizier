# Tasks: Agent Memory as Open Documents

**Input**: Design documents from `/specs/004-memory-open-format/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md (all present)

**Tests**: Not explicitly requested in the feature spec beyond the quickstart's manual verification and the storage layer's existing unit-test convention (`src/storage/memory.rs` already has unit tests today). Unit/integration test tasks below are included only where the contracts explicitly call out a testable behavior (collision rejection, link parsing, reconciliation) — this is not a TDD-first task list.

**Organization**: Tasks are grouped by user story (spec.md's US1–US4, priority order) plus a non-story **Backend Consolidation** phase for the filesystem-backend removal folded into this feature (FR-025) — it depends on Foundational + US1's `DocumentStore` work but isn't itself one of the four user-facing stories, so it carries no `[USx]` label, the same way Setup/Foundational/Polish don't.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: US1/US2/US3/US4 — omitted for Setup, Foundational, Backend Consolidation, and Polish
- File paths are exact and relative to the repository root

---

## Phase 1: Setup

**Purpose**: The one genuinely new external dependency this feature needs.

- [X] T001 Add the `zip` crate (read + write, deflate) to `Cargo.toml` (research.md §4) — needed only by User Story 4's export/import, added now so `cargo build` stays green while dependent phases land.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The `DocumentStore` abstraction, the Memory Graph Index schema, the revised memory schema, and a `BundleMemoryStore` that can address one concept by `(bundle, path)`. Every user story writes or reads through this.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T002 [P] Define the `DocumentStore` trait (`get`/`put`/`delete`/`list`) and implement `LocalDocumentStore` (local filesystem, `PathBuf` joins throughout, `glob`'s recursive `**` for `list`) in `src/storage/document/mod.rs`; register `pub mod document;` in `src/storage/mod.rs`. Per `contracts/document-store-trait.md`.
- [X] T003 [P] Add the `memory_node` and `memory_edge` tables to `SqliteStorage::init_schema` in `src/storage/sqlite/mod.rs`, per data-model.md's Memory Graph Index schema (composite PK `(agent_id, bundle, path)` on `memory_node`).
- [X] T004 Add a `document_store: Arc<dyn DocumentStore>` field to `SqliteStorage` in `src/storage/sqlite/mod.rs`, constructed from a new `LocalDocumentStore::new(workspace)` in `VizierDependencies::new` (`src/dependencies.rs`) and passed into `SqliteStorage::new`. Depends on T002.
- [X] T005 [P] Update `Memory`/`MemoryFrontMatter` in `src/schema/storage.rs`: drop `visibility`/`shared_to`, add `bundle: String` and `created_at: DateTime<Utc>`, rename `timestamp` → `updated_at`. Per data-model.md's Memory Concept Document.
- [X] T006 [P] Add `BundleSummary` (`name`, `concept_count`, `updated_at: Option<DateTime<Utc>>`) and `ImportReport` structs to `src/schema/storage.rs`.
- [X] T007 [P] Update `MemoryGraphNode`/`MemoryGraphEdge`/`MemoryGraph` in `src/schema/storage.rs`: drop `visibility` from `MemoryGraphNode`, add `bundle: String` and `boundary: bool`. Per data-model.md's Memory Graph entity.
- [X] T008 [P] Update `MemoryQueryParams` in `src/schema/storage.rs`: drop `visibility`, add `bundle: Option<String>`.
- [X] T009 Update the `MemoryStorage` trait in `src/storage/memory.rs`: add `bundle: Option<String>` to every method, reinterpret `slug` as a possibly multi-segment `path`, add `list_bundles`, `export_bundle`, `import_bundle`. Per `contracts/memory-storage-trait.md`. Depends on T005–T008.
- [X] T010 Create `src/storage/memory_bundle.rs` with `BundleMemoryStore`: single-concept write/read/delete addressed by `(bundle, path)`, collision rejection (FR-011), implicit bundle/subdirectory creation (FR-007/FR-008), same-bundle markdown-link + cross-bundle wikilink parsing (research.md §6), and `memory_node`/`memory_edge` upkeep on every write/update/delete. Register `pub mod memory_bundle;` in `src/storage/mod.rs`. Depends on T002, T003, T004, T009.
- [X] T011 Implement `index.md`/`log.md` generation and lazy reconciliation in `BundleMemoryStore` (`src/storage/memory_bundle.rs`), per research.md §5 and FR-017/FR-018. Depends on T010.
- [X] T012 Replace `impl MemoryStorage for SqliteStorage` in `src/storage/sqlite/memory.rs` with one that delegates every method to `BundleMemoryStore`. Depends on T010, T011.

**Checkpoint**: A single concept document can be written, read, and deleted by `(bundle, path)`, with `memory_node`/`memory_edge` kept in sync and `index.md`/`log.md` maintained. Search, cross-bundle links, graphs, and tools are not wired up yet.

---

## Phase 3: User Story 1 - Agent keeps writing, recalling, and linking its own memory without regression (Priority: P1) 🎯 MVP

**Goal**: Every memory operation the agent relies on (write, semantic search, link/follow, list, delete, read-count) works identically through bundles and documents, including cross-bundle links, with no regression versus the old row-based storage.

**Independent Test**: Have an agent write a memory (unnamed bundle → default), write a second naming a new bundle, link them same-bundle and cross-bundle, search semantically, follow the link, read it back, and confirm its read count incremented — all purely through the seven memory tools.

### Implementation for User Story 1

- [X] T013 [US1] Implement `query_memory` in `src/storage/memory_bundle.rs` (`bundle: None` = search across all bundles, reranked via `src/storage/rerank.rs` against `VizierIndexer`; `bundle: Some` narrows to one).
- [X] T014 [US1] Implement `get_all_agent_memory` and `get_filtered_memories` in `src/storage/memory_bundle.rs`, served entirely from `memory_node` (no `DocumentStore` reads on this path — contracts' behavioral contract #7).
- [X] T015 [US1] Implement `get_related_memories` and `has_incoming_links` in `src/storage/memory_bundle.rs`, resolving same-bundle markdown links and both cross-bundle wikilink forms via `memory_edge` (FR-003, FR-013), degrading to an absent result (not an error) on a broken link.
- [X] T016 [US1] Implement `get_memory_graph` in `src/storage/memory_bundle.rs` for both shapes — `bundle: None` (bundle-level: bundles as nodes, deduplicated cross-bundle edges) and `bundle: Some(name)` (that bundle's concepts as nodes plus synthetic `boundary: true` nodes/edges for outward links) — per `contracts/memory-storage-trait.md`'s concrete output-shape section.
- [X] T017 [US1] Implement `list_bundles` in `src/storage/memory_bundle.rs`, returning `BundleSummary` rows from `memory_node` grouped by bundle.
- [X] T018 [US1] Implement `increment_read_count` in `src/storage/memory_bundle.rs` (updates the document's frontmatter via `DocumentStore` and the `memory_node` cache row together).
- [X] T019 [P] [US1] Update `MemoryWriteArgs`/`MemoryWrite::call` in `src/agents/tools/vector_memory/mod.rs`: drop `visibility`/`shared_to`; add `bundle: Option<String>` and an explicit optional `path` field (multi-segment, defaulting to `slugify(title)`) for nested placement (FR-007); update the `content` field's description to teach both link forms (FR-004, FR-013).
- [X] T020 [P] [US1] Update `MemoryReadArgs`/`MemoryRead::call` in `src/agents/tools/vector_memory/mod.rs`: add `bundle: Option<String>` (omitted = search all bundles, per research.md §12).
- [X] T021 [P] [US1] Update `MemoryListArgs`/`MemorySummary`/`MemoryList::call` in `src/agents/tools/vector_memory/mod.rs`: add `bundle: Option<String>` — omitted calls `list_bundles` and returns bundle summary rows (the top-level view); named calls `get_all_agent_memory(bundle)` for that bundle's paginated concepts. Drop `visibility` from `MemorySummary`.
- [X] T022 [P] [US1] Update `MemoryDetailArgs`/`MemoryDetailOutput`/`MemoryDetail::call` in `src/agents/tools/vector_memory/mod.rs`: add `bundle: Option<String>` (omitted = agent's default bundle); drop `visibility`/`shared_to` from output, add `bundle`/`created_at`.
- [X] T023 [P] [US1] Update `MemoryFollowArgs`/`MemoryFollow::call` in `src/agents/tools/vector_memory/mod.rs`: add `bundle: Option<String>` (omitted = agent's default bundle).
- [X] T024 [P] [US1] Update `MemoryDeleteArgs`/`MemoryDelete::call` in `src/agents/tools/vector_memory/mod.rs`: add `bundle: Option<String>` (omitted = agent's default bundle).
- [X] T025 [US1] Rewrite every tool's `description()` string in `src/agents/tools/vector_memory/mod.rs` to teach bundles, nested paths, both link forms, and the browse-vs-search default distinction (`contracts/memory-tools.md`'s table and Exploration recipe). Depends on T019–T024.
- [X] T026 [US1] Replace the `## Memory` section of the boot doctrine in `src/agents/agent/system_prompt/boot.rs` with the text specified in `contracts/boot-doctrine.md`.
- [X] T027 [US1] Update `/api/v1/agents/{id}/memory` routes in `src/channels/http/api/v1/agents/memory.rs` for bundle scoping (`GET /`, `POST /`, `GET /query` gain `?bundle=`; `GET|PUT|DELETE /{slug}` and `GET /{slug}/related` become `/{bundle}/{path}` with a `{slug}`-only convenience alias into the default bundle); drop `visibility`/`shared_to` from request/response bodies. Per `contracts/http-api.md`'s "Existing routes" table (not yet the `/bundles*` routes — those are User Story 2/4).
- [X] T028 [US1] Implement `migrate_memory_to_bundles` (Part A) in `src/dependencies.rs` per `contracts/migration.md`: migrate every existing memory (sqlite `memory` table rows, and any flat files already on disk) into that agent's default bundle via `BundleMemoryStore`, dropping `visibility`/`shared_to`, preserving `read_count` and mapping the old single timestamp to both `created_at`/`updated_at`, not rewriting embedded `[[slug]]` link syntax, and resolving the `_global` special case to one designated agent with a `tracing::warn!`.
- [X] T029 [US1] Add unit tests in `src/storage/memory_bundle.rs` (and extend `src/storage/memory.rs`'s existing tests) for: `(bundle, path)` collision rejection, implicit bundle/subdirectory creation, same-bundle vs. cross-bundle link parsing, and broken-link tolerance — `contracts/memory-storage-trait.md`'s behavioral contracts #1–5.

**Checkpoint**: User Story 1 is fully functional and independently testable — the MVP.

---

## Phase 4: User Story 2 - A developer can understand what an agent remembers by reading the raw file (Priority: P2)

**Goal**: Any concept document is legible/editable in a plain text editor outside Vizier, a bundle's `index.md` shows its contents without opening every file, and the WebUI memory graph shows the bundle-level view first, descending into a bundle's concept-level view on click.

**Independent Test**: Open a memory document and a bundle's `index.md` in a plain editor; edit a document by hand and confirm the agent sees the change; open the WebUI graph and confirm the two-level navigation.

### Implementation for User Story 2

- [X] T030 [US2] Add `GET /bundles` and `GET /bundles/graph` routes, and change `GET /graph` to `GET /{bundle}/graph`, in `src/channels/http/api/v1/agents/memory.rs` — both graph routes call the same `get_memory_graph`, bundle filled in or not (`contracts/http-api.md`).
- [X] T031 [P] [US2] Update `webui/app/interfaces/types.ts`: drop `MemoryVisibility`/`shared_to` from `Memory`/`MemoryDetail`, add `bundle`/`created_at`/`updated_at`; `MemoryGraphNode` drops `visibility`, adds `bundle`/`boundary`; add a `BundleSummary` type.
- [X] T032 [P] [US2] Update `webui/app/services/vizier.tsx`: thread an optional `bundle` param through `getAllMemories`/`createMemory`/`updateMemory`/`getMemory`/`deleteMemory`/`queryMemories`/`getMemoryGraph`/`getRelatedMemories`; add `listBundles` and `getBundleGraph` calls hitting the new routes from T030.
- [X] T033 [US2] In `webui/app/routes/memory.tsx`: add a page-level "currently open bundle" state (`None` = top level) that fetches `listBundles`/`getBundleGraph` when unset and `getAllMemories(bundle)`/`getMemoryGraph(bundle)` when set, wire a click on a bundle node (or a `boundary: true` node in the concept-level graph) to open that bundle, and remove the now-gone visibility badge/field/shared-to UI from `MemoryManagement`. Depends on T031, T032.
- [X] T034 [US2] Add a unit test in `src/storage/memory_bundle.rs` asserting `index.md`'s and `log.md`'s generated content shape (listing table with path/title/tags/updated_at; chronological log entries) matches data-model.md's Index/Log Document definitions.

**Checkpoint**: User Stories 1 and 2 both work independently.

---

## Phase 5: User Story 3 - An agent's memory travels with it across deployments (Priority: P3)

**Goal**: Copying an agent's bundle directories to a fresh workspace preserves full functionality, and a copied set merges into an existing agent without id/slug collisions.

**Independent Test**: quickstart.md §3 — copy bundles to a new workspace, confirm search/list/graph parity, write a new memory there, confirm no collision.

### Implementation for User Story 3

- [X] T035 [US3] Add a test in `src/storage/memory_bundle.rs` that points a fresh `BundleMemoryStore`/`LocalDocumentStore` at a pre-populated bundle directory tree (simulating a copy from another deployment) and confirms the Memory Graph Index reconciles from the documents present with no data loss (exercises the same reconciliation path as research.md §9, not a new code path).
- [X] T036 [US3] Verify, and note in a code comment near the collision check in `src/storage/memory_bundle.rs`, that the existing `(bundle, path)` collision rejection (T010) is exactly what prevents id/slug collisions when a copied bundle is merged into an existing agent's memory — no new production code; this closes out FR-014/SC-005's collision requirement using what US1 already built.

**Checkpoint**: All three of Stories 1–3 are independently functional.

---

## Phase 6: User Story 4 - An operator exports and imports a bundle as a zip through the WebUI (Priority: P4)

**Goal**: An operator can download one bundle as a `.zip` and upload a `.zip` to bring a bundle into an agent, entirely through the WebUI.

**Independent Test**: quickstart.md §4 — export a bundle, re-import it, confirm every document/attachment/metadata matches; re-import into a colliding bundle name and confirm the skip-and-report behavior.

### Implementation for User Story 4

- [X] T037 [US4] Implement `export_bundle` in `src/storage/memory_bundle.rs`: stream a bundle's concept documents, index/log documents, and attachments (via `DocumentStore::list`/`get`) into a `.zip` using the `zip` crate (T001).
- [X] T038 [US4] Implement `import_bundle` in `src/storage/memory_bundle.rs`: validate the archive is a well-formed bundle structure before writing anything (reject malformed input atomically), detect per-concept `(bundle, path)` collisions against the destination and skip-and-report rather than overwrite, returning an `ImportReport` (FR-021).
- [X] T039 [US4] Add `GET /bundles/{bundle}/export` and `POST /bundles/import` (multipart) routes in `src/channels/http/api/v1/agents/memory.rs` per `contracts/http-api.md`'s error contract (400 before any write on a malformed zip, 404 on an unknown bundle for scoped routes).
- [X] T040 [P] [US4] Add `exportBundle`/`importBundle` calls to `webui/app/services/vizier.tsx`.
- [X] T041 [US4] Add export/import UI actions to `webui/app/routes/memory.tsx`: a per-bundle download action, and an upload dialog that prompts for a destination bundle name and displays the `ImportReport`'s skipped concepts on a collision. Depends on T040.

**Checkpoint**: All four user stories are independently functional.

---

## Phase 7: Backend Consolidation — remove the `filesystem` storage backend (FR-025, SC-014)

**Purpose**: Once memory no longer needs `FileSystemStorage` (Foundational + US1 already moved it onto `DocumentStore`), nothing else justifies keeping a second, duplicate storage backend — `SqliteStorage` already implements every other trait. Depends on Foundational + US1 (T001–T029); independent of US2–US4.

**Independent Test**: quickstart.md §5 (second half) — start the upgraded binary against a pre-existing `--storage filesystem` deployment and confirm every entity migrates with no data loss, and that `--storage filesystem` is rejected afterward.

- [X] T042 Implement `migrate_filesystem_backend_to_sqlite` (Part B) in `src/dependencies.rs` per `contracts/migration.md`: for a deployment with `config.storage == StorageConfig::Filesystem`, copy every entity `FileSystemStorage` holds (agents, tasks, sessions, users, providers, global config, dream journal/state, session file records) into `SqliteStorage` using the trait methods that already exist on both sides; log a `tracing::warn!` that the `filesystem` setting is no longer accepted going forward.
- [X] T043 Remove `StorageKind::Filesystem` from `src/cli/run.rs` (an explicit `--storage filesystem`/`VIZIER_STORAGE=filesystem` now fails fast via clap's own "invalid value" error). **Deviation from the literal task text, resolving a contradiction in `contracts/migration.md`**: `StorageConfig::Filesystem` in `src/config/storage.rs` is *kept* (with a doc comment explaining why) rather than deleted — `migration.md` itself requires it to still exist so an old `.vizier.yaml`/`VIZIER_STORAGE=filesystem` can still be *parsed* long enough for `VizierDependencies::new` to detect it and run the migration; deleting the variant would make config loading fail before migration ever runs. It is never used to construct a live backend.
- [X] T044 Simplify `VizierDependencies::new`'s backend-selection `match` in `src/dependencies.rs` to sqlite-only, and change `sqlite_conn: Option<Arc<Mutex<Connection>>>` to a non-`Option` field. Depends on T043.
- [X] T045 [P] `src/storage/fs/memory.rs` is deleted (no longer needed — Part A migration reads legacy flat memory files directly, not through a `MemoryStorage for FileSystemStorage` impl), and the blanket `impl VizierStorageProvider for FileSystemStorage {}` is removed (it can no longer hold once `MemoryStorage` is gone from it). **Deviation from the literal task text**, matching `contracts/migration.md`'s own stated ordering ("After this release, `src/storage/fs/` is deleted... Part B is the last code that ever constructs a `FileSystemStorage`" — i.e., full deletion is the *next* release's work): the other 11 non-memory trait impls in `src/storage/fs/` are kept as a read-only source for `migrate_filesystem_backend_to_sqlite` (T042/Part B), which needs them at the exact moment this release's migration runs. `FileSystemStorage` is no longer reachable via `VizierStorageProvider`/`VizierStorage` — only `dependencies.rs`'s migration constructs it directly.
- [X] T046 [P] Update `CLAUDE.md`'s "Config-less mode" section and `docker-entrypoint.sh`/README references to `VIZIER_STORAGE=filesystem` to reflect sqlite as the sole backend.
- [X] T047 Run `/speckit-constitution` to amend the Distribution & Technology Constraints section (drop the filesystem backend from the list of supported backends) — a companion governance action, not a code change in this repo pass.

**Checkpoint**: `SqliteStorage` is the only `VizierStorageProvider`; SC-014 holds.

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Final verification gates across the whole change.

- [X] T048 [P] Verify the `zip` crate requires no new `Cross.toml` pre-build steps for any existing cross-compilation target (plan.md's Target Platform constraint).
- [X] T049 Run `cargo clippy` and `cargo test` clean across the full change.
- [X] T050 Run `cd webui && npm run typecheck` clean.
- [ ] T051 Run through `quickstart.md` end-to-end (all seven sections) against a running `vizier run` instance.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately.
- **Foundational (Phase 2)**: Depends on Setup — BLOCKS every user story and Backend Consolidation.
- **User Story 1 (Phase 3)**: Depends on Foundational only. This is the MVP.
- **User Story 2 (Phase 4)**: Depends on Foundational; T030/T033 also depend on US1's route/tool work being in place to have anything bundle-shaped to display, but US2's own tasks touch different files than US1's and can start once Foundational is done.
- **User Story 3 (Phase 5)**: Depends on Foundational + US1 (it exercises US1's collision/reconciliation logic directly; it adds no new production code).
- **User Story 4 (Phase 6)**: Depends on Foundational + T001 (the `zip` dependency); independent of US2/US3.
- **Backend Consolidation (Phase 7)**: Depends on Foundational + US1 (memory must already be off `FileSystemStorage` before that struct can be deleted). Independent of US2/US3/US4.
- **Polish (Phase 8)**: Depends on all phases you choose to include being complete.

### Within Each Phase

- Foundational: schema tasks (T005–T008) can run in parallel with `DocumentStore`/schema-table tasks (T002, T003); the trait-signature update (T009) and `BundleMemoryStore` (T010–T012) are sequential on top of those.
- User Story 1: storage-layer tasks (T013–T018) touch one file (`src/storage/memory_bundle.rs`) sequentially; the six tool-arg tasks (T019–T024) touch one shared file too but are logically independent structs/impls within it, marked `[P]` for reviewability — apply them as one coordinated edit if working solo.
- Backend Consolidation: T042 → T043 → T044 → T045 is a strict chain (each removes what the last made safe to remove); T046/T047 are side documentation/governance tasks that can happen anytime after T043.

### Parallel Opportunities

- T002, T003 (Foundational) — different files.
- T005, T006, T007, T008 (Foundational) — same file (`src/schema/storage.rs`) but non-overlapping struct edits.
- T019–T024 (US1) — same file, non-overlapping tool structs.
- T031, T032 (US2) — different WebUI files.
- T040 (US4) can run alongside T037–T039 (different files, backend vs. frontend).
- T045, T046, T048 — independent cleanup/verification tasks once their own dependencies clear.

---

## Parallel Example: Foundational Phase

```bash
# Launch independent Foundational tasks together:
Task: "Define DocumentStore trait + LocalDocumentStore in src/storage/document/mod.rs"
Task: "Add memory_node/memory_edge tables to src/storage/sqlite/mod.rs::init_schema"
```

## Parallel Example: User Story 1 tool updates

```bash
# Once T013-T018 (storage layer) land, the six tool-arg updates touch independent
# structs/impls in the same file and can be reviewed as parallel edits:
Task: "Update MemoryWriteArgs/MemoryWrite::call in src/agents/tools/vector_memory/mod.rs"
Task: "Update MemoryReadArgs/MemoryRead::call in src/agents/tools/vector_memory/mod.rs"
Task: "Update MemoryListArgs/MemorySummary/MemoryList::call in src/agents/tools/vector_memory/mod.rs"
Task: "Update MemoryDetailArgs/MemoryDetailOutput/MemoryDetail::call in src/agents/tools/vector_memory/mod.rs"
Task: "Update MemoryFollowArgs/MemoryFollow::call in src/agents/tools/vector_memory/mod.rs"
Task: "Update MemoryDeleteArgs/MemoryDelete::call in src/agents/tools/vector_memory/mod.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup.
2. Complete Phase 2: Foundational (CRITICAL — blocks everything).
3. Complete Phase 3: User Story 1.
4. **STOP and VALIDATE**: run quickstart.md §1 against a real `vizier run` instance.
5. Ship if ready — the agent's own memory workflow already has no regression, in the new format.

### Incremental Delivery

1. Setup + Foundational → foundation ready.
2. User Story 1 → validate → ship (MVP).
3. User Story 2 → validate (raw-file legibility + WebUI two-level graph) → ship.
4. User Story 3 → validate (portability) → ship.
5. User Story 4 → validate (WebUI zip export/import) → ship.
6. Backend Consolidation → validate (SC-014) → ship — can land any time after US1, independent of US2–US4's order.
7. Polish → final gates, full quickstart pass.

### Suggested Order Given Dependencies

Setup → Foundational → **US1 (P1)** → **US2 (P2)** → **US3 (P3)** → **US4 (P4)** → Backend
Consolidation → Polish, matching spec.md's priority order for the four user stories and placing
the non-story consolidation phase after the memory work it depends on.

---

## Notes

- `[P]` tasks touch different files, or non-overlapping structs within one file, with no dependency on an incomplete task.
- `[Story]` labels trace every user-story task back to spec.md; Setup/Foundational/Backend Consolidation/Polish carry none, matching how those phases aren't part of the numbered user stories.
- No test-first (TDD) ordering is enforced here — this feature didn't request it — but T029/T034/T035 exist specifically because the contracts called out testable behaviors (collision rejection, link parsing, reconciliation, index/log shape) worth locking down given how sparse this codebase's test suite otherwise is.
- Commit after each task or logical group, per the repository's own conventional-commit convention (`feat:`/`fix:`/`refactor:`/`chore:`, `[**breaking**]` where applicable — FR-025 and FR-015's link-syntax change both qualify).
- Avoid: vague tasks, unnecessary same-file conflicts, and cross-story dependencies that would break a story's independent testability.
