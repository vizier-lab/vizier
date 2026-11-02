//! Which of the server's roles a process takes (Fleet, vizier-lab/vizier#161 and #159).
//!
//! A role is a piece of the server that can run on its own node: the HTTP gateway, the gRPC
//! gateway, the workers that run agents, the channel readers, and control (the scheduler and the
//! housekeeping around it). One process can take any set of them; [`ServerRoleSet::all`] is the
//! single node, and remains the default.
//!
//! The names are `ServerRole`, not `Role`, because [`crate::auth::Role`] is already a person's
//! permission role. `all` is not a variant: it is every role in one set, so a role can be added
//! without a second place to keep in step.

use std::fmt;
use std::str::FromStr;

/// One piece of the server a process can run (#159, `docs/src/configuration/cli.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum ServerRole {
    /// REST, WebSocket, WebUI, authentication and authorization.
    GatewayHttp,
    /// The same operations over gRPC, for other software.
    GatewayGrpc,
    /// Runs agents: supervisors and instances, one owner worker per agent.
    Worker,
    /// The Discord and Telegram readers, one reader per integration.
    Channels,
    /// The scheduler (cron, one-shot, dreams), retention purge and the other housekeeping.
    Control,
}

impl ServerRole {
    /// Every role, in the order the documentation lists them.
    pub const ALL: [Self; 5] = [
        Self::GatewayHttp,
        Self::GatewayGrpc,
        Self::Worker,
        Self::Channels,
        Self::Control,
    ];

    /// The name used by `--role`, `VIZIER_ROLE` and the logs.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GatewayHttp => "gateway-http",
            Self::GatewayGrpc => "gateway-grpc",
            Self::Worker => "worker",
            Self::Channels => "channels",
            Self::Control => "control",
        }
    }

    /// The Cargo feature that builds this role into the `vizier` binary (`role-*`, #160).
    pub fn feature(self) -> &'static str {
        match self {
            Self::GatewayHttp => "role-gateway-http",
            Self::GatewayGrpc => "role-gateway-grpc",
            Self::Worker => "role-worker",
            Self::Channels => "role-channels",
            Self::Control => "role-control",
        }
    }

    fn bit(self) -> u8 {
        1 << (self as u8)
    }

    /// [`ServerRole`] for `name`, case-insensitively and ignoring surrounding spaces.
    fn parse(name: &str) -> Option<Self> {
        let name = name.trim().to_ascii_lowercase();
        Self::ALL.iter().copied().find(|role| role.as_str() == name)
    }
}

impl fmt::Display for ServerRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ServerRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| format!("not a server role: {s:?} ({})", role_names()))
    }
}

/// A set of [`ServerRole`]s: what one process runs.
///
/// [`Default`] is every role, which is what a server that was not told otherwise does.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServerRoleSet(u8);

impl ServerRoleSet {
    /// Every role: the single-node default.
    pub const fn all() -> Self {
        Self(u8::MAX >> (8 - ServerRole::ALL.len() as u8))
    }

    /// No role at all. A server asked for this does nothing but hold the workspace.
    pub const fn none() -> Self {
        Self(0)
    }

    /// Exactly one role.
    pub fn only(role: ServerRole) -> Self {
        Self::none().with(role)
    }

    /// This set plus `role`.
    pub fn with(self, role: ServerRole) -> Self {
        Self(self.0 | role.bit())
    }

    pub fn insert(&mut self, role: ServerRole) {
        self.0 |= role.bit();
    }

    pub fn contains(self, role: ServerRole) -> bool {
        self.0 & role.bit() != 0
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn is_all(self) -> bool {
        self == Self::all()
    }

    /// Every role in this set, in [`ServerRole::ALL`] order.
    pub fn iter(self) -> impl Iterator<Item = ServerRole> {
        ServerRole::ALL.into_iter().filter(move |role| self.contains(*role))
    }
}

impl Default for ServerRoleSet {
    fn default() -> Self {
        Self::all()
    }
}

impl fmt::Debug for ServerRoleSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ServerRoleSet({self})")
    }
}

impl fmt::Display for ServerRoleSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_all() {
            return f.write_str("all");
        }
        if self.is_empty() {
            return f.write_str("none");
        }
        let names: Vec<&str> = self.iter().map(ServerRole::as_str).collect();
        f.write_str(&names.join(","))
    }
}

/// `all`, or a comma-separated list of role names. `all` stands alone: combining it with a role
/// it already contains hides a mistake instead of reporting it.
impl FromStr for ServerRoleSet {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(format!("a server role set cannot be empty ({})", role_names()));
        }
        if trimmed.eq_ignore_ascii_case("all") {
            return Ok(Self::all());
        }
        let mut set = Self::none();
        for part in trimmed.split(',') {
            let part = part.trim();
            if part.is_empty() {
                return Err(format!("{s:?} has an empty role ({})", role_names()));
            }
            if part.eq_ignore_ascii_case("all") {
                return Err(format!(
                    "\"all\" cannot be combined with other roles: {s:?}"
                ));
            }
            set.insert(ServerRole::parse(part).ok_or_else(|| {
                format!("not a server role: {part:?} ({})", role_names())
            })?);
        }
        Ok(set)
    }
}

fn role_names() -> String {
    let mut names: Vec<&str> = vec!["all"];
    names.extend(ServerRole::ALL.iter().map(|role| role.as_str()));
    names.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_parses_to_every_role() {
        assert_eq!("all".parse::<ServerRoleSet>(), Ok(ServerRoleSet::all()));
        assert_eq!(" ALL ".parse::<ServerRoleSet>(), Ok(ServerRoleSet::all()));
        assert!(ServerRoleSet::all().is_all());
    }

    #[test]
    fn one_role_and_a_list() {
        assert_eq!(
            "worker".parse::<ServerRoleSet>(),
            Ok(ServerRoleSet::only(ServerRole::Worker))
        );
        let set = " gateway-http, Control ".parse::<ServerRoleSet>().unwrap();
        assert!(set.contains(ServerRole::GatewayHttp));
        assert!(set.contains(ServerRole::Control));
        assert!(!set.contains(ServerRole::Worker));
        assert!(!set.is_all());
        // A repeated role is the same set, not an error.
        assert_eq!(
            "worker,worker".parse::<ServerRoleSet>(),
            Ok(ServerRoleSet::only(ServerRole::Worker))
        );
    }

    #[test]
    fn unknown_and_empty_are_refused() {
        for bad in ["", "  ", "workers", "worker,,channels", "worker,", "all,worker"] {
            assert!(bad.parse::<ServerRoleSet>().is_err(), "{bad:?} was accepted");
        }
        let error = "workers".parse::<ServerRoleSet>().unwrap_err();
        assert!(error.contains("workers") && error.contains("all"), "{error}");
    }

    #[test]
    fn display_round_trips() {
        // `Display` writes the roles in `ServerRole::ALL` order, whatever order they were
        // parsed in, so these are the canonical spellings.
        for text in ["all", "worker", "worker,control", "gateway-grpc,channels"] {
            let set: ServerRoleSet = text.parse().unwrap();
            assert_eq!(set.to_string(), text);
            assert_eq!(set.to_string().parse::<ServerRoleSet>(), Ok(set));
        }
        // Every role names itself `all`, which parses back to the same set.
        let every: ServerRoleSet = "gateway-http,gateway-grpc,worker,channels,control".parse().unwrap();
        assert!(every.is_all());
        assert_eq!(every.to_string(), "all");
        assert_eq!("all".parse::<ServerRoleSet>(), Ok(every));
        assert_eq!(ServerRoleSet::none().to_string(), "none");
    }

    #[test]
    fn every_role_is_in_all_and_names_its_feature() {
        for role in ServerRole::ALL {
            assert!(ServerRoleSet::all().contains(role), "{role} is not in all");
            assert_eq!(role.feature(), format!("role-{}", role.as_str()));
            assert_eq!(role.to_string().parse::<ServerRole>(), Ok(role));
        }
        assert_eq!(ServerRoleSet::all().iter().count(), ServerRole::ALL.len());
        assert_eq!(ServerRoleSet::default(), ServerRoleSet::all());
        assert!(ServerRoleSet::none().is_empty());
    }
}
