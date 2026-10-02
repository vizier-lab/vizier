# Contract: `chunk_markdown`

**Feature**: `009-memory-semantic-chunking` | New module: `src/storage/chunk.rs`

```rust
fn scan_blocks(content: &str) -> Vec<Block>;                     // pass 1 — pure, sync
fn pack_blocks(blocks: &[Block], limits: &ChunkLimits) -> Vec<PassageSpan>;  // pass 2 — pure, sync
pub fn chunk_markdown(content: &str, limits: &ChunkLimits) -> Vec<PassageSpan>;
```

No trait (research Decision 1). Both passes are pure and synchronous, which is where the test effort
belongs given how sparse the suite is.

**The embedding-chosen seam path was measured out of this contract, not built** (research Decision 1b,
task T005). Against the only real corpus available, every document was heading-dense and the largest
heading-delimited run was 697 bytes — below `2 × min_size` and 3.4x below `max_size`, so neither
embedding trigger fired on a single document. `pack_blocks` therefore takes no `block_embeddings`
argument, and `chunk_markdown` takes no embedder, is synchronous, and cannot fail. The seam
*preference* order below is what survives; preference 2 is struck.

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
2. ~~The lowest consecutive-block similarity~~ — struck; see above.
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

## Embeddings are not consulted (research Decision 1b, measured by T005)

No embedding call is made anywhere in the chunker. Passage *texts* are still embedded afterwards, for
the index, in one batched `add_document_indexes` call per save (research Decision 6) — that is the
write path, not the chunker.

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
| A paragraph alone larger than `max_size` | Split at sentence ends, then hard-split; parts 2..n continue |

## Invariants the implementation must hold

- Contiguous and non-overlapping: `spans[i].char_end == spans[i+1].char_start`.
- `spans.first().char_start == 0` and `spans.last().char_end == content.len()`.
- `scan_blocks`, `pack_blocks` and `chunk_markdown` are all deterministic for fixed input. FR-038
  still compares a document content hash rather than re-derived spans, because one hash comparison is
  cheaper than re-chunking and diffing (research Decision 12).
- Every `char_start`/`char_end` is a valid UTF-8 boundary in `content`.
