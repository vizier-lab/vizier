//! The messaging half of the bus.
//!
//! Semantics every backend must keep (contracts/bus-semantics.md, rules 1-7):
//!
//! 1. Delivery is at-most-once; there is no retry, redelivery or persistence.
//! 2. Messages from one publisher to one subject reach a subscriber in publish order.
//! 3. `publish` never blocks on subscribers; a slow one loses messages and is told it lagged
//!    ([`Subscription::lagged`]). A subject with no subscribers is not an error for `publish`.
//! 4. `request` returns an error in exactly two cases and never hangs: [`BusError::Timeout`]
//!    and [`BusError::NoResponders`].
//! 5. A payload over [`MAX_PAYLOAD`](crate::MAX_PAYLOAD) is refused with `PayloadTooLarge`.
//! 6. Patterns are exact, `*` (one token) or trailing `>`; anything else is `InvalidPattern`.
//! 7. An identifier is always one token, so it can never add levels or widen a subscription.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::envelope::Envelope;
use crate::error::BusError;
use crate::subject::{Subject, SubjectPattern};

/// Queue depth of one subscription before it starts losing messages (rule 3).
pub const SUBSCRIPTION_DEPTH: usize = 65_536;

#[async_trait]
pub trait Messaging: Send + Sync {
    /// Fire and forget. Succeeds with no subscribers.
    async fn publish(&self, subject: &Subject, envelope: Envelope) -> Result<(), BusError>;

    /// Publish with a fresh reply address and wait for the first reply.
    async fn request(
        &self,
        subject: &Subject,
        envelope: Envelope,
        timeout: Duration,
    ) -> Result<Envelope, BusError>;

    /// Receive every message whose subject matches `pattern`, from now on.
    async fn subscribe(&self, pattern: SubjectPattern) -> Result<Subscription, BusError>;
}

/// Answer a request: publish `response` to the request's `reply_to`.
///
/// A request without a reply address has nobody to answer, which is not an error.
pub async fn reply(
    messaging: &dyn Messaging,
    request: &Envelope,
    response: Envelope,
) -> Result<(), BusError> {
    match &request.reply_to {
        Some(subject) => messaging.publish(subject, response).await,
        None => Ok(()),
    }
}

/// The receiving end of a subscription. Dropping it unsubscribes.
pub struct Subscription {
    rx: mpsc::Receiver<(Subject, Envelope)>,
    lagged: Arc<AtomicU64>,
}

/// The sending end of a subscription, held by a backend.
#[derive(Clone)]
pub struct SubscriptionSender {
    tx: mpsc::Sender<(Subject, Envelope)>,
    lagged: Arc<AtomicU64>,
}

/// What happened to one delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    Delivered,
    /// The subscriber's queue was full; the message was dropped and counted as lag.
    Lagged,
    /// The subscriber is gone.
    Closed,
}

impl Subscription {
    /// A subscription queue of `depth` messages. Backends keep the sender.
    pub fn channel(depth: usize) -> (SubscriptionSender, Subscription) {
        let (tx, rx) = mpsc::channel(depth.max(1));
        let lagged = Arc::new(AtomicU64::new(0));
        (SubscriptionSender { tx, lagged: lagged.clone() }, Subscription { rx, lagged })
    }

    /// The next message, or `None` once the subscription is closed and drained.
    pub async fn recv(&mut self) -> Option<(Subject, Envelope)> {
        self.rx.recv().await
    }

    /// The next message if one is already queued.
    pub fn try_recv(&mut self) -> Option<(Subject, Envelope)> {
        self.rx.try_recv().ok()
    }

    /// How many messages were lost to a full queue since the last call.
    pub fn lagged(&self) -> u64 {
        self.lagged.swap(0, Ordering::Relaxed)
    }

    /// Stop receiving; already queued messages can still be read.
    pub fn close(&mut self) {
        self.rx.close();
    }
}

impl SubscriptionSender {
    /// Never blocks (rule 3).
    pub fn deliver(&self, subject: &Subject, envelope: &Envelope) -> Delivery {
        match self.tx.try_send((subject.clone(), envelope.clone())) {
            Ok(()) => Delivery::Delivered,
            Err(mpsc::error::TrySendError::Full(_)) => {
                self.lagged.fetch_add(1, Ordering::Relaxed);
                Delivery::Lagged
            }
            Err(mpsc::error::TrySendError::Closed(_)) => Delivery::Closed,
        }
    }

    pub fn is_closed(&self) -> bool {
        self.tx.is_closed()
    }
}
