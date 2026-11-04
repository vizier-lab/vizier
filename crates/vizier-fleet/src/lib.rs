//! The Fleet roles (vizier-lab/vizier#159): the *same* engine, run as a cluster.
//!
//! Each role is one piece of the server that can live on its own node, built from the bus
//! traits, the storage traits and its own config ([`ServerRole`] says which). A role never
//! imports another role's code and never assumes another role is running, so each one is
//! testable alone with an in-process bus and a scripted peer.
//!
//! **This crate is a skeleton today** (#161). [`pending`] names the sub-issue that wires each
//! role in, and it is what `vizier run --role <role>` refuses with: only `all`, one process
//! doing everything, works until those land. The bus traits live in `vizier-schema` (#162),
//! and are the first parameter of every constructor below. [`testing`] builds a role alone.

use vizier_core::server_role::ServerRole;

mod channels;
mod control;
mod gateway_grpc;
mod gateway_http;
mod worker;

pub mod testing;

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

/// What `role` needs from the bus. `vizier run` checks it against the backend before serving, so
/// a backend that lacks a capability is refused with the missing one named.
pub fn needs(role: ServerRole) -> vizier_schema::Capabilities {
    match role {
        ServerRole::GatewayHttp => GatewayHttp::needs(),
        ServerRole::GatewayGrpc => GatewayGrpc::needs(),
        ServerRole::Worker => Worker::needs(),
        ServerRole::Channels => Channels::needs(),
        ServerRole::Control => Control::needs(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_role_declares_what_it_needs_and_the_in_process_bus_meets_it() {
        let bus = vizier_schema::InProcess::new().bus();
        for role in ServerRole::ALL {
            assert!(bus.require(&needs(role)).is_ok(), "{role:?}");
        }
    }

    #[test]
    fn every_role_names_the_issue_that_wires_it() {
        for role in ServerRole::ALL {
            let issue = pending(role).expect("a role without an owner");
            assert!(issue.starts_with("https://github.com/vizier-lab/vizier/issues/"), "{issue}");
        }
    }
}
