//! The envelope every bus message travels in.

use std::collections::BTreeMap;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::error::BusError;
use crate::subject::Subject;

/// Largest payload, in bytes (1 MiB).
pub const MAX_PAYLOAD: usize = 1_048_576;

/// The wire version this crate writes and understands.
pub const WIRE_VERSION: u8 = 1;

/// Open string headers: W3C `traceparent`/`tracestate` and whatever else a message needs.
pub type Headers = BTreeMap<String, String>;

/// One message on the bus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Envelope {
    /// Wire version; an unknown one is a decode error.
    pub v: u8,
    /// Message kind (`request`, `response_frame`, `agent_command`, ...); routes decoding.
    pub kind: String,
    /// Where a reply goes; `None` for fire-and-forget.
    pub reply_to: Option<Subject>,
    pub headers: Headers,
    /// Unix milliseconds when first queued; kept across a re-queue.
    pub queued_at_ms: Option<u64>,
    pub payload: Vec<u8>,
}

impl Envelope {
    /// An envelope around raw bytes, refused when the payload is over [`MAX_PAYLOAD`].
    pub fn new(kind: impl Into<String>, payload: Vec<u8>) -> Result<Self, BusError> {
        check_payload(payload.len())?;
        Ok(Self {
            v: WIRE_VERSION,
            kind: kind.into(),
            reply_to: None,
            headers: Headers::new(),
            queued_at_ms: None,
            payload,
        })
    }

    /// An envelope around the JSON of `value`.
    pub fn json<T: Serialize>(kind: impl Into<String>, value: &T) -> Result<Self, BusError> {
        let payload = serde_json::to_vec(value).map_err(|e| BusError::Decode(e.to_string()))?;
        Self::new(kind, payload)
    }

    /// Decode the payload as `T`, after checking the wire version.
    pub fn decode<T: DeserializeOwned>(&self) -> Result<T, BusError> {
        self.check_version()?;
        serde_json::from_slice(&self.payload).map_err(|e| BusError::Decode(e.to_string()))
    }

    /// Refuse a version this crate does not know.
    pub fn check_version(&self) -> Result<(), BusError> {
        if self.v == WIRE_VERSION {
            Ok(())
        } else {
            Err(BusError::Decode(format!("unknown wire version {}", self.v)))
        }
    }

    /// Check the payload against the limit; backends call this on publish, because the fields
    /// are public and an envelope can be built without [`Envelope::new`].
    pub fn check_size(&self) -> Result<(), BusError> {
        check_payload(self.payload.len())
    }

    pub fn with_reply_to(mut self, reply_to: Subject) -> Self {
        self.reply_to = Some(reply_to);
        self
    }

    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    pub fn with_queued_at_ms(mut self, ms: u64) -> Self {
        self.queued_at_ms = Some(ms);
        self
    }

    pub fn header(&self, key: &str) -> Option<&str> {
        self.headers.get(key).map(String::as_str)
    }
}

fn check_payload(size: usize) -> Result<(), BusError> {
    if size > MAX_PAYLOAD {
        Err(BusError::PayloadTooLarge { size, limit: MAX_PAYLOAD })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_limit_is_inclusive() {
        assert!(Envelope::new("k", vec![0; MAX_PAYLOAD]).is_ok());
        assert_eq!(
            Envelope::new("k", vec![0; MAX_PAYLOAD + 1]).unwrap_err(),
            BusError::PayloadTooLarge { size: MAX_PAYLOAD + 1, limit: MAX_PAYLOAD }
        );
    }

    #[test]
    fn an_unknown_version_is_a_decode_error() {
        let mut e = Envelope::json("k", &1u32).unwrap();
        e.v = 9;
        assert!(matches!(e.decode::<u32>(), Err(BusError::Decode(_))));
    }

    #[test]
    fn json_round_trips_with_headers() {
        let e = Envelope::json("k", &vec!["a", "b"])
            .unwrap()
            .with_header("traceparent", "00-x")
            .with_queued_at_ms(5)
            .with_reply_to(Subject::agents_cmd());
        let wire = serde_json::to_vec(&e).unwrap();
        let back: Envelope = serde_json::from_slice(&wire).unwrap();
        assert_eq!(back, e);
        assert_eq!(back.header("traceparent"), Some("00-x"));
        assert_eq!(back.decode::<Vec<String>>().unwrap(), vec!["a", "b"]);
    }
}
