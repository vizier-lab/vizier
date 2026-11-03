//! The Fleet bus contract.
//!
//! **Dependency rule:** this crate depends on no other `vizier-*` crate and on no transport,
//! web or CLI crate. The engine (`vizier-core`) and the roles (`vizier-fleet`) depend on it, never
//! the other way round.
//!
//! The contract is three traits ([`Messaging`], [`Leases`], [`SharedState`]) bundled in a
//! [`Bus`] that also says which of them the backend implements.

mod bus;
mod capabilities;
mod envelope;
mod error;
mod leases;
mod messaging;
mod state;
mod subject;
mod watch;

pub use bus::Bus;
pub use capabilities::{Capabilities, Capability};
pub use envelope::{Envelope, Headers, MAX_PAYLOAD, WIRE_VERSION};
pub use error::BusError;
pub use leases::{FencingToken, Lease, LeaseEvent, LeaseWatch, Leases};
pub use messaging::{Delivery, Messaging, SUBSCRIPTION_DEPTH, Subscription, SubscriptionSender, reply};
pub use state::{Revision, SharedState, StateEvent, StateWatch, Versioned};
pub use subject::{
    AGENTS_CMD, DREAM_CMD, FILES_CMD, INTEGRATION_CHANGED, MAX_SUBJECT_LEN, PROCESS_CMD, Subject,
    SubjectPattern,
};
pub use watch::{Watch, WatchSender};
