use std::path::PathBuf;

use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header::{CONTENT_DISPOSITION, CONTENT_TYPE}},
    response::{IntoResponse, Response},
    Router,
    routing::post,
};
use axum_extra::extract::Multipart;
use serde::Serialize;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use utoipa::ToSchema;

use crate::channels::http::{
    models::response::{api_response, err_response, APIResponse},
    state::HTTPState,
};

const UPLOADS_DIR: &str = "uploads";

#[derive(Debug, Serialize, Clone, ToSchema)]
pub struct UploadResponse {
    pub file_id: String,
    pub filename: String,
    pub url: String,
}

pub fn files() -> axum::Router<HTTPState> {
    axum::Router::new()
        .route("/upload", post(upload_file))
        .route("/{file_id}", axum::routing::get(download_file))
}

#[utoipa::path(
    post,
    path = "/files/upload",
    request_body(content_type = "multipart/form-data"),
    responses(
        (status = 201, description = "File uploaded successfully", body = APIResponse<UploadResponse>),
        (status = 401, description = "Unauthorized", body = APIResponse<String>),
        (status = 500, description = "Internal server error", body = APIResponse<String>)
    )
)]
pub async fn upload_file(
    State(state): State<HTTPState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let workspace = &state.config.workspace;
    let uploads_dir = PathBuf::from(workspace).join(UPLOADS_DIR);

    if let Err(e) = fs::create_dir_all(&uploads_dir).await {
        log::error!("Failed to create uploads directory: {}", e);
        return err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to create upload directory".to_string(),
        );
    }

    let mut file_id = String::new();
    let mut original_filename = String::new();
    let mut file_data: Vec<u8> = Vec::new();

    loop {
        let field = match multipart.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => {
                log::error!("Error reading multipart field: {}", e);
                continue;
            }
        };

        let name = field.name().unwrap_or("file").to_string();

        if name == "file" {
            file_id = nanoid::nanoid!(10);
            original_filename = field.file_name().unwrap_or("unnamed").to_string();

            let bytes_result = field.bytes().await;
            match bytes_result {
                Ok(b) => {
                    file_data = b.to_vec();
                }
                Err(e) => {
                    log::error!("Error reading file bytes: {}", e);
                }
            }
        }
    }

    if file_data.is_empty() {
        return err_response(
            StatusCode::BAD_REQUEST,
            "No file provided".to_string(),
        );
    }

    let file_dir = uploads_dir.join(&file_id);
    if let Err(e) = fs::create_dir_all(&file_dir).await {
        log::error!("Failed to create file directory: {}", e);
        return err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to create file directory".to_string(),
        );
    }

    let file_path = file_dir.join(&original_filename);
    let mut file = match fs::File::create(&file_path).await {
        Ok(f) => f,
        Err(e) => {
            log::error!("Failed to create file: {}", e);
            return err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to save file".to_string(),
            );
        }
    };

    if let Err(e) = file.write_all(&file_data).await {
        log::error!("Failed to write file data: {}", e);
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
            let (status, json) = err_response::<String>(
                StatusCode::NOT_FOUND,
                "File not found".to_string(),
            );
            return (status, json).into_response();
        }
    };

    let entry = match entries.next_entry().await {
        Ok(Some(e)) => e,
        Ok(None) => {
            let (status, json) = err_response::<String>(
                StatusCode::NOT_FOUND,
                "File not found".to_string(),
            );
            return (status, json).into_response();
        }
        Err(_) => {
            let (status, json) = err_response::<String>(
                StatusCode::NOT_FOUND,
                "File not found".to_string(),
            );
            return (status, json).into_response();
        }
    };

    let file_path = entry.path();
    let filename = entry.file_name().to_string_lossy().to_string();

    let data = match fs::read(&file_path).await {
        Ok(d) => d,
        Err(_) => {
            let (status, json) = err_response::<String>(
                StatusCode::NOT_FOUND,
                "File not found".to_string(),
            );
            return (status, json).into_response();
        }
    };

    let mime_type = mime_guess::from_path(&file_path)
        .first_or_octet_stream()
        .to_string();

    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        mime_type.parse().unwrap(),
    );
    headers.insert(
        CONTENT_DISPOSITION,
        format!("attachment; filename=\"{}\"", filename)
            .parse()
            .unwrap(),
    );

    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, mime_type)
        .header(CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"", filename))
        .body(Body::from(data))
        .unwrap()
        .into_response()
}