//! An agent's secrets (`specs/027-agent-secrets`): named values it can set and read from a
//! script, managed by people in the WebUI. One validator and one redaction boundary are
//! shared by both surfaces so they cannot drift.
//!
//! Values are plaintext in the embedded SQLite database and rely on host/disk protection, as
//! provider credentials do. Application-level encryption is out of scope (spec Assumptions).

pub mod access;
pub mod redact;
pub mod service;

pub use redact::SecretRedactor;

/// Longest a secret name may be, in bytes.
pub const MAX_NAME_BYTES: usize = 128;

/// Shortest a secret value may be, in bytes: shorter values make exact-string redaction
/// corrupt ordinary report text (research R11).
pub const MIN_VALUE_BYTES: usize = 8;

/// Longest a secret value may be, in bytes.
pub const MAX_VALUE_BYTES: usize = 65_536;

/// The rule every refusal about a name states.
pub const NAME_RULE: &str = "Secret names are 1–128 characters of letters, digits, '-', '_' and '.'.";

/// The rule every refusal about a value states.
pub const VALUE_RULE: &str = "Secret values are 8–65,536 bytes of text.";

/// 1–128 bytes of ASCII `[A-Za-z0-9._-]`, case-sensitive. A refusal says which rule failed.
pub fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.len() > MAX_NAME_BYTES {
        return Err(NAME_RULE.to_string());
    }
    if !name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    {
        return Err(NAME_RULE.to_string());
    }
    Ok(())
}

/// 8–65,536 bytes of UTF-8, stored exactly as supplied — no trimming, normalisation or
/// newline conversion.
pub fn validate_value(value: &str) -> Result<(), String> {
    if value.len() < MIN_VALUE_BYTES || value.len() > MAX_VALUE_BYTES {
        return Err(VALUE_RULE.to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_accept_letters_digits_and_the_three_separators() {
        for name in ["A", "TOKEN", "linear-token", "a.b_c-1", &"x".repeat(MAX_NAME_BYTES)] {
            assert!(validate_name(name).is_ok(), "{name}");
        }
        assert!(validate_name("Token").is_ok());
        // Case-sensitive by construction: this is simply another valid name.
        assert_ne!("Token", "token");
    }

    #[test]
    fn names_refuse_empty_long_and_unsupported_characters() {
        for name in [
            "",
            "a/b",
            "a b",
            "a:b",
            "a\nb",
            "é",
            "token!",
            &"x".repeat(MAX_NAME_BYTES + 1),
        ] {
            let err = validate_name(name).unwrap_err();
            assert_eq!(err, NAME_RULE, "{name:?}");
        }
    }

    #[test]
    fn values_accept_the_full_range_and_keep_whitespace() {
        assert!(validate_value("12345678").is_ok());
        assert!(validate_value("  spaces  ").is_ok());
        assert!(validate_value("line one\nline two\r\n").is_ok());
        assert!(validate_value(&"x".repeat(MAX_VALUE_BYTES)).is_ok());
        // Bytes, not characters: four two-byte characters are exactly the minimum.
        assert!(validate_value("éééé").is_ok());
    }

    #[test]
    fn values_refuse_short_and_oversized_text() {
        for value in ["", "1234567", &"x".repeat(MAX_VALUE_BYTES + 1)] {
            let err = validate_value(value).unwrap_err();
            assert_eq!(err, VALUE_RULE);
        }
        // Three two-byte characters are six bytes: under the minimum.
        assert_eq!(validate_value("ééé").unwrap_err(), VALUE_RULE);
    }
}
