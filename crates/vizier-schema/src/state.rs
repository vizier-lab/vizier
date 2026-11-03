//! Shared key/value state with compare-and-swap.
//!
//! Rules every backend must keep (contracts/bus-semantics.md, rules 13-14):
//!
//! 13. `cas(key, expected, value)` succeeds only if the key's current revision equals
//!     `expected` (`None` meaning the key must not exist), else `Conflict`; revisions increase
//!     on every write.
//! 14. `watch(prefix)` yields put and delete events for keys under the prefix.

use async_trait::async_trait;

use crate::error::BusError;
use crate::watch::Watch;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Revision(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Versioned {
    pub revision: Revision,
    pub value: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateEvent {
    Put { key: String, value: Versioned },
    Delete { key: String },
}

pub type StateWatch = Watch<StateEvent>;

#[async_trait]
pub trait SharedState: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<Versioned>, BusError>;

    async fn put(&self, key: &str, value: Vec<u8>) -> Result<Revision, BusError>;

    async fn cas(
        &self,
        key: &str,
        expected: Option<Revision>,
        value: Vec<u8>,
    ) -> Result<Revision, BusError>;

    async fn delete(&self, key: &str) -> Result<(), BusError>;

    /// Events for every key that starts with `prefix`, from now on.
    async fn watch(&self, prefix: &str) -> Result<StateWatch, BusError>;
}
