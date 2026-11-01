//! One table of numbers, so a limit is stated once and read everywhere (research R12).
//!
//! A refusal names the limit it broke (FR-005, FR-060), the tool descriptions quote these values,
//! and the WebUI imports them through the generated schema rather than re-deriving them. Nothing
//! here is a tuning knob: each value is set by a platform's own budget or by what keeps one card
//! answerable in one submission.

use std::time::Duration;

/// Most questions one set may carry. Answered together, in one submission (FR-004): ten is the
/// most that stays a readable card on a phone.
pub const MAX_QUESTIONS: usize = 10;

/// Most options one choice question may offer. Discord's select menu holds 25 (FR-022), and it is
/// the smallest of the platform ceilings, so it bounds the model.
pub const MAX_OPTIONS: usize = 25;

/// Longest a question's prompt may be, in bytes.
pub const MAX_PROMPT_BYTES: usize = 500;

/// Longest a question's help text may be, in bytes.
pub const MAX_HELP_BYTES: usize = 500;

/// Longest an option's display label may be, in bytes.
pub const MAX_LABEL_BYTES: usize = 100;

/// Longest an option's value may be, in bytes. The value is what the agent receives and what a
/// platform control refers to by index, so it never travels inside a control id.
pub const MAX_OPTION_VALUE_BYTES: usize = 100;

/// Longest an option's description may be, in bytes.
pub const MAX_OPTION_DESCRIPTION_BYTES: usize = 200;

/// Longest a question id may be, in bytes.
pub const MAX_QUESTION_ID_BYTES: usize = 64;

/// Longest a free-text answer may be, in bytes. Discord's modal input stops at 4,000.
pub const MAX_TEXT_ANSWER_BYTES: usize = 4_000;

/// Longest a set's title may be, in bytes.
pub const MAX_TITLE_BYTES: usize = 200;

/// Longest a set's description may be, in bytes.
pub const MAX_DESCRIPTION_BYTES: usize = 1_000;

/// Longest a set's own submit or dismiss label may be, in bytes.
pub const MAX_ACTION_LABEL_BYTES: usize = 100;

/// Most bytes one serialised question set may take (FR-060).
pub const MAX_SET_PAYLOAD_BYTES: usize = 64 * 1024;

/// Most bytes one serialised answer may take (FR-060). The same budget as a client event's data.
pub const MAX_ANSWER_PAYLOAD_BYTES: usize = 64 * 1024;

/// How long a platform control id may be. Telegram's `callback_data` stops at 64 bytes and is the
/// smaller of the two platforms, so the short reference plus a question index and an option index
/// must fit inside it (contracts/channel-controls.md §1).
pub const MAX_CONTROL_ID_BYTES: usize = 64;

/// How long the reference a control carries is. Eight characters of a uuid's hex keep collisions
/// vanishingly rare while leaving room for the rest of the control id.
pub const SHORT_ID_BYTES: usize = 8;

/// How long the sensitive-input ask waits for a person before handing the script a timeout
/// (FR-051). A person is present; a script still has its own budget.
pub const ASK_WAIT: Duration = Duration::from_secs(120);

/// Shortest a sensitive value may be. Re-exported from the redactor rather than restated, so a
/// value is either protectable or explicitly flagged as not (FR-056).
pub const MIN_SENSITIVE_VALUE_BYTES: usize = crate::secrets::MIN_VALUE_BYTES;

/// Longest a sensitive value may be. The redactor's own maximum (FR-056).
pub const MAX_SENSITIVE_VALUE_BYTES: usize = crate::secrets::MAX_VALUE_BYTES;

/// Longest a sensitive ask's stated purpose may be, in bytes.
pub const MAX_ASK_PURPOSE_BYTES: usize = 1_000;

/// Most question sets one turn may present (FR-008).
pub const MAX_SETS_PER_TURN: usize = 1;

/// Most asks one conversation may have open at once (FR-053).
pub const MAX_OPEN_ASKS_PER_CONVERSATION: usize = 1;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::{MAX_VALUE_BYTES, MIN_VALUE_BYTES};

    #[test]
    fn the_stated_limits_are_the_ones_the_contracts_quote() {
        assert_eq!(MAX_QUESTIONS, 10);
        assert_eq!(MAX_OPTIONS, 25);
        assert_eq!(MAX_TEXT_ANSWER_BYTES, 4_000);
        assert_eq!(MAX_SET_PAYLOAD_BYTES, 65_536);
        assert_eq!(MAX_ANSWER_PAYLOAD_BYTES, 65_536);
        assert_eq!(MAX_CONTROL_ID_BYTES, 64);
        assert_eq!(SHORT_ID_BYTES, 8);
        assert_eq!(ASK_WAIT, Duration::from_secs(120));
        assert_eq!(MAX_SETS_PER_TURN, 1);
        assert_eq!(MAX_OPEN_ASKS_PER_CONVERSATION, 1);
    }

    /// Discord's select menu caps a question's options, so nothing may exceed it.
    #[test]
    fn no_question_offers_more_options_than_a_platform_control_holds() {
        assert!(MAX_OPTIONS <= 25);
    }

    /// Discord's modal input, the smallest text-entry ceiling of the two platforms.
    #[test]
    fn a_text_answer_fits_a_platform_text_input() {
        assert!(MAX_TEXT_ANSWER_BYTES <= 4_000);
    }

    /// A control carries `<short>.q<index>.<action>`: the set's short reference, the question's
    /// **position** in the set, then the action or the option's index. Telegram's `callback_data`
    /// stops at 64 bytes and is the smaller of the two platforms, so the encoding has to fit
    /// there at its worst.
    ///
    /// The question's own id deliberately does not travel in a control: it may be up to
    /// [`MAX_QUESTION_ID_BYTES`] long, which on its own would not fit (asserted below), and the
    /// stored set already maps a position to an id.
    #[test]
    fn a_control_id_fits_the_smallest_platform_budget() {
        // Two digits is enough for `MAX_QUESTIONS`; "dismiss" is the longest action word.
        let worst = SHORT_ID_BYTES + 1 + "q10".len() + 1 + "dismiss".len();
        assert!(
            worst <= MAX_CONTROL_ID_BYTES,
            "a control id of {worst} bytes does not fit {MAX_CONTROL_ID_BYTES}"
        );

        // The same control carrying the agent's question id would not fit, which is why it
        // carries the position instead.
        let carrying_the_id = SHORT_ID_BYTES + 1 + MAX_QUESTION_ID_BYTES + 1 + "dismiss".len();
        assert!(
            carrying_the_id > MAX_CONTROL_ID_BYTES,
            "an id-carrying control of {carrying_the_id} bytes would have fitted after all"
        );
    }

    /// The sensitive tool reuses the redaction bounds rather than inventing its own (FR-056).
    #[test]
    fn the_sensitive_value_bounds_are_the_redaction_bounds() {
        assert_eq!(MIN_VALUE_BYTES, 8);
        assert_eq!(MAX_VALUE_BYTES, 65_536);
        assert!(MIN_VALUE_BYTES < MAX_VALUE_BYTES);
    }
}
