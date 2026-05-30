use axum::{
    Router,
    extract::{Path, State},
    routing::get,
};
use reqwest::StatusCode;
use serde::Deserialize;
use std::collections::HashMap;

use crate::{
    channels::http::{
        models::{
            self,
            response::{api_response, err_response, APIResponse},
        },
        state::HTTPState,
    },
    config::{shell::ShellConfig, tools::mcp::McpClientConfig},
    schema::{GlobalCommand, GlobalCommandResult, GlobalConfigEntry, GlobalConfigValue},
    storage::global_config::GlobalConfigStorage,
};

pub fn global_config() -> Router<HTTPState> {
    Router::new()
        .route("/", get(list_global_configs))
        .route(
            "/{key}",
            get(get_global_config)
                .put(upsert_global_config)
                .delete(delete_global_config),
        )
}

#[utoipa::path(
    get,
    path = "/global-config",
    responses(
        (status = 200, description = "List of global configs", body = APIResponse<Vec<GlobalConfigEntry>>)
    )
)]
async fn list_global_configs(
    State(state): State<HTTPState>,
) -> models::response::Response<Vec<GlobalConfigEntry>> {
    match state.storage.list_global_configs().await {
        Ok(entries) => api_response(StatusCode::OK, entries),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string().into()),
    }
}

#[utoipa::path(
    get,
    path = "/global-config/{key}",
    params(
        ("key" = String, Path, description = "Config key")
    ),
    responses(
        (status = 200, description = "Global config entry", body = APIResponse<GlobalConfigEntry>),
        (status = 404, description = "Config not found", body = APIResponse<String>)
    )
)]
async fn get_global_config(
    Path(key): Path<String>,
    State(state): State<HTTPState>,
) -> models::response::Response<GlobalConfigEntry> {
    match state.storage.get_global_config(&key).await {
        Ok(Some(entry)) => api_response(StatusCode::OK, entry),
        Ok(None) => err_response(StatusCode::NOT_FOUND, "config not found".into()),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string().into()),
    }
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(tag = "type", content = "data")]
pub enum UpsertGlobalConfigRequest {
    McpServers(HashMap<String, McpClientConfig>),
    Shell(ShellConfig),
}

#[utoipa::path(
    put,
    path = "/global-config/{key}",
    params(
        ("key" = String, Path, description = "Config key")
    ),
    request_body = UpsertGlobalConfigRequest,
    responses(
        (status = 200, description = "Global config upserted", body = APIResponse<GlobalConfigEntry>),
        (status = 400, description = "Bad request", body = APIResponse<String>)
    )
)]
async fn upsert_global_config(
    Path(key): Path<String>,
    State(state): State<HTTPState>,
    axum::Json(body): axum::Json<UpsertGlobalConfigRequest>,
) -> models::response::Response<GlobalConfigEntry> {
    let value = match body {
        UpsertGlobalConfigRequest::McpServers(servers) => GlobalConfigValue::McpServers(servers),
        UpsertGlobalConfigRequest::Shell(shell) => GlobalConfigValue::Shell(shell),
    };

    let entry = GlobalConfigEntry {
        key: key.clone(),
        value: value.clone(),
    };

    if let Err(e) = state.storage.upsert_global_config(&entry).await {
        return err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to persist config: {}", e),
        );
    }

    // Send reload command via transport
    let reload_result = match &value {
        GlobalConfigValue::McpServers(servers) => {
            let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
            let cmd = GlobalCommand::ReloadMcp {
                config: servers.clone(),
                resp: resp_tx,
            };
            if let Err(e) = state.transport.send_global_command(cmd).await {
                return err_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("failed to send reload command: {}", e),
                );
            }
            resp_rx.await
        }
        GlobalConfigValue::Shell(shell) => {
            let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
            let cmd = GlobalCommand::ReloadShell {
                config: shell.clone(),
                resp: resp_tx,
            };
            if let Err(e) = state.transport.send_global_command(cmd).await {
                return err_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("failed to send reload command: {}", e),
                );
            }
            resp_rx.await
        }
    };

    match reload_result {
        Ok(GlobalCommandResult::Ok(_)) => api_response(StatusCode::OK, entry),
        Ok(GlobalCommandResult::Error(e)) => {
            err_response(StatusCode::INTERNAL_SERVER_ERROR, e.into())
        }
        Err(_) => err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "global resource manager unavailable".into(),
        ),
    }
}

#[utoipa::path(
    delete,
    path = "/global-config/{key}",
    params(
        ("key" = String, Path, description = "Config key")
    ),
    responses(
        (status = 200, description = "Global config deleted", body = APIResponse<String>),
        (status = 404, description = "Config not found", body = APIResponse<String>)
    )
)]
async fn delete_global_config(
    Path(key): Path<String>,
    State(state): State<HTTPState>,
) -> models::response::Response<String> {
    match state.storage.delete_global_config(&key).await {
        Ok(()) => api_response(StatusCode::OK, "deleted".into()),
        Err(e) => err_response(StatusCode::BAD_REQUEST, e.to_string().into()),
    }
}
