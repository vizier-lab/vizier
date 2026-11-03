//! The `gateway-http` role's bus contract. Empty until #169 fills it in.

use super::RoleContract;

pub const CONTRACT: RoleContract = RoleContract {
    role: "gateway-http",
    subscribes: &[],
    publishes: &[],
    leases: &[],
};
