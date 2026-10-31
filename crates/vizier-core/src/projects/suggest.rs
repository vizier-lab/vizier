//! `@` file suggestions (`specs/041-project-coding-workspace` R15): a 10-second cached walk per
//! folder and a hand-rolled subsequence scorer, small enough to keep at 10,000 files without a
//! fuzzy-matching dependency.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use crate::projects::path;

/// How long one folder's path list is reused.
pub const CACHE_TTL: Duration = Duration::from_secs(10);
/// The most paths one folder's cache holds.
pub const CACHE_LIMIT: usize = 50_000;

struct Cache {
    at: Instant,
    paths: Arc<Vec<(String, bool)>>,
}

fn cache() -> &'static Mutex<std::collections::HashMap<PathBuf, Cache>> {
    static CACHE: std::sync::OnceLock<Mutex<std::collections::HashMap<PathBuf, Cache>>> =
        std::sync::OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// A file or folder a person can pick: the path relative to the chat's folder, `/`-separated,
/// the folder with a trailing `/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub path: String,
    pub is_dir: bool,
}

fn paths_of(cwd: &Path) -> Arc<Vec<(String, bool)>> {
    let canonical = std::fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf());
    {
        let guard = cache().lock();
        if let Some(entry) = guard.get(&canonical)
            && entry.at.elapsed() < CACHE_TTL
        {
            return entry.paths.clone();
        }
    }
    let mut listed: Vec<(String, bool)> = super::tools::walk::walk(cwd, false)
        .into_iter()
        .take(CACHE_LIMIT)
        .map(|entry| (path::display_rel(cwd, &entry.path), entry.is_dir))
        .collect();
    listed.sort();
    let paths = Arc::new(listed);
    cache().lock().insert(
        canonical,
        Cache {
            at: Instant::now(),
            paths: paths.clone(),
        },
    );
    paths
}

/// How well `candidate` matches `query`: `None` when it is not a subsequence. Higher is better.
pub fn score(candidate: &str, query: &str) -> Option<i64> {
    if query.is_empty() {
        return Some(0);
    }
    let haystack: Vec<char> = candidate.chars().flat_map(char::to_lowercase).collect();
    let needle: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();
    let mut score = 0i64;
    let mut at = 0usize;
    let mut previous: Option<usize> = None;
    for wanted in needle {
        let found = haystack[at..].iter().position(|c| *c == wanted)? + at;
        if previous == Some(found.wrapping_sub(1)) {
            score += 8;
        }
        let before = if found == 0 { '/' } else { haystack[found - 1] };
        if matches!(before, '/' | '_' | '-' | '.' | ' ') {
            score += 6;
        }
        previous = Some(found);
        at = found + 1;
    }
    // A match in the final component is worth more than one buried in a folder name, and a
    // component that starts with the query more still (`src/agents/agent.rs` for "agent").
    let lower = query.to_lowercase();
    let basename = candidate.rsplit('/').next().unwrap_or(candidate);
    if basename.to_lowercase().contains(&lower) {
        score += 12;
    }
    for component in candidate.split('/') {
        if component.to_lowercase().starts_with(&lower) {
            score += 10;
        }
    }
    // Shorter paths win ties.
    Some(score - candidate.len() as i64)
}

fn depth(path: &str) -> usize {
    path.matches('/').count()
}

/// Rank `paths` for `query`; an empty query returns the shallowest entries. Files and folders
/// alike are candidates, and folders keep their trailing `/`.
pub fn rank(paths: &[(String, bool)], query: &str, limit: usize) -> Vec<Suggestion> {
    let query = query.trim();
    let mut scored: Vec<(i64, usize, &(String, bool))> = paths
        .iter()
        .filter_map(|entry| {
            let path = entry.0.as_str();
            let score = score(path, query)?;
            Some((score, depth(path), entry))
        })
        .collect();
    if query.is_empty() {
        scored.sort_by(|a, b| (a.1, &a.2.0).cmp(&(b.1, &b.2.0)));
    } else {
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.0.cmp(&b.2.0)));
    }
    scored
        .into_iter()
        .take(limit)
        .map(|(_, _, entry)| Suggestion {
            path: if entry.1 {
                format!("{}/", entry.0.trim_end_matches('/'))
            } else {
                entry.0.clone()
            },
            is_dir: entry.1,
        })
        .collect()
}

/// Suggestions for one folder, cached for ten seconds.
pub fn suggest(cwd: &Path, query: &str, limit: usize) -> Vec<Suggestion> {
    let paths = paths_of(cwd);
    rank(&paths, query, limit.clamp(1, 50))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths() -> Vec<(String, bool)> {
        vec![
            ("src/agents/agent.rs".into(), false),
            ("src/agents/".into(), true),
            ("crates/api/agent/mod.rs".into(), false),
            ("docs/agents.md".into(), false),
        ]
    }

    #[test]
    fn a_name_match_ranks_above_a_buried_one() {
        let ranked = rank(&paths(), "agent", 10);
        let names: Vec<&str> = ranked.iter().map(|s| s.path.as_str()).collect();
        let agent_rs = names.iter().position(|p| *p == "src/agents/agent.rs").unwrap();
        let docs = names.iter().position(|p| *p == "docs/agents.md").unwrap();
        assert!(agent_rs < docs, "{names:?}");
    }

    #[test]
    fn a_word_start_and_a_basename_beat_a_scattered_match() {
        let ranked = rank(&paths(), "agentsr", 10);
        assert_eq!(ranked[0].path, "src/agents/agent.rs", "{ranked:?}");
        assert!(score("src/agents/agent.rs", "agentsr").unwrap() > score("a_g_e_n_t_s_r", "agentsr").unwrap());
    }

    #[test]
    fn folders_keep_their_slash_and_an_empty_query_is_shallowest_first() {
        let ranked = rank(&paths(), "", 10);
        assert_eq!(ranked[0].path, "docs/agents.md", "{ranked:?}");
        assert!(ranked.iter().any(|s| s.path == "src/agents/" && s.is_dir));
    }
}
