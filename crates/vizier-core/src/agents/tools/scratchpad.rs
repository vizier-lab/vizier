//! `write_tmp`, `append_tmp` and `read_tmp` (`specs/020-scratchpad-tools/contracts/agent-surface.md`):
//! scratch files private to one conversation. Every outcome but a storage failure is data
//! (research R8), so an unguarded script survives a missing file or a full scratchpad.

use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{ToolCategory, ToolContext, VizierTool},
    error::VizierError,
    storage::{
        VizierStorage,
        scratch::{MAX_SCRATCH_NAME_BYTES, ScratchStorage, ScratchWrite},
    },
};

/// The rules every scratchpad tool states.
const SCRATCH_RULES: &str = "A scratch file is plain text, private to this conversation, and shared by every turn of it while turns keep coming back to back. All scratch files are deleted once the conversation has nothing running or queued, and on server restart: use memory for anything that must last. Nothing written here reaches a later background-job report. Names are 1-128 characters of letters, digits, '-', '_' and '.'. Limits: 1 MiB per file, 64 files per conversation. A refusal or a missing file comes back as a result with a `status`, never an exception.";

const INVALID_NAME: &str =
    "Scratch file names are 1–128 characters of letters, digits, '-', '_' and '.'.";

/// 1–128 bytes of `[A-Za-z0-9._-]`, and not `.` or `..` (research R9).
fn valid_scratch_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_SCRATCH_NAME_BYTES
        && name != "."
        && name != ".."
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ScratchResult {
    Written {
        name: String,
        size: u64,
    },
    Found {
        name: String,
        size: u64,
        content: String,
    },
    NotFound {
        name: String,
        /// The scratch files this conversation does have.
        existing: Vec<String>,
    },
    Refused {
        /// `invalid_name`, `too_large` or `too_many`.
        reason: String,
        name: String,
        message: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        current: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        existing: Option<Vec<String>>,
    },
}

impl ScratchResult {
    fn invalid_name(name: String) -> Self {
        Self::Refused {
            reason: "invalid_name".into(),
            name,
            message: INVALID_NAME.into(),
            limit: None,
            current: None,
            existing: None,
        }
    }

    fn from_write(name: String, write: ScratchWrite, append: bool) -> Self {
        match write {
            ScratchWrite::Written { size } => Self::Written { name, size },
            ScratchWrite::TooLarge { limit, current } => Self::Refused {
                reason: "too_large".into(),
                message: format!(
                    "{} would make '{name}' larger than 1 MiB; it is unchanged.",
                    if append { "This append" } else { "This write" }
                ),
                name,
                limit: Some(limit),
                current: Some(current),
                existing: None,
            },
            ScratchWrite::TooMany { limit, names } => Self::Refused {
                reason: "too_many".into(),
                message: format!(
                    "This conversation already has {limit} scratch files; write to an existing one."
                ),
                name,
                limit: Some(limit as u64),
                current: None,
                existing: Some(names),
            },
        }
    }
}

fn storage_error(err: anyhow::Error) -> VizierError {
    VizierError(format!("scratchpad storage: {err}"))
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct WriteTmpArgs {
    /// The scratch file's name.
    pub name: String,
    /// The file's whole new content.
    pub content: String,
}

pub struct WriteTmp {
    pub storage: Arc<VizierStorage>,
}

#[async_trait::async_trait]
impl VizierTool for WriteTmp {
    type Input = WriteTmpArgs;
    type Output = ScratchResult;

    fn name() -> String {
        "write_tmp".to_string()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Scratchpad)
    }

    fn description(&self) -> String {
        format!(
            "Create a scratch file or replace all of its content. Returns {{status: \"written\", name, size}} or {{status: \"refused\", reason, message}}. {SCRATCH_RULES}"
        )
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        if !valid_scratch_name(&args.name) {
            return Ok(ScratchResult::invalid_name(args.name));
        }
        let write = self
            .storage
            .write_scratch(&ctx.session, &args.name, &args.content)
            .await
            .map_err(storage_error)?;
        Ok(ScratchResult::from_write(args.name, write, false))
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct AppendTmpArgs {
    /// The scratch file's name.
    pub name: String,
    /// Text to add to the end, exactly as given: no newline or separator is added.
    pub content: String,
}

pub struct AppendTmp {
    pub storage: Arc<VizierStorage>,
}

#[async_trait::async_trait]
impl VizierTool for AppendTmp {
    type Input = AppendTmpArgs;
    type Output = ScratchResult;

    fn name() -> String {
        "append_tmp".to_string()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Scratchpad)
    }

    fn description(&self) -> String {
        format!(
            "Add content to the end of a scratch file exactly as given (no separator is added, so include your own newlines), creating the file if it does not exist. Returns {{status: \"written\", name, size}} or {{status: \"refused\", reason, message}}. {SCRATCH_RULES}"
        )
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        if !valid_scratch_name(&args.name) {
            return Ok(ScratchResult::invalid_name(args.name));
        }
        let write = self
            .storage
            .append_scratch(&ctx.session, &args.name, &args.content)
            .await
            .map_err(storage_error)?;
        Ok(ScratchResult::from_write(args.name, write, true))
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ReadTmpArgs {
    /// The scratch file's name.
    pub name: String,
}

pub struct ReadTmp {
    pub storage: Arc<VizierStorage>,
}

#[async_trait::async_trait]
impl VizierTool for ReadTmp {
    type Input = ReadTmpArgs;
    type Output = ScratchResult;

    fn name() -> String {
        "read_tmp".to_string()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Scratchpad)
    }

    fn description(&self) -> String {
        format!(
            "Read a scratch file. Returns {{status: \"found\", name, size, content}}, or {{status: \"not_found\", name, existing}} listing the scratch files this conversation does have. {SCRATCH_RULES}"
        )
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        if !valid_scratch_name(&args.name) {
            return Ok(ScratchResult::invalid_name(args.name));
        }
        let content = self
            .storage
            .read_scratch(&ctx.session, &args.name)
            .await
            .map_err(storage_error)?;
        Ok(match content {
            Some(content) => ScratchResult::Found {
                name: args.name,
                size: content.len() as u64,
                content,
            },
            None => ScratchResult::NotFound {
                existing: self
                    .storage
                    .scratch_names(&ctx.session)
                    .await
                    .map_err(storage_error)?,
                name: args.name,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_rule() {
        for name in ["plan", "a.b-c_d", "A9", "..x", &"n".repeat(128)] {
            assert!(valid_scratch_name(name), "{name:?} should be accepted");
        }
        for name in ["", ".", "..", "a/b", "../x", "a b", &"n".repeat(129), "é", "a\u{7}"] {
            assert!(!valid_scratch_name(name), "{name:?} should be refused");
        }
    }

    #[test]
    fn results_serialise_to_the_contract() {
        let refused = ScratchResult::from_write(
            "log".into(),
            ScratchWrite::TooLarge { limit: 1_048_576, current: 3 },
            true,
        );
        let json = serde_json::to_value(&refused).unwrap();
        assert_eq!(json["status"], "refused");
        assert_eq!(json["reason"], "too_large");
        assert_eq!(json["current"], 3);
        assert!(json.get("existing").is_none());

        let missing = ScratchResult::NotFound { name: "x".into(), existing: vec!["a".into()] };
        assert_eq!(
            serde_json::to_value(&missing).unwrap(),
            serde_json::json!({ "status": "not_found", "name": "x", "existing": ["a"] })
        );
    }
}
