//! The in-process backend: the default, and the reference every other backend is measured
//! against by [`crate::conformance`].
//!
//! Everything lives in memory behind a few mutexes held only for the span of a map operation
//! (never across an `.await`), so a publisher never waits on a subscriber (rule 3).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use async_trait::async_trait;

use crate::envelope::Envelope;
use crate::error::BusError;
use crate::messaging::{Delivery, Messaging, SUBSCRIPTION_DEPTH, Subscription, SubscriptionSender};
use crate::state::{Revision, SharedState, StateEvent, StateWatch, Versioned};
use crate::subject::{Subject, SubjectPattern};
use crate::watch::{Watch, WatchSender};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    // A poisoned map is still a consistent map: every critical section is a single operation.
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Default)]
pub(crate) struct StateStore {
    entries: BTreeMap<String, Versioned>,
    revision: u64,
    watchers: Vec<(String, WatchSender<StateEvent>)>,
}

impl StateStore {
    fn emit(&mut self, key: &str, event: StateEvent) {
        self.watchers.retain(|(prefix, tx)| {
            if tx.is_closed() {
                return false;
            }
            if key.starts_with(prefix.as_str()) {
                tx.send(event.clone());
            }
            true
        });
    }

    fn write(&mut self, key: &str, value: Vec<u8>) -> Revision {
        self.revision += 1;
        let revision = Revision(self.revision);
        let versioned = Versioned { revision, value };
        self.entries.insert(key.to_string(), versioned.clone());
        self.emit(key, StateEvent::Put { key: key.to_string(), value: versioned });
        revision
    }
}

pub(crate) struct Inner {
    pub(crate) subs: Mutex<Vec<(SubjectPattern, SubscriptionSender)>>,
    pub(crate) state: Mutex<StateStore>,
    pub(crate) leases: Mutex<crate::inprocess_leases::LeaseTable>,
    pub(crate) depth: usize,
}

/// The in-process messaging, lease and shared-state backend. Cloning shares the same bus.
#[derive(Clone)]
pub struct InProcess {
    pub(crate) inner: Arc<Inner>,
}

impl Default for InProcess {
    fn default() -> Self {
        Self::new()
    }
}

impl InProcess {
    pub fn new() -> Self {
        Self::with_depth(SUBSCRIPTION_DEPTH)
    }

    /// A bus whose subscriptions lose messages once `depth` are queued; for testing lag.
    pub fn with_depth(depth: usize) -> Self {
        Self {
            inner: Arc::new(Inner {
                subs: Mutex::new(Vec::new()),
                state: Mutex::new(StateStore::default()),
                leases: Mutex::new(Default::default()),
                depth,
            }),
        }
    }

    /// Hand `envelope` to every live matching subscription, in order, without waiting.
    /// Returns how many subscriptions matched.
    fn route(&self, subject: &Subject, envelope: &Envelope) -> usize {
        let mut subs = lock(&self.inner.subs);
        subs.retain(|(_, tx)| !tx.is_closed());
        let mut matched = 0;
        for (pattern, tx) in subs.iter() {
            if pattern.matches(subject) {
                matched += 1;
                let _: Delivery = tx.deliver(subject, envelope);
            }
        }
        matched
    }

    fn add_subscription(&self, pattern: SubjectPattern) -> Subscription {
        let (tx, rx) = Subscription::channel(self.inner.depth);
        lock(&self.inner.subs).push((pattern, tx));
        rx
    }
}

#[async_trait]
impl Messaging for InProcess {
    async fn publish(&self, subject: &Subject, envelope: Envelope) -> Result<(), BusError> {
        envelope.check_size()?;
        self.route(subject, &envelope);
        Ok(())
    }

    async fn request(
        &self,
        subject: &Subject,
        mut envelope: Envelope,
        timeout: Duration,
    ) -> Result<Envelope, BusError> {
        envelope.check_size()?;
        let reply_subject = Subject::reply(uuid::Uuid::new_v4());
        // Subscribe before publishing so a fast responder cannot answer into the void.
        let mut replies = self.add_subscription(SubjectPattern::from(&reply_subject));
        envelope.reply_to = Some(reply_subject);
        if self.route(subject, &envelope) == 0 {
            return Err(BusError::NoResponders);
        }
        match tokio::time::timeout(timeout, replies.recv()).await {
            Ok(Some((_, reply))) => Ok(reply),
            Ok(None) => Err(BusError::Closed),
            Err(_) => Err(BusError::Timeout),
        }
    }

    async fn subscribe(&self, pattern: SubjectPattern) -> Result<Subscription, BusError> {
        Ok(self.add_subscription(pattern))
    }
}

#[async_trait]
impl SharedState for InProcess {
    async fn get(&self, key: &str) -> Result<Option<Versioned>, BusError> {
        Ok(lock(&self.inner.state).entries.get(key).cloned())
    }

    async fn put(&self, key: &str, value: Vec<u8>) -> Result<Revision, BusError> {
        Ok(lock(&self.inner.state).write(key, value))
    }

    async fn cas(
        &self,
        key: &str,
        expected: Option<Revision>,
        value: Vec<u8>,
    ) -> Result<Revision, BusError> {
        let mut state = lock(&self.inner.state);
        let current = state.entries.get(key).map(|v| v.revision);
        if current != expected {
            return Err(BusError::Conflict);
        }
        Ok(state.write(key, value))
    }

    async fn delete(&self, key: &str) -> Result<(), BusError> {
        let mut state = lock(&self.inner.state);
        if state.entries.remove(key).is_some() {
            state.emit(key, StateEvent::Delete { key: key.to_string() });
        }
        Ok(())
    }

    async fn watch(&self, prefix: &str) -> Result<StateWatch, BusError> {
        let (tx, rx) = Watch::channel();
        lock(&self.inner.state).watchers.push((prefix.to_string(), tx));
        Ok(rx)
    }
}

impl InProcess {
    /// This backend as a [`crate::Bus`] advertising every capability.
    pub fn bus(&self) -> crate::Bus {
        crate::Bus::new(
            Arc::new(self.clone()),
            Arc::new(self.clone()),
            Arc::new(self.clone()),
            crate::Capabilities::all(),
        )
    }
}
