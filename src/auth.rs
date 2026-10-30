//! Who is making a request, as far as the engine cares: the identity an adapter authenticated.
//!
//! How that identity was established (JWT issue and verification, password and API-key
//! hashing) is the adapter's business and stays there; the engine only reads the result, for
//! permission checks ([`crate::access::Caller`]) and revision attribution
//! ([`crate::schema::revision::RevisionOrigin`]).

pub use crate::storage::user::Role;

/// How a request was authenticated. Used to attribute document revisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AuthMethod {
    /// A JWT bearer token: the WebUI's login flow.
    Jwt,
    /// An API key: a programmatic caller.
    ApiKey,
}

/// An authenticated person.
#[derive(Debug, Clone)]
pub struct Identity {
    pub user_id: String,
    pub username: String,
    pub role: Role,
    pub permissions: Vec<String>,
    pub method: AuthMethod,
}
