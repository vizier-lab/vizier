use axum::Json;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

pub type Response<T> = (StatusCode, Json<APIResponse<T>>);

#[derive(Debug, Deserialize, Serialize, Clone, utoipa::ToSchema)]
pub struct APIResponse<T: Serialize + Clone> {
    pub status: u16,
    pub message: Option<String>,
    pub data: Option<T>,
}

impl<T: Serialize + Clone> APIResponse<T> {
    fn new(status: StatusCode, response: Option<T>) -> Self {
        Self {
            status: status.as_u16(),
            data: response,
            message: None,
        }
    }

    #[allow(unused)]
    fn err(status: StatusCode, err: String) -> Self {
        Self {
            status: status.as_u16(),
            data: None,
            message: Some(err),
        }
    }
}

pub fn api_response<T: Serialize + Clone>(
    status: StatusCode,
    response: T,
) -> (StatusCode, Json<APIResponse<T>>) {
    (status, Json(APIResponse::new(status, Some(response))))
}

#[allow(unused)]
pub fn err_response<T: Serialize + Clone>(
    status: StatusCode,
    err: String,
) -> (StatusCode, Json<APIResponse<T>>) {
    (status, Json(APIResponse::err(status, err)))
}
