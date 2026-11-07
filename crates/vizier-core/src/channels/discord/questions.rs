//! Discord's way of presenting a question set (`specs/042-agent-asking-question`,
//! `contracts/channel-controls.md` §2).
//!
//! Components carry only `<short>.q<index>.<action>`: the set resolves from storage, so a button
//! pressed after a restart still answers, and the id fits Discord's 100-byte `custom_id`. Nothing
//! in a component is a prompt, a label or a value.

use std::sync::Arc;

use twilight_http::Client;
use twilight_model::channel::message::AllowedMentions;
use twilight_model::channel::message::component::{
    ActionRow, Button, ButtonStyle, Component, SelectMenu, SelectMenuOption, SelectMenuType,
};
use twilight_model::id::Id;
use twilight_model::id::marker::{ChannelMarker, MessageMarker};

use crate::{
    questions::{
        model::{ControlMessage, Controls, QuestionSetRecord, QuestionState, QuestionType},
        press::{control_id, set_control_id},
        surface::{QuestionSurface, Refusal},
        text_form,
    },
    schema::ChannelPlatform,
};

/// Discord's message limit, with room left for the edit that turns a set into its record.
const MESSAGE_LIMIT: usize = 1900;
/// A message holds five action rows; the set's own Submit/Dismiss row takes one.
const QUESTION_ROWS: usize = 4;
/// A select menu holds 25 options.
const SELECT_OPTIONS: usize = 25;
/// A row of buttons holds five.
const BUTTONS_PER_ROW: usize = 5;

/// Escape what Discord would otherwise render as markup, so an answer or a prompt reads as the
/// text it is (FR-059). Mentions are separately switched off on every message this posts.
pub fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '\\' | '*' | '_' | '~' | '`' | '|' | '>' | '#' | '[' | ']' | '@') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// The message body: the title, the description, and each question with its options named.
pub fn body(set: &QuestionSetRecord) -> String {
    let mut out = String::new();
    if let Some(title) = &set.set.title {
        out.push_str(&format!("**{}**\n", plain(title)));
    }
    if let Some(description) = &set.set.description {
        out.push_str(&format!("{}\n", plain(description)));
    }
    for (index, question) in set.set.questions.iter().enumerate() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&format!("{}. {}", index + 1, plain(&question.prompt)));
        if question.optional {
            out.push_str(" _(optional)_");
        }
        if let Some(help) = &question.help {
            out.push_str(&format!("\n-# {}", plain(help)));
        }
        if matches!(question.kind, QuestionType::Choice | QuestionType::MultiChoice)
            && question.options.len() > BUTTONS_PER_ROW
        {
            // A select menu hides its options until opened; buttons already name them.
            let names: Vec<String> = question.options.iter().map(|o| plain(&o.label)).collect();
            out.push_str(&format!("\n-# {}", names.join(" · ")));
        }
    }
    out
}

/// The controls for a set, or `None` when Discord cannot hold it natively and it must be posted
/// as the text form instead: too many questions for one message, an option list over 25, or a
/// body over the message limit. Falling back loses nothing (FR-027, FR-029).
pub fn rows(set: &QuestionSetRecord) -> Option<Vec<Component>> {
    if set.set.questions.len() > QUESTION_ROWS || body(set).chars().count() > MESSAGE_LIMIT {
        return None;
    }
    let mut rows: Vec<Component> = Vec::new();
    for (index, question) in set.set.questions.iter().enumerate() {
        let row = match question.kind {
            QuestionType::Choice if question.options.len() <= BUTTONS_PER_ROW => {
                buttons(
                    question.options.iter().enumerate().map(|(option, choice)| {
                        (
                            control_id(&set.short_id, index, &format!("o{option}")),
                            choice.label.clone(),
                            ButtonStyle::Secondary,
                        )
                    }),
                )
            }
            QuestionType::Choice | QuestionType::MultiChoice => {
                if question.options.len() > SELECT_OPTIONS {
                    return None;
                }
                let multi = question.kind == QuestionType::MultiChoice;
                let max = if multi {
                    question
                        .max_selections
                        .unwrap_or(question.options.len())
                        .min(question.options.len())
                } else {
                    1
                };
                Component::ActionRow(ActionRow {
                    id: None,
                    components: vec![Component::SelectMenu(SelectMenu {
                        id: None,
                        channel_types: None,
                        custom_id: control_id(&set.short_id, index, "select"),
                        default_values: None,
                        disabled: false,
                        kind: SelectMenuType::Text,
                        max_values: Some(max.clamp(1, SELECT_OPTIONS) as u8),
                        min_values: Some(0),
                        options: Some(
                            question
                                .options
                                .iter()
                                .enumerate()
                                .map(|(option, choice)| SelectMenuOption {
                                    default: false,
                                    description: None,
                                    emoji: None,
                                    label: truncate(&choice.label, 100),
                                    value: format!("o{option}"),
                                })
                                .collect(),
                        ),
                        placeholder: Some(truncate(&question.prompt, 150)),
                        required: None,
                    })],
                })
            }
            QuestionType::Confirm => buttons([
                (
                    control_id(&set.short_id, index, "yes"),
                    question.yes_label.clone().unwrap_or_else(|| "Yes".into()),
                    ButtonStyle::Success,
                ),
                (
                    control_id(&set.short_id, index, "no"),
                    question.no_label.clone().unwrap_or_else(|| "No".into()),
                    ButtonStyle::Danger,
                ),
            ]),
            // Free text opens Discord's own entry form.
            QuestionType::Text => buttons([(
                control_id(&set.short_id, index, "text"),
                truncate(&format!("Answer {}", index + 1), 80),
                ButtonStyle::Primary,
            )]),
        };
        rows.push(row);
    }
    // The set as a whole can always be dismissed (FR-014); a set of several questions is
    // submitted when the person says so, not only when the last one is answered.
    let mut closing = Vec::new();
    if set.set.questions.len() > 1 {
        closing.push((
            set_control_id(&set.short_id, "submit"),
            set.set.submit_label.clone().unwrap_or_else(|| "Submit".into()),
            ButtonStyle::Primary,
        ));
    }
    closing.push((
        set_control_id(&set.short_id, "dismiss"),
        set.set.dismiss_label.clone().unwrap_or_else(|| "Dismiss".into()),
        ButtonStyle::Secondary,
    ));
    rows.push(buttons(closing));
    Some(rows)
}

fn truncate(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

fn buttons(items: impl IntoIterator<Item = (String, String, ButtonStyle)>) -> Component {
    Component::ActionRow(ActionRow {
        id: None,
        components: items
            .into_iter()
            .map(|(id, label, style)| {
                Component::Button(Button {
                    id: None,
                    custom_id: Some(id),
                    disabled: false,
                    emoji: None,
                    label: Some(truncate(&label, 80)),
                    style,
                    url: None,
                    sku_id: None,
                })
            })
            .collect(),
    })
}

/// The text form, cut into messages under Discord's limit at line breaks.
pub fn text_chunks(set: &QuestionSetRecord) -> Vec<String> {
    let rendered = text_form::render(&set.set, &set.short_id);
    let mut chunks = vec![String::new()];
    for line in rendered.lines() {
        let last = chunks.last_mut().expect("one chunk exists");
        if !last.is_empty() && last.chars().count() + line.chars().count() + 1 > MESSAGE_LIMIT {
            chunks.push(String::new());
        }
        let last = chunks.last_mut().expect("one chunk exists");
        if !last.is_empty() {
            last.push('\n');
        }
        // A single line over the limit is cut: no valid line is this long, because every
        // prompt and label is limited well below it.
        last.push_str(&truncate(line, MESSAGE_LIMIT));
    }
    chunks
}

/// No mention in anything this posts can ping anybody.
fn no_mentions() -> AllowedMentions {
    AllowedMentions::default()
}

/// Discord's surface: posts the set as one message with its components (or as the text form when
/// it cannot be held natively), and edits it into a record of the answers when it closes.
pub struct DiscordQuestionSurface {
    pub http: Arc<Client>,
}

fn channel_of(set: &QuestionSetRecord) -> anyhow::Result<Id<ChannelMarker>> {
    match &set.session.1 {
        crate::schema::VizierChannelId::DiscordChanel(id) => Id::<ChannelMarker>::new_checked(*id)
            .ok_or_else(|| anyhow::anyhow!("not a discord channel id: {id}")),
        _ => anyhow::bail!("this conversation is not a discord channel"),
    }
}

#[async_trait::async_trait]
impl QuestionSurface for DiscordQuestionSurface {
    fn platform(&self) -> ChannelPlatform {
        ChannelPlatform::Discord
    }

    async fn present(&self, set: &QuestionSetRecord) -> anyhow::Result<Controls> {
        let channel = channel_of(set)?;
        let mentions = no_mentions();
        let mut messages = Vec::new();

        match rows(set) {
            Some(components) => {
                let content = body(set);
                let posted = self
                    .http
                    .create_message(channel)
                    .content(&content)
                    .components(&components)
                    .allowed_mentions(Some(&mentions))
                    .await
                    .map_err(|e| anyhow::anyhow!("discord rejected the question message: {e}"))?
                    .model()
                    .await
                    .map_err(|e| anyhow::anyhow!("discord did not return the posted message: {e}"))?;
                messages.push(ControlMessage {
                    chat_id: channel.to_string(),
                    message_id: posted.id.to_string(),
                    question: None,
                });
            }
            None => {
                for chunk in text_chunks(set) {
                    let posted = self
                        .http
                        .create_message(channel)
                        .content(&chunk)
                        .allowed_mentions(Some(&mentions))
                        .await
                        .map_err(|e| anyhow::anyhow!("discord rejected the question message: {e}"))?
                        .model()
                        .await
                        .map_err(|e| {
                            anyhow::anyhow!("discord did not return the posted message: {e}")
                        })?;
                    messages.push(ControlMessage {
                        chat_id: channel.to_string(),
                        message_id: posted.id.to_string(),
                        question: None,
                    });
                }
            }
        }

        Ok(Controls {
            platform: self.platform().to_string(),
            messages,
        })
    }

    async fn close(&self, set: &QuestionSetRecord, controls: &Controls) -> anyhow::Result<()> {
        let mentions = no_mentions();
        // The first message becomes the record; the others of a long text form are removed
        // components-wise and say they were answered. A press on any is impossible, not merely
        // refused (FR-024).
        for (position, control) in controls.messages.iter().enumerate() {
            let channel = Id::<ChannelMarker>::new_checked(control.chat_id.parse()?)
                .ok_or_else(|| anyhow::anyhow!("bad channel id"))?;
            let message = Id::<MessageMarker>::new_checked(control.message_id.parse()?)
                .ok_or_else(|| anyhow::anyhow!("bad message id"))?;
            let text = if position == 0 {
                record_text(set)
            } else {
                "-# (continued — answered above)".to_string()
            };
            self.http
                .update_message(channel, message)
                .content(Some(&text))
                .components(Some(&[]))
                .allowed_mentions(Some(&mentions))
                .await
                .map_err(|e| anyhow::anyhow!("discord would not edit the question message: {e}"))?;
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
        out.push_str(&format!("**{}**\n", plain(title)));
    }
    match set.state {
        QuestionState::Declined => out.push_str("_Dismissed._"),
        QuestionState::Withdrawn => out.push_str("_Withdrawn._"),
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
                        .map(|text| plain(&text))
                        .unwrap_or_else(|| "skipped".to_string()),
                    None => "skipped".to_string(),
                };
                out.push_str(&format!("\n{}: **{shown}**", plain(&question.prompt)));
            }
        }
    }
    if let Some(who) = &set.answerer {
        out.push_str(&format!("\n-# {}", plain(who)));
    }
    truncate(&out, 2000)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::questions::model::{ChoiceOption, Question, QuestionSet};
    use crate::schema::{VizierChannelId, VizierSession};

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

    fn record(questions: Vec<Question>) -> QuestionSetRecord {
        QuestionSetRecord {
            set_id: "s".into(),
            short_id: "7f3a91c2".into(),
            session: VizierSession("a".into(), VizierChannelId::DiscordChanel(1), None),
            state: QuestionState::Pending,
            set: QuestionSet {
                title: Some("Setup".into()),
                description: None,
                questions,
                submit_label: None,
                dismiss_label: None,
            },
            answers: None,
            answerer: None,
            controls: None,
            created_at: crate::time::now(),
            closed_at: None,
        }
    }

    fn custom_ids(components: &[Component]) -> Vec<String> {
        let mut ids = Vec::new();
        for row in components {
            let Component::ActionRow(row) = row else { continue };
            for c in &row.components {
                match c {
                    Component::Button(b) => ids.extend(b.custom_id.clone()),
                    Component::SelectMenu(m) => ids.push(m.custom_id.clone()),
                    _ => {}
                }
            }
        }
        ids
    }

    /// Every control id parses, carries the set's reference, and fits both platforms (FR-025).
    #[test]
    fn every_control_resolves_to_the_set_and_fits() {
        let set = record(vec![
            question("a", QuestionType::Choice, 3),
            question("b", QuestionType::Choice, 9),
            question("c", QuestionType::MultiChoice, 4),
            question("d", QuestionType::Confirm, 0),
        ]);
        let components = rows(&set).expect("four questions fit natively");
        let ids = custom_ids(&components);
        assert!(ids.len() > 8);
        for id in ids {
            assert!(id.len() <= crate::questions::limits::MAX_CONTROL_ID_BYTES, "{id}");
            let pressed = crate::questions::press::Pressed::parse(&id).expect(&id);
            assert_eq!(pressed.short_id, set.short_id);
        }
    }

    /// Up to five options are buttons; more is a select menu; a choose-several is a select with a
    /// maximum.
    #[test]
    fn the_control_follows_the_question() {
        let set = record(vec![
            question("a", QuestionType::Choice, 5),
            question("b", QuestionType::Choice, 6),
        ]);
        let components = rows(&set).unwrap();
        let Component::ActionRow(first) = &components[0] else { panic!() };
        assert!(first.components.iter().all(|c| matches!(c, Component::Button(_))));
        assert_eq!(first.components.len(), 5);
        let Component::ActionRow(second) = &components[1] else { panic!() };
        assert!(matches!(second.components[0], Component::SelectMenu(_)));

        let mut multi = question("m", QuestionType::MultiChoice, 6);
        multi.max_selections = Some(2);
        let components = rows(&record(vec![multi])).unwrap();
        let Component::ActionRow(row) = &components[0] else { panic!() };
        let Component::SelectMenu(menu) = &row.components[0] else { panic!() };
        assert_eq!(menu.max_values, Some(2));
        assert_eq!(menu.min_values, Some(0));
    }

    /// The set can always be dismissed, and a set of several questions can be submitted.
    #[test]
    fn a_set_can_always_be_dismissed_and_a_larger_one_submitted() {
        let one = custom_ids(&rows(&record(vec![question("a", QuestionType::Confirm, 0)])).unwrap());
        assert!(one.iter().any(|id| id.ends_with(".dismiss")));
        assert!(!one.iter().any(|id| id.ends_with(".submit")));

        let two = custom_ids(
            &rows(&record(vec![
                question("a", QuestionType::Confirm, 0),
                question("b", QuestionType::Text, 0),
            ]))
            .unwrap(),
        );
        assert!(two.iter().any(|id| id.ends_with(".submit")));
        assert!(two.iter().any(|id| id.ends_with(".dismiss")));
    }

    /// A set Discord cannot hold falls back to the text form and loses no question (FR-027,
    /// FR-029): more questions than rows, an option list over 25, or a body over the limit.
    #[test]
    fn a_set_that_does_not_fit_falls_back_to_the_text_form() {
        let many = record((0..5).map(|n| question(&format!("q{n}"), QuestionType::Confirm, 0)).collect());
        assert!(rows(&many).is_none());
        let chunks = text_chunks(&many);
        let joined = chunks.join("\n");
        for n in 0..5 {
            assert!(joined.contains(&format!("q{n}")), "q{n} was dropped");
        }

        assert!(rows(&record(vec![question("x", QuestionType::Choice, 26)])).is_none());

        let mut long = question("l", QuestionType::Text, 0);
        long.prompt = "x".repeat(2500);
        assert!(rows(&record(vec![long])).is_none());
        assert!(text_chunks(&record(vec![question("y", QuestionType::Choice, 26)]))
            .iter()
            .all(|c| c.chars().count() <= MESSAGE_LIMIT));
    }

    /// Answers are shown as text: markup is escaped and a mention cannot form (FR-059).
    #[test]
    fn a_closed_record_shows_answers_as_plain_text() {
        let mut set = record(vec![question("a", QuestionType::Text, 0)]);
        set.state = QuestionState::Answered;
        set.answerer = Some("@everyone".into());
        set.answers = Some(vec![crate::questions::model::Answer {
            question_id: "a".into(),
            state: crate::questions::model::AnswerState::Answered,
            value: Some(crate::questions::model::AnswerValue::Text("**bold** @everyone `x`".into())),
            label: None,
        }]);
        let text = record_text(&set);
        assert!(text.contains("\\*\\*bold\\*\\*"), "{text}");
        assert!(text.contains("\\@everyone"), "{text}");
        assert!(!text.contains(" @everyone"), "{text}");
    }
}
