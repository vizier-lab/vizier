//! What the woken agent reads (`contracts/background-report.md`) and what
//! `cancel_background_job` returns. Pure: no storage, no transport.

use std::fmt::Display;

use crate::schema::{BackgroundJob, BackgroundJobId, BackgroundReport, JobKind, ReportEntry};

/// A report entry's body is cut at this many characters.
pub const REPORT_TEXT_LIMIT: usize = 4000;

/// A heading quotes at most this many characters of the prompt's first line.
const HEADING_PROMPT_LIMIT: usize = 80;

const TRUNCATION_MARKER: &str = " … [truncated]";

/// Cut `text` to `limit` characters, marking the cut. Returns whether it cut.
pub fn truncate(text: &str, limit: usize) -> (String, bool) {
    match text.char_indices().nth(limit) {
        Some((end, _)) => (format!("{}{TRUNCATION_MARKER}", &text[..end]), true),
        None => (text.to_string(), false),
    }
}

/// The prompt's first line, shortened for a heading.
fn heading_prompt(prompt: &str) -> String {
    let line = prompt.lines().next().unwrap_or("").trim();
    match line.char_indices().nth(HEADING_PROMPT_LIMIT - 1) {
        Some((end, _)) if line.chars().count() > HEADING_PROMPT_LIMIT => {
            format!("{}…", &line[..end])
        }
        _ => line.to_string(),
    }
}

/// `batch b-…` or `delegation b-… to <agent>`.
fn job_title(kind: JobKind, job_id: &str, delegated_to: Option<&str>) -> String {
    match (kind, delegated_to) {
        (JobKind::Delegation, Some(target)) => format!("delegation {job_id} to {target}"),
        _ => format!("{} {job_id}", kind.as_str()),
    }
}

/// One `## <n>. <state> — <prompt>` section per entry, in the order given.
fn render_entries(entries: &[ReportEntry]) -> String {
    entries
        .iter()
        .map(|entry| {
            let heading = format!(
                "## {}. {} — {}",
                entry.ordinal + 1,
                entry.state.label(),
                heading_prompt(&entry.prompt)
            );
            if entry.text.is_empty() {
                heading
            } else {
                format!("{heading}\n{}", entry.text)
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn render_report(report: &BackgroundReport) -> String {
    let title = job_title(report.kind, &report.job_id, report.delegated_to.as_deref());
    let mut out = format!(
        "# Background report: {title}\n\n\
         This is not a message from a person. It reports the outcome of background work you started\n\
         earlier in this conversation. Act on it: tell the person if they are waiting on it, continue\n\
         the work, or do nothing if no follow-up is needed."
    );
    let sections = render_entries(&report.entries);
    if !sections.is_empty() {
        out.push_str("\n\n");
        out.push_str(&sections);
    }
    out
}

/// `cancel_background_job`'s answer (`contracts/agent-tools.md`).
pub fn render_cancel_result(
    job: &BackgroundJob,
    entries: &[ReportEntry],
    reason: Option<&str>,
    nested_ids: &[BackgroundJobId],
) -> String {
    let title = job_title(job.kind, &job.id, job.delegated_to().as_deref());
    let mut out = match reason {
        Some(reason) if !reason.trim().is_empty() => {
            format!("Cancelled background {title} (reason: {reason}).")
        }
        _ => format!("Cancelled background {title}."),
    };
    let sections = render_entries(entries);
    if !sections.is_empty() {
        out.push_str("\n\n");
        out.push_str(&sections);
    }
    if !nested_ids.is_empty() {
        out.push_str("\n\nAlso cancelled nested jobs: ");
        out.push_str(&nested_ids.join(", "));
    }
    out
}

impl Display for BackgroundReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", render_report(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::PieceState;

    fn entry(ordinal: u32, state: PieceState, prompt: &str, text: &str) -> ReportEntry {
        ReportEntry {
            ordinal,
            prompt: prompt.into(),
            state,
            text: text.into(),
            truncated: false,
        }
    }

    #[test]
    fn entries_render_in_the_order_given_with_one_based_numbers() {
        let report = BackgroundReport {
            job_id: "b-7f3a9c".into(),
            kind: JobKind::Batch,
            delegated_to: None,
            entries: vec![
                entry(0, PieceState::Answered, "Research topic A", "answer A"),
                entry(1, PieceState::TimedOut, "Research topic C", "No answer within 600s"),
                entry(2, PieceState::Failed, "Research topic B", "boom"),
            ],
        };

        let text = render_report(&report);
        assert!(text.starts_with("# Background report: batch b-7f3a9c\n\nThis is not a message from a person."));
        let a = text.find("## 1. answered — Research topic A\nanswer A").unwrap();
        let c = text.find("## 2. timed out — Research topic C\nNo answer within 600s").unwrap();
        let b = text.find("## 3. failed — Research topic B\nboom").unwrap();
        assert!(a < c && c < b, "{text}");
    }

    #[test]
    fn truncation_marks_the_cut() {
        let long = "x".repeat(REPORT_TEXT_LIMIT + 10);
        let (cut, truncated) = truncate(&long, REPORT_TEXT_LIMIT);
        assert!(truncated);
        assert!(cut.ends_with(" … [truncated]"));
        assert_eq!(cut.chars().count(), REPORT_TEXT_LIMIT + TRUNCATION_MARKER.chars().count());

        let (same, truncated) = truncate("short", REPORT_TEXT_LIMIT);
        assert!(!truncated);
        assert_eq!(same, "short");

        // Counts characters, not bytes.
        let (cut, truncated) = truncate("ééééé", 3);
        assert!(truncated);
        assert!(cut.starts_with("ééé "));
    }

    #[test]
    fn a_delegation_names_its_target() {
        let report = BackgroundReport {
            job_id: "b-91c2e0".into(),
            kind: JobKind::Delegation,
            delegated_to: Some("archivist".into()),
            entries: vec![entry(0, PieceState::Answered, "Archive notes", "done")],
        };
        assert!(render_report(&report).starts_with("# Background report: delegation b-91c2e0 to archivist"));
    }

    #[test]
    fn a_heading_uses_the_prompts_first_line_shortened() {
        let prompt = format!("{}\nsecond line", "y".repeat(120));
        let report = BackgroundReport {
            job_id: "b-1".into(),
            kind: JobKind::Batch,
            delegated_to: None,
            entries: vec![entry(0, PieceState::Answered, &prompt, "ok")],
        };
        let text = render_report(&report);
        let heading = text.lines().find(|l| l.starts_with("## 1.")).unwrap();
        assert!(!heading.contains("second line"));
        let quoted = heading.trim_start_matches("## 1. answered — ");
        assert_eq!(quoted.chars().count(), HEADING_PROMPT_LIMIT);
        assert!(quoted.ends_with('…'));
    }

    #[test]
    fn a_cancelled_entry_has_an_empty_body_and_nested_jobs_are_listed() {
        let job = BackgroundJob {
            id: "b-7f3a9c".into(),
            kind: JobKind::Batch,
            origin: crate::schema::VizierSession(
                "a".into(),
                crate::schema::VizierChannelId::Subagent,
                None,
            ),
            depth: 0,
            timeout_secs: 600,
            created_at: chrono::Utc::now(),
            finished_at: None,
            state: crate::schema::JobState::Cancelled,
            cancelled_by: None,
            reason: None,
            pieces: vec![],
        };
        let text = render_cancel_result(
            &job,
            &[
                entry(0, PieceState::Answered, "Research", "The Silk Road was"),
                entry(1, PieceState::Cancelled, "Summarise", ""),
            ],
            Some("user asked to stop"),
            &["b-000001".into(), "b-000002".into()],
        );
        assert!(text.starts_with("Cancelled background batch b-7f3a9c (reason: user asked to stop).\n\n"));
        assert!(text.contains("## 1. answered — Research\nThe Silk Road was\n\n## 2. cancelled — Summarise\n\nAlso cancelled nested jobs: b-000001, b-000002"), "{text}");
    }
}
