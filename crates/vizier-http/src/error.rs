//! `ServiceError` as the API's JSON envelope.

use axum::{Json, http::StatusCode, response::{IntoResponse, Response}};
use vizier_core::service::ServiceError;

pub struct ApiError(pub ServiceError);

impl From<ServiceError> for ApiError {
    fn from(e: ServiceError) -> Self {
        Self(e)
    }
}

pub fn envelope(e: &ServiceError) -> (u16, serde_json::Value) {
    let mut body = serde_json::json!({
        "status": e.status(),
        "message": e.message(),
        "data": null,
    });
    if let Some(code) = e.code() {
        body["code"] = code.into();
    }
    if let ServiceError::Conflict { data: Some(d), .. } = e {
        body["data"] = d.clone();
    }
    (e.status(), body)
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, body) = envelope(&self.0);
        (StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR), Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_kind_has_status_and_body() {
        let (s, b) = envelope(&ServiceError::NotFound("nope".into()));
        assert_eq!((s, b["message"].as_str()), (404, Some("nope")));
        let (s, b) = envelope(&ServiceError::Gone { code: "attach_removed".into(), message: "m".into() });
        assert_eq!((s, b["code"].as_str()), (410, Some("attach_removed")));
        let (s, b) = envelope(&ServiceError::Conflict { message: "m".into(), code: Some("c".into()), data: Some(serde_json::json!({"x":1})) });
        assert_eq!((s, b["data"]["x"].as_i64()), (409, Some(1)));
        assert_eq!(envelope(&ServiceError::Forbidden("f".into())).0, 403);
    }
}
