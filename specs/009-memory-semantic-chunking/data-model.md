# Phase 1 Data Model: Semantic Chunking for Memory Recall

**Feature**: `009-memory-semantic-chunking` | **Date**: 2026-10-01

Nothing here is a new source of truth. Memory concept documents on disk remain authoritative
(`specs/004-memory-open-format/`); everything below is derived and rebuildable from them.

---

## 1. `PassageSpan` (in-memory, `src/storage/chunk.rs`)

What `chunk_markdown` returns. Coordinates only — it never copies document text.

| Field | Type | Meaning |
|---|---|---|
| `ordinal` | `usize` | 0-based position among the document's passages. Stable address (FR-005). |
| `line_start` | `usize` | 1-based first line, inclusive (FR-007). |
| `line_end` | `usize` | 1-based last line, inclusive. |
| `char_start` | `usize` | Byte offset into the document body, inclusive (FR-007). |
| `char_end` | `usize` | Byte offset, exclusive. |
| `continues_previous` | `bool` | True when this passage is the tail of an indivisible block that had to be split (FR-004). |

`chunk_markdown` returns these; `scan_blocks`/`pack_blocks` are the pure passes behind it
(contracts/chunker.md).

**Invariants**
- At least one span for any non-empty document (SC-007: zero documents end up with no passages).
- Spans are contiguous and non-overlapping: `spans[i].char_end == spans[i+1].char_start`.
- `char_start`/`char_end` land on UTF-8 character boundaries. Slicing must not panic on multi-byte
  content; this is a real risk with emoji and CJK in agent memories.
- Every span is `>= min_size` except where the document itself is smaller (FR-003), and `<= max_size`
  except where FR-004 permits overflow for an indivisible block.

## 2. `ChunkLimits` (config)

| Field | Default | Requirement |
|---|---|---|
| `target_size` | 1200 bytes (≈200 words) | FR-002 |
| `min_size` | 400 bytes | FR-003 |
| `max_size` | 2400 bytes | FR-002 |

Defaults are provisional, bounded by FR-002/FR-041, and chosen so five passages cost roughly a
sixth of five whole documents on a typical corpus. Byte-denominated rather than token-denominated
because the chunker must stay synchronous and provider-agnostic — no tokenizer dependency
(Principle I).

## 3. `memory_passage` (new sqlite table)

Added to `init_memory_graph_schema` (`src/storage/sqlite/mod.rs:33`) so the existing
`memory_bundle` unit tests can stand it up against an in-memory connection without the full schema.

```sql
CREATE TABLE IF NOT EXISTS memory_passage (
    agent_id    TEXT    NOT NULL,
    bundle      TEXT    NOT NULL,
    path        TEXT    NOT NULL,
    ordinal     INTEGER NOT NULL,
    line_start  INTEGER NOT NULL,
    line_end    INTEGER NOT NULL,
    char_start  INTEGER NOT NULL,
    char_end    INTEGER NOT NULL,
    continues   INTEGER NOT NULL DEFAULT 0,
    content_hash TEXT   NOT NULL,
    PRIMARY KEY (agent_id, bundle, path, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_memory_passage_doc
    ON memory_passage(agent_id, bundle, path);
```

**Lifecycle**
| Event | Effect | Requirement |
|---|---|---|
| `write_memory` | Delete all rows for the document, insert the new set, in one transaction | FR-033 |
| `delete_memory` | Delete all rows for the document | FR-034 |
| `delete_bundle` | Delete all rows for the bundle | FR-034 |
| `import_bundle` | Insert rows for every imported document | FR-035 |
| reconcile | A document with no rows is unconverted; a document whose `content_hash` differs from its body has drifted. Rebuild either way | FR-037, FR-038, FR-039 |

**Absence is the resume marker.** No separate progress table (research Decision 8). A document with
zero rows has not been chunked yet; one with rows has.

**`content_hash` is how drift is detected**, not re-chunking (research Decision 12). It is the hash
of the document body the passages were derived from, repeated on every row of that document. Since
pass 2 consults embeddings, re-deriving spans is no longer stable across embedding-model versions —
a provider silently updating its model would make every unchanged document look drifted. Comparing
one hash answers the real question in one step and needs no embedder.

## 4. `document_index` (existing vec0 table — unchanged DDL)

`src/indexer/sqlite.rs:31`. No schema change. Row granularity changes from one-per-document to
one-per-passage, addressed by key:

```
{agent_id}/{bundle}/{path}#{ordinal}
```

Parsed by an extended `parse_indexer_key` (`src/storage/memory_bundle.rs:201`), which already
tolerates slashes in `path` via `splitn(3, '/')`. Documents carry no extension and no `#`, so the
suffix is unambiguous.

## 5. `DocumentIndex` — one field added

`src/schema/storage.rs:109`.

```rust
pub struct DocumentIndex {
    pub path: String,
    pub embedding: Vec<f64>,
    pub context: String,
    pub score: f64,   // NEW: cosine similarity, 1.0 - distance
}
```

Required by FR-010. The value is already computed for the threshold filter and currently thrown
away (research Decision 4). `NoopIndexer` returns `0.0`; it returns no search results at all, so
the value is never read from it.

## 6. `MemoryPassageResult` (new, returned by search)

What `query_memory` returns in place of `Vec<Memory>`.

| Field | Type | Requirement |
|---|---|---|
| `bundle` | `String` | FR-009 |
| `path` | `String` | FR-009 |
| `title` | `String` | FR-009 (metadata match support, FR-006) |
| `ordinal` | `usize` | FR-009 — first ordinal when merged |
| `ordinal_end` | `usize` | FR-011 — last ordinal when merged; equals `ordinal` otherwise |
| `line_start`, `line_end` | `usize` | FR-009 |
| `text` | `String` | Sliced from the document, never stored (Decision 3) |
| `score` | `f64` | FR-010 |
| `truncated` | `bool` | FR-028, automatic context only |

**Merge rule (FR-011)**: results from the same document with consecutive ordinals collapse into one
whose `text` spans `char_start` of the first through `char_end` of the last, keeping the best
`score`. Applied *after* the per-document cap (research Decision 7).

## 7. Embedded text per passage (FR-006)

Each passage is embedded as its document's title and tags followed by the passage body:

```
{title}
{tags joined by ", "}

{passage text}
```

This is what makes a query matching only a document's title surface that document, now that no
whole-document embedding exists to carry metadata. Cost: the prefix repeats per passage, which is a
few dozen bytes against a target of 1200.

## 8. What is deliberately absent

- **No passage versioning.** `memory_revision` (`specs/006-memory-version-history/`) stays
  document-scoped. Passages are derived; a revision of a derived artifact has no meaning.
- **No passage-level links.** `memory_edge` stays document-scoped. Links are authored, passages are not.
- **No passage text in sqlite.** Decision 3.
- **No cross-document passages.** A passage belongs to exactly one document.
