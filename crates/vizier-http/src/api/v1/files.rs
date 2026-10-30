use axum::{
    body::Body,
    extract::{Extension, Json, Path, State},
    http::{StatusCode, header::CONTENT_TYPE},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tokio::fs;
use utoipa::ToSchema;

use crate::{
    api::v1::integrations::svc,
    auth::Principal,
    models::response::{APIResponse, api_response, err_response},
    state::HTTPState,
};
use vizier_core::service::files as service;

use super::range::{RangeRequest, parse_range};

pub use service::UploadResponse;

#[derive(Debug, Deserialize, Clone, ToSchema)]
pub struct UploadRequest {
    /// Base64-encoded file content
    pub file: String,
    /// Original filename
    pub filename: String,
}

#[utoipa::path(
    post,
    path = "/files/upload",
    request_body = UploadRequest,
    responses(
        (status = 201, description = "File uploaded successfully", body = APIResponse<UploadResponse>),
        (status = 401, description = "Unauthorized", body = APIResponse<String>),
        (status = 400, description = "Bad request", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
pub async fn upload_file(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Json(body): Json<UploadRequest>,
) -> impl IntoResponse {
    match service::upload(&svc(&state), &user, &body.file, body.filename).await {
        Ok(uploaded) => api_response(StatusCode::CREATED, uploaded),
        Err(e) => err_response(
            StatusCode::from_u16(e.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            e.message().to_string(),
        ),
    }
}

#[utoipa::path(
    get,
    path = "/files/{file_id}",
    params(
        ("file_id" = String, Path, description = "File ID to download")
    ),
    responses(
        (status = 200, description = "File downloaded"),
        (status = 404, description = "File not found", body = APIResponse<String>)
    )
)]
pub async fn download_file(
    State(state): State<HTTPState>,
    Path(file_id): Path<String>,
    headers: axum::http::HeaderMap,
) -> Response {
    let not_found = || {
        let (status, json) =
            err_response::<String>(StatusCode::NOT_FOUND, "File not found".to_string());
        (status, json).into_response()
    };

    let Ok(file_path) = service::locate(&svc(&state), &file_id).await else {
        return not_found();
    };

    let mime_type = mime_guess::from_path(&file_path)
        .first_or_octet_stream()
        .to_string();

    let range = headers
        .get(axum::http::header::RANGE)
        .and_then(|v| v.to_str().ok());

    let Some(range) = range else {
        return match fs::read(&file_path).await {
            Ok(data) => Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_TYPE, mime_type)
                .header(axum::http::header::ACCEPT_RANGES, "bytes")
                .body(Body::from(data))
                .unwrap()
                .into_response(),
            Err(_) => not_found(),
        };
    };

    let Ok(meta) = fs::metadata(&file_path).await else {
        return not_found();
    };
    let len = meta.len();
    match parse_range(range, len) {
        RangeRequest::Ignore => match fs::read(&file_path).await {
            Ok(data) => Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_TYPE, mime_type)
                .header(axum::http::header::ACCEPT_RANGES, "bytes")
                .body(Body::from(data))
                .unwrap()
                .into_response(),
            Err(_) => not_found(),
        },
        RangeRequest::Unsatisfiable => Response::builder()
            .status(StatusCode::RANGE_NOT_SATISFIABLE)
            .header(axum::http::header::CONTENT_RANGE, format!("bytes */{len}"))
            .body(Body::empty())
            .unwrap()
            .into_response(),
        RangeRequest::Bytes { start, end } => {
            use tokio::io::{AsyncReadExt, AsyncSeekExt};
            let Ok(mut file) = fs::File::open(&file_path).await else {
                return not_found();
            };
            let mut buf = vec![0u8; (end - start + 1) as usize];
            if file.seek(std::io::SeekFrom::Start(start)).await.is_err()
                || file.read_exact(&mut buf).await.is_err()
            {
                return not_found();
            }
            Response::builder()
                .status(StatusCode::PARTIAL_CONTENT)
                .header(CONTENT_TYPE, mime_type)
                .header(axum::http::header::ACCEPT_RANGES, "bytes")
                .header(
                    axum::http::header::CONTENT_RANGE,
                    format!("bytes {start}-{end}/{len}"),
                )
                .body(Body::from(buf))
                .unwrap()
                .into_response()
        }
    }
}
