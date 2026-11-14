//! Names (`specs/061-agent-custom-tools` research R12): a tool is saved as `name` and called as
//! `custom_<name>`; categories share the clash rule MCP servers and client categories use.

/// The reserved prefix every custom tool is called by. No native, addon, MCP or client tool may
/// take a name that starts with it.
pub const PREFIX: &str = "custom_";

/// Where a tool saved with no category goes.
pub const DEFAULT_CATEGORY: &str = "custom";

/// The default category's fixed description.
pub const DEFAULT_CATEGORY_DESCRIPTION: &str = "Tools you made that have no category yet";

/// The suffix a clashing category name gets.
pub const CLASH_SUFFIX: &str = "custom";

/// `^[a-z][a-z0-9_]*$`, at most 64 characters: a tool or category name.
pub fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    name.len() <= 64
        && chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// The function a script calls for tool `name`.
pub fn call_as(name: &str) -> String {
    format!("{PREFIX}{name}")
}

/// The tool behind a `custom_<name>` function, if it is one.
pub fn tool_name(function: &str) -> Option<&str> {
    function.strip_prefix(PREFIX).filter(|name| !name.is_empty())
}

/// The stored name of a requested category: as asked, or `<name>_custom` when that is a built-in
/// category's name or in `reserved` (the addon ids and MCP server keys of the agent).
pub fn category_name(requested: &str, reserved: &[String]) -> String {
    crate::agents::tools::reserved_category_name(requested, reserved, CLASH_SUFFIX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_rule() {
        assert!(valid_name("weekly_report_2"));
        for bad in ["", "Weekly", "2x", "a-b", "a b", "_a", &"a".repeat(65)] {
            assert!(!valid_name(bad), "{bad:?}");
        }
    }

    #[test]
    fn calls_go_by_the_prefix() {
        assert_eq!(call_as("echo"), "custom_echo");
        assert_eq!(tool_name("custom_echo"), Some("echo"));
        assert_eq!(tool_name("custom_"), None);
        assert_eq!(tool_name("memory_read"), None);
    }

    #[test]
    fn a_clashing_category_gets_the_suffix() {
        assert_eq!(category_name("memory", &[]), "memory_custom");
        assert_eq!(category_name("brave_search", &["brave_search".into()]), "brave_search_custom");
        assert_eq!(category_name("reports", &[]), "reports");
    }
}
