//! A turn's tool scope (`specs/071-triggers` FR-030, research R4 and R9).
//!
//! A scope is a list of category names. A name is only dispatched when it is in the turn's
//! catalogue, so filtering the catalogue is enough to keep every call, from any script, inside
//! the scope. One extra entry, [`MEMORY_READ`], admits the memory tools that only read, so a run
//! fed by outside content can search memory without being able to write it (or `core/soul`).
//!
//! An entry starting with `!` excludes a category. A scope of exclusions only allows every other
//! category, including the agent's addon, MCP and custom ones: that is how "all tools except
//! messaging" is written without listing categories that do not exist yet.

use super::ToolCategory;

/// The scope entry for reading memory without writing it.
pub const MEMORY_READ: &str = "memory_read";

/// The memory tools that change nothing.
pub const READ_ONLY_MEMORY_TOOLS: &[&str] =
    &["memory_search", "memory_read", "memory_list", "memory_follow", "memory_graph", "read_dream_journal"];

/// The memory tools that write; every memory tool is in one list or the other (pinned by a test).
pub const WRITING_MEMORY_TOOLS: &[&str] = &["memory_write", "memory_delete", "memory_delete_bundle"];

/// The scope a webhook, and any trigger an agent creates, starts with.
pub fn narrow_default() -> Vec<String> {
    vec![MEMORY_READ.to_string()]
}

/// A positive entry nothing is named, so a scope of it admits nothing.
const NOTHING: &str = "__nothing__";

/// `entries` cut down to what `outer`, a turn's own scope, also allows. Memory is narrowed to
/// reading where `outer` allows no more than that.
pub fn within(entries: Vec<String>, outer: Option<&[String]>) -> Vec<String> {
    let Some(outer) = outer else {
        return entries;
    };
    let memory = ToolCategory::Memory.name();
    let mut kept: Vec<String> = entries
        .into_iter()
        .filter_map(|entry| {
            if entry == MEMORY_READ {
                admits_category(outer, memory).then_some(entry)
            } else if entry == memory {
                if admits(outer, memory, WRITING_MEMORY_TOOLS[0]) {
                    Some(entry)
                } else {
                    admits_category(outer, memory).then(|| MEMORY_READ.to_string())
                }
            } else {
                admits_category(outer, &entry).then_some(entry)
            }
        })
        .collect();
    kept.dedup();
    kept
}

/// The scope allowing only what both `a` and `b` allow; `None` is every tool.
pub fn intersect(a: Option<&[String]>, b: Option<&[String]>) -> Option<Vec<String>> {
    let (a, b) = match (a, b) {
        (None, other) | (other, None) => return other.map(<[String]>::to_vec),
        (Some(a), Some(b)) => (a, b),
    };
    let exclusions_only = |s: &[String]| !s.is_empty() && !has_positive(s);
    if exclusions_only(a) && exclusions_only(b) {
        let mut both = a.to_vec();
        both.extend(b.iter().filter(|entry| !a.contains(entry)).cloned());
        return Some(both);
    }
    // At least one side lists what it allows: keep of that list what the other side allows too,
    // along with the other side's per-tool exclusions.
    let (listed, other) = if exclusions_only(a) { (b, a) } else { (a, b) };
    let mut out = within(listed.iter().filter(|e| !e.starts_with('!')).cloned().collect(), Some(other));
    if out.is_empty() {
        out.push(NOTHING.into());
    }
    out.extend(a.iter().chain(b).filter(|e| e.starts_with("!fn:")).cloned());
    Some(out)
}

/// The categories that let a run reach a person on its own, which the run limit cannot bound
/// (FR-016): a monitor a person creates starts with every category except these.
pub const MESSAGING_CATEGORIES: &[ToolCategory] =
    &[ToolCategory::Discord, ToolCategory::Telegram, ToolCategory::Webui, ToolCategory::Sessions];

fn excluded(scope: &[String], category: &str) -> bool {
    scope.iter().any(|entry| entry.strip_prefix('!') == Some(category))
}

/// The engine's own form for keeping one tool out by name (a monitor's check keeps model-backed
/// tools out this way). People never write it: [`validate`] refuses it.
pub fn exclude_tool(name: &str) -> String {
    format!("!fn:{name}")
}

fn tool_excluded(scope: &[String], name: &str) -> bool {
    scope.iter().any(|entry| entry.strip_prefix("!fn:") == Some(name))
}

fn has_positive(scope: &[String]) -> bool {
    scope.iter().any(|entry| !entry.starts_with('!'))
}

/// Whether `scope` allows the tool `name` in `category`.
pub fn admits(scope: &[String], category: &str, name: &str) -> bool {
    // An empty list allows nothing; `None` is how "every tool" is written.
    if scope.is_empty() || excluded(scope, category) || tool_excluded(scope, name) {
        return false;
    }
    if !has_positive(scope) {
        return true;
    }
    scope.iter().any(|entry| entry == category)
        || (category == ToolCategory::Memory.name()
            && scope.iter().any(|entry| entry == MEMORY_READ)
            && READ_ONLY_MEMORY_TOOLS.contains(&name))
}

/// Whether `scope` allows anything in `category`, for the category lines of the prompt.
pub fn admits_category(scope: &[String], category: &str) -> bool {
    if scope.is_empty() || excluded(scope, category) {
        return false;
    }
    !has_positive(scope)
        || scope.iter().any(|entry| entry == category)
        || (category == ToolCategory::Memory.name() && scope.iter().any(|entry| entry == MEMORY_READ))
}

/// Every tool except those that reach a person on their own: a monitor's default (FR-030).
pub fn all_but_messaging() -> Vec<String> {
    MESSAGING_CATEGORIES.iter().map(|c| format!("!{}", c.name())).collect()
}

/// Whether `scope` lets a run send messages or wake conversations, which the interface warns
/// about (FR-016). `None` is every tool.
pub fn allows_messaging(scope: Option<&[String]>) -> bool {
    match scope {
        None => true,
        Some(scope) => MESSAGING_CATEGORIES.iter().any(|c| admits_category(scope, c.name())),
    }
}

/// Every entry names a known category, `memory_read`, or excludes one; or names a category the
/// agent may have (addon, MCP, custom), which cannot be checked here. Only the shape is checked.
pub fn validate(scope: &[String]) -> Result<(), String> {
    for entry in scope {
        let name = entry.strip_prefix('!').unwrap_or(entry);
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(format!("'{entry}' is not a tool category"));
        }
    }
    if has_positive(scope) && scope.iter().any(|e| e.starts_with('!')) {
        return Err("a tool scope lists categories to allow or to exclude, not both".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope(entries: &[&str]) -> Vec<String> {
        entries.iter().map(|e| e.to_string()).collect()
    }

    #[test]
    fn memory_read_admits_reading_and_never_writing() {
        let s = scope(&[MEMORY_READ]);
        for name in READ_ONLY_MEMORY_TOOLS {
            assert!(admits(&s, "memory", name), "{name}");
        }
        for name in WRITING_MEMORY_TOOLS {
            assert!(!admits(&s, "memory", name), "{name}");
        }
        assert!(admits_category(&s, "memory"));
        assert!(!admits_category(&s, "web"));
    }

    #[test]
    fn a_category_entry_admits_its_whole_category() {
        let s = scope(&["memory", "web"]);
        assert!(admits(&s, "memory", "memory_write"));
        assert!(admits(&s, "web", "fetch"));
        assert!(!admits(&s, "shell", "shell_exec"));
    }

    #[test]
    fn exclusions_allow_everything_else() {
        let s = all_but_messaging();
        assert!(admits(&s, "memory", "memory_write"));
        assert!(admits(&s, "brave_search", "brave_search_web"));
        assert!(!admits(&s, "telegram", "telegram_send_message"));
        assert!(!admits_category(&s, "sessions"));
        assert!(!allows_messaging(Some(&s)));
        assert!(validate(&s).is_ok());
        assert!(validate(&scope(&["memory", "!web"])).is_err());
        assert!(validate(&scope(&["bad name"])).is_err());
    }

    #[test]
    fn two_scopes_allow_only_what_both_do() {
        let narrow = scope(&[MEMORY_READ]);
        let wide = scope(&["memory", "web"]);
        assert_eq!(intersect(None, Some(&narrow)), Some(narrow.clone()));
        assert_eq!(intersect(Some(&wide), None), Some(wide.clone()));
        assert_eq!(intersect(Some(&wide), Some(&narrow)), Some(narrow.clone()));
        assert_eq!(intersect(Some(&narrow), Some(&wide)), Some(narrow.clone()));
        let open = all_but_messaging();
        assert_eq!(intersect(Some(&open), Some(&narrow)), Some(narrow.clone()));
        let both = intersect(Some(&open), Some(&scope(&["!shell"]))).unwrap();
        assert!(!admits(&both, "shell", "shell_exec") && !admits(&both, "telegram", "x"));
        assert!(admits(&both, "web", "fetch"));
        // Nothing in common stays nothing, never everything.
        let none = intersect(Some(&scope(&["web"])), Some(&narrow)).unwrap();
        assert!(!admits(&none, "web", "fetch") && !admits(&none, "memory", "memory_read"));
        assert!(!admits(&[], "web", "fetch") && !admits_category(&[], "memory"));
    }

    #[test]
    fn messaging_is_detected() {
        assert!(allows_messaging(None));
        assert!(!allows_messaging(Some(&narrow_default())));
        assert!(allows_messaging(Some(&scope(&["telegram"]))));
        assert!(allows_messaging(Some(&scope(&["sessions"]))));
    }
}
