//! Path confinement for every project surface (`specs/041-project-coding-workspace` R6).
//!
//! One function resolves a client-supplied relative path against the chat's `cwd` (the worktree
//! for a worktree chat, else the project root), and refuses anything that leaves it — through
//! `..`, an absolute path, a symbolic link, or the project's own `.vizier/` worktree area. File
//! tools, the workspace shell's `workdir`, mentions and the git routes all use it.

use std::path::{Component, Path, PathBuf};

use crate::{error::VizierError, projects::channel};

/// Whether a path must already exist, or may be created (its parents inside `cwd`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    Existing,
    Create,
}

fn outside(rel: &str) -> VizierError {
    VizierError(format!(
        "path_outside_project: {rel:?} is outside the project folder"
    ))
}

/// Resolve `rel` against `cwd`, returning a canonical absolute path that stays inside it.
pub fn resolve(cwd: &Path, rel: &str, purpose: Purpose) -> Result<PathBuf, VizierError> {
    let cwd = std::fs::canonicalize(cwd).map_err(|err| {
        VizierError(format!(
            "project_unavailable: cannot resolve {}: {err}",
            channel::display_path(cwd)
        ))
    })?;
    let rel = rel.trim();
    if rel.is_empty() {
        return match purpose {
            Purpose::Existing if !cwd.is_dir() => Err(outside(rel)),
            _ => Ok(cwd),
        };
    }
    if Path::new(rel).is_absolute() {
        return Err(outside(rel));
    }

    let candidate = cwd.join(rel);
    // Canonicalise the deepest existing ancestor. This catches a symlink anywhere on the way
    // out even when the final component does not exist yet.
    let mut ancestor = candidate.as_path();
    let mut missing: Vec<std::ffi::OsString> = Vec::new();
    loop {
        if ancestor.exists() {
            break;
        }
        let Some(name) = ancestor.file_name() else {
            return Err(outside(rel));
        };
        missing.push(name.to_os_string());
        let Some(parent) = ancestor.parent() else {
            return Err(outside(rel));
        };
        ancestor = parent;
    }
    let canonical_ancestor = std::fs::canonicalize(ancestor)
        .map_err(|_| outside(rel))?;
    if !canonical_ancestor.starts_with(&cwd) {
        return Err(outside(rel));
    }

    // The tail that did not exist yet cannot contain `..`; a symlink there cannot exist either.
    let mut result = canonical_ancestor;
    for name in missing.iter().rev() {
        let component = Path::new(name);
        if matches!(
            component.components().next(),
            Some(Component::ParentDir | Component::RootDir | Component::Prefix(_))
        ) {
            return Err(outside(rel));
        }
        result.push(name);
    }

    // `..` in the existing part is resolved by canonicalisation; a `..` in the tail is refused.
    if !result.starts_with(&cwd) {
        return Err(outside(rel));
    }
    // From the project root, the worktree area is not an ordinary project path (FR-026/FR-028).
    if result.starts_with(cwd.join(".vizier")) {
        return Err(VizierError(format!(
            "path_in_worktree_area: {rel:?} is inside the project's worktree area"
        )));
    }

    if purpose == Purpose::Existing && !result.exists() {
        return Err(VizierError(format!("not_found: {rel}")));
    }
    Ok(result)
}

/// A path as a tool result shows it: relative to `cwd`, `/` on every platform.
pub fn display_rel(cwd: &Path, path: &Path) -> String {
    let Ok(cwd) = std::fs::canonicalize(cwd) else {
        return channel::display_path(path).replace('\\', "/");
    };
    let Ok(path) = std::fs::canonicalize(path) else {
        return channel::display_path(path)
            .trim_end_matches(['/', '\\'])
            .to_string()
            .replace('\\', "/");
    };
    match path.strip_prefix(&cwd) {
        Ok(rel) => rel.to_string_lossy().replace('\\', "/"),
        Err(_) => channel::display_path(&path).replace('\\', "/"),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn tree() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src/nested")).unwrap();
        fs::write(dir.path().join("src/lib.rs"), "fn main() {}").unwrap();
        dir
    }

    #[test]
    fn an_existing_file_and_folder_resolve_inside() {
        let dir = tree();
        let file = resolve(dir.path(), "src/lib.rs", Purpose::Existing).unwrap();
        assert_eq!(file, fs::canonicalize(dir.path().join("src/lib.rs")).unwrap());
        assert_eq!(display_rel(dir.path(), &file), "src/lib.rs");
        let folder = resolve(dir.path(), "src", Purpose::Existing).unwrap();
        assert!(folder.is_dir());
    }

    #[test]
    fn a_new_nested_file_resolves_to_a_create_path() {
        let dir = tree();
        let path = resolve(dir.path(), "src/a/b/new.rs", Purpose::Create).unwrap();
        assert_eq!(path, fs::canonicalize(dir.path()).unwrap().join("src/a/b/new.rs"));
    }

    #[test]
    fn absolute_escaping_and_missing_paths_are_refused() {
        let dir = tree();
        assert!(resolve(dir.path(), "/etc/passwd", Purpose::Existing).is_err());
        let err = resolve(dir.path(), "../etc/passwd", Purpose::Create).unwrap_err();
        assert!(err.0.contains("path_outside_project"), "{err:?}");
        assert!(resolve(dir.path(), "src/a/../../outside", Purpose::Create).is_err());
        let err = resolve(dir.path(), "missing.rs", Purpose::Existing).unwrap_err();
        assert!(err.0.starts_with("not_found:"), "{err:?}");
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_leading_out_is_refused() {
        let dir = tree();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret"), "x").unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("link")).unwrap();
        let err = resolve(dir.path(), "link/secret", Purpose::Existing).unwrap_err();
        assert!(err.0.contains("path_outside_project"), "{err:?}");
        // A new file through the link is refused too.
        let err = resolve(dir.path(), "link/new", Purpose::Create).unwrap_err();
        assert!(err.0.contains("path_outside_project"), "{err:?}");
    }

    #[test]
    fn the_worktree_area_is_refused_from_the_root() {
        let dir = tree();
        fs::create_dir_all(dir.path().join(".vizier/dp-a/wt")).unwrap();
        fs::write(dir.path().join(".vizier/dp-a/wt/x"), "x").unwrap();
        let err = resolve(dir.path(), ".vizier/dp-a/wt/x", Purpose::Existing).unwrap_err();
        assert!(err.0.contains("path_in_worktree_area"), "{err:?}");
    }

    #[test]
    fn windows_separators_work_on_windows() {
        let dir = tree();
        let rel = if cfg!(windows) { "src\\lib.rs" } else { "src/lib.rs" };
        assert!(resolve(dir.path(), rel, Purpose::Existing).is_ok());
    }
}
