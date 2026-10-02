# Implementation Plan: Semantic Chunking for Memory Recall

**Branch**: `009-memory-semantic-chunking` | **Date**: 2026-10-01 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/009-memory-semantic-chunking/spec.md`

## Summary

Memory concept documents are split into structure-aware passages, and passages replace whole
documents as the only thing queries are matched against. Search returns addressed passages instead
of ten full document bodies; the per-turn related-memory lookup carries up to five passages instead
of ten bare titles. Documents remain the unit of authorship, versioning, linking, and reading.

Pass 1 of the chunker is structural — markdown blocks, deterministic, no model. Pass 2 packs those
blocks into passages and consults block embeddings to choose the seam, but only inside a run that
size bounds force it to split anyway; heading-dense documents issue no extra embedding call at all.

The work lands in five places: a new pure chunker (`src/storage/chunk.rs`), a derived
`memory_passage` coordinate table, passage-granular rows in the existing `document_index` vec0
table, the two tool renames, and the per-turn context assembly in `src/agents/process.rs` /
`system_prompt/context.rs`.

Two findings from Phase 0 shape the plan more than anything in the spec did. First, **relevance
score is computed and thrown away** — `SqliteIndexer` derives `1.0 - distance` for its threshold
filter and discards it, and `rerank_memories` substitutes alphabetical-by-slug as its
"similarity" rank at double weight. Ranking passages by match quality (FR-011) is therefore not an
addition but a repair. Second, **the indexer does not exist where migrations run** — it is built per
agent from that agent's embedding config, so the one-time conversion belongs in agent spawn, not in
`dependencies.rs` alongside every other migration.

## Technical Context

**Language/Version**: Rust (edition 2024, per `Cargo.toml`); WebUI untouched by this feature

**Primary Dependencies**: existing only — `rusqlite` (bundled) with `sqlite-vec` for the `vec0`
virtual table, `tokio`, `async-trait`, `chrono`, `serde`/`schemars`. **No new crate.** The chunker
is hand-rolled markdown boundary scanning over `str`, which Principle I prefers to pulling in a
markdown parser for span extraction. Pass 2 reuses the existing `VizierEmbeddingModel::embed_texts`
to pick seams inside oversized sections — no new embedding surface.

**Storage**: embedded SQLite. One new derived table (`memory_passage`); the `document_index` vec0
table keeps its DDL and changes only row granularity. Memory documents themselves stay on disk
behind `DocumentStore` — untouched.

**Testing**: `cargo test` for the chunker (pure function, the one cheaply unit-testable piece) and
for passage lifecycle against an in-memory connection, following the existing
`src/storage/memory_bundle.rs` test pattern; `cargo clippy`; end-to-end per
[quickstart.md](./quickstart.md) as dummyplug steps, with four checks reading tracing output
because dummyplug strips the injected context block.

**Target Platform**: Linux/macOS/Windows single binary, including static `musl`. No new system
library, so `Cross.toml` targets are unaffected.

**Project Type**: single Rust binary (agent framework) with a bundled WebUI; this feature is
backend-only.

**Performance Goals**: search latency within 20% of today's at 1,000 documents (SC-003); conversion
of 1,000 documents under 10 minutes unattended while serving requests (SC-006); automatic context
at most five passages within a size cap (SC-012).

**Constraints**: no new external service (Principle III); embedding stays opt-in — an agent without
`embedding` + `indexer` config has no indexer and degrades to no semantic search, exactly as today;
prompt-cacheable prefix must stay byte-identical across turns (SC-013).

**Scale/Scope**: ~1,000 memory documents per agent; single-digit-to-low-tens passages per document;
4 source areas changed, 1 new module, 1 new table, 2 tool renames, 1 breaking HTTP response.

## Constitution Check

*GATE: evaluated before Phase 0 and re-evaluated after Phase 1.*

| Principle | Assessment | Verdict |
|---|---|---|
| **I. Lean by Default** | No new dependency. The chunker is three functions, not a trait — only one chunking strategy is reachable, since every API-created agent is forced to have an embedder (`agents/mod.rs:472-481`), so size-packing is an error path rather than a second strategy (research D1). Passage deletion reuses `delete_index` per stored ordinal rather than widening the trait (D5). Passage text is not cached — only coordinates (D3). One trait method *is* added, `add_document_indexes`, justified by a concrete current cost: chunking turns one embedding per save into N, and without batching that is N sequential HTTP round-trips to a remote embedder on the default write path (D6). | **PASS** |
| **II. DRY via Trait-Based Extensibility** | No new `match` on a type tag. Batch indexing goes behind the existing `DocumentIndexer` trait and is implemented in both impls, not branched at call sites. One result type (`MemoryPassageResult`) serves the agent tool, the automatic-context assembly and the HTTP endpoint rather than three near-identical shapes. Chunking is not kind-varying behaviour, so a trait would be the violation here, not the fix. | **PASS** |
| **III. Self-Contained, Zero-Dependency Runtime** | Storage stays embedded SQLite; no sidecar, no external vector service. Embedding remains opt-in and absent config degrades to no search, as today. The one honesty note: `fastembed` downloads a model once, which is pre-existing and already gated behind explicit config per the constitution's own carve-out. | **PASS** |
| **IV. Portability by Default** | No `cfg`, no platform branch, no new system library. The chunker must count lines rather than bytes for line spans and handle `\r\n`, and must split only on UTF-8 boundaries — both are stated invariants in contracts/chunker.md and covered by required test cases. | **PASS** |
| **V. Unified Errors & Observability** | All new fallible paths return `crate::Result<T>` with `throw_vizier_error` conversion; no new error type. All logging via `tracing` (FR-042 conversion counts, FR-032 hit/miss records). Chunking failure must not fail the save (FR-036), so it is logged and the document stays readable. | **PASS** |
| **Workflow: dummyplug e2e gate** | This is squarely agent-observable — tools, memory, and the agent loop. [quickstart.md](./quickstart.md) scripts 12 dummyplug steps, with the three checks dummyplug structurally cannot cover listed separately as live-provider work. | **PASS** |
| **Workflow: conventional commits** | Breaking change to the `memory_search` tool output, the `memory_read` tool meaning, and the HTTP query response. Commits must carry `[**breaking**]`. | **PASS** |

**No violations. Complexity Tracking omitted.**

One pre-existing defect is repaired as a side effect rather than worked around: the broken
similarity rank (research D4). It is called out because it changes recall ordering for every
existing query, chunking aside, and reviewers should expect that.

## Project Structure

### Documentation (this feature)

```text
specs/009-memory-semantic-chunking/
├── plan.md               # This file
├── spec.md               # Feature specification
├── research.md           # Phase 0 — 11 decisions, 1 noted constraint
├── data-model.md         # Phase 1 — PassageSpan, memory_passage, DocumentIndex change
├── quickstart.md         # Phase 1 — dummyplug e2e script
├── contracts/
│   ├── chunker.md        # chunk_markdown signature, boundaries, required cases
│   ├── memory-tools.md   # memory_search / memory_read / automatic context
│   └── http-memory-query.md
├── checklists/
│   └── requirements.md   # Spec quality checklist (already passing)
└── tasks.md              # Phase 2 — created by /speckit-tasks, NOT here
```

### Source Code (repository root)

```text
src/
├── storage/
│   ├── chunk.rs              # NEW — scan_blocks + pack_blocks (pure, tested) behind
│   │                         #   async chunk_markdown; ChunkLimits, PassageSpan
│   ├── memory_bundle.rs      # passage write/delete/query; indexer_key gains #ordinal;
│   │                         #   query_memory returns passages, caps per document, merges adjacents
│   ├── memory.rs             # MemoryStorage trait: query_memory return type
│   ├── rerank.rs             # rank on the real score instead of alphabetical slug order
│   └── sqlite/mod.rs         # memory_passage DDL in init_memory_graph_schema
├── indexer/
│   ├── mod.rs                # DocumentIndexer: add_document_indexes (batch)
│   ├── sqlite.rs             # batch insert; populate DocumentIndex.score
│   └── noop.rs               # batch no-op; score 0.0
├── schema/storage.rs         # DocumentIndex.score; MemoryPassageResult
├── agents/
│   ├── mod.rs                # per-agent conversion task at spawn (research D8)
│   ├── process.rs            # auto-context: 5 on Chat, 0 on SilentRead, query gate
│   ├── agent/system_prompt/
│   │   └── context.rs        # render delimited passages, labelled as reference data
│   └── tools/vector_memory/
│       └── mod.rs            # memory_read→memory_search; memory_detail→memory_read
├── channels/http/api/v1/agents/
│   └── memory.rs             # query_memories returns passage results
└── config/                   # ChunkLimits + auto-context count/threshold/size cap
```

**Structure Decision**: The existing single-binary layout is kept as-is. The only new file is
`src/storage/chunk.rs`, placed beside `memory_bundle.rs` because passages are a storage-layer
derivation of documents, not an agent concern — and because keeping it a free function in `storage`
lets it be unit-tested without a `DocumentStore` or an agent. Everything else is an edit to a file
that already owns the behaviour, which is what Principle II's "register behind the trait, don't
branch at call sites" asks for.

## Implementation Sequence

Ordered so each step is independently verifiable, and so the breaking changes land together.

1. **Chunker** — `src/storage/chunk.rs`: `scan_blocks` (structural, pure), `pack_blocks` (pure,
   taking precomputed block embeddings or `None`), and the async `chunk_markdown` that batches
   embedding calls only for runs exceeding `max_size` or `2 × min_size` (research D1b). Unit tests
   from contracts/chunker.md, including the seam cases with hand-written vectors. No integration
   needed — verifiable by `cargo test` alone.
2. **Schema + score plumbing** — `memory_passage` DDL; `DocumentIndex.score`; `rerank.rs` ranking on
   it. Repairs the existing defect before anything depends on ranking.
3. **Batch indexing** — `add_document_indexes` on the trait and both impls.
4. **Write path** — passages written, replaced, and deleted with their documents (FR-033–FR-036).
   Step 9 of quickstart is the regression that matters: a shrinking document must not orphan vectors.
5. **Search path** — `query_memory` returns passages; over-fetch, per-document cap, adjacent merge
   (FR-008–FR-017, research D7).
6. **Tool renames** — both in one commit (contracts/memory-tools.md), with rewritten descriptions.
7. **HTTP response** — `query_memories` returns passage results (FR-017).
8. **Automatic context** — counts, gate, per-path budget, delimited rendering, hit/miss logging
   (FR-020–FR-032).
9. **Conversion** — per-agent background task at spawn, resumable (FR-037–FR-040).
10. **Threshold derivation** — replay stored history, replace the provisional 0.6 (research D10).

## Open item carried into tasks

The automatic-context threshold ships provisional at **0.6** and must be derived by replaying stored
session history (research D10, quickstart final section). This is the value that decides whether the
feature saves tokens or spends them: five passages cost roughly 15x the ten titles they replace, so
the saving comes entirely from how often the block is *correctly empty*. Everything else in this
plan is settled.
