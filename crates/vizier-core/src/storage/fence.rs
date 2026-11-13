//! Row ownership and fencing (`specs/057-fleet-row-ownership/contracts/ownership.md`).
//!
//! Each **scope** (`agent:{id}` or `control`) has one `owner_fence` row whose `token` the
//! database raises on every [`FenceStorage::claim`]. A process remembers the fence it obtained in
//! its [`Fences`] registry, and every write that advances in-flight work of a scope checks, in its
//! own transaction, that the stored token is still that one. A former owner's writes are refused
//! with [`OwnershipLost`] once a newer owner has claimed the scope.

use std::collections::HashMap;
use std::fmt;

use anyhow::Result;
use parking_lot::Mutex;
use tokio::sync::watch;
use vizier_schema::FencingToken;

use crate::schema::AgentId;

/// What one owner owns: one agent's in-flight work, or the control plane's (task runs).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Scope {
    Agent(AgentId),
    Control,
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Scope::Agent(id) => write!(f, "agent:{id}"),
            Scope::Control => f.write_str("control"),
        }
    }
}

/// What a successful claim returns: the scope, the claiming node, and the fence token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fence {
    pub scope: Scope,
    pub node: String,
    pub token: u64,
}

/// A fenced write refused because a newer owner has claimed the scope, or a claim refused
/// because a newer lease already claimed it. Travels inside `anyhow::Error`; recognise it with
/// [`is_ownership_lost`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnershipLost {
    pub scope: String,
}

impl fmt::Display for OwnershipLost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ownership of {} moved to a newer owner", self.scope)
    }
}

impl std::error::Error for OwnershipLost {}

/// True when `err` is (or wraps) an [`OwnershipLost`].
pub fn is_ownership_lost(err: &anyhow::Error) -> bool {
    err.chain()
        .any(|cause| cause.downcast_ref::<OwnershipLost>().is_some())
}

/// The fences this process holds, and the lost signal of each scope (data-model.md,
/// "Process-local state"). Entries are never removed: a scope this process stopped owning keeps
/// its old token, which is exactly what makes its late writes fail (research R3).
#[derive(Default)]
pub struct Fences {
    held: Mutex<HashMap<String, Fence>>,
    lost: Mutex<HashMap<String, watch::Sender<bool>>>,
}

impl Fences {
    /// Remember the fence of a successful claim; it replaces any earlier one of the scope.
    pub fn hold(&self, fence: Fence) {
        let key = fence.scope.to_string();
        self.held.lock().insert(key.clone(), fence);
        // A new claim is a fresh ownership: whoever watches from now on starts at `false`.
        let mut lost = self.lost.lock();
        match lost.get(&key) {
            Some(sender) if *sender.borrow() => {
                lost.insert(key, watch::channel(false).0);
            }
            _ => {}
        }
    }

    /// The token this process holds for `scope`, or `None` when it never claimed it.
    pub fn held(&self, scope: &Scope) -> Option<u64> {
        self.held
            .lock()
            .get(&scope.to_string())
            .map(|fence| fence.token)
    }

    /// A fenced write of `scope` was refused: flip its signal (and log it) the first time.
    pub fn mark_lost(&self, scope: &Scope) {
        let key = scope.to_string();
        let mut lost = self.lost.lock();
        let sender = lost
            .entry(key.clone())
            .or_insert_with(|| watch::channel(false).0);
        if !*sender.borrow() {
            tracing::warn!(scope = %key, "ownership of {key} moved to a newer owner; stopping its work here");
            sender.send_replace(true);
        }
    }

    /// The lost signal of `scope`: `true` once a fenced write of it has been refused.
    pub fn lost(&self, scope: &Scope) -> watch::Receiver<bool> {
        self.lost
            .lock()
            .entry(scope.to_string())
            .or_insert_with(|| watch::channel(false).0)
            .subscribe()
    }
}

#[async_trait::async_trait]
pub trait FenceStorage {
    /// Take ownership of `scope` for `node`. `lease` is the bus fencing token of the lease just
    /// acquired, or `None` in single node. On success every later fenced write of `scope` from
    /// this process is checked against the returned fence. Fails with [`OwnershipLost`] when a
    /// claim under a lease token at least as new already succeeded.
    async fn claim(&self, scope: &Scope, node: &str, lease: Option<FencingToken>) -> Result<Fence>;

    /// Fires `true` the first time a fenced write of `scope` from this process is refused.
    fn ownership_lost(&self, scope: &Scope) -> watch::Receiver<bool>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_display() {
        assert_eq!(Scope::Agent("dp".into()).to_string(), "agent:dp");
        assert_eq!(Scope::Control.to_string(), "control");
    }

    #[test]
    fn ownership_lost_is_recognised_through_context() {
        let err = anyhow::Error::new(OwnershipLost {
            scope: "agent:dp".into(),
        })
        .context("saving history");
        assert!(is_ownership_lost(&err));
        assert!(!is_ownership_lost(&anyhow::anyhow!("other")));
        assert_eq!(
            OwnershipLost {
                scope: "control".into()
            }
            .to_string(),
            "ownership of control moved to a newer owner"
        );
    }

    #[test]
    fn lost_signal_fires_once_and_resets_on_a_new_claim() {
        let fences = Fences::default();
        let scope = Scope::Agent("a".into());
        assert_eq!(fences.held(&scope), None);
        fences.hold(Fence {
            scope: scope.clone(),
            node: "n".into(),
            token: 3,
        });
        assert_eq!(fences.held(&scope), Some(3));
        let rx = fences.lost(&scope);
        assert!(!*rx.borrow());
        fences.mark_lost(&scope);
        fences.mark_lost(&scope);
        assert!(*rx.borrow());
        fences.hold(Fence {
            scope: scope.clone(),
            node: "n".into(),
            token: 5,
        });
        assert!(!*fences.lost(&scope).borrow());
    }
}
