//! The in-process backend passes the whole conformance suite.

use vizier_schema::conformance::{Outcome, run};
use vizier_schema::{InProcess, Bus};

fn backend() -> Bus {
    InProcess::new().bus()
}

#[tokio::test(start_paused = true)]
async fn in_process_passes_conformance() {
    let report = run(backend).await;
    assert!(report.is_ok(), "\n{report}");
    assert_eq!(report.passed(), 15, "\n{report}");
    assert!(report.results.iter().all(|r| r.outcome == Outcome::Passed));
}
