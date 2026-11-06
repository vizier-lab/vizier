//! The name rule for Workspaces and topics (`specs/053-workspace-channel/data-model.md`):
//! agents type these into tool calls, so they are short, lower-case and shell-safe.

/// The longest name, in characters.
pub const MAX_NAME_CHARS: usize = 64;

/// Why a name was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameError {
    Empty,
    TooLong,
    BadCharacters,
}

impl NameError {
    /// The rule in words, for an error message or a tool result.
    pub const RULE: &'static str =
        "1 to 64 characters: lower-case letters, digits and '-', starting with a letter or digit";
}

/// Checks a name against `^[a-z0-9][a-z0-9-]*$` with 1 to 64 characters.
pub fn validate(name: &str) -> Result<(), NameError> {
    if name.is_empty() {
        return Err(NameError::Empty);
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(NameError::TooLong);
    }
    let mut chars = name.chars();
    let first_ok = chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    if first_ok && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        Ok(())
    } else {
        Err(NameError::BadCharacters)
    }
}

/// The key two names are compared by, so `Ops` and `ops` cannot both exist (FR-001).
pub fn key(name: &str) -> String {
    name.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_documented_shapes() {
        for name in ["ops", "daily-incidents", "a", "9lives", "a-b-c"] {
            assert_eq!(validate(name), Ok(()), "{name}");
        }
        assert_eq!(validate(&"a".repeat(64)), Ok(()));
    }

    #[test]
    fn refuses_everything_else() {
        assert_eq!(validate(""), Err(NameError::Empty));
        assert_eq!(validate(&"a".repeat(65)), Err(NameError::TooLong));
        for name in ["-ops", "Ops", "ops team", "ops_team", "ops!", "é", "ops/x"] {
            assert_eq!(validate(name), Err(NameError::BadCharacters), "{name}");
        }
    }

    #[test]
    fn the_key_ignores_case() {
        assert_eq!(key("OPS"), key("ops"));
    }
}
