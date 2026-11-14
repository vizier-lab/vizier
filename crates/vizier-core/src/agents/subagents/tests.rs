use super::*;
use crate::schema::subagent::tests_support::row;

#[test]
fn timeouts_and_texts_are_validated() {
    assert_eq!(validate_timeout(None).unwrap(), DEFAULT_TIMEOUT_SECS);
    assert_eq!(validate_timeout(Some(3)).unwrap(), 3);
    assert!(validate_timeout(Some(0)).is_err());
    assert!(validate_timeout(Some(MAX_TIMEOUT_SECS + 1)).is_err());
    assert!(validate_text("a task", "  \n", MAX_TASK_BYTES).is_err());
    assert!(validate_text("a task", &"x".repeat(MAX_TASK_BYTES + 1), MAX_TASK_BYTES).is_err());
    assert!(validate_text("a task", "do it", MAX_TASK_BYTES).is_ok());
}

#[test]
fn an_answer_is_rendered_with_its_result() {
    let mut worker = row(SubagentState::Answered);
    worker.result = Some("41 invoices".into());
    let text = render_update(&worker);
    assert!(text.starts_with("[Subagent update] s_1 — task 1 of 2 \"do it\""), "{text}");
    assert!(text.contains("state: answered"));
    assert!(text.contains("result: 41 invoices"));
    assert!(!text.contains("get_subagent"));
}

#[test]
fn a_long_answer_points_at_the_whole_one() {
    let mut worker = row(SubagentState::Answered);
    worker.result = Some("x".repeat(5_000));
    let text = render_update(&worker);
    assert!(text.contains("[cut]"));
    assert!(text.contains("get_subagent(id=\"s_1\")"));
}

#[test]
fn a_question_says_how_to_reply() {
    let mut worker = row(SubagentState::Asking);
    worker.question = Some("EU or US?".into());
    let text = render_update(&worker);
    assert!(text.contains("state: asking"));
    assert!(text.contains("question: EU or US?"));
    assert!(text.contains("reply_subagent(id=\"s_1\""));
}

#[test]
fn a_failure_or_timeout_carries_its_error() {
    for state in [SubagentState::Failed, SubagentState::TimedOut] {
        let mut worker = row(state);
        worker.error = Some("boom".into());
        let text = render_update(&worker);
        assert!(text.contains(&format!("state: {}", state.as_str())));
        assert!(text.contains("error: boom"));
    }
}

#[test]
fn a_long_task_title_is_cut_on_one_line() {
    let mut worker = row(SubagentState::Answered);
    worker.task = format!("line one\n{}", "y".repeat(300));
    let first_line = render_update(&worker).lines().next().unwrap().to_string();
    assert!(first_line.contains("[cut]"), "{first_line}");
}

#[test]
fn refusals_are_status_data() {
    let unknown = Refusal::UnknownSubagent("s_9".into()).to_json();
    assert_eq!(unknown["status"], "unknown_subagent");
    let not_asking = Refusal::NotAsking { id: "s_1".into(), state: SubagentState::Answered }.to_json();
    assert_eq!(not_asking["status"], "not_asking");
    assert!(not_asking["message"].as_str().unwrap().contains("answered"));
}

#[test]
fn mid_turn_frames_are_not_terminal() {
    use crate::schema::VizierResponseContent;
    for frame in [
        VizierResponseContent::ThinkingStart,
        VizierResponseContent::Thinking("hmm".into()),
        VizierResponseContent::ToolChoice { name: "t".into(), args: serde_json::json!({}) },
        VizierResponseContent::ToolResponse { response: serde_json::json!("ok") },
        VizierResponseContent::Checkpoint { handover: None },
    ] {
        assert_eq!(classify(&frame), None, "{frame:?}");
    }
    assert_eq!(
        classify(&VizierResponseContent::Message { content: "hi".into(), stats: None }),
        Some(PieceOutcome::Answered("hi".into()))
    );
}

// --- The service, on an in-memory store and an in-process bus -------------------------------

mod service {
    use std::sync::Arc;
    use std::time::Duration;

    use super::super::*;
    use crate::{
        agents::delivery::{DeliverySource, Deliveries},
        schema::{VizierChannelId, VizierSession},
        storage::sqlite::SqlStorage,
        transport::Inbox,
    };

    struct Fixture {
        subagents: Subagents,
        inbox: Inbox,
        _dir: tempfile::TempDir,
    }

    async fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        SqlStorage::init_schema(&conn).unwrap();
        let storage = Arc::new(VizierStorage::new(SqlStorage::new(
            crate::storage::sql::SqlStore::sqlite(conn),
            Arc::new(crate::storage::document::BlobDocumentStore::local(dir.path().to_path_buf())),
        )));
        let transport = VizierTransport::new();
        let inbox = transport.register_agent("agent-1".into()).await;
        Fixture { subagents: Subagents::new(storage, transport), inbox, _dir: dir }
    }

    fn chat() -> VizierSession {
        VizierSession(
            "agent-1".into(),
            VizierChannelId::HTTP("dani".into(), "chat".into()),
            Some("t".into()),
        )
    }

    fn ctx(session: VizierSession, wait_secs: u64) -> ToolContext {
        ToolContext {
            loaded_images: Default::default(),
            recalled: Default::default(),
            client: None,
            questions: crate::questions::QuestionsHandle::new(Default::default()),
            interactive: true,
            session,
            pending_attachments: Default::default(),
            hooks: None,
            background_depth: 0,
            subagent_depth: 0,
            // The wait returns 2 s (its grace) before this.
            deadline: Some(tokio::time::Instant::now() + Duration::from_secs(2 + wait_secs)),
            dream_sources: vec![],
            secret_redactor: Arc::new(crate::secrets::SecretRedactor::default()),
            project: None,
            detached: false,
            parent_detached_id: None,
        }
    }

    /// The next request of `kind` in the agent's queue, skipping the others (pokes, aborts).
    async fn next_of(inbox: &Inbox, kind: &str) -> crate::transport::VizierRequestEnvelope {
        loop {
            let envelope = tokio::time::timeout(Duration::from_secs(5), inbox.recv_request())
                .await
                .unwrap_or_else(|_| panic!("a {kind} request"))
                .unwrap();
            let got = match &envelope.request.content {
                VizierRequestContent::Command(cmd) => cmd.as_str(),
                other => other.kind(),
            };
            if got == kind {
                return envelope;
            }
        }
    }

    async fn reply(f: &Fixture, worker: &Subagent, text: &str) {
        f.subagents
            .turn_ended(&worker.worker_session(), TurnEnd::Replied(text.into()))
            .await;
    }

    #[tokio::test]
    async fn spawn_returns_handles_at_once_and_starts_each_worker() {
        let f = fixture().await;
        let hirer = ctx(chat(), 1);
        let workers = f.subagents.spawn(&hirer, vec!["one".into(), "two".into()], 600).await.unwrap();
        assert_eq!(workers.iter().map(|w| w.position).collect::<Vec<_>>(), vec![1, 2]);
        assert!(workers.iter().all(|w| w.state == SubagentState::Running && w.group_size == 2));
        assert_eq!(workers[0].group_id, workers[1].group_id);
        assert_eq!(f.subagents.timer_count(), 2);

        for expected in &workers {
            let envelope = next_of(&f.inbox, "subagent_task").await;
            assert_eq!(envelope.session, expected.worker_session());
            assert_eq!(envelope.request.subagent_depth, 1);
            assert_eq!(envelope.request.background_depth, 1);
            assert!(!envelope.request.transient, "a worker's turn is durable");
            let VizierRequestContent::SubagentTask(brief) = &envelope.request.content else {
                unreachable!()
            };
            assert_eq!(brief.subagent_id, expected.id);
            assert_eq!(brief.text, expected.task);
        }
        assert!(f.subagents.spawn(&hirer, vec![], 600).await.is_err());
        assert!(f.subagents.spawn(&hirer, vec!["  ".into()], 600).await.is_err());
    }

    #[tokio::test]
    async fn wait_for_any_then_all_and_a_timed_out_wait_leaves_workers_running() {
        let f = fixture().await;
        let hirer = ctx(chat(), 1);
        let workers = f.subagents.spawn(&hirer, vec!["a".into(), "b".into()], 600).await.unwrap();
        let ids: Vec<String> = workers.iter().map(|w| w.id.clone()).collect();

        let waited = f.subagents.wait(&hirer, &ids, Until::Any).await.unwrap().unwrap();
        assert!(!waited.met, "nothing settled within the call's time");
        assert!(waited.workers.iter().all(|w| w.state == SubagentState::Running));

        // One settles while a wait is in flight: the frame wakes it.
        let subagents = f.subagents.clone();
        let first = workers[0].clone();
        let settle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            subagents.turn_ended(&first.worker_session(), TurnEnd::Replied("A".into())).await;
        });
        let waited = f.subagents.wait(&ctx(chat(), 5), &ids, Until::Any).await.unwrap().unwrap();
        settle.await.unwrap();
        assert!(waited.met);
        assert_eq!(waited.workers[0].state, SubagentState::Answered);
        assert_eq!(waited.workers[0].result.as_deref(), Some("A"));

        let waited = f.subagents.wait(&hirer, &ids, Until::All).await.unwrap().unwrap();
        assert!(!waited.met);
        reply(&f, &workers[1], "B").await;
        let waited = f.subagents.wait(&hirer, &ids, Until::All).await.unwrap().unwrap();
        assert!(waited.met);

        // Both outcomes were returned by a wait, so neither is delivered.
        assert!(f.subagents.claim(&chat()).await.is_empty());
    }

    #[tokio::test]
    async fn stop_one_leaves_the_others_running() {
        let f = fixture().await;
        let hirer = ctx(chat(), 1);
        let workers = f.subagents.spawn(&hirer, vec!["a".into(), "b".into()], 600).await.unwrap();
        let (stopped, changed) = f.subagents.stop(&hirer, &workers[0].id).await.unwrap().unwrap();
        assert!(changed);
        assert_eq!(stopped.state, SubagentState::Stopped);
        assert_eq!(stopped.stop_reason, Some(SubagentStopReason::ByAgent));
        let abort = next_of(&f.inbox, "abort").await;
        assert_eq!(abort.session, workers[0].worker_session());

        let others = f.subagents.list(&hirer).await.unwrap();
        assert!(others.iter().any(|w| w.id == workers[1].id && w.state == SubagentState::Running));
        let (_, changed) = f.subagents.stop(&hirer, &workers[0].id).await.unwrap().unwrap();
        assert!(!changed, "already finished");
        reply(&f, &workers[0], "late").await;
        assert!(f.subagents.claim(&chat()).await.is_empty(), "a stopped worker is never delivered");
    }

    #[tokio::test]
    async fn an_unknown_or_foreign_worker_is_refused() {
        let f = fixture().await;
        let workers = f.subagents.spawn(&ctx(chat(), 1), vec!["a".into()], 600).await.unwrap();
        let mut other = chat();
        other.2 = Some("another-topic".into());
        let stranger = ctx(other, 1);
        assert_eq!(
            f.subagents.stop(&stranger, &workers[0].id).await.unwrap().unwrap_err(),
            Refusal::UnknownSubagent(workers[0].id.clone())
        );
        assert!(f.subagents.get(&stranger, "s_nope").await.unwrap().is_err());
        assert!(f.subagents.wait(&stranger, &[workers[0].id.clone()], Until::Any).await.unwrap().is_err());
    }

    #[tokio::test]
    async fn the_time_limit_times_a_worker_out_and_aborts_it() {
        let f = fixture().await;
        let workers = f.subagents.spawn(&ctx(chat(), 1), vec!["slow".into()], 1).await.unwrap();
        let waited = f
            .subagents
            .wait(&ctx(chat(), 5), &[workers[0].id.clone()], Until::All)
            .await
            .unwrap()
            .unwrap();
        assert!(waited.met);
        assert_eq!(waited.workers[0].state, SubagentState::TimedOut);
        assert!(waited.workers[0].error.as_deref().unwrap().contains("time limit (1s)"));
        let abort = next_of(&f.inbox, "abort").await;
        assert_eq!(abort.session, workers[0].worker_session());
        assert_eq!(f.subagents.timer_count(), 0);
    }

    #[tokio::test]
    async fn a_worker_asks_the_main_agent_replies_and_it_continues() {
        let f = fixture().await;
        let hirer = ctx(chat(), 1);
        let workers = f.subagents.spawn(&hirer, vec!["find the sheet".into()], 600).await.unwrap();
        let worker_ctx = ctx(workers[0].worker_session(), 1);

        let asked = f.subagents.ask(&worker_ctx, "EU or US?").await.unwrap();
        assert_eq!(asked.state, SubagentState::Asking);
        assert!(asked.deadline_at.is_none() && asked.remaining_ms.unwrap() > 590_000);
        assert_eq!(f.subagents.timer_count(), 0, "asking does not spend the time limit");
        assert!(f.subagents.ask(&hirer, "not a worker").await.is_err());

        // The question is delivered once, unless the main agent read it first.
        let delivered = f.subagents.claim(&chat()).await;
        assert_eq!(delivered.len(), 1);
        assert!(delivered[0].contains("question: EU or US?"));
        assert!(f.subagents.claim(&chat()).await.is_empty());

        let replied = f.subagents.reply(&hirer, &workers[0].id, "EU").await.unwrap().unwrap();
        assert_eq!(replied.state, SubagentState::Running);
        assert_eq!(f.subagents.timer_count(), 1);
        let envelope = next_of(&f.inbox, "subagent_reply").await;
        assert_eq!(envelope.session, workers[0].worker_session());
        assert_eq!(envelope.request.content.to_string(), "EU");
        assert_eq!(
            f.subagents.reply(&hirer, &workers[0].id, "again").await.unwrap().unwrap_err(),
            Refusal::NotAsking { id: workers[0].id.clone(), state: SubagentState::Running }
        );

        // A turn that ends after asking leaves the worker asking.
        f.subagents.ask(&worker_ctx, "and the currency?").await.unwrap();
        reply(&f, &workers[0], "(the step text)").await;
        let got = f.subagents.get(&hirer, &workers[0].id).await.unwrap().unwrap();
        assert_eq!(got.state, SubagentState::Asking);
        assert_eq!(got.outcome_seq, 2);
        assert!(f.subagents.claim(&chat()).await.is_empty(), "read through get_subagent");
    }

    #[tokio::test]
    async fn an_unseen_answer_is_delivered_once_beside_other_sources() {
        let f = fixture().await;
        let workers = f.subagents.spawn(&ctx(chat(), 1), vec!["a".into()], 600).await.unwrap();
        reply(&f, &workers[0], "the answer").await;
        let deliveries = Deliveries::new(vec![Arc::new(f.subagents.clone())]);
        let text = deliveries.claim(&chat()).await.expect("a delivery");
        assert!(text.starts_with(UPDATE_HEADING));
        assert!(text.contains("result: the answer"));
        assert_eq!(deliveries.claim(&chat()).await, None);
        // The hiring conversation was poked.
        let poke = next_of(&f.inbox, "detached_delivery").await;
        assert_eq!(poke.session, chat());
    }

    #[tokio::test]
    async fn deleting_the_conversation_stops_its_workers_and_names_their_conversations() {
        let f = fixture().await;
        let workers = f.subagents.spawn(&ctx(chat(), 1), vec!["a".into(), "b".into()], 600).await.unwrap();
        reply(&f, &workers[1], "done").await;
        let gone = f.subagents.delete_session(&chat()).await;
        assert_eq!(gone.len(), 2);
        assert!(gone.contains(&workers[0].worker_session()));
        assert_eq!(f.subagents.timer_count(), 0);
        let abort = next_of(&f.inbox, "abort").await;
        assert_eq!(abort.session, workers[0].worker_session());
        assert!(f.subagents.list(&ctx(chat(), 1)).await.unwrap().is_empty());
        assert!(f.subagents.claim(&chat()).await.is_empty());
    }

    #[tokio::test]
    async fn turn_end_is_a_no_op_outside_a_worker_conversation() {
        let f = fixture().await;
        f.subagents.turn_ended(&chat(), TurnEnd::Replied("x".into())).await;
        let stray = VizierSession("agent-1".into(), VizierChannelId::Subagent, Some("nobody".into()));
        f.subagents.turn_ended(&stray, TurnEnd::Failed("x".into())).await;
    }
}

/// FR-018: under the default setting only the main agent manages workers, and only a worker
/// asks; raising the setting lets workers hire. A detached script inherits its turn's depth.
#[test]
fn who_is_offered_which_subagent_tool() {
    use crate::schema::{VizierChannelId, VizierSession};
    let base = |session: VizierSession, depth: u8| ToolContext {
        loaded_images: Default::default(),
        recalled: Default::default(),
        client: None,
        questions: crate::questions::QuestionsHandle::new(Default::default()),
        interactive: false,
        session,
        pending_attachments: Default::default(),
        hooks: None,
        background_depth: depth,
        subagent_depth: depth,
        deadline: None,
        dream_sources: vec![],
        secret_redactor: std::sync::Arc::new(crate::secrets::SecretRedactor::default()),
        project: None,
        detached: false,
        parent_detached_id: None,
    };
    let main = base(VizierSession("a".into(), VizierChannelId::HTTP("u".into(), "c".into()), Some("t".into())), 0);
    let worker = base(VizierSession("a".into(), VizierChannelId::Subagent, Some("w".into())), 1);
    let dream = base(
        VizierSession(
            "a".into(),
            VizierChannelId::Dream(Box::new(main.session.clone()), crate::schema::DreamStage::Extraction),
            None,
        ),
        0,
    );

    assert!(may_hire(&main, 1) && !is_worker(&main));
    assert!(!may_hire(&worker, 1) && is_worker(&worker));
    assert!(may_hire(&worker, 2), "a raised setting lets workers hire");
    assert!(!may_hire(&dream, 1), "never in the dream cycle");

    let mut script = worker.clone();
    script.detached = true;
    assert!(!may_hire(&script, 1), "a worker's detached script keeps the worker's depth");
}
