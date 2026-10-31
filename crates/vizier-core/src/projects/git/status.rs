//! Parsers for git's machine formats (`specs/041-project-coding-workspace` R12), kept pure so
//! fixtures recorded from real repositories cover renames, conflicts, detached heads, spaces
//! and non-ASCII paths without spawning git in the tests.

use std::path::Path;

use super::{BranchHolder, GitBranch, GitChange, GitChangeKind, GitCommit, Status, StatusEntry};

/// Files larger than this are counted as unknown rather than read for a line count.
pub const UNTRACKED_COUNT_CAP_BYTES: u64 = 4 * 1024 * 1024;

fn kind_of(xy: &str) -> GitChangeKind {
    let mut chars = xy.chars();
    let x = chars.next().unwrap_or('.');
    let y = chars.next().unwrap_or('.');
    if x == 'U' || y == 'U' || (x == 'A' && y == 'A') || (x == 'D' && y == 'D') {
        return GitChangeKind::Conflicted;
    }
    if x == 'R' || y == 'R' {
        return GitChangeKind::Renamed;
    }
    if x == 'A' || y == 'A' {
        return GitChangeKind::Added;
    }
    if x == 'D' || y == 'D' {
        return GitChangeKind::Deleted;
    }
    GitChangeKind::Modified
}

/// Parse `git status --porcelain=v2 -z --branch --untracked-files=all`.
pub fn parse_status(text: &str) -> Status {
    let mut status = Status::default();
    let mut fields = text.split('\0').filter(|field| !field.is_empty()).peekable();

    while let Some(field) = fields.next() {
        if let Some(header) = field.strip_prefix("# ") {
            let mut parts = header.splitn(2, ' ');
            match (parts.next(), parts.next()) {
                (Some("branch.head"), Some("(detached)")) => {
                    status.detached = true;
                    status.branch = None;
                }
                (Some("branch.head"), Some(head)) => status.branch = Some(head.to_string()),
                (Some("branch.ab"), Some(ab)) => {
                    for part in ab.split_whitespace() {
                        if let Some(ahead) = part.strip_prefix('+') {
                            status.ahead = ahead.parse().unwrap_or(0);
                        } else if let Some(behind) = part.strip_prefix('-') {
                            status.behind = behind.parse().unwrap_or(0);
                        }
                    }
                }
                _ => {}
            }
            continue;
        }

        match field.as_bytes().first().copied() {
            Some(b'?') => {
                let path = field[1..].trim_start_matches(' ').to_string();
                status.entries.push(StatusEntry {
                    path,
                    old_path: None,
                    kind: GitChangeKind::Untracked,
                });
            }
            // Ignored entries are not requested, but tolerate them.
            Some(b'!') => {}
            Some(b'1') => {
                let parts: Vec<&str> = field.splitn(9, ' ').collect();
                if parts.len() == 9 {
                    status.entries.push(StatusEntry {
                        path: parts[8].to_string(),
                        old_path: None,
                        kind: kind_of(parts[1]),
                    });
                }
            }
            Some(b'2') => {
                let parts: Vec<&str> = field.splitn(10, ' ').collect();
                if parts.len() == 10 {
                    let original = fields.next().map(str::to_string);
                    status.entries.push(StatusEntry {
                        path: parts[9].to_string(),
                        old_path: original,
                        kind: GitChangeKind::Renamed,
                    });
                }
            }
            Some(b'u') => {
                let parts: Vec<&str> = field.splitn(11, ' ').collect();
                if parts.len() == 11 {
                    status.entries.push(StatusEntry {
                        path: parts[10].to_string(),
                        old_path: None,
                        kind: GitChangeKind::Conflicted,
                    });
                }
            }
            _ => {}
        }
    }
    status
}

/// Parse `git diff HEAD --numstat -z` into `path → (added, removed)`.
pub fn parse_numstat(text: &str) -> std::collections::HashMap<String, (u64, u64)> {
    let mut out = std::collections::HashMap::new();
    let fields: Vec<&str> = text.split('\0').collect();
    let mut i = 0;
    while i < fields.len() {
        let field = fields[i];
        i += 1;
        if field.is_empty() {
            continue;
        }
        let mut parts = field.splitn(3, '\t');
        let added = parts.next().unwrap_or("");
        let removed = parts.next().unwrap_or("");
        let path = parts.next().unwrap_or("");
        let parse = |text: &str| text.parse::<u64>().ok();
        if path.is_empty() {
            // A rename: the next two NUL fields are the old and the new path.
            let new_path = fields.get(i + 1).copied().unwrap_or("");
            i += 2;
            out.insert(
                new_path.to_string(),
                (parse(added).unwrap_or(0), parse(removed).unwrap_or(0)),
            );
        } else {
            out.insert(
                path.to_string(),
                (parse(added).unwrap_or(0), parse(removed).unwrap_or(0)),
            );
        }
    }
    out
}

/// Count a file's lines without loading a huge file; `None` for binaries and files over the cap.
pub fn count_lines(path: &Path) -> Option<u64> {
    let metadata = std::fs::metadata(path).ok()?;
    if metadata.len() > UNTRACKED_COUNT_CAP_BYTES {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    if bytes.iter().take(8192).any(|b| *b == 0) {
        return None;
    }
    if bytes.is_empty() {
        return Some(0);
    }
    let newlines = bytes.iter().filter(|b| **b == b'\n').count() as u64;
    Some(newlines + u64::from(!bytes.ends_with(b"\n")))
}

/// Join the parsed status entries with line counts from numstat, plus untracked line counts.
pub fn changes_with_counts(
    status: &Status,
    numstat: &str,
    dir: &Path,
) -> Vec<GitChange> {
    let counts = parse_numstat(numstat);
    status
        .entries
        .iter()
        .map(|entry| {
            let (added, removed) = match entry.kind {
                GitChangeKind::Untracked => {
                    let lines = count_lines(&dir.join(&entry.path)).unwrap_or(0);
                    (lines, 0)
                }
                _ => counts.get(&entry.path).copied().unwrap_or((0, 0)),
            };
            GitChange {
                path: entry.path.clone(),
                old_path: entry.old_path.clone(),
                kind: entry.kind,
                added,
                removed,
            }
        })
        .collect()
}

/// An all-added unified diff for a file git does not track yet.
pub fn synthetic_untracked_diff(path: &str, bytes: &[u8], binary: bool) -> String {
    if binary {
        return format!("Binary file {path} added\n");
    }
    let text = String::from_utf8_lossy(bytes);
    let lines = text.lines().count();
    let mut out = format!(
        "diff --git a/{path} b/{path}\nnew file mode 100644\n--- /dev/null\n+++ b/{path}\n@@ -0,0 +1,{lines} @@\n"
    );
    for line in text.lines() {
        out.push('+');
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Parse `for-each-ref refs/heads` (`name\x1fupstream`) plus `worktree list --porcelain`.
pub fn parse_branches(refs: &str, current: Option<&str>, worktrees: &str) -> Vec<GitBranch> {
    // path → branch name, for the branches another worktree already holds.
    let mut holders: std::collections::HashMap<String, String> = Default::default();
    // The first block is the project's own folder: its branch is "current", not held elsewhere.
    let mut main: Option<String> = None;
    let mut path = String::new();
    for line in worktrees.lines() {
        if let Some(found) = line.strip_prefix("worktree ") {
            path = found.to_string();
            if main.is_none() {
                main = Some(path.clone());
            }
        } else if let Some(reference) = line.strip_prefix("branch ")
            && let Some(branch) = reference.strip_prefix("refs/heads/")
            && main.as_deref() != Some(path.as_str())
        {
            holders.insert(branch.to_string(), path.clone());
        }
    }

    refs.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let name = line.split('\x1f').next().unwrap_or(line).to_string();
            let checked_out_in = holders.get(&name).map(|holder| BranchHolder {
                worktree_id: None,
                name: holder.rsplit(['/', '\\']).next().unwrap_or(holder).to_string(),
            });
            GitBranch {
                current: current == Some(name.as_str()),
                name,
                checked_out_in,
            }
        })
        .collect()
}

/// Parse `git log --format=%H%x1f%an%x1f%aI%x1f%s`.
pub fn parse_log(text: &str) -> Vec<GitCommit> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| {
            let mut parts = line.splitn(4, '\x1f');
            Some(GitCommit {
                hash: parts.next()?.to_string(),
                author: parts.next()?.to_string(),
                date: parts.next()?.to_string(),
                subject: parts.next().unwrap_or("").to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status_fixture() -> String {
        [
            "# branch.oid 1234567890abcdef",
            "# branch.head main",
            "# branch.upstream origin/main",
            "# branch.ab +2 -1",
            "1 .M N... 100644 100644 100644 abcdef abcdef src/lib.rs",
            "1 A. N... 000000 100644 100644 000000 abcdef src/new.rs",
            "2 R. N... 100644 100644 100644 abcdef abcdef R100 new name.rs",
            "old name.rs",
            "u UU N... 100644 100644 100644 100644 a b c src/conflict.rs",
            "? target/out.txt",
            "? a file with spaces.txt",
            "? héllo.rs",
        ]
        .join("\0")
    }

    #[test]
    fn porcelain_v2_parses_every_record_kind() {
        let status = parse_status(&status_fixture());
        assert_eq!(status.branch.as_deref(), Some("main"));
        assert_eq!((status.ahead, status.behind), (2, 1));
        assert_eq!(status.entries.len(), 7);
        assert_eq!(status.entries[0].kind, GitChangeKind::Modified);
        assert_eq!(status.entries[1].kind, GitChangeKind::Added);
        assert_eq!(status.entries[2].kind, GitChangeKind::Renamed);
        assert_eq!(status.entries[2].path, "new name.rs");
        assert_eq!(status.entries[2].old_path.as_deref(), Some("old name.rs"));
        assert_eq!(status.entries[3].kind, GitChangeKind::Conflicted);
        assert_eq!(status.entries[4].path, "target/out.txt");
        assert_eq!(status.entries[5].path, "a file with spaces.txt");
        assert_eq!(status.entries[6].path, "héllo.rs");
    }

    #[test]
    fn a_detached_head_has_no_branch() {
        let text = ["# branch.oid 1234", "# branch.head (detached)"].join("\0");
        let status = parse_status(&text);
        assert!(status.detached);
        assert_eq!(status.branch, None);
    }

    #[test]
    fn numstat_including_a_rename_is_mapped_by_path() {
        let text = "3\t1\tsrc/lib.rs\0 0\t0\t\0old.rs\0new.rs\0";
        let counts = parse_numstat(text);
        assert_eq!(counts.get("src/lib.rs"), Some(&(3, 1)));
        assert_eq!(counts.get("new.rs"), Some(&(0, 0)));
    }

    #[test]
    fn untracked_counts_come_from_the_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join("sub/a.txt"), "one\ntwo\nthree").unwrap();
        std::fs::write(dir.path().join("binary"), b"a\0b").unwrap();
        let status = Status {
            entries: vec![
                StatusEntry { path: "sub/a.txt".into(), old_path: None, kind: GitChangeKind::Untracked },
                StatusEntry { path: "binary".into(), old_path: None, kind: GitChangeKind::Untracked },
            ],
            ..Default::default()
        };
        let changes = changes_with_counts(&status, "", dir.path());
        assert_eq!(changes[0].added, 3);
        assert_eq!(changes[1].added, 0, "a binary's line count is unknown");
    }

    #[test]
    fn branches_mark_the_holder_worktree_by_its_folder_name() {
        let refs = "master\x1forigin/master\nfix-ps\x1f\n";
        let worktrees = [
            "worktree /repo",
            "HEAD 1234",
            "branch refs/heads/master",
            "",
            "worktree /repo/.vizier/dp-a/fix-ps",
            "HEAD abcd",
            "branch refs/heads/fix-ps",
        ]
        .join("\n");
        let branches = parse_branches(refs, Some("master"), &worktrees);
        assert!(branches[0].current);
        assert_eq!(branches[0].checked_out_in, None);
        assert_eq!(
            branches[1].checked_out_in.as_ref().unwrap().name,
            "fix-ps"
        );
    }

    #[test]
    fn log_lines_parse_into_commits() {
        let text = "abc\x1fAda\x1f2026-01-02T03:04:05Z\x1finit\n";
        let commits = parse_log(text);
        assert_eq!(commits[0].hash, "abc");
        assert_eq!(commits[0].subject, "init");
    }

    #[test]
    fn a_synthetic_untracked_diff_is_all_added() {
        let diff = synthetic_untracked_diff("a.txt", b"one\ntwo\n", false);
        assert!(diff.contains("+one"), "{diff}");
        assert!(diff.contains("+two"), "{diff}");
        assert!(diff.contains("new file mode"), "{diff}");
    }
}
