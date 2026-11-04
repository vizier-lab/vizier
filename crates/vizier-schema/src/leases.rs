//! Leases with fencing tokens.
//!
//! Rules every backend must keep (contracts/bus-semantics.md, rules 8-12):
//!
//! 8. At most one holder per name; `acquire` on a held lease returns `LeaseHeld`.
//! 9. A lease not renewed within its TTL expires and can be acquired by another holder.
//! 10. Every acquisition of a name yields a strictly greater [`FencingToken`] than any earlier
//!     one for that name, including across expiry takeovers.
//! 11. If `renew` returns an error the holder has lost the lease and MUST stop the work it
//!     protected immediately.
//! 12. `watch` yields acquire, expire and release events for a name.

use std::time::Duration;

use async_trait::async_trait;
use tokio::time::Instant;

use crate::error::BusError;
use crate::watch::Watch;

/// Strictly increasing per lease name, across all holders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FencingToken(pub u64);

/// A held lease.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lease {
    pub name: String,
    pub holder: String,
    pub token: FencingToken,
    pub ttl: Duration,
    pub expires_at: Instant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeaseEvent {
    Acquired { holder: String, token: FencingToken },
    Expired,
    Released,
}

pub type LeaseWatch = Watch<LeaseEvent>;

#[async_trait]
pub trait Leases: Send + Sync {
    /// Take the lease `name` for `ttl`, as a fresh holder.
    async fn acquire(&self, name: &str, ttl: Duration) -> Result<Lease, BusError>;

    /// Extend a held lease by its TTL. `LeaseLost` means stop (rule 11).
    async fn renew(&self, lease: &Lease) -> Result<Lease, BusError>;

    /// Give the lease up. Releasing a lease already lost is not an error.
    async fn release(&self, lease: Lease) -> Result<(), BusError>;

    /// Events for `name` from now on.
    async fn watch(&self, name: &str) -> Result<LeaseWatch, BusError>;
}
