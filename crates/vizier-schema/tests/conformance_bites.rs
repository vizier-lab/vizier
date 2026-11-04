//! The suite must bite: a deliberately faulty backend is reported as failing the matching check.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use vizier_schema::conformance::{Outcome, Report, run};
use vizier_schema::{
    Bus, BusError, Capabilities, Envelope, FencingToken, InProcess, Lease, LeaseWatch, Leases,
    MAX_PAYLOAD, Messaging, Revision, SharedState, StateWatch, Subject, SubjectPattern,
    Subscription, Versioned,
};

#[derive(Clone, Copy)]
enum Fault {
    TimeoutInsteadOfNoResponders,
    ReorderMessages,
    IgnorePayloadLimit,
    ReuseFencingToken,
}

struct Faulty {
    inner: InProcess,
    fault: Fault,
    stash: Mutex<Option<(Subject, Envelope)>>,
}

fn faulty_bus(fault: Fault) -> Bus {
    let backend = Arc::new(Faulty { inner: InProcess::new(), fault, stash: Mutex::new(None) });
    Bus::new(backend.clone(), backend.clone(), backend, Capabilities::all())
}

#[async_trait]
impl Messaging for Faulty {
    async fn publish(&self, subject: &Subject, mut envelope: Envelope) -> Result<(), BusError> {
        match self.fault {
            Fault::ReorderMessages => {
                let held = {
                    let mut stash = self.stash.lock().unwrap();
                    match stash.take() {
                        None => {
                            *stash = Some((subject.clone(), envelope));
                            return Ok(());
                        }
                        Some(held) => held,
                    }
                };
                // The newer message overtakes the one held back.
                self.inner.publish(subject, envelope).await?;
                self.inner.publish(&held.0, held.1).await
            }
            Fault::IgnorePayloadLimit => {
                envelope.payload.truncate(MAX_PAYLOAD);
                self.inner.publish(subject, envelope).await
            }
            _ => self.inner.publish(subject, envelope).await,
        }
    }

    async fn request(
        &self,
        subject: &Subject,
        envelope: Envelope,
        timeout: Duration,
    ) -> Result<Envelope, BusError> {
        let result = self.inner.request(subject, envelope, timeout).await;
        match (self.fault, result) {
            (Fault::TimeoutInsteadOfNoResponders, Err(BusError::NoResponders)) => Err(BusError::Timeout),
            (_, other) => other,
        }
    }

    async fn subscribe(&self, pattern: SubjectPattern) -> Result<Subscription, BusError> {
        self.inner.subscribe(pattern).await
    }
}


#[async_trait]
impl Leases for Faulty {
    async fn acquire(&self, name: &str, ttl: Duration) -> Result<Lease, BusError> {
        let mut lease = self.inner.acquire(name, ttl).await?;
        if matches!(self.fault, Fault::ReuseFencingToken) {
            lease.token = FencingToken(1);
        }
        Ok(lease)
    }
    async fn renew(&self, lease: &Lease) -> Result<Lease, BusError> {
        self.inner.renew(lease).await
    }
    async fn release(&self, lease: Lease) -> Result<(), BusError> {
        self.inner.release(lease).await
    }
    async fn watch(&self, name: &str) -> Result<LeaseWatch, BusError> {
        Leases::watch(&self.inner, name).await
    }
}

#[async_trait]
impl SharedState for Faulty {
    async fn get(&self, key: &str) -> Result<Option<Versioned>, BusError> {
        self.inner.get(key).await
    }
    async fn put(&self, key: &str, value: Vec<u8>) -> Result<Revision, BusError> {
        self.inner.put(key, value).await
    }
    async fn cas(&self, key: &str, expected: Option<Revision>, value: Vec<u8>) -> Result<Revision, BusError> {
        self.inner.cas(key, expected, value).await
    }
    async fn delete(&self, key: &str) -> Result<(), BusError> {
        self.inner.delete(key).await
    }
    async fn watch(&self, prefix: &str) -> Result<StateWatch, BusError> {
        SharedState::watch(&self.inner, prefix).await
    }
}

fn assert_fails(report: &Report, check: &str) {
    assert!(
        matches!(report.outcome(check), Some(Outcome::Failed(_))),
        "{check} should have failed:\n{report}"
    );
}

#[tokio::test(start_paused = true)]
async fn a_backend_that_times_out_instead_of_no_responders_is_caught() {
    let report = run(|| faulty_bus(Fault::TimeoutInsteadOfNoResponders)).await;
    assert_fails(&report, "no_responder_error");
    assert_eq!(report.failed().len(), 1, "\n{report}");
}

#[tokio::test(start_paused = true)]
async fn a_backend_that_reorders_messages_is_caught() {
    let report = run(|| faulty_bus(Fault::ReorderMessages)).await;
    assert_fails(&report, "per_subject_ordering");
}

#[tokio::test(start_paused = true)]
async fn a_backend_that_ignores_the_payload_limit_is_caught() {
    let report = run(|| faulty_bus(Fault::IgnorePayloadLimit)).await;
    assert_fails(&report, "payload_limit_refusal");
    assert_eq!(report.failed().len(), 1, "\n{report}");
}

#[tokio::test(start_paused = true)]
async fn a_backend_that_reuses_a_fencing_token_is_caught() {
    let report = run(|| faulty_bus(Fault::ReuseFencingToken)).await;
    assert_fails(&report, "fencing_token_monotonic");
}
