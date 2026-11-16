//! The text the model reads when detached scripts finish (spec 058 contracts/detached-flag.md
//! §3). One message, the same whether it is added to a running turn's next round or starts a
//! turn of its own.

use crate::schema::{DetachedScript, DetachedScriptState, detached::MAX_OUTCOME_BYTES};

/// The first line of every delivery.
pub const DELIVERY_HEADING: &str = "[Background work finished]";

/// Marks a value cut to fit.
pub const CUT_MARKER: &str = "… [cut at 32 KiB]";

/// One message for every script in `scripts`, in the order given.
pub fn render_delivery(scripts: &[DetachedScript]) -> String {
    let mut out = String::from(DELIVERY_HEADING);
    for script in scripts {
        out.push('\n');
        out.push_str(&render_one(script));
    }
    out
}

fn render_one(script: &DetachedScript) -> String {
    let elapsed = script
        .finished_at
        .map(|end| (end - script.created_at).num_seconds().max(0) as u64)
        .unwrap_or_default();
    let mut lines = vec![format!(
        "{}  {} in {}  — {}",
        script.id,
        script.state.label(),
        human_duration(elapsed),
        script.intent
    )];

    match (&script.state, &script.outcome) {
        (_, Some(serde_json::Value::Object(report))) => {
            let ok = report.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
            if ok {
                let result = report.get("result").cloned().unwrap_or_default();
                lines.push(format!("result: {}", compact(&result)));
            }
            if let Some(stdout) = report.get("stdout").and_then(|v| v.as_str())
                && !stdout.is_empty()
            {
                lines.push(format!("stdout: {}", stdout.trim_end()));
            }
            if let Some(error) = report.get("error").filter(|v| !v.is_null()) {
                let message = error.get("message").and_then(|v| v.as_str()).unwrap_or_default();
                lines.push(format!("error: {message}"));
                if let Some(traceback) = error.get("traceback").and_then(|v| v.as_str())
                    && !traceback.is_empty()
                {
                    lines.push(format!("traceback:\n{}", traceback.trim_end()));
                }
            }
            if let Some(files) = report.get("files").and_then(|v| v.as_array())
                && !files.is_empty()
            {
                let names: Vec<&str> = files.iter().filter_map(|f| f.as_str()).collect();
                lines.push(format!("files (in your session files): {}", names.join(", ")));
            }
        }
        (_, Some(serde_json::Value::String(reason))) => lines.push(format!("reason: {reason}")),
        (DetachedScriptState::TimedOut, _) => lines.push(format!(
            "reason: the script's time limit ({}s) was reached",
            script.timeout_secs
        )),
        _ => {}
    }

    cut(lines.join("\n"), MAX_OUTCOME_BYTES)
}

fn compact(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// `41s`, `2m 5s`, `1h 3m`.
pub fn human_duration(secs: u64) -> String {
    match secs {
        s if s < 60 => format!("{s}s"),
        s if s < 3600 => format!("{}m {}s", s / 60, s % 60),
        s => format!("{}h {}m", s / 3600, (s % 3600) / 60),
    }
}

/// `text` cut to at most `limit` bytes on a character boundary, with [`CUT_MARKER`].
pub fn cut(text: String, limit: usize) -> String {
    if text.len() <= limit {
        return text;
    }
    let mut end = limit.saturating_sub(CUT_MARKER.len());
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{CUT_MARKER}", &text[..end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{VizierChannelId, VizierSession};

    fn script(state: DetachedScriptState, outcome: Option<serde_json::Value>) -> DetachedScript {
        let created_at = crate::time::now();
        DetachedScript {
            id: "d_k3x9a1b2".into(),
            agent_id: "a".into(),
            session: VizierSession("a".into(), VizierChannelId::System, None),
            parent_id: None,
            depth: 0,
            intent: "check both vendor pages".into(),
            state,
            timeout_secs: 600,
            code: String::new(),
            tool_calls: serde_json::json!([]),
            created_at,
            finished_at: Some(created_at + chrono::Duration::seconds(41)),
            outcome,
            delivered_at: None,
            cancelled_by: None,
        }
    }

    #[test]
    fn a_success_gives_result_stdout_and_files() {
        let text = render_delivery(&[script(
            DetachedScriptState::Succeeded,
            Some(serde_json::json!({
                "ok": true, "result": {"a": 1}, "stdout": "hello\n", "error": null,
                "files": ["chart.png"],
            })),
        )]);
        assert!(text.starts_with("[Background work finished]\nd_k3x9a1b2  succeeded in 41s  — check both vendor pages"), "{text}");
        assert!(text.contains("result: {\"a\":1}"), "{text}");
        assert!(text.contains("stdout: hello"), "{text}");
        assert!(text.contains("chart.png"), "{text}");
    }

    #[test]
    fn a_failure_gives_its_error_and_traceback() {
        let text = render_delivery(&[script(
            DetachedScriptState::Failed,
            Some(serde_json::json!({
                "ok": false, "result": null, "stdout": "",
                "error": {"kind": "script", "message": "NameError: x", "traceback": "line 1"},
            })),
        )]);
        assert!(text.contains("failed in 41s"), "{text}");
        assert!(text.contains("error: NameError: x"), "{text}");
        assert!(text.contains("traceback:\nline 1"), "{text}");
        assert!(!text.contains("result:"), "{text}");
    }

    #[test]
    fn timed_out_and_interrupted_say_why() {
        let text = render_delivery(&[script(DetachedScriptState::TimedOut, None)]);
        assert!(text.contains("timed out in 41s"), "{text}");
        assert!(text.contains("time limit (600s)"), "{text}");

        let text = render_delivery(&[script(
            DetachedScriptState::Interrupted,
            Some(serde_json::json!("the server restarted")),
        )]);
        assert!(text.contains("interrupted in 41s"), "{text}");
        assert!(text.contains("reason: the server restarted"), "{text}");
    }

    #[test]
    fn scripts_final_together_arrive_together() {
        let text = render_delivery(&[
            script(DetachedScriptState::Succeeded, Some(serde_json::json!({"ok": true, "result": 1}))),
            script(DetachedScriptState::Failed, Some(serde_json::json!({"ok": false, "error": {"message": "x"}}))),
        ]);
        assert_eq!(text.matches("d_k3x9a1b2").count(), 2);
        assert_eq!(text.matches(DELIVERY_HEADING).count(), 1);
    }

    #[test]
    fn a_long_result_is_cut_with_a_marker() {
        let big = "é".repeat(40_000);
        let text = render_delivery(&[script(
            DetachedScriptState::Succeeded,
            Some(serde_json::json!({"ok": true, "result": big})),
        )]);
        assert!(text.len() <= MAX_OUTCOME_BYTES + DELIVERY_HEADING.len() + 1);
        assert!(text.ends_with(CUT_MARKER));
    }

    #[test]
    fn durations_read_like_a_person_would_say_them() {
        assert_eq!(human_duration(41), "41s");
        assert_eq!(human_duration(125), "2m 5s");
        assert_eq!(human_duration(3780), "1h 3m");
    }
}
