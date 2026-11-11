use axum::{
    extract::Path,
    http::{HeaderMap, HeaderValue},
    response::{IntoResponse, Response},
};
use reqwest::{StatusCode, header};

#[derive(rust_embed::RustEmbed)]
#[folder = "$OUT_DIR/webui"]
struct WebUI;

/// Policy for the HTML served under `/share/*` (specs/055-shared-chat R5/R8, fixed by spike T004).
/// `'unsafe-inline'` is needed by the SPA's boot script; `'unsafe-eval'` and `worker-src blob:`
/// by the generative-UI logic runner, whose sandboxed `srcdoc` frame inherits this policy
/// (measured in Chromium, Firefox and WebKit: without them the worker's `Function` call fails).
const SHARE_CSP: &str = "default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval'; \
style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; \
media-src 'self' blob:; connect-src 'self'; worker-src blob:; object-src 'none'; \
base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

/// True for `share` and `share/...` (the path as the wildcard route captures it, no leading `/`).
fn is_share_path(path: &str) -> bool {
    let path = path.trim_start_matches('/');
    path == "share" || path.starts_with("share/")
}

fn html_response(data: impl Into<axum::body::Body>, share: bool) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "text/html".parse().unwrap());
    if share {
        headers.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(SHARE_CSP));
        headers.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    }
    (StatusCode::OK, headers, data.into()).into_response()
}

fn index_for(share: bool) -> Response {
    match WebUI::get("index.html") {
        Some(content) => html_response(content.data.into_owned(), share),
        None => (StatusCode::NOT_FOUND, "Index Not Found").into_response(),
    }
}

pub async fn index() -> impl IntoResponse {
    index_for(false)
}

pub async fn assets(Path(path): Path<String>) -> impl IntoResponse {
    match WebUI::get(&path) {
        Some(content) => {
            let mime_type = mime_guess::from_path(&path).first_or_octet_stream();

            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, mime_type.as_ref().parse().unwrap());

            (StatusCode::OK, headers, content.data).into_response()
        }
        // An unknown path is a client-side route; `/share/*` gets the stricter headers.
        None => index_for(is_share_path(&path)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn share_paths_are_recognised() {
        assert!(is_share_path("share/abc"));
        assert!(is_share_path("/share/abc"));
        assert!(is_share_path("share"));
        assert!(!is_share_path("shared/abc"));
        assert!(!is_share_path("sharer"));
        assert!(!is_share_path("settings/share"));
        assert!(!is_share_path(""));
    }

    #[test]
    fn share_html_carries_policy_headers() {
        let r = html_response("<html></html>", true);
        let csp = r.headers().get(header::CONTENT_SECURITY_POLICY).unwrap().to_str().unwrap();
        assert!(csp.contains("frame-ancestors 'none'"));
        assert!(csp.contains("worker-src blob:"));
        assert!(csp.contains("object-src 'none'"));
        assert_eq!(r.headers().get(header::REFERRER_POLICY).unwrap(), "no-referrer");
        assert_eq!(r.headers().get(header::CONTENT_TYPE).unwrap(), "text/html");
    }

    #[test]
    fn other_html_is_unchanged() {
        let r = html_response("<html></html>", false);
        assert!(r.headers().get(header::CONTENT_SECURITY_POLICY).is_none());
        assert!(r.headers().get(header::REFERRER_POLICY).is_none());
        assert_eq!(r.headers().get(header::CONTENT_TYPE).unwrap(), "text/html");
    }
}
