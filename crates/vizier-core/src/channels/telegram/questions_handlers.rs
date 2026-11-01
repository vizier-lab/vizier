//! A Telegram reader's side of a question set (`specs/042-agent-asking-question`,
//! `contracts/channel-controls.md` §3): the inline-keyboard presses and the replies that answer a
//! text question or the text form.
//!
//! What a press means is decided by `questions::press`, which Discord shares.

use teloxide::prelude::*;
use teloxide::types::{CallbackQuery, MessageId};

use super::{TelegramChannelReader, questions};
use crate::questions::{
    model::{QuestionAnswer, QuestionSetRecord},
    press::{self, Outcome, Pressed},
    submit::{self, SubmitError},
    text_form,
};
use crate::schema::VizierChannelId;

/// Telegram shows a callback notice of at most 200 characters.
const NOTICE_LIMIT: usize = 200;

impl TelegramChannelReader {
    fn question_answerer(user: &teloxide::types::User) -> String {
        format!("@{} (TelegramUser: {})", user.full_name(), user.id.0)
    }

    /// A press on a question's inline keyboard (FR-026). Always answers the callback, so the
    /// person never sees a spinner that times out (FR-023); a refusal is the answer's text.
    pub(super) async fn handle_question_press(&self, query: CallbackQuery) {
        let notice = self.process_question_press(&query).await;
        let mut answer = self.bot.answer_callback_query(query.id.clone());
        if let Some(text) = notice {
            answer = answer.text(text.chars().take(NOTICE_LIMIT).collect::<String>());
        }
        if let Err(err) = answer.await {
            tracing::debug!("could not answer a telegram callback query: {err}");
        }
    }

    async fn process_question_press(&self, query: &CallbackQuery) -> Option<String> {
        let data = query.data.as_deref()?;
        let pressed = Pressed::parse(data)?;
        let message = query.message.as_ref()?;
        let chat = VizierChannelId::TelegramChannel(message.chat().id.0);

        // A set belongs to the chat it was asked in: a press anywhere else reads as missing
        // (FR-018).
        let record = crate::storage::question::QuestionStorage::question_set_by_short_id(
            &*self.deps.storage,
            &pressed.short_id,
        )
        .await
        .ok()
        .flatten()
        .filter(|record| record.session.0 == self.agent_id && record.session.1 == chat);
        let Some(record) = record else {
            return Some(SubmitError::NotFound.message());
        };
        if !record.state.is_open() {
            return Some(SubmitError::AlreadyClosed(record.state).message());
        }

        let outcome = self
            .deps
            .questions
            .with_draft(&record.set_id, |draft| press::apply(&record, draft, &pressed, None));
        self.finish_question(&record, outcome, &query.from, pressed.question, Some(message.id()))
            .await
    }

    /// Act on what a press or a reply decided. Returns the notice to show, if any.
    async fn finish_question(
        &self,
        record: &QuestionSetRecord,
        outcome: Outcome,
        from: &teloxide::types::User,
        question: Option<usize>,
        message: Option<MessageId>,
    ) -> Option<String> {
        let who = Self::question_answerer(from);
        let answer = |answers, declined| QuestionAnswer {
            set_id: record.set_id.clone(),
            text: String::new(),
            answers,
            answerer: Some(who.clone()),
            declined,
        };
        match outcome {
            Outcome::Refused(refusal) => Some(refusal.message()),
            // Nothing to ask for here: a text question is answered by replying to its message.
            Outcome::AskText(_) => Some("Reply to the question message with your answer.".into()),
            Outcome::Updated => {
                if let (Some(index), Some(message)) = (question, message) {
                    self.redraw_question(record, index, message).await;
                }
                None
            }
            Outcome::Dismiss => self
                .submit_question_answer(record, answer(vec![], true), &who)
                .await
                .err()
                .map(|e| e.message()),
            Outcome::Submit(answers) => self
                .submit_question_answer(record, answer(answers, false), &who)
                .await
                .err()
                .map(|e| e.message()),
        }
    }

    /// Redraw one question's message from the draft: its ticks and its recorded text answer.
    async fn redraw_question(&self, record: &QuestionSetRecord, index: usize, message: MessageId) {
        let Some(chat) = record_chat(record) else { return };
        let draft = self.deps.questions.draft(&record.set_id);
        let text = questions::question_text(record, index, &draft);
        let mut edit = self.bot.edit_message_text(chat, message, text);
        if let Some(keyboard) = questions::keyboard(record, index, &draft) {
            edit = edit.reply_markup(keyboard);
        }
        if let Err(err) = edit.await {
            // "message is not modified" is the normal answer to a press that changed nothing.
            tracing::debug!("could not redraw a telegram question: {err}");
        }
    }

    /// Hand the answer to the one submission path, then start the turn it becomes in the chat the
    /// set was asked in.
    async fn submit_question_answer(
        &self,
        record: &QuestionSetRecord,
        answer: QuestionAnswer,
        who: &str,
    ) -> Result<(), SubmitError> {
        let request = submit::submit(
            &self.deps.storage,
            &self.deps.transport,
            &self.deps.questions,
            &record.session,
            answer,
            who,
        )
        .await?;
        // The reply streams through the session-event broadcast, which this reader's woken-turn
        // watcher renders into the chat.
        let responses = self.deps.transport.broadcast_sender(record.session.clone());
        self.deps
            .transport
            .send_request(record.session.clone(), request, Some(responses))
            .await
            .map_err(|e| SubmitError::Failed(e.to_string()))
    }

    /// A reply to one of a set's messages: the answer to a text question, or the whole text form.
    /// `true` when the message was one, answered or refused, and so is not an ordinary message.
    pub(super) async fn question_reply(&self, msg: &Message) -> bool {
        let (Some(from), Some(replied)) = (msg.from.as_ref().filter(|u| !u.is_bot), msg.reply_to_message()) else {
            return false;
        };
        let chat = VizierChannelId::TelegramChannel(msg.chat.id.0);
        let replied_id = replied.id.0.to_string();
        let Ok(open) = crate::storage::question::QuestionStorage::open_question_sets_all(&*self.deps.storage).await
        else {
            return false;
        };
        let Some((record, control_question)) = open.into_iter().find_map(|record| {
            if record.session.0 != self.agent_id || record.session.1 != chat {
                return None;
            }
            let question = record
                .controls
                .as_ref()?
                .messages
                .iter()
                .find(|m| m.message_id == replied_id)?
                .question;
            Some((record, question))
        }) else {
            return false;
        };
        let text = msg.text().unwrap_or("");

        let notice = match control_question {
            // A question's own message: only a text question is answered by a reply.
            Some(index) => {
                let is_text = record
                    .set
                    .questions
                    .get(index)
                    .is_some_and(|q| q.kind == crate::questions::model::QuestionType::Text);
                if !is_text {
                    return false;
                }
                let outcome = self
                    .deps
                    .questions
                    .with_draft(&record.set_id, |draft| press::apply_text(&record, draft, index, text));
                self.finish_question(&record, outcome, from, Some(index), Some(replied.id))
                    .await
            }
            // The text form: one reply answers the whole set.
            None => match text_form::parse(&record.set, text) {
                Ok(answers) => {
                    let who = Self::question_answerer(from);
                    let answer = QuestionAnswer {
                        set_id: record.set_id.clone(),
                        text: String::new(),
                        answers,
                        answerer: Some(who.clone()),
                        declined: false,
                    };
                    self.submit_question_answer(&record, answer, &who)
                        .await
                        .err()
                        .map(|e| e.message())
                }
                Err(problems) => Some(problems.join("\n")),
            },
        };
        if let Some(notice) = notice {
            let body: String = notice.chars().take(3500).collect();
            let _ = self.bot.send_message(msg.chat.id, body).reply_parameters(
                teloxide::types::ReplyParameters::new(msg.id),
            ).await;
        }
        true
    }
}

fn record_chat(record: &QuestionSetRecord) -> Option<ChatId> {
    match &record.session.1 {
        VizierChannelId::TelegramChannel(id) => Some(ChatId(*id)),
        _ => None,
    }
}
