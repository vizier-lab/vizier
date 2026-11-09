//! Who is making a request, as far as the engine cares: the identity an adapter authenticated.
//!
//! How that identity was established (JWT issue and verification, password and API-key
//! hashing) is the adapter's business and stays there; the engine only reads the result, for
//! permission checks ([`crate::access::Caller`]) and revision attribution
//! ([`crate::schema::revision::RevisionOrigin`]).

pub mod password;

pub use crate::storage::user::Role;

/// How a request was authenticated. Used to attribute document revisions.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AuthMethod {
    /// A JWT bearer token: the WebUI's login flow.
    Jwt,
    /// An API key: a programmatic caller.
    ///
    /// `scoped` says the key was made with a capability list, so it carries only those and not its
    /// owner's permissions (T109, FR-062). `key_id` and `expires_at` are how a session that key
    /// opened can be found again when the key is revoked or lapses (T110, FR-063) — without them
    /// the server would have to guess, or query storage on every frame. It lives here rather than
    /// on [`Principal`] because all of it is a property of the credential, not of the person.
    ApiKey {
        scoped: bool,
        /// Which key this is, when the caller had one. `None` only in tests and in callers that
        /// never came through the middleware.
        key_id: Option<String>,
        /// When the key lapses. The socket has no timer (T110 draws the line at an event), so this
        /// is what lets the frame path notice a lapse without a storage read per frame.
        expires_at: Option<crate::time::Timestamp>,
    },
    /// The desktop app's in-process caller: the window the person signed in to by opening the app.
    /// Only the embedded dispatcher can produce it; the network router never accepts it.
    Desktop,
}

impl AuthMethod {
    /// The API key this caller used, if any. Two keys of the same person must never be mistaken for
    /// one another when a session is owned (T110).
    pub fn key_id(&self) -> Option<&str> {
        match self {
            AuthMethod::ApiKey { key_id, .. } => key_id.as_deref(),
            _ => None,
        }
    }
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
