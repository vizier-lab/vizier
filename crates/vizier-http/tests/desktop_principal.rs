//! SC-010: the desktop identity exists only behind the in-process dispatcher.

use axum::{body::Body, http::Request};
use tower::ServiceExt;
use vizier_core::{
    config::VizierConfig,
    runtime::{RuntimeOptions, VizierRuntime},
    storage::user::UserStorage,
};
use vizier_http::embedded::Dispatcher;

async fn get(router: axum::Router, uri: &str, auth: Option<&str>) -> (u16, serde_json::Value) {
    let mut req = Request::builder().uri(uri);
    if let Some(a) = auth {
        req = req.header("authorization", a);
    }
    let res = router.oneshot(req.body(Body::empty()).unwrap()).await.unwrap();
    let status = res.status().as_u16();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or_default())
}

#[tokio::test]
async fn network_router_never_accepts_a_desktop_identity() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = VizierConfig::with_workspace(dir.path().to_path_buf());
    config.channels.http.as_mut().unwrap().jwt_secret = "test-secret".into();
    let runtime = VizierRuntime::start(RuntimeOptions { config }).await.unwrap();
    let storage = runtime.deps().storage.clone();

    let role = storage.create_role("owner", vec![], true).await.unwrap();
    let user = storage.create_user("owner", "", &role.role_id).await.unwrap();
    let http_cfg = runtime.deps().config.channels.http.clone().unwrap();

    let network = || vizier_http::router(&runtime, &http_cfg);
    let me = "/api/v1/auth/users/me";

    for (uri, auth) in [
        (me.to_string(), None),
        (me.to_string(), Some(format!("Desktop {}", user.user_id))),
        (me.to_string(), Some(format!("Bearer {}", user.user_id))),
        (format!("{me}?token={}", user.user_id), None),
        (me.to_string(), Some("ApiKey nope".to_string())),
    ] {
        let (status, _) = get(network(), &uri, auth.as_deref()).await;
        assert_eq!(status, 401, "{uri} {auth:?}");
    }

    let dispatcher = Dispatcher::new(&runtime, user.user_id.clone());
    let res = dispatcher
        .dispatch(Request::builder().uri(me).body(Body::empty()).unwrap())
        .await;
    assert_eq!(res.status().as_u16(), 200);
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["data"]["username"], "owner");
    assert_eq!(body["data"]["has_password"], false);
}

/// T099: with an explicit origin list the CORS layer refuses other origins.
#[tokio::test]
async fn cors_allow_list_refuses_other_origins() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = VizierConfig::with_workspace(dir.path().to_path_buf());
    let http = config.channels.http.as_mut().unwrap();
    http.jwt_secret = "test-secret".into();
    http.cors_origins = Some(vec!["http://127.0.0.1:9999".into()]);
    let runtime = VizierRuntime::start(RuntimeOptions { config }).await.unwrap();
    let http_cfg = runtime.deps().config.channels.http.clone().unwrap();

    let preflight = |origin: &'static str| {
        let router = vizier_http::router(&runtime, &http_cfg);
        async move {
            let res = router
                .oneshot(
                    Request::builder()
                        .method("OPTIONS")
                        .uri("/api/v1/status")
                        .header("origin", origin)
                        .header("access-control-request-method", "GET")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            res.headers().get("access-control-allow-origin").map(|v| v.to_str().unwrap().to_string())
        }
    };
    assert_eq!(preflight("http://127.0.0.1:9999").await.as_deref(), Some("http://127.0.0.1:9999"));
    assert_eq!(preflight("http://evil.example").await, None);
}
