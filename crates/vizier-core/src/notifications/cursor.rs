//! The inbox's opaque page cursor: URL-safe base64 (no padding) of `{"at": <millis>, "id": <i64>}`,
//! so `(created_at, id)` paging is stable even when two events share an instant.

use base64::Engine;
use chrono::{DateTime, TimeZone, Utc};

pub fn encode(at: DateTime<Utc>, id: i64) -> String {
    let json = serde_json::json!({ "at": at.timestamp_millis(), "id": id });
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json.to_string())
}

/// `None` for anything that is not this cursor: the API answers 400.
pub fn decode(cursor: &str) -> Option<(DateTime<Utc>, i64)> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(cursor)
        .ok()?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let at = Utc
        .timestamp_millis_opt(value.get("at")?.as_i64()?)
        .single()?;
    Some((at, value.get("id")?.as_i64()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cursor_round_trips() {
        let at = Utc
            .timestamp_millis_opt(1_761_380_442_000)
            .single()
            .unwrap();
        let (back_at, back_id) = decode(&encode(at, 42)).expect("round trip");
        assert_eq!(back_at, at);
        assert_eq!(back_id, 42);
    }

    #[test]
    fn a_malformed_cursor_is_refused() {
        assert!(decode("not a cursor!").is_none());
        assert!(decode("").is_none());
    }

    #[test]
    fn a_cursor_missing_a_field_is_refused() {
        let only_at = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode("{\"at\":1}");
        assert!(decode(&only_at).is_none());
    }
}
