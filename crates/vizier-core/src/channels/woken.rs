//! Turns a background report woke, rendered on the chat platform the work was started from.
//!
//! A person's own turn streams through the response sender they handed `send_request`. A woken
//! turn has no such sender: its frames are republished on the session-event broadcast instead
//! (`BackgroundJobs::broadcast_sender`). Discord and Telegram subscribe here and replay those
//! frames through the same renderer they use for a person's turn.

use std::collections::HashMap;
use std::future::Future;

use tokio::sync::broadcast::error::RecvError;

use crate::schema::{AgentId, VizierChannelId, VizierResponse, VizierResponseContent, VizierSession};
use crate::transport::{SessionFrame, VizierTransport};

/// Aborts the watcher when the channel reader that started it stops.
pub struct WokenTurnWatcher(tokio::task::JoinHandle<()>);

impl Drop for WokenTurnWatcher {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Whether a frame ends a turn, as the platform renderers read it.
fn is_terminal(frame: &VizierResponse) -> bool {
    matches!(
        frame.content,
        VizierResponseContent::Message { .. }
            | VizierResponseContent::AudioReply(..)
            | VizierResponseContent::Abort
            | VizierResponseContent::Error { .. }
    )
}

/// Watch the session-event broadcast for response frames of `agent_id`'s sessions whose
/// channel `owns`, and hand each woken turn to `render` as a stream of its frames. One
/// renderer runs per turn: it is spawned on a session's first frame and its stream closes
/// after the turn's terminal frame.
pub fn watch<O, R, F>(transport: &VizierTransport, agent_id: AgentId, owns: O, render: R) -> WokenTurnWatcher
where
    O: Fn(&VizierChannelId) -> bool + Send + 'static,
    R: Fn(VizierSession, flume::Receiver<VizierResponse>) -> F + Send + 'static,
    F: Future<Output = ()> + Send + 'static,
{
    let mut events = transport.subscribe_session_events();
    WokenTurnWatcher(tokio::spawn(async move {
        let mut turns: HashMap<VizierSession, flume::Sender<VizierResponse>> = HashMap::new();
        loop {
            let event = match events.recv().await {
                Ok(event) => event,
                Err(RecvError::Lagged(n)) => {
                    tracing::warn!("woken-turn watcher for {} lagged by {} events", agent_id, n);
                    continue;
                }
                Err(RecvError::Closed) => return,
            };
            let SessionFrame::Response(frame) = event.frame else {
                continue;
            };
            let session = event.session;
            if session.0 != agent_id || !owns(&session.1) {
                continue;
            }

            let terminal = is_terminal(&frame);
            let tx = turns.entry(session.clone()).or_insert_with(|| {
                let (tx, rx) = flume::unbounded();
                tokio::spawn(render(session.clone(), rx));
                tx
            });
            let _ = tx.send(frame);
            if terminal {
                turns.remove(&session);
            }
        }
    }))
}
