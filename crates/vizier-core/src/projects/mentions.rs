//! `@` mentions (`specs/041-project-coding-workspace` R15): resolved at turn start into a
//! `## Mentioned files` section of the per-message context, never stored (FR-018/FR-019).

use crate::{
    agents::tools::ToolContext,
    projects::{path, tools::walk},
};

/// The most a mentioned file is included whole.
pub const FILE_CAP_BYTES: usize = 64 * 1024;
/// The most the whole section may hold.
pub const TOTAL_CAP_BYTES: usize = 256 * 1024;
/// The most entries a mentioned folder lists.
pub const FOLDER_ENTRIES: usize = 200;

fn numbered(text: &str) -> String {
    let mut out = String::new();
    for (i, line) in text.split('\n').enumerate() {
        if i == 0 && line.is_empty() && text.is_empty() {
            break;
        }
        out.push_str(&format!("{:>6}\t{}\n", i + 1, line.trim_end_matches('\r')));
    }
    out
}

/// The section for this turn's mentions, or `None` when nothing was mentioned.
pub fn section(ctx: &ToolContext, mentions: &[String]) -> Option<String> {
    let project = ctx.project.as_ref()?;
    if mentions.is_empty() {
        return None;
    }
    let cwd = &project.cwd;
    let mut out = String::from("## Mentioned files\nFiles and folders the person pointed at with \
                                `@`. Their contents are current as this message starts.\n");
    let mut used = out.len();
    for mention in mentions.iter().take(10) {
        let mention = mention.trim();
        if mention.is_empty() {
            continue;
        }
        let resolved = match path::resolve(cwd, mention, path::Purpose::Existing) {
            Ok(resolved) => resolved,
            Err(_) => {
                out.push_str(&format!("\n### {mention}\n(missing)\n"));
                continue;
            }
        };
        let shown = path::display_rel(cwd, &resolved);
        if resolved.is_dir() {
            let mut entries = walk::walk(&resolved, false);
            entries.truncate(FOLDER_ENTRIES);
            out.push_str(&format!("\n### {shown}/\n"));
            for entry in entries {
                let rel = path::display_rel(&resolved, &entry.path);
                if entry.is_dir {
                    out.push_str(&format!("{rel}/\n"));
                } else {
                    out.push_str(&format!("{rel}\n"));
                }
            }
            continue;
        }
        let Ok(bytes) = std::fs::read(&resolved) else {
            out.push_str(&format!("\n### {shown}\n(missing)\n"));
            continue;
        };
        if bytes.len() > FILE_CAP_BYTES || walk::is_binary(&bytes) {
            out.push_str(&format!(
                "\n### {shown}\n(too large or binary to include; read it with read_file)\n"
            ));
            continue;
        }
        if used + bytes.len() > TOTAL_CAP_BYTES {
            out.push_str("\n(the remaining mentioned files were left out to fit the message limit)\n");
            break;
        }
        let text = String::from_utf8_lossy(&bytes);
        let block = format!("\n### {shown}\n```\n{}\n```\n", numbered(&text).trim_end());
        if used + block.len() > TOTAL_CAP_BYTES {
            out.push_str("\n(the remaining mentioned files were left out to fit the message limit)\n");
            break;
        }
        used += block.len();
        out.push_str(&block);
        // A mention counts as a read for the stale-read guard (quickstart §6).
        project.ledger.record(&ctx.session.to_slug(), &resolved, &bytes);
    }
    Some(out)
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
            mode: ProjectMode::Plan,
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

    #[test]
    fn a_file_is_numbered_and_a_missing_one_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "one\ntwo\n").unwrap();
        let ctx = ctx(dir.path());
        let out = section(&ctx, &["a.txt".into(), "gone.txt".into()]).unwrap();
        assert!(out.contains("1\tone"), "{out}");
        assert!(out.contains("2\ttwo"), "{out}");
        assert!(out.contains("(missing)"), "{out}");
        // The mention counts as a read.
        let file = std::fs::canonicalize(dir.path().join("a.txt")).unwrap();
        assert!(ctx.project.as_ref().unwrap().ledger.check(&ctx.session.to_slug(), &file, b"one\ntwo\n").is_ok());
    }

    #[test]
    fn a_folder_is_a_shallow_listing_and_a_large_file_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/lib.rs"), "x").unwrap();
        std::fs::write(dir.path().join("big.bin"), vec![b'a'; FILE_CAP_BYTES + 1]).unwrap();
        let ctx = ctx(dir.path());
        let out = section(&ctx, &["src".into(), "big.bin".into()]).unwrap();
        assert!(out.contains("src/\n"), "{out}");
        assert!(out.contains("lib.rs"), "{out}");
        assert!(out.contains("too large"), "{out}");
    }

    #[test]
    fn no_mentions_or_no_project_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = ctx(dir.path());
        assert!(section(&ctx, &[]).is_none());
        ctx.project = None;
        assert!(section(&ctx, &["a".into()]).is_none());
    }
}
