//! The project channel namespace (`specs/040-projects-mode/contracts/session-and-permissions.md`).
//!
//! A project session is an ordinary HTTP session whose channel id is `project__<project_id>`.
//! This module is the only implementation of that shape: the prefix, building, parsing and the
//! slug variant the conversation listing filters on.

use std::path::{Path, PathBuf};

/// The channel id prefix every project channel carries.
pub const PREFIX: &str = "project__";

/// The channel id of a project: `project__<id>`.
pub fn channel_id(project_id: &str) -> String {
    format!("{PREFIX}{project_id}")
}

/// The project id a channel id addresses, when it is one.
pub fn project_id(channel_id: &str) -> Option<&str> {
    let id = channel_id.strip_prefix(PREFIX)?;
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') {
        return None;
    }
    Some(id)
}

/// Whether a channel id is a project channel.
pub fn is_project_channel(channel_id: &str) -> bool {
    project_id(channel_id).is_some()
}

/// A path as people should read it: the canonical form with Windows' `\\?\` verbatim prefix
/// removed for display only. Comparison always uses the canonical path.
pub fn display_path(path: &Path) -> String {
    let text = path.to_string_lossy().into_owned();
    #[cfg(windows)]
    {
        if let Some(rest) = text.strip_prefix(r"\\?\") {
            // `\\?\UNC\server\share` is `\\server\share` without the prefix.
            if let Some(unc) = rest.strip_prefix("UNC\\") {
                return format!(r"\\{unc}");
            }
            return rest.to_string();
        }
    }
    text
}

/// The canonical absolute form of an existing folder, or `None` when it is not a directory.
pub fn canonical_folder(folder: &Path) -> Option<PathBuf> {
    let canonical = std::fs::canonicalize(folder).ok()?;
    canonical.is_dir().then_some(canonical)
}

/// Whether `folder` is `workspace` or inside it. Both must be canonical.
pub fn inside(folder: &Path, workspace: &Path) -> bool {
    folder == workspace || folder.starts_with(workspace)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_channel_id_round_trips() {
        let id = channel_id("p3xY9aQ2mZ");
        assert_eq!(id, "project__p3xY9aQ2mZ");
        assert_eq!(project_id(&id), Some("p3xY9aQ2mZ"));
        assert!(is_project_channel(&id));
    }

    #[test]
    fn ids_have_no_colon_or_path_separator() {
        let id = channel_id("abcDEF1234");
        for bad in [':', '/', '\\'] {
            assert!(!id.contains(bad), "{id:?} contains {bad:?}");
        }
    }

    #[test]
    fn an_empty_or_odd_id_is_not_a_project_channel() {
        assert_eq!(project_id("project__"), None);
        assert!(!is_project_channel("project__"));
        assert!(!is_project_channel("project:abc"));
        assert!(!is_project_channel("vizier-webui"));
        assert!(!is_project_channel(""));
    }

    #[test]
    fn containment_uses_canonical_paths() {
        let base = std::path::Path::new("/tmp/work");
        assert!(inside(base, base));
        assert!(inside(&base.join("nested"), base));
        assert!(!inside(std::path::Path::new("/tmp/other"), base));
    }

    #[test]
    fn windows_verbatim_prefixes_are_stripped_for_display() {
        let path = std::path::Path::new(r"\\?\C:\Users\alice\project");
        #[cfg(windows)]
        assert_eq!(display_path(path), r"C:\Users\alice\project");
        #[cfg(not(windows))]
        assert_eq!(display_path(path), path.to_string_lossy());
    }
}
