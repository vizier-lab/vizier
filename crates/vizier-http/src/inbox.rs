//! One person's live connection to their inbox, independent of how frames travel.
//!
//! The WebSocket handler (`GET /api/v1/inbox/stream`) and the desktop app's IPC both drive an
//! [`InboxConnection`]: it pushes notifications, read state and setup progress to `out` as JSON
//! frames (`specs/051-realtime-notifications/contracts/inbox-stream.md`) and never reads
//! anything from the client. What a person may be told is decided in
//! `vizier_core::service::inbox`, per delivery, so this file holds no authorization.

use tokio::sync::broadcast::{self, error::{RecvError, TryRecvError}};
use vizier_core::{
    auth::{AuthMethod, Principal},
    service::{
        ServiceError, Services,
        inbox::{self, Delivery, InboxFrame},
    },
    transport::InboxEvent,
};

use crate::state::HTTPState;

/// How many queued events one delivery takes at most, so a flood still yields frames steadily.
const BATCH: usize = 64;

pub struct InboxConnection {
    services: Services,
    user_id: String,
    method: AuthMethod,
    events: broadcast::Receiver<InboxEvent>,
    out: flume::Sender<serde_json::Value>,
}

fn json(frame: &InboxFrame) -> Option<serde_json::Value> {
    match serde_json::to_value(frame) {
        Ok(value) => Some(value),
        Err(e) => {
            tracing::error!("could not encode an inbox frame: {e}");
            None
        }
    }
}

impl InboxConnection {
    /// Subscribes **first**, then reads the snapshot and sends it as `hello`, so an event that
    /// happens in between is delivered rather than lost.
    pub async fn open(
        state: &HTTPState,
        principal: &Principal,
        out: flume::Sender<serde_json::Value>,
    ) -> Result<Self, ServiceError> {
        let services = Services::new(state.deps.clone());
        let events = state.transport.subscribe_inbox_events();
        let hello = inbox::hello(&services, principal).await?;
        if let Some(value) = json(&hello) {
            // The receiver is already gone: there is nobody to serve.
            out.send(value)
                .map_err(|_| ServiceError::Unavailable("the connection closed".into()))?;
        }
        Ok(Self {
            services,
            user_id: principal.user_id.clone(),
            method: principal.method.clone(),
            events,
            out,
        })
    }

    /// Pushes frames until the client goes away or the person no longer resolves. Slow
    /// delivery never blocks the work that caused an event: events wait in the broadcast, and a
    /// connection that falls too far behind is sent a fresh `hello`.
    pub async fn run(mut self) {
        loop {
            let mut batch = Vec::new();
            let mut lagged = false;
            match self.events.recv().await {
                Ok(event) => batch.push(event),
                Err(RecvError::Lagged(skipped)) => {
                    tracing::warn!(skipped, "an inbox stream fell behind");
                    lagged = true;
                }
                Err(RecvError::Closed) => return,
            }
            // Everything already queued is one delivery.
            while batch.len() < BATCH {
                match self.events.try_recv() {
                    Ok(event) => batch.push(event),
                    Err(TryRecvError::Lagged(_)) => {
                        lagged = true;
                        break;
                    }
                    Err(TryRecvError::Empty | TryRecvError::Closed) => break,
                }
            }

            if !batch.is_empty() {
                match inbox::deliver(&self.services, &self.user_id, self.method.clone(), &batch).await {
                    Delivery::Close => return,
                    Delivery::Frames(frames) => {
                        for frame in &frames {
                            let Some(value) = json(frame) else { continue };
                            if self.out.send_async(value).await.is_err() {
                                return;
                            }
                        }
                    }
                }
            }

            // Events were lost, so the client's picture may be wrong: start it over.
            if lagged {
                match inbox::hello_for(&self.services, &self.user_id, self.method.clone()).await {
                    Ok(hello) => {
                        if let Some(value) = json(&hello)
                            && self.out.send_async(value).await.is_err()
                        {
                            return;
                        }
                    }
                    Err(ServiceError::Unauthorized(_)) => return,
                    Err(e) => tracing::warn!("could not rebuild an inbox snapshot after a lag: {e}"),
                }
            }
        }
    }
}
