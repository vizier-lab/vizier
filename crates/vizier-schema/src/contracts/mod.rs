//! What each Fleet role subscribes to, publishes and leases.
//!
//! The gateway's and the worker's rows are filled in (`specs/163-agent-service-seam`); the
//! channels', the control's and the gRPC gateway's are still empty. The sub-issues that build
//! those roles (#166, #167, #170) fill them in, and [`RoleContract::validate`] already checks
//! whatever they add, so a bad pattern or a duplicate fails a test rather than a deployment.

use std::collections::HashSet;

use crate::subject::SubjectPattern;

pub mod channels;
pub mod control;
pub mod gateway_grpc;
pub mod gateway_http;
pub mod worker;

/// One role's bus contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoleContract {
    pub role: &'static str,
    /// Subject patterns the role subscribes to.
    pub subscribes: &'static [&'static str],
    /// Subjects (or subject shapes, with `*` for an identifier) the role publishes to.
    pub publishes: &'static [&'static str],
    /// Lease names the role takes.
    pub leases: &'static [&'static str],
}

impl RoleContract {
    /// Every pattern parses, no list repeats an entry, and every lease name is non-empty.
    pub fn validate(&self) -> Result<(), String> {
        let role = self.role;
        for (what, list) in [("subscribes", self.subscribes), ("publishes", self.publishes)] {
            for entry in list {
                SubjectPattern::parse(*entry).map_err(|e| format!("{role} {what}: {e}"))?;
            }
            no_duplicates(role, what, list)?;
        }
        for lease in self.leases {
            if lease.trim().is_empty() {
                return Err(format!("{role} leases: an empty lease name"));
            }
        }
        no_duplicates(role, "leases", self.leases)
    }

    /// Every role, in a fixed order.
    pub const ALL: [RoleContract; 5] = [
        gateway_http::CONTRACT,
        gateway_grpc::CONTRACT,
        worker::CONTRACT,
        channels::CONTRACT,
        control::CONTRACT,
    ];
}

fn no_duplicates(role: &str, what: &str, list: &[&str]) -> Result<(), String> {
    let mut seen = HashSet::new();
    match list.iter().find(|entry| !seen.insert(**entry)) {
        Some(dup) => Err(format!("{role} {what}: {dup:?} is listed twice")),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_role_contract_validates() {
        for contract in RoleContract::ALL {
            contract.validate().unwrap_or_else(|e| panic!("{e}"));
        }
    }

    #[test]
    fn every_role_has_a_distinct_name() {
        let names: HashSet<_> = RoleContract::ALL.iter().map(|c| c.role).collect();
        assert_eq!(names.len(), RoleContract::ALL.len());
    }

    #[test]
    fn a_bad_pattern_is_rejected() {
        let bad = RoleContract { role: "x", subscribes: &["a.>.b"], publishes: &[], leases: &[] };
        assert!(bad.validate().unwrap_err().contains("subscribes"));
        let bad = RoleContract { role: "x", subscribes: &[], publishes: &["ag*nt"], leases: &[] };
        assert!(bad.validate().unwrap_err().contains("publishes"));
    }

    #[test]
    fn a_duplicate_is_rejected() {
        let bad = RoleContract { role: "x", subscribes: &["a.b", "a.b"], publishes: &[], leases: &[] };
        assert!(bad.validate().unwrap_err().contains("twice"));
        let bad = RoleContract { role: "x", subscribes: &[], publishes: &[], leases: &["leader", "leader"] };
        assert!(bad.validate().unwrap_err().contains("twice"));
    }

    #[test]
    fn an_empty_lease_name_is_rejected() {
        let bad = RoleContract { role: "x", subscribes: &[], publishes: &[], leases: &[" "] };
        assert!(bad.validate().is_err());
    }
}
