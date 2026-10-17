---
description: "Task list for the scratchpad tools"
---

# Tasks: Scratchpad Tools

**Input**: Design documents from `specs/020-scratchpad-tools/`

**Prerequisites**: plan.md, spec.md, research.md (R1–R10), data-model.md, contracts/agent-surface.md, quickstart.md

**Tests**: included. research.md R10 plans unit tests for storage, the name rule, the category table and the dream tool list, and the constitution requires the dummyplug e2e walk.

**Organization**: tasks are grouped by user story. US1 (write/read) and US4 (isolation) are P1; US2 (append) and US3 (idle clean-up) are P2. Dream access (FR-016) is cross-cutting and sits in the Polish phase.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: the user story the task belongs to (US1–US4)

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: the category and the storage contract that everything else is written against.

- [ ] T001 [P] Add `ToolCategory::Scratchpad` in `src/agents/tools/mod.rs`. Place it after `Files` in the enum and in `ToolCategory::ALL` (now `[ToolCategory; 15]`), with `name()` = `"scratchpad"` and `description()` = `"Temporary notes private to this conversation, deleted once it goes idle"` (research R6). Fix any code or test that assumes 14 categories, e.g. the loop at the `for category in ToolCategory::ALL` test.
- [ ] T002 [P] Create `src/storage/scratch.rs` per data-model §3–§4:
  - the constants `MAX_SCRATCH_FILE_BYTES = 1_048_576`, `MAX_SCRATCH_FILES = 64` and `MAX_SCRATCH_NAME_BYTES = 128`;
  - `pub enum ScratchWrite { Written { size: u64 }, TooLarge { limit: u64, current: u64 }, TooMany { limit: usize, names: Vec<String> } }`;
  - the `#[async_trait] pub trait ScratchStorage` with `write_scratch`, `append_scratch`, `read_scratch`, `scratch_names`, `clear_scratch` and `clear_all_scratch` (all `anyhow::Result`), each with a doc comment;
  - `impl ScratchStorage for VizierStorage`, forwarding each method to `self.0`, as in `src/storage/client_call.rs`.

  Then, in `src/storage/mod.rs`, add `pub mod scratch;` and the import, and add `+ ScratchStorage` to the `VizierStorageProvider` supertrait list.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: the table, the read/clear half of the SQLite impl and the shared tool plumbing. No story works without these.

- [ ] T003 Add `pub fn init_scratch_schema(conn: &Connection) -> Result<()>` in `src/storage/sqlite/mod.rs`. It creates `scratch_file` exactly as in data-model §1: `topic TEXT NOT NULL DEFAULT ''` and `PRIMARY KEY (agent_id, channel, topic, name)`. Give it a doc comment in the style of `init_client_call_schema`, call it from `init_schema` right after `init_client_call_schema(conn)?;`, and add `mod scratch;` next to `mod client_call;`.
- [ ] T004 Create `src/storage/sqlite/scratch.rs` with `impl ScratchStorage for SqliteStorage` (depends on T002, T003). Add a private helper `fn key(session: &VizierSession) -> (&str, String, &str)` returning `(agent_id, channel.to_slug(), topic or "")` (research R2). Implement:
  - `read_scratch`: `SELECT content … WHERE agent_id, channel, topic, name`;
  - `scratch_names`: sorted by `name`;
  - `clear_scratch`: `DELETE … WHERE agent_id = ?1 AND channel = ?2 AND topic = ?3`, returning the rows deleted;
  - `clear_all_scratch`: `DELETE FROM scratch_file`.

  Leave `write_scratch` and `append_scratch` as `todo!()` stubs only until T006 and T012 replace them.
- [ ] T005 [P] Create `src/agents/tools/scratchpad.rs` with the pieces all three tools share (research R8, R9; contracts/agent-surface.md):
  - `fn valid_scratch_name(name: &str) -> bool`: 1–128 bytes of `[A-Za-z0-9._-]`, not `.` or `..`;
  - a `#[serde(tag = "status", rename_all = "snake_case")]` output enum `ScratchResult` with variants `Written { name, size }`, `Found { name, size, content }`, `NotFound { name, existing }` and `Refused { reason, name, message, limit: Option<u64>, current: Option<u64>, existing: Option<Vec<String>> }` (`skip_serializing_if = "Option::is_none"`), deriving `JsonSchema`;
  - a helper turning a `ScratchWrite` into a `ScratchResult` with the contract's messages;
  - one `const SCRATCH_RULES: &str` holding the shared description paragraph (temporary, private to this conversation, cleared once nothing is running or queued and on restart, use memory for anything lasting, never reaches a later background-job report, name rule, limits).

  Add `mod scratchpad;` in `src/agents/tools/mod.rs`.

**Checkpoint**: `cargo build` passes with the stubs. The table exists on a fresh start.

---

## Phase 3: User Story 1 - Keep working notes within a turn (Priority: P1) 🎯 MVP

**Goal**: `write_tmp` and `read_tmp` work in a turn: write, overwrite, read back, and a non-fatal not-found that lists existing names.

**Independent Test**: quickstart.md §1, §2 (without the `log` appends) and §3, the `write_tmp` refusals.

- [ ] T006 [US1] Implement `write_scratch` in `src/storage/sqlite/scratch.rs` (research R3). Under the held connection, open a transaction (`conn.unchecked_transaction()` or the pattern used elsewhere in `src/storage/sqlite/`), then:
  1. Read this name's current `size`, if any.
  2. If `content.len() > MAX_SCRATCH_FILE_BYTES`, return `TooLarge { current }` (0 when the file is absent).
  3. If the file is absent and `COUNT(*)` for the session is `>= MAX_SCRATCH_FILES`, return `TooMany { names }`.
  4. Otherwise `INSERT … ON CONFLICT(agent_id, channel, topic, name) DO UPDATE SET content = excluded.content, size = excluded.size, updated_at = excluded.updated_at`, commit, and return `Written { size }`.
- [ ] T007 [US1] Implement the `WriteTmp` and `ReadTmp` tools in `src/agents/tools/scratchpad.rs`. Each is a struct with `pub storage: Arc<VizierStorage>`, implementing `VizierTool` with category `Some(ToolCategory::Scratchpad)`, names `write_tmp`/`read_tmp`, and `Input { name: String, content: String }` / `{ name: String }` documented with `///` field docs. Write `description()` from the contract plus `SCRATCH_RULES`. `call` does the following:
  - An invalid name returns `Refused { reason: "invalid_name" }`.
  - Write calls `storage.write_scratch(&ctx.session, …)` and maps the result.
  - Read returns `Found`, or on a miss returns `NotFound { existing: scratch_names(..) }`.
  - Only storage errors become `Err(VizierError(..))`.
- [ ] T008 [US1] Register `WriteTmp` and `ReadTmp` on `default_toolset` in `BaseTools::new()` in `src/agents/tools/mod.rs` (next to `ListSessionFiles`, with `storage: deps.storage.clone()`), add `("write_tmp", Scratchpad)` and `("read_tmp", Scratchpad)` to the `every_tool_is_in_its_category` table, and add the `use` for the new types.
- [ ] T009 [P] [US1] Add unit tests in `src/storage/sqlite/scratch.rs` (`#[cfg(test)] mod tests`, in-memory connection plus `init_scratch_schema`, following `src/storage/sqlite/client_call.rs`'s tests):
  - overwrite replaces the content;
  - `None` topic upserts to one row rather than two;
  - over-1 MiB write returns `TooLarge` and leaves the old content;
  - the 65th new name returns `TooMany` with the 64 names, while overwriting an existing name still succeeds;
  - sizes are UTF-8 bytes (a multi-byte string).
- [ ] T010 [P] [US1] Add unit tests in `src/agents/tools/scratchpad.rs` for `valid_scratch_name`. Accepted: `plan`, `a.b-c_d`, a 128-byte name. Refused: `""`, `"."`, `".."`, `"a/b"`, `"../x"`, `"a b"`, a 129-byte name, `"é"` and a control character.

**Checkpoint**: an agent can write, overwrite and read scratch files within a turn. This is the MVP.

---

## Phase 4: User Story 4 - Conversations never see each other's scratch files (Priority: P1)

**Goal**: scratch files are scoped to `(agent, channel, topic)` and never cross conversations or agents.

**Independent Test**: quickstart.md §6.

- [ ] T011 [US4] Add unit tests in `src/storage/sqlite/scratch.rs`:
  - the same name written in two sessions (different topic; different channel; different agent) reads back each session's own content;
  - `scratch_names` lists only the session's own files;
  - `clear_scratch` on one session leaves the others' rows;
  - a session with `topic: None` and one with `topic: Some("x")` are distinct.

**Checkpoint**: isolation is proven at the storage layer, which is the only place the scope is decided.

---

## Phase 5: User Story 2 - Accumulate output incrementally (Priority: P2)

**Goal**: `append_tmp` adds content exactly as given, creating the file when absent, under the same limits.

**Independent Test**: quickstart.md §2 (the `log` appends) and §3 (`append_tmp` over the limit).

- [ ] T012 [US2] Implement `append_scratch` in `src/storage/sqlite/scratch.rs`, replacing the stub (research R3). In one transaction:
  1. Read the current `size`.
  2. If `current + content.len() > MAX_SCRATCH_FILE_BYTES`, return `TooLarge { current }`.
  3. If the file is absent and the session already has `MAX_SCRATCH_FILES`, return `TooMany`.
  4. Otherwise `INSERT … ON CONFLICT … DO UPDATE SET content = scratch_file.content || excluded.content, size = scratch_file.size + excluded.size, updated_at = excluded.updated_at`, so the existing content is never read into Rust, and return `Written { size: new size }`.
- [ ] T013 [US2] Add the `AppendTmp` tool (`append_tmp`) in `src/agents/tools/scratchpad.rs`, shaped like `WriteTmp` and calling `append_scratch`. Its description says content is joined with no separator. Register it on `default_toolset` and add `("append_tmp", Scratchpad)` to the category table in `src/agents/tools/mod.rs`.
- [ ] T014 [P] [US2] Add unit tests in `src/storage/sqlite/scratch.rs`:
  - append to an absent name creates it;
  - `a`, `b`, `c` reads `abc`;
  - an empty append creates an empty file and changes nothing on an existing one;
  - an append crossing 1 MiB returns `TooLarge` with the content unchanged;
  - an append to a new name at 64 files returns `TooMany`.

**Checkpoint**: all three tools work within a turn.

---

## Phase 6: User Story 3 - Scratch space spans the chat burst, then disappears (Priority: P2)

**Goal**: scratch files last while a conversation has a running or queued turn. They are cleared when it goes idle, on `/abort`, when an instance stops with nothing to hand on, at startup, and on conversation or agent deletion.

**Independent Test**: quickstart.md §4, §5, §7, §8, §9.

- [ ] T015 [US3] In `src/agents/process.rs`, add `async fn clear_idle_scratch(storage: &VizierStorage, session: &VizierSession)`. It awaits `storage.clear_scratch(session)` and logs a failure with `tracing::warn!` (never propagated). Then call it **awaited** in the `turns.join_next()` arm (research R4):
  - For a non-`current` finished turn, call it before the existing `continue` when `!session_is_busy(&turns, &session_queues, &finished.session)`.
  - For a current one, call it in the `else` (idle) branch next to `session_queues.remove(&session)`.

  Add a one-line comment saying why it is awaited inline: no turn of the session can start between "idle" and "cleared".
- [ ] T016 [US3] In `abort_session_notify` in `src/agents/process.rs`, after `abort_session_silent(...)` returns, await `clear_idle_scratch(storage, session)`. The session's turn and queue are gone at that point (research R4 point 2).
- [ ] T017 [US3] In `agent_process` in `src/agents/process.rs`, after `turns.stop_all(TURN_STOP_TIMEOUT)` and its `notify_turn_stopped` loop, but before `hand_on_queued(session_queues, &deps)`, await `clear_idle_scratch` for each stopped session that has no non-empty entry in `session_queues`. Sessions whose queue is handed on keep their scratch files (research R4 point 3). Collect the stopped sessions from the `stop_all` result first.
- [ ] T018 [P] [US3] Delete the session's rows in `delete_session` in `src/storage/sqlite/session.rs`: `DELETE FROM scratch_file WHERE agent_id = ?1 AND channel = ?2 AND topic = ?3`, with the topic mapped `None → ''`. Put it next to the `client_tool_call` delete, with a one-line comment. Also delete the agent's rows (`DELETE FROM scratch_file WHERE agent_id = ?1`) in `delete_agent` in `src/storage/sqlite/agent.rs`, next to the `client_tool_call` delete.
- [ ] T019 [P] [US3] Add `async fn clear_orphaned_scratch(storage: &VizierStorage)` in `src/dependencies.rs`, modelled on `interrupt_orphaned_client_calls`. It calls `clear_all_scratch()`, logs `tracing::info!` with the count when non-zero, and logs `warn!` on error without failing boot. Its doc comment says nothing can be running before agents start, so every row is a leftover. Call it where `interrupt_orphaned_client_calls` is called.
- [ ] T020 [P] [US3] Add unit tests in `src/storage/sqlite/scratch.rs`:
  - `clear_scratch` returns the number of rows and leaves the session empty;
  - `clear_all_scratch` empties every session;
  - a write after a clear starts fresh (count limit reset).

**Checkpoint**: idle, abort, stop, restart and deletion all clear scratch files.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: dream access (FR-016), docs, verification.

- [ ] T021 Add `"write_tmp"`, `"append_tmp"` and `"read_tmp"` under a `// Scratchpad (3)` comment to `VizierTools::DREAM_TOOL_NAMES` in `src/agents/tools/mod.rs` (research R7). Add a unit test that each name is in `DREAM_TOOL_NAMES` and resolves in `default_toolset`, following the existing tests' construction of a toolset.
- [ ] T022 [P] Add a short "Scratchpad" section to `docs/src/configuration/tools-embedding.md`: what the three tools do, the lifetime (until the conversation is idle, and cleared on restart), the limits, and that dreams get them too.
- [ ] T023 [P] Update `CLAUDE.md`'s Tools section:
  - "one of 14 variants" becomes 15, with `Scratchpad` after `Files` in the list;
  - add one paragraph: the `scratchpad` tools, the `scratch_file` table behind `ScratchStorage`, cleared by `agents/process.rs` when a session goes idle, on `/abort` and on instance stop with an empty queue, plus the startup sweep in `dependencies.rs` and the deletes in `delete_session`/`delete_agent`; included in `DREAM_TOOL_NAMES`.
- [ ] T024 Run `cargo clippy` and `cargo test`, and fix any findings in the touched files.
- [ ] T025 End-to-end check with a `dummyplug` agent on a running binary (constitution e2e gate): walk quickstart.md §0–§9 against `just run`, recording any deviation from the expected replies in the task notes.
- [ ] T026 Live-provider step (marked as such in quickstart.md §10): trigger a dream on an agent with a real model, then confirm no `DREAM__%` rows remain in `scratch_file` afterwards.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: T001 and T002 are independent and can run in parallel.
- **Foundational (Phase 2)**: T003 and T004 depend on T002. T005 depends on T001 and T002 (it uses `ToolCategory::Scratchpad` and `ScratchWrite`). Phase 2 blocks every story.
- **US1 (Phase 3)**: after Phase 2. T006 → T007 → T008; T009 needs T006; T010 needs only T005.
- **US4 (Phase 4)**: after T006, since its tests need `write_scratch`. Independent of US2 and US3.
- **US2 (Phase 5)**: after Phase 2. T012 → T013. T013 edits the same files as T007/T008, so do it after them; T014 needs T012.
- **US3 (Phase 6)**: after Phase 2. T015 → T016 → T017 (same file); T018, T019 and T020 are parallel with them and with each other.
- **Polish (Phase 7)**: T021 after T008 and T013; T022 and T023 any time after the design is fixed; T024 and T025 last; T026 after T021.

### User Story Dependencies

- **US1 (P1)**: none beyond Foundational. MVP.
- **US4 (P1)**: needs `write_scratch` from US1 (T006). It adds tests only, since isolation is a property of the key from T004.
- **US2 (P2)**: independent of US1's storage code, but shares the tool file and registration with it.
- **US3 (P2)**: independent of the tools. It can be built and unit-tested with only Phase 2 in place. Its e2e check needs US1.

### Parallel Opportunities

- T001 ∥ T002.
- T005 ∥ T003/T004.
- Within US1: T009 ∥ T010 once T006 and T005 are done.
- US3's T018 ∥ T019 ∥ T020 ∥ the `process.rs` chain T015–T017.
- T022 ∥ T023 ∥ any implementation task.

---

## Parallel Example: User Story 3

```text
Task: "T015–T017 idle/abort/stop clears in src/agents/process.rs"   # one file, sequential
Task: "T018 scratch deletes in src/storage/sqlite/session.rs and agent.rs"
Task: "T019 clear_orphaned_scratch startup sweep in src/dependencies.rs"
Task: "T020 clear unit tests in src/storage/sqlite/scratch.rs"
```

## Parallel Example: User Story 1

```text
Task: "T009 write_scratch unit tests in src/storage/sqlite/scratch.rs"
Task: "T010 valid_scratch_name unit tests in src/agents/tools/scratchpad.rs"
```

---

## Implementation Strategy

### MVP first (US1)

1. Phases 1–2 (T001–T005).
2. Phase 3 (T006–T010): write and read work in a turn. Check quickstart §1–§2 without the appends.
3. **Stop and validate.** Without US3 the files are not yet cleared, so do not ship the MVP alone. Scratch files would pile up until the next restart once T019 lands, or forever without it.

### Incremental delivery

1. MVP (US1) + US4 tests → isolation proven.
2. US2 → all three tools.
3. US3 → lifecycle complete. This is the first shippable point, because the spec's clean-up guarantee is part of the feature.
4. Polish → dream access, docs, CLAUDE.md, clippy/test, dummyplug e2e, live dream check.

One commit for the whole feature (`feat: scratchpad tools for temporary per-conversation notes`), noting that `scratchpad` is now a reserved category name (research R6).
