use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use axum::{Extension, Json, Router, http::StatusCode, routing::get};
use serde::Serialize;

/// Minimal process-health state for workload managers.
///
/// This intentionally does not inspect agents or optional dependencies: those
/// failures must not make Kubernetes restart an otherwise healthy process.
#[derive(Clone, Default)]
pub struct HealthHandle {
    state: Arc<HealthState>,
}

#[derive(Default)]
struct HealthState {
    started: AtomicBool,
    ready: AtomicBool,
}

impl HealthHandle {
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks initialization and the HTTP listener complete.
    pub fn mark_started(&self) {
        self.state.started.store(true, Ordering::Release);
        self.state.ready.store(true, Ordering::Release);
    }

    /// Removes the process from service before HTTP admission closes.
    pub fn begin_shutdown(&self) {
        self.state.ready.store(false, Ordering::Release);
    }

    fn started(&self) -> bool {
        self.state.started.load(Ordering::Acquire)
    }

    fn ready(&self) -> bool {
        self.state.ready.load(Ordering::Acquire)
    }
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

type HealthResult = (StatusCode, Json<HealthResponse>);

fn result(available: bool) -> HealthResult {
    if available {
        (StatusCode::OK, Json(HealthResponse { status: "ok" }))
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(HealthResponse {
                status: "unavailable",
            }),
        )
    }
}

async fn startup(Extension(health): Extension<HealthHandle>) -> HealthResult {
    result(health.started())
}

async fn readiness(Extension(health): Extension<HealthHandle>) -> HealthResult {
    result(health.ready())
}

async fn liveness() -> HealthResult {
    result(true)
}

pub(crate) fn routes<S>(health: HealthHandle) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/health/startup", get(startup))
        .route("/health/ready", get(readiness))
        .route("/health/live", get(liveness))
        .layer(Extension(health))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    use super::*;

    async fn get(app: Router, path: &str) -> (StatusCode, String) {
        let response = app
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn startup_and_readiness_follow_lifecycle() {
        let health = HealthHandle::new();
        let app = routes::<()>(health.clone());

        assert_eq!(
            get(app.clone(), "/health/startup").await,
            (
                StatusCode::SERVICE_UNAVAILABLE,
                r#"{"status":"unavailable"}"#.into()
            )
        );
        assert_eq!(
            get(app.clone(), "/health/ready").await,
            (
                StatusCode::SERVICE_UNAVAILABLE,
                r#"{"status":"unavailable"}"#.into()
            )
        );

        health.mark_started();
        assert_eq!(
            get(app.clone(), "/health/startup").await,
            (StatusCode::OK, r#"{"status":"ok"}"#.into())
        );
        assert_eq!(
            get(app.clone(), "/health/ready").await,
            (StatusCode::OK, r#"{"status":"ok"}"#.into())
        );

        health.begin_shutdown();
        assert_eq!(
            get(app.clone(), "/health/ready").await,
            (
                StatusCode::SERVICE_UNAVAILABLE,
                r#"{"status":"unavailable"}"#.into()
            )
        );
        assert_eq!(
            get(app, "/health/startup").await,
            (StatusCode::OK, r#"{"status":"ok"}"#.into())
        );
    }

    #[tokio::test]
    async fn liveness_is_process_only_and_minimal() {
        let (status, body) = get(routes::<()>(HealthHandle::new()), "/health/live").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, r#"{"status":"ok"}"#);
    }
}
