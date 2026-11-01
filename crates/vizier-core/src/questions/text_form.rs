//! The text form a set falls back to when native controls cannot hold it (FR-027).
//!
//! One message, one submission: a list of the questions with their ids, then one `key: value`
//! line per question in the reply. The grammar is here, apart from any platform, so both readers
//! share it and it can be tested without a bot.
//!
//! It exists because a platform can be unable to express a set natively — more options than a
//! control holds, more questions than one message carries, or free text where the platform has
//! no form. Falling back must lose nothing: not a question, its order, its id, or the rule that
//! one submission answers the whole set.

use crate::questions::model::{Answer, AnswerState, AnswerValue, Question, QuestionSet, QuestionType};

/// The message a person replies to.
pub fn render(set: &QuestionSet, short_id: &str) -> String {
    let mut out = String::new();
    match (&set.title, &set.questions.len()) {
        (Some(title), _) => out.push_str(&format!("{title} (set {short_id})\n\n")),
        (None, _) => out.push_str(&format!("Questions (set {short_id})\n\n")),
    }
    for (index, question) in set.questions.iter().enumerate() {
        out.push_str(&format!("{}. {} — {}", index + 1, question.id, question.prompt));
        match question.kind {
            QuestionType::Text => {
                out.push_str(" (text");
                if let Some(max) = question.max_length {
                    out.push_str(&format!(", up to {max} bytes"));
                }
                out.push(')');
            }
            QuestionType::Choice => {
                out.push_str(&format!(" (one of: {})", option_values(question)));
            }
            QuestionType::MultiChoice => {
                let (min, max) = question.selection_bounds();
                out.push_str(&format!(
                    " (any of: {}, {}–{})",
                    option_values(question),
                    min,
                    max
                ));
            }
            QuestionType::Confirm => out.push_str(" (yes or no)"),
        }
        if !question.optional {
            out.push_str(" *");
        }
        out.push('\n');
    }
    out.push_str(
        "\nReply with one line per question:\n  <id>: <answer>\n\n`*` marks a required question.",
    );
    out
}

fn option_values(question: &Question) -> String {
    question
        .options
        .iter()
        .map(|option| option.value.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// What a reply parsed to: the answers, or every problem at once — the same discipline the
/// WebUI card uses, so a person gets one message per attempt whichever surface they are on.
pub fn parse(set: &QuestionSet, reply: &str) -> Result<Vec<Answer>, Vec<String>> {
    let mut answers: Vec<Answer> = Vec::new();
    let mut problems: Vec<String> = Vec::new();

    for line in reply.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            problems.push(format!("`{line}` is not `<id>: <answer>`"));
            continue;
        };
        let key = key.trim();
        let value = value.trim();

        let Some(question) = set.question(key) else {
            problems.push(format!("`{key}` is not a question in this set"));
            continue;
        };
        if answers.iter().any(|answer| answer.question_id == key) {
            problems.push(format!("`{key}` was answered more than once"));
            continue;
        }

        let parsed = match question.kind {
            QuestionType::Text => {
                if value.is_empty() {
                    Answer::skipped(key)
                } else {
                    answer_with(key, AnswerValue::Text(value.to_string()), None)
                }
            }
            QuestionType::Choice => match question.option(value) {
                Some(option) => answer_with(
                    key,
                    AnswerValue::Text(option.value.clone()),
                    Some(option.label.clone()),
                ),
                None => {
                    problems.push(format!(
                        "`{value}` is not one of `{key}`'s options ({})",
                        option_values(question)
                    ));
                    continue;
                }
            },
            QuestionType::MultiChoice => {
                let mut chosen: Vec<String> = Vec::new();
                let mut labels: Vec<String> = Vec::new();
                let mut unknown = false;
                for piece in value.split(',').map(str::trim).filter(|p| !p.is_empty()) {
                    match question.option(piece) {
                        Some(option) => {
                            chosen.push(option.value.clone());
                            labels.push(option.label.clone());
                        }
                        None => {
                            problems.push(format!("`{piece}` is not one of `{key}`'s options"));
                            unknown = true;
                        }
                    }
                }
                if unknown {
                    continue;
                }
                if chosen.is_empty() {
                    Answer::skipped(key)
                } else {
                    answer_with(key, AnswerValue::Choices(chosen), Some(labels.join(", ")))
                }
            }
            QuestionType::Confirm => match value.to_ascii_lowercase().as_str() {
                "yes" | "y" | "true" => answer_with(key, AnswerValue::Confirm(true), None),
                "no" | "n" | "false" => answer_with(key, AnswerValue::Confirm(false), None),
                _ => {
                    problems.push(format!("`{key}` takes yes or no"));
                    continue;
                }
            },
        };
        answers.push(parsed);
    }

    // A required question nobody mentioned is missing, exactly as an empty one is.
    for question in &set.questions {
        if question.optional || question.default.is_some() {
            continue;
        }
        if !answers.iter().any(|answer| answer.question_id == question.id) {
            problems.push(format!(
                "`{}` is required and was not answered",
                question.id
            ));
        }
    }

    if problems.is_empty() {
        Ok(answers)
    } else {
        Err(problems)
    }
}

fn answer_with(id: &str, value: AnswerValue, label: Option<String>) -> Answer {
    Answer {
        question_id: id.to_string(),
        state: AnswerState::Answered,
        value: Some(value),
        label,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::questions::model::ChoiceOption;

    fn set() -> QuestionSet {
        let mut name = Question {
            id: "name".into(),
            prompt: "Project name".into(),
            kind: QuestionType::Text,
            help: None,
            optional: false,
            default: None,
            options: vec![],
            placeholder: None,
            max_length: Some(60),
            multiline: false,
            min_selections: None,
            max_selections: None,
            yes_label: None,
            no_label: None,
        };
        let mut db = name.clone();
        db.id = "db".into();
        db.prompt = "Database".into();
        db.kind = QuestionType::Choice;
        db.max_length = None;
        db.options = vec![
            ChoiceOption {
                value: "postgres".into(),
                label: "PostgreSQL".into(),
                description: None,
            },
            ChoiceOption {
                value: "sqlite".into(),
                label: "SQLite".into(),
                description: None,
            },
        ];
        let mut features = db.clone();
        features.id = "features".into();
        features.prompt = "Features".into();
        features.kind = QuestionType::MultiChoice;
        features.optional = true;
        features.options = vec![
            ChoiceOption {
                value: "auth".into(),
                label: "Auth".into(),
                description: None,
            },
            ChoiceOption {
                value: "ci".into(),
                label: "CI".into(),
                description: None,
            },
        ];
        QuestionSet {
            title: Some("Project setup".into()),
            description: None,
            questions: vec![name, db, features],
            submit_label: None,
            dismiss_label: None,
        }
    }

    /// The message names every question, its id, its options and which are required (FR-027).
    #[test]
    fn the_text_form_loses_nothing() {
        let text = render(&set(), "7f3a91c2");
        assert!(text.contains("7f3a91c2"));
        assert!(text.contains("name — Project name"));
        assert!(text.contains("one of: postgres, sqlite"));
        assert!(text.contains("features — Features"));
        // The optional question is the only one without the required marker.
        let required_marks = text.matches(" *").count();
        assert_eq!(required_marks, 2);
    }

    #[test]
    fn a_well_formed_reply_parses() {
        let answers = parse(&set(), "name: my-service\ndb: sqlite\nfeatures: auth, ci").unwrap();
        assert_eq!(answers.len(), 3);
        assert_eq!(answers[0].value, Some(AnswerValue::Text("my-service".into())));
        assert_eq!(answers[1].value, Some(AnswerValue::Text("sqlite".into())));
        assert_eq!(answers[1].label.as_deref(), Some("SQLite"));
        assert_eq!(
            answers[2].value,
            Some(AnswerValue::Choices(vec!["auth".into(), "ci".into()]))
        );
    }

    /// Every problem comes back together, and none of them is silently accepted.
    #[test]
    fn a_bad_reply_names_every_problem() {
        let problems = parse(&set(), "name: svc\ndb: mysql\nghost: x\nnot a line").unwrap_err();
        assert!(problems.iter().any(|p| p.contains("mysql")));
        assert!(problems.iter().any(|p| p.contains("ghost")));
        assert!(problems.iter().any(|p| p.contains("not `<id>: <answer>`")));
    }

    /// A required question left out is refused, and an optional one may simply be omitted.
    #[test]
    fn a_missing_required_question_is_refused() {
        let problems = parse(&set(), "db: sqlite").unwrap_err();
        assert!(problems.iter().any(|p| p.contains("`name` is required")));

        // Both required questions answered, the optional one omitted: accepted.
        assert!(parse(&set(), "name: svc\ndb: sqlite").is_ok());
    }

    #[test]
    fn a_duplicate_line_is_refused() {
        let problems = parse(&set(), "name: a\nname: b\ndb: sqlite").unwrap_err();
        assert!(problems.iter().any(|p| p.contains("more than once")));
    }

    #[test]
    fn a_confirm_takes_yes_or_no() {
        let mut set = set();
        set.questions.push(Question {
            id: "go".into(),
            prompt: "Proceed?".into(),
            kind: QuestionType::Confirm,
            help: None,
            optional: false,
            default: None,
            options: vec![],
            placeholder: None,
            max_length: None,
            multiline: false,
            min_selections: None,
            max_selections: None,
            yes_label: None,
            no_label: None,
        });
        let answers = parse(&set, "name: s\ndb: sqlite\ngo: yes").unwrap();
        assert_eq!(answers[2].value, Some(AnswerValue::Confirm(true)));
        assert!(parse(&set, "name: s\ndb: sqlite\ngo: maybe").is_err());
    }
}
