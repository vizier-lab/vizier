use axum::{
    extract::{Extension, Json, Path, State},
    http::{StatusCode, header::CONTENT_TYPE},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::{
    api::v1::integrations::svc,
    auth::Principal,
    models::response::{APIResponse, api_response, err_response},
    state::HTTPState,
};
use vizier_core::service::files as service;

use super::range::blob_response;

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

    let Ok(blob) = service::locate(&svc(&state), &file_id).await else {
        return not_found();
    };

    let mime_type = mime_guess::from_path(blob.meta.key.file_name())
        .first_or_octet_stream()
        .to_string();
    let range = headers
        .get(axum::http::header::RANGE)
        .and_then(|v| v.to_str().ok());

    blob_response(&blob, range, &[(CONTENT_TYPE, mime_type)])
        .await
        .unwrap_or_else(not_found)
}
