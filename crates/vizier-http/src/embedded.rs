//! The desktop app's way into the API: requests dispatched in-process, with no socket and no
//! credentials. Every request is the one person the app signed in as (`AuthMode::Desktop`).
//!
//! This is deliberately the only way to get that mode: nothing here hands out the router or
//! the state, so a network listener can never be built on it.
//!
//! ```compile_fail
//! # fn f(d: vizier_http::embedded::Dispatcher) { let _r: axum::Router = d.router; }
//! ```

use axum::{Router, body::Body};
use tower::ServiceExt;
use vizier_core::runtime::VizierRuntime;

use crate::{api, state::AuthMode, topic::TopicConnection};

pub struct Dispatcher {
    router: Router,
    state: crate::state::HTTPState,
}

impl Dispatcher {
    pub fn new(runtime: &VizierRuntime, user_id: String) -> Self {
        let state = crate::new_state(runtime, AuthMode::Desktop { user_id });
        let router = Router::new()
            .nest("/api", api::api(state.clone()))
            .layer(axum::extract::DefaultBodyLimit::disable())
            .with_state(state.clone());
        Self { router, state }
    }

    pub async fn dispatch(&self, req: axum::http::Request<Body>) -> axum::http::Response<Body> {
        match self.router.clone().oneshot(req).await {
            Ok(res) => res,
            Err(never) => match never {},
        }
    }

    /// Opens a conversation connection as the desktop person.
    pub async fn open_topic(
        &self,
        agent_id: &str,
        channel_id: &str,
        topic_id: &str,
        out: flume::Sender<serde_json::Value>,
    ) -> Result<TopicConnection, vizier_core::service::ServiceError> {
        let AuthMode::Desktop { user_id } = &self.state.auth else {
            unreachable!("a dispatcher is always desktop mode")
        };
        let principal = vizier_core::service::Services::new(self.state.deps.clone())
            .resolve(user_id, vizier_core::auth::AuthMethod::Desktop)
            .await?;
        TopicConnection::open(&self.state, &principal, agent_id, channel_id, topic_id, out).await
    }
}
