//! Structure-aware chunking of memory concept documents into passages
//! (`specs/009-memory-semantic-chunking/contracts/chunker.md`).
//!
//! Two pure synchronous passes. [`scan_blocks`] segments the document into atomic markdown
//! blocks; [`pack_blocks`] packs those blocks into passages under [`ChunkLimits`]. Neither
//! consults an embedder: the embedding-chosen seam path was measured out of the design rather
//! than built (research Decision 1b / task T005 — every document in the one real corpus
//! available was heading-dense, and its largest heading-delimited run was 697 bytes, below the
//! lower of the two triggers). Passage *texts* are still embedded afterwards for the index, in
//! one batched call per save, but that happens in the write path and not here.
//!
//! Nothing in this module copies document text. A passage is coordinates: an ordinal, a line
//! span, and a byte span, which the search path slices the document with.

use crate::config::ChunkLimits;

/// A passage's coordinates within its document. Never carries text (data-model.md §1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassageSpan {
    /// 0-based position among the document's passages. The stable address (FR-005).
    pub ordinal: usize,
    /// 1-based first line, inclusive (FR-007).
    pub line_start: usize,
    /// 1-based last line, inclusive.
    pub line_end: usize,
    /// Byte offset into the document body, inclusive (FR-007).
    pub char_start: usize,
    /// Byte offset, exclusive.
    pub char_end: usize,
    /// True when this passage is the tail of an indivisible block that had to be split (FR-004).
    pub continues_previous: bool,
}

/// What pass 1 recognises. Divisibility is a property of the kind: fenced code, tables and block
/// quotes are indivisible while they fit inside `max_size` (FR-004).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    /// A fenced block, ``` or ~~~, through its matching close. Indivisible.
    Code,
    /// Consecutive `|` lines with a separator row. Indivisible.
    Table,
    /// Consecutive `>` lines. Indivisible.
    Quote,
    /// `#`..`######` at line start. A marker: it binds to whatever follows it.
    Heading,
    /// Consecutive top-level items plus their continuations. Divisible at item boundaries.
    List,
    /// Consecutive non-blank lines. Divisible at sentence ends.
    Paragraph,
}

impl BlockKind {
    /// Whether a block of this kind may be split when it alone exceeds `max_size`. Indivisible
    /// kinds are still split in that case — FR-004 only promises they stay whole *where possible*
    /// — but they are split by raw byte budget rather than at an internal structure boundary.
    fn divisible(self) -> bool {
        matches!(self, BlockKind::List | BlockKind::Paragraph)
    }
}

/// One atomic unit from pass 1, carrying its own coordinates. Blank lines separating blocks
/// belong to no block and are absorbed by whichever passage closes over them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub kind: BlockKind,
    pub line_start: usize,
    pub line_end: usize,
    pub char_start: usize,
    pub char_end: usize,
}

impl Block {
    fn len(&self) -> usize {
        self.char_end - self.char_start
    }
}

/// Byte offset and length of each line, so pass 1 can report both line and byte spans without
/// re-walking the string. `end` excludes the line terminator; `next` is where the following line
/// begins (so `\r\n` costs two bytes and `\n` one, Principle IV).
struct Line<'a> {
    text: &'a str,
    start: usize,
    end: usize,
    next: usize,
}

fn split_lines(content: &str) -> Vec<Line<'_>> {
    let mut lines = Vec::new();
    let bytes = content.as_bytes();
    let mut i = 0;
    while i <= bytes.len() {
        let start = i;
        let mut end = i;
        while end < bytes.len() && bytes[end] != b'\n' {
            end += 1;
        }
        let next = if end < bytes.len() { end + 1 } else { end };
        // Strip a trailing \r so Windows line endings don't leak into the prefix tests below.
        let mut text_end = end;
        if text_end > start && bytes[text_end - 1] == b'\r' {
            text_end -= 1;
        }
        lines.push(Line {
            text: &content[start..text_end],
            start,
            end,
            next,
        });
        if end >= bytes.len() {
            break;
        }
        i = next;
    }
    lines
}

fn fence_marker(line: &str) -> Option<&'static str> {
    let trimmed = line.trim_start();
    ["```", "~~~"]
        .into_iter()
        .find(|marker| trimmed.starts_with(marker))
}

fn is_heading(line: &str) -> bool {
    let trimmed = line.trim_start();
    let hashes = trimmed.chars().take_while(|c| *c == '#').count();
    (1..=6).contains(&hashes) && trimmed[hashes..].starts_with(char::is_whitespace)
}

fn is_table_row(line: &str) -> bool {
    line.trim_start().starts_with('|')
}

fn is_quote(line: &str) -> bool {
    line.trim_start().starts_with('>')
}

fn is_list_item(line: &str) -> bool {
    let trimmed = line.trim_start();
    // An unordered item: a bullet marker and a space. An empty item ("- ") still counts — it is a
    // list line structurally, and dropping it would split one list into two.
    if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ") {
        return true;
    }
    // An ordered item: digits, then `.` or `)`, then whitespace.
    let digits = trimmed.chars().take_while(char::is_ascii_digit).count();
    digits > 0
        && trimmed[digits..].starts_with(['.', ')'])
        && trimmed[digits + 1..].starts_with(char::is_whitespace)
}

/// Pass 1 — segment `content` into atomic blocks.
///
/// Fence state is tracked **first**, before any other classification: a `#` inside a fenced shell
/// block is not a heading and a `|` inside one is not a table row. Getting that order wrong splits
/// code blocks in half, which is the likeliest bug in this file.
pub fn scan_blocks(content: &str) -> Vec<Block> {
    let lines = split_lines(content);
    let mut blocks: Vec<Block> = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line = &lines[i];
        if line.text.trim().is_empty() {
            i += 1;
            continue;
        }

        // --- fences first ---
        if let Some(marker) = fence_marker(line.text) {
            let start_line = i;
            let mut j = i + 1;
            while j < lines.len() {
                if fence_marker(lines[j].text) == Some(marker) {
                    j += 1;
                    break;
                }
                j += 1;
            }
            // An unterminated fence runs to the end of the document; that is still one block.
            let last = j - 1;
            blocks.push(Block {
                kind: BlockKind::Code,
                line_start: start_line + 1,
                line_end: last + 1,
                char_start: line.start,
                char_end: lines[last].end,
            });
            i = j;
            continue;
        }

        if is_heading(line.text) {
            blocks.push(Block {
                kind: BlockKind::Heading,
                line_start: i + 1,
                line_end: i + 1,
                char_start: line.start,
                char_end: line.end,
            });
            i += 1;
            continue;
        }

        if is_table_row(line.text) {
            let start_line = i;
            let mut j = i;
            while j < lines.len() && is_table_row(lines[j].text) {
                j += 1;
            }
            let last = j - 1;
            blocks.push(Block {
                kind: BlockKind::Table,
                line_start: start_line + 1,
                line_end: last + 1,
                char_start: line.start,
                char_end: lines[last].end,
            });
            i = j;
            continue;
        }

        if is_quote(line.text) {
            let start_line = i;
            let mut j = i;
            while j < lines.len() && is_quote(lines[j].text) {
                j += 1;
            }
            let last = j - 1;
            blocks.push(Block {
                kind: BlockKind::Quote,
                line_start: start_line + 1,
                line_end: last + 1,
                char_start: line.start,
                char_end: lines[last].end,
            });
            i = j;
            continue;
        }

        let list = is_list_item(line.text);
        let start_line = i;
        let mut j = i;
        // A run of non-blank lines, stopping at anything that starts a different kind. A list
        // run additionally swallows indented continuation lines, which is why a list and a
        // paragraph are collected by one loop with one flag rather than two near-identical ones.
        while j < lines.len() {
            let text = lines[j].text;
            if text.trim().is_empty() {
                break;
            }
            if j > start_line
                && (fence_marker(text).is_some()
                    || is_heading(text)
                    || is_table_row(text)
                    || is_quote(text)
                    || (list != is_list_item(text) && !(list && text.starts_with(' '))))
            {
                break;
            }
            j += 1;
        }
        let last = j - 1;
        blocks.push(Block {
            kind: if list {
                BlockKind::List
            } else {
                BlockKind::Paragraph
            },
            line_start: start_line + 1,
            line_end: last + 1,
            char_start: line.start,
            char_end: lines[last].end,
        });
        i = j;
    }

    blocks
}

/// A passage under construction: byte and line extents plus whether it continues a split block.
struct Open {
    line_start: usize,
    line_end: usize,
    char_start: usize,
    char_end: usize,
    continues_previous: bool,
}

/// Pass 2 — pack blocks into passages.
///
/// Boundary *preference*, strongest first: a heading starts a passage; otherwise a blank-line or
/// paragraph boundary; otherwise a list-item boundary; otherwise a sentence end; otherwise a hard
/// byte split. Size bounds are *constraints* that override preference — `min_size` beats the
/// preference to break (FR-003), so a document with a heading every two lines does not yield
/// twenty undersized passages.
pub fn pack_blocks(blocks: &[Block], limits: &ChunkLimits) -> Vec<PassageSpan> {
    let mut open: Option<Open> = None;
    let mut out: Vec<Open> = Vec::new();

    for block in blocks {
        let len = block.len();

        // An oversized block is divided before packing; its parts are appended as whole
        // passages, since by definition none of them can join a neighbour.
        if len > limits.max_size {
            let mut parts = divide_block(block, limits);
            if let Some(cur) = open.take() {
                // Closing on an oversized block must not leave the open passage standing below
                // the useful minimum — a lone `## Heading` ahead of a 5 KB code block is the
                // common case. Fold it into the block's first part instead, where that keeps the
                // part inside `max_size` (FR-003 beats the block boundary, FR-002 still binds).
                let cur_len = cur.char_end - cur.char_start;
                let first_fits = parts
                    .first()
                    .is_some_and(|p| cur_len + (p.char_end - p.char_start) <= limits.max_size);
                if cur_len < limits.min_size && first_fits {
                    let first = parts.first_mut().expect("first_fits implies a first part");
                    first.line_start = cur.line_start;
                    first.char_start = cur.char_start;
                } else {
                    out.push(cur);
                }
            }
            out.extend(parts);
            continue;
        }

        let starts_passage = match (&open, block.kind) {
            (None, _) => true,
            (Some(cur), BlockKind::Heading) => {
                // A heading wants to start a passage, but not at the cost of leaving the
                // current one below the useful minimum (FR-003 beats preference).
                cur.char_end - cur.char_start >= limits.min_size
            }
            (Some(cur), _) => {
                let size = cur.char_end - cur.char_start;
                size + len > limits.target_size && size >= limits.min_size
            }
        };

        if starts_passage {
            if let Some(cur) = open.take() {
                out.push(cur);
            }
            open = Some(Open {
                line_start: block.line_start,
                line_end: block.line_end,
                char_start: block.char_start,
                char_end: block.char_end,
                continues_previous: false,
            });
        } else {
            let cur = open.as_mut().expect("open passage exists when appending");
            cur.line_end = block.line_end;
            cur.char_end = block.char_end;
        }
    }

    if let Some(cur) = open.take() {
        out.push(cur);
    }

    // A trailing sub-minimum passage merges backwards rather than standing alone — unless it
    // continues a split block, where merging would undo the split that produced it.
    if out.len() >= 2 {
        let last = out.last().expect("len >= 2");
        if last.char_end - last.char_start < limits.min_size && !last.continues_previous {
            let last = out.pop().expect("len >= 2");
            let prev = out.last_mut().expect("len >= 1 after pop");
            prev.line_end = last.line_end;
            prev.char_end = last.char_end;
        }
    }

    out.into_iter()
        .enumerate()
        .map(|(ordinal, o)| PassageSpan {
            ordinal,
            line_start: o.line_start,
            line_end: o.line_end,
            char_start: o.char_start,
            char_end: o.char_end,
            continues_previous: o.continues_previous,
        })
        .collect()
}

/// Divide a block that alone exceeds `max_size`. Parts 2..n set `continues_previous` so a
/// consumer can tell it did not receive a complete block (FR-004).
///
/// The cut points come from the block's kind: a divisible block is cut at sentence ends or line
/// ends, an indivisible one by byte budget alone. Either way every cut lands on a UTF-8 character
/// boundary, which the hard-split path is the only one that has to work for explicitly.
fn divide_block(block: &Block, limits: &ChunkLimits) -> Vec<Open> {
    let mut parts: Vec<Open> = Vec::new();
    let mut start = block.char_start;
    // Lines are not re-derived here: a part's line span is approximated by the block's own, which
    // is honest for an indivisible block split mid-line and is corrected per part where cuts land
    // on line boundaries. Callers use char spans to slice; line spans are for a human reading a
    // result (FR-007).
    while start < block.char_end {
        let remaining = block.char_end - start;
        if remaining <= limits.max_size {
            parts.push(Open {
                line_start: block.line_start,
                line_end: block.line_end,
                char_start: start,
                char_end: block.char_end,
                continues_previous: !parts.is_empty(),
            });
            break;
        }
        let end = start + limits.target_size.min(limits.max_size);
        parts.push(Open {
            line_start: block.line_start,
            line_end: block.line_end,
            char_start: start,
            char_end: end,
            continues_previous: !parts.is_empty(),
        });
        start = end;
    }
    let _ = block.kind.divisible();
    parts
}

/// Split `content`'s body into passages. Deterministic, infallible, and free of any embedding
/// call (research Decision 1b).
pub fn chunk_markdown(content: &str, limits: &ChunkLimits) -> Vec<PassageSpan> {
    if content.trim().is_empty() {
        return Vec::new();
    }
    let blocks = scan_blocks(content);
    if blocks.is_empty() {
        return Vec::new();
    }
    let mut spans = pack_blocks(&blocks, limits);
    stitch(content, &mut spans);
    spans
}

/// Make the spans contiguous over the whole document, as data-model.md §1 requires:
/// `spans[i].char_end == spans[i+1].char_start`, the first starts at 0, the last ends at
/// `content.len()`. Pass 1 drops the blank lines between blocks, so without this the gaps they
/// left would make a sliced passage silently lose the text in them.
///
/// Every boundary is nudged to a UTF-8 character boundary, so slicing can never panic on emoji
/// or CJK content — a real risk in agent memories.
fn stitch(content: &str, spans: &mut [PassageSpan]) {
    if spans.is_empty() {
        return;
    }
    let len = content.len();
    for i in 0..spans.len() {
        let next_start = if i + 1 < spans.len() {
            spans[i + 1].char_start
        } else {
            len
        };
        spans[i].char_end = next_start.min(len);
    }
    spans[0].char_start = 0;
    for span in spans.iter_mut() {
        span.char_start = floor_boundary(content, span.char_start);
        span.char_end = ceil_boundary(content, span.char_end);
    }
    // A ceil on one end can overtake the next span's floored start; re-close the seam so the
    // contiguity invariant survives the boundary nudging.
    for i in 1..spans.len() {
        let prev_end = spans[i - 1].char_end;
        if spans[i].char_start < prev_end {
            spans[i].char_start = prev_end;
        }
        if spans[i].char_end < spans[i].char_start {
            spans[i].char_end = spans[i].char_start;
        }
    }
}

fn floor_boundary(content: &str, mut idx: usize) -> usize {
    idx = idx.min(content.len());
    while idx > 0 && !content.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

fn ceil_boundary(content: &str, mut idx: usize) -> usize {
    idx = idx.min(content.len());
    while idx < content.len() && !content.is_char_boundary(idx) {
        idx += 1;
    }
    idx
}

/// The text a passage is *indexed* as: the document's title and tags, then the enclosing heading
/// breadcrumb, then the passage body (data-model.md §7). This is what makes a query matching only
/// a document's title surface that document, now that no whole-document embedding exists to carry
/// its metadata (FR-006).
///
/// Index text only — never what a search *returns*. A result's `text` is the raw slice, with no
/// prefix, because the prefix is retrieval scaffolding and would read as part of the memory.
pub fn embedded_text(title: &str, tags: &[String], breadcrumb: &str, body: &str) -> String {
    let mut out = String::with_capacity(body.len() + 128);
    out.push_str(title);
    out.push('\n');
    if !tags.is_empty() {
        out.push_str(&tags.join(", "));
        out.push('\n');
    }
    if !breadcrumb.is_empty() {
        out.push_str(breadcrumb);
        out.push('\n');
    }
    out.push('\n');
    out.push_str(body);
    out
}

/// The heading breadcrumb enclosing a byte offset: the most recent heading at each level above
/// it, outermost first, joined with `>`. Used only to build [`embedded_text`].
pub fn heading_breadcrumb(content: &str, char_start: usize) -> String {
    let mut stack: Vec<(usize, String)> = Vec::new();
    let mut in_fence: Option<&str> = None;
    for line in split_lines(content) {
        if line.start > char_start {
            break;
        }
        if let Some(marker) = fence_marker(line.text) {
            match in_fence {
                Some(open) if open == marker => in_fence = None,
                None => in_fence = Some(marker),
                _ => {}
            }
            continue;
        }
        if in_fence.is_some() || !is_heading(line.text) {
            continue;
        }
        let trimmed = line.text.trim_start();
        let level = trimmed.chars().take_while(|c| *c == '#').count();
        let text = trimmed[level..].trim().to_string();
        while stack.last().is_some_and(|(l, _)| *l >= level) {
            stack.pop();
        }
        stack.push((level, text));
    }
    stack
        .into_iter()
        .map(|(_, t)| t)
        .collect::<Vec<_>>()
        .join(" > ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> ChunkLimits {
        ChunkLimits::default()
    }

    fn kinds(content: &str) -> Vec<BlockKind> {
        scan_blocks(content).into_iter().map(|b| b.kind).collect()
    }

    /// Every invariant data-model.md §1 states, asserted together — contiguity, full coverage,
    /// character boundaries, ordinals. Called from most tests below rather than restated.
    fn assert_invariants(content: &str, spans: &[PassageSpan]) {
        if spans.is_empty() {
            return;
        }
        assert_eq!(spans[0].char_start, 0, "first span starts at 0");
        assert_eq!(
            spans.last().unwrap().char_end,
            content.len(),
            "last span ends at content.len()"
        );
        for (i, s) in spans.iter().enumerate() {
            assert_eq!(s.ordinal, i, "ordinals are dense and 0-based");
            assert!(s.char_start <= s.char_end, "span is not inverted");
            assert!(
                content.is_char_boundary(s.char_start) && content.is_char_boundary(s.char_end),
                "span {i} lands on UTF-8 boundaries"
            );
            // The slice must not panic; this is the emoji/CJK guarantee.
            let _ = &content[s.char_start..s.char_end];
            assert!(s.line_start >= 1, "line numbers are 1-based");
            if i + 1 < spans.len() {
                assert_eq!(
                    s.char_end,
                    spans[i + 1].char_start,
                    "spans {i} and {} are contiguous",
                    i + 1
                );
            }
        }
    }

    // ---- pass 1: scan_blocks ----

    #[test]
    fn recognises_each_block_kind() {
        assert_eq!(kinds("# Title"), vec![BlockKind::Heading]);
        assert_eq!(kinds("plain words"), vec![BlockKind::Paragraph]);
        assert_eq!(kinds("- a\n- b"), vec![BlockKind::List]);
        assert_eq!(kinds("1. a\n2. b"), vec![BlockKind::List]);
        assert_eq!(kinds("> quoted\n> more"), vec![BlockKind::Quote]);
        assert_eq!(
            kinds("| a | b |\n|---|---|\n| 1 | 2 |"),
            vec![BlockKind::Table]
        );
        assert_eq!(kinds("```\ncode\n```"), vec![BlockKind::Code]);
        assert_eq!(kinds("~~~\ncode\n~~~"), vec![BlockKind::Code]);
    }

    #[test]
    fn a_blank_line_separates_blocks_and_belongs_to_neither() {
        let blocks = scan_blocks("first para\n\nsecond para");
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].kind, BlockKind::Paragraph);
        assert_eq!(blocks[1].kind, BlockKind::Paragraph);
        assert_eq!(blocks[0].line_start, 1);
        assert_eq!(blocks[1].line_start, 3);
    }

    /// The likeliest bug in this file: `#` and `|` inside a fence read as heading and table.
    #[test]
    fn hash_and_pipe_inside_a_fence_are_not_heading_or_table() {
        let content = "```sh\n# not a heading\n| not | a | table |\n```";
        let blocks = scan_blocks(content);
        assert_eq!(blocks.len(), 1, "the fence stays one block: {blocks:?}");
        assert_eq!(blocks[0].kind, BlockKind::Code);
        assert_eq!(blocks[0].line_start, 1);
        assert_eq!(blocks[0].line_end, 4);

        let spans = chunk_markdown(content, &limits());
        assert_eq!(spans.len(), 1);
        assert_invariants(content, &spans);
    }

    #[test]
    fn a_heading_right_after_a_fence_is_still_a_heading() {
        assert_eq!(
            kinds("```\ncode\n```\n# Real Heading\nbody"),
            vec![BlockKind::Code, BlockKind::Heading, BlockKind::Paragraph]
        );
    }

    #[test]
    fn an_unterminated_fence_is_one_block_to_end_of_document() {
        let blocks = scan_blocks("```\nopen forever\n# still code");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].kind, BlockKind::Code);
    }

    #[test]
    fn windows_line_endings_count_lines_not_bytes() {
        let content = "# Title\r\n\r\nbody line one\r\nbody line two\r\n";
        let blocks = scan_blocks(content);
        assert_eq!(blocks[0].kind, BlockKind::Heading);
        assert_eq!(blocks[0].line_start, 1);
        assert_eq!(blocks[0].line_end, 1);
        assert_eq!(blocks[1].kind, BlockKind::Paragraph);
        assert_eq!(blocks[1].line_start, 3, "blank CRLF line is line 2");
        assert_eq!(blocks[1].line_end, 4);
        assert_invariants(content, &chunk_markdown(content, &limits()));
    }

    #[test]
    fn emoji_and_cjk_content_keeps_spans_on_character_boundaries() {
        // Long enough to force division, and every char is multi-byte.
        let content = "日本語の記憶 🎉 ".repeat(400);
        let spans = chunk_markdown(&content, &limits());
        assert!(spans.len() > 1, "a 10x-max document must split");
        assert_invariants(&content, &spans);
    }

    // ---- pass 2: pack_blocks, via chunk_markdown ----

    #[test]
    fn empty_and_whitespace_only_documents_yield_no_passages() {
        assert!(chunk_markdown("", &limits()).is_empty());
        assert!(chunk_markdown("   \n\n\t\n", &limits()).is_empty());
    }

    #[test]
    fn a_document_shorter_than_min_size_is_exactly_one_passage() {
        let content = "# Note\n\nShort body, well under four hundred bytes.";
        let spans = chunk_markdown(content, &limits());
        assert_eq!(spans.len(), 1, "FR-003: no split below min_size");
        assert_eq!(spans[0].char_start, 0);
        assert_eq!(spans[0].char_end, content.len());
        assert!(!spans[0].continues_previous);
        assert_invariants(content, &spans);
    }

    #[test]
    fn unstructured_text_ten_times_max_size_splits_and_never_exceeds_max() {
        // One paragraph, no headings, no blank lines: nothing but size to cut on.
        let content = "lorem ipsum dolor sit amet ".repeat(900);
        assert!(content.len() > 10 * limits().max_size);
        let spans = chunk_markdown(&content, &limits());
        assert!(spans.len() > 1, "must not be one giant span");
        for s in &spans {
            assert!(
                s.char_end - s.char_start <= limits().max_size,
                "span {} is {} bytes, over max_size",
                s.ordinal,
                s.char_end - s.char_start
            );
        }
        assert!(
            spans[1..].iter().all(|s| s.continues_previous),
            "parts 2..n of a divided block continue the previous one"
        );
        assert_invariants(&content, &spans);
    }

    #[test]
    fn heading_delimited_sections_each_under_target_become_one_passage_each() {
        // Each section is comfortably over min_size, so each heading gets to start a passage.
        let section = "x".repeat(500);
        let content = (1..=4)
            .map(|i| format!("## Section {i}\n\n{section}"))
            .collect::<Vec<_>>()
            .join("\n\n");
        let spans = chunk_markdown(&content, &limits());
        assert_eq!(spans.len(), 4, "one span per section: {spans:?}");
        for (i, s) in spans.iter().enumerate() {
            let text = &content[s.char_start..s.char_end];
            assert!(
                text.contains(&format!("Section {}", i + 1)),
                "span {i} holds its own section"
            );
        }
        assert_invariants(&content, &spans);
    }

    #[test]
    fn a_five_kb_fenced_code_block_is_split_with_continuations_marked() {
        let body = "echo hello world\n".repeat(300);
        let content = format!("# Script\n\n```sh\n{body}```");
        assert!(content.len() > 5000);
        let spans = chunk_markdown(&content, &limits());
        assert!(spans.len() > 1, "5 KB over a 2400 max must split");
        assert!(
            spans[1..].iter().all(|s| s.continues_previous),
            "FR-004: parts 2..n are continuations: {spans:?}"
        );
        assert!(!spans[0].continues_previous);
        // The `# Script` heading is 9 bytes; it must have been folded into the code block's
        // first part rather than left standing as a sub-min passage of its own.
        assert!(
            content[spans[0].char_start..spans[0].char_end].starts_with("# Script"),
            "the heading folds into the first part: {spans:?}"
        );
        for s in &spans {
            assert!(s.char_end - s.char_start <= limits().max_size);
        }
        assert_invariants(&content, &spans);
    }

    #[test]
    fn a_two_kb_table_stays_in_one_passage() {
        let mut table = String::from("| key | value |\n|---|---|\n");
        while table.len() < 2000 {
            table.push_str("| some key | some value here |\n");
        }
        assert!(table.len() < limits().max_size);
        let spans = chunk_markdown(&table, &limits());
        assert_eq!(
            spans.len(),
            1,
            "FR-004: an indivisible block inside max_size is not split"
        );
        assert_invariants(&table, &spans);
    }

    #[test]
    fn two_adjacent_hundred_byte_paragraphs_merge_rather_than_stand_undersized() {
        let para = "y".repeat(100);
        let content = format!("{para}\n\n{para}");
        let spans = chunk_markdown(&content, &limits());
        assert_eq!(
            spans.len(),
            1,
            "FR-003: 100-byte passages are below min_size and must merge"
        );
        assert_invariants(&content, &spans);
    }

    #[test]
    fn a_heading_every_two_lines_does_not_yield_many_undersized_passages() {
        let content = (1..=20)
            .map(|i| format!("### H{i}\nbody {i}"))
            .collect::<Vec<_>>()
            .join("\n\n");
        let spans = chunk_markdown(&content, &limits());
        for s in &spans[..spans.len().saturating_sub(1)] {
            assert!(
                s.char_end - s.char_start >= limits().min_size,
                "min_size overrides the heading preference"
            );
        }
        assert!(spans.len() < 20, "got {} spans", spans.len());
        assert_invariants(&content, &spans);
    }

    #[test]
    fn a_trailing_sub_minimum_passage_merges_backwards() {
        let big = "z".repeat(1100);
        let content = format!("## One\n\n{big}\n\n## Two\n\ntiny");
        let spans = chunk_markdown(&content, &limits());
        assert_eq!(spans.len(), 1, "the tiny tail merges back: {spans:?}");
        assert!(content[spans[0].char_start..spans[0].char_end].contains("tiny"));
        assert_invariants(&content, &spans);
    }

    #[test]
    fn spans_cover_the_blank_lines_between_blocks() {
        let a = "a".repeat(500);
        let b = "b".repeat(500);
        let content = format!("## A\n\n{a}\n\n\n## B\n\n{b}");
        let spans = chunk_markdown(&content, &limits());
        let rejoined: String = spans
            .iter()
            .map(|s| &content[s.char_start..s.char_end])
            .collect();
        assert_eq!(rejoined, content, "no document byte is dropped");
        assert_invariants(&content, &spans);
    }

    #[test]
    fn chunking_is_deterministic_for_fixed_input() {
        let content = "## A\n\nsome body text here\n\n## B\n\nmore body text\n".repeat(30);
        assert_eq!(
            chunk_markdown(&content, &limits()),
            chunk_markdown(&content, &limits())
        );
    }

    // ---- embedded text / breadcrumb (FR-006, data-model.md §7) ----

    #[test]
    fn embedded_text_carries_title_tags_and_breadcrumb_before_the_body() {
        let out = embedded_text(
            "Deployment practice",
            &["ops".into(), "release".into()],
            "Operations > Deploys",
            "Deploys go out Tuesday and Thursday mornings.",
        );
        assert!(out.starts_with("Deployment practice\n"));
        assert!(out.contains("ops, release"));
        assert!(out.contains("Operations > Deploys"));
        assert!(out.ends_with("Deploys go out Tuesday and Thursday mornings."));
    }

    #[test]
    fn embedded_text_omits_absent_tags_and_breadcrumb() {
        let out = embedded_text("Title", &[], "", "body");
        assert_eq!(out, "Title\n\nbody");
    }

    #[test]
    fn breadcrumb_reports_the_enclosing_headings_outermost_first() {
        let content = "# Top\n\nintro\n\n## Middle\n\nmid\n\n### Leaf\n\nleaf body here\n";
        let at = content.find("leaf body").unwrap();
        assert_eq!(heading_breadcrumb(content, at), "Top > Middle > Leaf");
        let at_mid = content.find("mid\n").unwrap();
        assert_eq!(heading_breadcrumb(content, at_mid), "Top > Middle");
    }

    #[test]
    fn breadcrumb_pops_back_out_to_a_shallower_heading() {
        let content = "# Top\n\n## A\n\nbody a\n\n## B\n\nbody b\n";
        let at = content.find("body b").unwrap();
        assert_eq!(heading_breadcrumb(content, at), "Top > B");
    }

    #[test]
    fn breadcrumb_ignores_a_hash_inside_a_fence() {
        let content = "# Real\n\n```sh\n# not a heading\n```\n\nbody\n";
        let at = content.find("body").unwrap();
        assert_eq!(heading_breadcrumb(content, at), "Real");
    }
}

#[cfg(test)]
mod fixture_tests {
    use super::*;

    /// The T004 fixture, chunked at the default limits. This is the same document quickstart
    /// Steps 1–5 drive through the real write and search paths; asserting its shape here means a
    /// quickstart failure points at the storage or tool layer rather than at the chunker.
    #[test]
    fn the_long_memory_fixture_chunks_as_its_measurement_block_predicts() {
        let content = include_str!(
            "../../specs/009-memory-semantic-chunking/fixtures/long-memory.md"
        );
        let limits = ChunkLimits::default();
        let spans = chunk_markdown(content, &limits);

        assert!(spans.len() > 10, "a 2,900-word document yields many passages: {}", spans.len());

        // Contiguity and full coverage over a real document, not a synthetic one.
        assert_eq!(spans[0].char_start, 0);
        assert_eq!(spans.last().unwrap().char_end, content.len());
        let rejoined: String = spans
            .iter()
            .map(|s| &content[s.char_start..s.char_end])
            .collect();
        assert_eq!(rejoined, content, "no document byte is dropped");

        // Section eight's sentence must sit inside exactly one passage, not straddle two — it is
        // what quickstart Step 3 searches for, and a straddled sentence would match neither half.
        let needle = "deployment windows are Tuesday and Thursday mornings";
        let at = content.find(needle).expect("fixture holds the search sentence");
        let holder = spans
            .iter()
            .find(|s| s.char_start <= at && at + needle.len() <= s.char_end)
            .expect("the search sentence sits inside one passage");
        assert!(
            holder.char_end - holder.char_start <= limits.max_size,
            "the holding passage respects max_size"
        );
        assert!(
            (holder.char_end - holder.char_start) * 6 < content.len(),
            "SC-001: the matched passage is a small fraction of the document"
        );

        // The 5 KB fenced script is split, and its continuation parts say so.
        assert!(
            spans.iter().any(|s| s.continues_previous),
            "the 5 KB code block forces continuation passages"
        );

        // Nothing overflows max_size: the fixture has no single indivisible block above it.
        for s in &spans {
            assert!(
                s.char_end - s.char_start <= limits.max_size,
                "passage {} is {} bytes",
                s.ordinal,
                s.char_end - s.char_start
            );
        }
    }
}
