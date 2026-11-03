//! The reply stream of a request, carried over the bus.
//!
//! [`reply_channel`] makes a pair that behaves like the unbounded channel it replaces: the
//! stream subscribes to a fresh `reply.{uuid}` subject **before** anything can publish to it, the
//! sender publishes each [`VizierResponse`] as a frame in order, and when the last clone of a
//! sender that wrote to the stream is dropped an end-of-stream frame closes it. That close is what
//! tells a requester its agent went away (`AgentUnavailable`, a restart), so it is kept even
//! though the bus itself has no notion of a closed reply address.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::mpsc;
use vizier_schema::{Bus, BusError, Envelope, MAX_PAYLOAD, Messaging, Subject, SubjectPattern, Subscription};

use crate::schema::{VizierResponse, VizierResponseContent};

/// `Envelope.kind` of one response frame.
pub const KIND_FRAME: &str = "response_frame";
/// `Envelope.kind` of the frame that ends a reply stream.
pub const KIND_END: &str = "response_end";

/// The other end of a reply is gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplyClosed;

impl std::fmt::Display for ReplyClosed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the reply stream is closed")
    }
}

impl std::error::Error for ReplyClosed {}

struct Inner {
    subject: Subject,
    outbox: mpsc::UnboundedSender<VizierResponse>,
    /// Set when the sender only names the subject for a request: the agent's side then owns the
    /// stream and its end.
    detached: Arc<AtomicBool>,
}

/// Where a turn's frames go. Cloning shares the same stream.
#[derive(Clone)]
pub struct ResponseSender {
    inner: Arc<Inner>,
}

/// The frames of one turn, in order.
pub struct ResponseStream {
    sub: tokio::sync::Mutex<Subscription>,
    done: AtomicBool,
}

impl ResponseSender {
    /// A sender that publishes to `subject` on `messaging` and ends the stream when dropped.
    pub fn to_subject(messaging: Arc<dyn Messaging>, subject: Subject) -> Self {
        let (outbox, rx) = mpsc::unbounded_channel();
        let detached = Arc::new(AtomicBool::new(false));
        tokio::spawn(forward(messaging, subject.clone(), rx, detached.clone()));
        Self { inner: Arc::new(Inner { subject, outbox, detached }) }
    }

    pub fn send(&self, frame: VizierResponse) -> Result<(), ReplyClosed> {
        self.inner.outbox.send(frame).map_err(|_| ReplyClosed)
    }

    pub async fn send_async(&self, frame: VizierResponse) -> Result<(), ReplyClosed> {
        self.send(frame)
    }

    pub fn subject(&self) -> &Subject {
        &self.inner.subject
    }

    /// The subject a request carries as its `reply_to`. Consumes the sender without ending the
    /// stream: whoever receives the request builds its own sender for the subject.
    pub fn into_subject(self) -> Subject {
        self.inner.detached.store(true, Ordering::SeqCst);
        self.inner.subject.clone()
    }

    pub fn is_disconnected(&self) -> bool {
        self.inner.outbox.is_closed()
    }
}

impl std::fmt::Debug for ResponseSender {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ResponseSender({})", self.inner.subject)
    }
}

impl ResponseStream {
    /// The next frame, or `Err` once the stream has ended.
    pub async fn recv_async(&self) -> Result<VizierResponse, ReplyClosed> {
        let mut sub = self.sub.lock().await;
        loop {
            if self.done.load(Ordering::SeqCst) {
                return Err(ReplyClosed);
            }
            let Some((_, envelope)) = sub.recv().await else {
                self.done.store(true, Ordering::SeqCst);
                return Err(ReplyClosed);
            };
            match envelope.kind.as_str() {
                KIND_END => {
                    self.done.store(true, Ordering::SeqCst);
                    sub.close();
                    return Err(ReplyClosed);
                }
                KIND_FRAME => match envelope.decode::<VizierResponse>() {
                    Ok(frame) => return Ok(frame),
                    Err(e) => tracing::warn!("dropping an undecodable response frame: {e}"),
                },
                other => tracing::warn!(kind = other, "unexpected message on a reply subject"),
            }
        }
    }

    /// A frame that has already arrived, if any.
    pub fn try_recv(&self) -> Option<VizierResponse> {
        let mut sub = self.sub.try_lock().ok()?;
        loop {
            let (_, envelope) = sub.try_recv()?;
            match envelope.kind.as_str() {
                KIND_END => {
                    self.done.store(true, Ordering::SeqCst);
                    return None;
                }
                KIND_FRAME => {
                    if let Ok(frame) = envelope.decode::<VizierResponse>() {
                        return Some(frame);
                    }
                }
                _ => {}
            }
        }
    }
}

/// A reply stream and the sender that feeds it, on `bus`. The subscription exists when this
/// returns, so no frame can be published before anyone listens.
pub async fn reply_channel(bus: &Bus) -> Result<(ResponseSender, ResponseStream), BusError> {
    let subject = Subject::reply(uuid::Uuid::new_v4());
    let sub = bus.messaging.subscribe(SubjectPattern::from(&subject)).await?;
    Ok((
        ResponseSender::to_subject(bus.messaging.clone(), subject),
        ResponseStream { sub: tokio::sync::Mutex::new(sub), done: AtomicBool::new(false) },
    ))
}

/// Publishes a sender's frames in order, then ends the stream unless the sender was detached.
async fn forward(
    messaging: Arc<dyn Messaging>,
    subject: Subject,
    mut rx: mpsc::UnboundedReceiver<VizierResponse>,
    detached: Arc<AtomicBool>,
) {
    while let Some(frame) = rx.recv().await {
        let Some(envelope) = frame_envelope(frame) else { continue };
        if let Err(e) = messaging.publish(&subject, envelope).await {
            tracing::warn!(%subject, "could not publish a response frame: {e}");
        }
    }
    if !detached.load(Ordering::SeqCst)
        && let Ok(end) = Envelope::new(KIND_END, Vec::new())
    {
        let _ = messaging.publish(&subject, end).await;
    }
}

/// A frame as an envelope. One over the bus payload limit is cut down rather than lost: message
/// text is truncated with a notice (the full reply is in history, saved before it is sent), and
/// any other kind is dropped with a warning.
fn frame_envelope(mut frame: VizierResponse) -> Option<Envelope> {
    match Envelope::json(KIND_FRAME, &frame) {
        Ok(envelope) => Some(envelope),
        Err(BusError::PayloadTooLarge { size, limit }) => {
            tracing::warn!(size, limit, "a response frame is over the bus payload limit");
            if let VizierResponseContent::Message { content, .. } = &mut frame.content {
                let keep = MAX_PAYLOAD / 2;
                let mut end = keep.min(content.len());
                while !content.is_char_boundary(end) {
                    end -= 1;
                }
                content.truncate(end);
                content.push_str("\n\n[reply cut: it is over the 1 MiB limit of a live frame; the full text is in the history]");
                return Envelope::json(KIND_FRAME, &frame).ok();
            }
            None
        }
        Err(e) => {
            tracing::warn!("could not encode a response frame: {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use vizier_schema::InProcess;

    use super::*;

    fn text(s: &str) -> VizierResponse {
        VizierResponse {
            content: VizierResponseContent::Message { content: s.into(), stats: None },
            ..Default::default()
        }
    }

    fn body(frame: &VizierResponse) -> &str {
        match &frame.content {
            VizierResponseContent::Message { content, .. } => content,
            other => panic!("{other:?}"),
        }
    }

    #[tokio::test]
    async fn frames_arrive_in_order_and_the_stream_ends_when_the_sender_drops() {
        let bus = InProcess::new().bus();
        let (tx, rx) = reply_channel(&bus).await.unwrap();
        for i in 0..50 {
            tx.send_async(text(&i.to_string())).await.unwrap();
        }
        drop(tx);
        for i in 0..50 {
            assert_eq!(body(&rx.recv_async().await.unwrap()), i.to_string());
        }
        assert_eq!(rx.recv_async().await.unwrap_err(), ReplyClosed);
        assert_eq!(rx.recv_async().await.unwrap_err(), ReplyClosed);
    }

    #[tokio::test]
    async fn a_detached_sender_does_not_end_the_stream_but_the_agents_side_does() {
        let bus = InProcess::new().bus();
        let (tx, rx) = reply_channel(&bus).await.unwrap();
        let subject = tx.into_subject();
        // The requester's handle is gone; nothing has closed the stream.
        tokio::task::yield_now().await;
        assert!(rx.try_recv().is_none());

        let agent_side = ResponseSender::to_subject(bus.messaging.clone(), subject);
        agent_side.send(text("hello")).unwrap();
        assert_eq!(body(&rx.recv_async().await.unwrap()), "hello");
        drop(agent_side);
        assert_eq!(rx.recv_async().await.unwrap_err(), ReplyClosed);
    }

    #[tokio::test]
    async fn a_clone_keeps_the_stream_open() {
        let bus = InProcess::new().bus();
        let (tx, rx) = reply_channel(&bus).await.unwrap();
        let other = tx.clone();
        drop(tx);
        other.send(text("still here")).unwrap();
        assert_eq!(body(&rx.recv_async().await.unwrap()), "still here");
    }

    #[tokio::test]
    async fn an_oversized_message_is_cut_not_lost() {
        let bus = InProcess::new().bus();
        let (tx, rx) = reply_channel(&bus).await.unwrap();
        tx.send(text(&"x".repeat(MAX_PAYLOAD + 10))).unwrap();
        let frame = rx.recv_async().await.unwrap();
        assert!(body(&frame).contains("reply cut"));
        assert!(body(&frame).len() < MAX_PAYLOAD);
    }
}

/// A reply stream on a bus of its own, for tests and examples that write into a sender
/// directly instead of sending a request.
#[doc(hidden)]
pub async fn standalone_reply_channel() -> (ResponseSender, ResponseStream) {
    reply_channel(&vizier_schema::InProcess::new().bus())
        .await
        .expect("the in-process bus accepts a subscription")
}
