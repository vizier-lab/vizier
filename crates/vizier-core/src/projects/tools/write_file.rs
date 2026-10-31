//! `write_file`: write a whole file atomically, refusing to overwrite what this chat has not read.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{ToolCategory, ToolContext, VizierTool},
    error::VizierError,
    projects::path,
};

use super::{atomic_write, check_read, cwd, record_read};

/// The largest file `write_file` accepts.
pub const MAX_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct WriteFileInput {
    /// A path relative to the project folder. Parent folders inside it are created.
    pub path: String,
    /// The complete file content.
    pub content: String,
}

pub struct WriteFile;

#[async_trait::async_trait]
impl VizierTool for WriteFile {
    type Input = WriteFileInput;
    type Output = serde_json::Value;

    fn name() -> String {
        "write_file".into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Coding)
    }

    fn description(&self) -> String {
        "Write a whole file in the project folder, creating parent folders inside it. Refuses a \
         file over 4 MiB, and overwriting an existing file this chat has not read (or that \
         changed on disk since it did). The write is atomic, so a failure leaves the old file."
            .into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let cwd = cwd(ctx)?;
        let file = path::resolve(&cwd, &args.path, path::Purpose::Create)?;
        if args.content.len() > MAX_BYTES {
            return Err(VizierError(format!(
                "too_large: {} is {} bytes; the limit is {MAX_BYTES}",
                args.path,
                args.content.len()
            )));
        }
        let existed = file.exists();
        if existed {
            if !file.is_file() {
                return Err(VizierError(format!("not_a_file: {}", args.path)));
            }
            let current = tokio::fs::read(&file)
                .await
                .map_err(|err| VizierError(format!("not_found: {} ({err})", args.path)))?;
            check_read(ctx, &file, &current)?;
        }
        if let Some(parent) = file.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|err| VizierError(format!("write_failed: {err}")))?;
            // A parent symlink created between `resolve` and now must not escape the folder.
            let resolved = path::resolve(&cwd, &args.path, path::Purpose::Create)?;
            if resolved != file {
                return Err(VizierError(format!(
                    "path_outside_project: {} changed while writing",
                    args.path
                )));
            }
        }
        let bytes = args.content.into_bytes();
        let target = file.clone();
        let to_write = bytes.clone();
        tokio::task::spawn_blocking(move || atomic_write(&target, &to_write))
            .await
            .map_err(|err| VizierError(err.to_string()))??;
        record_read(ctx, &file, &bytes);
        Ok(serde_json::json!({
            "path": path::display_rel(&cwd, &file),
            "created": !existed,
            "bytes": bytes.len(),
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

    #[tokio::test]
    async fn a_new_file_and_its_folders_are_created() {
        let dir = tempfile::tempdir().unwrap();
        let out = WriteFile
            .call(
                WriteFileInput { path: "src/a/new.rs".into(), content: "hi\n".into() },
                &ctx(dir.path()),
            )
            .await
            .unwrap();
        assert_eq!(out["created"], true);
        assert_eq!(std::fs::read_to_string(dir.path().join("src/a/new.rs")).unwrap(), "hi\n");
    }

    #[tokio::test]
    async fn overwriting_requires_a_read() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "old").unwrap();
        let err = WriteFile
            .call(
                WriteFileInput { path: "a.txt".into(), content: "new".into() },
                &ctx(dir.path()),
            )
            .await
            .unwrap_err();
        assert!(err.0.starts_with("stale_read:"), "{err:?}");
        assert_eq!(std::fs::read_to_string(dir.path().join("a.txt")).unwrap(), "old");
    }
}
