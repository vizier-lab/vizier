//! Connect codes: eight symbols a person can type on a phone, kept only as a hash.

use ring::{
    digest,
    rand::{SecureRandom, SystemRandom},
};

/// No `0 O 1 I L`, so a code read off one screen types the same on another.
pub const ALPHABET: &[u8] = b"23456789ABCDEFGHJKMNPQRSTUVWXYZ";

pub const LENGTH: usize = 8;

/// How long a code can be redeemed.
pub const LIFETIME_MS: i64 = 10 * 60 * 1000;

/// A new code, uniformly random over [`ALPHABET`].
pub fn generate() -> crate::Result<String> {
    // Bytes at or above the last multiple of the alphabet's size are thrown away, so no symbol is
    // likelier than another.
    let limit = (256 / ALPHABET.len() * ALPHABET.len()) as u8;
    let random = SystemRandom::new();
    let mut code = String::with_capacity(LENGTH);
    let mut bytes = [0u8; LENGTH * 2];
    while code.len() < LENGTH {
        random
            .fill(&mut bytes)
            .map_err(|_| crate::error::VizierError("no random numbers available".into()))?;
        for byte in bytes {
            if byte < limit && code.len() < LENGTH {
                code.push(ALPHABET[byte as usize % ALPHABET.len()] as char);
            }
        }
    }
    Ok(code)
}

/// `ABCD-EFGH`, as people are shown it.
pub fn display(code: &str) -> String {
    let split = code.len().min(LENGTH / 2);
    format!("{}-{}", &code[..split], &code[split..])
}

/// What a person typed, as the code it means: case, spaces and dashes don't matter. `None` for
/// anything that cannot be a code.
pub fn normalize(input: &str) -> Option<String> {
    let code: String = input
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| c.to_ascii_uppercase())
        .collect();
    (code.len() == LENGTH && code.bytes().all(|b| ALPHABET.contains(&b))).then_some(code)
}

/// The hex SHA-256 of a normalised code, which is all that is stored.
pub fn hash(normalized: &str) -> String {
    digest::digest(&digest::SHA256, normalized.as_bytes())
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_is_eight_symbols_of_the_alphabet() {
        assert_eq!(ALPHABET.len(), 31);
        for ambiguous in *b"0O1IL" {
            assert!(!ALPHABET.contains(&ambiguous));
        }
        for _ in 0..200 {
            let code = generate().unwrap();
            assert_eq!(code.len(), LENGTH);
            assert!(code.bytes().all(|b| ALPHABET.contains(&b)), "{code}");
        }
        assert_ne!(generate().unwrap(), generate().unwrap());
    }

    #[test]
    fn what_a_person_types_normalises_to_the_code() {
        let code = generate().unwrap();
        assert_eq!(normalize(&display(&code)).as_deref(), Some(code.as_str()));
        assert_eq!(normalize(" abcd-efgh ").as_deref(), Some("ABCDEFGH"));
        assert_eq!(normalize("ab cd ef gh").as_deref(), Some("ABCDEFGH"));
        assert_eq!(display("ABCDEFGH"), "ABCD-EFGH");
    }

    #[test]
    fn what_cannot_be_a_code_is_refused() {
        for input in ["", "ABCD", "ABCDEFGHJ", "ABCD0FGH", "ABCDOFGH", "ABCD1FGH", "ABCD_FGH"] {
            assert_eq!(normalize(input), None, "{input:?}");
        }
    }

    #[test]
    fn the_hash_is_stable_and_hex() {
        assert_eq!(hash("ABCDEFGH"), hash("ABCDEFGH"));
        assert_ne!(hash("ABCDEFGH"), hash("ABCDEFGJ"));
        assert_eq!(hash("ABCDEFGH").len(), 64);
        assert!(hash("ABCDEFGH").chars().all(|c| c.is_ascii_hexdigit()));
    }
}
