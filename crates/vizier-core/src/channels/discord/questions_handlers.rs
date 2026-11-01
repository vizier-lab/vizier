
/// Every text value a modal submission carried, in field order.
fn modal_values(components: &[twilight_model::channel::message::component::Component]) -> Vec<String> {
    use twilight_model::channel::message::component::Component;
    let mut values = Vec::new();
    for row in components {
        let Component::ActionRow(row) = row else { continue };
        for component in &row.components {
            if let Component::TextInput(input) = component
                && let Some(value) = &input.value
            {
                values.push(value.clone());
            }
        }
    }
    values
}

impl Handler {
    /// Acknowledge a component or modal interaction inside Discord's deadline, before any work.
    async fn acknowledge(&self, interaction: &Interaction) {
        let response = InteractionResponse {
            kind: InteractionResponseType::DeferredUpdateMessage,
            data: None,
        };
        if let Err(err) = self
            .http
            .interaction(interaction.application_id)
            .create_response(interaction.id, &interaction.token, &response)
            .await
        {
            tracing::debug!("could not acknowledge a discord interaction: {err:?}");
        }
    }

    /// One press on a question's control: resolve the set from its short reference, work out what
    /// the press means, and submit it through the one path every surface uses (FR-026).
    async fn question_press(&self, interaction: &Interaction, custom_id: &str) {
        use crate::questions::{model::QuestionType, submit, surface::Refusal};

        let Ok(pressed) = questions::Pressed::parse(custom_id).ok_or(()) else {
            return;
        };
        let Some(mut record) = self.set_by_short(&pressed.short_id).await else {
            return;
        };
        if !record.state.is_open() {
            self.notice(interaction, Refusal::AlreadyAnswered.message()).await;
            return;
        }
        // A previous answer to this question, so a select or a toggle can build on it.
        let mut answer = crate::questions::model::QuestionAnswer {
            set_id: record.set_id.clone(),
            text: String::new(),
            answers: Vec::new(),
            answerer: interaction
                .author()
                .map(|user| format!("@{} (DiscordId: {})", user.name, user.id)),
            declined: false,
        };
        answer.answers = record.answers.clone().unwrap_or_default();

        if pressed.action == "dismiss" {
            answer.declined = true;
            self.submit_answer(record, answer, interaction).await;
            return;
        }
        let Some(question) = record.set.questions.get(pressed.question) else {
            return;
        };
        let value = match question.kind {
            QuestionType::Confirm => match pressed.action.as_str() {
                "yes" => Some((crate::questions::model::AnswerValue::Confirm(true), None)),
                "no" => Some((crate::questions::model::AnswerValue::Confirm(false), None)),
                _ => None,
            },
            QuestionType::Choice | QuestionType::MultiChoice => {
                let option = pressed
                    .option_index()
                    .and_then(|index| question.options.get(index));
                match (question.kind, option) {
                    (QuestionType::Choice, Some(option)) => Some((
                        crate::questions::model::AnswerValue::Text(option.value.clone()),
                        Some(option.label.clone()),
                    )),
                    // A choose-several toggles the option and waits for `pick`, so one press
                    // never submits a half-made choice (FR-022).
                    (QuestionType::MultiChoice, Some(option)) => {
                        let mut chosen: Vec<String> = answer
                            .answers
                            .iter()
                            .find(|a| a.question_id == question.id)
                            .and_then(|a| match &a.value {
                                Some(crate::questions::model::AnswerValue::Choices(values)) => {
                                    Some(values.clone())
                                }
                                _ => None,
                            })
                            .unwrap_or_default();
                        if let Some(at) = chosen.iter().position(|v| *v == option.value) {
                            chosen.remove(at);
                        } else {
                            chosen.push(option.value.clone());
                        }
                        record.answers = Some(vec![crate::questions::model::Answer {
                            question_id: question.id.clone(),
                            state: crate::questions::model::AnswerState::Answered,
                            value: Some(crate::questions::model::AnswerValue::Choices(chosen)),
                            label: None,
                        }]);
                        // The toggle is recorded for the next press and the set stays open.
                        let _ = crate::storage::question::QuestionStorage::claim_question_set(
                            &*self.deps.storage,
                            &record.set_id,
                            &record.session,
                            &[],
                            "",
                        )
                        .await;
                        return;
                    }
                    _ => None,
                }
            }
            QuestionType::Text => {
                // A free-text question is answered through the platform's own text entry: the
                // press opens a modal, which submits through `question_modal`.
                self.open_text_modal(interaction, &record, pressed.question)
                    .await;
                return;
            }
        };
        let Some((value, label)) = value else { return };
        let answer_value = crate::questions::model::Answer {
            question_id: question.id.clone(),
            state: crate::questions::model::AnswerState::Answered,
            value: Some(value),
            label,
        };
        let mut answers: Vec<crate::questions::model::Answer> = record
            .answers
            .clone()
            .unwrap_or_default()
            .into_iter()
            .filter(|a| a.question_id != question.id)
            .collect();
        answers.push(answer_value);
        // Every required question answered means the whole set submits on this press.
        let complete = record
            .set
            .questions
            .iter()
            .filter(|q| !q.optional && q.default.is_none())
            .all(|q| answers.iter().any(|a| a.question_id == q.id));
        answer.answers = answers;
        if complete || question.kind == QuestionType::Choice {
            self.submit_answer(record, answer, interaction).await;
        } else {
            record.answers = Some(answer.answers);
            let _ = submit::record_set(
                &self.deps.storage,
                &self.deps.transport,
                &record.session,
                &record,
            )
            .await;
        }
    }

    /// A modal's value for one free-text question.
    async fn question_modal(&self, interaction: &Interaction, custom_id: &str, values: Vec<String>) {
        let Ok(pressed) = questions::Pressed::parse(custom_id).ok_or(()) else {
            return;
        };
        let Some(record) = self.set_by_short(&pressed.short_id).await else {
            return;
        };
        let Some(question) = record.set.questions.get(pressed.question) else {
            return;
        };
        let Some(text) = values.first() else { return };
        let answer = crate::questions::model::QuestionAnswer {
            set_id: record.set_id.clone(),
            text: String::new(),
            answers: vec![crate::questions::model::Answer {
                question_id: question.id.clone(),
                state: crate::questions::model::AnswerState::Answered,
                value: Some(crate::questions::model::AnswerValue::Text(text.clone())),
                label: None,
            }],
            answerer: interaction
                .author()
                .map(|user| format!("@{} (DiscordId: {})", user.name, user.id)),
            declined: false,
        };
        self.submit_answer(record, answer, interaction).await;
    }

    /// Open the platform's text entry for a free-text question (FR-022).
    async fn open_text_modal(
        &self,
        interaction: &Interaction,
        record: &crate::questions::model::QuestionSetRecord,
        index: usize,
    ) {
        use twilight_util::builder::InteractionResponseDataBuilder;

        let Some(question) = record.set.questions.get(index) else {
            return;
        };
        let input = twilight_model::channel::message::component::TextInputBuilder::new(
            questions::control_id(&record.short_id, index, "text"),
            twilight_model::channel::message::component::TextInputStyle::Short,
        )
        .label(&question.prompt)
        .required(!question.optional)
        .build();
        let data = InteractionResponseDataBuilder::new()
            .custom_id(questions::control_id(&record.short_id, index, "form"))
            .title(record.set.title.clone().unwrap_or_else(|| "Answer".into()))
            .components([twilight_model::channel::message::component::Component::ActionRow(
                twilight_model::channel::message::component::ActionRow {
                    components: vec![
                        twilight_model::channel::message::component::Component::TextInput(input),
                    ],
                },
            )])
            .build();
        let response = InteractionResponse {
            kind: InteractionResponseType::Modal,
            data: Some(data),
        };
        // A modal is the acknowledgement itself: it replaces the deferred update.
        if let Err(err) = self
            .http
            .interaction(interaction.application_id)
            .create_response(interaction.id, &interaction.token, &response)
            .await
        {
            tracing::debug!("could not open a discord modal: {err:?}");
        }
    }

    /// Hand the answer to the one submission path, then start the turn it becomes.
    async fn submit_answer(
        &self,
        record: crate::questions::model::QuestionSetRecord,
        answer: crate::questions::model::QuestionAnswer,
        interaction: &Interaction,
    ) {
        let who = answer
            .answerer
            .clone()
            .unwrap_or_else(|| self.agent_id.clone());
        match crate::questions::submit::submit(
            &self.deps.storage,
            &self.deps.transport,
            &self.deps.questions,
            &record.session,
            answer,
            &who,
        )
        .await
        {
            Ok(request) => {
                let _ = self
                    .deps
                    .transport
                    .send_request(record.session.clone(), request, None)
                    .await;
            }
            Err(error) => {
                self.notice(interaction, error.message()).await;
            }
        }
    }

    /// A short, private notice for a press that changed nothing (FR-024).
    async fn notice(&self, interaction: &Interaction, message: String) {
        use twilight_util::builder::InteractionResponseDataBuilder;

        let response = InteractionResponse {
            kind: InteractionResponseType::ChannelMessageWithSource,
            data: Some(
                InteractionResponseDataBuilder::new()
                    .content(message)
                    .flags(MessageFlags::EPHEMERAL)
                    .build(),
            ),
        };
        if let Err(err) = self
            .http
            .interaction(interaction.application_id)
            .create_response(interaction.id, &interaction.token, &response)
            .await
        {
            tracing::debug!("could not send a discord notice: {err:?}");
        }
    }

    /// The set a control's short reference names (FR-025).
    async fn set_by_short(
        &self,
        short_id: &str,
    ) -> Option<crate::questions::model::QuestionSetRecord> {
        crate::storage::question::QuestionStorage::question_set_by_short_id(
            &*self.deps.storage,
            short_id,
        )
        .await
        .ok()
        .flatten()
        .filter(|record| record.session.0 == self.agent_id)
    }
}
