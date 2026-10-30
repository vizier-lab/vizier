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
        v("GET", "/catalogue", None),
        v("GET", "/catalogue/providers", None),
        v("GET", "/catalogue/providers/nope/models", None),
        v("GET", "/catalogue/providers/nope/models/m", None),
        v("GET", "/catalogue/media-models", None),
        v("GET", "/embedding-models/local", None),
    ]
}
