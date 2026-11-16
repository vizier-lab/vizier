//! Frame tickets (`specs/063-html-generative-ui` research R7a): a page frame is loaded from
//! `/api/v1/ui-frames/{ticket}`, a URL that carries no credential of the person. The page's own
//! script can read its URL, and `https` images give it a way out, so the URL must grant nothing
//! beyond the one version it shows.
//!
//! A ticket is `{page}.{version}.{expires_ms}.{signature}`, the signature an HMAC-SHA256 of the
//! first three parts under a key made when the process starts. It is minted by an authenticated
//! route after the same check as reading the page, and lasts [`TICKET_TTL_MS`]. A restart turns
//! open tickets into 404s; the WebUI mints a new one when it draws a page again.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::{hmac, rand::SecureRandom};

/// How long a ticket opens its frame.
pub const TICKET_TTL_MS: i64 = 10 * 60 * 1000;

/// What a valid ticket names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ticket {
    pub page_id: String,
    pub version: u32,
    pub expires_ms: i64,
}

/// The process's ticket key.
#[derive(Clone)]
pub struct TicketKey(hmac::Key);

impl TicketKey {
    /// A fresh random key.
    pub fn generate() -> Self {
        let mut bytes = [0u8; 32];
        // The system RNG failing is not something the process can work around; a key of zeros
        // would still sign, so fall back to the clock rather than refuse to start.
        if ring::rand::SystemRandom::new().fill(&mut bytes).is_err() {
            let nanos = crate::time::now().timestamp_nanos_opt().unwrap_or_default();
            bytes[..8].copy_from_slice(&nanos.to_le_bytes());
            tracing::warn!("the system random source failed; page frame tickets use a weaker key");
        }
        Self(hmac::Key::new(hmac::HMAC_SHA256, &bytes))
    }

    fn sign(&self, body: &str) -> String {
        URL_SAFE_NO_PAD.encode(hmac::sign(&self.0, body.as_bytes()).as_ref())
    }

    /// A ticket for `page_id` v`version` that expires `TICKET_TTL_MS` after `now_ms`.
    pub fn mint(&self, page_id: &str, version: u32, now_ms: i64) -> (String, i64) {
        let expires_ms = now_ms + TICKET_TTL_MS;
        let body = format!("{page_id}.{version}.{expires_ms}");
        let signature = self.sign(&body);
        (format!("{body}.{signature}"), expires_ms)
    }

    /// The page and version a ticket opens, if it is genuine and not expired at `now_ms`.
    pub fn verify(&self, ticket: &str, now_ms: i64) -> Option<Ticket> {
        let (body, signature) = ticket.rsplit_once('.')?;
        let signature = URL_SAFE_NO_PAD.decode(signature).ok()?;
        hmac::verify(&self.0, body.as_bytes(), &signature).ok()?;
        let mut parts = body.splitn(3, '.');
        let page_id = parts.next()?.to_string();
        let version = parts.next()?.parse().ok()?;
        let expires_ms: i64 = parts.next()?.parse().ok()?;
        (expires_ms > now_ms).then_some(Ticket { page_id, version, expires_ms })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_minted_ticket_opens_its_version_until_it_expires() {
        let key = TicketKey::generate();
        let (ticket, expires) = key.mint("pg_abc", 3, 1_000);
        assert_eq!(expires, 1_000 + TICKET_TTL_MS);
        assert_eq!(
            key.verify(&ticket, 2_000),
            Some(Ticket { page_id: "pg_abc".into(), version: 3, expires_ms: expires })
        );
        assert_eq!(key.verify(&ticket, expires), None, "expired");
    }

    #[test]
    fn a_changed_or_foreign_ticket_opens_nothing() {
        let key = TicketKey::generate();
        let (ticket, _) = key.mint("pg_abc", 3, 1_000);
        let other_version = ticket.replacen(".3.", ".4.", 1);
        assert_eq!(key.verify(&other_version, 2_000), None);
        assert_eq!(TicketKey::generate().verify(&ticket, 2_000), None, "another process's key");
        assert_eq!(key.verify("garbage", 2_000), None);
        assert_eq!(key.verify("pg_abc.3.9999999999999.", 2_000), None);
    }
}
