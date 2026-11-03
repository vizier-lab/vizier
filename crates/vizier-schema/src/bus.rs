//! The three halves of the contract, bundled.

use std::sync::Arc;

use crate::capabilities::{Capabilities, Capability};
use crate::error::BusError;
use crate::leases::Leases;
use crate::messaging::Messaging;
use crate::state::SharedState;

/// What a role or the engine is handed: messaging, leases and shared state, plus what the
/// backend behind them implements.
#[derive(Clone)]
pub struct Bus {
    pub messaging: Arc<dyn Messaging>,
    pub leases: Arc<dyn Leases>,
    pub state: Arc<dyn SharedState>,
    pub capabilities: Capabilities,
}

impl Bus {
    pub fn new(
        messaging: Arc<dyn Messaging>,
        leases: Arc<dyn Leases>,
        state: Arc<dyn SharedState>,
        capabilities: Capabilities,
    ) -> Self {
        Self { messaging, leases, state, capabilities }
    }

    /// Refuse startup when the backend lacks something `needed` asks for, naming the first
    /// missing capability (rule 15).
    pub fn require(&self, needed: &Capabilities) -> Result<(), BusError> {
        match self.capabilities.missing(needed).first() {
            Some(missing) => Err(BusError::Unsupported(*missing)),
            None => Ok(()),
        }
    }

    pub fn has(&self, capability: Capability) -> bool {
        self.capabilities.has(capability)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use async_trait::async_trait;

    use super::*;
    use crate::envelope::Envelope;
    use crate::leases::{Lease, LeaseWatch};
    use crate::messaging::Subscription;
    use crate::state::{Revision, StateWatch, Versioned};
    use crate::subject::{Subject, SubjectPattern};

    /// A backend that implements nothing; only its advertised capabilities matter here.
    struct Stub;

    #[async_trait]
    impl Messaging for Stub {
        async fn publish(&self, _: &Subject, _: Envelope) -> Result<(), BusError> {
            Err(BusError::Closed)
        }
        async fn request(&self, _: &Subject, _: Envelope, _: Duration) -> Result<Envelope, BusError> {
            Err(BusError::Closed)
        }
        async fn subscribe(&self, _: SubjectPattern) -> Result<Subscription, BusError> {
            Err(BusError::Closed)
        }
    }

    #[async_trait]
    impl Leases for Stub {
        async fn acquire(&self, _: &str, _: Duration) -> Result<Lease, BusError> {
            Err(BusError::Unsupported(Capability::Leases))
        }
        async fn renew(&self, _: &Lease) -> Result<Lease, BusError> {
            Err(BusError::Unsupported(Capability::Leases))
        }
        async fn release(&self, _: Lease) -> Result<(), BusError> {
            Err(BusError::Unsupported(Capability::Leases))
        }
        async fn watch(&self, _: &str) -> Result<LeaseWatch, BusError> {
            Err(BusError::Unsupported(Capability::Leases))
        }
    }

    #[async_trait]
    impl SharedState for Stub {
        async fn get(&self, _: &str) -> Result<Option<Versioned>, BusError> {
            Err(BusError::Closed)
        }
        async fn put(&self, _: &str, _: Vec<u8>) -> Result<Revision, BusError> {
            Err(BusError::Closed)
        }
        async fn cas(&self, _: &str, _: Option<Revision>, _: Vec<u8>) -> Result<Revision, BusError> {
            Err(BusError::Closed)
        }
        async fn delete(&self, _: &str) -> Result<(), BusError> {
            Err(BusError::Closed)
        }
        async fn watch(&self, _: &str) -> Result<StateWatch, BusError> {
            Err(BusError::Closed)
        }
    }

    #[test]
    fn require_names_the_missing_capability() {
        let stub = Arc::new(Stub);
        let bus = Bus::new(
            stub.clone(),
            stub.clone(),
            stub,
            Capabilities { messaging: true, leases: false, shared_state: true },
        );
        assert!(bus.require(&Capabilities { messaging: true, ..Capabilities::NONE }).is_ok());
        let err = bus.require(&Capabilities::all()).unwrap_err();
        assert_eq!(err, BusError::Unsupported(Capability::Leases));
        assert!(err.to_string().contains("leases"));
    }
}
