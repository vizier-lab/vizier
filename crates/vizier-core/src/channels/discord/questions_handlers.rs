//! A Discord reader's side of a question set (`specs/042-agent-asking-question`,
//! `contracts/channel-controls.md` §2): the presses, the text modal, and the reply that answers
//! the text form.
//!
//! What a press *means* is decided by `questions::press`, which Telegram shares; what is here is
//! reading Discord's events and answering them inside its deadline.

use twilight_model::application::interaction::modal::{ModalInteractionComponent, ModalInteractionData};
use twilight_model::channel::message::component::{
    Component, Label, TextInput, TextInputStyle,
};
use twilight_model::channel::message::MessageFlags;
use twilight_model::http::interaction::{
    InteractionResponse, InteractionResponseData, InteractionResponseType,
};
use twilight_model::application::interaction::Interaction;
use twilight_model::channel::Message;

use super::Handler;
use crate::questions::{
    model::{Answer, QuestionAnswer, QuestionSetRecord},
    press::{self, Outcome, Pressed},
    submit::{self, SubmitError},
    surface::Refusal,
    text_form,
};
use crate::schema::VizierChannelId;

/// Every text value a modal submission carried, in field order.
fn modal_values(data: &ModalInteractionData) -> Vec<String> {
    fn collect(component: &ModalInteractionComponent, out: &mut Vec<String>) {
        match component {
            ModalInteractionComponent::TextInput(input) => out.push(input.value.clone()),
            ModalInteractionComponent::ActionRow(row) => {
                row.components.iter().for_each(|c| collect(c, out))
            }
            ModalInteractionComponent::Label(label) => collect(&label.component, out),
            _ => {}
        }
    }
    let mut values = Vec::new();
    data.components.iter().for_each(|c| collect(c, &mut values));
    values
}

/// The option indices a select menu sent back (`o3` → 3). A value that is not one of ours is
/// dropped, so the press is refused rather than guessed at.
fn selected(values: &[String]) -> Vec<usize> {
    values
        .iter()
        .filter_map(|value| value.strip_prefix('o')?.parse().ok())
        .collect()
}

impl Handler {
    fn answerer(interaction: &Interaction) -> String {
        match interaction.author() {
            Some(user) => format!("@{} (DiscordId: {})", user.name, user.id),
            None => "a Discord member".to_string(),
        }
    }

    /// Acknowledge a component interaction inside Discord's deadline, before any work (FR-023).
    async fn acknowledge(&self, interaction: &Interaction) {
        let response = InteractionResponse {
            kind: InteractionResponseType::DeferredUpdateMessage,
            data: None,
        };
        self.q_respond(interaction, &response, "acknowledge").await;
    }

    async fn q_respond(&self, interaction: &Interaction, response: &InteractionResponse, what: &str) {
        if let Err(err) = self
            .http
            .interaction(interaction.application_id)
            .create_response(interaction.id, &interaction.token, response)
            .await
        {
            tracing::debug!("could not {what} a discord interaction: {err}");
        }
    }

    /// A short private notice for a press that changed nothing (FR-024). Sent as a follow-up to
    /// the acknowledgement, so only the person who pressed sees it.
    async fn notice(&self, interaction: &Interaction, message: String) {
        let message: String = message.chars().take(1900).collect();
        if let Err(err) = self
            .http
            .interaction(interaction.application_id)
            .create_followup(&interaction.token)
            .flags(MessageFlags::EPHEMERAL)
            .content(&message)
            .await
        {
            tracing::debug!("could not send a discord notice: {err}");
        }
    }

    /// The set a control's short reference names, if it belongs to this agent and to the channel
    /// the press came from. A press in any other channel reads as a set that is not there
    /// (FR-018).
    async fn set_for_press(&self, interaction: &Interaction, short_id: &str) -> Option<QuestionSetRecord> {
        let record = crate::storage::question::QuestionStorage::question_set_by_short_id(
            &*self.deps.storage,
            short_id,
        )
        .await
        .ok()
        .flatten()?;
        let channel = interaction.channel.as_ref()?.id;
        (record.session.0 == self.agent_id
            && record.session.1 == VizierChannelId::DiscordChanel(channel.get()))
        .then_some(record)
    }

    /// A press on a button or select menu.
    pub(super) async fn question_press(&self, interaction: &Interaction, custom_id: &str, values: &[String]) {
        let Some(pressed) = Pressed::parse(custom_id) else {
            // Not one of ours (another feature's component): acknowledge so Discord does not
            // show a failed interaction, and do nothing else.
            self.acknowledge(interaction).await;
            return;
        };

        // A text question opens a modal, which is itself the interaction's one response.
        if pressed.action == "text" {
            match self.set_for_press(interaction, &pressed.short_id).await {
                Some(record) if record.state.is_open() => {
                    if let Some(index) = pressed.question {
                        self.open_text_modal(interaction, &record, index).await;
                        return;
                    }
                }
                _ => {}
            }
            self.acknowledge(interaction).await;
            self.notice(interaction, Refusal::AlreadyAnswered.message()).await;
            return;
        }

        self.acknowledge(interaction).await;
        let Some(record) = self.set_for_press(interaction, &pressed.short_id).await else {
            self.notice(interaction, SubmitError::NotFound.message()).await;
            return;
        };
        if !record.state.is_open() {
            self.notice(interaction, SubmitError::AlreadyClosed(record.state).message()).await;
            return;
        }

        let picked = selected(values);
        let outcome = self.deps.questions.with_draft(&record.set_id, |draft| {
            press::apply(&record, draft, &pressed, (pressed.action == "select").then_some(&picked[..]))
        });
        self.finish_question(interaction, record, outcome).await;
    }

    /// A submitted text modal.
    pub(super) async fn question_modal(&self, interaction: &Interaction, data: &ModalInteractionData) {
        self.acknowledge(interaction).await;
        let Some(pressed) = Pressed::parse(&data.custom_id) else {
            return;
        };
        let Some(record) = self.set_for_press(interaction, &pressed.short_id).await else {
            self.notice(interaction, SubmitError::NotFound.message()).await;
            return;
        };
        if !record.state.is_open() {
            self.notice(interaction, SubmitError::AlreadyClosed(record.state).message()).await;
            return;
        }
        let (Some(index), Some(text)) = (pressed.question, modal_values(data).into_iter().next()) else {
            return;
        };
        let outcome = self
            .deps
            .questions
            .with_draft(&record.set_id, |draft| press::apply_text(&record, draft, index, &text));
        self.finish_question(interaction, record, outcome).await;
    }

    async fn finish_question(&self, interaction: &Interaction, record: QuestionSetRecord, outcome: Outcome) {
        let who = Self::answerer(interaction);
        match outcome {
            // The set stays open; nothing to redraw, because Discord's own controls already show
            // what was picked.
            Outcome::Updated | Outcome::AskText(_) => {}
            Outcome::Refused(refusal) => self.notice(interaction, refusal.message()).await,
            Outcome::Dismiss => {
                let declined = QuestionAnswer {
                    set_id: record.set_id.clone(),
                    text: String::new(),
                    answers: vec![],
                    answerer: Some(who.clone()),
                    declined: true,
                };
                if let Err(error) = self.submit_answer(&record, declined, &who).await {
                    self.notice(interaction, error.message()).await;
                }
            }
            Outcome::Submit(answers) => {
                let answer = QuestionAnswer {
                    set_id: record.set_id.clone(),
                    text: String::new(),
                    answers,
                    answerer: Some(who.clone()),
                    declined: false,
                };
                if let Err(error) = self.submit_answer(&record, answer, &who).await {
                    self.notice(interaction, error.message()).await;
                }
            }
        }
    }

    /// Open Discord's text entry for one free-text question (FR-022). The prompt rides on the
    /// modal's `Label`; the input's own `label` field is deprecated, so it is left unset.
    #[allow(deprecated)]
    async fn open_text_modal(&self, interaction: &Interaction, record: &QuestionSetRecord, index: usize) {
        let Some(question) = record.set.questions.get(index) else {
            return;
        };
        let input = TextInput {
            id: None,
            custom_id: press::control_id(&record.short_id, index, "form"),
            label: None,
            max_length: Some(
                question
                    .max_length
                    .unwrap_or(crate::questions::limits::MAX_TEXT_ANSWER_BYTES)
                    .min(4000) as u16,
            ),
            min_length: None,
            placeholder: question.placeholder.as_ref().map(|p| p.chars().take(100).collect()),
            required: Some(!question.optional),
            style: if question.multiline { TextInputStyle::Paragraph } else { TextInputStyle::Short },
            value: None,
        };
        let data = InteractionResponseData {
            custom_id: Some(press::control_id(&record.short_id, index, "form")),
            title: Some(
                record
                    .set
                    .title
                    .clone()
                    .unwrap_or_else(|| "Answer".into())
                    .chars()
                    .take(45)
                    .collect(),
            ),
            components: Some(vec![Component::Label(Label {
                id: None,
                label: question.prompt.chars().take(45).collect(),
                description: question.help.as_ref().map(|h| h.chars().take(100).collect()),
                component: Box::new(Component::TextInput(input)),
            })]),
            ..Default::default()
        };
        let response = InteractionResponse {
            kind: InteractionResponseType::Modal,
            data: Some(data),
        };
        self.q_respond(interaction, &response, "open a modal for").await;
    }

    /// Hand the answer to the one submission path, then start the turn it becomes in the channel
    /// the set was asked in.
    async fn submit_answer(
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
        // watcher renders into the channel.
        let responses = self
            .deps
            .transport
            .broadcast_sender(record.session.clone())
            .await
            .map_err(|e| SubmitError::Failed(e.to_string()))?;
        self.deps
            .transport
            .send_request(record.session.clone(), request, Some(responses))
            .await
            .map_err(|e| SubmitError::Failed(e.to_string()))
    }

    /// A reply to a text-form message answers the whole set. `true` when the message was one,
    /// answered or refused, and so is not an ordinary message.
    pub(super) async fn question_reply(&self, msg: &Message) -> bool {
        let Some(referenced) = msg.referenced_message.as_ref() else {
            return false;
        };
        let channel = VizierChannelId::DiscordChanel(msg.channel_id.get());
        let message_id = referenced.id.get().to_string();
        let Ok(open) = crate::storage::question::QuestionStorage::open_question_sets_all(&*self.deps.storage).await
        else {
            return false;
        };
        let Some(record) = open.into_iter().find(|record| {
            record.session.0 == self.agent_id
                && record.session.1 == channel
                && super::questions::rows(record).is_none()
                && record
                    .controls
                    .as_ref()
                    .is_some_and(|c| c.messages.iter().any(|m| m.message_id == message_id))
        }) else {
            return false;
        };

        let who = format!("@{} (DiscordId: {})", msg.author.name, msg.author.id);
        let answers: Result<Vec<Answer>, Vec<String>> = text_form::parse(&record.set, &msg.content);
        let reply = match answers {
            Ok(answers) => {
                let answer = QuestionAnswer {
                    set_id: record.set_id.clone(),
                    text: String::new(),
                    answers,
                    answerer: Some(who.clone()),
                    declined: false,
                };
                match self.submit_answer(&record, answer, &who).await {
                    Ok(()) => return true,
                    Err(error) => error.message(),
                }
            }
            Err(problems) => problems.join("\n"),
        };
        // Refused: say why, in the channel, and keep the set open.
        let text: String = reply.chars().take(1900).collect();
        let _ = self
            .http
            .create_message(msg.channel_id)
            .reply(msg.id)
            .content(&text)
            .allowed_mentions(None)
            .await;
        true
    }
}
