//! The `control` role's bus contract. Empty until #166 fills it in.

use super::RoleContract;

pub const CONTRACT: RoleContract = RoleContract {
    role: "control",
    subscribes: &[],
    publishes: &[],
    leases: &[],
};
