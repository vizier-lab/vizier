//! The Fleet roles (vizier-lab/vizier#159): the *same* engine, run as a cluster.
//!
//! Each role is one piece of the server that can live on its own node, built from the bus
//! traits, the storage traits and its own config ([`ServerRole`] says which). A role never
//! imports another role's code and never assumes another role is running, so each one is
//! testable alone with an in-process bus and a scripted peer.
//!
//! **This crate is a skeleton today** (#161). [`pending`] names the sub-issue that wires each
//! role in, and it is what `vizier run --role <role>` refuses with: only `all`, one process
//! doing everything, works until those land. The bus traits arrive with `vizier-schema` (#162),
//! and become the first parameter of every constructor below.

use vizier_core::server_role::ServerRole;

mod channels;
mod control;
mod gateway_grpc;
mod gateway_http;
mod worker;

pub use channels::Channels;
pub use control::Control;
pub use gateway_grpc::GatewayGrpc;
pub use gateway_http::GatewayHttp;
pub use worker::Worker;

/// The sub-issue that wires `role` into `vizier run`, while it is still a skeleton.
///
/// `None` once the role is real. This is the one place that knows the order of the work, so
/// the CLI's refusal message and the README cannot drift apart.
pub fn pending(role: ServerRole) -> Option<&'static str> {
    let issue = match role {
        ServerRole::GatewayHttp => "https://github.com/vizier-lab/vizier/issues/169",
        ServerRole::GatewayGrpc => "https://github.com/vizier-lab/vizier/issues/170",
        ServerRole::Worker => "https://github.com/vizier-lab/vizier/issues/168",
        ServerRole::Channels => "https://github.com/vizier-lab/vizier/issues/167",
        ServerRole::Control => "https://github.com/vizier-lab/vizier/issues/166",
    };
    Some(issue)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_role_names_the_issue_that_wires_it() {
        for role in ServerRole::ALL {
            let issue = pending(role).expect("a role without an owner");
            assert!(issue.starts_with("https://github.com/vizier-lab/vizier/issues/"), "{issue}");
        }
    }
}
