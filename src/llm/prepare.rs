//! The one shared step before every dialect encodes (contract S6, S8): attachments by
//! capability, then system messages normalised for the dialect's style.

use super::dialect::{PreparedRequest, SystemStyle};
use super::resolve::ResolvedModel;
use super::types::{CompletionRequest, Message, Part};

pub fn prepare(req: CompletionRequest, m: &ResolvedModel, style: SystemStyle) -> PreparedRequest {
    let messages = req
        .messages
        .into_iter()
        .map(|message| attachments_by_capability(message, m))
        .collect();
    let (system, messages) = normalize_system(messages, style);
    PreparedRequest {
        system,
        messages,
        tools: req.tools,
        tool_choice: req.tool_choice,
        max_tokens: req.max_tokens,
    }
}

fn attachments_by_capability(message: Message, m: &ResolvedModel) -> Message {
    let fix = |parts: Vec<Part>| -> Vec<Part> {
        parts
            .into_iter()
            .map(|part| match part {
                Part::Attachment { mime, data, name } => {
                    match attachment_note(&mime, name.as_deref(), m) {
                        Some(note) => Part::Text(note),
                        None => Part::Attachment { mime, data, name },
                    }
                }
                text => text,
            })
            .collect()
    };
    match message {
        Message::User(parts) => Message::User(fix(parts)),
        Message::ToolResult { call_id, content } => Message::ToolResult {
            call_id,
            content: fix(content),
        },
        other => other,
    }
}

/// `Some(note)` when the attachment can't go to this model: images unless the model is known
/// not to see them, PDFs only when it is known to read them, audio never (FR-013).
fn attachment_note(mime: &str, name: Option<&str>, m: &ResolvedModel) -> Option<String> {
    let name = name.unwrap_or("file");
    if mime.starts_with("image/") {
        (m.caps.image_input() == Some(false))
            .then(|| format!("[image attached: {name}, but this model cannot view images]"))
    } else if mime.starts_with("audio/") {
        Some(format!(
            "[audio attached: {name}, but audio cannot be sent to this model]"
        ))
    } else if mime == "application/pdf" {
        (m.caps.pdf_input() != Some(true))
            .then(|| format!("[document attached: {name}, but this model cannot read documents]"))
    } else {
        Some(format!(
            "[file attached: {name} ({mime}), but this model cannot read it]"
        ))
    }
}

/// Apply the dialect's [`SystemStyle`]. "Leading" system messages are those before the first
/// non-system message.
pub fn normalize_system(
    messages: Vec<Message>,
    style: SystemStyle,
) -> (Option<String>, Vec<Message>) {
    if style == SystemStyle::Inline {
        return (None, messages);
    }

    let mut leading = Vec::new();
    let mut rest = Vec::with_capacity(messages.len());
    let mut in_lead = true;
    for message in messages {
        match message {
            Message::System(text) if in_lead => leading.push(text),
            Message::System(text) if style == SystemStyle::Field => {
                rest.push(Message::User(vec![Part::Text(format!(
                    "<system>\n{text}\n</system>"
                ))]));
            }
            other => {
                in_lead = false;
                rest.push(other);
            }
        }
    }

    let system = leading
        .into_iter()
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>();
    let system = (!system.is_empty()).then(|| system.join("\n\n"));
    (system, rest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalogue::data::Capabilities;
    use crate::llm::resolve::ResolvedModel;
    use crate::llm::types::AssistantPart;

    fn png() -> Part {
        Part::Attachment {
            mime: "image/png".into(),
            data: vec![1],
            name: Some("cat.png".into()),
        }
    }

    fn model(input: &[&str]) -> ResolvedModel {
        ResolvedModel {
            caps: Capabilities {
                input: input.iter().map(|s| s.to_string()).collect(),
                ..Default::default()
            },
            ..ResolvedModel::test("m")
        }
    }

    fn req(messages: Vec<Message>) -> CompletionRequest {
        CompletionRequest {
            messages,
            ..Default::default()
        }
    }

    #[test]
    fn images_are_kept_unless_the_model_cannot_see_them() {
        let msg = Message::User(vec![Part::Text("look".into()), png()]);

        let kept = prepare(req(vec![msg.clone()]), &model(&[]), SystemStyle::Inline);
        assert_eq!(kept.messages, vec![msg.clone()], "unknown counts as supported");

        let kept = prepare(req(vec![msg.clone()]), &model(&["text", "image"]), SystemStyle::Inline);
        assert_eq!(kept.messages, vec![msg.clone()]);

        let replaced = prepare(req(vec![msg]), &model(&["text"]), SystemStyle::Inline);
        assert_eq!(
            replaced.messages,
            vec![Message::User(vec![
                Part::Text("look".into()),
                Part::Text("[image attached: cat.png, but this model cannot view images]".into())
            ])]
        );
    }

    #[test]
    fn pdfs_need_pdf_input_and_audio_is_always_replaced() {
        let pdf = Part::Attachment {
            mime: "application/pdf".into(),
            data: vec![1],
            name: Some("a.pdf".into()),
        };
        let audio = Part::Attachment {
            mime: "audio/ogg".into(),
            data: vec![1],
            name: Some("v.ogg".into()),
        };
        let msg = Message::User(vec![pdf.clone(), audio]);

        let out = prepare(req(vec![msg.clone()]), &model(&["text", "pdf"]), SystemStyle::Inline);
        let Message::User(parts) = &out.messages[0] else { panic!() };
        assert_eq!(parts[0], pdf);
        assert!(parts[1].text().unwrap().starts_with("[audio attached: v.ogg"));

        let out = prepare(req(vec![msg]), &model(&[]), SystemStyle::Inline);
        let Message::User(parts) = &out.messages[0] else { panic!() };
        assert!(parts[0].text().unwrap().starts_with("[document attached: a.pdf"));
    }

    #[test]
    fn field_style_joins_leading_and_wraps_later_system_messages() {
        let messages = vec![
            Message::system("one"),
            Message::system("two"),
            Message::user_text("hi").unwrap(),
            Message::Assistant(vec![AssistantPart::Text("yo".into())]),
            Message::system("later"),
        ];
        let (system, rest) = normalize_system(messages, SystemStyle::Field);
        assert_eq!(system.as_deref(), Some("one\n\ntwo"));
        assert_eq!(rest.len(), 3);
        assert_eq!(
            rest[2],
            Message::User(vec![Part::Text("<system>\nlater\n</system>".into())])
        );
    }

    #[test]
    fn inline_style_is_unchanged() {
        let messages = vec![Message::system("one"), Message::user_text("hi").unwrap()];
        let (system, rest) = normalize_system(messages.clone(), SystemStyle::Inline);
        assert_eq!(system, None);
        assert_eq!(rest, messages);
    }

    #[test]
    fn instructions_style_keeps_later_system_messages() {
        let messages = vec![
            Message::system("one"),
            Message::user_text("hi").unwrap(),
            Message::system("later"),
        ];
        let (system, rest) = normalize_system(messages, SystemStyle::Instructions);
        assert_eq!(system.as_deref(), Some("one"));
        assert_eq!(rest[1], Message::system("later"));
    }
}
