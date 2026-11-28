//! The storage contract of `specs/071-triggers/data-model.md` (tasks T003, T005).

use serde_json::json;

use super::*;
use crate::{
    schema::{OverlapPolicy, Requester},
    storage::sqlite::init_trigger_schema,
};

fn setup() -> SqlStorage {
    let store = crate::tests_support::test_store(|| {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        init_trigger_schema(&conn).unwrap();
        crate::storage::sql::SqlStore::sqlite(conn)
    });
    SqlStorage::new(store)
}

fn trigger(slug: &str) -> Trigger {
    Trigger {
        id: format!("t-{slug}"),
        agent_id: "dp".into(),
        slug: slug.into(),
        kind: "webhook".into(),
        title: "CI".into(),
        instruction: "Tell me when CI fails".into(),
        settings: json!({"secret": "s3cret"}),
        requester: Requester::User("alice".into()),
        delivers_to: Some("u1".into()),
        is_active: true,
        awaiting_approval: false,
        needs_attention: None,
        tool_scope: Some(vec!["memory_read".into()]),
        overlap: OverlapPolicy::Wait,
        hourly_limit: Some(20),
        cooldown_secs: 0,
        triage: None,
        destination: None,
        summary_hours: Some(5),
        last_summary_at: None,
        created_at: crate::time::now(),
        last_fired_at: None,
    }
}

fn event(trigger: &Trigger, status: EventStatus) -> NewEvent {
    NewEvent {
        trigger_id: trigger.id.clone(),
        agent_id: trigger.agent_id.clone(),
        received_at: crate::time::now(),
        payload: Some(json!({"branch": "master"})),
        delivery_id: None,
        status,
        reason: None,
        when_full: WhenFull::DropOldest,
        monitor_state: None,
    }
}

async fn start(storage: &SqlStorage, trigger: &Trigger) -> Option<StartedRun> {
    let now = crate::time::now();
    storage.start_trigger_run(trigger, now, format!("key-{}", now.timestamp_micros())).await.unwrap()
}

#[tokio::test]
async fn a_trigger_round_trips_and_its_slug_is_unique() {
    let storage = setup();
    let t = trigger("ci");
    storage.create_trigger(&t).await.unwrap();
    let back = storage.get_trigger("dp", "ci").await.unwrap().unwrap();
    assert_eq!(back.settings, json!({"secret": "s3cret"}));
    assert_eq!(back.tool_scope, t.tool_scope);
    assert_eq!(storage.get_trigger_by_id("t-ci").await.unwrap().unwrap().slug, "ci");

    let mut again = trigger("ci");
    again.id = "t-other".into();
    assert!(storage.create_trigger(&again).await.is_err());
}

#[tokio::test]
async fn a_redelivery_is_a_duplicate_of_the_first_event() {
    let storage = setup();
    let t = trigger("ci");
    storage.create_trigger(&t).await.unwrap();
    let mut first = event(&t, EventStatus::Waiting);
    first.delivery_id = Some("d-1".into());
    let a = storage.admit_event(first.clone()).await.unwrap();
    assert_eq!(a.outcome, AdmitOutcome::Accepted);
    let b = storage.admit_event(first).await.unwrap();
    assert_eq!(b, Admission { event_id: a.event_id, outcome: AdmitOutcome::Duplicate });
}

#[tokio::test]
async fn runs_never_overlap_and_take_up_to_twenty_events() {
    let storage = setup();
    let t = trigger("ci");
    storage.create_trigger(&t).await.unwrap();
    for _ in 0..25 {
        storage.admit_event(event(&t, EventStatus::Waiting)).await.unwrap();
    }
    let run = start(&storage, &t).await.unwrap();
    assert_eq!(run.events.len(), MAX_EVENTS_PER_RUN);
    assert!(run.events.iter().all(|e| e.status == EventStatus::Handled && e.attempts == 1));
    assert!(start(&storage, &t).await.is_none(), "a second run must wait for the first");

    storage
        .close_trigger_run(run.run.id, TriggerRunState::Answered, crate::time::now(), None)
        .await
        .unwrap()
        .unwrap();
    let next = start(&storage, &t).await.unwrap();
    assert_eq!(next.events.len(), 5);
}

#[tokio::test]
async fn the_hourly_limit_holds_and_events_keep_waiting() {
    let storage = setup();
    let mut t = trigger("ci");
    t.hourly_limit = Some(2);
    storage.create_trigger(&t).await.unwrap();
    for _ in 0..3 {
        storage.admit_event(event(&t, EventStatus::Waiting)).await.unwrap();
        if let Some(run) = start(&storage, &t).await {
            storage
                .close_trigger_run(run.run.id, TriggerRunState::Answered, crate::time::now(), None)
                .await
                .unwrap();
        }
    }
    let events = storage.list_trigger_events(&t.id, None, 10).await.unwrap();
    let waiting = events.iter().filter(|e| e.status == EventStatus::Waiting).count();
    let handled = events.iter().filter(|e| e.status == EventStatus::Handled).count();
    assert_eq!((handled, waiting), (2, 1));
    assert!(storage.triggers_with_work().await.unwrap().contains(&t.id));
}

#[tokio::test]
async fn a_full_webhook_queue_drops_its_oldest_and_a_monitor_refuses() {
    let storage = setup();
    let t = trigger("ci");
    storage.create_trigger(&t).await.unwrap();
    let mut first = None;
    for _ in 0..MAX_WAITING_EVENTS {
        let a = storage.admit_event(event(&t, EventStatus::Waiting)).await.unwrap();
        first.get_or_insert(a.event_id);
    }
    let a = storage.admit_event(event(&t, EventStatus::Waiting)).await.unwrap();
    assert_eq!(a.outcome, AdmitOutcome::Accepted);
    let events = storage.list_trigger_events(&t.id, None, 200).await.unwrap();
    let oldest = events.iter().find(|e| Some(e.id) == first).unwrap();
    assert_eq!(oldest.status, EventStatus::Dropped);

    let mut m = trigger("watch");
    m.kind = "monitor".into();
    storage.create_trigger(&m).await.unwrap();
    for _ in 0..MAX_WAITING_EVENTS {
        storage.admit_event(event(&m, EventStatus::Waiting)).await.unwrap();
    }
    let mut refused = event(&m, EventStatus::Waiting);
    refused.when_full = WhenFull::RefuseNew;
    refused.monitor_state = Some(json!({"seen": 2}));
    let a = storage.admit_event(refused.clone()).await.unwrap();
    assert_eq!(a.outcome, AdmitOutcome::Refused);
    assert_eq!(storage.monitor_record(&m.id).await.unwrap().state, None, "a refusal keeps the old state");
    // Refusals collapse into one record.
    let b = storage.admit_event(refused).await.unwrap();
    assert_eq!(a.event_id, b.event_id);
}

#[tokio::test]
async fn a_monitor_state_is_saved_only_when_its_event_is_accepted() {
    let storage = setup();
    let mut m = trigger("watch");
    m.kind = "monitor".into();
    storage.create_trigger(&m).await.unwrap();
    let mut e = event(&m, EventStatus::Waiting);
    e.monitor_state = Some(json!({"seen": 1}));
    storage.admit_event(e).await.unwrap();
    assert_eq!(storage.monitor_record(&m.id).await.unwrap().state, Some(json!({"seen": 1})));
}

#[tokio::test]
async fn a_failed_run_retries_its_events_once_then_fails_them() {
    let storage = setup();
    let t = trigger("ci");
    storage.create_trigger(&t).await.unwrap();
    storage.admit_event(event(&t, EventStatus::Waiting)).await.unwrap();

    let run = start(&storage, &t).await.unwrap();
    let closed = storage
        .close_trigger_run(run.run.id, TriggerRunState::NoResponse, crate::time::now(), None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!((closed.retried_events, closed.failed_events), (1, 0));

    let run = start(&storage, &t).await.unwrap();
    assert_eq!(run.events[0].attempts, 2);
    let closed = storage
        .close_trigger_run(run.run.id, TriggerRunState::Interrupted, crate::time::now(), None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!((closed.retried_events, closed.failed_events), (0, 1));
    assert!(start(&storage, &t).await.is_none());
}

#[tokio::test]
async fn a_closed_run_stays_closed_and_delivers_once() {
    let storage = setup();
    let t = trigger("ci");
    storage.create_trigger(&t).await.unwrap();
    storage.admit_event(event(&t, EventStatus::Waiting)).await.unwrap();
    let run = start(&storage, &t).await.unwrap();
    let now = crate::time::now();
    assert!(storage.close_trigger_run(run.run.id, TriggerRunState::Answered, now, None).await.unwrap().is_some());
    assert!(storage.close_trigger_run(run.run.id, TriggerRunState::NoResponse, now, None).await.unwrap().is_none());
    assert!(storage.claim_delivery(run.run.id, now).await.unwrap());
    assert!(!storage.claim_delivery(run.run.id, now).await.unwrap());
}

#[tokio::test]
async fn verdicts_split_events_and_unimportant_ones_reach_the_summary() {
    let storage = setup();
    let t = trigger("ci");
    storage.create_trigger(&t).await.unwrap();
    let a = storage.admit_event(event(&t, EventStatus::TriagePending)).await.unwrap();
    let b = storage.admit_event(event(&t, EventStatus::TriagePending)).await.unwrap();
    assert_eq!(storage.pending_triage(&t.id, 20).await.unwrap().len(), 2);
    storage
        .record_verdicts(
            &t.id,
            &[
                Verdict { event_id: a.event_id, important: true, reason: "master".into(), untriaged: false },
                Verdict { event_id: b.event_id, important: false, reason: "a branch".into(), untriaged: false },
            ],
            WhenFull::DropOldest,
        )
        .await
        .unwrap();
    let run = start(&storage, &t).await.unwrap();
    assert_eq!(run.events.iter().map(|e| e.id).collect::<Vec<_>>(), vec![a.event_id]);

    storage
        .close_trigger_run(run.run.id, TriggerRunState::Answered, crate::time::now(), Some("all green".into()))
        .await
        .unwrap();
    let items = storage.summary_items(&t.id).await.unwrap();
    assert_eq!(items.unimportant.len(), 1);
    assert_eq!(items.quiet_runs.len(), 1);
    storage
        .mark_summarised(&t.id, &[b.event_id], &[run.run.id], crate::time::now())
        .await
        .unwrap();
    let items = storage.summary_items(&t.id).await.unwrap();
    assert!(items.unimportant.is_empty() && items.quiet_runs.is_empty());
}

#[tokio::test]
async fn drops_collapse_and_payloads_expire() {
    let storage = setup();
    let t = trigger("ci");
    storage.create_trigger(&t).await.unwrap();
    let a = storage.admit_event(event(&t, EventStatus::Dropped)).await.unwrap();
    let b = storage.admit_event(event(&t, EventStatus::Dropped)).await.unwrap();
    assert_eq!(a.event_id, b.event_id);
    let record = &storage.list_trigger_events(&t.id, None, 10).await.unwrap()[0];
    assert_eq!(record.count, 2);

    let later = crate::time::now() + chrono::Duration::seconds(1);
    assert_eq!(storage.purge_event_payloads(later).await.unwrap(), 1);
    assert!(storage.list_trigger_events(&t.id, None, 10).await.unwrap()[0].payload.is_none());
}

#[tokio::test]
async fn deleting_a_trigger_deletes_what_it_recorded() {
    let storage = setup();
    let t = trigger("ci");
    storage.create_trigger(&t).await.unwrap();
    storage.admit_event(event(&t, EventStatus::Waiting)).await.unwrap();
    start(&storage, &t).await.unwrap();
    storage.delete_trigger(&t.id).await.unwrap();
    assert!(storage.get_trigger("dp", "ci").await.unwrap().is_none());
    assert!(storage.list_trigger_events(&t.id, None, 10).await.unwrap().is_empty());
    assert!(storage.list_trigger_runs(&t.id, None, 10).await.unwrap().is_empty());
}

#[tokio::test]
async fn expired_delivery_ids_are_deleted_and_live_ones_kept() {
    let storage = setup();
    let t = trigger("ci");
    storage.create_trigger(&t).await.unwrap();
    let mut old = event(&t, EventStatus::Waiting);
    old.delivery_id = Some("old".into());
    old.received_at = crate::time::now() - chrono::Duration::hours(25);
    storage.admit_event(old).await.unwrap();
    let mut live = event(&t, EventStatus::Waiting);
    live.delivery_id = Some("live".into());
    storage.admit_event(live.clone()).await.unwrap();

    assert_eq!(storage.purge_deliveries(crate::time::now()).await.unwrap(), 1);
    assert_eq!(storage.admit_event(live).await.unwrap().outcome, AdmitOutcome::Duplicate);
}

#[tokio::test]
async fn noting_a_firing_keeps_an_edit_made_meanwhile() {
    let storage = setup();
    let t = trigger("ci");
    storage.create_trigger(&t).await.unwrap();
    let mut edited = t.clone();
    edited.title = "Renamed".into();
    edited.is_active = false;
    storage.save_trigger(&edited).await.unwrap();

    let now = crate::time::now();
    storage.note_trigger_fired(&t.id, now).await.unwrap();
    storage.note_trigger_summarised(&t.id, now).await.unwrap();
    let back = storage.get_trigger_by_id(&t.id).await.unwrap().unwrap();
    assert_eq!((back.title.as_str(), back.is_active), ("Renamed", false));
    assert!(back.last_fired_at.is_some() && back.last_summary_at.is_some());
    assert_eq!(back.settings, t.settings, "the sealed settings are untouched");

    assert!(storage.set_trigger_attention(&t.id, Some("gone".into())).await.unwrap());
    assert!(!storage.set_trigger_attention(&t.id, Some("gone".into())).await.unwrap(), "told once");
}

#[tokio::test]
async fn runs_are_counted_from_a_moment() {
    let storage = setup();
    let t = trigger("ci");
    storage.create_trigger(&t).await.unwrap();
    let before = crate::time::now() - chrono::Duration::seconds(1);
    storage.admit_event(event(&t, EventStatus::Waiting)).await.unwrap();
    start(&storage, &t).await.expect("a run");
    assert_eq!(storage.trigger_runs_since(&t.id, before).await.unwrap(), 1);
    let later = crate::time::now() + chrono::Duration::seconds(1);
    assert_eq!(storage.trigger_runs_since(&t.id, later).await.unwrap(), 0);
    assert_eq!(storage.trigger_runs_since("other", before).await.unwrap(), 0);
}

crate::postgres_tests!(
    runs_are_counted_from_a_moment,
    a_trigger_round_trips_and_its_slug_is_unique,
    a_redelivery_is_a_duplicate_of_the_first_event,
    runs_never_overlap_and_take_up_to_twenty_events,
    the_hourly_limit_holds_and_events_keep_waiting,
    a_full_webhook_queue_drops_its_oldest_and_a_monitor_refuses,
    a_monitor_state_is_saved_only_when_its_event_is_accepted,
    a_failed_run_retries_its_events_once_then_fails_them,
    a_closed_run_stays_closed_and_delivers_once,
    verdicts_split_events_and_unimportant_ones_reach_the_summary,
    drops_collapse_and_payloads_expire,
    deleting_a_trigger_deletes_what_it_recorded,
    expired_delivery_ids_are_deleted_and_live_ones_kept,
    noting_a_firing_keeps_an_edit_made_meanwhile,
);
