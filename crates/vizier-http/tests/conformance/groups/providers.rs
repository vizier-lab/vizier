use crate::common::{Vector, v};
use serde_json::json;

/// Providers, custom providers, usage, the catalogue and embedding models. Nothing here
/// reaches the network: refusals, reads of the bundled snapshot, unknown ids.
pub fn vectors() -> Vec<Vector> {
    vec![
        v("GET", "/providers", None),
        v("GET", "/providers/custom", None),
        v("GET", "/providers/nope", None),
        v("PUT", "/providers/nope", Some(json!({}))),
        v("DELETE", "/providers/nope", None),
        v("POST", "/providers/nope/test", None),
        v("POST", "/providers/custom", Some(json!({ "name": "" }))),
        v("PUT", "/providers/custom/nope", Some(json!({ "name": "n" }))),
        v("DELETE", "/providers/custom/nope", None),
        v("POST", "/providers/custom/nope/fetch-models", None),
        // Inference-profile entries (contract `provider-credentials-api.md` §4) and the IAM Identity
        // Center sign-in (§3), against an id that is not the Bedrock SSO connection: every one is
        // refused by the id check before a row is read or a request leaves the process, so this
        // file's "nothing here reaches the network" still holds. The ARN is percent-encoded, as the
        // route takes it in one path segment, and equals the one in the bodies.
        v("GET", "/providers/nope/profiles", None),
        v(
            "POST",
            "/providers/nope/profiles",
            Some(json!({ "arn": "arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/abc123" })),
        ),
        v(
            "PUT",
            "/providers/nope/profiles/arn%3Aaws%3Abedrock%3Aus-east-1%3A123456789012%3Aapplication-inference-profile%2Fabc123",
            Some(json!({ "arn": "arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/abc123", "name": "renamed" })),
        ),
        v(
            "DELETE",
            "/providers/nope/profiles/arn%3Aaws%3Abedrock%3Aus-east-1%3A123456789012%3Aapplication-inference-profile%2Fabc123",
            None,
        ),
        v("POST", "/providers/nope/sso/sign-in", None),
        v("GET", "/providers/nope/sso/accounts", None),
        v("GET", "/providers/nope/sso/accounts/123456789012/roles", None),
        v("POST", "/providers/nope/sso/sign-out", None),
        v("GET", "/catalogue", None),
        v("GET", "/catalogue/providers", None),
        v("GET", "/catalogue/providers/nope/models", None),
        v("GET", "/catalogue/providers/nope/models/m", None),
        v("GET", "/catalogue/media-models", None),
        v("GET", "/embedding-models/local", None),
    ]
}
