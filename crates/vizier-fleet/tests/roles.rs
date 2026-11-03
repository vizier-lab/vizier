//! The sample role test CI runs: one role built alone on an in-process bus, with a scripted peer
//! playing its neighbour. The roles are still skeletons (#166-#170), so what is asserted is the
//! seam every later role test relies on: the role is handed the bus, and what crosses it reaches
//! the peer and comes back.

use std::sync::Arc;
use std::time::Duration;

use vizier_fleet::testing::Harness;
use vizier_fleet::Worker;
use vizier_schema::{Bus, BusError, Capabilities, Capability, Envelope, InProcess, Subject};

const WAIT: Duration = Duration::from_secs(5);

#[tokio::test(start_paused = true)]
async fn a_worker_built_alone_talks_to_a_scripted_peer() {
    let harness = Harness::new().await.expect("harness");
    let worker = harness.build_worker();
    let mut peer = harness.peer();

    // The peer plays the gateway: it answers whatever is asked of an agent.
    peer.on("agent.*.req", |subject, request| {
        let body = format!("{} saw {} bytes", subject, request.payload.len());
        Envelope::new("response_frame", body.into_bytes()).ok()
    })
    .await
    .expect("peer subscribes");

    // The worker reaches the bus through the handle it was built with, and nothing else.
    let answer = worker
        .bus()
        .messaging
        .request(&Subject::agent_req("alpha"), Envelope::new("request", vec![1, 2, 3]).unwrap(), WAIT)
        .await
        .expect("the peer answers");
    assert_eq!(answer.kind, "response_frame");
    assert_eq!(String::from_utf8(answer.payload).unwrap(), "agent.alpha.req saw 3 bytes");
}

#[tokio::test(start_paused = true)]
async fn what_a_role_publishes_reaches_a_listening_peer() {
    let harness = Harness::new().await.expect("harness");
    let control = harness.build_control();
    let peer = harness.peer();
    let mut listener = peer.listen("dream.cmd").await.expect("listen");

    control
        .bus()
        .messaging
        .publish(&Subject::dream_cmd(), Envelope::new("dream_command", b"{}".to_vec()).unwrap())
        .await
        .expect("publish");

    let (subject, envelope) = listener.expect(WAIT).await.expect("the peer hears it");
    assert_eq!(subject, Subject::dream_cmd());
    assert_eq!(envelope.kind, "dream_command");
}

#[tokio::test(start_paused = true)]
async fn a_backend_without_leases_is_refused_for_a_role_that_needs_them() {
    // Messaging and state only: the in-process backend, advertised as less than it is.
    let backend = InProcess::new();
    let bus = Bus::new(
        Arc::new(backend.clone()),
        Arc::new(backend.clone()),
        Arc::new(backend),
        Capabilities { messaging: true, leases: false, shared_state: true },
    );
    let harness = Harness::with_bus(bus).await.expect("harness");

    let err = harness.require_for(&Worker::needs()).unwrap_err();
    assert_eq!(err, BusError::Unsupported(Capability::Leases));
    assert!(err.to_string().contains("leases"));

    // A role that only needs messaging is still fine on the same backend.
    harness.require_for(&vizier_fleet::GatewayHttp::needs()).expect("messaging only");
}
