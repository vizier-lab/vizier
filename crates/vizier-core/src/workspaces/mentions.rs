//! Finds `@handle` mentions in a message (`specs/053-workspace-channel/research.md` R4).
//!
//! Pure: it takes the member handles and returns the ones named. It skips inline code, fenced
//! blocks and quoted lines, matches the longest handle (so no rule for what an id may contain is
//! needed), and requires that the handle is not followed by a word character, so `@alpha-two`
//! is not a mention of `alpha`.

/// One mention found in a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mention {
    /// The member's handle as stored, whatever case was typed.
    pub handle: String,
    /// Byte offset of the `@`.
    pub start: usize,
    /// Byte offset just past the handle.
    pub end: usize,
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Whether the text after a match continues the word: a letter, digit or `_`, or `-` and then one.
fn continues(rest: &str) -> bool {
    let mut chars = rest.chars();
    match chars.next() {
        Some(c) if is_word(c) => true,
        Some('-') => chars.next().is_some_and(is_word),
        _ => false,
    }
}

/// Every distinct member mentioned, in order of first appearance.
pub fn parse(text: &str, handles: &[&str]) -> Vec<Mention> {
    let mut found: Vec<Mention> = Vec::new();
    let mut in_fence = false;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let line_start = offset;
        offset += line.len();
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence || trimmed.starts_with('>') {
            continue;
        }
        scan_line(line, line_start, handles, &mut found);
    }
    found
}

fn scan_line(line: &str, base: usize, handles: &[&str], found: &mut Vec<Mention>) {
    let mut in_code = false;
    let mut previous: Option<char> = None;
    for (i, c) in line.char_indices() {
        if c == '`' {
            in_code = !in_code;
        } else if c == '@' && !in_code && !previous.is_some_and(|p| is_word(p) || p == '.') {
            let rest = &line[i + 1..];
            let best = handles
                .iter()
                .filter(|h| {
                    rest.len() >= h.len()
                        && rest.is_char_boundary(h.len())
                        && rest[..h.len()].eq_ignore_ascii_case(h)
                        && !continues(&rest[h.len()..])
                })
                .max_by_key(|h| h.len());
            if let Some(handle) = best.filter(|h| !found.iter().any(|m| m.handle == **h)) {
                found.push(Mention {
                    handle: (*handle).to_string(),
                    start: base + i,
                    end: base + i + 1 + handle.len(),
                });
            }
        }
        previous = Some(c);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MEMBERS: &[&str] = &["alpha", "beta", "alpha-two", "rafi"];

    fn names(text: &str) -> Vec<String> {
        parse(text, MEMBERS).into_iter().map(|m| m.handle).collect()
    }

    #[test]
    fn finds_members_in_order() {
        assert_eq!(names("@beta and @alpha, please"), ["beta", "alpha"]);
    }

    #[test]
    fn the_longest_handle_wins() {
        assert_eq!(names("@alpha-two look"), ["alpha-two"]);
    }

    #[test]
    fn a_handle_that_continues_into_a_longer_word_is_not_a_mention() {
        assert!(names("@alphabet").is_empty());
        assert!(names("@alpha_x").is_empty());
        assert!(names("@alpha-x").is_empty());
    }

    #[test]
    fn punctuation_after_a_handle_still_counts() {
        assert_eq!(names("@alpha, @beta. @rafi! (@alpha)"), ["alpha", "beta", "rafi"]);
        assert_eq!(names("@alpha-"), ["alpha"]);
    }

    #[test]
    fn repeats_count_once() {
        assert_eq!(names("@alpha @alpha again @ALPHA"), ["alpha"]);
    }

    #[test]
    fn non_members_and_bare_ats_are_ignored() {
        assert!(names("@nobody and @ alone").is_empty());
    }

    #[test]
    fn email_addresses_are_not_mentions() {
        assert!(names("mail me at rafi@alpha.com").is_empty());
    }

    #[test]
    fn code_spans_fences_and_quotes_are_skipped() {
        assert!(names("`@alpha`").is_empty());
        assert!(names("```\n@alpha\n```").is_empty());
        assert!(names("> @alpha said so").is_empty());
        assert_eq!(names("`@alpha` but @beta"), ["beta"]);
        assert_eq!(names("```\n@alpha\n```\n@beta"), ["beta"]);
    }

    #[test]
    fn case_is_ignored_and_the_stored_handle_comes_back() {
        assert_eq!(names("@Rafi hi"), ["rafi"]);
    }

    #[test]
    fn offsets_cover_the_at_sign_and_the_handle() {
        let found = parse("hi @beta!", MEMBERS);
        assert_eq!((found[0].start, found[0].end), (3, 8));
    }

    #[test]
    fn multibyte_text_does_not_panic() {
        assert_eq!(names("héllo @alpha — ok"), ["alpha"]);
        assert!(names("@é").is_empty());
    }
}
