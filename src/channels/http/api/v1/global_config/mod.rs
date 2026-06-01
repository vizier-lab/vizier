use axum::{
    Router,
    extract::State,
    routing::{delete, get, put},
};
use reqwest::StatusCode;
use std::collections::HashMap;

use crate::{
    channels::http::{
        models::{
            self,
            response::{api_response, err_response},
        },
        state::HTTPState,
    },
    config::{shell::ShellConfig, tools::mcp::McpClientConfig},
    schema::{GlobalCommand, GlobalCommandResult, GlobalConfigEntry, GlobalConfigValue},
    storage::global_config::GlobalConfigStorage,
};

pub fn mcp_servers_routes() -> Router<HTTPState> {
    Router::new()
        .route("/", get(get_mcp_servers).put(upsert_mcp_servers).delete(delete_mcp_servers))
}

pub fn shell_routes() -> Router<HTTPState> {
    Router::new()
        .route("/", get(get_shell_config).put(upsert_shell_config).delete(delete_shell_config))
}

// ============================================================================
// MCP SERVERS
// ============================================================================

async fn get_mcp_servers(
    State(state): State<HTTPState>,
) -> models::response::Response<GlobalConfigEntry> {
    match state.storage.get_global_config("mcp_servers").await {
        Ok(Some(entry)) => api_response(StatusCode::OK, entry),
        Ok(None) => err_response(StatusCode::NOT_FOUND, "MCP servers config not found".into()),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string().into()),
    }
}

async fn upsert_mcp_servers(
    State(state): State<HTTPState>,
    axum::Json(servers): axum::Json<HashMap<String, McpClientConfig>>,
) -> models::response::Response<GlobalConfigEntry> {
    let value = GlobalConfigValue::McpServers(servers.clone());
    let entry = GlobalConfigEntry {
        key: "mcp_servers".to_string(),
        value: value.clone(),
    };

    if let Err(e) = state.storage.upsert_global_config(&entry).await {
        return err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to persist config: {}", e),
        );
    }

    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
    let cmd = GlobalCommand::ReloadMcp {
        config: servers,
        resp: resp_tx,
    };
    if let Err(e) = state.transport.send_global_command(cmd).await {
        return err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to send reload command: {}", e),
        );
    }

    match resp_rx.await {
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

async fn delete_mcp_servers(
    State(state): State<HTTPState>,
) -> models::response::Response<String> {
    match state.storage.delete_global_config("mcp_servers").await {
        Ok(()) => api_response(StatusCode::OK, "deleted".into()),
        Err(e) => err_response(StatusCode::BAD_REQUEST, e.to_string().into()),
    }
}

// ============================================================================
// SHELL CONFIG
// ============================================================================

async fn get_shell_config(
    State(state): State<HTTPState>,
) -> models::response::Response<GlobalConfigEntry> {
    match state.storage.get_global_config("shell").await {
        Ok(Some(entry)) => api_response(StatusCode::OK, entry),
        Ok(None) => err_response(StatusCode::NOT_FOUND, "Shell config not found".into()),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string().into()),
    }
}

async fn upsert_shell_config(
    State(state): State<HTTPState>,
    axum::Json(shell): axum::Json<ShellConfig>,
) -> models::response::Response<GlobalConfigEntry> {
    let value = GlobalConfigValue::Shell(shell.clone());
    let entry = GlobalConfigEntry {
        key: "shell".to_string(),
        value: value.clone(),
    };

    if let Err(e) = state.storage.upsert_global_config(&entry).await {
        return err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to persist config: {}", e),
        );
    }

    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
    let cmd = GlobalCommand::ReloadShell {
        config: shell,
        resp: resp_tx,
    };
    if let Err(e) = state.transport.send_global_command(cmd).await {
        return err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to send reload command: {}", e),
        );
    }

    match resp_rx.await {
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

async fn delete_shell_config(
    State(state): State<HTTPState>,
) -> models::response::Response<String> {
    match state.storage.delete_global_config("shell").await {
        Ok(()) => api_response(StatusCode::OK, "deleted".into()),
        Err(e) => err_response(StatusCode::BAD_REQUEST, e.to_string().into()),
    }
}
