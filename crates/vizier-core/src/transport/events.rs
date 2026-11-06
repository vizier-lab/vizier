//! Session and integration events on the bus.
//!
//! `publish_session_event` and `publish_integration_change` stay synchronous fire-and-forget calls,
//! and the subscribers keep their `broadcast` receivers (and their lag behaviour). In between sits
//! one pump task: it subscribes to `session.>` and `integration.changed` **before** it publishes
//! anything, publishes the outbox in order, and fans what comes back into the local broadcasts. So an
//! event another node publishes reaches the same subscribers as one published here.
//!
//! An event that cannot be put on the bus (over the payload limit, or not encodable) is still
//! delivered to the local subscribers directly rather than lost.

use tokio::sync::{broadcast, mpsc};
use vizier_schema::{Bus, Envelope, Subject, SubjectPattern};

use super::{InboxEvent, SessionEvent};
use crate::schema::IntegrationChanged;

pub const KIND_SESSION_EVENT: &str = "session_event";
pub const KIND_INTEGRATION_CHANGED: &str = "integration_changed";
pub const KIND_INBOX_EVENT: &str = "inbox_event";

pub(super) enum Outgoing {
    Session(SessionEvent),
    Integration(IntegrationChanged),
    Inbox(InboxEvent),
}

/// Start the pump. Needs a running tokio runtime.
pub(super) fn start(
    bus: Bus,
    session_tx: broadcast::Sender<SessionEvent>,
    integration_tx: broadcast::Sender<IntegrationChanged>,
    inbox_tx: broadcast::Sender<InboxEvent>,
) -> mpsc::UnboundedSender<Outgoing> {
    let (outbox, rx) = mpsc::unbounded_channel();
    tokio::spawn(pump(bus, rx, session_tx, integration_tx, inbox_tx));
    outbox
}

async fn pump(
    bus: Bus,
    mut outbox: mpsc::UnboundedReceiver<Outgoing>,
    session_tx: broadcast::Sender<SessionEvent>,
    integration_tx: broadcast::Sender<IntegrationChanged>,
    inbox_tx: broadcast::Sender<InboxEvent>,
) {
    let sessions = bus.messaging.subscribe(SubjectPattern::parse("session.>").expect("a valid pattern")).await;
    let integrations = bus.messaging.subscribe(SubjectPattern::from(&Subject::integration_changed())).await;
    let inboxes = bus.messaging.subscribe(SubjectPattern::from(&Subject::inbox_events())).await;
    let (mut sessions, mut integrations, mut inboxes) = match (sessions, integrations, inboxes) {
        (Ok(s), Ok(i), Ok(b)) => (s, i, b),
        (s, i, b) => {
            tracing::error!(
                "could not subscribe to events on the bus: {:?}",
                s.err().or(i.err()).or(b.err())
            );
            // Nothing can come back, so deliver what is published here straight to the subscribers.
            while let Some(out) = outbox.recv().await {
                match out {
                    Outgoing::Session(ev) => {
                        let _ = session_tx.send(ev);
                    }
                    Outgoing::Integration(ev) => {
                        let _ = integration_tx.send(ev);
                    }
                    Outgoing::Inbox(ev) => {
                        let _ = inbox_tx.send(ev);
                    }
                }
            }
            return;
        }
    };

    loop {
        tokio::select! {
            out = outbox.recv() => {
                let Some(out) = out else { break };
                match out {
                    Outgoing::Session(ev) => publish_session(&bus, &session_tx, ev).await,
                    Outgoing::Integration(ev) => publish_integration(&bus, &integration_tx, ev).await,
                    Outgoing::Inbox(ev) => publish_inbox(&bus, &inbox_tx, ev).await,
                }
            }
            Some((_, envelope)) = inboxes.recv() => {
                match envelope.decode::<InboxEvent>() {
                    Ok(ev) => { let _ = inbox_tx.send(ev); }
                    Err(e) => tracing::warn!("dropping an undecodable inbox event: {e}"),
                }
                let lost = inboxes.lagged();
                if lost > 0 {
                    tracing::warn!(lost, "the inbox event pump fell behind and dropped events");
                }
            }
            Some((_, envelope)) = sessions.recv() => {
                match envelope.decode::<SessionEvent>() {
                    Ok(ev) => { let _ = session_tx.send(ev); }
                    Err(e) => tracing::warn!("dropping an undecodable session event: {e}"),
                }
                // A lagging bus subscription loses events like a lagging broadcast does; say so.
                let lost = sessions.lagged();
                if lost > 0 {
                    tracing::warn!(lost, "the session event pump fell behind and dropped events");
                }
            }
            Some((_, envelope)) = integrations.recv() => {
                match envelope.decode::<IntegrationChanged>() {
                    Ok(ev) => { let _ = integration_tx.send(ev); }
                    Err(e) => tracing::warn!("dropping an undecodable integration event: {e}"),
                }
            }
        }
    }
}

async fn publish_session(bus: &Bus, local: &broadcast::Sender<SessionEvent>, ev: SessionEvent) {
    let subject = Subject::session_events(&ev.session.to_slug());
    match Envelope::json(KIND_SESSION_EVENT, &ev) {
        Ok(envelope) => {
            if let Err(e) = bus.messaging.publish(&subject, envelope).await {
                tracing::warn!("could not publish a session event, delivering it locally: {e}");
                let _ = local.send(ev);
            }
        }
        Err(e) => {
            tracing::warn!("a session event cannot go on the bus ({e}); delivering it locally");
            let _ = local.send(ev);
        }
    }
}

async fn publish_inbox(bus: &Bus, local: &broadcast::Sender<InboxEvent>, ev: InboxEvent) {
    match Envelope::json(KIND_INBOX_EVENT, &ev) {
        Ok(envelope) => {
            if let Err(e) = bus.messaging.publish(&Subject::inbox_events(), envelope).await {
                tracing::warn!("could not publish an inbox event, delivering it locally: {e}");
                let _ = local.send(ev);
            }
        }
        Err(e) => {
            tracing::warn!("an inbox event cannot go on the bus ({e}); delivering it locally");
            let _ = local.send(ev);
        }
    }
}

async fn publish_integration(bus: &Bus, local: &broadcast::Sender<IntegrationChanged>, ev: IntegrationChanged) {
    match Envelope::json(KIND_INTEGRATION_CHANGED, &ev) {
        Ok(envelope) => {
            if let Err(e) = bus.messaging.publish(&Subject::integration_changed(), envelope).await {
                tracing::warn!("could not publish an integration change, delivering it locally: {e}");
                let _ = local.send(ev);
            }
        }
        Err(e) => {
            tracing::warn!("an integration change cannot go on the bus ({e}); delivering it locally");
            let _ = local.send(ev);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::super::{SessionFrame, VizierTransport};
    use super::*;
    use crate::schema::{
        ReactionEntry, VizierChannelId, VizierResponse, VizierResponseContent, VizierSession,
    };

    fn session(id: &str) -> VizierSession {
        VizierSession(id.into(), VizierChannelId::System, None)
    }

    fn title(id: &str, text: &str) -> SessionEvent {
        SessionEvent { session: session(id), frame: SessionFrame::Title { title: text.into() } }
    }

    async fn next(rx: &mut broadcast::Receiver<SessionEvent>) -> SessionEvent {
        tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("an event in time")
            .expect("an open channel")
    }

    #[tokio::test]
    async fn two_subscribers_each_get_a_frame_once_and_in_order() {
        let transport = VizierTransport::new();
        let mut a = transport.subscribe_session_events();
        let mut b = transport.subscribe_session_events();
        for i in 0..20 {
            transport.publish_session_event(title("s", &i.to_string()));
        }
        for rx in [&mut a, &mut b] {
            for i in 0..20 {
                match next(rx).await.frame {
                    SessionFrame::Title { title } => assert_eq!(title, i.to_string()),
                    other => panic!("{other:?}"),
                }
            }
            assert!(rx.try_recv().is_err(), "an event was delivered twice");
        }
    }

    #[tokio::test]
    async fn frames_survive_the_trip_through_the_bus() {
        let transport = VizierTransport::new();
        let mut rx = transport.subscribe_session_events();
        let response = VizierResponse {
            content: VizierResponseContent::Message { content: "hi".into(), stats: None },
            history_uid: Some("uid-1".into()),
            resumed: true,
            ..Default::default()
        };
        transport.publish_session_event(SessionEvent {
            session: session("s"),
            frame: SessionFrame::Response(response),
        });
        transport.publish_session_event(SessionEvent {
            session: session("s"),
            frame: SessionFrame::Reactions { history_uid: "uid-1".into(), reactions: Vec::<ReactionEntry>::new() },
        });
        let first = next(&mut rx).await;
        assert_eq!(first.session, session("s"));
        let SessionFrame::Response(r) = first.frame else { panic!("expected a response") };
        assert_eq!(r.history_uid.as_deref(), Some("uid-1"));
        assert!(r.resumed);
        assert!(matches!(next(&mut rx).await.frame, SessionFrame::Reactions { .. }));
    }

    #[tokio::test]
    async fn a_lagging_subscriber_loses_events_and_carries_on() {
        let transport = VizierTransport::new();
        let mut slow = transport.subscribe_session_events();
        for i in 0..(super::super::SESSION_EVENTS_CAPACITY + 200) {
            transport.publish_session_event(title("s", &i.to_string()));
        }
        // Let the pump drain its outbox into the broadcast before reading.
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(matches!(slow.recv().await, Err(broadcast::error::RecvError::Lagged(_))));
        // It keeps working afterwards.
        transport.publish_session_event(title("s", "after"));
        loop {
            if let SessionFrame::Title { title } = next(&mut slow).await.frame
                && title == "after"
            {
                break;
            }
        }
    }

    #[tokio::test]
    async fn an_integration_change_reaches_every_subscriber() {
        let transport = VizierTransport::new();
        let mut a = transport.subscribe_integration_changes();
        let mut b = transport.subscribe_integration_changes();
        transport.publish_integration_change(IntegrationChanged {
            integration_id: "int-1".into(),
            family: crate::schema::IntegrationFamily::Mcp,
            agents: vec!["alpha".into()],
        });
        for rx in [&mut a, &mut b] {
            let got = tokio::time::timeout(Duration::from_secs(5), rx.recv()).await.unwrap().unwrap();
            assert_eq!(got.integration_id, "int-1");
            assert_eq!(got.agents, vec!["alpha".to_string()]);
        }
    }

    fn setup_event() -> InboxEvent {
        InboxEvent::Setup {
            state: crate::setup::SetupState {
                agent_id: "scout".into(),
                name: "Scout".into(),
                phase: crate::setup::SetupPhase::Downloading,
                message: "BGE small".into(),
                error: None,
                downloads: vec![],
                // `owner_id` is `serde(skip)` on the state; it must still survive the bus.
                owner_id: None,
            },
            owner_id: Some("u-ana".into()),
        }
    }

    #[tokio::test]
    async fn an_inbox_event_reaches_every_subscriber_through_the_bus_with_its_owner() {
        let transport = VizierTransport::new();
        let mut a = transport.subscribe_inbox_events();
        let mut b = transport.subscribe_inbox_events();
        transport.publish_inbox_event(setup_event());
        transport.publish_inbox_event(InboxEvent::Read { user_id: "u1".into(), id: Some(4), read: true });
        transport.publish_inbox_event(InboxEvent::SetupCleared { agent_id: "scout".into() });
        for rx in [&mut a, &mut b] {
            let first = tokio::time::timeout(Duration::from_secs(5), rx.recv()).await.unwrap().unwrap();
            let InboxEvent::Setup { state, owner_id } = first else { panic!("expected setup first") };
            assert_eq!(state.agent_id, "scout");
            assert_eq!(owner_id.as_deref(), Some("u-ana"));
            let second = tokio::time::timeout(Duration::from_secs(5), rx.recv()).await.unwrap().unwrap();
            assert!(matches!(second, InboxEvent::Read { id: Some(4), read: true, .. }));
            let third = tokio::time::timeout(Duration::from_secs(5), rx.recv()).await.unwrap().unwrap();
            assert!(matches!(third, InboxEvent::SetupCleared { .. }));
            assert!(rx.try_recv().is_err(), "an event was delivered twice");
        }
    }

    #[tokio::test]
    async fn a_created_row_keeps_its_source_session_across_the_bus() {
        use crate::schema::{NotificationCategory, NotificationItem, NotificationTone};
        let transport = VizierTransport::new();
        let mut rx = transport.subscribe_inbox_events();
        let session = session("s");
        let item = NotificationItem {
            id: 9,
            at: crate::time::now(),
            agent_id: "a".into(),
            category: NotificationCategory::ChatReply,
            tone: NotificationTone::Default,
            text: "Replied in your conversation.".into(),
            href: "/a/chat/s".into(),
            read: false,
            title: None,
            excerpt: Some("hello".into()),
            // `serde(skip)`: lost on the wire, which is why the event carries it separately.
            source_session: Some(session.clone()),
        };
        transport.publish_inbox_event(InboxEvent::Created {
            item,
            audience: Some("u1".into()),
            source_session: Some(session.clone()),
        });
        let got = tokio::time::timeout(Duration::from_secs(5), rx.recv()).await.unwrap().unwrap();
        let InboxEvent::Created { item, audience, source_session } = got else { panic!("expected created") };
        assert_eq!(item.id, 9);
        assert_eq!(item.excerpt.as_deref(), Some("hello"));
        assert!(item.source_session.is_none());
        assert_eq!(audience.as_deref(), Some("u1"));
        assert_eq!(source_session, Some(session));
    }

    #[test]
    fn without_a_runtime_inbox_events_still_reach_local_subscribers() {
        let transport = VizierTransport::new();
        let mut rx = transport.subscribe_inbox_events();
        transport.publish_inbox_event(InboxEvent::Stale { user: None, agent_id: Some("a".into()) });
        assert!(matches!(rx.try_recv().unwrap(), InboxEvent::Stale { .. }));
    }

    #[tokio::test]
    async fn a_lagging_inbox_subscriber_loses_events_and_carries_on() {
        let transport = VizierTransport::new();
        let mut slow = transport.subscribe_inbox_events();
        for i in 0..(super::super::INBOX_EVENTS_CAPACITY + 100) {
            transport.publish_inbox_event(InboxEvent::SetupCleared { agent_id: i.to_string() });
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(matches!(slow.recv().await, Err(broadcast::error::RecvError::Lagged(_))));
        transport.publish_inbox_event(InboxEvent::SetupCleared { agent_id: "after".into() });
        loop {
            let got = tokio::time::timeout(Duration::from_secs(5), slow.recv()).await.unwrap().unwrap();
            if matches!(&got, InboxEvent::SetupCleared { agent_id } if agent_id == "after") {
                break;
            }
        }
    }

    #[test]
    fn without_a_runtime_events_still_reach_local_subscribers() {
        let transport = VizierTransport::new();
        // `subscribe` outside a runtime cannot start the pump; publish falls back to local delivery.
        let mut rx = transport.subscribe_session_events();
        transport.publish_session_event(title("s", "x"));
        assert!(matches!(rx.try_recv().unwrap().frame, SessionFrame::Title { .. }));
    }
}
