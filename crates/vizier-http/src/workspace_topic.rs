//! One person's live connection to one Workspace topic, independent of how frames travel.
//!
//! The WebSocket handler (`GET /api/v1/workspaces/{id}/topics/{tid}/stream`) and the desktop
//! app's IPC both drive a [`WorkspaceConnection`] with the same shape as a `TopicConnection`:
//! `open`, `handle_inbound`, `outbound().run()` and `close`
//! (`specs/053-workspace-channel/contracts/socket-frames.md`). It holds no authorization: what a
//! person may see is decided per delivery in `vizier_core::service::workspaces`, so a removal
//! takes effect on an open socket.

use tokio::sync::broadcast::{self, error::RecvError};
use vizier_core::{
    auth::{AuthMethod, Principal},
    service::{
        ServiceError, Services,
        workspaces::{self, Delivery, PostRequest},
    },
    transport::{SessionEvent, WorkspaceEvent},
};

use crate::state::HTTPState;

pub struct WorkspaceConnection {
    services: Services,
    user_id: String,
    method: AuthMethod,
    workspace_id: String,
    topic_id: String,
    out: flume::Sender<serde_json::Value>,
    outbound: Option<WorkspaceOutbound>,
}

/// The half that pushes frames. Take it with [`WorkspaceConnection::outbound`] and run it.
pub struct WorkspaceOutbound {
    services: Services,
    user_id: String,
    method: AuthMethod,
    workspace_id: String,
    topic_id: String,
    workspace_events: broadcast::Receiver<WorkspaceEvent>,
    session_events: broadcast::Receiver<SessionEvent>,
    out: flume::Sender<serde_json::Value>,
}

impl WorkspaceConnection {
    /// Subscribes **first**, then reads where the timeline ends and sends it as `hello`, so an
    /// event between the two is delivered rather than lost. A person with no standing gets
    /// `NotFound`, as for a Workspace that does not exist.
    pub async fn open(
        state: &HTTPState,
        principal: &Principal,
        workspace_id: &str,
        topic_id: &str,
        out: flume::Sender<serde_json::Value>,
    ) -> Result<Self, ServiceError> {
        let services = Services::new(state.deps.clone());
        let workspace_events = state.transport.subscribe_workspace_events();
        let session_events = state.transport.subscribe_session_events();
        let hello = workspaces::hello(&services, principal, workspace_id, topic_id).await?;
        out.send(hello)
            .map_err(|_| ServiceError::Unavailable("the connection closed".into()))?;
        let outbound = WorkspaceOutbound {
            services: services.clone(),
            user_id: principal.user_id.clone(),
            method: principal.method.clone(),
            workspace_id: workspace_id.to_string(),
            topic_id: topic_id.to_string(),
            workspace_events,
            session_events,
            out: out.clone(),
        };
        Ok(Self {
            services,
            user_id: principal.user_id.clone(),
            method: principal.method.clone(),
            workspace_id: workspace_id.to_string(),
            topic_id: topic_id.to_string(),
            out,
            outbound: Some(outbound),
        })
    }

    /// The pushing half, once.
    pub fn outbound(&mut self) -> Option<WorkspaceOutbound> {
        self.outbound.take()
    }

    /// A frame from the client. Only `{ "post": { "text": "...", "files": [] } }` is read, and
    /// it does what the POST route does. A failure answers a `post_error` frame.
    pub async fn handle_inbound(&self, frame: serde_json::Value) {
        let Some(post) = frame.get("post") else { return };
        let request: PostRequest = match serde_json::from_value(post.clone()) {
            Ok(request) => request,
            Err(e) => {
                self.post_error(&ServiceError::invalid(format!("not a message: {e}")));
                return;
            }
        };
        let who = match self.services.resolve(&self.user_id, self.method.clone()).await {
            Ok(who) => who,
            Err(e) => return self.post_error(&e),
        };
        // The entry (or the note a command made) reaches everyone, this client included, as a
        // `workspace_entry` frame, so a successful post sends nothing back here.
        if let Err(e) =
            workspaces::post(&self.services, &who, &self.workspace_id, &self.topic_id, request).await
        {
            self.post_error(&e);
        }
    }

    fn post_error(&self, e: &ServiceError) {
        let _ = self.out.send(serde_json::json!({
            "post_error": { "status": e.status(), "code": e.code(), "message": e.message() }
        }));
    }

    pub fn close(self) {}
}

impl WorkspaceOutbound {
    /// Pushes frames until the client goes away, the person is removed, or the Workspace is
    /// deleted. A connection that falls behind is told to re-read from its last `seq`.
    pub async fn run(mut self) {
        loop {
            let delivery = tokio::select! {
                event = self.workspace_events.recv() => match event {
                    Ok(event) => Some(workspaces::deliver_workspace(
                        &self.services, &self.user_id, self.method.clone(),
                        &self.workspace_id, &self.topic_id, &event,
                    ).await),
                    Err(RecvError::Lagged(skipped)) => {
                        tracing::warn!(skipped, "a Workspace stream fell behind");
                        self.resync().await;
                        None
                    }
                    Err(RecvError::Closed) => return,
                },
                event = self.session_events.recv() => match event {
                    Ok(event) => Some(workspaces::deliver_session(
                        &self.services, &self.user_id, self.method.clone(),
                        &self.workspace_id, &self.topic_id, &event,
                    ).await),
                    // Session events are many; a lag here only drops live trail steps, which a
                    // reload reads back from history, so say nothing.
                    Err(RecvError::Lagged(_)) => None,
                    Err(RecvError::Closed) => return,
                },
            };
            match delivery {
                Some(Delivery::Frames(frames)) => {
                    for frame in frames {
                        if self.out.send_async(frame).await.is_err() {
                            return;
                        }
                    }
                }
                Some(Delivery::Close(reason)) => {
                    let _ = self
                        .out
                        .send_async(serde_json::json!({ "closed": { "reason": reason } }))
                        .await;
                    return;
                }
                None => {}
            }
        }
    }

    async fn resync(&self) {
        let _ = self.out.send_async(serde_json::json!({ "resync": {} })).await;
    }
}
