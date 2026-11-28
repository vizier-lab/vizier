//! `specs/071-triggers` T024, SC-007: a webhook's secret, in the address or in a header, never
//! reaches a log line or a span. Its own test binary, because the subscriber it records with is
//! the process's.

use std::{
    io::Write,
    sync::{Arc, Mutex},
};

use axum::{body::Body, http::Request};
use serde_json::json;
use tower::ServiceExt;
use tracing_subscriber::fmt::format::FmtSpan;
use vizier_core::{
    config::VizierConfig,
    runtime::{RuntimeOptions, VizierRuntime},
    schema::{OverlapPolicy, Requester, Trigger},
    storage::trigger::TriggerStorage,
};

#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

const SECRET: &str = "s3cr3t-0123456789abcdef-never-logged";

#[tokio::test]
async fn a_webhook_secret_never_reaches_logs_or_spans() {
    let captured = Captured::default();
    let writer = captured.clone();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_span_events(FmtSpan::FULL)
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .try_init()
        .expect("the only subscriber of this binary");

    let dir = tempfile::tempdir().unwrap();
    let mut config = VizierConfig::with_workspace(dir.path().to_path_buf());
    config.channels.http.as_mut().unwrap().jwt_secret = "triggers-secret".into();
    config.catalogue.refresh = false;
    let runtime = VizierRuntime::start(RuntimeOptions { config, ..Default::default() }).await.unwrap();
    let http = runtime.deps().config.channels.http.clone().unwrap();
    runtime
        .deps()
        .storage
        .create_trigger(&Trigger {
            id: "trg_logs".into(),
            agent_id: "a".into(),
            slug: "ci".into(),
            kind: "webhook".into(),
            title: "CI".into(),
            instruction: "Tell me".into(),
            settings: json!({ "token": SECRET }),
            requester: Requester::User("alice".into()),
            delivers_to: None,
            is_active: true,
            awaiting_approval: false,
            needs_attention: None,
            tool_scope: None,
            overlap: OverlapPolicy::Wait,
            hourly_limit: Some(20),
            cooldown_secs: 0,
            triage: None,
            destination: None,
            summary_hours: None,
            last_summary_at: None,
            created_at: vizier_core::time::now(),
            last_fired_at: None,
        })
        .await
        .unwrap();
    let router = vizier_http::router(&runtime, &http);

    for (path, auth, expected) in [
        (format!("/api/v1/hooks/trg_logs/{SECRET}"), None, 202),
        ("/api/v1/hooks/trg_logs".to_string(), Some(format!("Bearer {SECRET}")), 202),
        // A miss that carries the secret, to an address that does not exist.
        (format!("/api/v1/hooks/trg_nothere/{SECRET}"), None, 404),
    ] {
        let mut request = Request::builder().method("POST").uri(path).header("content-type", "application/json");
        if let Some(auth) = auth {
            request = request.header("authorization", auth);
        }
        let response = router.clone().oneshot(request.body(Body::from("{}")).unwrap()).await.unwrap();
        assert_eq!(response.status().as_u16(), expected);
    }
    // Let what the events set going log too.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    runtime.shutdown().await.unwrap();

    let logs = String::from_utf8_lossy(&captured.0.lock().unwrap()).into_owned();
    assert!(!logs.is_empty(), "something was captured");
    let leaked: Vec<&str> = logs.lines().filter(|line| line.contains(SECRET)).take(5).collect();
    assert!(leaked.is_empty(), "{leaked:#?}");
}
