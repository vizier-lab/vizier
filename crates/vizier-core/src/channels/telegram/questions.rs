//! Telegram's way of presenting a question set (`specs/042-agent-asking-question`,
//! `contracts/channel-controls.md` §3).
//!
//! One message per question, each keyboard carrying the same 8-character set reference. Telegram
//! has no multi-select and no modal, so a choose-several toggles its buttons until **Done**, and a
//! free-text question is answered by replying to its message. A set Telegram cannot hold that way
//! is posted as the text form instead, losing nothing (FR-027, FR-029).
//!
//! Everything is plain text: no parse mode, so a prompt or an answer can never be read as markup
//! (FR-059).

use std::sync::Arc;

use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, MessageId};

use crate::{
    questions::{
        model::{
            AnswerValue, ControlMessage, Controls, QuestionSetRecord, QuestionState, QuestionType,
        },
        press::{Draft, control_id, set_control_id},
        surface::{QuestionSurface, Refusal},
        text_form,
    },
    schema::ChannelPlatform,
};

/// Telegram's message limit is 4 096; the text form is cut well under it, at line breaks.
const MESSAGE_LIMIT: usize = 3500;
/// More questions than this is a flood of messages; the text form is one.
const MAX_NATIVE_QUESTIONS: usize = 10;
/// More options than this is a keyboard nobody can use.
const MAX_NATIVE_OPTIONS: usize = 25;

/// Whether Telegram can hold the set as one keyboard per question.
pub fn is_native(set: &QuestionSetRecord) -> bool {
    set.set.questions.len() <= MAX_NATIVE_QUESTIONS
        && set
            .set
            .questions
            .iter()
            .all(|q| q.options.len() <= MAX_NATIVE_OPTIONS)
}

/// The message that asks one question. A text question shows the answer once it has one.
pub fn question_text(set: &QuestionSetRecord, index: usize, draft: &Draft) -> String {
    let mut out = String::new();
    let Some(question) = set.set.questions.get(index) else {
        return out;
    };
    if index == 0 {
        if let Some(title) = &set.set.title {
            out.push_str(&format!("{title}\n"));
        }
        if let Some(description) = &set.set.description {
            out.push_str(&format!("{description}\n"));
        }
        if !out.is_empty() {
            out.push('\n');
        }
    }
    let total = set.set.questions.len();
    if total > 1 {
        out.push_str(&format!("{}/{total}. ", index + 1));
    }
    out.push_str(&question.prompt);
    if question.optional {
        out.push_str(" (optional)");
    }
    if let Some(help) = &question.help {
        out.push_str(&format!("\n{help}"));
    }
    if question.kind == QuestionType::Text {
        match draft.answer(&question.id).and_then(|a| a.value.as_ref()) {
            Some(value) => out.push_str(&format!("\n\n\u{2713} {}", value.display())),
            None => out.push_str("\n\nReply to this message with your answer."),
        }
    }
    out
}

fn chosen(draft: &Draft, question_id: &str, value: &str) -> bool {
    match draft.answer(question_id).and_then(|a| a.value.as_ref()) {
        Some(AnswerValue::Text(text)) => text == value,
        Some(AnswerValue::Choices(values)) => values.iter().any(|v| v == value),
        _ => false,
    }
}

/// The keyboard for one question. The last question's keyboard also carries the set's own
/// Submit and Dismiss, so a set can always be declined (FR-014).
pub fn keyboard(set: &QuestionSetRecord, index: usize, draft: &Draft) -> Option<InlineKeyboardMarkup> {
    let question = set.set.questions.get(index)?;
    let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();
    match question.kind {
        QuestionType::Choice => {
            for (option, choice) in question.options.iter().enumerate() {
                let mark = if chosen(draft, &question.id, &choice.value) { "\u{2713} " } else { "" };
                rows.push(vec![InlineKeyboardButton::callback(
                    format!("{mark}{}", choice.label),
                    control_id(&set.short_id, index, &format!("o{option}")),
                )]);
            }
        }
        QuestionType::Confirm => {
            let yes = matches!(
                draft.answer(&question.id).and_then(|a| a.value.as_ref()),
                Some(AnswerValue::Confirm(true))
            );
            let no = matches!(
                draft.answer(&question.id).and_then(|a| a.value.as_ref()),
                Some(AnswerValue::Confirm(false))
            );
            rows.push(vec![
                InlineKeyboardButton::callback(
                    format!(
                        "{}{}",
                        if yes { "\u{2713} " } else { "" },
                        question.yes_label.as_deref().unwrap_or("Yes")
                    ),
                    control_id(&set.short_id, index, "yes"),
                ),
                InlineKeyboardButton::callback(
                    format!(
                        "{}{}",
                        if no { "\u{2713} " } else { "" },
                        question.no_label.as_deref().unwrap_or("No")
                    ),
                    control_id(&set.short_id, index, "no"),
                ),
            ]);
        }
        QuestionType::MultiChoice => {
            let ticked = draft.picks.get(&index);
            for (option, choice) in question.options.iter().enumerate() {
                let on = ticked.is_some_and(|picks| picks.contains(&option));
                rows.push(vec![InlineKeyboardButton::callback(
                    format!("{} {}", if on { "\u{2611}" } else { "\u{2610}" }, choice.label),
                    control_id(&set.short_id, index, &format!("o{option}")),
                )]);
            }
            rows.push(vec![InlineKeyboardButton::callback(
                "Done",
                control_id(&set.short_id, index, "pick"),
            )]);
        }
        QuestionType::Text => {}
    }
    if index + 1 == set.set.questions.len() {
        let mut closing = Vec::new();
        if set.set.questions.len() > 1 {
            closing.push(InlineKeyboardButton::callback(
                set.set.submit_label.clone().unwrap_or_else(|| "Submit".into()),
                set_control_id(&set.short_id, "submit"),
            ));
        }
        closing.push(InlineKeyboardButton::callback(
            set.set.dismiss_label.clone().unwrap_or_else(|| "Dismiss".into()),
            set_control_id(&set.short_id, "dismiss"),
        ));
        rows.push(closing);
    }
    if rows.is_empty() {
        None
    } else {
        Some(InlineKeyboardMarkup::new(rows))
    }
}

/// The text form, cut into messages under Telegram's limit at line breaks.
pub fn text_chunks(set: &QuestionSetRecord) -> Vec<String> {
    let rendered = text_form::render(&set.set, &set.short_id);
    let mut chunks = vec![String::new()];
    for line in rendered.lines() {
        let fits = {
            let last = chunks.last().expect("one chunk exists");
            last.is_empty() || last.chars().count() + line.chars().count() < MESSAGE_LIMIT
        };
        if !fits {
            chunks.push(String::new());
        }
        let last = chunks.last_mut().expect("one chunk exists");
        if !last.is_empty() {
            last.push('\n');
        }
        last.extend(line.chars().take(MESSAGE_LIMIT));
    }
    chunks
}

/// What a closed set's message says.
pub fn record_text(set: &QuestionSetRecord, index: Option<usize>) -> String {
    match set.state {
        QuestionState::Declined => return "Dismissed.".to_string(),
        QuestionState::Withdrawn => return "Withdrawn.".to_string(),
        _ => {}
    }
    let shown = |question: &crate::questions::model::Question| {
        set.answers
            .as_ref()
            .and_then(|answers| answers.iter().find(|a| a.question_id == question.id))
            .and_then(|a| a.label.clone().or_else(|| a.value.as_ref().map(|v| v.display())))
            .unwrap_or_else(|| "skipped".to_string())
    };
    match index {
        Some(index) => set
            .set
            .questions
            .get(index)
            .map(|q| format!("{}\n= {}", q.prompt, shown(q)))
            .unwrap_or_default(),
        None => {
            let mut out = String::new();
            if let Some(title) = &set.set.title {
                out.push_str(&format!("{title}\n"));
            }
            for question in &set.set.questions {
                out.push_str(&format!("\n{}: {}", question.prompt, shown(question)));
            }
            if let Some(who) = &set.answerer {
                out.push_str(&format!("\n\n{who}"));
            }
            out.chars().take(MESSAGE_LIMIT).collect()
        }
    }
}

/// Telegram's surface: one message per question, edited into its answer when the set closes.
pub struct TelegramQuestionSurface {
    pub bot: Bot,
    pub storage: Arc<crate::storage::VizierStorage>,
}

fn chat_of(set: &QuestionSetRecord) -> anyhow::Result<ChatId> {
    match &set.session.1 {
        crate::schema::VizierChannelId::TelegramChannel(id) => Ok(ChatId(*id)),
        _ => anyhow::bail!("this conversation is not a telegram chat"),
    }
}

#[async_trait::async_trait]
impl QuestionSurface for TelegramQuestionSurface {
    fn platform(&self) -> ChannelPlatform {
        ChannelPlatform::Telegram
    }

    async fn present(&self, set: &QuestionSetRecord) -> anyhow::Result<Controls> {
        let chat_id = chat_of(set)?;
        let mut messages = Vec::new();
        if is_native(set) {
            let draft = Draft::default();
            for index in 0..set.set.questions.len() {
                let mut request = self.bot.send_message(chat_id, question_text(set, index, &draft));
                if let Some(keyboard) = keyboard(set, index, &draft) {
                    request = request.reply_markup(keyboard);
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
        } else {
            for chunk in text_chunks(set) {
                let sent = self
                    .bot
                    .send_message(chat_id, chunk)
                    .await
                    .map_err(|e| anyhow::anyhow!("telegram rejected a question message: {e}"))?;
                messages.push(ControlMessage {
                    chat_id: chat_id.0.to_string(),
                    message_id: sent.id.0.to_string(),
                    question: None,
                });
            }
        }
        Ok(Controls {
            platform: self.platform().to_string(),
            messages,
        })
    }

    async fn close(&self, set: &QuestionSetRecord, controls: &Controls) -> anyhow::Result<()> {
        for (position, control) in controls.messages.iter().enumerate() {
            let chat_id = ChatId(control.chat_id.parse()?);
            let message_id = MessageId(control.message_id.parse()?);
            let text = match control.question {
                Some(index) => record_text(set, Some(index)),
                None if position == 0 => record_text(set, None),
                None => "(continued \u{2014} answered above)".to_string(),
            };
            // The keyboard goes with the edit, so nothing is left to press (FR-024).
            self.bot
                .edit_message_text(chat_id, message_id, text)
                .reply_markup(InlineKeyboardMarkup::default())
                .await
                .map_err(|e| anyhow::anyhow!("telegram would not edit a question message: {e}"))?;
        }
        Ok(())
    }

    async fn refuse(
        &self,
        _set: &QuestionSetRecord,
        _who: &str,
        _refusal: Refusal,
    ) -> anyhow::Result<()> {
        // Telegram's refusal is the answer to the callback query, which the reader owns.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::questions::model::{ChoiceOption, Question, QuestionSet};
    use crate::questions::press::{self, Pressed};

    fn question(id: &str, kind: QuestionType, options: usize) -> Question {
        Question {
            id: id.into(),
            prompt: format!("Prompt {id}"),
            kind,
            help: None,
            optional: false,
            default: None,
            options: (0..options)
                .map(|n| ChoiceOption {
                    value: format!("v{n}"),
                    label: format!("Option {n}"),
                    description: None,
                })
                .collect(),
            placeholder: None,
            max_length: None,
            multiline: false,
            min_selections: None,
            max_selections: None,
            yes_label: None,
            no_label: None,
        }
    }

    fn set(questions: Vec<Question>) -> QuestionSetRecord {
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
                questions,
            },
            answers: None,
            answerer: None,
            controls: None,
            created_at: chrono::Utc::now(),
            closed_at: None,
        }
    }

    fn data_of(markup: &InlineKeyboardMarkup) -> Vec<String> {
        markup
            .inline_keyboard
            .iter()
            .flatten()
            .filter_map(|b| match &b.kind {
                teloxide::types::InlineKeyboardButtonKind::CallbackData(d) => Some(d.clone()),
                _ => None,
            })
            .collect()
    }

    /// A choice gets a keyboard; a free-text question is answered by a reply, so it gets none
    /// of its own (FR-027).
    #[test]
    fn only_a_question_a_keyboard_can_express_gets_one() {
        let s = set(vec![
            question("db", QuestionType::Choice, 2),
            question("name", QuestionType::Text, 0),
        ]);
        assert!(keyboard(&s, 0, &Draft::default()).is_some());
        // The last question is free text: only the set's own Submit/Dismiss remain.
        let last = keyboard(&s, 1, &Draft::default()).unwrap();
        let ids = data_of(&last);
        assert!(ids.iter().any(|d| d.ends_with(".submit")) && ids.iter().any(|d| d.ends_with(".dismiss")));
        assert!(!ids.iter().any(|d| d.contains(".q1.")));
    }

    /// Every button carries the same short reference and fits `callback_data` (FR-025).
    #[test]
    fn every_button_carries_the_sets_short_reference_and_fits() {
        let s = set(vec![
            question("a", QuestionType::Choice, 3),
            question("b", QuestionType::MultiChoice, 3),
            question("c", QuestionType::Confirm, 0),
        ]);
        for index in 0..3 {
            for data in data_of(&keyboard(&s, index, &Draft::default()).unwrap()) {
                assert!(data.len() <= crate::questions::limits::MAX_CONTROL_ID_BYTES, "{data}");
                assert_eq!(Pressed::parse(&data).expect(&data).short_id, s.short_id);
            }
        }
    }

    /// A toggle redraws as a tick, and a chosen option as a check mark (the press edits the
    /// keyboard, §3).
    #[test]
    fn the_keyboard_shows_what_is_picked() {
        let s = set(vec![question("f", QuestionType::MultiChoice, 3)]);
        let mut draft = Draft::default();
        press::apply(&s, &mut draft, &Pressed::parse("7f3a91c2.q0.o1").unwrap(), None);
        let markup = keyboard(&s, 0, &draft).unwrap();
        let labels: Vec<String> = markup
            .inline_keyboard
            .iter()
            .flatten()
            .map(|b| b.text.clone())
            .collect();
        assert!(labels.contains(&"\u{2611} Option 1".to_string()), "{labels:?}");
        assert!(labels.contains(&"\u{2610} Option 0".to_string()), "{labels:?}");

        let single = set(vec![question("c", QuestionType::Choice, 2)]);
        let mut draft = Draft::default();
        press::apply(&single, &mut draft, &Pressed::parse("7f3a91c2.q0.o0").unwrap(), None);
        let labels: Vec<String> = keyboard(&single, 0, &draft)
            .unwrap()
            .inline_keyboard
            .iter()
            .flatten()
            .map(|b| b.text.clone())
            .collect();
        assert!(labels.contains(&"\u{2713} Option 0".to_string()), "{labels:?}");
    }

    /// A set that does not fit becomes the text form and every question survives (FR-029).
    #[test]
    fn a_set_that_does_not_fit_becomes_the_text_form() {
        let big = set((0..11).map(|n| question(&format!("q{n}"), QuestionType::Confirm, 0)).collect());
        assert!(!is_native(&big));
        let joined = text_chunks(&big).join("\n");
        for n in 0..11 {
            assert!(joined.contains(&format!("q{n}")), "q{n} was dropped");
        }
        assert!(!is_native(&set(vec![question("x", QuestionType::Choice, 26)])));
        assert!(is_native(&set(vec![question("x", QuestionType::Choice, 25)])));
    }

    /// An answered text question shows its answer, in plain text.
    #[test]
    fn a_text_question_shows_its_answer() {
        let s = set(vec![question("n", QuestionType::Text, 0)]);
        assert!(question_text(&s, 0, &Draft::default()).contains("Reply to this message"));
        let mut draft = Draft::default();
        press::apply_text(&s, &mut draft, 0, "*demo*");
        let text = question_text(&s, 0, &draft);
        assert!(text.contains("\u{2713} *demo*"), "{text}");
    }
}
