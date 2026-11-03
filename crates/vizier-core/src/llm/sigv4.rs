//! AWS Signature Version 4, service `bedrock` (research R3, contract `bedrock-wire.md` §2).
//!
//! A pure signer: method, already-encoded path, sorted lowercase headers and the exact body
//! bytes in; `x-amz-date` and `Authorization` out. No I/O, no clock of its own (the caller
//! passes the time), so the AWS test vectors can run against a fixed instant.
//!
//! The canonical URI applies SigV4's *double* encoding (`%` → `%25`), matching the AWS SDK's
//! `signing_options.double_uri_encode = true` for Bedrock: a model id sent as
//! `anthropic.claude-…-v1%3A0` is signed as `…-v1%253A0`.

use chrono::{DateTime, Utc};
use ring::{digest, hmac};

/// One request to sign. `path` and `query` are what goes on the wire (already encoded);
/// `content_type` is signed when the request has one.
pub struct SigV4Request<'a> {
    pub method: &'a str,
    /// The encoded path, e.g. `/model/anthropic.claude-v1%3A0/converse`.
    pub path: &'a str,
    /// The encoded query string without `?` (empty for every Bedrock call).
    pub query: &'a str,
    pub host: &'a str,
    pub content_type: Option<&'a str>,
    /// The exact bytes the request sends.
    pub body: &'a [u8],
    pub access_key_id: &'a str,
    pub secret_access_key: &'a str,
    /// Sent only for temporary credentials.
    pub session_token: Option<&'a str>,
    pub region: &'a str,
    pub service: &'a str,
    pub now: DateTime<Utc>,
}

/// What to add to the request: `x-amz-date` (always), `Authorization` (always) and
/// `x-amz-security-token` (when a session token was supplied).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigV4Signature {
    pub amz_date: String,
    pub authorization: String,
    /// `host;x-amz-date`, for tests and diagnostics.
    pub signed_headers: String,
}

/// Lowercase hex SHA-256, as SigV4 spells a payload hash.
pub fn sha256_hex(data: &[u8]) -> String {
    hex(digest::digest(&digest::SHA256, data).as_ref())
}

fn hmack(key: &[u8], data: &[u8]) -> Vec<u8> {
    let key = hmac::Key::new(hmac::HMAC_SHA256, key);
    hmac::sign(&key, data).as_ref().to_vec()
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// SigV4's canonical URI. With `double` (Bedrock), the sent path is encoded a second time, so
/// `%` becomes `%25`; without it (S3), the already-encoded path is used as sent. Unreserved
/// bytes and `/` pass through; anything else becomes uppercase `%XX`.
fn canonical_uri(path: &str, double: bool) -> String {
    if !double {
        return path.to_string();
    }
    let mut out = String::with_capacity(path.len());
    for b in path.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Per-service signing differences. The default is Bedrock's: double-encoded path, payload hash
/// computed from `body`, no extra signed headers.
pub struct SignOptions<'a> {
    /// Extra headers to sign, lowercase names (e.g. `x-amz-content-sha256`, `range`).
    pub extra_headers: &'a [(&'a str, &'a str)],
    /// The payload hash to sign instead of hashing `body` (S3 sends it as
    /// `x-amz-content-sha256`).
    pub payload_hash: Option<&'a str>,
    pub double_encode_path: bool,
}

impl Default for SignOptions<'_> {
    fn default() -> Self {
        Self {
            extra_headers: &[],
            payload_hash: None,
            double_encode_path: true,
        }
    }
}

fn signed_header_list(req: &SigV4Request, opts: &SignOptions) -> Vec<(String, String)> {
    let mut headers: Vec<(String, String)> = vec![
        ("host".into(), req.host.trim().to_string()),
        ("x-amz-date".into(), req.now.format("%Y%m%dT%H%M%SZ").to_string()),
    ];
    if let Some(content_type) = req.content_type {
        headers.push(("content-type".into(), content_type.trim().to_string()));
    }
    if let Some(token) = req.session_token {
        headers.push(("x-amz-security-token".into(), token.to_string()));
    }
    for (name, value) in opts.extra_headers {
        headers.push((name.to_ascii_lowercase(), value.trim().to_string()));
    }
    headers.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    headers
}

/// The canonical query: parameters sorted by name then value. The caller supplies them already
/// percent-encoded (Bedrock sends none), so sorting is all that is added.
fn canonical_query(query: &str) -> String {
    let mut pairs: Vec<&str> = query.split('&').filter(|p| !p.is_empty()).collect();
    pairs.sort_unstable();
    pairs.join("&")
}

impl SigV4Request<'_> {
    /// The canonical request, exposed for the tests and for tracing a signature mismatch.
    pub fn canonical_request(&self) -> String {
        self.canonical_request_with(&SignOptions::default())
    }

    /// The canonical request under `opts`.
    pub fn canonical_request_with(&self, opts: &SignOptions) -> String {
        let headers = signed_header_list(self, opts);
        let signed: Vec<&str> = headers.iter().map(|(name, _)| name.as_str()).collect();

        let mut canonical_headers = String::new();
        for (name, value) in &headers {
            canonical_headers.push_str(name);
            canonical_headers.push(':');
            canonical_headers.push_str(value);
            canonical_headers.push('\n');
        }

        let payload_hash = match opts.payload_hash {
            Some(hash) => hash.to_string(),
            None => sha256_hex(self.body),
        };
        format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            self.method,
            canonical_uri(self.path, opts.double_encode_path),
            canonical_query(self.query),
            canonical_headers,
            signed.join(";"),
            payload_hash,
        )
    }
}

/// Sign one request (contract `bedrock-wire.md` §2).
pub fn sign(req: &SigV4Request) -> SigV4Signature {
    sign_with(req, &SignOptions::default())
}

/// Sign one request with per-service options (S3: single-encoded path, signed
/// `x-amz-content-sha256`).
pub fn sign_with(req: &SigV4Request, opts: &SignOptions) -> SigV4Signature {
    let amz_date = req.now.format("%Y%m%dT%H%M%SZ").to_string();
    let date = &amz_date[..8];
    let scope = format!("{date}/{}/{}/aws4_request", req.region, req.service);

    let signed_headers = signed_header_list(req, opts)
        .into_iter()
        .map(|(name, _)| name)
        .collect::<Vec<_>>()
        .join(";");

    let canonical = req.canonical_request_with(opts);
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
        sha256_hex(canonical.as_bytes())
    );

    let mut key = hmack(
        format!("AWS4{}", req.secret_access_key).as_bytes(),
        date.as_bytes(),
    );
    key = hmack(&key, req.region.as_bytes());
    key = hmack(&key, req.service.as_bytes());
    key = hmack(&key, b"aws4_request");
    let signature = hex(&hmack(&key, string_to_sign.as_bytes()));

    SigV4Signature {
        amz_date,
        authorization: format!(
            "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed_headers}, Signature={signature}",
            req.access_key_id
        ),
        signed_headers,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    /// The AWS SigV4 test suite's credentials and clock.
    const ACCESS_KEY: &str = "AKIDEXAMPLE";
    const SECRET_KEY: &str = "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY";

    fn at_2015() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2015, 8, 30, 12, 36, 0).unwrap()
    }

    fn aws_request<'a>(
        method: &'a str,
        path: &'a str,
        query: &'a str,
        body: &'a [u8],
    ) -> SigV4Request<'a> {
        SigV4Request {
            method,
            path,
            query,
            host: "example.amazonaws.com",
            content_type: None,
            body,
            access_key_id: ACCESS_KEY,
            secret_access_key: SECRET_KEY,
            session_token: None,
            region: "us-east-1",
            service: "service",
            now: at_2015(),
        }
    }

    /// The "get-vanilla" vector from the AWS SigV4 test suite (service name `service`,
    /// region `us-east-1`, the suite's clock and credentials).
    #[test]
    fn matches_the_aws_get_vanilla_vector() {
        let signed = sign(&aws_request("GET", "/", "", b""));
        assert_eq!(signed.amz_date, "20150830T123600Z");
        assert_eq!(signed.signed_headers, "host;x-amz-date");
        assert_eq!(
            signed.authorization,
            "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, \
             SignedHeaders=host;x-amz-date, \
             Signature=5fa00fa31553b73ebf1942676e86291e8372ff2a2260956d9b8aae1d763fbf31"
        );
    }

    #[test]
    fn matches_the_aws_post_vanilla_vector() {
        let signed = sign(&aws_request("POST", "/", "", b""));
        assert_eq!(
            signed.authorization.rsplit_once("Signature=").unwrap().1,
            "5da7c1a2acd57cee7505fc6676e4e544621c30862966e37dddb68e92efbe5d6b"
        );
    }

    #[test]
    fn matches_the_aws_post_vanilla_query_vector() {
        let signed = sign(&aws_request("POST", "/", "Param1=value1", b""));
        assert_eq!(
            signed.authorization.rsplit_once("Signature=").unwrap().1,
            "28038455d6de14eafc1f9222cf5aa6f1a96197d7deb8263271d420d138af7f11"
        );
    }

    /// The documented IAM `ListUsers` example: a query, a content type and the `iam` service.
    #[test]
    fn matches_the_aws_iam_list_users_example() {
        let signed = sign(&SigV4Request {
            method: "GET",
            path: "/",
            query: "Action=ListUsers&Version=2010-05-08",
            host: "iam.amazonaws.com",
            content_type: Some("application/x-www-form-urlencoded; charset=utf-8"),
            body: b"",
            access_key_id: ACCESS_KEY,
            secret_access_key: SECRET_KEY,
            session_token: None,
            region: "us-east-1",
            service: "iam",
            now: at_2015(),
        });
        assert_eq!(
            signed.authorization,
            "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/iam/aws4_request, \
             SignedHeaders=content-type;host;x-amz-date, \
             Signature=5d672d79c15b13162d9279b0855cfba6789a8edb4c82c400e06b5924a6f2b5d7"
        );
    }

    /// A Bedrock-shaped POST: the model id is double-encoded in the canonical URI and a session
    /// token joins the signed headers.
    #[test]
    fn a_bedrock_request_double_encodes_the_path_and_signs_the_session_token() {
        let body = br#"{"messages":[]}"#;
        let path = "/model/anthropic.claude-haiku-4-5-20251001-v1%3A0/converse";
        let plain = SigV4Request {
            method: "POST",
            path,
            query: "",
            host: "bedrock-runtime.us-east-1.amazonaws.com",
            content_type: Some("application/json"),
            body,
            access_key_id: ACCESS_KEY,
            secret_access_key: SECRET_KEY,
            session_token: None,
            region: "us-east-1",
            service: "bedrock",
            now: at_2015(),
        };
        assert!(
            plain.canonical_request().contains("%253A"),
            "the path is double-encoded in the canonical request: {}",
            plain.canonical_request()
        );
        let signed = sign(&plain);
        assert_eq!(
            signed.authorization.rsplit_once("Signature=").unwrap().1,
            "12022bdcc7aa58aa0318a8347415a126aa0b5354e1e8fe0aa1f9585bc3d47c9f"
        );

        let temporary = SigV4Request {
            session_token: Some("session-token"),
            ..plain
        };
        let signed = sign(&temporary);
        assert_eq!(
            signed.signed_headers,
            "content-type;host;x-amz-date;x-amz-security-token"
        );
        assert_eq!(
            signed.authorization.rsplit_once("Signature=").unwrap().1,
            "c14127a6fb56c7c9c651417318474d3480be65e83cd8459522443ae3b5dc3889"
        );
    }

    #[test]
    fn the_payload_hash_is_the_exact_body() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"{}"),
            "44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"
        );
    }

    #[test]
    fn query_parameters_are_sorted() {
        assert_eq!(canonical_query("b=2&a=1"), "a=1&b=2");
        assert_eq!(canonical_query(""), "");
    }

    #[test]
    fn canonical_uri_double_encodes_percent_and_keeps_slashes_and_unreserved() {
        assert_eq!(
            canonical_uri("/model/a.b_c-d~e%3A0/converse", true),
            "/model/a.b_c-d~e%253A0/converse"
        );
        assert_eq!(canonical_uri("/foundation-models", true), "/foundation-models");
    }

    /// The inference-profile ARN from the live 403 (bug 044). The wire path is encoded **once**
    /// (`aws_uri_encode`), the canonical URI twice per SigV4's `double_uri_encode`, and
    /// `content-type` appears on exactly one canonical-header line: a second copy is what AWS
    /// canonicalises as `application/json,application/json` and rejects as "signature does not
    /// match". Either half of this test fails if another layer encodes or dresses the request.
    #[test]
    fn an_inference_profile_arn_is_double_encoded_once_with_one_content_type_line() {
        let arn = "arn:aws:bedrock:ap-southeast-1:392852851960:application-inference-profile/q549rhlez69w";
        let wire = crate::llm::bedrock::model_path(arn, "converse");
        assert_eq!(
            wire,
            "/model/arn%3Aaws%3Abedrock%3Aap-southeast-1%3A392852851960%3Aapplication-inference-profile%2Fq549rhlez69w/converse"
        );
        assert!(
            !wire.contains("%253A") && !wire.contains("%252F"),
            "the wire carries one encoding, not two: {wire}"
        );

        let request = SigV4Request {
            method: "POST",
            path: &wire,
            query: "",
            host: "bedrock-runtime.ap-southeast-1.amazonaws.com",
            content_type: Some("application/json"),
            body: br#"{"messages":[]}"#,
            access_key_id: ACCESS_KEY,
            secret_access_key: SECRET_KEY,
            session_token: Some("session-token"),
            region: "ap-southeast-1",
            service: "bedrock",
            now: at_2015(),
        };
        let canonical = request.canonical_request();
        let lines: Vec<&str> = canonical.lines().collect();
        assert_eq!(
            lines[1],
            "/model/arn%253Aaws%253Abedrock%253Aap-southeast-1%253A392852851960%253Aapplication-inference-profile%252Fq549rhlez69w/converse",
            "the canonical URI double-encodes the wire path exactly once: {canonical}"
        );
        assert!(
            !canonical.contains("%2525"),
            "the path is encoded twice, never three times: {canonical}"
        );
        assert_eq!(
            canonical.matches("content-type:").count(),
            1,
            "one content-type canonical header, never two: {canonical}"
        );
        assert_eq!(
            canonical.matches("\ncontent-type:application/json\n").count(),
            1,
            "the canonical value is the single bare media type: {canonical}"
        );
    }
}
