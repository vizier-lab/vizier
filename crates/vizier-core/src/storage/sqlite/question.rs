use crate::time::Timestamp;
use crate::storage::sql::{OptionalExtension, Row, params};
use anyhow::Result;
use chrono::DateTime;

use crate::{
    questions::model::{Answer, Claim, Controls, QuestionSet, QuestionSetRecord, QuestionState},
    schema::VizierSession,
    storage::{question::QuestionStorage, sqlite::SqlStorage},
};

/// The columns a record is read from, in the order [`row_to_record`] expects.
const COLUMNS: &str = "set_id, short_id, agent_id, channel_json, topic, state, questions, \
                       answers, answerer, controls, created_at, closed_at";

/// `channel` is the slug, for querying and for the delete that runs with the conversation;
/// `channel_json` is the enum itself, so a row reads back into the exact session it was written
/// for without a slug parser to get subtly wrong.
fn row_to_record(row: &Row) -> crate::storage::sql::Result<QuestionSetRecord> {
    let agent_id: String = row.get(2)?;
    let channel_json: String = row.get(3)?;
    let topic: Option<String> = row.get(4)?;
    let state: String = row.get(5)?;
    let questions: String = row.get(6)?;
    let answers: Option<String> = row.get(7)?;
    let controls: Option<String> = row.get(9)?;
    let created_at: i64 = row.get(10)?;
    let closed_at: Option<i64> = row.get(11)?;

    let channel = serde_json::from_str(&channel_json).map_err(|e| {
        crate::storage::sql::Error::FromSqlConversionFailure(
            3,
            crate::storage::sql::types::Type::Text,
            Box::new(e),
        )
    })?;

    Ok(QuestionSetRecord {
        set_id: row.get(0)?,
        short_id: row.get(1)?,
        session: VizierSession(agent_id, channel, topic),
        state: QuestionState::from_stored(&state),
        set: serde_json::from_str::<QuestionSet>(&questions).map_err(|e| {
            crate::storage::sql::Error::FromSqlConversionFailure(
                6,
                crate::storage::sql::types::Type::Text,
                Box::new(e),
            )
        })?,
        answers: answers.and_then(|text| serde_json::from_str(&text).ok()),
        answerer: row.get(8)?,
        controls: controls.and_then(|text| serde_json::from_str::<Controls>(&text).ok()),
        created_at: crate::time::from_millis_opt(created_at).unwrap_or_else(crate::time::now),
        closed_at: closed_at.and_then(crate::time::from_millis_opt),
    })
}

/// The predicate that scopes a row to one conversation. `IS NOT DISTINCT FROM` matches a NULL
/// topic, which is what a session without one stores.
const IN_SESSION: &str = "agent_id = ?2 AND channel = ?3 AND topic IS NOT DISTINCT FROM ?4";

fn millis(t: Timestamp) -> i64 {
    t.timestamp_millis()
}

/// Delete a conversation's question sets and sensitive asks, on a connection the caller already
/// holds (`delete_session`). Reference-only: nothing else points at them (FR-064).
pub(super) fn delete_session(
    conn: &crate::storage::sql::Connection,
    agent_id: &str,
    channel_slug: &str,
    topic: &str,
) -> Result<()> {
    for table in ["question_set", "sensitive_ask"] {
        conn.execute(
            &format!(
                "DELETE FROM {table} WHERE agent_id = ?1 AND channel = ?2 \
                 AND topic IS NOT DISTINCT FROM ?3"
            ),
            params![agent_id, channel_slug, topic],
        )?;
    }
    Ok(())
}

/// Delete everything an agent asked, with the agent (`delete_agent`).
pub(super) fn delete_agent(conn: &crate::storage::sql::Connection, agent_id: &str) -> Result<()> {
    for table in ["question_set", "sensitive_ask"] {
        conn.execute(
            &format!("DELETE FROM {table} WHERE agent_id = ?1"),
            params![agent_id],
        )?;
    }
    Ok(())
}

#[async_trait::async_trait]
impl QuestionStorage for SqlStorage {
    async fn create_question_set(&self, record: &QuestionSetRecord) -> Result<()> {
        let questions = serde_json::to_string(&record.set)?;
        let controls = record
            .controls
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        let channel_json = serde_json::to_string(&record.session.1)?;
        let conn = self.conn.lock()?;
        conn.execute(
            "INSERT INTO question_set
                (set_id, short_id, agent_id, channel, channel_json, topic, state, questions,
                 answers, answerer, controls, created_at, closed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, NULL, ?9, ?10, NULL)",
            params![
                record.set_id,
                record.short_id,
                record.session.0,
                record.session.1.to_slug(),
                channel_json,
                record.session.2,
                record.state.as_str(),
                questions,
                controls,
                millis(record.created_at),
            ],
        )?;
        Ok(())
    }

    async fn claim_question_set(
        &self,
        set_id: &str,
        session: &VizierSession,
        answers: &[Answer],
        answerer: &str,
    ) -> Result<Claim> {
        let answers = serde_json::to_string(answers)?;
        let conn = self.conn.lock()?;
        // One statement, so the winner is decided by the database and not by the caller: a row
        // that is no longer pending, or belongs to another conversation, changes nothing.
        let changed = conn.execute(
            &format!(
                "UPDATE question_set
                    SET state = 'answered', answers = ?1, answerer = ?5, closed_at = ?6
                  WHERE set_id = ?7 AND state = 'pending' AND {IN_SESSION}"
            ),
            params![
                answers,
                session.0,
                session.1.to_slug(),
                session.2,
                answerer,
                millis(crate::time::now()),
                set_id,
            ],
        )?;
        Ok(if changed > 0 {
            Claim::Won
        } else {
            Claim::AlreadyClosed
        })
    }

    async fn decline_question_set(
        &self,
        set_id: &str,
        session: &VizierSession,
        answerer: &str,
    ) -> Result<Claim> {
        let conn = self.conn.lock()?;
        let changed = conn.execute(
            &format!(
                "UPDATE question_set
                    SET state = 'declined', answerer = ?1, closed_at = ?5
                  WHERE set_id = ?6 AND state = 'pending' AND {IN_SESSION}"
            ),
            params![
                answerer,
                session.0,
                session.1.to_slug(),
                session.2,
                millis(crate::time::now()),
                set_id,
            ],
        )?;
        Ok(if changed > 0 {
            Claim::Won
        } else {
            Claim::AlreadyClosed
        })
    }

    async fn withdraw_question_set(&self, set_id: &str) -> Result<bool> {
        let conn = self.conn.lock()?;
        let changed = conn.execute(
            "UPDATE question_set SET state = 'withdrawn', closed_at = ?1
              WHERE set_id = ?2 AND state = 'pending'",
            params![millis(crate::time::now()), set_id],
        )?;
        Ok(changed > 0)
    }

    async fn set_question_controls(&self, set_id: &str, controls: &Controls) -> Result<()> {
        let controls = serde_json::to_string(controls)?;
        let conn = self.conn.lock()?;
        conn.execute(
            "UPDATE question_set SET controls = ?1 WHERE set_id = ?2",
            params![controls, set_id],
        )?;
        Ok(())
    }

    async fn get_question_set(&self, set_id: &str) -> Result<Option<QuestionSetRecord>> {
        let conn = self.conn.lock()?;
        let record = conn
            .query_row(
                &format!("SELECT {COLUMNS} FROM question_set WHERE set_id = ?1"),
                [set_id],
                row_to_record,
            )
            .optional()?;
        Ok(record)
    }

    async fn question_set_by_short_id(&self, short_id: &str) -> Result<Option<QuestionSetRecord>> {
        let conn = self.conn.lock()?;
        let record = conn
            .query_row(
                &format!("SELECT {COLUMNS} FROM question_set WHERE short_id = ?1"),
                [short_id],
                row_to_record,
            )
            .optional()?;
        Ok(record)
    }

    async fn open_question_sets(&self, session: &VizierSession) -> Result<Vec<QuestionSetRecord>> {
        let conn = self.conn.lock()?;
        // Its own predicate, so the numbering starts at ?1 rather than sharing `IN_SESSION`'s.
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM question_set
              WHERE state = 'pending' AND agent_id = ?1 AND channel = ?2
                AND topic IS NOT DISTINCT FROM ?3
              ORDER BY created_at, rowid"
        ))?;
        let records = stmt
            .query_map(
                params![session.0, session.1.to_slug(), session.2],
                row_to_record,
            )?
            .collect::<crate::storage::sql::Result<Vec<_>>>()?;
        Ok(records)
    }

    async fn open_question_sets_all(&self) -> Result<Vec<QuestionSetRecord>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM question_set WHERE state = 'pending' ORDER BY created_at, rowid"
        ))?;
        let records = stmt
            .query_map([], row_to_record)?
            .collect::<crate::storage::sql::Result<Vec<_>>>()?;
        Ok(records)
    }
}

/// The columns a sensitive ask is read from, matching [`ask_row`].
const ASK_COLUMNS: &str = "ask_id, agent_id, channel_json, topic, purpose, label, state, \
                           requested_at, closed_at";

fn ask_row(row: &Row) -> crate::storage::sql::Result<crate::questions::model::SensitiveAskRecord> {
    use crate::questions::model::{AskState, SensitiveAskRecord};

    let agent_id: String = row.get(1)?;
    let channel_json: String = row.get(2)?;
    let topic: Option<String> = row.get(3)?;
    let state: String = row.get(6)?;
    let requested_at: i64 = row.get(7)?;
    let closed_at: Option<i64> = row.get(8)?;

    let channel = serde_json::from_str(&channel_json).map_err(|e| {
        crate::storage::sql::Error::FromSqlConversionFailure(
            2,
            crate::storage::sql::types::Type::Text,
            Box::new(e),
        )
    })?;

    Ok(SensitiveAskRecord {
        ask_id: row.get(0)?,
        purpose: row.get(4)?,
        label: row.get(5)?,
        session: VizierSession(agent_id, channel, topic),
        state: AskState::from_stored(&state),
        requested_at: crate::time::from_millis_opt(requested_at).unwrap_or_else(crate::time::now),
        closed_at: closed_at.and_then(crate::time::from_millis_opt),
    })
}

#[async_trait::async_trait]
impl crate::storage::question::SensitiveAskStorage for SqlStorage {
    async fn open_sensitive_ask(
        &self,
        record: &crate::questions::model::SensitiveAskRecord,
    ) -> Result<()> {
        let channel_json = serde_json::to_string(&record.session.1)?;
        let conn = self.conn.lock()?;
        conn.execute(
            "INSERT INTO sensitive_ask
                (ask_id, agent_id, channel, channel_json, topic, purpose, label, state,
                 requested_at, closed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL)",
            params![
                record.ask_id,
                record.session.0,
                record.session.1.to_slug(),
                channel_json,
                record.session.2,
                record.purpose,
                record.label,
                record.state.as_str(),
                millis(record.requested_at),
            ],
        )?;
        Ok(())
    }

    async fn close_sensitive_ask(
        &self,
        ask_id: &str,
        state: crate::questions::model::AskState,
    ) -> Result<bool> {
        let conn = self.conn.lock()?;
        // Only an open ask changes: a late submission after a timeout, or a second tab's answer,
        // finds nothing to update (FR-049).
        let changed = conn.execute(
            "UPDATE sensitive_ask SET state = ?1, closed_at = ?2
              WHERE ask_id = ?3 AND state = 'open'",
            params![state.as_str(), millis(crate::time::now()), ask_id],
        )?;
        Ok(changed > 0)
    }

    async fn get_sensitive_ask(
        &self,
        ask_id: &str,
    ) -> Result<Option<crate::questions::model::SensitiveAskRecord>> {
        let conn = self.conn.lock()?;
        Ok(conn
            .query_row(
                &format!("SELECT {ASK_COLUMNS} FROM sensitive_ask WHERE ask_id = ?1"),
                [ask_id],
                ask_row,
            )
            .optional()?)
    }

    async fn sensitive_asks(
        &self,
        session: &VizierSession,
        state: crate::questions::model::AskState,
    ) -> Result<Vec<crate::questions::model::SensitiveAskRecord>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {ASK_COLUMNS} FROM sensitive_ask
              WHERE state = ?1 AND agent_id = ?2 AND channel = ?3
                AND topic IS NOT DISTINCT FROM ?4
              ORDER BY requested_at, rowid"
        ))?;
        let records = stmt
            .query_map(
                params![state.as_str(), session.0, session.1.to_slug(), session.2],
                ask_row,
            )?
            .collect::<crate::storage::sql::Result<Vec<_>>>()?;
        Ok(records)
    }

    async fn open_sensitive_asks_all(
        &self,
    ) -> Result<Vec<crate::questions::model::SensitiveAskRecord>> {
        let conn = self.conn.lock()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {ASK_COLUMNS} FROM sensitive_ask WHERE state = 'open' ORDER BY requested_at"
        ))?;
        let records = stmt
            .query_map([], ask_row)?
            .collect::<crate::storage::sql::Result<Vec<_>>>()?;
        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rusqlite::Connection;

    use super::*;
    use crate::{
        questions::model::{AnswerState, AnswerValue, ChoiceOption, Question, QuestionType},
        schema::VizierChannelId,
        storage::document::BlobDocumentStore,
    };

    fn setup() -> (SqlStorage, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::tests_support::test_store(|| {
            let conn = Connection::open_in_memory().unwrap();
            crate::storage::sqlite::init_question_schema(&conn).unwrap();
            crate::storage::sqlite::init_sensitive_ask_schema(&conn).unwrap();
            crate::storage::sql::SqlStore::sqlite(conn)
        });
        let storage = SqlStorage::new(
            store,
            Arc::new(BlobDocumentStore::local(dir.path().to_path_buf())),
        );
        (storage, dir)
    }

    fn session() -> VizierSession {
        VizierSession(
            "agent-1".into(),
            VizierChannelId::HTTP("alice".into(), "webui".into()),
            Some("t".into()),
        )
    }

    fn other_session() -> VizierSession {
        VizierSession(
            "agent-1".into(),
            VizierChannelId::HTTP("alice".into(), "webui".into()),
            Some("other".into()),
        )
    }

    fn record(set_id: &str, session: VizierSession) -> QuestionSetRecord {
        QuestionSetRecord {
            set_id: set_id.into(),
            short_id: format!("short{set_id}"),
            session,
            state: QuestionState::Pending,
            set: QuestionSet {
                title: Some("Pick".into()),
                description: None,
                submit_label: None,
                dismiss_label: None,
                questions: vec![Question {
                    id: "db".into(),
                    prompt: "Database".into(),
                    kind: QuestionType::Choice,
                    help: None,
                    optional: false,
                    default: None,
                    options: vec![ChoiceOption {
                        value: "sqlite".into(),
                        label: "SQLite".into(),
                        description: None,
                    }],
                    placeholder: None,
                    max_length: None,
                    multiline: false,
                    min_selections: None,
                    max_selections: None,
                    yes_label: None,
                    no_label: None,
                }],
            },
            answers: None,
            answerer: None,
            controls: None,
            created_at: crate::time::now(),
            closed_at: None,
        }
    }

    fn answer() -> Answer {
        Answer {
            question_id: "db".into(),
            state: AnswerState::Answered,
            value: Some(AnswerValue::Text("sqlite".into())),
            label: Some("SQLite".into()),
        }
    }

    /// The claim is the one place a race is decided: two submissions of one set, exactly one wins
    /// and the loser is told the set is closed (FR-017, SC-010).
    #[tokio::test]
    async fn two_submissions_of_one_set_leave_exactly_one_winner() {
        let (storage, _dir) = setup();
        storage
            .create_question_set(&record("s1", session()))
            .await
            .unwrap();

        let first = storage
            .claim_question_set("s1", &session(), &[answer()], "alice")
            .await
            .unwrap();
        let second = storage
            .claim_question_set("s1", &session(), &[answer()], "alice")
            .await
            .unwrap();

        assert_eq!(first, Claim::Won);
        assert_eq!(second, Claim::AlreadyClosed);

        let after = storage.get_question_set("s1").await.unwrap().unwrap();
        assert_eq!(after.state, QuestionState::Answered);
        assert_eq!(after.answerer.as_deref(), Some("alice"));
        assert_eq!(after.answers.as_ref().map(Vec::len), Some(1));
        assert!(after.closed_at.is_some());
    }

    /// A set belongs to its conversation: another conversation's session cannot claim it, which is
    /// the storage half of "no answer from someone who cannot access the conversation" (FR-018).
    #[tokio::test]
    async fn a_set_cannot_be_claimed_from_another_conversation() {
        let (storage, _dir) = setup();
        storage
            .create_question_set(&record("s1", session()))
            .await
            .unwrap();

        assert_eq!(
            storage
                .claim_question_set("s1", &other_session(), &[answer()], "mallory")
                .await
                .unwrap(),
            Claim::AlreadyClosed
        );
        // And it is still answerable in its own conversation.
        assert_eq!(
            storage
                .claim_question_set("s1", &session(), &[answer()], "alice")
                .await
                .unwrap(),
            Claim::Won
        );
    }

    /// An id that names no set claims nothing and writes nothing (FR-017, T015).
    #[tokio::test]
    async fn an_unknown_set_id_is_refused() {
        let (storage, _dir) = setup();
        assert_eq!(
            storage
                .claim_question_set("nope", &session(), &[answer()], "alice")
                .await
                .unwrap(),
            Claim::AlreadyClosed
        );
        assert!(storage.get_question_set("nope").await.unwrap().is_none());
    }

    /// A dismissal closes the set with no answers, and an answered set cannot then be declined
    /// (FR-014, FR-017).
    #[tokio::test]
    async fn a_dismissal_closes_the_set_without_answers() {
        let (storage, _dir) = setup();
        storage
            .create_question_set(&record("s1", session()))
            .await
            .unwrap();

        assert_eq!(
            storage
                .decline_question_set("s1", &session(), "alice")
                .await
                .unwrap(),
            Claim::Won
        );
        let after = storage.get_question_set("s1").await.unwrap().unwrap();
        assert_eq!(after.state, QuestionState::Declined);
        assert!(after.answers.is_none());
        assert_eq!(after.answerer.as_deref(), Some("alice"));

        assert_eq!(
            storage
                .claim_question_set("s1", &session(), &[answer()], "alice")
                .await
                .unwrap(),
            Claim::AlreadyClosed
        );
    }

    /// A set asked before a restart is still answerable afterwards, because everything a control
    /// needs is in the row — not in the process (FR-025, SC-011).
    #[tokio::test]
    async fn a_pending_set_is_found_by_its_short_id_and_survives_a_rebuild() {
        let (storage, _dir) = setup();
        storage
            .create_question_set(&record("s1", session()))
            .await
            .unwrap();

        let by_short = storage
            .question_set_by_short_id("shorts1")
            .await
            .unwrap()
            .expect("a pending set resolves from its short id");
        assert_eq!(by_short.set_id, "s1");
        assert_eq!(by_short.state, QuestionState::Pending);
        // The whole conversation reads back, so the answer lands in the right one.
        assert_eq!(by_short.session, session());
        assert_eq!(by_short.set.questions.len(), 1);

        assert!(
            storage
                .question_set_by_short_id("nope")
                .await
                .unwrap()
                .is_none()
        );
    }

    /// A short id is unique across every conversation, because a control carries nothing else to
    /// disambiguate it with.
    #[tokio::test]
    async fn a_short_id_cannot_be_reused() {
        let (storage, _dir) = setup();
        let mut first = record("s1", session());
        first.short_id = "7f3a91c2".into();
        storage.create_question_set(&first).await.unwrap();

        let mut second = record("s2", other_session());
        second.short_id = "7f3a91c2".into();
        assert!(storage.create_question_set(&second).await.is_err());
    }

    /// A script that fails after asking takes its set with it: it must not stay answerable
    /// (FR-010). A set already answered keeps its answer.
    #[tokio::test]
    async fn withdrawing_takes_an_unanswered_set_away() {
        let (storage, _dir) = setup();
        storage
            .create_question_set(&record("s1", session()))
            .await
            .unwrap();

        assert!(storage.withdraw_question_set("s1").await.unwrap());
        assert_eq!(
            storage.get_question_set("s1").await.unwrap().unwrap().state,
            QuestionState::Withdrawn
        );
        // Withdrawing again changes nothing.
        assert!(!storage.withdraw_question_set("s1").await.unwrap());

        storage
            .create_question_set(&record("s2", session()))
            .await
            .unwrap();
        storage
            .claim_question_set("s2", &session(), &[answer()], "alice")
            .await
            .unwrap();
        assert!(!storage.withdraw_question_set("s2").await.unwrap());
        assert_eq!(
            storage.get_question_set("s2").await.unwrap().unwrap().state,
            QuestionState::Answered
        );
    }

    /// Only open sets are listed, and only for their own conversation: the WebUI replays what is
    /// answerable and nothing else.
    #[tokio::test]
    async fn only_open_sets_of_one_conversation_are_listed() {
        let (storage, _dir) = setup();
        storage
            .create_question_set(&record("s1", session()))
            .await
            .unwrap();
        storage
            .create_question_set(&record("s2", session()))
            .await
            .unwrap();
        storage
            .create_question_set(&record("s3", other_session()))
            .await
            .unwrap();
        storage
            .claim_question_set("s2", &session(), &[answer()], "alice")
            .await
            .unwrap();

        let open = storage.open_question_sets(&session()).await.unwrap();
        assert_eq!(
            open.iter().map(|r| r.set_id.as_str()).collect::<Vec<_>>(),
            ["s1"]
        );
        assert_eq!(storage.open_question_sets_all().await.unwrap().len(), 2);
    }

    /// Where the platform posted a set is remembered, so it can be edited or stripped when the
    /// set closes (FR-024).
    #[tokio::test]
    async fn controls_are_stored_with_the_set() {
        let (storage, _dir) = setup();
        storage
            .create_question_set(&record("s1", session()))
            .await
            .unwrap();

        let controls = Controls {
            platform: "discord".into(),
            messages: vec![crate::questions::model::ControlMessage {
                chat_id: "1".into(),
                message_id: "2".into(),
                question: None,
            }],
        };
        storage
            .set_question_controls("s1", &controls)
            .await
            .unwrap();

        let after = storage.get_question_set("s1").await.unwrap().unwrap();
        assert_eq!(after.controls, Some(controls));
    }

    /// A conversation's sets and asks go with the conversation, and an agent's with the agent;
    /// another conversation's are left alone (FR-064).
    #[test]
    fn sets_and_asks_are_deleted_with_their_conversation_and_agent() {
        use crate::storage::question::SensitiveAskStorage;
        let (storage, _dir) = setup();
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        rt.block_on(async {
            storage
                .create_question_set(&record("keep", other_session()))
                .await
                .unwrap();
            storage
                .create_question_set(&record("gone", session()))
                .await
                .unwrap();
        });
        let ask = |id: &str, s: &VizierSession| crate::questions::model::SensitiveAskRecord {
            ask_id: id.into(),
            purpose: "p".into(),
            label: None,
            session: s.clone(),
            state: crate::questions::model::AskState::Open,
            requested_at: crate::time::now(),
            closed_at: None,
        };
        rt.block_on(async {
            storage
                .open_sensitive_ask(&ask("a-keep", &other_session()))
                .await
                .unwrap();
            storage
                .open_sensitive_ask(&ask("a-gone", &session()))
                .await
                .unwrap();
        });

        let s = session();
        {
            let conn = storage.conn.lock().unwrap();
            delete_session(&conn, &s.0, &s.1.to_slug(), s.2.as_deref().unwrap()).unwrap();
        }
        rt.block_on(async {
            assert!(storage.get_question_set("gone").await.unwrap().is_none());
            assert!(storage.get_question_set("keep").await.unwrap().is_some());
            assert_eq!(storage.open_sensitive_asks_all().await.unwrap().len(), 1);
        });

        delete_agent(&storage.conn.lock().unwrap(), "agent-1").unwrap();
        rt.block_on(async {
            assert!(storage.get_question_set("keep").await.unwrap().is_none());
            assert!(storage.open_sensitive_asks_all().await.unwrap().is_empty());
        });
    }

    /// A pending card is still there, still pending and still answerable after the process that
    /// asked has gone and a new one opens the same database; and a history entry written while it
    /// was pending reads back with the state the table now holds (FR-016, FR-025).
    #[tokio::test]
    async fn a_pending_set_and_its_card_survive_a_restart() {
        use crate::schema::SessionHistory;

        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("vizier.db");
        let open = || {
            let conn = Connection::open(&db).unwrap();
            crate::storage::sqlite::init_question_schema(&conn).unwrap();
            SqlStorage::new(
                crate::storage::sql::SqlStore::sqlite(conn),
                Arc::new(BlobDocumentStore::local(dir.path().to_path_buf())),
            )
        };

        // The process that asked.
        let asked = open();
        asked
            .create_question_set(&record("s1", session()))
            .await
            .unwrap();
        let card = || SessionHistory {
            // The history row's own id, which is not the set's.
            uid: "history-row-uid".into(),
            vizier_session: session(),
            content: crate::schema::SessionHistoryContent::QuestionSet {
                set_id: "s1".into(),
                title: None,
                description: None,
                questions: vec![],
                submit_label: None,
                dismiss_label: None,
                state: QuestionState::Pending,
                answers: None,
                answerer: None,
            },
            timestamp: crate::time::now(),
            reactions: vec![],
            seq: None,
        };
        drop(asked);

        // A new process finds it, shows it pending, and the person answers it.
        let restarted = open();
        let found = restarted
            .question_set_by_short_id("shorts1")
            .await
            .unwrap()
            .expect("still there");
        assert_eq!(found.state, QuestionState::Pending);
        let mut cards = [card()];
        super::super::history::fill_question_sets(&restarted.conn.lock().unwrap(), &mut cards)
            .unwrap();
        let crate::schema::SessionHistoryContent::QuestionSet { state, .. } = &cards[0].content
        else {
            panic!()
        };
        assert_eq!(
            *state,
            QuestionState::Pending,
            "a pending card must not read as withdrawn"
        );

        assert_eq!(
            restarted
                .claim_question_set("s1", &session(), &[answer()], "alice")
                .await
                .unwrap(),
            Claim::Won
        );
        let mut cards = [card()];
        super::super::history::fill_question_sets(&restarted.conn.lock().unwrap(), &mut cards)
            .unwrap();
        let crate::schema::SessionHistoryContent::QuestionSet {
            state,
            answers,
            answerer,
            ..
        } = &cards[0].content
        else {
            panic!()
        };
        assert_eq!(*state, QuestionState::Answered);
        assert_eq!(answerer.as_deref(), Some("alice"));
        assert_eq!(answers.as_ref().map(Vec::len), Some(1));
    }

    crate::postgres_tests!(
        two_submissions_of_one_set_leave_exactly_one_winner,
        a_set_cannot_be_claimed_from_another_conversation,
        an_unknown_set_id_is_refused,
        a_dismissal_closes_the_set_without_answers,
        a_pending_set_is_found_by_its_short_id_and_survives_a_rebuild,
        a_short_id_cannot_be_reused,
        withdrawing_takes_an_unanswered_set_away,
        only_open_sets_of_one_conversation_are_listed,
        controls_are_stored_with_the_set,
        sets_and_asks_are_deleted_with_their_conversation_and_agent,
    );
}
