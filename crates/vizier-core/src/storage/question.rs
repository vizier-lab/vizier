//! The question sets an agent asked, and what became of them
//! (`specs/042-agent-asking-question` research R3).
//!
//! The set is written once, when it is presented; only its state, answer and controls change
//! afterwards. That split is why the history entry carries the ask and this table carries the
//! answer: the entry is filled from here on every read, so a reload, a second tab and a restart
//! all show the same card without anything being rewritten (FR-016).
//!
//! The claim is the one place a race is decided: two tabs, two channels and a dismissal all go
//! through it, and exactly one of them changes a row.

use anyhow::Result;

use crate::{
    questions::model::{Answer, Claim, Controls, QuestionSetRecord},
    schema::VizierSession,
    storage::VizierStorage,
};

#[async_trait::async_trait]
pub trait QuestionStorage {
    /// Save the set as `pending`, when it is presented.
    async fn create_question_set(&self, record: &QuestionSetRecord) -> Result<()>;

    /// Record the answers, if the set is still `pending` in this conversation. `AlreadyClosed`
    /// means somebody else got there first, or the script withdrew it: nothing was written and no
    /// turn should start (FR-017).
    async fn claim_question_set(
        &self,
        set_id: &str,
        session: &VizierSession,
        answers: &[Answer],
        answerer: &str,
    ) -> Result<Claim>;

    /// Close the set as dismissed, if it is still `pending` in this conversation (FR-014).
    async fn decline_question_set(
        &self,
        set_id: &str,
        session: &VizierSession,
        answerer: &str,
    ) -> Result<Claim>;

    /// Take the set away: the script that asked then failed, so it must not stay answerable
    /// (FR-010). A set already answered keeps its answer.
    async fn withdraw_question_set(&self, set_id: &str) -> Result<bool>;

    /// Remember where the platform posted it, so it can be edited or stripped later (FR-024).
    async fn set_question_controls(&self, set_id: &str, controls: &Controls) -> Result<()>;

    /// One set by id.
    async fn get_question_set(&self, set_id: &str) -> Result<Option<QuestionSetRecord>>;

    /// One set by the short reference a platform control carries (FR-025).
    async fn question_set_by_short_id(&self, short_id: &str) -> Result<Option<QuestionSetRecord>>;

    /// The sets of one conversation still open, oldest first. Used to replay a pending card to a
    /// socket that has just opened.
    async fn open_question_sets(&self, session: &VizierSession) -> Result<Vec<QuestionSetRecord>>;

    /// Every open set in the database: startup has no conversation to scope to.
    async fn open_question_sets_all(&self) -> Result<Vec<QuestionSetRecord>>;
}

#[async_trait::async_trait]
impl QuestionStorage for VizierStorage {
    async fn create_question_set(&self, record: &QuestionSetRecord) -> Result<()> {
        self.0.create_question_set(record).await
    }

    async fn claim_question_set(
        &self,
        set_id: &str,
        session: &VizierSession,
        answers: &[Answer],
        answerer: &str,
    ) -> Result<Claim> {
        self.0
            .claim_question_set(set_id, session, answers, answerer)
            .await
    }

    async fn decline_question_set(
        &self,
        set_id: &str,
        session: &VizierSession,
        answerer: &str,
    ) -> Result<Claim> {
        self.0.decline_question_set(set_id, session, answerer).await
    }

    async fn withdraw_question_set(&self, set_id: &str) -> Result<bool> {
        self.0.withdraw_question_set(set_id).await
    }

    async fn set_question_controls(&self, set_id: &str, controls: &Controls) -> Result<()> {
        self.0.set_question_controls(set_id, controls).await
    }

    async fn get_question_set(&self, set_id: &str) -> Result<Option<QuestionSetRecord>> {
        self.0.get_question_set(set_id).await
    }

    async fn question_set_by_short_id(&self, short_id: &str) -> Result<Option<QuestionSetRecord>> {
        self.0.question_set_by_short_id(short_id).await
    }

    async fn open_question_sets(&self, session: &VizierSession) -> Result<Vec<QuestionSetRecord>> {
        self.0.open_question_sets(session).await
    }

    async fn open_question_sets_all(&self) -> Result<Vec<QuestionSetRecord>> {
        self.0.open_question_sets_all().await
    }
}

/// The sensitive asks of one conversation (`specs/042-agent-asking-question` research R9).
///
/// A row records that a value was asked for and what came of it, never the value (FR-044). It is
/// what the transcript shows and what a socket replays when it opens, so a dialog survives a
/// reload without the value ever existing anywhere but in the running tool.
#[async_trait::async_trait]
pub trait SensitiveAskStorage {
    /// Record an ask as `open`, when the dialog goes up.
    ///
    /// Fenced: refused with `OwnershipLost` when a newer owner holds `agent:{agent_id}`
    /// (specs/057 contracts/ownership.md).
    async fn open_sensitive_ask(&self, record: &crate::questions::model::SensitiveAskRecord)
    -> Result<()>;

    /// Close an open ask with `state`. Only an `open` row changes, so a late submission after a
    /// timeout cannot reopen or rewrite one (FR-049). Returns whether this call closed it.
    ///
    /// Fenced: refused with `OwnershipLost` when a newer owner holds `agent:{agent_id}`
    /// (specs/057 contracts/ownership.md).
    async fn close_sensitive_ask(
        &self,
        ask_id: &str,
        state: crate::questions::model::AskState,
    ) -> Result<bool>;

    /// One ask by id.
    async fn get_sensitive_ask(
        &self,
        ask_id: &str,
    ) -> Result<Option<crate::questions::model::SensitiveAskRecord>>;

    /// The asks of a conversation with this state, oldest first.
    async fn sensitive_asks(
        &self,
        session: &VizierSession,
        state: crate::questions::model::AskState,
    ) -> Result<Vec<crate::questions::model::SensitiveAskRecord>>;

    /// Every open ask in the database: startup has no conversation to scope to (research R9).
    async fn open_sensitive_asks_all(&self) -> Result<Vec<crate::questions::model::SensitiveAskRecord>>;

    /// Recovery of `agent:{agent_id}`: its `open` asks whose owner is older than `below` (or null)
    /// become `abandoned` and are re-owned by the held fence, in one statement. Returns how many.
    ///
    /// Fenced: refused with `OwnershipLost` when a newer owner holds `agent:{agent_id}`.
    async fn abandon_open_sensitive_asks(&self, agent_id: &str, below: u64) -> Result<usize>;
}

#[async_trait::async_trait]
impl SensitiveAskStorage for VizierStorage {
    async fn open_sensitive_ask(
        &self,
        record: &crate::questions::model::SensitiveAskRecord,
    ) -> Result<()> {
        self.0.open_sensitive_ask(record).await
    }

    async fn close_sensitive_ask(
        &self,
        ask_id: &str,
        state: crate::questions::model::AskState,
    ) -> Result<bool> {
        self.0.close_sensitive_ask(ask_id, state).await
    }

    async fn get_sensitive_ask(
        &self,
        ask_id: &str,
    ) -> Result<Option<crate::questions::model::SensitiveAskRecord>> {
        self.0.get_sensitive_ask(ask_id).await
    }

    async fn sensitive_asks(
        &self,
        session: &VizierSession,
        state: crate::questions::model::AskState,
    ) -> Result<Vec<crate::questions::model::SensitiveAskRecord>> {
        self.0.sensitive_asks(session, state).await
    }

    async fn abandon_open_sensitive_asks(&self, agent_id: &str, below: u64) -> Result<usize> {
        self.0.abandon_open_sensitive_asks(agent_id, below).await
    }

    async fn open_sensitive_asks_all(
        &self,
    ) -> Result<Vec<crate::questions::model::SensitiveAskRecord>> {
        self.0.open_sensitive_asks_all().await
    }
}
