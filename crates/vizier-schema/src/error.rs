//! The one error type of the bus.

use std::fmt;

use crate::capabilities::Capability;

/// Why a bus operation failed. Hand-written `Display`/`Error` keep the crate free of derive
/// dependencies; `vizier-core` maps this into its own error through one `From`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BusError {
    /// The payload is over the limit; nothing was sent.
    PayloadTooLarge { size: usize, limit: usize },
    /// A request found nobody subscribed.
    NoResponders,
    /// A responder existed but did not answer in time.
    Timeout,
    /// Not a valid subject.
    InvalidSubject(String),
    /// Not a valid subject pattern.
    InvalidPattern(String),
    /// The backend or subscription is closed.
    Closed,
    /// The backend does not implement this capability.
    Unsupported(Capability),
    /// `acquire` on a lease someone else holds.
    LeaseHeld,
    /// `renew` by a holder that no longer holds the lease; it must stop its work.
    LeaseLost,
    /// A compare-and-swap saw a different revision.
    Conflict,
    /// A message could not be decoded.
    Decode(String),
}

impl fmt::Display for BusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PayloadTooLarge { size, limit } => {
                write!(f, "payload of {size} bytes is over the {limit} byte limit")
            }
            Self::NoResponders => write!(f, "no responders: nothing is subscribed to the subject"),
            Self::Timeout => write!(f, "timed out waiting for a reply"),
            Self::InvalidSubject(s) => write!(f, "invalid subject: {s}"),
            Self::InvalidPattern(s) => write!(f, "invalid subject pattern: {s}"),
            Self::Closed => write!(f, "the bus is closed"),
            Self::Unsupported(c) => write!(f, "the backend does not support {c}"),
            Self::LeaseHeld => write!(f, "the lease is held by another holder"),
            Self::LeaseLost => write!(f, "the lease was lost"),
            Self::Conflict => write!(f, "the key changed since it was read"),
            Self::Decode(s) => write!(f, "could not decode message: {s}"),
        }
    }
}

impl std::error::Error for BusError {}
