---

description: "Task list for semantic chunking of memories"
---

# Tasks: Semantic Chunking for Memory Recall

**Input**: Design documents from `/specs/009-memory-semantic-chunking/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Included. Not because TDD was requested, but because `contracts/chunker.md` specifies 14
required cases and the plan names the chunker as the one cheaply unit-testable piece of this feature
— the suite is sparse enough that this is where test effort actually pays. The constitution also
gates on `cargo test`, `cargo clippy`, and a dummyplug end-to-end walk.

**Organization**: Grouped by user story. Note that US2 consumes US1's search path, so it is testable
independently but not *implementable* before it — see Dependencies.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: US1 / US2; Setup, Foundational, Transition and Polish carry no story label

## Path Conventions

Single Rust binary at repository root: `src/`, with unit tests inline in `#[cfg(test)]` modules
following the existing `src/storage/memory_bundle.rs` pattern. No `tests/` directory in this project.

---

## Phase 1: Setup

**Purpose**: Config surface and the fixtures the rest of the work is verified against

- [X] T001 Add `ChunkLimits { target_size, min_size, max_size }` with defaults 1200/400/2400 to `src/config/mod.rs` and expose it on `AgentConfig` in `src/schema/agent.rs`
- [X] T002 Add automatic-context settings to `src/schema/agent.rs` — per-request-kind count (Chat default 5, SilentRead default 0), total size cap, and a threshold field separate from the search threshold
- [X] T003 [P] Create `src/storage/chunk.rs` with `PassageSpan`, `Block`, `BlockKind` types and register `mod chunk;` in `src/storage/mod.rs`
- [X] T004 [P] Write the test fixture `specs/009-memory-semantic-chunking/fixtures/long-memory.md` — ten headed sections, ~3,000 words, the string `deployment windows are Tuesday and Thursday mornings` in section eight, a ~5 KB fenced code block, and one ~4,000-byte section covering two distinct topics with no sub-heading. Record the measured byte size of each section in a comment block so quickstart Step 2 asserts real numbers rather than approximations
- [X] T005 [P] Measure section-length distribution across an existing memory corpus (`SELECT path, length(content)` over `memory_node` plus a scan of real documents) and record the result in `specs/009-memory-semantic-chunking/research.md` under Decision 1b. **This gates T010/T011**: if real agent memories are uniformly heading-dense, embedding-chosen seams never fire and that work should be dropped rather than built

**Checkpoint**: config surface exists, fixture exists, and the empirical question behind Decision 1b is answered

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The chunker, the passage index, and the score repair. Neither user story can begin until
these are in place.

**⚠️ CRITICAL**: No user story work starts before this phase completes

### The chunker

- [X] T006 Implement `scan_blocks(content: &str) -> Vec<Block>` (pass 1) in `src/storage/chunk.rs` — line-walking state machine recognising Code, Table, Quote, Heading, List, Paragraph per `contracts/chunker.md`. Track fence state **first** so `#` and `|` inside a fenced block are not read as heading or table
- [X] T007 [P] Unit tests for `scan_blocks` in `src/storage/chunk.rs` — fence/table/quote/heading/list/paragraph recognition, `#` and `|` inside a code block, `\r\n` line endings, emoji and CJK content
- [X] T008 Implement `pack_blocks(blocks, limits, None)` size-packing path in `src/storage/chunk.rs` — heading starts a passage, min-size overrides boundary preference, oversized blocks divided per kind with `continues_previous` on parts 2..n, trailing sub-min passage merged backwards
- [X] T009 [P] Unit tests for the size-packing path in `src/storage/chunk.rs` covering every applicable row of the `contracts/chunker.md` required-cases table: empty input, sub-`min` document, 10x-`max` unstructured text, heading-delimited sections, 5 KB code block, 2 KB table, two adjacent 100-byte paragraphs
- [X] ~~T010~~ **Dropped by the T005 gate** (research Decision 1b: 0% of real runs reach either embedding trigger). Was: implement seam selection in `pack_blocks` from precomputed block embeddings in `src/storage/chunk.rs` — cut at the lowest consecutive-block cosine similarity, with the size constraint winning when a seam would leave a sub-`min` passage (gated by T005)
- [X] ~~T011~~ **Dropped with T010.** Was: unit tests for seam selection in `src/storage/chunk.rs` using hand-written vectors — weakest seam differs from the size-greedy cut, and a weak seam that would leave a sub-`min` passage is ignored (gated by T005)
- [X] T012 Implement `async fn chunk_markdown(content, limits, &dyn VizierEmbeddingModel)` in `src/storage/chunk.rs` — invoke `embed_texts` only for runs under one heading exceeding `max_size` or `2 × min_size`, one batched call per run, and fall back to `pack_blocks(.., None)` with a `tracing::warn!` on embedder error so the save still succeeds (FR-036)
- [X] T013 [P] Unit test in `src/storage/chunk.rs` that a stub `VizierEmbeddingModel` returning `Err` yields valid size-packed spans rather than an error
- [X] T014 [P] Implement the embedded-text composition helper in `src/storage/chunk.rs` — document title, tags, enclosing heading breadcrumb, then passage body (data-model.md §7), used for index text only and never for returned text

### Schema and index

- [X] T015 Add the `memory_passage` table DDL (including `content_hash`) to `init_memory_graph_schema` in `src/storage/sqlite/mod.rs` per data-model.md §3, so the existing `memory_bundle` unit tests can stand it up on an in-memory connection
- [X] T016 [P] Add `MemoryPassageResult` to `src/schema/storage.rs` per data-model.md §6
- [X] T017 Add `score: f64` to `DocumentIndex` in `src/schema/storage.rs` and populate it from `1.0 - distance` in `SqliteIndexer::search_document_index` (`src/indexer/sqlite.rs`), which currently computes the value for its threshold filter and discards it. Return `0.0` from `src/indexer/noop.rs`
- [X] T018 Rank on the real score in `rerank_memories` (`src/storage/rerank.rs`), replacing `assign_ranks(&unique, |m| m.slug.clone())` — the current "similarity" rank is alphabetical by slug at double weight (research D4). **This changes recall ordering for every existing query, independent of chunking**
- [X] T019 Add `add_document_indexes(context, Vec<(String, String)>)` to the `DocumentIndexer` trait in `src/indexer/mod.rs`, implement it over `embed_texts` with a batched insert in `src/indexer/sqlite.rs`, and as a no-op in `src/indexer/noop.rs`
- [X] T020 Extend `indexer_key` / `parse_indexer_key` in `src/storage/memory_bundle.rs` to carry and strip a trailing `#{ordinal}`, keeping the existing `splitn(3, '/')` tolerance for nested paths

### Passage lifecycle

- [X] T021 Write passages in `BundleMemoryStore::write_memory` (`src/storage/memory_bundle.rs`) — chunk the content, delete the document's existing `memory_passage` rows and index entries by their stored ordinals, insert the new rows with `content_hash`, and index all passages through `add_document_indexes` in one batch
- [X] T022 Delete passages in `delete_memory` and `delete_bundle` (`src/storage/memory_bundle.rs`) — remove `memory_passage` rows and the matching `document_index` entries for every stored ordinal
- [X] T023 Produce passages for every document in `import_bundle` (`src/storage/memory_bundle.rs`) with no extra manual step
- [X] T024 [P] Unit tests for the passage lifecycle in `src/storage/memory_bundle.rs` against an in-memory connection — a write replaces rather than appends; **a document shrinking from twelve passages to four leaves no orphaned `document_index` rows**; delete clears both tables; import populates both
- [X] T025 Chunking or indexing failure must not fail the save (FR-036) in `src/storage/memory_bundle.rs` — log via `tracing` and leave the document readable and re-convertible

**Checkpoint**: passages are built, stored, indexed, and kept in step with their documents. Nothing agent-visible has changed yet.

---

## Phase 3: User Story 1 - Search returns focused passages (Priority: P1) 🎯 MVP

**Goal**: An agent searching memory receives short addressed passages instead of ten whole document
bodies, ranked by each passage's own match, with the whole document still reachable by address.

**Independent Test**: Write the T004 fixture through `memory_write`, search `deployment windows`, and
verify the returned text contains the section-eight sentence, is a small fraction of the document,
and carries bundle, path, title, ordinal, line span and score — then fetch the whole document by the
address in that result. Quickstart Steps 1–5.

### Implementation for User Story 1

- [X] T026 [US1] Change `query_memory` in `src/storage/memory_bundle.rs` to return `Vec<MemoryPassageResult>` — over-fetch `limit * 5` passages, parse agent/bundle/path/ordinal from each key, and drop entries for other agents or bundles
- [X] T027 [US1] Apply the per-document cap in `query_memory` (`src/storage/memory_bundle.rs`) so one long document cannot fill the result set (FR-013)
- [X] T028 [US1] Merge consecutive ordinals of the same document into one result in `query_memory` (`src/storage/memory_bundle.rs`), spanning first `char_start` to last `char_end` and keeping the best score, applied **after** the cap (research D7)
- [X] T029 [US1] Slice passage text from the document body in `query_memory` (`src/storage/memory_bundle.rs`) using the stored char spans, reusing the `get_memory_detail` read already performed per candidate
- [X] T030 [US1] Update the `query_memory` signature in the `MemoryStorage` trait (`src/storage/memory.rs`) and its hand-forwarded implementation in `src/storage/mod.rs`
- [X] T031 [US1] Add a passage-list variant to `MemoryOpResponse` in `src/schema/` and handle it in the `MemoryOpRequest::Query` arm of `src/agents/memory_ops.rs`
- [X] T032 [US1] Rename `memory_read` to `memory_search` in `src/agents/tools/vector_memory/mod.rs` — change `Output` from `Vec<String>` to the passage result type and rewrite the description to say results are passages and that `memory_read` fetches the whole document when a passage is not enough (contracts/memory-tools.md)
- [X] T033 [US1] Rename `memory_detail` to `memory_read` in `src/agents/tools/vector_memory/mod.rs`, retiring `memory_detail` without an alias. **Must land in the same commit as T032** — tool names are the dispatch key
- [X] T034 [US1] Return an explicit "no longer exists" outcome from `memory_read` for a missing document in `src/agents/tools/vector_memory/mod.rs`, not an empty result or an opaque error (FR-019)
- [X] T035 [US1] Keep `increment_read_count` firing per source document when its passages are returned, in `src/agents/tools/vector_memory/mod.rs` (FR-017)
- [X] T036 [US1] Return passage results from `query_memories` in `src/channels/http/api/v1/agents/memory.rs` per `contracts/http-memory-query.md`, reusing the `MemoryPassageResult` type rather than a parallel shape. No WebUI work — nothing calls this endpoint yet

**Checkpoint**: search is passage-level end to end, through both the agent tool and the HTTP API. Quickstart Steps 1–5 pass.

---

## Phase 4: User Story 2 - Related memories arrive with substance (Priority: P2)

**Goal**: The per-turn related-memory lookup injects up to five actual passages with their addresses
instead of ten bare titles, bounded and gated so the one unconditional path does not become the most
expensive one.

**Independent Test**: Send a message whose answer sits in one stored passage and verify the agent can
answer with no memory tool call, with the assembled block inside its count and size budget; then send
`ok` and verify nothing was retrieved. Quickstart Steps 6–8 (read from `RUST_LOG=vizier=debug`, since
dummyplug strips the injected block).

### Implementation for User Story 2

- [X] T037 [P] [US2] Add a query-usability gate helper in `src/agents/process.rs` — decline retrieval for messages under a few words or purely referential (`ok`, `thanks`, `do that one`), before any relevance query is issued (FR-029)
- [X] T038 [US2] Change the Chat / AudioChat retrieval at `src/agents/process.rs:782` to fetch 5 passages at the automatic-context threshold, applying the gate from T037
- [X] T039 [US2] Change the SilentRead retrieval at `src/agents/process.rs:817` to use its own budget, defaulting to 0 — this path fires for every non-mention message in a Discord guild channel (`src/channels/discord/mod.rs:508`) and every Telegram group message (`src/channels/telegram/mod.rs:497`), so its cost scales with channel traffic (FR-030)
- [X] T040 [US2] Apply the per-document cap to automatic context in `src/agents/process.rs` so one long memory cannot take all five slots (FR-024)
- [X] T041 [US2] Enforce the total size cap in `src/agents/process.rs`, spending the budget in rank order and omitting the remainder with no partial passage at the tail; truncate a single oversized passage and mark it, stating its address (FR-028)
- [X] T042 [US2] Render passages in `context_md` (`src/agents/agent/system_prompt/context.rs`) inside explicit delimiters carrying bundle, path and ordinal, labelled as retrieved reference material that may be irrelevant and is not instruction (FR-021, FR-031). Keep it flowing through `with_context` into the **user message**, never a system message, so the cacheable prefix is unaffected
- [X] T043 [US2] Omit the related-memory section entirely when nothing clears the threshold in `src/agents/agent/system_prompt/context.rs` — no weakly-related filler (FR-026)
- [X] T044 [US2] Let a failed or timed-out assembly proceed without the block in `src/agents/process.rs`; the turn must never fail (FR-027)
- [X] T045 [US2] Record per turn whether an agent searched memory after context was injected, via `tracing`, in `src/agents/process.rs` — the hit/miss signal the threshold is tuned against (FR-032)

**Checkpoint**: both stories work. Quickstart Steps 1–10 pass.

---

## Phase 5: Transition & Conversion

**Purpose**: Existing deployments. Required for release, not optional polish — a corpus written by
the current release has no passages and is unsearchable until converted.

- [X] T046 Spawn a per-agent background conversion task in `VizierAgents::spawn_agent` (`src/agents/mod.rs`) after `build_indexer` resolves — **not** in `src/dependencies.rs`, where every other migration lives but no embedder exists (research D8)
- [X] T047 Make the conversion resumable in `src/agents/mod.rs` by skipping documents that already have `memory_passage` rows; absence of rows is the only progress marker (FR-040)
- [X] T048 Keep the agent serving requests throughout the conversion (FR-040) — the task spawned in `src/agents/mod.rs` must not hold the agent's request channel or the sqlite write lock for the duration; verify by messaging a dummyplug agent while a large corpus converts
- [X] T049 Detect drift by comparing stored `content_hash` against the document body in `reconcile_bundle` (`src/storage/memory_bundle.rs`) and rebuild only the documents that differ (FR-038, research D12). Do **not** re-chunk to diff spans — pass 2 consults embeddings, so re-derived spans are not stable across embedding-model versions
- [X] T050 Log conversion outcome via `tracing` in `src/agents/mod.rs` — documents processed, documents failed, and the reason for each failure (FR-042)

**Checkpoint**: an existing install upgrades unattended and converges on a fully passage-indexed corpus.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [X] T051 **Partly done — measured, not fully derived.** Both thresholds were calibrated against a live index (quickstart Step 3, research D10's measurement section): automatic context moved 0.6 → **0.45**, because nothing in a real corpus reached 0.6 and the block would have been empty on every turn. The 70%-empty target still needs a replay over *real* stored session history, which a fresh install has none of. Was: derive the automatic-context threshold by replaying stored session history through the passage index and pick the lowest value that leaves the block empty on at least 70% of real messages (SC-010); replace the provisional 0.6 in `src/schema/agent.rs`. `HistoryStorage` already persists the data, so this needs no new capture. **This is the number that decides whether the feature saves tokens or spends them** — five passages cost roughly 15x the ten titles they replace, so the saving comes entirely from how often the block is correctly empty
- [X] T052 Re-check the search threshold default for the agent tool in `src/agents/tools/vector_memory/mod.rs` — it is currently 0.1 against the automatic path's 0.5, a 5x disagreement between two callers of one index, and passage scores will not distribute like document scores (research D10)
- [X] T053 [P] Update the memory and tools sections of `CLAUDE.md` for passage-level retrieval, the `memory_search` / `memory_read` renames, and the new `memory_passage` table
- [X] T054 Run `cargo clippy` and `cargo test` to green (constitution quality gate)
- [X] T055 **Walked; Steps 8 and 10 not reachable by this harness, both documented in quickstart.** Steps 1–7, 9, 11, 12 verified against a running binary with two dummyplug agents. Step 8 needs Discord/Telegram credentials; Step 10's scenario cannot be configured (an agent with an unreachable embedder fails to start) and is covered by the `an_indexing_failure_does_not_fail_the_save` unit test. Was: walk every dummyplug step in `quickstart.md` (Steps 1–12) against a running binary with a `dummyplug` agent — the constitution's end-to-end gate. Steps 6, 7, 8 and 10 read `RUST_LOG=vizier=debug` output, because dummyplug strips the injected context block
- [X] T056 **Functionally verified against a live model** (manual run by the repo owner, 2026-10-03): the passage-level tools, the renames and the automatic-context block all behave correctly with a real provider. The two *numeric* criteria this task also names remain unmeasured — SC-009 (60% of answerable messages answered with no tool call) and SC-008 (context spend per turn) need counted rates over a run of real turns, not a functional pass, and both are provider-dependent. Was: run the live-provider checks `quickstart.md` lists as uncoverable by dummyplug
- [X] T057 Commit with `[**breaking**]` in the subject: the `memory_search` output shape, the `memory_read` meaning, and the HTTP query response all change (constitution conventional-commits gate)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies. T005 gates T010/T011
- **Foundational (Phase 2)**: depends on Setup. **Blocks both user stories**
- **US1 (Phase 3)**: depends on Foundational
- **US2 (Phase 4)**: depends on Foundational **and on T026–T031** — it consumes the passage-returning `query_memory`. Independently *testable*, not independently *implementable*
- **Transition (Phase 5)**: depends on Foundational. Can run parallel to Phase 3/4
- **Polish (Phase 6)**: T051 depends on Phase 4; the rest depends on all desired stories

### Critical path

```
T001-T003 → T006 → T008 → T012 → T015-T021 → T026-T031 → T037-T045 → T051 → T055
```

### Ordering constraints worth stating

- **T032 and T033 must land in one commit.** Tool names are the dispatch key; no intermediate state can exist where `memory_read` means two things
- **T028 after T027.** Capping before merging would count passages that are about to become one result (research D7)
- **T017 before T018.** The score has nowhere to live until `DocumentIndex` carries it
- **T005 before T010/T011.** If real memories are heading-dense, the embedding seam path never fires and should be dropped rather than built
- **T021 before T024.** The orphaned-row test needs the write path it is testing

### Parallel Opportunities

- T003, T004, T005 in parallel
- T007, T009, T011, T013 (test tasks, same file but independent `#[cfg(test)]` functions — coordinate or serialise if editing concurrently)
- T014, T016 in parallel with the schema tasks
- Phase 5 (Transition) in parallel with Phase 3 and Phase 4 by a second developer
- T053 in parallel with anything

---

## Parallel Example: Phase 1

```bash
Task: "Create src/storage/chunk.rs with PassageSpan, Block, BlockKind types"
Task: "Write the long-memory fixture with measured section sizes"
Task: "Measure section-length distribution across an existing memory corpus"
```

---

## Implementation Strategy

### MVP (User Story 1 only)

1. Phase 1 Setup
2. Phase 2 Foundational — the bulk of the work
3. Phase 3 US1
4. **Stop and validate**: quickstart Steps 1–5
5. At this point search is passage-level and the context saving in SC-001 is already realised. The
   per-turn path still shows titles only, so agents still need a search call to get anything useful

### Incremental Delivery

1. Setup + Foundational → passages exist and stay in step with documents
2. US1 → searches return passages → validate → the 70% text reduction lands here
3. US2 → per-turn context carries five passages → validate → the round-trip saving lands here
4. Transition → existing installs converge
5. Polish → threshold derived from data rather than guessed

### Where the risk is

Not in the chunker, which is pure and testable. It is in three places:

- **T024's orphan test.** A document shrinking from twelve passages to four leaves eight stale
  vectors unless deletion is driven by stored ordinals. Stale passages returning deleted text is
  the worst failure this feature can produce
- **T051's threshold.** Set loosely, the feature increases total token spend. It is the one number
  with no safe default
- **T018's ranking change.** Fixing the alphabetical similarity rank alters recall ordering for
  every existing query. Expected, but it will look like a regression to anyone who had tuned around
  the old behaviour

---

## Notes

- [P] = different files, no dependencies
- Unit tests live inline in `#[cfg(test)]` modules; this project has no `tests/` directory
- Commit after each task or logical group; use `[**breaking**]` where the subject warrants it
- Stop at any checkpoint to validate a story independently

---

## Implementation notes (added during /speckit-implement)

Four things changed shape against the plan, each for a measured reason rather than a preference.

**The embedding-seam path was dropped, not built (T005's gate fired).** Measured against the only
real corpus available: 57 documents, every one heading-dense, largest heading-delimited run **697
bytes** — below `2 × min_size` and 3.4x below `max_size`. Neither embedding trigger fired on a single
document, which is exactly the condition T005 said should drop T010/T011. With them went
`pack_blocks`' `block_embeddings` parameter and `chunk_markdown`'s embedder, so the chunker is now
synchronous, pure and infallible. Decision 6 (`add_document_indexes`) is unaffected — it batches the
embedding of finished passage *texts*, which happens on every save regardless. Recorded in
research.md Decision 1b; contracts/chunker.md updated to match.

**Both thresholds were wrong, in opposite directions.** Calibrated against a live index (quickstart
Step 3): `memory_search` moved 0.1 → **0.20** (0.1 filtered nothing at all), and automatic context
moved 0.6 → **0.45** because *nothing in a real corpus reached 0.6*, so the per-turn block would have
been empty on every turn while appearing configured. An intermediate pass set search to 0.35 and that
rejected a direct heading match (`incident review`, 0.32) — caught by the walk, not by a test.

**Two defects were found in the ranking blend, not one.** Research D4 called out the alphabetical
similarity rank. Fixing it exposed a second: `assign_ranks` gave ties distinct ranks by fetch order,
and because similarity carries double weight, that arbitrary order outweighed every real signal — a
stale, never-read document beat a fresh, often-read one on an exact score tie. `ranks_by_desc` now
assigns tied ranks (group average), so a tie is a tie and the other signals decide it.

**The chunker had an FR-003 hole the contract's pseudocode did not cover.** An oversized block forces
the open passage closed, which left a lone `# Heading` standing as a 9-byte passage. The contract only
merges a sub-minimum passage *backwards* at the end of a document. Fixed by folding an undersized open
passage into the oversized block's first part where that stays inside `max_size`.

Incidental findings, both pre-existing and out of scope, recorded in quickstart.md so the next walk
does not rediscover them: setting `RUST_LOG=vizier=debug` **disables** debug logging
(`main.rs:39-60` only installs the `EnvFilter` when `RUST_LOG` is absent, and that default is already
`vizier=debug`); and a failed agent update leaves the agent unregistered, because `handle_update`
shuts the old process down before spawning its replacement.

Verification state: **201 unit tests pass**, `cargo clippy` reports **0 errors** and 117 warnings —
exactly the count on `HEAD` before this work. WebUI typecheck clean.
