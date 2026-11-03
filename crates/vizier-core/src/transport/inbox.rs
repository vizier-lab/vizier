//! An agent's request queue: a shared subscription to `agent.{id}.req`.
//!
//! It is cloneable and every clone reads the same queue, because the receiver outlives any one
//! supervisor: it is handed from instance to instance across updates so requests sent during a
//! swap wait rather than fail (#43).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::{Mutex, Notify};
use vizier_schema::{Messaging, Subscription};

use super::reply::ResponseSender;
use super::wire::decode_request;
use super::VizierRequestEnvelope;

/// The inbox is closed and drained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InboxClosed;

impl std::fmt::Display for InboxClosed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the agent's request queue is closed")
    }
}

impl std::error::Error for InboxClosed {}

struct Shared {
    sub: Mutex<Subscription>,
    closed: AtomicBool,
    wake: Notify,
}

#[derive(Clone)]
pub struct Inbox {
    shared: Arc<Shared>,
    messaging: Arc<dyn Messaging>,
}

impl Inbox {
    pub(super) fn new(sub: Subscription, messaging: Arc<dyn Messaging>) -> Self {
        Self {
            shared: Arc::new(Shared { sub: Mutex::new(sub), closed: AtomicBool::new(false), wake: Notify::new() }),
            messaging,
        }
    }

    /// The next request, oldest first, with its reply stream attached. `Err` once the inbox has
    /// been closed and everything queued before the close has been read.
    pub async fn recv_request(&self) -> Result<VizierRequestEnvelope, InboxClosed> {
        let mut sub = self.shared.sub.lock().await;
        loop {
            let message = match sub.try_recv() {
                Some(message) => Some(message),
                None if self.is_disconnected() => {
                    // Stop the bus routing to a queue nobody reads.
                    sub.close();
                    return Err(InboxClosed);
                }
                None => {
                    let woken = self.shared.wake.notified();
                    tokio::pin!(woken);
                    // Registered before the check below, so a close cannot slip between them.
                    woken.as_mut().enable();
                    if self.is_disconnected() {
                        continue;
                    }
                    tokio::select! {
                        message = sub.recv() => match message {
                            Some(message) => Some(message),
                            None => return Err(InboxClosed),
                        },
                        _ = woken => continue,
                    }
                }
            };
            let Some((_, envelope)) = message else { continue };
            match decode_request(&envelope) {
                Ok(mut request) => {
                    request.response_tx = envelope
                        .reply_to
                        .clone()
                        .map(|subject| ResponseSender::to_subject(self.messaging.clone(), subject));
                    return Ok(request);
                }
                Err(e) => tracing::warn!("dropping an undecodable request: {e}"),
            }
        }
    }

    /// Close the inbox: what is queued can still be read, then `recv_request` ends.
    pub fn close(&self) {
        self.shared.closed.store(true, Ordering::SeqCst);
        // Stop the bus feeding a queue that is closing. A reader waiting on the queue holds its
        // lock; it closes the subscription itself when the wake below reaches it.
        if let Ok(mut sub) = self.shared.sub.try_lock() {
            sub.close();
        }
        self.shared.wake.notify_waiters();
    }

    /// Closed by `unregister_agent`. Like a flume receiver whose sender is gone.
    pub fn is_disconnected(&self) -> bool {
        self.shared.closed.load(Ordering::SeqCst)
    }
}

impl std::fmt::Debug for Inbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Inbox")
    }
}
