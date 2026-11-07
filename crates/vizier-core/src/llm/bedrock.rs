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

/// Which of AWS's two inference-profile resource types an ARN names (research D11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileResource {
    /// `inference-profile`: the system-defined profile AWS maintains.
    System,
    /// `application-inference-profile`: the profile a person creates in the console.
    Application,
}

impl ProfileResource {
    /// The resource type as it appears in the ARN.
    pub fn as_str(self) -> &'static str {
        match self {
            ProfileResource::System => "inference-profile",
            ProfileResource::Application => "application-inference-profile",
        }
    }
}

/// The partitions an inference profile may live in (FR-023).
pub const ARN_PARTITIONS: [&str; 5] = ["aws", "aws-cn", "aws-us-gov", "aws-iso", "aws-iso-b"];

/// A value that parsed as
/// `arn:{partition}:bedrock:{region}:{account}:{resource}/{profile-id}`
/// (contract `bedrock-sso-wire.md` §6, FR-023).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InferenceProfileArn {
    /// One of [`ARN_PARTITIONS`].
    pub partition: String,
    /// The region segment, checked by [`valid_region`]; FR-024 compares it with the connection's.
    pub region: String,
    /// The 12-digit account segment.
    pub account: String,
    /// Which of the two resource types the ARN names.
    pub resource: ProfileResource,
    /// The profile identifier, used as an entry's display name when none was given (FR-026).
    pub id: String,
    /// The value trimmed once: what is sent to the runtime, unchanged (FR-027).
    pub raw: String,
}

/// Why a value is not an inference-profile ARN; every message names the expected shape
/// (FR-023, SC-008).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArnError {
    /// The literal `arn:` prefix is missing (a console URL, a bare model id).
    NotArn,
    /// Fewer than the six colon-separated segments the grammar has.
    Incomplete,
    /// The partition segment is not one of AWS's five.
    Partition(String),
    /// The service segment is not `bedrock`.
    Service(String),
    /// The region segment is not an AWS region.
    Region(String),
    /// The account segment is not 12 digits.
    Account(String),
    /// The resource segment is neither inference-profile type.
    Resource(String),
    /// The profile-id segment is empty or outside `[A-Za-z0-9.:-]`.
    NoProfileId,
}

impl std::fmt::Display for ArnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArnError::NotArn => write!(f, "an inference profile ARN starts with \"arn:\""),
            ArnError::Incomplete => write!(
                f,
                "an inference profile ARN has the form \
                 arn:{{partition}}:bedrock:{{region}}:{{account}}:{{resource}}/{{profile-id}}"
            ),
            ArnError::Partition(p) => write!(
                f,
                "unsupported ARN partition \"{p}\": expected aws, aws-cn, aws-us-gov, aws-iso or aws-iso-b"
            ),
            ArnError::Service(s) => write!(
                f,
                "\"{s}\" is not a Bedrock ARN: the service segment must be \"bedrock\""
            ),
            ArnError::Region(r) => write!(f, "\"{r}\" is not an AWS region"),
            ArnError::Account(a) => {
                write!(
                    f,
                    "the ARN's account segment must be 12 digits, not \"{a}\""
                )
            }
            ArnError::Resource(r) => write!(
                f,
                "an inference profile ARN names \"inference-profile\" or \
                 \"application-inference-profile\", not \"{r}\""
            ),
            ArnError::NoProfileId => write!(f, "the ARN names no profile id"),
        }
    }
}

impl std::error::Error for ArnError {}

/// Parse an inference-profile ARN. Pure: no AWS call and no credential (research D11).
///
/// The value is trimmed once, so a copy out of the console's table is accepted; an internal
/// space still fails its segment rule. Both resource types are accepted on equal footing and
/// neither is rewritten, because the ARN is sent to the runtime exactly as saved (FR-023,
/// FR-027).
pub fn parse_inference_profile_arn(raw: &str) -> Result<InferenceProfileArn, ArnError> {
    let value = raw.trim();
    let rest = value.strip_prefix("arn:").ok_or(ArnError::NotArn)?;
    let mut segments = rest.splitn(5, ':');
    let partition = segments.next().unwrap_or_default();
    let service = segments.next().ok_or(ArnError::Incomplete)?;
    let region = segments.next().ok_or(ArnError::Incomplete)?;
    let account = segments.next().ok_or(ArnError::Incomplete)?;
    let tail = segments.next().ok_or(ArnError::Incomplete)?;

    if !ARN_PARTITIONS.contains(&partition) {
        return Err(ArnError::Partition(partition.to_string()));
    }
    if service != SERVICE {
        return Err(ArnError::Service(service.to_string()));
    }
    if !valid_region(region) {
        return Err(ArnError::Region(region.to_string()));
    }
    if account.len() != 12 || !account.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ArnError::Account(account.to_string()));
    }
    // The resource segment is compared as a whole: the application form is never read as the
    // system form with an odd id.
    let (resource, id) = tail.split_once('/').unwrap_or((tail, ""));
    let resource = match resource {
        "inference-profile" => ProfileResource::System,
        "application-inference-profile" => ProfileResource::Application,
        other => return Err(ArnError::Resource(other.to_string())),
    };
    if id.is_empty()
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b':' | b'-'))
    {
        return Err(ArnError::NoProfileId);
    }

    Ok(InferenceProfileArn {
        partition: partition.to_string(),
        region: region.to_string(),
        account: account.to_string(),
        resource,
        id: id.to_string(),
        raw: value.to_string(),
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

    // ---- Inference-profile ARNs (FR-023, FR-024, SC-008) ----

    /// SC-008: both AWS resource types, in all five partitions, parse on equal footing and each
    /// segment is returned as it was written.
    #[test]
    fn both_resource_types_parse_in_every_partition() {
        for (partition, region) in [
            ("aws", "us-east-1"),
            ("aws-cn", "cn-north-1"),
            ("aws-us-gov", "us-gov-west-1"),
            ("aws-iso", "us-iso-east-1"),
            ("aws-iso-b", "us-isob-east-1"),
        ] {
            for (resource, id) in [
                (ProfileResource::Application, "abc123def456"),
                (
                    ProfileResource::System,
                    "us.anthropic.claude-sonnet-4-5-20250929-v1:0",
                ),
            ] {
                let raw = format!(
                    "arn:{partition}:bedrock:{region}:123456789012:{}/{id}",
                    resource.as_str()
                );
                let arn =
                    parse_inference_profile_arn(&raw).unwrap_or_else(|e| panic!("{raw}: {e}"));
                assert_eq!(arn.partition, partition);
                assert_eq!(arn.region, region, "{raw}");
                assert_eq!(arn.account, "123456789012");
                assert_eq!(arn.resource, resource);
                assert_eq!(arn.id, id);
                assert_eq!(arn.raw, raw);
            }
        }
    }

    /// FR-024 compares the connection's region with the ARN's own segment, so the parser returns
    /// that segment and nothing derived from it.
    #[test]
    fn the_parsed_region_is_the_arn_segment() {
        let arn = parse_inference_profile_arn(
            "arn:aws-us-gov:bedrock:us-gov-east-1:123456789012:application-inference-profile/x1:0",
        )
        .unwrap();
        assert_eq!(arn.partition, "aws-us-gov");
        assert_eq!(arn.region, "us-gov-east-1");
        assert_eq!(arn.resource, ProfileResource::Application);
        assert_eq!(arn.id, "x1:0");
    }

    /// A copy out of the console's table carries padding: trimmed once, then parsed (contract §6).
    #[test]
    fn surrounding_padding_is_trimmed_once() {
        let padded =
            "\t arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/abc123 \n";
        let arn = parse_inference_profile_arn(padded).unwrap();
        assert_eq!(
            arn.raw,
            "arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/abc123"
        );
        assert_eq!(arn.id, "abc123");
    }

    /// Every refusal names the expected shape (FR-023, SC-008): a foundation-model ARN, another
    /// service's ARN, a bare model id, a truncated ARN, a console URL, and padding inside a
    /// segment.
    #[test]
    fn non_inference_profile_arn_values_are_refused() {
        for (value, expected) in [
            (
                "arn:aws:bedrock:us-east-1:123456789012:foundation-model/anthropic.claude-3-5-sonnet-20241022-v2:0",
                "an inference profile ARN names \"inference-profile\" or \"application-inference-profile\"",
            ),
            (
                "arn:aws:bedrock:us-east-1:123456789012:inference-profile-v2/x",
                "an inference profile ARN names",
            ),
            (
                "arn:aws:s3:us-east-1:123456789012:inference-profile/x",
                "is not a Bedrock ARN",
            ),
            (
                "arn:aws:bedrock-runtime:us-east-1:123456789012:application-inference-profile/x",
                "is not a Bedrock ARN",
            ),
            (
                "anthropic.claude-sonnet-4-5-20250929-v1:0",
                "starts with \"arn:\"",
            ),
            (
                "us.anthropic.claude-sonnet-4-5-20250929-v1:0",
                "starts with \"arn:\"",
            ),
            (
                "arn:aws:bedrock:us-east-1:123456789012:inference-profile",
                "names no profile id",
            ),
            (
                "arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/",
                "names no profile id",
            ),
            (
                "arn:aws:bedrock:us-east-1:123456789012:inference-profile/a/b",
                "names no profile id",
            ),
            ("arn:aws:bedrock:us-east-1:123456789012", "has the form"),
            (
                "arn:aws:bedrock:us-east-1:123456789012:",
                "an inference profile ARN names",
            ),
            (
                "https://console.aws.amazon.com/bedrock/home?region=us-east-1#/inference-profiles",
                "starts with \"arn:\"",
            ),
            ("", "starts with \"arn:\""),
            (
                "arn:aws-iso-e:bedrock:us-iso-east-1:123456789012:inference-profile/x",
                "unsupported ARN partition \"aws-iso-e\"",
            ),
            (
                "arn:aws:bedrock: us-east-1:123456789012:inference-profile/x",
                "is not an AWS region",
            ),
            (
                "arn:aws:bedrock:us-east:123456789012:inference-profile/x",
                "is not an AWS region",
            ),
            (
                "arn:aws:bedrock:us-east-1:12345678901:inference-profile/x",
                "must be 12 digits",
            ),
            (
                "arn:aws:bedrock:us-east-1: 123456789012:inference-profile/x",
                "must be 12 digits",
            ),
            (
                "arn:aws:bedrock:us-east-1:123456789012:inference-profile/ x",
                "names no profile id",
            ),
            (
                "arn:aws:bedrock:us-east-1:123456789012:inference-profile/x y",
                "names no profile id",
            ),
        ] {
            let err = parse_inference_profile_arn(value).unwrap_err();
            assert!(
                err.to_string().contains(expected),
                "{value:?} gave {err:?}, which does not name {expected:?}"
            );
        }
    }

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
