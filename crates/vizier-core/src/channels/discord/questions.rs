//! Discord's way of presenting a question set (`specs/042-agent-asking-question`,
//! `contracts/channel-controls.md` §2).
//!
//! Components carry only `<short>.q<index>.<action>`: the set resolves from storage, so a button
//! pressed after a restart still answers, and the id fits Discord's 100-byte `custom_id`. Nothing
//! in a component is a prompt, a label or a value.

use std::sync::Arc;

use twilight_http::Client;
use twilight_model::channel::message::component::{ActionRow, Button, ButtonStyle, Component};
use twilight_model::id::Id;
use twilight_model::id::marker::ChannelMarker;

use crate::{
    questions::{
        model::{ControlMessage, Controls, Question, QuestionSetRecord, QuestionType},
        surface::{QuestionSurface, Refusal},
    },
    schema::ChannelPlatform,
};

/// One control's payload: the set's short reference, the question's **position**, and the action.
pub fn control_id(short_id: &str, index: usize, action: &str) -> String {
    format!("{short_id}.q{index}.{action}")
}

/// What a pressed control asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pressed {
    pub short_id: String,
    pub question: usize,
    pub action: String,
}

impl Pressed {
    pub fn parse(id: &str) -> Option<Self> {
        let mut parts = id.split('.');
        let short_id = parts.next()?.to_string();
        let question = parts.next()?.strip_prefix('q')?.parse().ok()?;
        let action = parts.next()?.to_string();
        Some(Self {
            short_id,
            question,
            action,
        })
    }

    /// The option value this press chose, if it chose one.
    pub fn option_index(&self) -> Option<usize> {
        self.action.strip_prefix('o')?.parse().ok()
    }
}

/// The message body: the title, the description, and each question with its options named.
pub fn body(set: &QuestionSetRecord) -> String {
    let mut out = String::new();
    if let Some(title) = &set.set.title {
        out.push_str(&format!("**{title}**\n"));
    }
    if let Some(description) = &set.set.description {
        out.push_str(&format!("{description}\n"));
    }
    for (index, question) in set.set.questions.iter().enumerate() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&format!("{}. ", index + 1));
        out.push_str(&question.prompt);
        if question.optional {
            out.push_str(" _(optional)_");
        }
        if let Some(help) = &question.help {
            out.push_str(&format!("\n-# {help}"));
        }
        match question.kind {
            QuestionType::Choice | QuestionType::MultiChoice => {
                let names: Vec<&str> = question
                    .options
                    .iter()
                    .map(|option| option.label.as_str())
                    .collect();
                out.push_str(&format!("\n-# {}", names.join(" · ")));
            }
            _ => {}
        }
    }
    if let Some(label) = &set.set.dismiss_label {
        out.push_str(&format!("\n\n-# {label} closes this."));
    }
    out
}

/// One action row per question the platform can express with buttons. A question with more than
/// five options needs a select, which this builds as a second choice when the caller asks.
pub fn rows(set: &QuestionSetRecord) -> Vec<Component> {
    let mut rows: Vec<Component> = Vec::new();
    for (index, question) in set.set.questions.iter().enumerate() {
        if rows.len() >= 5 {
            break;
        }
        let buttons: Vec<Component> = match question.kind {
            QuestionType::Choice if question.options.len() <= 5 => question
                .options
                .iter()
                .enumerate()
                .map(|(option, choice)| {
                    button(
                        &control_id(&set.short_id, index, &format!("o{option}")),
                        &choice.label,
                        ButtonStyle::Secondary,
                    )
                })
                .collect(),
            QuestionType::Confirm => vec![
                button(
                    &control_id(&set.short_id, index, "yes"),
                    question.yes_label.as_deref().unwrap_or("Yes"),
                    ButtonStyle::Success,
                ),
                button(
                    &control_id(&set.short_id, index, "no"),
                    question.no_label.as_deref().unwrap_or("No"),
                    ButtonStyle::Danger,
                ),
            ],
            // Free text and a long option list need the platform's own entry point: a button
            // opens it, and the press is what the reader turns into a modal or a prompt.
            _ => vec![button(
                &control_id(&set.short_id, index, "text"),
                &opening_label(question),
                ButtonStyle::Primary,
            )],
        };
        if !buttons.is_empty() {
            rows.push(Component::ActionRow(ActionRow { components: buttons }));
        }
    }
    // The set as a whole can always be dismissed (FR-014).
    rows.push(Component::ActionRow(ActionRow {
        components: vec![button(
            &format!("{}.dismiss", set.short_id),
            set.set.dismiss_label.as_deref().unwrap_or("Dismiss"),
            ButtonStyle::Secondary,
        )],
    }));
    rows
}

fn opening_label(question: &Question) -> String {
    match question.kind {
        QuestionType::Text => "Answer".to_string(),
        _ => "Choose".to_string(),
    }
}

fn button(custom_id: &str, label: &str, style: ButtonStyle) -> Component {
    Component::Button(Button {
        custom_id: Some(custom_id.to_string()),
        disabled: false,
        emoji: None,
        label: Some(label.to_string()),
        style,
        url: None,
        sku_id: None,
    })
}

/// Discord's surface: posts the set as one message with its components, and edits it into a
/// record of the answers when it closes.
pub struct DiscordQuestionSurface {
    pub http: Arc<Client>,
}

#[async_trait::async_trait]
impl QuestionSurface for DiscordQuestionSurface {
    fn platform(&self) -> ChannelPlatform {
        ChannelPlatform::Discord
    }

    async fn present(&self, set: &QuestionSetRecord) -> anyhow::Result<Controls> {
        use twilight_util::builder::InteractionResponseDataBuilder;

        let channel = match &set.session.1 {
            crate::schema::VizierChannelId::DiscordChanel(id) => {
                Id::<ChannelMarker>::new_checked(*id)
                    .ok_or_else(|| anyhow::anyhow!("not a discord channel id: {id}"))?
            }
            _ => anyhow::bail!("this conversation is not a discord channel"),
        };

        let data = InteractionResponseDataBuilder::new()
            .content(body(set))
            .components(rows(set))
            .build();
        let message = self
            .http
            .create_message(channel)
            .content(&body(set))
            .components(&data.components)
            .await
            .map_err(|e| anyhow::anyhow!("discord rejected the question message: {e:?}"))?
            .model()
            .await
            .map_err(|e| anyhow::anyhow!("discord did not return the posted message: {e:?}"))?;

        Ok(Controls {
            platform: self.platform().to_string(),
            messages: vec![ControlMessage {
                chat_id: channel.to_string(),
                message_id: message.id.to_string(),
                question: None,
            }],
        })
    }

    async fn close(&self, set: &QuestionSetRecord, controls: &Controls) -> anyhow::Result<()> {
        for control in &controls.messages {
            let channel = Id::<ChannelMarker>::new_checked(control.chat_id.parse()?)
                .ok_or_else(|| anyhow::anyhow!("bad channel id"))?;
            let message = Id::new(control.message_id.parse()?);
            // The components go and the message becomes a record of what was answered, so a press
            // on it is impossible rather than merely refused (FR-024).
            self.http
                .update_message(channel, message)
                .content(Some(&record_text(set)))
                .components(Some(&[]))
                .await
                .map_err(|e| anyhow::anyhow!("discord would not edit the question message: {e:?}"))?;
        }
        Ok(())
    }

    async fn refuse(
        &self,
        _set: &QuestionSetRecord,
        _who: &str,
        _refusal: Refusal,
    ) -> anyhow::Result<()> {
        // Discord's refusals are ephemeral interaction responses, which the reader sends because
        // it owns the interaction token; a surface has no way to reach it after the fact.
        Ok(())
    }
}

/// What a closed set's message says: every answer, and every question the person skipped.
pub fn record_text(set: &QuestionSetRecord) -> String {
    let mut out = String::new();
    if let Some(title) = &set.set.title {
        out.push_str(&format!("**{title}**\n"));
    }
    match set.state {
        crate::questions::model::QuestionState::Declined => out.push_str("_Dismissed._"),
        crate::questions::model::QuestionState::Withdrawn => out.push_str("_Withdrawn._"),
        _ => {
            for question in &set.set.questions {
                let answer = set
                    .answers
                    .as_ref()
                    .and_then(|answers| answers.iter().find(|a| a.question_id == question.id));
                let shown = match answer {
                    Some(answer) => answer
                        .label
                        .clone()
                        .or_else(|| answer.value.as_ref().map(|v| v.display()))
                        .unwrap_or_else(|| "skipped".to_string()),
                    None => "skipped".to_string(),
                };
                out.push_str(&format!("\n{}: **{shown}**", question.prompt));
            }
        }
    }
    if let Some(who) = &set.answerer {
        out.push_str(&format!("\n-# {who}"));
    }
    out
}

/// Every control id a set posts, so a press can be resolved without guessing.
pub fn set_short_id(id: &str) -> Option<&str> {
    id.split('.').next()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A control id round-trips, and stays inside Telegram's 64-byte budget too, so one encoding
    /// serves both platforms (contracts/channel-controls.md §1).
    #[test]
    fn a_control_id_round_trips_and_fits_both_platforms() {
        let id = control_id("7f3a91c2", 9, "dismiss");
        assert_eq!(id, "7f3a91c2.q9.dismiss");
        assert!(id.len() <= crate::questions::limits::MAX_CONTROL_ID_BYTES);

        let pressed = Pressed::parse(&id).unwrap();
        assert_eq!(pressed.short_id, "7f3a91c2");
        assert_eq!(pressed.question, 9);
        assert_eq!(pressed.action, "dismiss");
    }

    #[test]
    fn an_option_press_names_its_index() {
        let pressed = Pressed::parse(&control_id("abc", 2, "o13")).unwrap();
        assert_eq!(pressed.option_index(), Some(13));
        assert_eq!(Pressed::parse("garbage"), None);
        assert_eq!(Pressed::parse("abc.qx.o1"), None);
    }
}
