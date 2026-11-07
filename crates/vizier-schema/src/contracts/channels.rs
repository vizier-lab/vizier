//! The `channels` role's bus contract. Empty until #167 fills it in.

use super::RoleContract;

pub const CONTRACT: RoleContract = RoleContract {
    role: "channels",
    subscribes: &[],
    publishes: &[],
    leases: &[],
};
