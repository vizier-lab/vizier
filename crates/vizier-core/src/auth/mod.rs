//! Who is making a request, as far as the engine cares: the identity an adapter authenticated.
//!
//! How that identity was established (JWT issue and verification, password and API-key
//! hashing) is the adapter's business and stays there; the engine only reads the result, for
//! permission checks ([`crate::access::Caller`]) and revision attribution
//! ([`crate::schema::revision::RevisionOrigin`]).

pub mod password;

pub use crate::storage::user::Role;

/// How a request was authenticated. Used to attribute document revisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AuthMethod {
    /// A JWT bearer token: the WebUI's login flow.
    Jwt,
    /// An API key: a programmatic caller.
    ApiKey,
    /// The desktop app's in-process caller: the window the person signed in to by opening the app.
    /// Only the embedded dispatcher can produce it; the network router never accepts it.
    Desktop,
}

/// An authenticated person.
#[derive(Debug, Clone)]
pub struct Principal {
    pub user_id: String,
    pub username: String,
    pub role: Role,
    pub permissions: Vec<String>,
    pub method: AuthMethod,
}

/// The pre-038 name of [`Principal`]; removed at the end of the desktop app work.
pub type Identity = Principal;
