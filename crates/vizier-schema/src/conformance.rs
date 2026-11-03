//! The backend conformance suite (contracts/conformance-suite.md).
//!
//! `run(make_backend)` builds a fresh [`Bus`] per check and reports each named check. A backend
//! crate runs it from its own test; the in-process backend runs it in this crate's tests.
//!
//! **Time:** the suite waits out TTLs with `tokio::time::sleep`, so run it on a
//! current-thread runtime started with `start_paused = true`. TTLs then expire deterministically
//! and the whole suite takes milliseconds.

use std::fmt::Display;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use crate::bus::Bus;
use crate::capabilities::Capability;
use crate::envelope::{Envelope, MAX_PAYLOAD};
use crate::error::BusError;
use crate::leases::LeaseEvent;
use crate::messaging::{Subscription, reply};
use crate::state::{Revision, StateEvent};
use crate::subject::{Subject, SubjectPattern};

/// What one check came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Passed,
    Failed(String),
    /// The backend does not advertise the capability, so the group was not run.
    Skipped(Capability),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckResult {
    pub name: &'static str,
    pub outcome: Outcome,
}

/// All results of one run.
#[derive(Debug, Clone, Default)]
pub struct Report {
    pub results: Vec<CheckResult>,
}

impl Report {
    pub fn outcome(&self, name: &str) -> Option<&Outcome> {
        self.results.iter().find(|r| r.name == name).map(|r| &r.outcome)
    }

    pub fn failed(&self) -> Vec<&CheckResult> {
        self.results.iter().filter(|r| matches!(r.outcome, Outcome::Failed(_))).collect()
    }

    pub fn passed(&self) -> usize {
        self.results.iter().filter(|r| r.outcome == Outcome::Passed).count()
    }

    pub fn is_ok(&self) -> bool {
        self.failed().is_empty()
    }
}

impl std::fmt::Display for Report {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for r in &self.results {
            match &r.outcome {
                Outcome::Passed => writeln!(f, "ok      {}", r.name)?,
                Outcome::Failed(why) => writeln!(f, "FAILED  {}: {why}", r.name)?,
                Outcome::Skipped(c) => writeln!(f, "skipped {} ({c})", r.name)?,
            }
        }
        Ok(())
    }
}

type Verdict = Result<(), String>;
type Check = fn(Bus) -> Pin<Box<dyn Future<Output = Verdict> + Send>>;

macro_rules! check {
    ($f:ident) => {
        |bus| Box::pin($f(bus))
    };
}

fn checks() -> Vec<(&'static str, Capability, Check)> {
    use Capability::*;
    vec![
        ("request_reply", Messaging, check!(request_reply)),
        ("no_responder_error", Messaging, check!(no_responder_error)),
        ("request_timeout", Messaging, check!(request_timeout)),
        ("per_subject_ordering", Messaging, check!(per_subject_ordering)),
        ("wildcard_matching", Messaging, check!(wildcard_matching)),
        ("id_encoding_is_one_token", Messaging, check!(id_encoding_is_one_token)),
        ("payload_limit_refusal", Messaging, check!(payload_limit_refusal)),
        ("publish_without_subscribers", Messaging, check!(publish_without_subscribers)),
        ("lease_exclusive", Leases, check!(lease_exclusive)),
        ("lease_expiry_and_takeover", Leases, check!(lease_expiry_and_takeover)),
        ("fencing_token_monotonic", Leases, check!(fencing_token_monotonic)),
        ("lease_renew_after_loss_fails", Leases, check!(lease_renew_after_loss_fails)),
        ("lease_watch", Leases, check!(lease_watch)),
        ("state_cas", SharedState, check!(state_cas)),
        ("state_watch", SharedState, check!(state_watch)),
    ]
}

/// Run every check against a fresh bus from `make_backend`.
pub async fn run(make_backend: impl Fn() -> Bus) -> Report {
    let mut report = Report::default();
    for (name, capability, check) in checks() {
        let bus = make_backend();
        let outcome = if !bus.capabilities.has(capability) {
            Outcome::Skipped(capability)
        } else {
            match check(bus).await {
                Ok(()) => Outcome::Passed,
                Err(why) => Outcome::Failed(why),
            }
        };
        report.results.push(CheckResult { name, outcome });
    }
    report
}

// --- helpers -------------------------------------------------------------------------------

fn s<E: Display>(e: E) -> String {
    e.to_string()
}

fn ensure(cond: bool, msg: impl Into<String>) -> Verdict {
    if cond { Ok(()) } else { Err(msg.into()) }
}

fn subject(x: &str) -> Subject {
    Subject::parse(x).expect("a literal subject")
}

fn pattern(x: &str) -> SubjectPattern {
    SubjectPattern::parse(x).expect("a literal pattern")
}

fn msg(payload: Vec<u8>) -> Envelope {
    Envelope::new("conformance", payload).expect("under the limit")
}

async fn next(sub: &mut Subscription, wait: Duration) -> Option<(Subject, Envelope)> {
    tokio::time::timeout(wait, sub.recv()).await.ok().flatten()
}

/// Everything that arrives within a short quiet period.
async fn drain(sub: &mut Subscription) -> Vec<Subject> {
    let mut got = Vec::new();
    while let Some((subject, _)) = next(sub, Duration::from_millis(50)).await {
        got.push(subject);
    }
    got
}

const SECOND: Duration = Duration::from_secs(1);

// --- messaging -----------------------------------------------------------------------------

async fn request_reply(bus: Bus) -> Verdict {
    let m = bus.messaging.clone();
    let mut sub = m.subscribe(pattern("svc.echo")).await.map_err(s)?;
    let responder = m.clone();
    tokio::spawn(async move {
        while let Some((_, request)) = sub.recv().await {
            let answer = msg(request.payload.iter().rev().copied().collect());
            let _ = reply(responder.as_ref(), &request, answer).await;
        }
    });
    let answer = m.request(&subject("svc.echo"), msg(vec![1, 2, 3]), SECOND * 5).await.map_err(s)?;
    ensure(answer.payload == vec![3, 2, 1], format!("unexpected reply {:?}", answer.payload))
}

async fn no_responder_error(bus: Bus) -> Verdict {
    let started = tokio::time::Instant::now();
    let result = bus.messaging.request(&subject("nobody.home"), msg(vec![]), SECOND * 30).await;
    ensure(result.as_ref().err() == Some(&BusError::NoResponders), format!("got {result:?}"))?;
    ensure(started.elapsed() < SECOND * 5, "NoResponders was not prompt")
}

async fn request_timeout(bus: Bus) -> Verdict {
    // Subscribed, never answers.
    let _silent = bus.messaging.subscribe(pattern("svc.silent")).await.map_err(s)?;
    let result = bus
        .messaging
        .request(&subject("svc.silent"), msg(vec![]), Duration::from_millis(50))
        .await;
    ensure(result.as_ref().err() == Some(&BusError::Timeout), format!("got {result:?}"))
}

async fn per_subject_ordering(bus: Bus) -> Verdict {
    let mut sub = bus.messaging.subscribe(pattern("ord.stream")).await.map_err(s)?;
    let target = subject("ord.stream");
    for i in 0u32..1000 {
        bus.messaging.publish(&target, msg(i.to_be_bytes().to_vec())).await.map_err(s)?;
    }
    for i in 0u32..1000 {
        let Some((_, got)) = next(&mut sub, SECOND * 5).await else {
            return Err(format!("only {i} of 1000 messages arrived"));
        };
        ensure(got.payload == i.to_be_bytes(), format!("message {i} arrived out of order"))?;
    }
    Ok(())
}

async fn wildcard_matching(bus: Bus) -> Verdict {
    let m = &bus.messaging;
    let mut star = m.subscribe(pattern("w.*")).await.map_err(s)?;
    let mut rest = m.subscribe(pattern("w.>")).await.map_err(s)?;
    let mut exact = m.subscribe(pattern("w.a.b")).await.map_err(s)?;
    for target in ["w.a", "w.a.b", "w.a.b.c"] {
        m.publish(&subject(target), msg(vec![])).await.map_err(s)?;
    }
    let names = |v: Vec<Subject>| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    let (star, rest, exact) = (names(drain(&mut star).await), names(drain(&mut rest).await), names(drain(&mut exact).await));
    ensure(star == ["w.a"], format!("`w.*` received {star:?}"))?;
    ensure(rest == ["w.a", "w.a.b", "w.a.b.c"], format!("`w.>` received {rest:?}"))?;
    ensure(exact == ["w.a.b"], format!("`w.a.b` received {exact:?}"))
}

async fn id_encoding_is_one_token(bus: Bus) -> Verdict {
    let m = &bus.messaging;
    let mut sibling = m.subscribe(pattern("agent.my.>")).await.map_err(s)?;
    let mut any_agent = m.subscribe(pattern("agent.*.req")).await.map_err(s)?;
    m.publish(&Subject::agent_req("my.agent"), msg(vec![])).await.map_err(s)?;
    let leaked = drain(&mut sibling).await;
    ensure(leaked.is_empty(), format!("an id with a dot reached a sibling's pattern: {leaked:?}"))?;
    let seen = drain(&mut any_agent).await;
    ensure(seen.len() == 1, format!("`agent.*.req` received {seen:?}"))
}

async fn payload_limit_refusal(bus: Bus) -> Verdict {
    let m = &bus.messaging;
    let target = subject("limit.test");
    let mut sub = m.subscribe(pattern("limit.test")).await.map_err(s)?;

    m.publish(&target, msg(vec![0; MAX_PAYLOAD])).await.map_err(|e| format!("limit bytes refused: {e}"))?;
    ensure(next(&mut sub, SECOND).await.is_some(), "a payload of exactly the limit was not delivered")?;

    // Built field by field: the constructor would refuse it before the backend ever saw it.
    let mut over = msg(vec![]);
    over.payload = vec![0; MAX_PAYLOAD + 1];
    let result = m.publish(&target, over).await;
    ensure(matches!(result, Err(BusError::PayloadTooLarge { .. })), format!("limit+1 gave {result:?}"))?;
    ensure(next(&mut sub, Duration::from_millis(50)).await.is_none(), "a refused payload was still delivered")
}

async fn publish_without_subscribers(bus: Bus) -> Verdict {
    bus.messaging.publish(&subject("empty.room"), msg(vec![1])).await.map_err(s)
}

// --- leases --------------------------------------------------------------------------------

const TTL: Duration = Duration::from_secs(10);

/// Wait past a TTL (instant on a paused clock) and let any expiry task run.
async fn pass_ttl() {
    tokio::time::sleep(TTL + SECOND).await;
    for _ in 0..4 {
        tokio::task::yield_now().await;
    }
}

async fn lease_exclusive(bus: Bus) -> Verdict {
    let _held = bus.leases.acquire("leader", TTL).await.map_err(s)?;
    let second = bus.leases.acquire("leader", TTL).await;
    ensure(second.as_ref().err() == Some(&BusError::LeaseHeld), format!("second acquire gave {second:?}"))?;
    bus.leases.acquire("other", TTL).await.map(|_| ()).map_err(|e| format!("an unrelated name was refused: {e}"))
}

async fn lease_expiry_and_takeover(bus: Bus) -> Verdict {
    let _first = bus.leases.acquire("leader", TTL).await.map_err(s)?;
    pass_ttl().await;
    bus.leases.acquire("leader", TTL).await.map(|_| ()).map_err(|e| format!("takeover after the TTL failed: {e}"))
}

async fn fencing_token_monotonic(bus: Bus) -> Verdict {
    let mut last = None;
    for round in 0..5 {
        let lease = bus.leases.acquire("leader", TTL).await.map_err(s)?;
        if let Some(previous) = last {
            ensure(lease.token > previous, format!("round {round}: {:?} is not above {previous:?}", lease.token))?;
        }
        last = Some(lease.token);
        // Alternate the two ways a holder goes away.
        if round % 2 == 0 {
            bus.leases.release(lease).await.map_err(s)?;
        } else {
            pass_ttl().await;
        }
    }
    Ok(())
}

async fn lease_renew_after_loss_fails(bus: Bus) -> Verdict {
    let old = bus.leases.acquire("leader", TTL).await.map_err(s)?;
    pass_ttl().await;
    let _new = bus.leases.acquire("leader", TTL).await.map_err(s)?;
    ensure(bus.leases.renew(&old).await.is_err(), "the old holder renewed a lease that was taken over")
}

async fn lease_watch(bus: Bus) -> Verdict {
    let mut watch = bus.leases.watch("leader").await.map_err(s)?;
    let wait = SECOND * 20;
    let mut event = async |what: &str| -> Result<LeaseEvent, String> {
        tokio::time::timeout(wait, watch.recv())
            .await
            .ok()
            .flatten()
            .ok_or_else(|| format!("no {what} event"))
    };

    let lease = bus.leases.acquire("leader", TTL).await.map_err(s)?;
    let got = event("acquire").await?;
    ensure(matches!(got, LeaseEvent::Acquired { token, .. } if token == lease.token), format!("expected Acquired, got {got:?}"))?;

    bus.leases.release(lease).await.map_err(s)?;
    let got = event("release").await?;
    ensure(got == LeaseEvent::Released, format!("expected Released, got {got:?}"))?;

    let _again = bus.leases.acquire("leader", TTL).await.map_err(s)?;
    let got = event("second acquire").await?;
    ensure(matches!(got, LeaseEvent::Acquired { .. }), format!("expected Acquired, got {got:?}"))?;
    pass_ttl().await;
    let got = event("expiry").await?;
    ensure(got == LeaseEvent::Expired, format!("expected Expired, got {got:?}"))
}

// --- shared state --------------------------------------------------------------------------

async fn state_cas(bus: Bus) -> Verdict {
    let st = &bus.state;
    let r1 = st.put("k", b"one".to_vec()).await.map_err(s)?;
    let r2 = st.cas("k", Some(r1), b"two".to_vec()).await.map_err(|e| format!("cas on the current revision: {e}"))?;
    ensure(r2 > r1, format!("revision did not increase: {r1:?} then {r2:?}"))?;
    let stale = st.cas("k", Some(r1), b"three".to_vec()).await;
    ensure(stale.as_ref().err() == Some(&BusError::Conflict), format!("cas on a stale revision gave {stale:?}"))?;
    let exists = st.cas("k", None, b"three".to_vec()).await;
    ensure(exists.as_ref().err() == Some(&BusError::Conflict), format!("cas-create over an existing key gave {exists:?}"))?;
    st.cas("fresh", None, b"x".to_vec()).await.map_err(|e| format!("cas-create of a new key: {e}"))?;
    let current = st.get("k").await.map_err(s)?;
    ensure(current.map(|v| v.value) == Some(b"two".to_vec()), "a refused cas changed the value")?;
    let _: Revision = r2;
    Ok(())
}

async fn state_watch(bus: Bus) -> Verdict {
    let st = &bus.state;
    let mut watch = st.watch("k/").await.map_err(s)?;
    st.put("k/a", b"1".to_vec()).await.map_err(s)?;
    st.put("other", b"x".to_vec()).await.map_err(s)?;
    st.delete("k/a").await.map_err(s)?;
    let mut events = Vec::new();
    while let Ok(Some(event)) = tokio::time::timeout(Duration::from_millis(50), watch.recv()).await {
        events.push(event);
    }
    let shape: Vec<_> = events
        .iter()
        .map(|e| match e {
            StateEvent::Put { key, .. } => format!("put {key}"),
            StateEvent::Delete { key } => format!("delete {key}"),
        })
        .collect();
    ensure(shape == ["put k/a", "delete k/a"], format!("watcher saw {shape:?}"))
}
