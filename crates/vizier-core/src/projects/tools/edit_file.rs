//! `edit_file`: replace exact text, atomically, refusing a stale read, a miss or an ambiguity.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{ToolCategory, ToolContext, VizierTool},
    error::VizierError,
    projects::path,
};

use super::{atomic_write, check_read, cwd, record_read};

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct EditFileInput {
    /// A path relative to the project folder.
    pub path: String,
    /// The exact text to find. Empty text is refused.
    pub old_text: String,
    /// What to replace it with.
    pub new_text: String,
    /// Replace every occurrence instead of requiring exactly one.
    #[serde(default)]
    pub replace_all: Option<bool>,
}

pub struct EditFile;

/// How many line numbers an ambiguity message lists.
const MAX_LISTED_LINES: usize = 5;

fn first_lines(text: &str, needle: &str) -> Vec<usize> {
    let mut lines = vec![];
    let mut from = 0;
    while let Some(at) = text[from..].find(needle) {
        let absolute = from + at;
        lines.push(text[..absolute].bytes().filter(|b| *b == b'\n').count() + 1);
        from = absolute + needle.len().max(1);
        if lines.len() >= MAX_LISTED_LINES {
            break;
        }
    }
    lines
}

#[async_trait::async_trait]
impl VizierTool for EditFile {
    type Input = EditFileInput;
    type Output = serde_json::Value;

    fn name() -> String {
        "edit_file".into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Coding)
    }

    fn description(&self) -> String {
        "Edit a file in the project folder by replacing exact text. Refused when the chat has not \
         read the file (or it changed on disk since), when the text is absent, or when it occurs \
         more than once without `replace_all`. A refused edit leaves the file byte-identical. \
         CRLF line endings are preserved."
            .into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        if args.old_text.is_empty() || args.old_text == args.new_text {
            return Err(VizierError(
                "invalid_edit: old_text must be non-empty and different from new_text".into(),
            ));
        }
        let cwd = cwd(ctx)?;
        let file = path::resolve(&cwd, &args.path, path::Purpose::Existing)?;
        if !file.is_file() {
            return Err(VizierError(format!("not_a_file: {}", args.path)));
        }
        let bytes = tokio::fs::read(&file)
            .await
            .map_err(|err| VizierError(format!("not_found: {} ({err})", args.path)))?;
        if super::walk::is_binary(&bytes) {
            return Err(VizierError(format!("not_a_file: {} is binary", args.path)));
        }
        check_read(ctx, &file, &bytes)?;

        let original = String::from_utf8(bytes)
            .map_err(|_| VizierError(format!("not_a_file: {} is not UTF-8", args.path)))?;
        let crlf = original.contains("\r\n");
        let (old_text, new_text) = if crlf {
            (
                args.old_text.replace("\r\n", "\n").replace('\n', "\r\n"),
                args.new_text.replace("\r\n", "\n").replace('\n', "\r\n"),
            )
        } else {
            (args.old_text.clone(), args.new_text.clone())
        };

        let count = original.matches(&old_text).count();
        if count == 0 {
            return Err(VizierError(format!("edit_not_found: {}", args.path)));
        }
        let replace_all = args.replace_all.unwrap_or(false);
        if count > 1 && !replace_all {
            let lines = first_lines(&original, &old_text)
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            return Err(VizierError(format!(
                "edit_ambiguous: {} occurs {count} times (lines {lines}); pass replace_all or use more context",
                args.path
            )));
        }
        let first_line = original[..original.find(&old_text).unwrap_or(0)]
            .bytes()
            .filter(|b| *b == b'\n')
            .count()
            + 1;
        let updated = if replace_all {
            original.replace(&old_text, &new_text)
        } else {
            original.replacen(&old_text, &new_text, 1)
        };
        if updated == original {
            return Err(VizierError(format!(
                "invalid_edit: replacing this text would not change {}",
                args.path
            )));
        }

        let bytes = updated.into_bytes();
        let target = file.clone();
        let to_write = bytes.clone();
        tokio::task::spawn_blocking(move || atomic_write(&target, &to_write))
            .await
            .map_err(|err| VizierError(err.to_string()))??;
        record_read(ctx, &file, &bytes);
        Ok(serde_json::json!({
            "path": path::display_rel(&cwd, &file),
            "replacements": if replace_all { count } else { 1 },
            "first_line": first_line,
        }))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{
        projects::context::ProjectTurnContext,
        schema::{ProjectMode, VizierChannelId, VizierSession},
    };

    fn ctx(folder: &std::path::Path) -> ToolContext {
        let folder = std::fs::canonicalize(folder).unwrap();
        let project = ProjectTurnContext {
            project_id: "p".into(),
            name: "alpha".into(),
            folder: folder.clone(),
            cwd: folder,
            worktree: None,
            mode: ProjectMode::Build,
            git: None,
            available: true,
            shell_enabled: true,
            instructions: None,
            instructions_truncated: false,
            is_piece: false,
            ledger: Default::default(),
        };
        ToolContext {
            session: VizierSession(
                "dp-a".into(),
                VizierChannelId::Project("alice".into(), "p".into()),
                Some("t1".into()),
            ),
            pending_attachments: Default::default(),
            hooks: None,
            background_depth: 0,
            loaded_images: Default::default(),
            recalled: Default::default(),
            deadline: None,
            client: None,
            secret_redactor: Arc::new(crate::secrets::SecretRedactor::default()),
            dream_sources: vec![],
            project: Some(Arc::new(project)),
        }
    }

    fn input(old: &str, new: &str) -> EditFileInput {
        EditFileInput {
            path: "a.txt".into(),
            old_text: old.into(),
            new_text: new.into(),
            replace_all: None,
        }
    }

    async fn read(ctx: &ToolContext, dir: &std::path::Path) {
        let file = std::fs::canonicalize(dir.join("a.txt")).unwrap();
        let bytes = std::fs::read(&file).unwrap();
        record_read(ctx, &file, &bytes);
    }

    #[tokio::test]
    async fn an_exact_unique_match_applies() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "one two\n").unwrap();
        let ctx = ctx(dir.path());
        read(&ctx, dir.path()).await;
        let out = EditFile.call(input("two", "three"), &ctx).await.unwrap();
        assert_eq!(out["replacements"], 1);
        assert_eq!(std::fs::read_to_string(dir.path().join("a.txt")).unwrap(), "one three\n");
    }

    #[tokio::test]
    async fn refusals_leave_the_file_byte_identical() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "x x\n").unwrap();
        let ctx = ctx(dir.path());
        let before = std::fs::read(dir.path().join("a.txt")).unwrap();

        // Not read in this chat.
        let err = EditFile.call(input("x", "y"), &ctx).await.unwrap_err();
        assert!(err.0.starts_with("stale_read:"), "{err:?}");
        assert_eq!(std::fs::read(dir.path().join("a.txt")).unwrap(), before);

        read(&ctx, dir.path()).await;
        // Ambiguous.
        let err = EditFile.call(input("x", "y"), &ctx).await.unwrap_err();
        assert!(err.0.starts_with("edit_ambiguous:"), "{err:?}");
        assert_eq!(std::fs::read(dir.path().join("a.txt")).unwrap(), before);
        // Missing.
        let err = EditFile.call(input("nope", "y"), &ctx).await.unwrap_err();
        assert!(err.0.starts_with("edit_not_found:"), "{err:?}");
        assert_eq!(std::fs::read(dir.path().join("a.txt")).unwrap(), before);

        // Changed on disk after the read.
        std::fs::write(dir.path().join("a.txt"), "changed\n").unwrap();
        let err = EditFile.call(input("changed", "y"), &ctx).await.unwrap_err();
        assert!(err.0.starts_with("stale_read:"), "{err:?}");
        assert_eq!(std::fs::read_to_string(dir.path().join("a.txt")).unwrap(), "changed\n");
    }

    #[tokio::test]
    async fn replace_all_and_crlf_are_preserved() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "one\r\ntwo\r\n").unwrap();
        let ctx = ctx(dir.path());
        read(&ctx, dir.path()).await;
        let mut args = input("one\ntwo", "x\ny");
        args.replace_all = Some(true);
        EditFile.call(args, &ctx).await.unwrap();
        assert_eq!(std::fs::read(dir.path().join("a.txt")).unwrap(), b"x\r\ny\r\n");
    }
}
