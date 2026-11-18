//! Last-resort backstop for `POST /shares` (FR-051): refuse a snapshot whose text holds a
//! secret that can be told with near certainty. This is NOT redaction and does not promise a
//! clean snapshot; the client removes what it can find and the person reviews the rest. Only
//! PEM private-key blocks and vendor-prefixed tokens are matched. Generic `password=` /
//! `token=` shapes are left alone on purpose (too many false positives in ordinary prose).

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

/// Stable error code of the refusal (HTTP 422).
pub const SECRET_DETECTED: &str = "secret_detected";

/// Names the kind of match only; never the text.
struct Rule {
    kind: &'static str,
    re: Regex,
    /// Extra condition on the matched text (rules out hyphenated words).
    needs_digit: bool,
}

static RULES: LazyLock<Vec<Rule>> = LazyLock::new(|| {
    let rule = |kind, pat: &str, needs_digit| Rule {
        kind,
        re: Regex::new(pat).expect("share_guard pattern"),
        needs_digit,
    };
    vec![
        rule("private key", r"-----BEGIN [A-Z0-9 ]*PRIVATE KEY(?: BLOCK)?-----", false),
        rule("API key", r"\bsk-[A-Za-z0-9_-]{32,}", true),
        rule("GitHub token", r"\bgh[pousr]_[A-Za-z0-9]{36,}", false),
        rule("GitHub token", r"\bgithub_pat_[A-Za-z0-9_]{22,}", false),
        rule("AWS access key", r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b", false),
        rule("GitLab token", r"\bglpat-[A-Za-z0-9_-]{20,}", false),
        rule("Slack token", r"\bxox[abpr]-[A-Za-z0-9-]{10,}", false),
        rule("Hugging Face token", r"\bhf_[A-Za-z0-9]{30,}", false),
        rule("npm token", r"\bnpm_[A-Za-z0-9]{36}\b", false),
        rule("SendGrid key", r"\bSG\.[A-Za-z0-9_-]{16,}\.[A-Za-z0-9_-]{16,}", false),
        rule("Google token", r"\bya29\.[A-Za-z0-9_-]{20,}", false),
        rule("Google client secret", r"\bGOCSPX-[A-Za-z0-9_-]{20,}", false),
    ]
});

fn scan_text(text: &str) -> Option<&'static str> {
    RULES.iter().find_map(|r| {
        r.re.find_iter(text)
            .any(|m| !r.needs_digit || m.as_str().bytes().any(|b| b.is_ascii_digit()))
            .then_some(r.kind)
    })
}

/// Walks every string of the document (values and keys) except the base64 `media` payloads,
/// and returns the kind of the first secret found.
fn scan_value(v: &Value) -> Option<&'static str> {
    match v {
        Value::String(s) => scan_text(s),
        Value::Array(a) => a.iter().find_map(scan_value),
        Value::Object(o) => o.iter().find_map(|(k, v)| scan_text(k).or_else(|| scan_value(v))),
        _ => None,
    }
}

/// `Some(kind)` when the snapshot or the title holds a certain secret. `media` is skipped at
/// the top level: it is base64 image data, not text anyone wrote.
pub fn find_secret(doc: &serde_json::Map<String, Value>, title: &str) -> Option<&'static str> {
    scan_text(title).or_else(|| {
        doc.iter().filter(|(k, _)| k.as_str() != "media").find_map(|(k, v)| scan_text(k).or_else(|| scan_value(v)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn doc(text: &str) -> serde_json::Map<String, Value> {
        json!({"turns": [{"parts": [{"kind": "text", "markdown": text}]}]}).as_object().unwrap().clone()
    }
    fn hit(text: &str) -> bool {
        find_secret(&doc(text), "t").is_some()
    }

    #[test]
    fn each_pattern_is_found() {
        let a36 = "a1B2c3D4e5F6g7H8i9J0k1L2m3N4o5P6q7R8";
        for s in [
            "-----BEGIN RSA PRIVATE KEY-----\nMIIE",
            "-----BEGIN PRIVATE KEY-----",
            "-----BEGIN OPENSSH PRIVATE KEY-----",
            "-----BEGIN ENCRYPTED PRIVATE KEY-----",
            "key sk-proj-abcdEFGH1234abcdEFGH1234abcdEFGH1234",
            "sk-ant-api03-abcdEFGH1234abcdEFGH1234abcdEFGH1234",
            &format!("ghp_{a36}"),
            &format!("use gho_{a36} now"),
            &format!("ghs_{a36}"),
            "github_pat_11ABCDEFG0abcdefghijkl_mnopqrstuvwxyz",
            "AKIAIOSFODNN7EXAMPLE",
            "ASIAIOSFODNN7EXAMPLE",
            "glpat-abcdefghij1234567890",
            "xoxb-1234567890-abcdefghij",
            "xoxp-1234567890-abcdefghij",
            "hf_abcdefghijklmnopqrstuvwxyzABCDEF12",
            &format!("npm_{a36}"),
            "SG.abcdefghijklmnop1234.abcdefghijklmnop1234",
            "ya29.a0AfH6SMBabcdefghijklmnop",
            "GOCSPX-abcdefghijklmnopqrstuvwx",
        ] {
            assert!(hit(s), "should be found: {s}");
        }
    }

    #[test]
    fn ordinary_text_is_left_alone() {
        for s in [
            "the password is stored in a vault",
            "pip install sk-learn",
            "set max_token=2048 and token: abc",
            "password=hunter2",
            "Authorization: Bearer abc",
            "sk-short",
            "ghp_short",
            "hf_abc",
            "AKIA short",
            "risk-assessment-of-the-whole-migration-plan-for-q3",
            "sk-the-quick-brown-fox-jumps-over-the-lazy-dog",
            "-----BEGIN CERTIFICATE-----",
            "-----BEGIN PUBLIC KEY-----",
            "npm_short",
            "",
        ] {
            assert!(!hit(s), "should pass: {s}");
        }
    }

    #[test]
    fn every_text_field_is_scanned_but_not_media() {
        let key = "ghp_a1B2c3D4e5F6g7H8i9J0k1L2m3N4o5P6q7R8";
        assert!(find_secret(&doc("x"), key).is_some(), "title");
        let d = json!({"turns": [{"trail": [{"args": {"cmd": format!("curl -H {key}")}}]}]});
        assert!(find_secret(d.as_object().unwrap(), "t").is_some(), "trail args");
        let d = json!({"turns": [{"parts": [{"kind": "media", "alt": key}]}]});
        assert!(find_secret(d.as_object().unwrap(), "t").is_some(), "alt text");
        let d = json!({"turns": [{"parts": [{"kind": "ui", "spec": {"children": [{"label": key}]}}]}]});
        assert!(find_secret(d.as_object().unwrap(), "t").is_some(), "ui spec");
        let d = json!({"turns": [{"parts": []}], "media": {"k": {"data": format!("AKIAIOSFODNN7EXAMPLE{key}")}}});
        assert!(find_secret(d.as_object().unwrap(), "t").is_none(), "media payload skipped");
    }
}
