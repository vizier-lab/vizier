//! The rules a set and an answer are checked against (FR-005, FR-012).
//!
//! One module because both sides of a round trip have to agree: the same limits decide what the
//! agent may ask and what a person may submit, and a divergence is a set that can be shown but
//! never answered.
//!
//! A refusal collects **every** problem rather than the first, because the agent gets one round to
//! correct its call and a message naming one fault at a time would cost one round per fault
//! (FR-005). Each problem names the question it belongs to and a stable `rule` key, so a surface
//! can attach it to the field it came from and a test can assert on the rule rather than on prose.

use crate::questions::limits;
use crate::questions::model::{
    Answer, AnswerState, AnswerValue, Question, QuestionSet, QuestionType,
};

/// One thing wrong with a set or an answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// The question it belongs to; `None` for a problem with the set as a whole.
    pub question_id: Option<String>,
    /// A stable key, e.g. `duplicate_question_id`. Asserted on by tests and shown to the agent.
    pub rule: &'static str,
    /// What is wrong, in a sentence the agent can act on.
    pub message: String,
}

impl Problem {
    /// A problem on one question, for the tests of crates that format it.
    #[doc(hidden)]
    pub fn question_for_test(
        question_id: impl Into<String>,
        rule: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self::question(question_id, rule, message)
    }

    fn set(rule: &'static str, message: impl Into<String>) -> Self {
        Self {
            question_id: None,
            rule,
            message: message.into(),
        }
    }

    fn question(question_id: impl Into<String>, rule: &'static str, message: impl Into<String>) -> Self {
        Self {
            question_id: Some(question_id.into()),
            rule,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.question_id {
            Some(id) => write!(f, "{} ({}): {}", id, self.rule, self.message),
            None => write!(f, "{}: {}", self.rule, self.message),
        }
    }
}

/// Every way this set is unusable, or `Ok` when it may be shown (FR-005).
pub fn validate_set(set: &QuestionSet) -> Result<(), Vec<Problem>> {
    let mut problems: Vec<Problem> = Vec::new();

    if set.questions.is_empty() {
        problems.push(Problem::set(
            "no_questions",
            "a set needs at least one question",
        ));
    }
    if set.questions.len() > limits::MAX_QUESTIONS {
        problems.push(Problem::set(
            "too_many_questions",
            format!(
                "{} questions; the limit is {}",
                set.questions.len(),
                limits::MAX_QUESTIONS
            ),
        ));
    }
    if let Some(title) = &set.title
        && title.len() > limits::MAX_TITLE_BYTES
    {
        problems.push(Problem::set(
            "title_too_long",
            format!(
                "the title is {} bytes; the limit is {}",
                title.len(),
                limits::MAX_TITLE_BYTES
            ),
        ));
    }
    if let Some(description) = &set.description
        && description.len() > limits::MAX_DESCRIPTION_BYTES
    {
        problems.push(Problem::set(
            "description_too_long",
            format!(
                "the description is {} bytes; the limit is {}",
                description.len(),
                limits::MAX_DESCRIPTION_BYTES
            ),
        ));
    }
    for (name, label) in [
        ("submit_label", &set.submit_label),
        ("dismiss_label", &set.dismiss_label),
    ] {
        if let Some(label) = label
            && label.len() > limits::MAX_ACTION_LABEL_BYTES
        {
            problems.push(Problem::set(
                "action_label_too_long",
                format!(
                    "`{name}` is {} bytes; the limit is {}",
                    label.len(),
                    limits::MAX_ACTION_LABEL_BYTES
                ),
            ));
        }
    }

    let mut seen: Vec<String> = Vec::with_capacity(set.questions.len());
    for question in &set.questions {
        validate_question(question, &mut seen, &mut problems);
    }

    // The sum of limits that each hold on their own can still be more than a card, a history
    // entry and a platform message can carry (FR-004).
    if let Ok(encoded) = serde_json::to_vec(set)
        && encoded.len() > limits::MAX_SET_PAYLOAD_BYTES
    {
        problems.push(Problem::set(
            "set_too_large",
            format!(
                "the whole set is {} bytes; the limit is {}",
                encoded.len(),
                limits::MAX_SET_PAYLOAD_BYTES
            ),
        ));
    }

    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}

fn validate_question(question: &Question, seen: &mut Vec<String>, problems: &mut Vec<Problem>) {
    let id = question.id.clone();

    if id.is_empty() || id.len() > limits::MAX_QUESTION_ID_BYTES || !is_valid_id(&id) {
        problems.push(Problem::question(
            &id,
            "invalid_question_id",
            format!(
                "a question id is 1–{} bytes of lowercase letters, digits, `-` and `_`, starting \
                 with a letter or digit",
                limits::MAX_QUESTION_ID_BYTES
            ),
        ));
    } else if seen.contains(&id) {
        problems.push(Problem::question(
            &id,
            "duplicate_question_id",
            "another question in this set already has this id",
        ));
    } else {
        seen.push(id.clone());
    }

    if question.prompt.trim().is_empty() {
        problems.push(Problem::question(
            &id,
            "empty_prompt",
            "a question needs a prompt",
        ));
    } else if question.prompt.len() > limits::MAX_PROMPT_BYTES {
        problems.push(Problem::question(
            &id,
            "prompt_too_long",
            format!(
                "the prompt is {} bytes; the limit is {}",
                question.prompt.len(),
                limits::MAX_PROMPT_BYTES
            ),
        ));
    }
    if let Some(help) = &question.help
        && help.len() > limits::MAX_HELP_BYTES
    {
        problems.push(Problem::question(
            &id,
            "help_too_long",
            format!(
                "the help text is {} bytes; the limit is {}",
                help.len(),
                limits::MAX_HELP_BYTES
            ),
        ));
    }

    match question.kind {
        QuestionType::Text => {
            if let Some(max) = question.max_length
                && (max == 0 || max > limits::MAX_TEXT_ANSWER_BYTES)
            {
                problems.push(Problem::question(
                    &id,
                    "text_limit",
                    format!(
                        "`max_length` is {max}; it must be 1 or more and at most {}",
                        limits::MAX_TEXT_ANSWER_BYTES
                    ),
                ));
            }
            if question
                .default
                .as_ref()
                .is_some_and(|value| !matches!(value, AnswerValue::Text(_)))
            {
                problems.push(Problem::question(
                    &id,
                    "default_wrong_kind",
                    "a text question's default is a string",
                ));
            }
        }
        QuestionType::Choice | QuestionType::MultiChoice => {
            validate_options(question, &id, problems);
            validate_default(question, &id, problems);
            if question.kind == QuestionType::Choice
                && (question.min_selections.is_some() || question.max_selections.is_some())
            {
                problems.push(Problem::question(
                    &id,
                    "selection_bounds",
                    "`min_selections`/`max_selections` apply to `multi_choice`, not `choice`",
                ));
            }
            if question.kind == QuestionType::MultiChoice {
                let (min, max) = question.selection_bounds();
                if question.min_selections.unwrap_or(0) > question.options.len()
                    || question.max_selections.unwrap_or(question.options.len()) > question.options.len()
                {
                    problems.push(Problem::question(
                        &id,
                        "selection_bounds",
                        format!(
                            "the selection bounds are {min}–{max} for {} options",
                            question.options.len()
                        ),
                    ));
                } else if min > max {
                    problems.push(Problem::question(
                        &id,
                        "selection_bounds",
                        format!("`min_selections` ({min}) is greater than `max_selections` ({max})"),
                    ));
                }
            }
        }
        QuestionType::Confirm => {
            if question
                .default
                .as_ref()
                .is_some_and(|value| !matches!(value, AnswerValue::Confirm(_)))
            {
                problems.push(Problem::question(
                    &id,
                    "default_wrong_kind",
                    "a confirm question's default is `true` or `false`",
                ));
            }
            for (name, label) in [
                ("yes_label", &question.yes_label),
                ("no_label", &question.no_label),
            ] {
                if let Some(label) = label
                    && label.len() > limits::MAX_LABEL_BYTES
                {
                    problems.push(Problem::question(
                        &id,
                        "label_too_long",
                        format!(
                            "`{name}` is {} bytes; the limit is {}",
                            label.len(),
                            limits::MAX_LABEL_BYTES
                        ),
                    ));
                }
            }
        }
    }
}

fn validate_options(question: &Question, id: &str, problems: &mut Vec<Problem>) {
    if question.options.is_empty() {
        problems.push(Problem::question(
            id,
            "no_options",
            "a choice question needs at least one option",
        ));
        return;
    }
    if question.options.len() > limits::MAX_OPTIONS {
        problems.push(Problem::question(
            id,
            "too_many_options",
            format!(
                "{} options; the limit is {}",
                question.options.len(),
                limits::MAX_OPTIONS
            ),
        ));
    }
    let mut values: Vec<&str> = Vec::with_capacity(question.options.len());
    for option in &question.options {
        if option.value.is_empty() || option.value.len() > limits::MAX_OPTION_VALUE_BYTES {
            problems.push(Problem::question(
                id,
                "invalid_option_value",
                format!(
                    "an option value is 1–{} bytes",
                    limits::MAX_OPTION_VALUE_BYTES
                ),
            ));
        } else if values.contains(&option.value.as_str()) {
            problems.push(Problem::question(
                id,
                "duplicate_option_value",
                format!("the option value {:?} appears more than once", option.value),
            ));
        } else {
            values.push(&option.value);
        }
        if option.label.trim().is_empty() {
            problems.push(Problem::question(
                id,
                "empty_option_label",
                format!("the option {:?} needs a label", option.value),
            ));
        } else if option.label.len() > limits::MAX_LABEL_BYTES {
            problems.push(Problem::question(
                id,
                "label_too_long",
                format!(
                    "an option label is {} bytes; the limit is {}",
                    option.label.len(),
                    limits::MAX_LABEL_BYTES
                ),
            ));
        }
        if let Some(description) = &option.description
            && description.len() > limits::MAX_OPTION_DESCRIPTION_BYTES
        {
            problems.push(Problem::question(
                id,
                "option_description_too_long",
                format!(
                    "an option description is {} bytes; the limit is {}",
                    description.len(),
                    limits::MAX_OPTION_DESCRIPTION_BYTES
                ),
            ));
        }
    }
}

fn validate_default(question: &Question, id: &str, problems: &mut Vec<Problem>) {
    let Some(default) = &question.default else {
        return;
    };
    match (question.kind, default) {
        (QuestionType::Choice, AnswerValue::Text(value)) => {
            if question.option(value).is_none() {
                problems.push(Problem::question(
                    id,
                    "default_not_an_option",
                    format!("the default {value:?} is not one of this question's options"),
                ));
            }
        }
        (QuestionType::MultiChoice, AnswerValue::Choices(values)) => {
            if values.iter().any(|value| question.option(value).is_none()) {
                problems.push(Problem::question(
                    id,
                    "default_not_an_option",
                    "the default names an option this question does not offer",
                ));
            } else {
                let (min, max) = question.selection_bounds();
                if values.len() < min || values.len() > max {
                    problems.push(Problem::question(
                        id,
                        "selection_bounds",
                        format!(
                            "the default picks {} option(s); this question takes {min}–{max}",
                            values.len()
                        ),
                    ));
                }
            }
        }
        _ => problems.push(Problem::question(
            id,
            "default_wrong_kind",
            "the default does not match the question's type",
        )),
    }
}

/// 1–64 bytes, `^[a-z0-9][a-z0-9_-]*$`: what fits at the head of a control id and in a log line.
fn is_valid_id(id: &str) -> bool {
    let mut chars = id.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() || first.is_ascii_digit() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// Every way this answer is not acceptable for `set`, or `Ok` when it may be submitted (FR-012).
pub fn validate_answer(set: &QuestionSet, answers: &[Answer]) -> Result<(), Vec<Problem>> {
    let mut problems: Vec<Problem> = Vec::new();
    if let Ok(encoded) = serde_json::to_vec(answers)
        && encoded.len() > limits::MAX_ANSWER_PAYLOAD_BYTES
    {
        problems.push(Problem::set(
            "answer_too_large",
            format!(
                "the answers are {} bytes; the limit is {}",
                encoded.len(),
                limits::MAX_ANSWER_PAYLOAD_BYTES
            ),
        ));
    }
    let mut seen: Vec<&str> = Vec::with_capacity(answers.len());

    for answer in answers {
        let Some(question) = set.question(&answer.question_id) else {
            problems.push(Problem::question(
                &answer.question_id,
                "unknown_question",
                "this set has no such question",
            ));
            continue;
        };
        if seen.contains(&answer.question_id.as_str()) {
            problems.push(Problem::question(
                &answer.question_id,
                "duplicate_answer",
                "this question was answered more than once",
            ));
            continue;
        }
        seen.push(&answer.question_id);
        validate_answer_value(question, answer, &mut problems);
    }

    // Every required question must be present and answered. A question with a default counts as
    // answered without the person touching it, which is what a default is for.
    for question in &set.questions {
        if question.optional || question.default.is_some() {
            continue;
        }
        match answers.iter().find(|answer| answer.question_id == question.id) {
            None => problems.push(Problem::question(
                &question.id,
                "unanswered_required",
                "this question is required and was not answered",
            )),
            // A whitespace-only text counts as unanswered for a required question, exactly as a
            // skipped one does (US2 edge case), so the two cannot be told apart by the person.
            Some(answer) if is_unanswered(answer) => problems.push(Problem::question(
                &question.id,
                "unanswered_required",
                "this question is required and was left blank",
            )),
            Some(_) => {}
        }
    }

    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}

/// Whether the person in effect said nothing: the question was skipped, or a text answer is only
/// whitespace. One predicate, so the required-question check and the agent's view cannot disagree.
pub fn is_unanswered(answer: &Answer) -> bool {
    match (&answer.state, &answer.value) {
        (AnswerState::Skipped, _) => true,
        (AnswerState::Answered, Some(AnswerValue::Text(text))) => text.trim().is_empty(),
        _ => false,
    }
}

/// Turn each blank text answer into an explicit `skipped` one, so an optional question the person
/// left empty reaches the agent as skipped rather than as an empty string (US2 #3).
///
/// Applied once, when an answer is submitted; `validate_answer` still refuses a blank answer that
/// reaches it unnormalised, because a surface could construct one.
pub fn normalise(answers: Vec<Answer>) -> Vec<Answer> {
    answers
        .into_iter()
        .map(|answer| match (&answer.state, &answer.value) {
            (AnswerState::Answered, Some(AnswerValue::Text(text))) if text.trim().is_empty() => {
                Answer::skipped(answer.question_id)
            }
            _ => answer,
        })
        .collect()
}

fn validate_answer_value(question: &Question, answer: &Answer, problems: &mut Vec<Problem>) {
    if answer.state == AnswerState::Skipped {
        return;
    }
    let Some(value) = &answer.value else {
        problems.push(Problem::question(
            &question.id,
            "missing_value",
            "an answered question needs a value",
        ));
        return;
    };

    match (question.kind, value) {
        (QuestionType::Text, AnswerValue::Text(text)) => {
            if text.trim().is_empty() {
                problems.push(Problem::question(
                    &question.id,
                    "empty_text",
                    "a text answer cannot be only whitespace",
                ));
            } else if text.len() > question.text_limit() {
                problems.push(Problem::question(
                    &question.id,
                    "text_too_long",
                    format!(
                        "the answer is {} bytes; this question takes at most {}",
                        text.len(),
                        question.text_limit()
                    ),
                ));
            }
        }
        (QuestionType::Choice, AnswerValue::Text(chosen)) => {
            if question.option(chosen).is_none() {
                problems.push(Problem::question(
                    &question.id,
                    "value_not_offered",
                    format!("{chosen:?} is not one of this question's options"),
                ));
            }
        }
        (QuestionType::MultiChoice, AnswerValue::Choices(chosen)) => {
            let (min, max) = question.selection_bounds();
            let mut unique: Vec<&str> = Vec::with_capacity(chosen.len());
            for value in chosen {
                if question.option(value).is_none() {
                    problems.push(Problem::question(
                        &question.id,
                        "value_not_offered",
                        format!("{value:?} is not one of this question's options"),
                    ));
                } else if unique.contains(&value.as_str()) {
                    problems.push(Problem::question(
                        &question.id,
                        "duplicate_selection",
                        format!("{value:?} was selected more than once"),
                    ));
                } else {
                    unique.push(value);
                }
            }
            if chosen.len() < min {
                problems.push(Problem::question(
                    &question.id,
                    "too_few_selections",
                    format!("pick at least {min}; {} were chosen", chosen.len()),
                ));
            }
            if chosen.len() > max {
                problems.push(Problem::question(
                    &question.id,
                    "too_many_selections",
                    format!("pick at most {max}; {} were chosen", chosen.len()),
                ));
            }
        }
        (QuestionType::Confirm, AnswerValue::Confirm(_)) => {}
        _ => problems.push(Problem::question(
            &question.id,
            "wrong_value_kind",
            format!("this question takes a {}", question.kind.name()),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::questions::model::{ChoiceOption, Question};

    fn text(id: &str, optional: bool) -> Question {
        Question {
            id: id.into(),
            prompt: format!("Prompt for {id}"),
            kind: QuestionType::Text,
            help: None,
            optional,
            default: None,
            options: vec![],
            placeholder: None,
            max_length: None,
            multiline: false,
            min_selections: None,
            max_selections: None,
            yes_label: None,
            no_label: None,
        }
    }

    fn choice(id: &str, values: &[&str]) -> Question {
        Question {
            kind: QuestionType::Choice,
            options: values
                .iter()
                .map(|value| ChoiceOption {
                    value: (*value).into(),
                    label: value.to_uppercase(),
                    description: None,
                })
                .collect(),
            ..text(id, false)
        }
    }

    fn multi(id: &str, values: &[&str], min: Option<usize>, max: Option<usize>) -> Question {
        Question {
            kind: QuestionType::MultiChoice,
            min_selections: min,
            max_selections: max,
            ..choice(id, values)
        }
    }

    fn confirmed(id: &str) -> Question {
        Question {
            kind: QuestionType::Confirm,
            options: vec![],
            ..text(id, false)
        }
    }

    fn set_of(questions: Vec<Question>) -> QuestionSet {
        QuestionSet {
            title: None,
            description: None,
            questions,
            submit_label: None,
            dismiss_label: None,
        }
    }

    fn rules(problems: &[Problem]) -> Vec<&'static str> {
        let mut rules: Vec<&'static str> = problems.iter().map(|p| p.rule).collect();
        rules.sort_unstable();
        rules
    }

    fn answers(pairs: Vec<(&str, AnswerValue)>) -> Vec<Answer> {
        pairs
            .into_iter()
            .map(|(question_id, value)| Answer {
                question_id: question_id.into(),
                state: AnswerState::Answered,
                value: Some(value),
                label: None,
            })
            .collect()
    }

    fn answered(id: &str, value: AnswerValue) -> Answer {
        Answer {
            question_id: id.into(),
            state: AnswerState::Answered,
            value: Some(value),
            label: None,
        }
    }


    // ---- the set (FR-005) -------------------------------------------------------------

    /// A well-formed set is accepted; the happy path must not be made expensive by the rules.
    #[test]
    fn a_valid_set_is_accepted() {
        let set = set_of(vec![
            text("name", false),
            choice("db", &["postgres", "sqlite"]),
            multi("features", &["auth", "ci"], Some(0), Some(2)),
            confirmed("go"),
        ]);
        assert_eq!(validate_set(&set), Ok(()));
    }

    /// Every problem is reported at once: the agent gets one round to fix the call, and a message
    /// naming one fault at a time would cost a round per fault (FR-005).
    #[test]
    fn every_problem_is_reported_together() {
        let mut broken = choice("db", &["postgres", "postgres"]);
        broken.prompt = "   ".into();
        let set = set_of(vec![broken, text("db", false)]);
        let problems = validate_set(&set).unwrap_err();
        assert_eq!(
            rules(&problems),
            vec![
                "duplicate_option_value",
                "duplicate_question_id",
                "empty_prompt"
            ],
            "{problems:?}"
        );
        assert!(problems.iter().all(|p| p.question_id.as_deref() == Some("db")));
    }

    #[test]
    fn a_set_with_no_questions_and_one_over_the_limit_is_refused() {
        assert_eq!(
            rules(&validate_set(&set_of(vec![])).unwrap_err()),
            ["no_questions"]
        );

        let many = (0..crate::questions::limits::MAX_QUESTIONS + 1)
            .map(|i| text(&format!("q{i}"), true))
            .collect();
        assert_eq!(
            rules(&validate_set(&set_of(many)).unwrap_err()),
            ["too_many_questions"]
        );
    }


    #[test]
    fn a_question_id_must_be_addressable() {
        let long = "x".repeat(65);
        for bad in ["", "Has Space", "-leading", "UPPER", long.as_str()] {
            let set = set_of(vec![text(bad, false)]);
            assert_eq!(
                rules(&validate_set(&set).unwrap_err()),
                ["invalid_question_id"],
                "{bad:?} should be refused"
            );
        }
        for good in ["q", "db", "q1", "a-b_c1"] {
            let set = set_of(vec![text(good, false)]);
            assert_eq!(validate_set(&set), Ok(()), "{good:?} should be accepted");
        }
    }

    #[test]
    fn a_choice_needs_options_and_they_must_be_distinct() {
        assert_eq!(
            rules(&validate_set(&set_of(vec![choice("db", &[])])).unwrap_err()),
            ["no_options"]
        );

        let too_many: Vec<String> = (0..crate::questions::limits::MAX_OPTIONS + 1)
            .map(|i| format!("o{i}"))
            .collect();
        let too_many: Vec<&str> = too_many.iter().map(String::as_str).collect();
        assert_eq!(
            rules(&validate_set(&set_of(vec![choice("db", &too_many)])).unwrap_err()),
            ["too_many_options"]
        );

        assert_eq!(
            rules(&validate_set(&set_of(vec![choice("db", &["a", "a"])])).unwrap_err()),
            ["duplicate_option_value"]
        );
    }


    #[test]
    fn a_default_must_be_something_the_question_offers() {
        let mut bad = choice("db", &["postgres", "sqlite"]);
        bad.default = Some(AnswerValue::Text("mysql".into()));
        assert_eq!(
            rules(&validate_set(&set_of(vec![bad])).unwrap_err()),
            ["default_not_an_option"]
        );

        let mut good = choice("db", &["postgres", "sqlite"]);
        good.default = Some(AnswerValue::Text("sqlite".into()));
        assert_eq!(validate_set(&set_of(vec![good])), Ok(()));

        // A default of the wrong shape is its own problem, not a silent pass.
        let mut wrong = confirmed("go");
        wrong.default = Some(AnswerValue::Text("yes".into()));
        assert_eq!(
            rules(&validate_set(&set_of(vec![wrong])).unwrap_err()),
            ["default_wrong_kind"]
        );
    }

    #[test]
    fn inconsistent_selection_bounds_are_refused() {
        let inverted = multi("f", &["a", "b"], Some(2), Some(1));
        assert_eq!(
            rules(&validate_set(&set_of(vec![inverted])).unwrap_err()),
            ["selection_bounds"]
        );

        let beyond = multi("f", &["a", "b"], Some(0), Some(3));
        assert_eq!(
            rules(&validate_set(&set_of(vec![beyond])).unwrap_err()),
            ["selection_bounds"]
        );

        let mut single = choice("db", &["a", "b"]);
        single.min_selections = Some(1);
        assert_eq!(
            rules(&validate_set(&set_of(vec![single])).unwrap_err()),
            ["selection_bounds"]
        );
    }


    #[test]
    fn an_over_limit_prompt_or_help_text_is_refused() {
        let mut long_prompt = text("a", false);
        long_prompt.prompt = "x".repeat(crate::questions::limits::MAX_PROMPT_BYTES + 1);
        assert_eq!(
            rules(&validate_set(&set_of(vec![long_prompt])).unwrap_err()),
            ["prompt_too_long"]
        );

        let mut long_help = text("a", false);
        long_help.help = Some("x".repeat(crate::questions::limits::MAX_HELP_BYTES + 1));
        assert_eq!(
            rules(&validate_set(&set_of(vec![long_help])).unwrap_err()),
            ["help_too_long"]
        );
    }

    // ---- the answer (FR-012) ----------------------------------------------------------

    /// A value that was never offered, too few selections, and a happy path, all against the
    /// same set so the rules cannot drift apart.
    #[test]
    fn an_answer_is_checked_against_the_set_it_answers() {
        let set = set_of(vec![
            text("name", false),
            choice("db", &["postgres", "sqlite"]),
            multi("features", &["auth", "ci"], Some(1), Some(2)),
        ]);

        let good = answers(vec![
            ("name", AnswerValue::Text("svc".into())),
            ("db", AnswerValue::Text("sqlite".into())),
            ("features", AnswerValue::Choices(vec!["auth".into()])),
        ]);
        assert_eq!(validate_answer(&set, &good), Ok(()));

        let not_offered = answers(vec![
            ("name", AnswerValue::Text("svc".into())),
            ("db", AnswerValue::Text("mysql".into())),
            ("features", AnswerValue::Choices(vec!["auth".into()])),
        ]);
        assert_eq!(
            rules(&validate_answer(&set, &not_offered).unwrap_err()),
            ["value_not_offered"]
        );

        let too_few = answers(vec![
            ("name", AnswerValue::Text("svc".into())),
            ("db", AnswerValue::Text("sqlite".into())),
            ("features", AnswerValue::Choices(vec![])),
        ]);
        assert_eq!(
            rules(&validate_answer(&set, &too_few).unwrap_err()),
            ["too_few_selections"]
        );
    }


    #[test]
    fn a_required_question_left_out_or_skipped_is_refused() {
        let set = set_of(vec![text("name", false), text("note", true)]);

        assert_eq!(
            rules(&validate_answer(&set, &[]).unwrap_err()),
            ["unanswered_required"]
        );
        assert_eq!(
            rules(&validate_answer(&set, &[Answer::skipped("name")]).unwrap_err()),
            ["unanswered_required"]
        );

        // An optional question may be skipped, and the agent is told so rather than left to guess.
        let with_skip = vec![
            answered("name", AnswerValue::Text("svc".into())),
            Answer::skipped("note"),
        ];
        assert_eq!(validate_answer(&set, &with_skip), Ok(()));
    }

    /// A default counts as an answer, so a required question with one may be submitted untouched.
    #[test]
    fn a_default_answers_a_required_question() {
        let mut db = choice("db", &["postgres", "sqlite"]);
        db.default = Some(AnswerValue::Text("sqlite".into()));
        let set = set_of(vec![db]);
        assert_eq!(validate_answer(&set, &[]), Ok(()));
    }

    /// The spec's edge case: a whitespace-only text is unanswered for a required question and
    /// skipped for an optional one, and the two paths must agree.
    #[test]
    fn a_whitespace_only_text_counts_as_unanswered() {
        let required = set_of(vec![text("name", false)]);
        let blank = answers(vec![("name", AnswerValue::Text("   \n ".into()))]);
        assert_eq!(
            rules(&validate_answer(&required, &blank).unwrap_err()),
            ["empty_text", "unanswered_required"]
        );

        // Normalised first — what a submission actually does — the same blank is simply skipped.
        let blank_answer = answers(vec![("name", AnswerValue::Text("   \n ".into()))]);
        let normalised = normalise(blank_answer);
        assert_eq!(normalised[0].state, AnswerState::Skipped);
        assert_eq!(normalised[0].value, None);
        assert!(is_unanswered(&normalised[0]));

        // An optional question left blank reaches the agent as skipped, not as an empty string.
        let optional = set_of(vec![text("note", true)]);
        let blank_note = normalise(answers(vec![("note", AnswerValue::Text(" ".into()))]));
        assert_eq!(blank_note[0].state, AnswerState::Skipped);
        assert_eq!(validate_answer(&optional, &blank_note), Ok(()));
    }


    #[test]
    fn a_text_answer_over_its_own_limit_is_refused() {
        let mut name = text("name", false);
        name.max_length = Some(4);
        let set = set_of(vec![name]);

        let fits = answers(vec![("name", AnswerValue::Text("abcd".into()))]);
        assert_eq!(validate_answer(&set, &fits), Ok(()));

        let over = answers(vec![("name", AnswerValue::Text("abcde".into()))]);
        assert_eq!(
            rules(&validate_answer(&set, &over).unwrap_err()),
            ["text_too_long"]
        );
    }

    #[test]
    fn unknown_and_duplicated_questions_are_refused() {
        let set = set_of(vec![text("name", true)]);

        let ghost = answers(vec![("ghost", AnswerValue::Text("x".into()))]);
        assert_eq!(
            rules(&validate_answer(&set, &ghost).unwrap_err()),
            ["unknown_question"]
        );

        let twice = vec![
            answered("name", AnswerValue::Text("a".into())),
            answered("name", AnswerValue::Text("b".into())),
        ];
        assert_eq!(
            rules(&validate_answer(&set, &twice).unwrap_err()),
            ["duplicate_answer"]
        );
    }

    #[test]
    fn a_value_of_the_wrong_kind_is_refused() {
        let set = set_of(vec![choice("db", &["a", "b"]), confirmed("go")]);
        let wrong = answers(vec![
            ("db", AnswerValue::Choices(vec!["a".into()])),
            ("go", AnswerValue::Text("yes".into())),
        ]);
        let problems = validate_answer(&set, &wrong).unwrap_err();
        assert_eq!(rules(&problems), ["wrong_value_kind", "wrong_value_kind"]);
    }

    /// A refusal always says something the agent can act on, and names the rule that broke.
    #[test]
    fn a_problem_reads_as_a_sentence() {
        let problem = Problem::question("db", "value_not_offered", "\"mysql\" is not an option");
        assert_eq!(
            problem.to_string(),
            "db (value_not_offered): \"mysql\" is not an option"
        );
        let set_problem = Problem::set("no_questions", "a set needs at least one question");
        assert_eq!(
            set_problem.to_string(),
            "no_questions: a set needs at least one question"
        );
    }

    /// A choose-several answer: values distinct, inside the bounds, all offered (US2, T028).
    #[test]
    fn a_choose_several_answer_is_checked_for_distinct_values_and_bounds() {
        let set = set_of(vec![multi("f", &["a", "b", "c"], Some(1), Some(2))]);
        let check = |values: &[&str]| {
            let chosen = AnswerValue::Choices(values.iter().map(|v| v.to_string()).collect());
            validate_answer(&set, &answers(vec![("f", chosen)])).map_err(|p| rules(&p))
        };
        assert_eq!(check(&["a"]), Ok(()));
        assert_eq!(check(&["a", "b"]), Ok(()));
        assert_eq!(check(&["a", "a"]), Err(vec!["duplicate_selection"]));
        assert_eq!(check(&[]), Err(vec!["too_few_selections"]));
        assert_eq!(check(&["a", "b", "c"]), Err(vec!["too_many_selections"]));
        assert_eq!(check(&["z"]), Err(vec!["value_not_offered"]));
    }

    /// The limits that hold per field also hold in sum: a set can be made only of fields under
    /// their own limit and still be more than a card, a history entry or a message can carry.
    #[test]
    fn a_whole_set_over_the_payload_limit_is_refused() {
        let many_options = |id: &str| {
            let mut q = choice(id, &[]);
            q.options = (0..crate::questions::limits::MAX_OPTIONS)
                .map(|n| crate::questions::model::ChoiceOption {
                    value: format!("{}{n}", "v".repeat(90)),
                    label: "l".repeat(crate::questions::limits::MAX_LABEL_BYTES),
                    description: Some("d".repeat(crate::questions::limits::MAX_OPTION_DESCRIPTION_BYTES)),
                })
                .collect();
            q.prompt = "p".repeat(crate::questions::limits::MAX_PROMPT_BYTES);
            q.help = Some("h".repeat(crate::questions::limits::MAX_HELP_BYTES));
            q
        };
        let ids = ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"];
        let set = set_of(ids.iter().map(|id| many_options(id)).collect());
        // Every field is within its own limit, so the only fault is the total.
        assert_eq!(rules(&validate_set(&set).unwrap_err()), ["set_too_large"]);

        // One such question is a perfectly good set.
        assert_eq!(validate_set(&set_of(vec![many_options("a")])), Ok(()));
    }

    /// An answer whose total is over the limit is refused whole.
    #[test]
    fn answers_over_the_payload_limit_are_refused() {
        let set = set_of(vec![text("a", false)]);
        let huge = vec![answered("a", AnswerValue::Text("x".repeat(70 * 1024)))];
        let rules = rules(&validate_answer(&set, &huge).unwrap_err());
        assert!(rules.contains(&"answer_too_large"), "{rules:?}");
    }
}
