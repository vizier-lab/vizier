//! What a backend implements.

use std::fmt;

/// One of the three groups of the contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    Messaging,
    Leases,
    SharedState,
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Messaging => "messaging",
            Self::Leases => "leases",
            Self::SharedState => "shared_state",
        })
    }
}

/// The capabilities a backend advertises, or that a role needs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Capabilities {
    pub messaging: bool,
    pub leases: bool,
    pub shared_state: bool,
}

impl Capabilities {
    /// Nothing.
    pub const NONE: Self = Self { messaging: false, leases: false, shared_state: false };

    /// Everything.
    pub const fn all() -> Self {
        Self { messaging: true, leases: true, shared_state: true }
    }

    pub fn has(&self, capability: Capability) -> bool {
        match capability {
            Capability::Messaging => self.messaging,
            Capability::Leases => self.leases,
            Capability::SharedState => self.shared_state,
        }
    }

    /// The capabilities in `needed` that `self` lacks, in a fixed order.
    pub fn missing(&self, needed: &Capabilities) -> Vec<Capability> {
        [Capability::Messaging, Capability::Leases, Capability::SharedState]
            .into_iter()
            .filter(|c| needed.has(*c) && !self.has(*c))
            .collect()
    }
}
