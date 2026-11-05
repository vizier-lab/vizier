//! A short plain-text excerpt of what a notification is about
//! (`specs/051-realtime-notifications` research R7).
//!
//! Built from text that has already been through saving, so anything the secret redactor
//! scrubbed is already `[redacted]` here. Only a reply's own words can appear: never tool
//! arguments, tool results or reasoning.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use crate::schema::{VizierResponse, VizierResponseContent};

/// The excerpt is cut at this many characters, then an ellipsis.
pub const MAX_CHARS: usize = 140;

/// What stands in for a code block's body.
const CODE_MARKER: &str = "Sent some code";

/// Markdown reduced to one line of plain text, or `None` when nothing readable is left.
pub fn from_markdown(markdown: &str) -> Option<String> {
    let mut text = String::new();
    let mut in_code_block = false;
    let mut had_code_block = false;
    for event in Parser::new_ext(markdown, Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH) {
        match event {
            Event::Start(Tag::CodeBlock(_)) => {
                in_code_block = true;
                had_code_block = true;
            }
            Event::End(TagEnd::CodeBlock) => in_code_block = false,
            // A code block's body is noise in a one-line preview.
            Event::Text(_) if in_code_block => {}
            Event::Text(t) | Event::Code(t) => text.push_str(&t),
            Event::SoftBreak | Event::HardBreak | Event::Rule => text.push(' '),
            Event::End(
                TagEnd::Paragraph
                | TagEnd::Heading(_)
                | TagEnd::Item
                | TagEnd::TableCell
                | TagEnd::TableRow
                | TagEnd::BlockQuote(_),
            ) => text.push(' '),
            // Raw HTML and footnote references say nothing readable.
            _ => {}
        }
    }
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return had_code_block.then(|| CODE_MARKER.to_string());
    }
    Some(cut(&collapsed))
}

/// Cut on a character boundary, never inside a multi-byte one.
fn cut(text: &str) -> String {
    if text.chars().count() <= MAX_CHARS {
        return text.to_string();
    }
    let mut out: String = text.chars().take(MAX_CHARS).collect();
    out.truncate(out.trim_end().len());
    out.push('…');
    out
}

pub fn sent_files(count: usize) -> String {
    match count {
        1 => "Sent a file".to_string(),
        n => format!("Sent {n} files"),
    }
}

/// The excerpt of a saved reply: its words, or what it sent instead of words.
pub fn of_response(response: &VizierResponse) -> Option<String> {
    match &response.content {
        VizierResponseContent::Message { content, stats } => from_markdown(content).or_else(|| {
            if stats
                .as_ref()
                .and_then(|s| s.ended_by_tool.as_deref())
                .is_some_and(|tool| tool == "ui_render")
            {
                Some("Sent a page".to_string())
            } else if !response.attachments.is_empty() {
                Some(sent_files(response.attachments.len()))
            } else {
                None
            }
        }),
        VizierResponseContent::AudioReply(_, transcript, _) => transcript
            .as_deref()
            .and_then(from_markdown)
            .or_else(|| Some("Sent a voice reply".to_string())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{VizierAttachment, VizierAttachmentContent, VizierResponseStats};

    fn attachment() -> VizierAttachment {
        VizierAttachment { filename: "a.png".into(), content: VizierAttachmentContent::Local("f1".into()) }
    }

    #[test]
    fn markdown_marks_are_removed_and_whitespace_collapses() {
        assert_eq!(
            from_markdown("# Plan\n\nThe **middle** plan is _underpriced_ by [about 12%](http://x.y).\n\n- one\n- two").as_deref(),
            Some("Plan The middle plan is underpriced by about 12%. one two")
        );
    }

    #[test]
    fn a_code_block_body_is_dropped_and_an_all_code_reply_gets_a_marker() {
        assert_eq!(
            from_markdown("Run this:\n\n```sh\nrm -rf /\n```\nthen wait.").as_deref(),
            Some("Run this: then wait.")
        );
        assert_eq!(from_markdown("```py\nprint(1)\n```").as_deref(), Some("Sent some code"));
    }

    #[test]
    fn inline_code_keeps_its_words_and_a_table_reads_as_cells() {
        assert_eq!(from_markdown("Use `foo()` now").as_deref(), Some("Use foo() now"));
        assert_eq!(
            from_markdown("| a | b |\n|---|---|\n| 1 | 2 |").as_deref(),
            Some("a b 1 2")
        );
    }

    #[test]
    fn nothing_readable_is_none() {
        assert_eq!(from_markdown(""), None);
        assert_eq!(from_markdown("   \n\n  "), None);
        assert_eq!(from_markdown("<div></div>"), None);
    }

    #[test]
    fn a_long_reply_is_cut_at_140_characters_with_an_ellipsis() {
        let long = "word ".repeat(100);
        let out = from_markdown(&long).unwrap();
        assert!(out.chars().count() <= MAX_CHARS + 1, "{} characters", out.chars().count());
        assert!(out.chars().count() >= MAX_CHARS - 5);
        assert!(out.ends_with('…'));
        assert!(!out.trim_end_matches('…').ends_with(' '));
    }

    #[test]
    fn the_cut_never_splits_a_multi_byte_character() {
        let long = "é".repeat(300);
        let out = from_markdown(&long).unwrap();
        assert_eq!(out.chars().count(), MAX_CHARS + 1);
        assert!(out.starts_with('é') && out.ends_with('…'));
        let emoji = "🙂".repeat(200);
        assert_eq!(from_markdown(&emoji).unwrap().chars().count(), MAX_CHARS + 1);
    }

    #[test]
    fn short_text_is_left_alone() {
        assert_eq!(from_markdown("Done.").as_deref(), Some("Done."));
    }

    fn reply(content: VizierResponseContent) -> VizierResponse {
        VizierResponse { content, ..Default::default() }
    }

    fn page_stats() -> VizierResponseStats {
        VizierResponseStats { ended_by_tool: Some("ui_render".into()), ..Default::default() }
    }

    #[test]
    fn a_reply_with_no_words_says_what_it_sent() {
        let page = reply(VizierResponseContent::Message { content: String::new(), stats: Some(page_stats()) });
        assert_eq!(of_response(&page).as_deref(), Some("Sent a page"));

        let mut files = reply(VizierResponseContent::Message { content: String::new(), stats: None });
        files.attachments = vec![attachment(), attachment()];
        assert_eq!(of_response(&files).as_deref(), Some("Sent 2 files"));
        files.attachments.truncate(1);
        assert_eq!(of_response(&files).as_deref(), Some("Sent a file"));

        let voice = reply(VizierResponseContent::AudioReply(attachment(), None, None));
        assert_eq!(of_response(&voice).as_deref(), Some("Sent a voice reply"));
    }

    #[test]
    fn a_reply_with_words_uses_them_and_other_content_has_no_excerpt() {
        let r = reply(VizierResponseContent::Message { content: "**Hello** there".into(), stats: None });
        assert_eq!(of_response(&r).as_deref(), Some("Hello there"));
        let empty = reply(VizierResponseContent::Message { content: String::new(), stats: None });
        assert_eq!(of_response(&empty), None);
        assert_eq!(of_response(&reply(VizierResponseContent::Thinking("secret thoughts".into()))), None);
    }

    #[test]
    fn a_redacted_secret_stays_redacted() {
        let r = reply(VizierResponseContent::Message {
            content: "The key is [redacted], keep it safe.".into(),
            stats: None,
        });
        let out = of_response(&r).unwrap();
        assert!(out.contains("[redacted]"));
    }
}
