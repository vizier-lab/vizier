//! The `gateway-grpc` role's bus contract. Empty until #170 fills it in.

use super::RoleContract;

pub const CONTRACT: RoleContract = RoleContract {
    role: "gateway-grpc",
    subscribes: &[],
    publishes: &[],
    leases: &[],
};
