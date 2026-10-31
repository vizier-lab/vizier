//! The project instruction document (`specs/040-projects-mode/data-model.md` §3): `AGENTS.md`
//! first, then `CLAUDE.md`, from the project root only, UTF-8 lossy, capped at
//! [`INSTRUCTIONS_CAP_BYTES`] on a character boundary. A missing or unreadable file is harmless.

use std::path::Path;

use crate::schema::INSTRUCTIONS_CAP_BYTES;

/// The candidate documents, in precedence order.
pub const CANDIDATES: &[&str] = &["AGENTS.md", "CLAUDE.md"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectInstructions {
    pub text: String,
    /// The document was longer than the cap and was cut.
    pub truncated: bool,
}

/// Read the project's instruction document, if it has one.
pub fn read(folder: &Path) -> Option<ProjectInstructions> {
    for name in CANDIDATES {
        let Ok(bytes) = std::fs::read(folder.join(name)) else {
            continue;
        };
        return Some(capped(&bytes));
    }
    None
}

/// Decode lossily and keep at most the cap, cut on a UTF-8 boundary.
fn capped(bytes: &[u8]) -> ProjectInstructions {
    let (slice, truncated) = match bytes.len() > INSTRUCTIONS_CAP_BYTES {
        true => (&bytes[..INSTRUCTIONS_CAP_BYTES], true),
        false => (bytes, false),
    };
    // An incomplete character at the cut is dropped, so the prompt ends on a whole one; an
    // invalid byte inside the document is replaced, never used as a reason to truncate.
    let text = match std::str::from_utf8(slice) {
        Ok(text) => text.to_string(),
        Err(err) => match err.error_len() {
            None => String::from_utf8_lossy(&slice[..err.valid_up_to()]).into_owned(),
            Some(_) => String::from_utf8_lossy(slice).into_owned(),
        },
    };
    ProjectInstructions { text, truncated }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agents_md_wins_over_claude_md() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), "from agents").unwrap();
        std::fs::write(dir.path().join("CLAUDE.md"), "from claude").unwrap();
        let read = read(dir.path()).unwrap();
        assert_eq!(read.text, "from agents");
        assert!(!read.truncated);
    }

    #[test]
    fn claude_md_is_the_fallback() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("CLAUDE.md"), "from claude").unwrap();
        assert_eq!(read(dir.path()).unwrap().text, "from claude");
    }

    #[test]
    fn a_missing_document_is_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read(dir.path()).is_none());
    }

    #[test]
    fn invalid_utf8_is_read_lossily() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), [b'a', 0xff, b'b']).unwrap();
        let read = read(dir.path()).unwrap();
        assert!(read.text.starts_with('a'));
        assert!(read.text.ends_with('b'));
    }

    #[test]
    fn forty_kib_is_cut_to_the_cap_on_a_boundary() {
        let dir = tempfile::tempdir().unwrap();
        // A multi-byte character straddling the cut must not leave half of itself behind.
        let body = "é".repeat(40 * 1024);
        std::fs::write(dir.path().join("AGENTS.md"), body).unwrap();
        let read = read(dir.path()).unwrap();
        assert!(read.truncated);
        assert!(read.text.len() <= INSTRUCTIONS_CAP_BYTES);
        assert!(read.text.is_char_boundary(read.text.len()));
    }
}
