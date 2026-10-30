//! Line-level diff between two document snapshots, shared by CORE and memory history.
//!
//! Kind-agnostic: takes two strings, returns unified-diff-style hunks (`similar`, Myers).
//! `\r\n` is normalized to `\n` first so a document saved from Windows and re-saved from Linux
//! doesn't show every line as changed, and a missing final newline is supplied so that adding
//! a line after the last one reads as a pure insertion rather than "last line replaced".

use similar::{ChangeTag, TextDiff};

use crate::schema::{DiffHunk, DiffLine};

/// Context lines kept around each change when grouping into hunks.
const CONTEXT: usize = 3;

fn normalize(text: &str) -> String {
    let mut out = text.replace("\r\n", "\n");
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Returns `(hunks, additions, deletions)` describing how to turn `from` into `to`.
/// Identical inputs produce no hunks and zero counts.
pub fn diff_lines(from: &str, to: &str) -> (Vec<DiffHunk>, usize, usize) {
    let from = normalize(from);
    let to = normalize(to);
    let diff = TextDiff::from_lines(from.as_str(), to.as_str());

    let mut additions = 0;
    let mut deletions = 0;
    let mut hunks = Vec::new();

    for group in diff.grouped_ops(CONTEXT) {
        let mut lines = Vec::new();
        let mut old_start: Option<usize> = None;
        let mut new_start: Option<usize> = None;
        let mut old_lines = 0;
        let mut new_lines = 0;

        for op in &group {
            for change in diff.iter_changes(op) {
                let old_line = change.old_index().map(|i| i + 1);
                let new_line = change.new_index().map(|i| i + 1);
                let op = match change.tag() {
                    ChangeTag::Equal => "equal",
                    ChangeTag::Insert => "insert",
                    ChangeTag::Delete => "delete",
                };
                if old_line.is_some() {
                    old_lines += 1;
                    old_start.get_or_insert(old_line.unwrap_or(1));
                }
                if new_line.is_some() {
                    new_lines += 1;
                    new_start.get_or_insert(new_line.unwrap_or(1));
                }
                match change.tag() {
                    ChangeTag::Insert => additions += 1,
                    ChangeTag::Delete => deletions += 1,
                    ChangeTag::Equal => {}
                }
                lines.push(DiffLine {
                    op: op.to_string(),
                    old_line,
                    new_line,
                    text: change.value().trim_end_matches('\n').to_string(),
                });
            }
        }

        // A side with zero lines in this hunk starts "after" the position of the other side, the
        // way unified diff reports `-0,0` / `+N,0`.
        let old_start = old_start.unwrap_or_else(|| {
            group
                .first()
                .map(|op| op.old_range().start)
                .unwrap_or(0)
        });
        let new_start = new_start.unwrap_or_else(|| {
            group
                .first()
                .map(|op| op.new_range().start)
                .unwrap_or(0)
        });

        hunks.push(DiffHunk {
            old_start,
            old_lines,
            new_start,
            new_lines,
            lines,
        });
    }

    (hunks, additions, deletions)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ops(hunk: &DiffHunk) -> Vec<(&str, Option<usize>, Option<usize>, &str)> {
        hunk.lines
            .iter()
            .map(|l| (l.op.as_str(), l.old_line, l.new_line, l.text.as_str()))
            .collect()
    }

    #[test]
    fn identical_input_yields_no_hunks() {
        let (hunks, add, del) = diff_lines("a\nb\nc\n", "a\nb\nc\n");
        assert!(hunks.is_empty());
        assert_eq!((add, del), (0, 0));
    }

    #[test]
    fn single_insert() {
        let (hunks, add, del) = diff_lines("a\nb\n", "a\nx\nb\n");
        assert_eq!((add, del), (1, 0));
        assert_eq!(hunks.len(), 1);
        assert_eq!(
            ops(&hunks[0]),
            vec![
                ("equal", Some(1), Some(1), "a"),
                ("insert", None, Some(2), "x"),
                ("equal", Some(2), Some(3), "b"),
            ]
        );
        assert_eq!((hunks[0].old_start, hunks[0].old_lines), (1, 2));
        assert_eq!((hunks[0].new_start, hunks[0].new_lines), (1, 3));
    }

    #[test]
    fn single_delete() {
        let (hunks, add, del) = diff_lines("a\nx\nb\n", "a\nb\n");
        assert_eq!((add, del), (0, 1));
        assert_eq!(hunks.len(), 1);
        assert_eq!(
            ops(&hunks[0]),
            vec![
                ("equal", Some(1), Some(1), "a"),
                ("delete", Some(2), None, "x"),
                ("equal", Some(3), Some(2), "b"),
            ]
        );
    }

    #[test]
    fn replace_is_delete_plus_insert_in_one_hunk() {
        let (hunks, add, del) = diff_lines("a\nold\nb\n", "a\nnew\nb\n");
        assert_eq!((add, del), (1, 1));
        assert_eq!(hunks.len(), 1);
        let o = ops(&hunks[0]);
        assert!(o.contains(&("delete", Some(2), None, "old")));
        assert!(o.contains(&("insert", None, Some(2), "new")));
    }

    #[test]
    fn crlf_is_normalized() {
        let unix = diff_lines("a\nb\n", "a\nc\n");
        let windows = diff_lines("a\r\nb\r\n", "a\r\nc\r\n");
        assert_eq!(unix, windows);
        // and CRLF-only differences are not changes at all
        let (hunks, _, _) = diff_lines("a\r\nb\r\n", "a\nb\n");
        assert!(hunks.is_empty());
    }

    #[test]
    fn appending_a_line_after_an_unterminated_last_line_is_a_pure_insert() {
        let (hunks, add, del) = diff_lines("a\nb", "a\nb\nc\n");
        assert_eq!((add, del), (1, 0));
        assert_eq!(hunks.len(), 1);
        assert!(hunks[0].lines.iter().all(|l| l.op != "delete"));
        // and a trailing newline on its own is not a change
        assert!(diff_lines("a\nb", "a\nb\n").0.is_empty());
    }

    #[test]
    fn distant_changes_become_separate_hunks() {
        let from: String = (1..=20).map(|i| format!("l{i}\n")).collect();
        let to = from.replace("l2\n", "L2\n").replace("l19\n", "L19\n");
        let (hunks, add, del) = diff_lines(&from, &to);
        assert_eq!((add, del), (2, 2));
        assert_eq!(hunks.len(), 2);
    }

    #[test]
    fn empty_from_reports_all_insertions() {
        let (hunks, add, del) = diff_lines("", "a\nb\n");
        assert_eq!((add, del), (2, 0));
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].old_lines, 0);
        assert_eq!(hunks[0].new_lines, 2);
    }
}
