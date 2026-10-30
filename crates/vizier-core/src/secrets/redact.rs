//! The turn's secret redactor (`specs/027-agent-secrets` research R2–R3): exact occurrences of
//! a value this turn handled are replaced with [`MARKER`] before the execution report, the
//! bridge's log record or the `execute_tool` telemetry can carry them.
//!
//! It is armed by `set_secret` and `get_secret` only, so an ordinary turn pays nothing: with
//! no values, every method returns its input unchanged. A value never persists beyond the turn
//! that read it. Redaction is exact-string only; a transformed form (base64, URL-encoding,
//! hashing) is outside the guarantee (spec Assumptions).

use parking_lot::Mutex;

use crate::{
    sandbox::{ExecutionError, ExecutionReport},
    secrets::MIN_VALUE_BYTES,
};

/// What an exact occurrence of a secret becomes.
pub const MARKER: &str = "[redacted]";

/// Most values one turn arms: a script cannot grow the scan without bound.
pub const MAX_ARMED_VALUES: usize = 64;

#[derive(Default)]
pub struct SecretRedactor {
    values: Mutex<Vec<String>>,
}

impl SecretRedactor {
    /// Remember a value handled in this turn. Values shorter than [`MIN_VALUE_BYTES`] are
    /// ignored (they would corrupt ordinary text), duplicates by name are free, and at most
    /// [`MAX_ARMED_VALUES`] are kept.
    pub fn arm(&self, value: &str) {
        if value.len() < MIN_VALUE_BYTES {
            return;
        }
        let mut values = self.values.lock();
        if values.len() >= MAX_ARMED_VALUES || values.iter().any(|v| v == value) {
            return;
        }
        values.push(value.to_string());
    }

    /// Whether this turn has handled a secret, and so whether scrubbing does anything.
    pub fn is_armed(&self) -> bool {
        !self.values.lock().is_empty()
    }

    /// Exact substring replacement, longest value first so a value containing another is
    /// replaced whole.
    pub fn text(&self, input: &str) -> String {
        let mut values = self.values.lock().clone();
        if values.is_empty() {
            return input.to_string();
        }
        values.sort_by_key(|value| std::cmp::Reverse(value.len()));
        let mut out = input.to_string();
        for value in values {
            if out.contains(&value) {
                out = out.replace(&value, MARKER);
            }
        }
        out
    }

    /// Recursively scrub every string in a JSON value, including object keys.
    pub fn json(&self, value: &mut serde_json::Value) {
        if !self.is_armed() {
            return;
        }
        match value {
            serde_json::Value::String(text) => *text = self.text(text),
            serde_json::Value::Array(items) => {
                for item in items {
                    self.json(item);
                }
            }
            serde_json::Value::Object(map) => {
                let entries = std::mem::take(map);
                for (key, mut value) in entries {
                    self.json(&mut value);
                    map.insert(self.text(&key), value);
                }
            }
            _ => {}
        }
    }

    /// Scrub everything a script's report may show: `result`, `stdout`, the error's message and
    /// traceback, and every nested tool call's `arguments` and `error`.
    pub fn report(&self, report: &mut ExecutionReport) {
        if !self.is_armed() {
            return;
        }
        self.json(&mut report.result);
        report.stdout = self.text(&report.stdout);
        if let Some(ExecutionError { message, traceback, .. }) = &mut report.error {
            *message = self.text(message);
            *traceback = self.text(traceback);
        }
        for call in &mut report.tool_calls {
            self.json(&mut call.arguments);
            if let Some(error) = &mut call.error {
                *error = self.text(error);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sandbox::{ExecutionErrorKind, ToolInvocationRecord};

    const TOKEN: &str = "agent-token-abc123";
    const OTHER: &str = "rotated-token-xyz789";

    fn armed() -> SecretRedactor {
        let redactor = SecretRedactor::default();
        redactor.arm(TOKEN);
        redactor
    }

    fn report() -> ExecutionReport {
        ExecutionReport {
            ok: true,
            result: serde_json::json!({ "status": "found", "value": TOKEN }),
            stdout: format!("value is {TOKEN}"),
            error: None,
            tool_calls: vec![],
            duration_ms: 1,
        }
    }

    #[test]
    fn an_unarmed_redactor_changes_nothing() {
        let redactor = SecretRedactor::default();
        assert!(!redactor.is_armed());
        let mut json = serde_json::json!({ "value": TOKEN, TOKEN: 1 });
        redactor.json(&mut json);
        assert_eq!(json, serde_json::json!({ "value": TOKEN, TOKEN: 1 }));
        assert_eq!(redactor.text(TOKEN), TOKEN);
        let mut report = report();
        let before = serde_json::to_string(&report).unwrap();
        redactor.report(&mut report);
        assert_eq!(serde_json::to_string(&report).unwrap(), before);
    }

    #[test]
    fn values_are_armed_once_and_only_when_long_enough() {
        let redactor = SecretRedactor::default();
        redactor.arm("short");
        assert!(!redactor.is_armed());
        redactor.arm(TOKEN);
        redactor.arm(TOKEN);
        assert!(redactor.is_armed());
        assert_eq!(redactor.values.lock().len(), 1);

        let many = SecretRedactor::default();
        for i in 0..MAX_ARMED_VALUES + 10 {
            many.arm(&format!("value-{i:04}"));
        }
        assert_eq!(many.values.lock().len(), MAX_ARMED_VALUES);
    }

    #[test]
    fn exact_occurrences_become_the_marker() {
        let redactor = armed();
        assert_eq!(redactor.text(&format!("Bearer {TOKEN}")), "Bearer [redacted]");
        assert_eq!(redactor.text("nothing here"), "nothing here");
        assert_eq!(redactor.text(&format!("{TOKEN}{TOKEN}")), "[redacted][redacted]");
    }

    /// The longer value goes first, so its shorter part never leaves a fragment behind.
    #[test]
    fn the_longest_value_is_replaced_first() {
        let redactor = SecretRedactor::default();
        redactor.arm("token-abc123");
        redactor.arm("agent-token-abc123");
        assert_eq!(redactor.text(TOKEN), "[redacted]");
    }

    #[test]
    fn json_strings_and_keys_are_scrubbed() {
        let redactor = armed();
        let mut json = serde_json::json!({
            "value": TOKEN,
            "nested": { "header": format!("Bearer {TOKEN}"), "list": [TOKEN, 1, null] },
            TOKEN: "as a key",
        });
        redactor.json(&mut json);
        let text = serde_json::to_string(&json).unwrap();
        assert!(!text.contains(TOKEN), "{text}");
        assert!(text.contains(MARKER));
        assert_eq!(json["value"], MARKER);
        assert_eq!(json["nested"]["header"], "Bearer [redacted]");
        assert_eq!(json["nested"]["list"][0], MARKER);
        assert_eq!(json[MARKER], "as a key");
        assert_eq!(json["nested"]["list"][1], 1);
        assert_eq!(json["nested"]["list"][2], serde_json::Value::Null);
    }

    #[test]
    fn a_report_is_scrubbed_everywhere_including_nested_calls() {
        let redactor = armed();
        let mut report = report();
        report.error = Some(ExecutionError {
            kind: ExecutionErrorKind::Script,
            message: format!("failed with {TOKEN}"),
            traceback: format!("line 1: http_client({TOKEN})"),
            limit: None,
        });
        report.tool_calls.push(ToolInvocationRecord {
            seq: 1,
            name: "http_client".into(),
            arguments: serde_json::json!({ "headers": { "Authorization": format!("Bearer {TOKEN}") } }),
            ok: false,
            error: Some(format!("401 for {TOKEN}")),
            duration_ms: 1,
            ends_turn: false,
        });

        redactor.report(&mut report);
        let text = serde_json::to_string(&report).unwrap();
        assert!(!text.contains(TOKEN), "{text}");
        assert_eq!(report.error.unwrap().message, "failed with [redacted]");
        assert_eq!(report.tool_calls[0].arguments["headers"]["Authorization"], "Bearer [redacted]");
        assert_eq!(report.tool_calls[0].error.as_deref(), Some("401 for [redacted]"));
    }

    /// Only the values this turn handled are scrubbed; another value is ordinary text.
    #[test]
    fn other_text_is_untouched() {
        let redactor = armed();
        assert_eq!(redactor.text(OTHER), OTHER);
        let mut report = report();
        report.stdout = OTHER.to_string();
        redactor.report(&mut report);
        assert_eq!(report.stdout, OTHER);
    }
}
