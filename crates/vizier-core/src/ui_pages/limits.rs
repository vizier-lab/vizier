//! The limits of pages and their checks (research R15).

use std::time::Duration;

/// The largest HTML document a version may hold, in bytes.
pub const MAX_VERSION_BYTES: usize = 1_048_576;
/// The most edits one edit set may carry.
pub const MAX_EDITS: usize = 100;
/// The longest page title, in characters, after trimming.
pub const MAX_TITLE_CHARS: usize = 120;
/// How long a tab watches a new version for problems.
pub const VERIFY_OBSERVE_MS: u64 = 2_000;
/// How long the tool waits for a tab's report before treating the version as unchecked.
pub const VERIFY_DEADLINE: Duration = Duration::from_secs(10);
/// The most problems one report keeps; the rest are counted.
pub const MAX_PROBLEMS: usize = 20;
/// The longest problem message or detail, in characters.
pub const MAX_PROBLEM_CHARS: usize = 500;
