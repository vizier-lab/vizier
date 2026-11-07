//! Telling watchers that a project's servers changed: one `InboxEvent::ProjectLsp` per change,
//! with progress coalesced to one per 250 ms per project (`contracts/http-api.md`).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use super::LspHost;
use crate::{
    schema::LspStatusSnapshot,
    transport::{InboxEvent, VizierTransport},
};

const MIN_GAP: Duration = Duration::from_millis(250);

/// What [`LspHost::set_listener`](super::LspHost::set_listener) takes.
pub type Listener = Arc<dyn Fn(&LspHost, &str, &str) + Send + Sync>;

/// The status of a project's servers as the strip shows it. No override values.
pub fn snapshot(host: &LspHost, project_id: &str) -> LspStatusSnapshot {
    let enabled = !host.is_closed(project_id);
    LspStatusSnapshot {
        project_id: project_id.to_owned(),
        enabled,
        capacity: host.capacity(),
        servers: match enabled {
            true => host.status(project_id),
            false => vec![],
        },
    }
}

#[derive(Default)]
struct Slot {
    last: Option<Instant>,
    scheduled: bool,
}

/// The host listener that publishes on the transport. It must be called inside a Tokio runtime.
pub fn listener(transport: VizierTransport) -> Listener {
    let slots: Arc<Mutex<HashMap<String, Slot>>> = Arc::default();
    Arc::new(move |host: &LspHost, agent_id: &str, project_id: &str| {
        let wait = {
            let mut slots = slots.lock();
            let slot = slots.entry(project_id.to_owned()).or_default();
            if slot.scheduled {
                return;
            }
            let wait = slot
                .last
                .map(|t| MIN_GAP.saturating_sub(t.elapsed()))
                .unwrap_or_default();
            if wait.is_zero() {
                slot.last = Some(Instant::now());
            } else {
                slot.scheduled = true;
            }
            wait
        };
        let (host, transport, slots) = (host.clone(), transport.clone(), slots.clone());
        let (agent_id, project_id) = (agent_id.to_owned(), project_id.to_owned());
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        runtime.spawn(async move {
            if !wait.is_zero() {
                tokio::time::sleep(wait).await;
                let mut slots = slots.lock();
                if let Some(slot) = slots.get_mut(&project_id) {
                    slot.scheduled = false;
                    slot.last = Some(Instant::now());
                }
            }
            transport.publish_inbox_event(InboxEvent::ProjectLsp {
                agent_id,
                snapshot: snapshot(&host, &project_id),
            });
        });
    })
}
