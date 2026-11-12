use axum::{
    extract::Path,
    http::{HeaderMap, HeaderValue},
    response::{IntoResponse, Response},
};
use reqwest::{StatusCode, header};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "$OUT_DIR/webui"]
struct WebUI;

/// Policy for the HTML served under `/share/*` (specs/056-shared-chat R5/R8, fixed by spike T004).
/// `'unsafe-inline'` is needed by the SPA's boot script; `'unsafe-eval'` and `worker-src blob:`
/// by the generative-UI logic runner, whose sandboxed `srcdoc` frame inherits this policy
/// (measured in Chromium, Firefox and WebKit: without them the worker's `Function` call fails).
const SHARE_CSP: &str = "default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval'; \
style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; \
media-src 'self' blob:; connect-src 'self'; worker-src blob:; object-src 'none'; \
base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

/// The cache policy for a static file: the shell must be revalidated, hashed assets never.
///
/// The worker at the origin root is never stored: a CDN in front may keep its own copy and hand
/// the browser a stale one, and a stale worker means an old app. The built assets are
/// content-hashed, so they can be kept for a year.
fn cache_control(path: &str) -> &'static str {
    let file = path.rsplit('/').next().unwrap_or(path);
    if file == "service-worker.js" {
        "no-store"
    } else if file == "index.html" || file == "manifest.webmanifest" {
        "no-cache"
    } else if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "public, max-age=3600"
    }
}

/// The extensions a static file carries. A path that ends in anything else is a route, even when
/// its last segment has a dot: a memory document is named after its file, minus the `.md`.
const ASSET_EXTENSIONS: &[&str] = &[
    "js", "mjs", "css", "map", "json", "webmanifest", "txt", "xml", "svg", "png", "jpg", "jpeg",
    "gif", "webp", "avif", "ico", "woff", "woff2", "ttf", "otf", "eot", "mp3", "ogg", "wav",
    "m4a", "webm", "wasm", "pdf", "zip",
];

/// Whether a path that is not in the embed is an asset request (a miss is a 404) or a
/// client-side route (a miss is the app shell, so the SPA router can take over).
fn is_asset_path(path: &str) -> bool {
    let file = path.rsplit('/').next().unwrap_or(path);
    match file.rsplit_once('.') {
        Some((_, ext)) => ASSET_EXTENSIONS.contains(&ext),
        None => false,
    }
}

/// True for `share` and `share/...` (the path as the wildcard route captures it, no leading `/`).
fn is_share_path(path: &str) -> bool {
    let path = path.trim_start_matches('/');
    path == "share" || path.starts_with("share/")
}

fn html_response(data: impl Into<axum::body::Body>, share: bool) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "text/html".parse().unwrap());
    headers.insert(header::CACHE_CONTROL, cache_control("index.html").parse().unwrap());
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
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, mime.as_ref().parse().unwrap());
            headers.insert(header::CACHE_CONTROL, cache_control(&path).parse().unwrap());

            (StatusCode::OK, headers, content.data).into_response()
        }
        // A missing file is a 404; a missing route is the app shell; `/share/*` gets the stricter headers.
        None if is_asset_path(&path) => (StatusCode::NOT_FOUND, "Not Found").into_response(),
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
        assert_eq!(r.headers().get(header::CACHE_CONTROL).unwrap(), "no-cache");
    }

    #[test]
    fn other_html_is_unchanged() {
        let r = html_response("<html></html>", false);
        assert!(r.headers().get(header::CONTENT_SECURITY_POLICY).is_none());
        assert!(r.headers().get(header::REFERRER_POLICY).is_none());
        assert_eq!(r.headers().get(header::CONTENT_TYPE).unwrap(), "text/html");
        assert_eq!(r.headers().get(header::CACHE_CONTROL).unwrap(), "no-cache");
    }

    #[test]
    fn an_asset_miss_is_a_404_and_a_route_miss_is_the_shell() {
        assert!(is_asset_path("service-worker.js"));
        assert!(is_asset_path("assets/root-Bp7Ii7RU.css"));
        assert!(is_asset_path("manifest.webmanifest"));
        assert!(!is_asset_path("settings/roles"));
        assert!(!is_asset_path("login"));
        assert!(!is_asset_path("my-agent/chat/topic-1"));
    }

    #[test]
    fn a_route_with_a_dotted_name_is_still_a_route() {
        // A memory document is named after its file, minus the .md: `notes.v1.md` is `notes.v1`.
        assert!(!is_asset_path("my-agent/memory/core/notes.v1"));
        assert!(!is_asset_path("my-agent/memory/core/notes"));
        assert!(is_asset_path("favicon.svg"));
    }

    #[test]
    fn the_shell_is_revalidated_and_hashed_assets_are_not() {
        assert_eq!(cache_control("index.html"), "no-cache");
        assert_eq!(cache_control("service-worker.js"), "no-store");
        assert_eq!(cache_control("manifest.webmanifest"), "no-cache");
        assert_eq!(
            cache_control("assets/root-Bp7Ii7RU.css"),
            "public, max-age=31536000, immutable"
        );
        assert_eq!(cache_control("favicon.svg"), "public, max-age=3600");
    }
}
