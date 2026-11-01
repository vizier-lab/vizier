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
