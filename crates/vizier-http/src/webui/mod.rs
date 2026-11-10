use axum::{extract::Path, http::HeaderMap, response::IntoResponse};
use reqwest::{StatusCode, header};

#[derive(rust_embed::RustEmbed)]
#[folder = "$OUT_DIR/webui"]
struct WebUI;

/// The cache policy for a static file: the shell must be revalidated, hashed assets never.
///
/// The worker at the origin root is revalidated, or the browser pins a stale one and the app
/// never updates. The built assets are content-hashed, so they can be kept for a year.
fn cache_control(path: &str) -> &'static str {
    let file = path.rsplit('/').next().unwrap_or(path);
    if file == "index.html" || file == "service-worker.js" || file == "manifest.webmanifest" {
        "no-cache"
    } else if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "public, max-age=3600"
    }
}

/// Whether a path that is not in the embed is an asset request (a miss is a 404) or a
/// client-side route (a miss is the app shell, so the SPA router can take over).
fn is_asset_path(path: &str) -> bool {
    path.rsplit('/').next().unwrap_or(path).contains('.')
}

pub async fn index() -> impl IntoResponse {
    match WebUI::get("index.html") {
        Some(content) => {
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, "text/html".parse().unwrap());
            headers.insert(header::CACHE_CONTROL, cache_control("index.html").parse().unwrap());

            (StatusCode::OK, headers, content.data).into_response()
        }
        None => (StatusCode::NOT_FOUND, "Index Not Found").into_response(),
    }
}

pub async fn assets(Path(path): Path<String>) -> impl IntoResponse {
    match WebUI::get(&path) {
        Some(content) => {
            let mime_type = mime_guess::from_path(&path).first_or_octet_stream();

            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, mime_type.as_ref().parse().unwrap());
            headers.insert(header::CACHE_CONTROL, cache_control(&path).parse().unwrap());

            (StatusCode::OK, headers, content.data).into_response()
        }
        // A missing file is a 404; a missing route is the app shell.
        None if is_asset_path(&path) => (StatusCode::NOT_FOUND, "Not Found").into_response(),
        None => index().await.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn the_shell_is_revalidated_and_hashed_assets_are_not() {
        assert_eq!(cache_control("index.html"), "no-cache");
        assert_eq!(cache_control("service-worker.js"), "no-cache");
        assert_eq!(cache_control("manifest.webmanifest"), "no-cache");
        assert_eq!(
            cache_control("assets/root-Bp7Ii7RU.css"),
            "public, max-age=31536000, immutable"
        );
        assert_eq!(cache_control("favicon.svg"), "public, max-age=3600");
    }
}
