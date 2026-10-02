# Phase 0 Research: Semantic Chunking for Memory Recall

**Feature**: `009-memory-semantic-chunking` | **Date**: 2026-10-01

Every decision below was taken against the code as it stands, with file and line references so a
reviewer can check the premise rather than the conclusion.

---

## Decision 1 — Two pure passes behind one thin async orchestrator, no trait

**Decision**: `src/storage/chunk.rs` exposes three functions:

```rust
fn scan_blocks(content: &str) -> Vec<Block>                     // pass 1 — pure, sync
fn pack_blocks(                                                  // pass 2 — pure, sync
    blocks: &[Block],
    limits: &ChunkLimits,
    block_embeddings: Option<&[Vec<f64>]>,
) -> Vec<PassageSpan>
async fn chunk_markdown(                                         // orchestrator
    content: &str,
    limits: &ChunkLimits,
    embedder: &dyn VizierEmbeddingModel,
) -> Result<Vec<PassageSpan>>
```

Pass 1 segments the document into atomic blocks (structural, deterministic). Pass 2 packs blocks
into passages, using block embeddings to choose seams where available and falling back to
size-packing where not. The orchestrator decides whether embeddings are needed at all and batches
the call.

**Rationale**: Both passes stay pure and unit-testable — `pack_blocks` takes *precomputed* block
embeddings, so its seam logic is exercised with hand-written vectors and no embedder. Only the thin
orchestrator is async, and it depends on `&dyn VizierEmbeddingModel` (`src/embedding/mod.rs:26`)
rather than on `VizierEmbedder`, whose `build` is private to the `embedding` module and so could not
be faked from a test. Against the narrow trait a test stub is two lines — no visibility change and
no new trait.

No `Chunker` or `SeamScorer` trait: Principle I forbids a trait for a single reachable
implementation. An earlier draft of this decision justified a `SeamScorer` trait on the grounds that
agents without an embedder need a size-packing fallback. **That premise was wrong.**
`CreateAgentRequest::into_config` forces `embedding: Some(...)` (defaulting to local fastembed
`all_mini_lml6_v2`) and `indexer: Some(Sqlite)`
(`src/channels/http/api/v1/agents/mod.rs:472-481`); create and update share that one `into_config`;
and agents are only ever created through the API, never YAML. So no user-creatable agent lacks an
embedder, and size-packing is an error path rather than a second strategy.

**Alternatives rejected**:
- A `Chunker` or `SeamScorer` trait — speculative; one reachable implementation.
- One monolithic `chunk_markdown` — would make the seam rules untestable without an embedder,
  losing the only cheaply-testable part of this feature.
- Chunking inside `BundleMemoryStore` — untestable without a `DocumentStore` and a sqlite connection.

---

## Decision 1b — Pass 2 uses block embeddings to choose seams, lazily

**Decision**: Where a run of blocks under one heading exceeds `max_size` (so it must be split) or
exceeds `2 × min_size` (so it could profitably be split), embed that run's blocks in one batched
`embed_texts` call and cut at the lowest consecutive-block similarity. Everywhere else, pack by size
alone and issue no embedding call.

**Rationale**: Structure carries most of the signal in agent-written markdown, which is
heading-dense — where headings already mark topic shifts, embeddings agree with size-packing and add
nothing. The value concentrates in one case: a long section whose headings have run out of
information, where size-packing cuts at an arbitrary byte offset mid-topic. A secondary win is two
unrelated short paragraphs under one heading, which size-packing blends into one passage but which
are individually more matchable when split. Lazy invocation means the typical document costs zero
extra embeddings and the cost lands only where it buys a better boundary.

Passage text is still embedded for the index after packing. Mean-pooling block vectors into a
synthetic passage vector would save a call, but pooling loses cross-block context in the exact
artefact search quality is judged on (SC-001, SC-002).

**Alternatives rejected**:
- Embedding-chosen seams everywhere — pays N block embeddings per save to reproduce boundaries that
  `##` already marks.
- Mean-pooled passage vectors — a quality shortcut on the feature's primary output.
- Sentence-level similarity — finer than any boundary the packer can use, since blocks are the
  atomic unit from pass 1.

**Fallback**: if the embedder errors or times out, pass 2 packs by size and the save proceeds
(FR-036). That is a `match` on a `Result`, not branching over a type tag — Principle II concerns
kind-varying behaviour, and an error path is not a kind.

---

## Decision 2 — Passage identity is encoded in the existing indexer key

**Decision**: Index each passage under `{agent_id}/{bundle}/{path}#{ordinal}`, reusing the existing
key scheme (`BundleMemoryStore::indexer_key`, `src/storage/memory_bundle.rs:197`). Extend
`parse_indexer_key` to split a trailing `#{ordinal}` off the path.

**Rationale**: `parse_indexer_key` uses `splitn(3, '/')`, so the path component already tolerates
slashes for nested concepts ("friends/bred"). A `#` suffix is unambiguous because document paths
carry no extension and no `#`. The `document_index` vec0 table (`src/indexer/sqlite.rs:31`) needs no
schema change at all — it keys on `(context, path)` and we are simply addressing finer rows.

**Alternatives rejected**:
- A second vec0 table for passages — duplicates the DDL, the dimension negotiation, and the
  insert/delete paths for no behavioural gain.
- A synthetic opaque passage id — would require a lookup table just to get back to the document,
  when the document address is the useful part of every result (FR-009).

---

## Decision 3 — Passage text is sliced from the document, never cached

**Decision**: A new `memory_passage` table stores only each passage's *coordinates* — ordinal, line
span, character span, continuation flag. Passage text is sliced out of the document's content at
query time.

**Rationale**: The spec is explicit that a passage "holds no content that is not in its document."
More practically, `query_memory` already reads every candidate document in full via
`get_memory_detail` (`src/storage/memory_bundle.rs:818`) in order to build its `Memory` values, so
the document text is already in hand when results are assembled — slicing it costs nothing. Caching
passage text in sqlite would duplicate the corpus, add a staleness class the reconcile path would
have to handle, and buy nothing measurable.

**Alternatives rejected**:
- Storing passage text alongside the embedding — duplication plus a new staleness mode, with no
  demonstrated need. Revisit only if profiling shows the document reads dominate.
- Storing spans inside vec0 auxiliary columns — couples derived coordinates to the vector table's
  lifecycle; `memory_passage` mirrors the established `memory_node`/`memory_edge` pattern
  (`src/storage/sqlite/mod.rs:33`) and can be rebuilt independently.

---

## Decision 4 — Relevance score must be plumbed through `DocumentIndex` (fixes an existing defect)

**Decision**: Add `score: f64` to `DocumentIndex` (`src/schema/storage.rs:109`), populate it in
`SqliteIndexer::search_document_index`, and rank on it in `rerank_memories`.

**Rationale**: FR-010 requires ranking by how well the individual passage matches, and that is
currently impossible — not merely unimplemented. `SqliteIndexer::search_document_index` computes
`let similarity = 1.0 - distance` to apply the threshold and then **drops the value** on the next
line (`.map(|(path, ctx, _)| ...)`, `src/indexer/sqlite.rs:110`). `DocumentIndex` has nowhere to put
it. Downstream, `rerank_memories` builds what it calls the similarity rank as
`assign_ranks(&unique, |m| m.slug.clone())` (`src/storage/rerank.rs:69`) — **alphabetical order by
slug**, weighted `W_SIMILARITY = 2.0`, double every other signal. So today the strongest ranking
input to memory recall is the alphabet. Passage-level ranking cannot be built on that, and carrying
the score fixes it.

**Alternatives rejected**:
- Re-embedding results to recover scores — absurd cost for data the query already computed.
- Leaving rerank alone and sorting only on raw distance — throws away the link, recency, and
  read-count signals that the RRF blend exists to combine. Keep the blend; fix its broken input.

**Note**: this changes recall ordering for every existing query, independent of chunking. It is a
behaviour change reviewers should expect, and it is covered by SC-002.

---

## Decision 5 — Deleting a document's passages needs no trait change

**Decision**: Read the document's ordinals from `memory_passage`, then call the existing
`delete_index(context, key)` once per ordinal before rewriting the rows.

**Rationale**: Principle I — do not widen a trait with two implementations (`SqliteIndexer`,
`NoopIndexer`) ahead of a second caller. The ordinals are already being rewritten on every save, so
they are in hand for free, and a document has single-digit-to-low-tens passages.

**Alternatives rejected**:
- Adding `delete_index_prefix` to `DocumentIndexer` — a trait change serving one call site. If
  passage counts per document ever reach the hundreds, revisit.

---

## Decision 6 — Writing passages uses the batch embedding call

**Decision**: Add `add_document_indexes(context, Vec<(path, content)>)` to `DocumentIndexer` and
implement it over the existing `VizierEmbeddingModel::embed_texts`
(`src/embedding/mod.rs:28`).

**Rationale**: Chunking turns one embedding per save into N. For remote embedding providers (openai,
gemini, voyageai, cohere, …) the single-text path means N sequential HTTP round-trips per memory
write, which would make saving a long memory visibly slow. `embed_texts` already exists on the
embedding trait and every provider implements it, so the batch capability is present and merely
unreachable from the indexer. Unlike Decision 5, this trait change is driven by a concrete,
current cost on the default path — not a hypothetical second implementation.

**Alternatives rejected**:
- Looping `add_document_index` — N round-trips per save; the reason `embed_texts` exists.
- Embedding in the caller and inserting rows directly — leaks vector-table mechanics out of the
  indexer and past its trait, violating Principle II's "register behind the trait" rule.

---

## Decision 7 — Over-fetch, then cap per document, then merge adjacents

**Decision**: Fetch `limit * 5` passages from the index, group by source document, apply the
per-document cap (FR-012), merge consecutive ordinals (FR-011), then take `limit`.

**Rationale**: Two existing behaviours force the over-fetch. First, the vec0 query applies
`LIMIT ?3` *before* the threshold filter runs in Rust (`src/indexer/sqlite.rs:103-113`), so a
thresholded search already returns fewer rows than asked. Second, `query_memory` already over-fetches
`limit * 5` (`src/storage/memory_bundle.rs:801`) for exactly this reason. Chunking makes it sharper:
one long document can now occupy many of the fetched rows, so without the cap a single memory
crowds the result set — which is the failure FR-012 names. Order matters: capping before merging
would let the cap count passages that are about to become one result.

**Alternatives rejected**:
- Pushing the per-document cap into SQL (window function over vec0) — sqlite-vec's virtual table
  does not compose with window functions reliably, and the result sets here are tens of rows.

---

## Decision 8 — The one-time conversion runs per agent at spawn, not in `dependencies.rs`

**Decision**: Convert in `VizierAgents::spawn_agent`, as a background task, after
`build_indexer` resolves. A document with no `memory_passage` rows is unconverted; that absence is
the resume marker.

**Rationale**: This is the constraint most likely to be got wrong. Every other migration lives in
`VizierDependencies::new` (`src/dependencies.rs:104-112`), but **the indexer does not exist there**
— it is built per agent from that agent's own embedding config in `VizierAgents::build_indexer`
(`src/agents/mod.rs:78-99`), and returns `None` when the agent has no embedding configured. A
conversion in `dependencies.rs` would have no embedder to call, which is exactly why the two
existing memory migrations there pass `VizierIndexer::build(NoopIndexer)`
(`src/dependencies.rs:276`, `:362`) and leave semantic search to catch up later. Running it at spawn
also satisfies FR-039 for free: the agent is already serving requests while the task progresses.

**Alternatives rejected**:
- A migration in `dependencies.rs` — no embedder available; would silently no-op.
- A blocking conversion before the agent accepts traffic — violates FR-039 and would stall startup
  proportional to corpus size.
- A dedicated progress table — `memory_passage` already answers "is this document converted?".

---

## Decision 9 — Automatic context: five on the conversational path, zero on the observation path

**Decision**: At `src/agents/process.rs:782` (Chat / AudioChat) fetch 5. At `:817` (SilentRead)
fetch 0 by default — configurable, but off. Gate both on the message being a usable query. Render
passages in `context_md` (`src/agents/agent/system_prompt/context.rs:28-44`) inside an explicit
delimiter, labelled as retrieved reference material.

**Rationale**: The two call sites look interchangeable and are not. `SilentRead` is sent for every
non-mention message in a Discord guild channel (`src/channels/discord/mod.rs:508`) and every
Telegram group message (`src/channels/telegram/mod.rs:497`), so its frequency is channel traffic,
not conversation volume. Injecting passages there multiplies memory cost by how chatty a room is,
for an agent nobody addressed. Keeping it at zero by default (FR-030) makes the feature's cost
proportional to actual conversations.

Rendering stays inside `context_md`, which `with_context` prepends to the **user message**
(`src/agents/agent/mod.rs:436`, deliberate in commit 52a8487) rather than to a system message —
that is what keeps FR-025 and SC-013 true as the block grows.

**Alternatives rejected**:
- Five on both paths — makes the highest-frequency path the most expensive one.
- Disabling SilentRead retrieval outright — removes a capability someone may want; a configurable
  budget defaulting to zero costs nothing and keeps it reachable.

---

## Decision 10 — The automatic-context threshold is measured, not guessed

**Decision**: Ship a provisional default of **0.6** and treat deriving the real value as an
explicit task: replay stored session history through the passage index and pick the value that
produces an empty block on at least 70% of real messages (SC-010).

**Rationale**: The current 0.5 (`src/agents/process.rs:782`) has no derivation behind it, and the
search tool uses 0.1 (`src/agents/tools/vector_memory/mod.rs:226`) — a 5x disagreement between two
callers of the same index, which is itself evidence nobody set either deliberately. Passage
embeddings will not score like document embeddings: a focused few-hundred-word passage is a tighter
semantic target, so cosine similarity against a real query should rise for true matches and fall for
weak ones, widening the gap the threshold sits in. The direction is predictable; the magnitude is
not, and this threshold is what decides whether the feature saves tokens or spends them.

`HistoryStorage` already persists session history, so the replay needs no new capture — it is a
one-off harness over data already on disk.

**Alternatives rejected**:
- Keeping 0.5 — tuned (if at all) for document-level scores, and applied to a payload 15x more
  expensive than the titles it was set for.
- Blocking the feature on the measurement — the provisional default is safe because FR-026 omits
  the block entirely when nothing clears the bar; a too-high threshold degrades to today's
  behaviour minus the titles, not to a regression.

---

## Decision 11 — Tool renames are mechanical and land in one commit

**Decision**: `memory_read` → `memory_search` (same file, `src/agents/tools/vector_memory/mod.rs:209`),
`memory_detail` → `memory_read` (`:421`), `memory_detail` retired. The other six `memory_*` tools are
untouched.

**Rationale**: Covered in the spec's assumptions. One point worth restating for implementation: the
two renames must land together. An intermediate state where `memory_read` means search while a new
`memory_read` means read-document cannot exist, since tool names are the dispatch key.

---

## Constraint noted, explicitly out of scope

**One vector table, per-agent embedders.** `SqliteIndexer::new` creates `document_index` with the
dimension of whichever embedder it sees first (`CREATE VIRTUAL TABLE IF NOT EXISTS ... float[{dim}]`,
`src/indexer/sqlite.rs:31`), but the indexer is built per agent from that agent's own embedding
config (`src/agents/mod.rs:78`). Two agents configured with different embedding models therefore
write mismatched vectors into one fixed-dimension table. This is pre-existing, unrelated to
chunking, and not touched here — but the conversion task must use *each agent's own* indexer rather
than any single shared one, or it would make the existing mismatch worse.


---

## Decision 12 — Drift detection compares a content hash, not re-derived spans

**Decision**: FR-038 ("detect that a document's stored passages no longer match its content") is
implemented by storing a hash of the document body alongside its passage rows and comparing hashes —
not by re-chunking and diffing spans.

**Rationale**: Re-chunking was already the more expensive option, and Decision 1b makes it unsound.
Remote embedding models are not version-stable; a provider silently updating its model would shift
seams for unchanged documents, and a span diff would conclude the entire corpus had drifted and
rebuild all of it. A content hash answers the question actually being asked — did the document
change? — in one comparison, with no embedder involved. FR-038 requires detection, not
re-derivation, so no spec change is needed.

**Alternatives rejected**:
- Re-chunk and diff spans — unsound under Decision 1b, and far more expensive.
- Storing the embedding model identity per passage and rebuilding on change — solves a different
  problem (model migration) with a rebuild trigger nobody asked for. Revisit only if embedding
  models are swapped in practice.
