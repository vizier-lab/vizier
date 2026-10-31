//! Gitignore-aware walking shared by the listing, search, glob and `@` suggestion tools
//! (`specs/041-project-coding-workspace` R5). Hidden files are shown; `.git/` and, from the
//! project root, `.vizier/` are always skipped.

use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

/// The most entries any one walk returns; callers cap their own results below this.
pub const MAX_WALK_ENTRIES: usize = 50_000;

/// The include-ignored switch: `false` honours `.gitignore`, `.git/info/exclude` and global
/// excludes; `true` reaches ignored files deliberately, but still never `.git/`/`.vizier/`.
pub fn builder(cwd: &Path, include_ignored: bool) -> WalkBuilder {
    let mut builder = WalkBuilder::new(cwd);
    builder
        .hidden(false)
        .git_ignore(!include_ignored)
        .git_exclude(!include_ignored)
        .git_global(!include_ignored)
        .parents(true)
        .require_git(false)
        .follow_links(false);
    let root = cwd.to_path_buf();
    builder.filter_entry(move |entry| {
        let name = entry.file_name().to_string_lossy();
        if name == ".git" {
            return false;
        }
        if name == ".vizier" && entry.path().parent() == Some(root.as_path()) {
            return false;
        }
        true
    });
    builder
}

/// One walked path and whether it is a directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Walked {
    pub path: PathBuf,
    pub is_dir: bool,
}

/// Every file and folder under `cwd` (the root itself excluded), bounded by
/// [`MAX_WALK_ENTRIES`]. Synchronous: callers run it in `spawn_blocking`.
pub fn walk(cwd: &Path, include_ignored: bool) -> Vec<Walked> {
    let mut out = Vec::new();
    for entry in builder(cwd, include_ignored).build().flatten() {
        if entry.depth() == 0 {
            continue;
        }
        let is_dir = entry.file_type().is_some_and(|kind| kind.is_dir());
        out.push(Walked {
            path: entry.into_path(),
            is_dir,
        });
        if out.len() >= MAX_WALK_ENTRIES {
            break;
        }
    }
    out
}

/// Whether bytes look binary: a NUL in the first 8 KiB (the rule every tool uses).
pub fn is_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(8 * 1024).any(|byte| *byte == 0)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn tree() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src/nested")).unwrap();
        fs::create_dir_all(dir.path().join("target")).unwrap();
        fs::create_dir_all(dir.path().join("nested/.git")).unwrap();
        fs::create_dir_all(dir.path().join(".vizier/dp-a/wt")).unwrap();
        fs::write(dir.path().join(".gitignore"), "target/\n").unwrap();
        fs::write(dir.path().join("src/lib.rs"), "x").unwrap();
        fs::write(dir.path().join("src/nested/.gitignore"), "*.skip\n").unwrap();
        fs::write(dir.path().join("src/nested/y.skip"), "x").unwrap();
        fs::write(dir.path().join("src/nested/.keep"), "x").unwrap();
        fs::write(dir.path().join("target/out.txt"), "x").unwrap();
        fs::write(dir.path().join(".vizier/dp-a/wt/x"), "x").unwrap();
        dir
    }

    fn names(dir: &Path, include_ignored: bool) -> Vec<String> {
        walk(dir, include_ignored)
            .into_iter()
            .map(|w| {
                w.path
                    .strip_prefix(dir)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect()
    }

    #[test]
    fn gitignore_rules_are_honoured_including_nested_and_negated_files() {
        let dir = tree();
        let listed = names(dir.path(), false);
        assert!(listed.contains(&"src/lib.rs".to_string()), "{listed:?}");
        assert!(!listed.iter().any(|p| p.starts_with("target/")), "{listed:?}");
        assert!(!listed.contains(&"src/nested/y.skip".to_string()), "{listed:?}");
        assert!(!listed.iter().any(|p| p.contains(".git/")), "{listed:?}");
        assert!(!listed.iter().any(|p| p.starts_with(".vizier/")), "{listed:?}");
        // Hidden files are shown (only `.git` is skipped).
        assert!(listed.contains(&"src/nested/.keep".to_string()), "{listed:?}");
    }

    #[test]
    fn include_ignored_reaches_ignored_files_but_never_git_or_vizier() {
        let dir = tree();
        let listed = names(dir.path(), true);
        assert!(listed.contains(&"target/out.txt".to_string()), "{listed:?}");
        assert!(!listed.iter().any(|p| p.contains(".git/")), "{listed:?}");
        assert!(!listed.iter().any(|p| p.starts_with(".vizier/")), "{listed:?}");
    }

    #[test]
    fn binary_is_a_nul_in_the_first_eight_kib() {
        assert!(is_binary(b"a\0b"));
        assert!(!is_binary(b"plain text\n"));
    }
}
