//! What a press on a channel control means, decided once for every platform
//! (`contracts/channel-controls.md` §1).
//!
//! A platform control carries only `<short>.q<index>.<action>`, or `<short>.submit` /
//! `<short>.dismiss` for the set as a whole. The reader parses that into a [`Pressed`], lets
//! [`apply`] decide what it does to the person's [`Draft`], and acts on the [`Outcome`]. Discord
//! and Telegram therefore differ only in how they draw controls and read events, never in what a
//! press does — which is what keeps a Discord answer and a Telegram answer the same structure
//! (FR-026).

use std::collections::BTreeMap;

use crate::questions::model::{
    Answer, AnswerState, AnswerValue, Question, QuestionSetRecord, QuestionType,
};
use crate::questions::surface::Refusal;

/// One control's payload: the set's short reference, the question's **position**, and the action.
pub fn control_id(short_id: &str, index: usize, action: &str) -> String {
    format!("{short_id}.q{index}.{action}")
}

/// The id of a control that acts on the set as a whole.
pub fn set_control_id(short_id: &str, action: &str) -> String {
    format!("{short_id}.{action}")
}

/// What a pressed control asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pressed {
    pub short_id: String,
    /// The question's position; `None` for a control on the set (`submit`, `dismiss`).
    pub question: Option<usize>,
    pub action: String,
}

impl Pressed {
    /// Read a control id. Anything else — another bot's component, a malformed id — is `None`.
    pub fn parse(id: &str) -> Option<Self> {
        let mut parts = id.split('.');
        let short_id = parts.next().filter(|s| !s.is_empty())?.to_string();
        let second = parts.next()?;
        let pressed = match second.strip_prefix('q').and_then(|n| n.parse::<usize>().ok()) {
            Some(question) => Self {
                short_id,
                question: Some(question),
                action: parts.next()?.to_string(),
            },
            None => Self {
                short_id,
                question: None,
                action: second.to_string(),
            },
        };
        if parts.next().is_some() || pressed.action.is_empty() {
            return None;
        }
        Some(pressed)
    }

    /// The option this press chose, if it chose one (`o3`).
    pub fn option_index(&self) -> Option<usize> {
        self.action.strip_prefix('o')?.parse().ok()
    }
}

/// What a person has answered so far in a set the platform shows over several controls. Kept in
/// memory by [`crate::questions::Questions`]: a restart loses a half-made answer, never a set —
/// the set and its answerability live in storage (FR-025).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Draft {
    /// Committed answers, at most one per question.
    pub answers: Vec<Answer>,
    /// A choose-several's ticked options, by question position, before `pick` commits them.
    pub picks: BTreeMap<usize, Vec<usize>>,
}

impl Draft {
    /// The committed answer to `question_id`, if any.
    pub fn answer(&self, question_id: &str) -> Option<&Answer> {
        self.answers.iter().find(|a| a.question_id == question_id)
    }

    fn commit(&mut self, answer: Answer) {
        self.answers.retain(|a| a.question_id != answer.question_id);
        self.answers.push(answer);
    }
}

/// What the reader does next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The person dismissed the whole set.
    Dismiss,
    /// Submit these answers through [`crate::questions::submit::submit`].
    Submit(Vec<Answer>),
    /// The draft changed and the set stays open; a platform that shows ticks redraws them.
    Updated,
    /// Open the platform's text entry for this question.
    AskText(usize),
    /// The press named something the set does not have, or something it cannot do.
    Refused(Refusal),
}

fn answered(question: &Question, value: AnswerValue, label: Option<String>) -> Answer {
    Answer {
        question_id: question.id.clone(),
        state: AnswerState::Answered,
        value: Some(value),
        label,
    }
}

/// Whether the set is complete enough to submit without being asked to: a one-question set the
/// moment it is answered, any other once every question has an answer. A set with an optional
/// question the person may want to skip is submitted with the Submit control instead.
fn complete(record: &QuestionSetRecord, draft: &Draft) -> bool {
    let questions = &record.set.questions;
    !questions.is_empty()
        && questions
            .iter()
            .all(|question| draft.answer(&question.id).is_some())
}

fn finish(record: &QuestionSetRecord, draft: &Draft) -> Outcome {
    if complete(record, draft) {
        Outcome::Submit(draft.answers.clone())
    } else {
        Outcome::Updated
    }
}

/// Work out what `pressed` does. `selected` carries a select menu's chosen option indices.
pub fn apply(
    record: &QuestionSetRecord,
    draft: &mut Draft,
    pressed: &Pressed,
    selected: Option<&[usize]>,
) -> Outcome {
    let Some(index) = pressed.question else {
        return match pressed.action.as_str() {
            "dismiss" => Outcome::Dismiss,
            "submit" => Outcome::Submit(draft.answers.clone()),
            _ => Outcome::Refused(Refusal::UnknownChoice),
        };
    };
    let Some(question) = record.set.questions.get(index) else {
        return Outcome::Refused(Refusal::UnknownChoice);
    };

    match (question.kind, pressed.action.as_str()) {
        (QuestionType::Confirm, action @ ("yes" | "no")) => {
            let yes = action == "yes";
            let label = if yes {
                question.yes_label.clone().unwrap_or_else(|| "Yes".into())
            } else {
                question.no_label.clone().unwrap_or_else(|| "No".into())
            };
            draft.commit(answered(question, AnswerValue::Confirm(yes), Some(label)));
            finish(record, draft)
        }
        (QuestionType::Text, "text") => Outcome::AskText(index),
        (QuestionType::Choice, "select") => {
            let Some(option) = selected
                .and_then(|picked| picked.first())
                .and_then(|i| question.options.get(*i))
            else {
                return Outcome::Refused(Refusal::UnknownChoice);
            };
            draft.commit(answered(
                question,
                AnswerValue::Text(option.value.clone()),
                Some(option.label.clone()),
            ));
            finish(record, draft)
        }
        (QuestionType::MultiChoice, "select") => {
            let picked = selected.unwrap_or_default();
            if picked.iter().any(|i| *i >= question.options.len()) {
                return Outcome::Refused(Refusal::UnknownChoice);
            }
            draft.picks.insert(index, picked.to_vec());
            commit_picks(record, draft, index, question)
        }
        (QuestionType::Choice, _) if pressed.option_index().is_some() => {
            let Some(option) = pressed.option_index().and_then(|i| question.options.get(i))
            else {
                return Outcome::Refused(Refusal::UnknownChoice);
            };
            draft.commit(answered(
                question,
                AnswerValue::Text(option.value.clone()),
                Some(option.label.clone()),
            ));
            finish(record, draft)
        }
        // A toggle never commits: half a choice must not submit a set (FR-022).
        (QuestionType::MultiChoice, _) if pressed.option_index().is_some() => {
            let Some(option) = pressed.option_index().filter(|i| *i < question.options.len())
            else {
                return Outcome::Refused(Refusal::UnknownChoice);
            };
            let picks = draft.picks.entry(index).or_default();
            if let Some(at) = picks.iter().position(|p| *p == option) {
                picks.remove(at);
            } else {
                picks.push(option);
            }
            Outcome::Updated
        }
        (QuestionType::MultiChoice, "pick") => commit_picks(record, draft, index, question),
        _ => Outcome::Refused(Refusal::UnknownChoice),
    }
}

/// Turn the ticked options into the question's answer, in the order the options were offered. An
/// empty selection on an optional question is a skip; on a required one it is committed as
/// nothing chosen, so validation names the problem rather than the press doing nothing.
fn commit_picks(
    record: &QuestionSetRecord,
    draft: &mut Draft,
    index: usize,
    question: &Question,
) -> Outcome {
    let mut picks = draft.picks.get(&index).cloned().unwrap_or_default();
    picks.sort_unstable();
    picks.dedup();
    if picks.is_empty() && question.optional {
        draft.commit(Answer::skipped(question.id.clone()));
    } else {
        let values: Vec<String> = picks
            .iter()
            .filter_map(|i| question.options.get(*i))
            .map(|o| o.value.clone())
            .collect();
        let labels: Vec<String> = picks
            .iter()
            .filter_map(|i| question.options.get(*i))
            .map(|o| o.label.clone())
            .collect();
        draft.commit(answered(
            question,
            AnswerValue::Choices(values),
            Some(labels.join(", ")),
        ));
    }
    finish(record, draft)
}

/// A free-text answer (a Discord modal, a Telegram reply) to question `index`.
pub fn apply_text(
    record: &QuestionSetRecord,
    draft: &mut Draft,
    index: usize,
    text: &str,
) -> Outcome {
    let Some(question) = record.set.questions.get(index) else {
        return Outcome::Refused(Refusal::UnknownChoice);
    };
    if question.kind != QuestionType::Text {
        return Outcome::Refused(Refusal::UnknownChoice);
    }
    // A blank answer to an optional question is a skip; `normalise` does the same at submission.
    if text.trim().is_empty() && question.optional {
        draft.commit(Answer::skipped(question.id.clone()));
    } else {
        draft.commit(answered(question, AnswerValue::Text(text.to_string()), None));
    }
    finish(record, draft)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::questions::model::{ChoiceOption, QuestionSet, QuestionState};
    use crate::schema::{VizierChannelId, VizierSession};

    fn q(id: &str, kind: QuestionType, options: &[&str]) -> Question {
        Question {
            id: id.into(),
            prompt: id.into(),
            kind,
            help: None,
            optional: false,
            default: None,
            options: options
                .iter()
                .map(|o| ChoiceOption { value: (*o).into(), label: o.to_uppercase(), description: None })
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
                title: None,
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

    fn press(id: &str) -> Pressed {
        Pressed::parse(id).unwrap()
    }

    /// An id round-trips and fits both platforms' budgets (contracts/channel-controls.md §1).
    #[test]
    fn a_control_id_round_trips_and_fits_both_platforms() {
        let id = control_id("7f3a91c2", 9, "dismiss");
        assert_eq!(id, "7f3a91c2.q9.dismiss");
        assert!(id.len() <= crate::questions::limits::MAX_CONTROL_ID_BYTES);
        let p = Pressed::parse(&id).unwrap();
        assert_eq!((p.short_id.as_str(), p.question, p.action.as_str()), ("7f3a91c2", Some(9), "dismiss"));
    }

    /// The set-level controls have no question, and nothing else parses.
    #[test]
    fn the_set_controls_parse_and_garbage_does_not() {
        let p = Pressed::parse(&set_control_id("abc", "dismiss")).unwrap();
        assert_eq!((p.question, p.action.as_str()), (None, "dismiss"));
        assert_eq!(Pressed::parse("garbage"), None);
        assert_eq!(Pressed::parse("abc.qx.o1.extra"), None);
        assert_eq!(Pressed::parse(".dismiss"), None);
        assert_eq!(Pressed::parse("abc.q1."), None);
        assert_eq!(press(&control_id("abc", 2, "o13")).option_index(), Some(13));
    }

    /// One question answered by one press submits the set.
    #[test]
    fn a_single_choice_press_submits_the_set() {
        let r = record(vec![q("db", QuestionType::Choice, &["pg", "sqlite"])]);
        let mut d = Draft::default();
        match apply(&r, &mut d, &press("7f3a91c2.q0.o1"), None) {
            Outcome::Submit(answers) => {
                assert_eq!(answers.len(), 1);
                assert_eq!(answers[0].value, Some(AnswerValue::Text("sqlite".into())));
                assert_eq!(answers[0].label.as_deref(), Some("SQLITE"));
            }
            other => panic!("expected a submission, got {other:?}"),
        }
    }

    /// With several questions nothing submits until every one is answered, and a second press on
    /// one question replaces its answer.
    #[test]
    fn a_set_submits_when_every_question_is_answered() {
        let r = record(vec![
            q("db", QuestionType::Choice, &["pg", "sqlite"]),
            q("ok", QuestionType::Confirm, &[]),
        ]);
        let mut d = Draft::default();
        assert_eq!(apply(&r, &mut d, &press("7f3a91c2.q0.o0"), None), Outcome::Updated);
        assert_eq!(apply(&r, &mut d, &press("7f3a91c2.q0.o1"), None), Outcome::Updated);
        match apply(&r, &mut d, &press("7f3a91c2.q1.yes"), None) {
            Outcome::Submit(answers) => {
                assert_eq!(answers.len(), 2);
                let db = answers.iter().find(|a| a.question_id == "db").unwrap();
                assert_eq!(db.value, Some(AnswerValue::Text("sqlite".into())), "the later press wins");
            }
            other => panic!("expected a submission, got {other:?}"),
        }
    }

    /// A toggle changes the draft and never submits; `pick` commits the options in offered order
    /// (FR-022).
    #[test]
    fn a_choose_several_toggles_then_commits_on_pick() {
        let r = record(vec![q("f", QuestionType::MultiChoice, &["auth", "ci", "docs"])]);
        let mut d = Draft::default();
        assert_eq!(apply(&r, &mut d, &press("7f3a91c2.q0.o2"), None), Outcome::Updated);
        assert_eq!(apply(&r, &mut d, &press("7f3a91c2.q0.o0"), None), Outcome::Updated);
        assert_eq!(apply(&r, &mut d, &press("7f3a91c2.q0.o2"), None), Outcome::Updated, "toggled off");
        assert!(d.answers.is_empty(), "a toggle commits nothing");
        match apply(&r, &mut d, &press("7f3a91c2.q0.pick"), None) {
            Outcome::Submit(a) => assert_eq!(a[0].value, Some(AnswerValue::Choices(vec!["auth".into()]))),
            other => panic!("expected a submission, got {other:?}"),
        }
    }

    /// A select menu hands over its whole selection.
    #[test]
    fn a_select_menu_commits_its_selection() {
        let r = record(vec![q("f", QuestionType::MultiChoice, &["auth", "ci", "docs"])]);
        let mut d = Draft::default();
        match apply(&r, &mut d, &press("7f3a91c2.q0.select"), Some(&[2, 0])) {
            Outcome::Submit(a) => {
                assert_eq!(a[0].value, Some(AnswerValue::Choices(vec!["auth".into(), "docs".into()])))
            }
            other => panic!("expected a submission, got {other:?}"),
        }
        let single = record(vec![q("db", QuestionType::Choice, &["pg", "sqlite"])]);
        match apply(&single, &mut Draft::default(), &press("7f3a91c2.q0.select"), Some(&[1])) {
            Outcome::Submit(a) => assert_eq!(a[0].value, Some(AnswerValue::Text("sqlite".into()))),
            other => panic!("expected a submission, got {other:?}"),
        }
    }

    /// A press on something the set does not have is refused and changes nothing (FR-024).
    #[test]
    fn an_unknown_option_or_question_is_refused() {
        let r = record(vec![q("db", QuestionType::Choice, &["pg"])]);
        let mut d = Draft::default();
        for id in ["7f3a91c2.q0.o9", "7f3a91c2.q5.o0", "7f3a91c2.q0.yes", "7f3a91c2.bogus"] {
            assert_eq!(apply(&r, &mut d, &press(id), None), Outcome::Refused(Refusal::UnknownChoice), "{id}");
        }
        assert_eq!(
            apply(&r, &mut d, &press("7f3a91c2.q0.select"), Some(&[7])),
            Outcome::Refused(Refusal::UnknownChoice)
        );
        assert_eq!(d, Draft::default());
    }

    /// Dismiss declines the whole set; Submit sends what was answered, for the platform to
    /// validate; a text question asks for its entry point.
    #[test]
    fn dismiss_submit_and_text_do_what_they_say() {
        let r = record(vec![q("n", QuestionType::Text, &[]), q("ok", QuestionType::Confirm, &[])]);
        let mut d = Draft::default();
        assert_eq!(apply(&r, &mut d, &press("7f3a91c2.dismiss"), None), Outcome::Dismiss);
        assert_eq!(apply(&r, &mut d, &press("7f3a91c2.q0.text"), None), Outcome::AskText(0));
        assert_eq!(apply(&r, &mut d, &press("7f3a91c2.submit"), None), Outcome::Submit(vec![]));
    }

    /// A text answer fills its question, and a blank one to an optional question is a skip.
    #[test]
    fn a_text_answer_fills_its_question() {
        let mut optional = q("n", QuestionType::Text, &[]);
        optional.optional = true;
        let r = record(vec![optional, q("ok", QuestionType::Confirm, &[])]);
        let mut d = Draft::default();
        assert_eq!(apply_text(&r, &mut d, 0, "  "), Outcome::Updated);
        assert_eq!(d.answers[0].state, AnswerState::Skipped);
        assert_eq!(apply_text(&r, &mut d, 0, "demo"), Outcome::Updated);
        assert_eq!(d.answers.len(), 1, "the later answer replaces the earlier");
        assert_eq!(apply_text(&r, &mut d, 1, "x"), Outcome::Refused(Refusal::UnknownChoice), "not a text question");
        assert_eq!(apply_text(&r, &mut d, 9, "x"), Outcome::Refused(Refusal::UnknownChoice));
    }
}
