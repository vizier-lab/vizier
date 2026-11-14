//! Names (`specs/061-agent-custom-tools` research R12, revised 2026-11-15): a custom tool is
//! called by its own name, with no prefix. A save is refused when the name is already taken (by
//! any tool the agent has, a retired tool name or a sandbox helper); a tool attached later wins
//! its name, and the custom tool needs attention until it is renamed. Categories share the clash
//! rule MCP servers and client categories use.

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

/// Why `name` can never be a custom tool's, whatever the agent has: a Python builtin or sandbox
/// helper a script could not reach it past, the sandbox tools, or a retired tool name that
/// answers with guidance.
pub fn reserved(name: &str) -> Option<&'static str> {
    if super::calls::is_builtin(name) {
        return Some("a Python builtin or sandbox helper");
    }
    if ["think", "execute_python", "list_tool_functions", "describe_tool_function"].contains(&name) {
        return Some("a sandbox tool");
    }
    if crate::agents::tools::retired_hint(name).is_some() {
        return Some("a retired tool name");
    }
    None
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
    fn builtins_sandbox_tools_and_retired_names_are_reserved() {
        for name in ["len", "print", "list_tools", "execute_python", "parallel_subagents"] {
            assert!(reserved(name).is_some(), "{name}");
        }
        assert!(reserved("weekly_digest").is_none());
    }

    #[test]
    fn a_clashing_category_gets_the_suffix() {
        assert_eq!(category_name("memory", &[]), "memory_custom");
        assert_eq!(category_name("brave_search", &["brave_search".into()]), "brave_search_custom");
        assert_eq!(category_name("reports", &[]), "reports");
    }
}
