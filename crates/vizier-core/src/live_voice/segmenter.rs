//! Reply text into speakable segments (`specs/055-live-voice` T031, FR-013, FR-014).
//!
//! The reply arrives whole (the engine does not stream tokens, `llm/sse.rs`), so the first segment
//! is cut at the first sentence end after roughly [`FIRST_BOUNDARY_CHARS`] characters: that is what
//! puts a first word in the person's ear before the rest of the answer is synthesised, and why that
//! number is small. A segment is
//! never cut inside a word, and a reply made of code, tables and links is not read aloud at all —
//! it is announced once and left on screen.

/// Where the first segment prefers to end. Short enough to speak soon, long enough to be a clause.
///
/// 40, not 80: the whole first segment has to be synthesised before one sample is sent, so this
/// number is a direct multiplier on how long the person waits for the first word. Halving it halves
/// that wait at the cost of one more segment per reply (F2, latency work after a live session).
pub const FIRST_BOUNDARY_CHARS: usize = 40;
/// No segment grows past this; a wall of text is cut at the nearest clause or word.
pub const MAX_SEGMENT_CHARS: usize = 400;
/// What is said instead of reading code, tables or a URL aloud.
pub const NOTICE: &str = "The details are on your screen.";

/// One thing to synthesise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub text: String,
    /// The reply's last segment, so the client knows to return to `listening` (FR-015).
    pub last: bool,
}

/// Cut `text` into segments in reading order.
pub fn segment(text: &str) -> Vec<Segment> {
    let mut skipped = false;
    let prose = strip_blocks(text, &mut skipped);

    let mut parts: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut first = true;

    for word in prose.split_whitespace() {
        if is_url(word) {
            skipped = true;
            continue;
        }
        let word = clean(word);
        if word.is_empty() {
            continue;
        }
        // A segment never grows past the cap: the word that would overflow it starts the next
        // one instead, so a cut always falls between words (FR-013).
        let grows = current.chars().count() + 1 + word.chars().count();
        if !current.is_empty() && grows > MAX_SEGMENT_CHARS {
            parts.push(std::mem::take(&mut current));
            first = false;
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(&word);

        // A boundary is only usable once the segment is long enough to be worth saying, which is
        // what keeps time-to-first-word inside the budget for the first segment.
        let threshold = if first { FIRST_BOUNDARY_CHARS } else { MAX_SEGMENT_CHARS };
        if ends_sentence(&word) && current.chars().count() >= threshold {
            parts.push(std::mem::take(&mut current));
            first = false;
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }

    // One notice for the whole reply, not one per block: a voice that says "the details are on
    // your screen" three times is worse than saying it once (FR-014).
    if skipped {
        parts.push(NOTICE.to_string());
    }

    let count = parts.len();
    parts
        .into_iter()
        .enumerate()
        .map(|(at, text)| Segment { text, last: at + 1 == count })
        .collect()
}

/// Drop fenced code and table rows, keeping everything else, and remember that something was
/// dropped so one notice can be said for the whole reply.
fn strip_blocks(text: &str, skipped: &mut bool) -> String {
    let mut out = String::new();
    let mut in_fence = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            *skipped = true;
            continue;
        }
        if in_fence {
            continue;
        }
        if is_table_row(trimmed) {
            *skipped = true;
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn is_table_row(line: &str) -> bool {
    line.starts_with('|') || line.matches('|').count() >= 2
}

fn is_url(word: &str) -> bool {
    let lower = word.to_ascii_lowercase();
    lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("www.")
        || lower.contains("://")
}


/// Strip the markdown a voice cannot say: emphasis, inline code ticks, heading marks and
/// quote marks. A word is never otherwise altered.
fn clean(word: &str) -> String {
    word.chars()
        .filter(|c| !matches!(c, '`' | '*' | '_' | '#' | '>'))
        .collect()
}

/// Sentence enders. A clause ender is not enough to close the first segment unless it is long.
fn ends_sentence(word: &str) -> bool {
    word.chars()
        .last()
        .is_some_and(|c| matches!(c, '.' | '!' | '?' | ':' | ';'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(segments: &[Segment]) -> Vec<&str> {
        segments.iter().map(|s| s.text.as_str()).collect()
    }

    /// FR-013: the first segment ends at the first sentence end after ~40 characters, so the
    /// person hears something before the whole reply is synthesised.
    #[test]
    fn the_first_segment_ends_after_the_preferred_boundary() {
        let reply = "This is a short opening clause that runs on for a while without stopping at all, and then it ends properly. The rest follows here. And a third sentence to finish the reply off.";
        let segments = segment(reply);
        assert!(segments.len() >= 2);
        assert!(segments[0].text.ends_with('.'));
        assert!(
            segments[0].text.chars().count() >= FIRST_BOUNDARY_CHARS,
            "first segment was {:?}",
            segments[0].text
        );
        assert!(!segments[0].last);
        assert!(segments.last().unwrap().last);
        assert_eq!(segments.iter().filter(|s| s.last).count(), 1);
    }

    /// F2: the first cut comes after the *first* sentence once it passes the threshold, not after
    /// two of them. With 80 the two sentences below would have been spoken as one segment, which is
    /// twice the synthesis before a single sample leaves.
    #[test]
    fn the_first_segment_is_one_sentence_not_two() {
        let first = "This opening sentence is about forty five characters long.";
        let second = "This second sentence is also about that long, roughly.";
        let reply = format!("{first} {second} And a third one closes the reply.");
        let segments = segment(&reply);
        assert!(segments.len() >= 2, "{segments:?}");
        assert_eq!(
            segments[0].text, first,
            "the first segment is the first sentence, not the first two: {segments:?}"
        );
        assert!(
            segments[0].text.chars().count() < 80,
            "well under the old boundary: {:?}",
            segments[0].text
        );
    }

    /// A boundary is only used at a word end: no segment splits a word in half.
    #[test]
    fn no_segment_splits_inside_a_word() {
        let reply = "antidisestablishmentarianism ".repeat(40);
        let segments = segment(&reply);
        for segment in &segments {
            for word in segment.text.split_whitespace() {
                assert_eq!(word, "antidisestablishmentarianism");
            }
        }
        assert!(segments.len() > 1, "a wall of text is still cut");
        assert!(
            segments
                .iter()
                .all(|s| s.text.chars().count() <= MAX_SEGMENT_CHARS)
        );
    }

    /// FR-014: fenced code, tables and raw URLs are skipped, and the reply says so once — not once
    /// per block.
    #[test]
    fn code_tables_and_links_are_skipped_with_one_notice() {
        let reply = "Here is the fix.\n\n```rust\nfn main() {}\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\nSee https://example.com/docs for the rest.\n";
        let segments = segment(reply);
        let joined = segments.iter().map(|s| s.text.clone()).collect::<Vec<_>>().join(" | ");
        assert!(!joined.contains("fn main"), "{joined}");
        assert!(!joined.contains("example.com"), "{joined}");
        assert!(!joined.contains("| a |"), "{joined}");
        assert_eq!(
            segments.iter().filter(|s| s.text == NOTICE).count(),
            1,
            "one notice for the whole reply"
        );
    }

    /// A reply that is only code still says something rather than nothing.
    #[test]
    fn a_reply_of_only_code_is_the_notice_alone() {
        let segments = segment("```\nSELECT 1;\n```\n");
        assert_eq!(texts(&segments), vec![NOTICE]);
        assert!(segments[0].last);
    }

    /// Markdown a voice cannot pronounce is stripped, not spelled out.
    #[test]
    fn markdown_is_not_spoken() {
        let segments = segment("## Heading\n\n**bold** and _italic_ and `code` are read as words.");
        let joined = segments.iter().map(|s| s.text.clone()).collect::<Vec<_>>().join(" ");
        assert!(!joined.contains('*'));
        assert!(!joined.contains('`'));
        assert!(!joined.contains('#'));
        assert!(joined.contains("bold"));
    }

    #[test]
    fn an_empty_reply_has_no_segments() {
        assert!(segment("   \n\n ").is_empty());
    }
}
