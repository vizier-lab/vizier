//! Applying an edit set to a page version (`specs/063-html-generative-ui` research R3): exact
//! find/replace pairs, in order, each applied to the text the one before produced and matching
//! exactly once there. All or nothing: the caller keeps the base when any edit fails.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One exact replacement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Edit {
    /// Text that occurs exactly once in the page at this point of the set, copied exactly
    /// (whitespace included). Make it long enough to be unique.
    pub find: String,
    /// What replaces it. May be empty, to delete `find`.
    pub replace: String,
}

/// Why an edit set was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    /// Edit `index` has an empty `find`.
    EmptyFind { index: usize },
    /// Edit `index`'s `find` occurs nowhere at its point of the set.
    NotFound { index: usize },
    /// Edit `index`'s `find` occurs `count` times at its point of the set.
    Ambiguous { index: usize, count: usize },
}

/// `base` with every edit applied in order, or why not.
pub fn apply_edits(base: &str, edits: &[Edit]) -> Result<String, EditError> {
    let mut text = base.to_string();
    for (index, edit) in edits.iter().enumerate() {
        if edit.find.is_empty() {
            return Err(EditError::EmptyFind { index });
        }
        let mut found = text.match_indices(edit.find.as_str());
        let Some((at, _)) = found.next() else {
            return Err(EditError::NotFound { index });
        };
        let others = found.count();
        if others > 0 {
            return Err(EditError::Ambiguous { index, count: others + 1 });
        }
        text.replace_range(at..at + edit.find.len(), &edit.replace);
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(find: &str, replace: &str) -> Edit {
        Edit { find: find.into(), replace: replace.into() }
    }

    #[test]
    fn edits_apply_in_order_each_to_the_text_before_it() {
        let base = "<h1>Hi</h1><p>one</p>";
        let out = apply_edits(base, &[e("Hi", "Hello"), e("<h1>Hello</h1>", "<h1>Hello there</h1>"), e("<p>one</p>", "")]);
        assert_eq!(out.unwrap(), "<h1>Hello there</h1>");
    }

    #[test]
    fn a_find_must_match_exactly_once() {
        assert_eq!(apply_edits("a b a", &[e("a", "x")]), Err(EditError::Ambiguous { index: 0, count: 2 }));
        assert_eq!(apply_edits("a b", &[e("b", "c"), e("zzz", "y")]), Err(EditError::NotFound { index: 1 }));
        assert_eq!(apply_edits("a", &[e("", "y")]), Err(EditError::EmptyFind { index: 0 }));
    }

    #[test]
    fn a_later_edit_may_match_text_an_earlier_one_made_unique() {
        // "item" is ambiguous at first; after the first edit only one remains.
        let out = apply_edits("<li>item</li><li>item</li>", &[e("<li>item</li><li>", "<li>first</li><li>"), e("item", "second")]);
        assert_eq!(out.unwrap(), "<li>first</li><li>second</li>");
    }

    #[test]
    fn multi_byte_text_is_replaced_by_bytes_not_characters() {
        let out = apply_edits("♟ pawn ♞ knight", &[e("♞ knight", "♝ bishop")]);
        assert_eq!(out.unwrap(), "♟ pawn ♝ bishop");
    }

    #[test]
    fn overlapping_matches_count_as_separate_occurrences() {
        // `match_indices` does not overlap: "aaa" holds "aa" once.
        assert_eq!(apply_edits("aaa", &[e("aa", "b")]).unwrap(), "ba");
    }
}
