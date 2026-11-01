//! Telegram's way of presenting a question set (`specs/042-agent-asking-question`,
//! `contracts/channel-controls.md` §3).
//!
//! One message per question, each keyboard carrying the same 8-character set reference: Telegram
//! has no multi-select and no modal, so a `multi_choice` toggles its buttons and submits, and a
//! free-text question is answered by replying to its message (FR-027's text form).

use std::sync::Arc;

use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

use crate::{
    channels::discord::questions::{Pressed, control_id},
    questions::{
        model::{ControlMessage, Controls, QuestionSetRecord, QuestionType},
        surface::{QuestionSurface, Refusal},
    },
    schema::ChannelPlatform,
};

/// The message that asks one question, with its keyboard.
pub fn question_text(set: &QuestionSetRecord, index: usize) -> String {
    let mut out = String::new();
    if let Some(title) = &set.set.title {
        out.push_str(&format!("*{}*\n", title));
    }
    let Some(question) = set.set.questions.get(index) else {
        return out;
    };
    out.push_str(&question.prompt);
    if question.optional {
        out.push_str(" (optional)");
    }
    if let Some(help) = &question.help {
        out.push_str(&format!("\n_{help}_"));
    }
    if question.kind == QuestionType::Text {
        out.push_str("\n\nReply to this message with the answer.");
    }
    out
}

/// The keyboard for one question, or `None` when it is answered by a reply (a free-text question).
pub fn keyboard(set: &QuestionSetRecord, index: usize, chosen: &[usize]) -> Option<InlineKeyboardMarkup> {
    let question = set.set.questions.get(index)?;
    let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();
    match question.kind {
        QuestionType::Choice | QuestionType::Confirm => {
            let labels: Vec<(String, String)> = match question.kind {
                QuestionType::Confirm => vec![
                    (
                        control_id(&set.short_id, index, "yes"),
                        question.yes_label.clone().unwrap_or_else(|| "Yes".into()),
                    ),
                    (
                        control_id(&set.short_id, index, "no"),
                        question.no_label.clone().unwrap_or_else(|| "No".into()),
                    ),
                ],
                _ => question
                    .options
                    .iter()
                    .enumerate()
                    .map(|(option, choice)| {
                        (
                            control_id(&set.short_id, index, &format!("o{option}")),
                            choice.label.clone(),
                        )
                    })
                    .collect(),
            };
            for (id, label) in labels {
                rows.push(vec![InlineKeyboardButton::callback(label, id)]);
            }
        }
        QuestionType::MultiChoice => {
            for (option, choice) in question.options.iter().enumerate() {
                // A tick shows what is already chosen, so a toggle needs no second message.
                let mark = if chosen.contains(&option) { "☑" } else { "☐" };
                rows.push(vec![InlineKeyboardButton::callback(
                    format!("{mark} {}", choice.label),
                    control_id(&set.short_id, index, &format!("o{option}")),
                )]);
            }
            rows.push(vec![InlineKeyboardButton::callback(
                "Submit",
                control_id(&set.short_id, index, "pick"),
            )]);
        }
        QuestionType::Text => return None,
    }
    Some(InlineKeyboardMarkup::new(rows))
}

/// The last row of the last message carries the dismissal, so a set can always be declined.
pub fn with_dismiss(mut keyboard: InlineKeyboardMarkup, set: &QuestionSetRecord) -> InlineKeyboardMarkup {
    let mut rows = keyboard.inline_keyboard;
    rows.push(vec![InlineKeyboardButton::callback(
        set.set
            .dismiss_label
            .clone()
            .unwrap_or_else(|| "Dismiss".into()),
        format!("{}.dismiss", set.short_id),
    )]);
    keyboard.inline_keyboard = rows;
    keyboard
}

/// Telegram's surface: one message per question, edited into its answer when the set closes.
pub struct TelegramQuestionSurface {
    pub bot: Bot,
    pub storage: Arc<crate::storage::VizierStorage>,
}

#[async_trait::async_trait]
impl QuestionSurface for TelegramQuestionSurface {
    fn platform(&self) -> ChannelPlatform {
        ChannelPlatform::Telegram
    }

    async fn present(&self, set: &QuestionSetRecord) -> anyhow::Result<Controls> {
        let chat_id = match &set.session.1 {
            crate::schema::VizierChannelId::TelegramChannel(id) => ChatId(*id),
            _ => anyhow::bail!("this conversation is not a telegram chat"),
        };
        let mut messages = Vec::with_capacity(set.set.questions.len());
        for index in 0..set.set.questions.len() {
            let mut request = self
                .bot
                .send_message(chat_id, question_text(set, index))
                .parse_mode(teloxide::types::ParseMode::MarkdownV2);
            if let Some(keyboard) = keyboard(set, index, &[]) {
                request = request.reply_markup(with_dismiss(keyboard, set));
            }
            let sent = request
                .await
                .map_err(|e| anyhow::anyhow!("telegram rejected a question message: {e}"))?;
            messages.push(ControlMessage {
                chat_id: chat_id.0.to_string(),
                message_id: sent.id.0.to_string(),
                question: Some(index),
            });
        }
        Ok(Controls {
            platform: self.platform().to_string(),
            messages,
        })
    }

    async fn close(&self, set: &QuestionSetRecord, controls: &Controls) -> anyhow::Result<()> {
        for control in &controls.messages {
            let chat_id = ChatId(control.chat_id.parse()?);
            let message_id = teloxide::types::MessageId(control.message_id.parse()?);
            let text = match control.question {
                Some(index) => {
                    let prompt = set
                        .set
                        .questions
                        .get(index)
                        .map(|q| q.prompt.clone())
                        .unwrap_or_default();
                    let answer = set
                        .answers
                        .as_ref()
                        .and_then(|answers| {
                            answers
                                .iter()
                                .find(|a| a.question_id == set.set.questions[index].id)
                        })
                        .and_then(|a| a.label.clone().or_else(|| a.value.as_ref().map(|v| v.display())))
                        .unwrap_or_else(|| "skipped".to_string());
                    format!("{prompt}\n= {answer}")
                }
                None => crate::channels::discord::questions::record_text(set),
            };
            // The keyboard goes with the edit, so nothing is left to press (FR-024).
            let _ = self
                .bot
                .edit_message_text(chat_id, message_id, text)
                .reply_markup(InlineKeyboardMarkup::new(Vec::new()))
                .await;
        }
        Ok(())
    }

    async fn refuse(
        &self,
        set: &QuestionSetRecord,
        _who: &str,
        refusal: Refusal,
    ) -> anyhow::Result<()> {
        // Telegram needs the callback id, which the reader has; the surface only owns the text.
        if let crate::schema::VizierChannelId::TelegramChannel(id) = &set.session.1 {
            let _ = self
                .bot
                .send_message(ChatId(*id), refusal.message())
                .await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::questions::model::*;

    fn set() -> QuestionSetRecord {
        QuestionSetRecord {
            set_id: "s".into(),
            short_id: "7f3a91c2".into(),
            session: crate::schema::VizierSession(
                "a".into(),
                crate::schema::VizierChannelId::TelegramChannel(1),
                None,
            ),
            state: QuestionState::Pending,
            set: QuestionSet {
                title: Some("Pick".into()),
                description: None,
                submit_label: None,
                dismiss_label: None,
                questions: vec![
                    Question {
                        id: "db".into(),
                        prompt: "Database".into(),
                        kind: QuestionType::Choice,
                        help: None,
                        optional: false,
                        default: None,
                        options: vec![ChoiceOption {
                            value: "sqlite".into(),
                            label: "SQLite".into(),
                            description: None,
                        }],
                        placeholder: None,
                        max_length: None,
                        multiline: false,
                        min_selections: None,
                        max_selections: None,
                        yes_label: None,
                        no_label: None,
                    },
                    Question {
                        id: "name".into(),
                        prompt: "Name".into(),
                        kind: QuestionType::Text,
                        help: None,
                        optional: true,
                        default: None,
                        options: vec![],
                        placeholder: None,
                        max_length: None,
                        multiline: false,
                        min_selections: None,
                        max_selections: None,
                        yes_label: None,
                        no_label: None,
                    },
                ],
            },
            answers: None,
            answerer: None,
            controls: None,
            created_at: chrono::Utc::now(),
            closed_at: None,
        }
    }

    /// A choice gets a keyboard; a free-text question is answered by a reply, so it gets none
    /// (FR-027's text form).
    #[test]
    fn only_a_question_a_keyboard_can_express_gets_one() {
        let set = set();
        let choice = keyboard(&set, 0, &[]).expect("a choice gets a keyboard");
        assert_eq!(choice.inline_keyboard.len(), 1);
        assert!(keyboard(&set, 1, &[]).is_none());
    }

    /// Every button carries the same short reference, so a press resolves to the set whatever
    /// message it came from (FR-025).
    #[test]
    fn every_button_carries_the_sets_short_reference() {
        let set = set();
        let keyboard = with_dismiss(keyboard(&set, 0, &[]).unwrap(), &set);
        for row in &keyboard.inline_keyboard {
            for button in row {
                let data = button
                    .callback_data
                    .clone()
                    .expect("a callback button carries data");
                let pressed = Pressed::parse(&data).expect("the id parses");
                assert_eq!(pressed.short_id, set.short_id);
                assert!(
                    data.len() <= crate::questions::limits::MAX_CONTROL_ID_BYTES,
                    "{data} does not fit telegram's callback_data"
                );
            }
        }
    }
}
