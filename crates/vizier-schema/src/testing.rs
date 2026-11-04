//! Test doubles for role tests: a scripted peer that plays the neighbours of the role under
//! test. It is a plain module, not behind a feature, because every role crate uses it.

use std::sync::Arc;
use std::time::Duration;

use tokio::task::JoinHandle;

use crate::bus::Bus;
use crate::envelope::Envelope;
use crate::error::BusError;
use crate::messaging::{Messaging, Subscription, reply};
use crate::subject::{Subject, SubjectPattern};

/// Plays the other roles on the bus: answers requests, publishes, and asserts on what the role
/// under test sent.
pub struct ScriptedPeer {
    messaging: Arc<dyn Messaging>,
    tasks: Vec<JoinHandle<()>>,
}

/// A subscription made before the action it watches for, so nothing can slip past.
pub struct Listener {
    sub: Subscription,
}

impl ScriptedPeer {
    pub fn new(bus: &Bus) -> Self {
        Self { messaging: bus.messaging.clone(), tasks: Vec::new() }
    }

    /// Answer every request on `pattern` with what `handler` returns; `None` stays silent.
    /// The subscription exists when this returns, so a request sent next is never missed.
    pub async fn on<F>(&mut self, pattern: &str, handler: F) -> Result<(), BusError>
    where
        F: Fn(&Subject, &Envelope) -> Option<Envelope> + Send + Sync + 'static,
    {
        let mut sub = self.messaging.subscribe(SubjectPattern::parse(pattern)?).await?;
        let messaging = self.messaging.clone();
        self.tasks.push(tokio::spawn(async move {
            while let Some((subject, request)) = sub.recv().await {
                if let Some(answer) = handler(&subject, &request) {
                    let _ = reply(messaging.as_ref(), &request, answer).await;
                }
            }
        }));
        Ok(())
    }

    /// Start watching `pattern`; call [`Listener::expect`] after the action.
    pub async fn listen(&self, pattern: &str) -> Result<Listener, BusError> {
        Ok(Listener { sub: self.messaging.subscribe(SubjectPattern::parse(pattern)?).await? })
    }

    pub async fn publish(&self, subject: &Subject, envelope: Envelope) -> Result<(), BusError> {
        self.messaging.publish(subject, envelope).await
    }

    pub async fn request(
        &self,
        subject: &Subject,
        envelope: Envelope,
        timeout: Duration,
    ) -> Result<Envelope, BusError> {
        self.messaging.request(subject, envelope, timeout).await
    }

    /// Answer a request the peer received some other way.
    pub async fn reply(&self, request: &Envelope, response: Envelope) -> Result<(), BusError> {
        reply(self.messaging.as_ref(), request, response).await
    }
}

impl Listener {
    /// The next message within `timeout`, or [`BusError::Timeout`].
    pub async fn expect(&mut self, timeout: Duration) -> Result<(Subject, Envelope), BusError> {
        match tokio::time::timeout(timeout, self.sub.recv()).await {
            Ok(Some(message)) => Ok(message),
            Ok(None) => Err(BusError::Closed),
            Err(_) => Err(BusError::Timeout),
        }
    }

    /// `Ok` when nothing arrives within `quiet`; otherwise the message that did.
    pub async fn expect_silence(&mut self, quiet: Duration) -> Result<(), Box<(Subject, Envelope)>> {
        match tokio::time::timeout(quiet, self.sub.recv()).await {
            Ok(Some(message)) => Err(Box::new(message)),
            _ => Ok(()),
        }
    }
}

impl Drop for ScriptedPeer {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InProcess;

    #[tokio::test(start_paused = true)]
    async fn a_peer_answers_a_request() {
        let bus = InProcess::new().bus();
        let mut peer = ScriptedPeer::new(&bus);
        peer.on("svc.*", |_, req| Envelope::new("answer", req.payload.clone()).ok()).await.unwrap();
        let answer = bus
            .messaging
            .request(
                &Subject::parse("svc.x").unwrap(),
                Envelope::new("ask", vec![7]).unwrap(),
                Duration::from_secs(1),
            )
            .await
            .unwrap();
        assert_eq!(answer.payload, vec![7]);
    }

    #[tokio::test(start_paused = true)]
    async fn a_listener_sees_what_was_published_and_what_was_not() {
        let bus = InProcess::new().bus();
        let peer = ScriptedPeer::new(&bus);
        let mut listener = peer.listen("out.>").await.unwrap();
        assert!(listener.expect_silence(Duration::from_millis(10)).await.is_ok());
        bus.messaging
            .publish(&Subject::parse("out.a").unwrap(), Envelope::new("k", vec![]).unwrap())
            .await
            .unwrap();
        let (subject, _) = listener.expect(Duration::from_secs(1)).await.unwrap();
        assert_eq!(subject.as_str(), "out.a");
        assert_eq!(listener.expect(Duration::from_secs(1)).await.unwrap_err(), BusError::Timeout);
    }
}
