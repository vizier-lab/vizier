//! The `worker` role's bus contract. Empty until #168 fills it in.

use super::RoleContract;

pub const CONTRACT: RoleContract = RoleContract {
    role: "worker",
    subscribes: &[],
    publishes: &[],
    leases: &[],
};
