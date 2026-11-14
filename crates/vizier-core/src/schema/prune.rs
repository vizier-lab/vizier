//! Pruning old tool outputs from the model's view (`specs/060-prune-tool-outputs`).
//!
//! [`prune_for_model`] runs after [`super::messages_for_model`] and replaces old, large tool
//! results with a one-line placeholder naming the history uid they were saved under, which
//! `recall_history` reads back. Storage and the WebUI never see a placeholder: it is built per
//! request, from a copy.
//!
//! The rule is one forward walk (`contracts/prune-rule.md`): a decision depends only on the
//! messages before it, so a result once pruned stays pruned, and the live turn and the replay
//! of the next turn agree. Two marks keep prunes rare: nothing happens until the eligible output
//! passes [`HIGH_TOKENS`], and a prune then goes down to [`LOW_TOKENS`], so the prompt prefix,
//! and with it the provider's cache, changes only at a prune.

use std::collections::{HashSet, VecDeque};

use serde_json::Value;

use crate::{
    llm::types::{Message, Part},
    sandbox::ExecutionReport,
};

/// A prune starts once the eligible, unpruned output passes this many estimated tokens.
pub const HIGH_TOKENS: u64 = 60_000;
/// A prune stops once that output is at or under this many.
pub const LOW_TOKENS: u64 = 30_000;
/// A result shorter than this is never pruned: its placeholder would save almost nothing.
pub const MIN_BYTES: usize = 2_048;

/// What one call of [`prune_for_model`] removed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PruneStats {
    /// Results replaced by placeholders.
    pub pruned: usize,
    /// Estimated tokens of those results, minus their placeholders.
    pub tokens_removed: u64,
}

/// The messages with old, large tool results replaced by placeholders, and what that removed.
/// Below [`HIGH_TOKENS`] the messages come back unchanged.
pub fn prune_for_model(messages: &[Message]) -> (Vec<Message>, PruneStats) {
    let mut round = 0usize;
    let mut queue: VecDeque<(usize, u64, usize)> = VecDeque::new();
    let mut total = 0u64;
    let mut pruned: HashSet<usize> = HashSet::new();

    for (index, message) in messages.iter().enumerate() {
        match message {
            Message::Assistant(_) => round += 1,
            Message::ToolResult { .. } => {
                let Some(text) = eligible_text(message) else {
                    continue;
                };
                let tokens = estimate_tokens(text.len());
                queue.push_back((index, tokens, round));
                total += tokens;
                if total > HIGH_TOKENS {
                    while total > LOW_TOKENS {
                        match queue.front() {
                            Some(&(oldest, oldest_tokens, oldest_round)) if oldest_round < round => {
                                queue.pop_front();
                                total -= oldest_tokens;
                                pruned.insert(oldest);
                            }
                            _ => break,
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if pruned.is_empty() {
        return (messages.to_vec(), PruneStats::default());
    }

    let mut stats = PruneStats::default();
    let out = messages
        .iter()
        .enumerate()
        .map(|(index, message)| match message {
            Message::ToolResult {
                call_id,
                content,
                record: Some(record),
            } if pruned.contains(&index) => {
                let size = text_bytes(content);
                let stub = placeholder(size, record);
                stats.pruned += 1;
                stats.tokens_removed += estimate_tokens(size)
                    .saturating_sub(estimate_tokens(stub.len()));
                Message::ToolResult {
                    call_id: call_id.clone(),
                    content: vec![Part::Text(stub)],
                    record: Some(record.clone()),
                }
            }
            other => other.clone(),
        })
        .collect();
    (out, stats)
}

/// The text a pruned result shows the model.
pub fn placeholder(bytes: usize, record: &str) -> String {
    format!(
        "[output pruned ({}) - use recall_history(\"{record}\") to recall]",
        human_size(bytes)
    )
}

/// Whether a tool result shows the model a placeholder rather than its output.
pub fn is_placeholder(text: &str) -> bool {
    text.starts_with("[output pruned (")
}

/// The result's text when it may be pruned (research R4): it was saved, it is text only, it is
/// at least [`MIN_BYTES`], and it is not a failed or turn-ending script report.
fn eligible_text(message: &Message) -> Option<String> {
    let Message::ToolResult {
        content,
        record: Some(_),
        ..
    } = message
    else {
        return None;
    };
    if content.iter().any(|part| !matches!(part, Part::Text(_))) {
        return None;
    }
    let text = content
        .iter()
        .filter_map(Part::text)
        .collect::<Vec<_>>()
        .join("\n");
    if text.len() < MIN_BYTES || is_placeholder(&text) {
        return None;
    }
    // Only a report can have failed or ended the turn; a cheap check keeps other results from
    // being parsed.
    if text.contains("\"tool_calls\"")
        && let Ok(value) = serde_json::from_str::<Value>(&text)
        && ExecutionReport::looks_like(&value)
        && (value.get("ok") != Some(&Value::Bool(true))
            || ExecutionReport::ending_call(&value).is_some())
    {
        return None;
    }
    Some(text)
}

fn text_bytes(content: &[Part]) -> usize {
    let texts: Vec<&str> = content.iter().filter_map(Part::text).collect();
    texts.iter().map(|t| t.len()).sum::<usize>() + texts.len().saturating_sub(1)
}

/// About four bytes a token: the marks are budgets, not limits (research R5).
fn estimate_tokens(bytes: usize) -> u64 {
    (bytes as u64).div_ceil(4)
}

/// `812 B`, `74.2 KB`, `200 KB`, `1.2 MB`: base 1,024, with one decimal below 100 KB and for MB.
fn human_size(bytes: usize) -> String {
    const KB: f64 = 1024.0;
    let b = bytes as f64;
    if b < KB {
        format!("{bytes} B")
    } else if b < KB * KB {
        let kb = b / KB;
        if kb < 100.0 {
            format!("{kb:.1} KB")
        } else {
            format!("{kb:.0} KB")
        }
    } else {
        format!("{:.1} MB", b / (KB * KB))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::llm::types::AssistantPart;

    fn call() -> Message {
        Message::Assistant(vec![AssistantPart::ToolCall {
            id: "execute_python".into(),
            name: "execute_python".into(),
            arguments: json!({}),
            provider_data: None,
        }])
    }

    /// A successful report whose result is `tokens` estimated tokens long.
    fn report(tokens: u64) -> String {
        json!({
            "ok": true,
            "result": "x".repeat((tokens * 4) as usize),
            "stdout": "",
            "tool_calls": [],
            "duration_ms": 1,
        })
        .to_string()
    }

    fn result(record: &str, text: String) -> Message {
        Message::ToolResult {
            call_id: "execute_python".into(),
            content: vec![Part::Text(text)],
            record: Some(record.into()),
        }
    }

    /// One round per result: a call, then its result.
    fn rounds(sizes: &[u64]) -> Vec<Message> {
        sizes
            .iter()
            .enumerate()
            .flat_map(|(i, &tokens)| [call(), result(&format!("r{i}"), report(tokens))])
            .collect()
    }

    fn pruned_records(messages: &[Message]) -> Vec<String> {
        messages
            .iter()
            .filter_map(|m| match m {
                Message::ToolResult {
                    content,
                    record: Some(record),
                    ..
                } if content.iter().filter_map(Part::text).any(is_placeholder) => {
                    Some(record.clone())
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn below_the_mark_the_view_is_unchanged() {
        let messages = rounds(&[19_000, 19_000, 19_000]);
        let (out, stats) = prune_for_model(&messages);
        assert_eq!(out, messages);
        assert_eq!(stats, PruneStats::default());
    }

    #[test]
    fn passing_the_mark_prunes_the_oldest_down_to_the_low_mark() {
        let (out, stats) = prune_for_model(&rounds(&[19_000, 19_000, 19_000, 19_000]));
        assert_eq!(pruned_records(&out), ["r0", "r1", "r2"]);
        assert_eq!(stats.pruned, 3);
        assert!(stats.tokens_removed > 56_000 && stats.tokens_removed < 57_100, "{stats:?}");
    }

    #[test]
    fn the_placeholder_keeps_the_call_and_names_the_record() {
        let (out, _) = prune_for_model(&rounds(&[19_000, 19_000, 19_000, 19_000]));
        let Message::ToolResult {
            call_id,
            content,
            record,
        } = &out[1]
        else {
            panic!("expected a result")
        };
        assert_eq!(call_id, "execute_python");
        assert_eq!(record.as_deref(), Some("r0"));
        assert_eq!(
            content,
            &[Part::Text(
                "[output pruned (74.3 KB) - use recall_history(\"r0\") to recall]".into()
            )]
        );
    }

    #[test]
    fn a_pruned_result_stays_pruned_as_the_conversation_grows() {
        let a = rounds(&[19_000, 19_000, 19_000, 19_000]);
        let (pa, _) = prune_for_model(&a);
        let mut longer = a.clone();
        for i in 0..6 {
            longer.push(call());
            longer.push(result(&format!("more{i}"), report(5_000)));
        }
        let (pab, _) = prune_for_model(&longer);
        let before = pruned_records(&pa);
        let after = pruned_records(&pab);
        assert!(before.iter().all(|r| after.contains(r)), "{before:?} ⊄ {after:?}");
    }

    #[test]
    fn the_prefix_is_unchanged_until_the_next_prune() {
        let a = rounds(&[19_000, 19_000, 19_000, 19_000]);
        let (pa, _) = prune_for_model(&a);
        // 19,000 left unpruned; 20,000 more stays under the high mark.
        let mut longer = a.clone();
        longer.extend(rounds(&[10_000, 10_000]).into_iter().map(|m| match m {
            Message::ToolResult { call_id, content, record } => Message::ToolResult {
                call_id,
                content,
                record: record.map(|r| format!("b-{r}")),
            },
            other => other,
        }));
        let (pab, _) = prune_for_model(&longer);
        assert_eq!(&pab[..pa.len()], &pa[..]);
    }

    #[test]
    fn the_newest_round_is_never_pruned() {
        // One huge result in the newest round, past the mark on its own.
        let (out, _) = prune_for_model(&rounds(&[5_000, 70_000]));
        assert_eq!(pruned_records(&out), ["r0"]);
        let (out, _) = prune_for_model(&rounds(&[70_000]));
        assert!(pruned_records(&out).is_empty());
    }

    #[test]
    fn results_of_one_round_share_it() {
        // Two results answering one assistant message: neither is pruned while it is newest.
        let messages = vec![
            call(),
            result("a", report(40_000)),
            result("b", report(40_000)),
        ];
        let (out, _) = prune_for_model(&messages);
        assert!(pruned_records(&out).is_empty());
    }

    #[test]
    fn prunes_are_spaced_by_the_gap_between_the_marks() {
        // After the first prune (at the 4th result) 19,000 are left. The next must not fire
        // until another HIGH − LOW (30,000) has arrived on top of what was left.
        let mut sizes = vec![19_000, 19_000, 19_000, 19_000];
        let first = pruned_records(&prune_for_model(&rounds(&sizes)).0);
        sizes.extend([10_000, 10_000, 10_000]); // 49,000 unpruned: under the mark
        assert_eq!(pruned_records(&prune_for_model(&rounds(&sizes)).0), first);
        sizes.push(12_000); // 61,000: over
        assert!(pruned_records(&prune_for_model(&rounds(&sizes)).0).len() > first.len());
    }

    #[test]
    fn ineligible_results_pass_through_and_are_not_counted() {
        let failed = json!({
            "ok": false, "result": "x".repeat(100_000), "stdout": "", "tool_calls": [],
            "duration_ms": 1,
        })
        .to_string();
        let ended = json!({
            "ok": true, "result": "x".repeat(100_000), "stdout": "", "duration_ms": 1,
            "tool_calls": [{"seq": 1, "name": "ui_render", "ok": true, "ends_turn": true,
                            "duration_ms": 1}],
        })
        .to_string();
        let unrecorded = Message::ToolResult {
            call_id: "x".into(),
            content: vec![Part::Text(report(30_000))],
            record: None,
        };
        let image = Message::ToolResult {
            call_id: "x".into(),
            content: vec![
                Part::Text(report(30_000)),
                Part::Attachment {
                    data: vec![1, 2, 3],
                    mime: "image/png".into(),
                    name: None,
                },
            ],
            record: Some("img".into()),
        };
        let mut messages = vec![
            call(),
            result("failed", failed),
            call(),
            result("ended", ended),
            call(),
            unrecorded,
            call(),
            image,
        ];
        // Small results: 300 of them would pass the mark if they counted.
        for i in 0..300 {
            messages.push(call());
            messages.push(result(&format!("small{i}"), "y".repeat(MIN_BYTES - 1)));
        }
        let (out, stats) = prune_for_model(&messages);
        assert_eq!(out, messages);
        assert_eq!(stats, PruneStats::default());
    }

    #[test]
    fn sizes_read_as_people_write_them() {
        assert_eq!(human_size(812), "812 B");
        assert_eq!(human_size(76_000), "74.2 KB");
        assert_eq!(human_size(200 * 1024), "200 KB");
        assert_eq!(human_size(1_258_291), "1.2 MB");
    }
}
