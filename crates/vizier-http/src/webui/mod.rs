use axum::{extract::Path, http::HeaderMap, response::IntoResponse};
use reqwest::{StatusCode, header};

#[derive(rust_embed::RustEmbed)]
#[folder = "$OUT_DIR/webui"]
struct WebUI;

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
