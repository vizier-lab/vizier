use axum::Router;

use crate::state::HTTPState;

pub mod v1;

pub fn api(state: HTTPState) -> Router<HTTPState> {
    Router::new().nest("/v1", v1::v1(state))
}

