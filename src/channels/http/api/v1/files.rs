use std::path::PathBuf;

use axum::{
    body::Body,
    extract::{Json, Path, State},
    http::{StatusCode, header::CONTENT_TYPE},
    response::{IntoResponse, Response},
};
use base64::Engine;
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use utoipa::ToSchema;

use crate::channels::http::{
    models::response::{APIResponse, api_response, err_response},
    state::HTTPState,
};

const UPLOADS_DIR: &str = "uploads";

#[derive(Debug, Serialize, Clone, ToSchema)]
pub struct UploadResponse {
    pub file_id: String,
    pub filename: String,
    pub url: String,
}

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
    Json(body): Json<UploadRequest>,
) -> impl IntoResponse {
    let workspace = &state.config.workspace;
    let uploads_dir = PathBuf::from(workspace).join(UPLOADS_DIR);

    if let Err(e) = fs::create_dir_all(&uploads_dir).await {
        tracing::error!("Failed to create uploads directory: {}", e);
        return err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to create upload directory".to_string(),
        );
    }

    if body.file.is_empty() {
        return err_response(StatusCode::BAD_REQUEST, "No file provided".to_string());
    }

    let file_data = match base64::engine::general_purpose::STANDARD.decode(&body.file) {
        Ok(data) => data,
        Err(e) => {
            return err_response(
                StatusCode::BAD_REQUEST,
                format!("Invalid base64: {}", e),
            );
        }
    };

    if file_data.is_empty() {
        return err_response(StatusCode::BAD_REQUEST, "Empty file content".to_string());
    }

    let file_id = nanoid::nanoid!(10);
    let original_filename = body.filename;

    let file_dir = uploads_dir.join(&file_id);
    if let Err(e) = fs::create_dir_all(&file_dir).await {
        tracing::error!("Failed to create file directory: {}", e);
        return err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to create file directory".to_string(),
        );
    }

    let file_path = file_dir.join(&original_filename);
    let mut file = match fs::File::create(&file_path).await {
        Ok(f) => f,
        Err(e) => {
            tracing::error!("Failed to create file: {}", e);
            return err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to save file".to_string(),
            );
        }
    };

    if let Err(e) = file.write_all(&file_data).await {
        tracing::error!("Failed to write file data: {}", e);
        return err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to write file".to_string(),
        );
    }

    let url = format!("/api/v1/files/{}", file_id);

    api_response(
        StatusCode::CREATED,
        UploadResponse {
            file_id,
            filename: original_filename,
            url,
        },
    )
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
) -> Response {
    let workspace = &state.config.workspace;
    let uploads_dir = PathBuf::from(workspace).join(UPLOADS_DIR).join(&file_id);

    let mut entries = match fs::read_dir(&uploads_dir).await {
        Ok(entries) => entries,
        Err(_) => {
            let (status, json) =
                err_response::<String>(StatusCode::NOT_FOUND, "File not found".to_string());
            return (status, json).into_response();
        }
    };

    let entry = match entries.next_entry().await {
        Ok(Some(e)) => e,
        Ok(None) => {
            let (status, json) =
                err_response::<String>(StatusCode::NOT_FOUND, "File not found".to_string());
            return (status, json).into_response();
        }
        Err(_) => {
            let (status, json) =
                err_response::<String>(StatusCode::NOT_FOUND, "File not found".to_string());
            return (status, json).into_response();
        }
    };

    let file_path = entry.path();
    let _filename = entry.file_name().to_string_lossy().to_string();

    let data = match fs::read(&file_path).await {
        Ok(d) => d,
        Err(_) => {
            let (status, json) =
                err_response::<String>(StatusCode::NOT_FOUND, "File not found".to_string());
            return (status, json).into_response();
        }
    };

    let mime_type = mime_guess::from_path(&file_path)
        .first_or_octet_stream()
        .to_string();

    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, mime_type)
        .body(Body::from(data))
        .unwrap()
        .into_response()
}
