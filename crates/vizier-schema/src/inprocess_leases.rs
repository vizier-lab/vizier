//! The in-process lease table: exclusive holders, TTL expiry, fencing tokens and watchers.

use std::collections::HashMap;
use std::sync::{Arc, Weak};
use std::time::Duration;

use async_trait::async_trait;
use tokio::time::Instant;

use crate::error::BusError;
use crate::inprocess::{InProcess, Inner};
use crate::leases::{FencingToken, Lease, LeaseEvent, LeaseWatch, Leases};
use crate::watch::{Watch, WatchSender};

struct Held {
    holder: String,
    token: u64,
    expires_at: Instant,
}

#[derive(Default)]
struct Slot {
    /// The last token issued for this name; the next acquisition gets one more.
    last_token: u64,
    held: Option<Held>,
    watchers: Vec<WatchSender<LeaseEvent>>,
}

impl Slot {
    fn emit(&mut self, event: LeaseEvent) {
        self.watchers.retain(|w| w.send(event.clone()));
    }

    /// Free the lease if its TTL has run out. Safe to call from anywhere, any number of times.
    fn expire_if_due(&mut self, now: Instant) {
        if self.held.as_ref().is_some_and(|h| now >= h.expires_at) {
            self.held = None;
            self.emit(LeaseEvent::Expired);
        }
    }
}

#[derive(Default)]
pub(crate) struct LeaseTable {
    slots: HashMap<String, Slot>,
}

fn table(inner: &Inner) -> std::sync::MutexGuard<'_, LeaseTable> {
    inner.leases.lock().unwrap_or_else(|e| e.into_inner())
}

/// Expires the grant `token` of `name` at its deadline, so a watcher hears `Expired` even if
/// nobody asks for the lease again.
fn spawn_expiry(inner: Weak<Inner>, name: String, token: u64) {
    tokio::spawn(async move {
        loop {
            let deadline = {
                let Some(inner) = inner.upgrade() else { return };
                let t = table(&inner);
                match t.slots.get(&name).and_then(|s| s.held.as_ref()).filter(|h| h.token == token) {
                    Some(held) => held.expires_at,
                    None => return,
                }
            };
            tokio::time::sleep_until(deadline).await;
            let Some(inner) = inner.upgrade() else { return };
            let mut t = table(&inner);
            let Some(slot) = t.slots.get_mut(&name) else { return };
            slot.expire_if_due(Instant::now());
            // Renewed in the meantime: go round again with the new deadline.
            if !slot.held.as_ref().is_some_and(|h| h.token == token) {
                return;
            }
        }
    });
}

#[async_trait]
impl Leases for InProcess {
    async fn acquire(&self, name: &str, ttl: Duration) -> Result<Lease, BusError> {
        let now = Instant::now();
        let lease = {
            let mut t = table(&self.inner);
            let slot = t.slots.entry(name.to_string()).or_default();
            slot.expire_if_due(now);
            if slot.held.is_some() {
                return Err(BusError::LeaseHeld);
            }
            slot.last_token += 1;
            let token = slot.last_token;
            let holder = uuid::Uuid::new_v4().simple().to_string();
            let expires_at = now + ttl;
            slot.held = Some(Held { holder: holder.clone(), token, expires_at });
            slot.emit(LeaseEvent::Acquired { holder: holder.clone(), token: FencingToken(token) });
            Lease { name: name.to_string(), holder, token: FencingToken(token), ttl, expires_at }
        };
        spawn_expiry(Arc::downgrade(&self.inner), name.to_string(), lease.token.0);
        Ok(lease)
    }

    async fn renew(&self, lease: &Lease) -> Result<Lease, BusError> {
        let now = Instant::now();
        let mut t = table(&self.inner);
        let slot = t.slots.get_mut(&lease.name).ok_or(BusError::LeaseLost)?;
        slot.expire_if_due(now);
        match slot.held.as_mut() {
            Some(held) if held.token == lease.token.0 && held.holder == lease.holder => {
                held.expires_at = now + lease.ttl;
                Ok(Lease { expires_at: held.expires_at, ..lease.clone() })
            }
            _ => Err(BusError::LeaseLost),
        }
    }

    async fn release(&self, lease: Lease) -> Result<(), BusError> {
        let mut t = table(&self.inner);
        if let Some(slot) = t.slots.get_mut(&lease.name) {
            slot.expire_if_due(Instant::now());
            if slot.held.as_ref().is_some_and(|h| h.token == lease.token.0 && h.holder == lease.holder) {
                slot.held = None;
                slot.emit(LeaseEvent::Released);
            }
        }
        Ok(())
    }

    async fn watch(&self, name: &str) -> Result<LeaseWatch, BusError> {
        let (tx, rx) = Watch::channel();
        table(&self.inner).slots.entry(name.to_string()).or_default().watchers.push(tx);
        Ok(rx)
    }
}
