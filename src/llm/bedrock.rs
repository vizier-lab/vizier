//! Bedrock's own wire facts: model-id encoding, regional endpoints and the control-plane
//! `ListFoundationModels` call behind the connection test (research R3/R8, contract
//! `bedrock-wire.md`).

use std::time::Duration;

use serde_json::Value;

use super::auth::{CredentialSource, RequestInfo};
use super::error::LlmError;
use super::http;

/// The SigV4 service name for both the runtime and the control plane.
pub const SERVICE: &str = "bedrock";

/// Percent-encode one opaque model id as a single path segment: unreserved bytes pass and every
/// other byte becomes uppercase `%XX` (Smithy's `BASE_SET`, as the AWS SDK's label formatter
/// does). `:` in a versioned id and `/` in an ARN are both encoded.
pub fn aws_uri_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// `/model/{encoded}/converse` or `/model/{encoded}/invoke`.
pub fn model_path(model: &str, operation: &str) -> String {
    format!("/model/{}/{}", aws_uri_encode(model), operation)
}

/// The runtime host for one region: chat, embeddings and images.
pub fn runtime_endpoint(region: &str) -> String {
    format!("https://bedrock-runtime.{region}.amazonaws.com")
}

/// The control-plane host for one region: the foundation-model listing behind Test.
pub fn control_endpoint(region: &str) -> String {
    format!("https://bedrock.{region}.amazonaws.com")
}

/// The control-plane URL for a runtime base URL (`…/bedrock-runtime.{region}…` → `…/bedrock.{region}…`),
/// so the Test action needs no second copy of the region resolution.
pub fn control_endpoint_from_runtime(runtime_base_url: &str) -> Result<String, LlmError> {
    let parsed = reqwest::Url::parse(runtime_base_url)
        .map_err(|e| LlmError::Config(format!("bad Bedrock endpoint {runtime_base_url:?}: {e}")))?;
    let host = parsed.host_str().ok_or_else(|| {
        LlmError::Config(format!(
            "bad Bedrock endpoint {runtime_base_url:?}: no host"
        ))
    })?;
    let control_host = match host.strip_prefix("bedrock-runtime.") {
        Some(rest) => format!("bedrock.{rest}"),
        // A custom or future endpoint: keep the host and let AWS answer.
        None => host.to_string(),
    };
    Ok(format!("https://{control_host}"))
}

/// `^[a-z]{2,4}(-[a-z0-9]+)+-\d+$`, checked without a regex: a prefix, one or more lowercase
/// segments and a numeric tail. Covers `us-east-1`, `us-gov-west-1`, `eusc-de-east-1`.
pub fn valid_region(region: &str) -> bool {
    let parts: Vec<&str> = region.split('-').collect();
    if parts.len() < 3 {
        return false;
    }
    let Some((first, rest)) = parts.split_first() else {
        return false;
    };
    let Some((last, middle)) = rest.split_last() else {
        return false;
    };
    (2..=4).contains(&first.len())
        && first.bytes().all(|b| b.is_ascii_lowercase())
        && !last.is_empty()
        && last.bytes().all(|b| b.is_ascii_digit())
        && middle.iter().all(|p| {
            !p.is_empty()
                && p.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

/// Count the `modelSummaries` in a `ListFoundationModels` reply.
pub fn count_foundation_models(body: &Value) -> Result<usize, LlmError> {
    body.get("modelSummaries")
        .and_then(Value::as_array)
        .map(Vec::len)
        .ok_or_else(|| {
            LlmError::Decode(format!(
                "the Bedrock foundation-model listing had no modelSummaries: {}",
                body.to_string().chars().take(500).collect::<String>()
            ))
        })
}

/// The connection test (research R8): a signed control-plane listing, no model access needed and
/// no tokens spent. One attempt; the caller applies the timeout.
pub async fn list_foundation_models(
    client: &reqwest::Client,
    credential: &dyn CredentialSource,
    runtime_base_url: &str,
    timeout: Duration,
) -> Result<usize, LlmError> {
    let url = format!(
        "{}/foundation-models",
        control_endpoint_from_runtime(runtime_base_url)?
    );
    let req = client
        .get(&url)
        .header(reqwest::header::ACCEPT, "application/json")
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .timeout(timeout);
    let req = credential
        .authorize_with_body(req, &RequestInfo::default(), "GET", &url, b"")
        .await?;
    let body = http::send_json(req).await?;
    count_foundation_models(&body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn model_ids_are_one_encoded_segment() {
        assert_eq!(
            aws_uri_encode("anthropic.claude-haiku-4-5-20251001-v1:0"),
            "anthropic.claude-haiku-4-5-20251001-v1%3A0"
        );
        assert_eq!(
            aws_uri_encode("us.anthropic.claude-haiku-4-5-20251001-v1:0"),
            "us.anthropic.claude-haiku-4-5-20251001-v1%3A0"
        );
        assert_eq!(
            aws_uri_encode("eu.amazon.nova-lite-v1:0"),
            "eu.amazon.nova-lite-v1%3A0"
        );
        assert_eq!(
            aws_uri_encode("apac.amazon.nova-micro-v1:0"),
            "apac.amazon.nova-micro-v1%3A0"
        );
        assert_eq!(
            aws_uri_encode("global.anthropic.claude-sonnet-4-5-20250929-v1:0"),
            "global.anthropic.claude-sonnet-4-5-20250929-v1%3A0"
        );
        // An ARN is one opaque segment too: `:` and `/` are encoded.
        assert_eq!(
            aws_uri_encode("arn:aws:bedrock:us-east-1:123456789012:inference-profile/x"),
            "arn%3Aaws%3Abedrock%3Aus-east-1%3A123456789012%3Ainference-profile%2Fx"
        );
        // `%` and `,` are sub-delims outside the unreserved set.
        assert_eq!(aws_uri_encode("a%b,c"), "a%25b%2Cc");
        assert_eq!(aws_uri_encode("plain-._~"), "plain-._~");
    }

    #[test]
    fn paths_name_the_operation() {
        assert_eq!(
            model_path("amazon.nova-lite-v1:0", "converse"),
            "/model/amazon.nova-lite-v1%3A0/converse"
        );
        assert_eq!(
            model_path("amazon.titan-embed-text-v2:0", "invoke"),
            "/model/amazon.titan-embed-text-v2%3A0/invoke"
        );
    }

    #[test]
    fn regional_endpoints() {
        assert_eq!(
            runtime_endpoint("us-east-1"),
            "https://bedrock-runtime.us-east-1.amazonaws.com"
        );
        assert_eq!(
            control_endpoint("eu-west-1"),
            "https://bedrock.eu-west-1.amazonaws.com"
        );
        assert_eq!(
            runtime_endpoint("us-gov-west-1"),
            "https://bedrock-runtime.us-gov-west-1.amazonaws.com"
        );
        assert_eq!(
            control_endpoint_from_runtime("https://bedrock-runtime.us-east-1.amazonaws.com")
                .unwrap(),
            "https://bedrock.us-east-1.amazonaws.com"
        );
        assert!(control_endpoint_from_runtime("not a url").is_err());
    }

    #[test]
    fn regions_are_lightly_validated() {
        for good in [
            "us-east-1",
            "eu-west-1",
            "us-gov-west-1",
            "eusc-de-east-1",
            "ap-southeast-2",
        ] {
            assert!(valid_region(good), "{good}");
        }
        for bad in [
            "",
            "us-east",
            "US-EAST-1",
            "us-east-x",
            "us-east-1-",
            "u-east-1",
            "useast1",
            " us-east-1",
        ] {
            assert!(!valid_region(bad), "{bad}");
        }
    }

    #[test]
    fn the_listing_counts_the_models() {
        assert_eq!(
            count_foundation_models(&json!({"modelSummaries": [{}, {}, {}]})).unwrap(),
            3
        );
        assert_eq!(
            count_foundation_models(&json!({"modelSummaries": []})).unwrap(),
            0
        );
        assert!(count_foundation_models(&json!({"other": 1})).is_err());
    }
}
