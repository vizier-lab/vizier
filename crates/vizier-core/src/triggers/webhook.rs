//! The `webhook` kind (`specs/071-triggers` FR-018 to FR-024): an external system pushes a request
//! to the trigger's own address.
//!
//! Order matters: the secret is checked first, then the optional signature over the exact bytes
//! and the optional timestamp; only an authenticated sender learns anything about size or type.
//! Every authentication failure is the same [`Intake::Reject`], answered like an unknown trigger.

use ring::hmac;
use serde_json::{Map, Value, json};

use super::kind::{Intake, PushRefusal, PushRequest, TriggerKind, new_secret};
use crate::{
    integrations::kind::{FieldType, KindField, KindListing},
    schema::Trigger,
};

/// The largest body accepted (FR-022).
pub const MAX_BODY_BYTES: usize = 256 * 1024;
/// The default timestamp tolerance (FR-019).
pub const DEFAULT_TOLERANCE_SECS: i64 = 300;

pub struct WebhookKind;

fn text(settings: &Value, key: &str) -> Option<String> {
    settings
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn list(settings: &Value, key: &str) -> Vec<String> {
    settings
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Constant-time equality of two byte strings.
fn same(a: &[u8], b: &[u8]) -> bool {
    // Every byte is compared whatever the earlier ones were, so the time says nothing about how
    // much of a guess was right. Only the length can differ, and a secret's length is no secret.
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |diff, (x, y)| diff | (x ^ y)) == 0
}

fn hex_decode(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

/// One filter rule: `<path> = <value>` or `<path> != <value>`, where the path starts with
/// `header.` or `body.` and descends with dots.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Rule {
    path: String,
    value: String,
    negate: bool,
}

fn parse_rule(rule: &str) -> Option<Rule> {
    let (path, value, negate) = match rule.split_once("!=") {
        Some((p, v)) => (p, v, true),
        None => {
            let (p, v) = rule.split_once('=')?;
            (p, v, false)
        }
    };
    let path = path.trim();
    (path.starts_with("header.") || path.starts_with("body."))
        .then(|| Rule { path: path.to_string(), value: value.trim().to_string(), negate })
}

/// The value at `path` in a payload, as text.
fn lookup(payload: &Value, path: &str) -> Option<String> {
    let mut parts = path.split('.');
    let root = match parts.next()? {
        "header" => payload.get("headers")?,
        "body" => payload.get("body")?,
        _ => return None,
    };
    let mut at = root;
    for part in parts {
        at = match at {
            Value::Object(map) => map.get(part).or_else(|| map.get(&part.to_ascii_lowercase()))?,
            Value::Array(items) => items.get(part.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    match at {
        Value::String(s) => Some(s.clone()),
        Value::Null => None,
        other => Some(other.to_string()),
    }
}

/// The first rule a payload fails, if any.
fn failed_rule(payload: &Value, rules: &[String]) -> Option<String> {
    rules.iter().find_map(|raw| {
        let rule = parse_rule(raw)?;
        let found = lookup(payload, &rule.path);
        let matches = found.as_deref() == Some(rule.value.as_str());
        (matches == rule.negate).then(|| raw.clone())
    })
}

/// The request's time from the configured header: Unix seconds, milliseconds or RFC 3339.
fn sent_at(value: &str) -> Option<i64> {
    let value = value.trim();
    if let Ok(n) = value.parse::<i64>() {
        return Some(if n > 100_000_000_000 { n / 1000 } else { n });
    }
    chrono::DateTime::parse_from_rfc3339(value).ok().map(|t| t.timestamp())
}

fn signature_ok(settings: &Value, request: &PushRequest) -> bool {
    let Some(secret) = text(settings, "signing_secret") else {
        return true;
    };
    let header = text(settings, "signature_header").unwrap_or_else(|| "x-hub-signature-256".into());
    let algorithm = match text(settings, "signature_algorithm").as_deref() {
        Some("hmac-sha1") => hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY,
        _ => hmac::HMAC_SHA256,
    };
    let Some(given) = request.header(&header) else {
        return false;
    };
    // GitHub-style values carry the algorithm before the hex digest.
    let given = given.trim();
    let given = given.split_once('=').map_or(given, |(_, digest)| digest);
    let Some(given) = hex_decode(given) else {
        return false;
    };
    let key = hmac::Key::new(algorithm, secret.as_bytes());
    hmac::verify(&key, &request.body, &given).is_ok()
}

fn timestamp_ok(settings: &Value, request: &PushRequest) -> bool {
    let Some(header) = text(settings, "timestamp_header") else {
        return true;
    };
    let tolerance = settings
        .get("timestamp_tolerance_secs")
        .and_then(Value::as_i64)
        .unwrap_or(DEFAULT_TOLERANCE_SECS);
    let Some(sent) = request.header(&header).and_then(sent_at) else {
        return false;
    };
    (request.received_at.timestamp() - sent).abs() <= tolerance
}

fn authenticated(settings: &Value, request: &PushRequest) -> bool {
    let Some(token) = text(settings, "token") else {
        return false;
    };
    let bearer = request
        .header("authorization")
        .and_then(|v| v.strip_prefix("Bearer ").or_else(|| v.strip_prefix("bearer ")))
        .map(str::trim);
    let given = bearer.or(request.path_secret.as_deref());
    given.is_some_and(|given| same(given.as_bytes(), token.as_bytes()))
}

/// The body as data the agent and triage can read.
fn parse_body(request: &PushRequest) -> Result<Value, PushRefusal> {
    let content_type = request
        .header("content-type")
        .map(|v| v.split(';').next().unwrap_or("").trim().to_ascii_lowercase())
        .unwrap_or_default();
    let as_text = || String::from_utf8_lossy(&request.body).into_owned();
    if content_type.is_empty() || content_type.starts_with("text/") {
        return Ok(Value::String(as_text()));
    }
    if content_type == "application/json" || content_type.ends_with("+json") {
        return Ok(serde_json::from_slice(&request.body).unwrap_or_else(|_| Value::String(as_text())));
    }
    if content_type == "application/x-www-form-urlencoded" {
        let mut map = Map::new();
        for (key, value) in url::form_urlencoded::parse(&request.body) {
            map.insert(key.into_owned(), Value::String(value.into_owned()));
        }
        return Ok(Value::Object(map));
    }
    Err(PushRefusal::UnsupportedType)
}

impl TriggerKind for WebhookKind {
    fn id(&self) -> &'static str {
        "webhook"
    }

    fn listing(&self) -> KindListing {
        KindListing {
            name: "Webhook",
            description: "An outside system starts a run by sending a request to this trigger's address",
            icon: "webhook",
            fields: vec![
                KindField::new("token", "Secret", FieldType::Secret).generated(),
                KindField::new("filter", "Only accept requests where (e.g. body.ref=refs/heads/master)", FieldType::StringList),
                KindField::new("delivery_id", "Delivery id (e.g. header.x-github-delivery)", FieldType::String),
                KindField::new("forward_headers", "Headers the agent may see", FieldType::StringList),
                KindField::new("signing_secret", "Signing secret (optional)", FieldType::Secret).advanced(),
                KindField::new("signature_header", "Signature header", FieldType::String)
                    .default(json!("x-hub-signature-256"))
                    .advanced(),
                KindField::new("signature_algorithm", "Signature algorithm", FieldType::Select)
                    .options(vec!["hmac-sha256", "hmac-sha1"])
                    .default(json!("hmac-sha256"))
                    .advanced(),
                KindField::new("timestamp_header", "Timestamp header (optional)", FieldType::String)
                    .advanced(),
            ],
        }
    }

    fn validate(&self, settings: &Value) -> Result<(), String> {
        if !settings.is_object() {
            return Err("webhook settings must be an object".into());
        }
        for rule in list(settings, "filter") {
            if parse_rule(&rule).is_none() {
                return Err(format!(
                    "filter rule '{rule}' must read header.<name>=<value> or body.<path>=<value> (or !=)"
                ));
            }
        }
        if let Some(path) = text(settings, "delivery_id")
            && !(path.starts_with("header.") || path.starts_with("body."))
        {
            return Err("delivery id must name header.<name> or body.<path>".into());
        }
        if let Some(t) = settings.get("timestamp_tolerance_secs")
            && !t.as_i64().is_some_and(|n| (1..=86_400).contains(&n))
        {
            return Err("timestamp tolerance must be between 1 second and a day".into());
        }
        for header in list(settings, "forward_headers") {
            let lower = header.to_ascii_lowercase();
            if lower == "authorization" || lower.contains("signature") || lower == "cookie" {
                return Err(format!("the '{header}' header is never passed to the agent"));
            }
        }
        Ok(())
    }

    fn prepare(&self, settings: &mut Value) -> Option<String> {
        self.replace_secret(settings)
    }

    fn replace_secret(&self, settings: &mut Value) -> Option<String> {
        let secret = new_secret();
        if let Some(map) = settings.as_object_mut() {
            map.insert("token".into(), Value::String(secret.clone()));
        }
        Some(secret)
    }

    fn is_public(&self) -> bool {
        true
    }

    fn authenticates_without_body(&self, trigger: &Trigger, request: &PushRequest) -> bool {
        authenticated(&trigger.settings, request)
    }

    fn receive(&self, trigger: &Trigger, request: &PushRequest) -> Result<Intake, PushRefusal> {
        let settings = &trigger.settings;
        if !authenticated(settings, request) || !signature_ok(settings, request) || !timestamp_ok(settings, request) {
            return Ok(Intake::Reject);
        }
        if request.body.len() > MAX_BODY_BYTES {
            return Err(PushRefusal::TooLarge);
        }
        let body = parse_body(request)?;

        let signature_header = text(settings, "signature_header").unwrap_or_else(|| "x-hub-signature-256".into());
        let mut headers = Map::new();
        for name in list(settings, "forward_headers") {
            let lower = name.to_ascii_lowercase();
            if lower == "authorization" || lower == signature_header.to_ascii_lowercase() {
                continue;
            }
            if let Some(value) = request.header(&lower) {
                headers.insert(lower, Value::String(value.to_string()));
            }
        }
        // Filters and the delivery id may read any header, but only forwarded ones are stored.
        let mut all_headers = Map::new();
        for (name, value) in &request.headers {
            if name != "authorization" {
                all_headers.insert(name.clone(), Value::String(value.clone()));
            }
        }
        let lookup_view = json!({ "headers": all_headers, "body": body });
        let delivery_id = text(settings, "delivery_id").and_then(|path| lookup(&lookup_view, &path));

        if let Some(rule) = failed_rule(&lookup_view, &list(settings, "filter")) {
            return Ok(Intake::Ignore { reason: format!("filter: {rule}"), delivery_id });
        }
        let content_type = request.header("content-type").unwrap_or("text/plain").to_string();
        Ok(Intake::Fire {
            payload: json!({ "content_type": content_type, "headers": headers, "body": body }),
            delivery_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{OverlapPolicy, Requester};

    fn trigger(settings: Value) -> Trigger {
        Trigger {
            id: "t".into(),
            agent_id: "a".into(),
            slug: "ci".into(),
            kind: "webhook".into(),
            title: "CI".into(),
            instruction: "".into(),
            settings,
            requester: Requester::User("alice".into()),
            delivers_to: None,
            is_active: true,
            awaiting_approval: false,
            needs_attention: None,
            tool_scope: None,
            overlap: OverlapPolicy::Wait,
            hourly_limit: Some(20),
            cooldown_secs: 0,
            triage: None,
            destination: None,
            summary_hours: None,
            last_summary_at: None,
            created_at: crate::time::now(),
            last_fired_at: None,
        }
    }

    fn request(headers: &[(&str, &str)], body: &str) -> PushRequest {
        PushRequest {
            headers: headers.iter().map(|(n, v)| (n.to_ascii_lowercase(), v.to_string())).collect(),
            body: body.as_bytes().to_vec(),
            path_secret: None,
            received_at: crate::time::now(),
        }
    }

    const JSON: (&str, &str) = ("content-type", "application/json");

    #[test]
    fn the_secret_is_accepted_in_a_header_or_the_address_and_nothing_else() {
        let t = trigger(json!({"token": "abc"}));
        let ok = request(&[("authorization", "Bearer abc"), JSON], "{}");
        assert!(matches!(WebhookKind.receive(&t, &ok), Ok(Intake::Fire { .. })));
        let mut by_path = request(&[JSON], "{}");
        by_path.path_secret = Some("abc".into());
        assert!(matches!(WebhookKind.receive(&t, &by_path), Ok(Intake::Fire { .. })));
        let wrong = request(&[("authorization", "Bearer abd"), JSON], "{}");
        assert_eq!(WebhookKind.receive(&t, &wrong), Ok(Intake::Reject));
        assert_eq!(WebhookKind.receive(&t, &request(&[JSON], "{}")), Ok(Intake::Reject));
    }

    #[test]
    fn a_signature_is_checked_over_the_exact_bytes() {
        let body = r#"{"ref":"refs/heads/master"}"#;
        let key = hmac::Key::new(hmac::HMAC_SHA256, b"sign");
        let tag = hmac::sign(&key, body.as_bytes());
        let hex: String = tag.as_ref().iter().map(|b| format!("{b:02x}")).collect();
        let t = trigger(json!({"token": "abc", "signing_secret": "sign"}));
        let good = format!("sha256={hex}");
        let ok = request(&[("authorization", "Bearer abc"), ("x-hub-signature-256", &good), JSON], body);
        assert!(matches!(WebhookKind.receive(&t, &ok), Ok(Intake::Fire { .. })));
        let tampered = request(
            &[("authorization", "Bearer abc"), ("x-hub-signature-256", &good), JSON],
            r#"{"ref":"refs/heads/evil"}"#,
        );
        assert_eq!(WebhookKind.receive(&t, &tampered), Ok(Intake::Reject));
    }

    #[test]
    fn an_old_request_is_refused_when_a_timestamp_is_required() {
        let t = trigger(json!({"token": "abc", "timestamp_header": "x-time"}));
        let now = crate::time::now().timestamp();
        let fresh = now.to_string();
        let stale = (now - 3600).to_string();
        let ok = request(&[("authorization", "Bearer abc"), ("x-time", &fresh), JSON], "{}");
        assert!(matches!(WebhookKind.receive(&t, &ok), Ok(Intake::Fire { .. })));
        let old = request(&[("authorization", "Bearer abc"), ("x-time", &stale), JSON], "{}");
        assert_eq!(WebhookKind.receive(&t, &old), Ok(Intake::Reject));
    }

    #[test]
    fn a_filter_on_a_body_field_ignores_other_branches() {
        let t = trigger(json!({"token": "abc", "filter": ["body.ref=refs/heads/master"]}));
        let master = request(&[("authorization", "Bearer abc"), JSON], r#"{"ref":"refs/heads/master"}"#);
        assert!(matches!(WebhookKind.receive(&t, &master), Ok(Intake::Fire { .. })));
        let branch = request(&[("authorization", "Bearer abc"), JSON], r#"{"ref":"refs/heads/x"}"#);
        assert!(matches!(WebhookKind.receive(&t, &branch), Ok(Intake::Ignore { .. })));
    }

    #[test]
    fn the_delivery_id_comes_from_a_header_or_the_body() {
        let t = trigger(json!({"token": "abc", "delivery_id": "header.x-github-delivery"}));
        let r = request(&[("authorization", "Bearer abc"), ("X-GitHub-Delivery", "d-1"), JSON], "{}");
        assert!(matches!(WebhookKind.receive(&t, &r), Ok(Intake::Fire { delivery_id: Some(id), .. }) if id == "d-1"));
        let t = trigger(json!({"token": "abc", "delivery_id": "body.message.messageId"}));
        let r = request(&[("authorization", "Bearer abc"), JSON], r#"{"message":{"messageId":"m-9"}}"#);
        assert!(matches!(WebhookKind.receive(&t, &r), Ok(Intake::Fire { delivery_id: Some(id), .. }) if id == "m-9"));
    }

    #[test]
    fn secrets_never_reach_the_payload_and_size_is_checked_after_the_secret() {
        let t = trigger(json!({"token": "abc", "forward_headers": ["x-github-event"]}));
        let r = request(&[("authorization", "Bearer abc"), ("x-github-event", "push"), JSON], r#"{"a":1}"#);
        let Ok(Intake::Fire { payload, .. }) = WebhookKind.receive(&t, &r) else { panic!() };
        assert_eq!(payload["headers"], json!({"x-github-event": "push"}));
        assert!(!payload.to_string().contains("abc"));

        let big = "x".repeat(MAX_BODY_BYTES + 1);
        let unauth = request(&[JSON], &big);
        assert_eq!(WebhookKind.receive(&t, &unauth), Ok(Intake::Reject));
        let auth = request(&[("authorization", "Bearer abc"), JSON], &big);
        assert_eq!(WebhookKind.receive(&t, &auth), Err(PushRefusal::TooLarge));
        let binary = request(&[("authorization", "Bearer abc"), ("content-type", "image/png")], "x");
        assert_eq!(WebhookKind.receive(&t, &binary), Err(PushRefusal::UnsupportedType));
    }

    #[test]
    fn settings_are_validated_and_a_secret_is_generated_once() {
        assert!(WebhookKind.validate(&json!({"filter": ["nonsense"]})).is_err());
        assert!(WebhookKind.validate(&json!({"forward_headers": ["Authorization"]})).is_err());
        assert!(WebhookKind.validate(&json!({"filter": ["header.x-github-event=push"]})).is_ok());
        let mut settings = json!({});
        let secret = WebhookKind.prepare(&mut settings).unwrap();
        assert_eq!(settings["token"], json!(secret));
        assert_eq!(secret.len(), 64);
        assert_ne!(WebhookKind.redact(&settings)["token"], json!(secret));
    }
}
