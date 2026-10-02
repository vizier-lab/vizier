# Contract: `chunk_markdown`

**Feature**: `009-memory-semantic-chunking` | New module: `src/storage/chunk.rs`

```rust
fn scan_blocks(content: &str) -> Vec<Block>;                     // pass 1 — pure, sync

fn pack_blocks(                                                   // pass 2 — pure, sync
    blocks: &[Block],
    limits: &ChunkLimits,
    block_embeddings: Option<&[Vec<f64>]>,
) -> Vec<PassageSpan>;

async fn chunk_markdown(                                          // orchestrator
    content: &str,
    limits: &ChunkLimits,
    embedder: &dyn VizierEmbeddingModel,
) -> Result<Vec<PassageSpan>>;
```

No trait (research Decision 1). Both passes are pure and synchronous, which is where the test effort
belongs given how sparse the suite is. `pack_blocks` takes *precomputed* block embeddings, so its
seam logic is tested with hand-written vectors and no embedder at all; pass `None` to test pure
size-packing. The orchestrator depends on `&dyn VizierEmbeddingModel`, not `VizierEmbedder` — the
latter's `build` is private to the `embedding` module, so a test could not construct one. A stub over
the narrow trait is two lines.

## Pass 1 — `scan_blocks`

A line-walking state machine. **Fence state is tracked first**: a `#` inside a fenced shell block is
not a heading, and a `|` inside one is not a table row. Getting that order wrong splits code blocks
in half, which is the likeliest bug in this file.

| Block kind | Recognised by | Divisible |
|---|---|---|
| `Code` | ` ``` ` or `~~~` through its matching close | No |
| `Table` | consecutive `\|` lines with a separator row | No |
| `Quote` | consecutive `>` lines | No |
| `Heading` | `#`..`######` at line start | Marker — binds to what follows |
| `List` | consecutive top-level items plus continuations | At item boundaries |
| `Paragraph` | consecutive non-blank lines | At sentence ends |

Each `Block` carries its line range, byte range, and kind. Blank lines are separators belonging to no
block.

## Pass 2 — `pack_blocks`

Boundary *preference*, strongest first:

1. A `Heading` block starts a new passage (FR-001).
2. The lowest consecutive-block similarity, when `block_embeddings` is supplied for this run.
3. Blank-line / paragraph boundary.
4. List-item boundary at the top nesting level.
5. Sentence end, only when one paragraph alone exceeds `max_size`.
6. Hard byte split at a UTF-8 boundary — last resort, sets `continues_previous` (FR-004).

Size bounds are *constraints* that override preference:

```
for each block:
    if block is Heading               -> start a new passage
    else if current + block <= target -> append
    else if current < min             -> append anyway        (FR-003 beats preference)
    else                              -> close, start new
    if block alone > max              -> divide per its kind;
                                         parts 2..n set continues_previous
if last passage < min                 -> merge backwards
```

A document with a heading every two lines therefore does not yield twenty undersized passages — the
min-size rule merges them back.

## When embeddings are consulted (research Decision 1b)

`chunk_markdown` embeds a run's blocks only when that run, under one heading, exceeds `max_size` or
`2 × min_size`. One batched `embed_texts` call per such run; a heading-dense document issues none. On
embedder error the orchestrator calls `pack_blocks(.., None)` and the save proceeds (FR-036) — the
error is logged, never propagated to the write.

## Indivisible blocks (FR-004)

Fenced code, tables, and block quotes are never split while the result fits inside `max_size`, even
when that overshoots `target_size`. A single block larger than `max_size` is split, and every part
after the first sets `continues_previous = true` so a consumer can tell it did not receive a
complete block.

## Required cases

| Input | Expected |
|---|---|
| Empty / whitespace-only | `[]` — and no index rows, so nothing to search |
| Shorter than `min_size` | Exactly one span covering everything (FR-003) |
| No headings, no blank lines, 10x `max_size` | Multiple spans, each `<= max_size` (never one giant span) |
| Heading-delimited sections each under `target_size` | One span per section |
| A 5 KB fenced code block with `max_size` 2400 | Split; parts 2..n set `continues_previous` |
| A 2 KB table with `max_size` 2400 | One span, unsplit |
| `#` and `\|` inside a fenced code block | Not treated as heading or table; block stays whole |
| Two adjacent 100-byte paragraphs, `min_size` 400 | Merged into one span, not two undersized ones |
| Emoji / CJK content | Spans land on character boundaries; slicing never panics |
| Windows line endings (`\r\n`) | Line numbers count lines, not bytes; spans stay valid (Principle IV) |
| `pack_blocks` with `None` embeddings | Pure size-packing — the pre-embedding behaviour |
| `pack_blocks` with embeddings whose weakest seam differs from the size-greedy cut | Cuts at the weak seam, not the byte budget |
| `pack_blocks` where the weak seam would leave a sub-`min` passage | Size constraint wins; seam ignored |
| Oversized run, embedder errors | Falls back to size-packing; the save still succeeds (FR-036) |

## Invariants the implementation must hold

- Contiguous and non-overlapping: `spans[i].char_end == spans[i+1].char_start`.
- `spans.first().char_start == 0` and `spans.last().char_end == content.len()`.
- `scan_blocks` and `pack_blocks` are deterministic for fixed inputs, including fixed embeddings.
  `chunk_markdown` as a whole is **not** stable across embedding-model versions, which is why FR-038
  compares a document content hash rather than re-derived spans (research Decision 12).
- Every `char_start`/`char_end` is a valid UTF-8 boundary in `content`.
