use axum::Router;
use reqwest::StatusCode;

use crate::channels::http::models::{self, response::api_response};

pub mod memory;
pub mod session;

pub async fn ping() -> models::response::Response<String> {
    api_response(StatusCode::OK, "pong".into())
}
