use axum::{extract::Path, http::HeaderMap, response::IntoResponse};
use reqwest::{StatusCode, header};

#[derive(rust_embed::RustEmbed)]
#[folder = "$OUT_DIR/webui"]
struct WebUI;

pub async fn index() -> impl IntoResponse {
    match WebUI::get("index.html") {
        Some(content) => {
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, "text/html".parse().unwrap());

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

            (StatusCode::OK, headers, content.data).into_response()
        }
        None => index().await.into_response(),
    }
}
