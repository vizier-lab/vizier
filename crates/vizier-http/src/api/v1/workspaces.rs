//! Workspace routes (`specs/053-workspace-channel/contracts/http-api.md`). Authorization and
//! validation are in `vizier_core::service::workspaces`; these handlers only translate. A person
//! with no standing in a Workspace gets `404` from every route, as for one that does not exist.

use axum::{
    Extension, Json, Router,
    extract::{
        Path, Query, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::Response,
    routing::{delete, get, post},
};
use futures::{SinkExt, StreamExt};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use vizier_core::{
    auth::Principal,
    service::{ServiceError, workspaces as service},
};

use crate::{
    auth::middleware::TokenExpiry,
    api::v1::integrations::{fail, ok, svc},
    models::response::APIResponse,
    state::HTTPState,
    workspace_topic::WorkspaceConnection,
};

pub use vizier_core::service::workspaces::{
    AddMemberRequest, AuthorView, CommandView, CommandsView, CreateTopicRequest,
    CreateWorkspaceRequest, EntryView, MemberView, MentionView, PostRequest, PostResponse,
    SkillView, StepView, StopResponse, Timeline, TopicSummary, WorkspaceDetail, WorkspaceSummary,
};

pub fn workspaces() -> Router<HTTPState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/{id}", get(detail))
        .route("/{id}/topics", post(create_topic))
        .route("/{id}/members", post(add_member))
        .route("/{id}/members/{kind}/{handle}", delete(remove_member))
        .route("/{id}/commands", get(commands))
        .route("/{id}/stop", post(stop_all))
        .route("/{id}/topics/{tid}/entries", get(timeline).post(post_entry))
        .route("/{id}/topics/{tid}/turns/{agent}/stop", post(stop_agent))
        .route("/{id}/topics/{tid}/stream", get(stream))
}

/// A service refusal in the existing envelope, with its stable `code` in `data.error`.
fn refusal(e: ServiceError) -> Response {
    let status = StatusCode::from_u16(e.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let code = match e.code() {
        Some(code) => code.to_string(),
        None => match status {
            StatusCode::NOT_FOUND => "not_found".into(),
            StatusCode::FORBIDDEN => "forbidden".into(),
            _ => "error".into(),
        },
    };
    fail(status, e.message().to_string(), serde_json::json!({ "error": code }))
}

fn respond<T: Serialize>(result: Result<T, ServiceError>, status: StatusCode) -> Response {
    match result {
        Ok(value) => ok(status, value),
        Err(e) => refusal(e),
    }
}

#[utoipa::path(
    get,
    path = "/workspaces",
    responses((status = 200, description = "The caller's Workspaces, with their topics", body = APIResponse<Vec<WorkspaceSummary>>))
)]
pub async fn list(State(state): State<HTTPState>, Extension(user): Extension<Principal>) -> Response {
    respond(service::list(&svc(&state), &user).await, StatusCode::OK)
}

#[utoipa::path(
    post,
    path = "/workspaces",
    request_body = CreateWorkspaceRequest,
    responses(
        (status = 201, description = "Created, with a first topic and the caller as owner", body = APIResponse<WorkspaceDetail>),
        (status = 403, description = "Missing the `workspaces:create` permission"),
        (status = 409, description = "`name_in_use`"),
        (status = 422, description = "`invalid_name`")
    )
)]
pub async fn create(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Json(body): Json<CreateWorkspaceRequest>,
) -> Response {
    respond(service::create(&svc(&state), &user, body).await, StatusCode::CREATED)
}

#[utoipa::path(
    get,
    path = "/workspaces/{id}",
    params(("id" = String, Path, description = "Workspace ID")),
    responses(
        (status = 200, description = "The Workspace with its members", body = APIResponse<WorkspaceDetail>),
        (status = 404, description = "No such Workspace, or the caller has no standing in it")
    )
)]
pub async fn detail(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(id): Path<String>,
) -> Response {
    respond(service::detail(&svc(&state), &user, &id).await, StatusCode::OK)
}

#[utoipa::path(
    post,
    path = "/workspaces/{id}/topics",
    params(("id" = String, Path, description = "Workspace ID")),
    request_body = CreateTopicRequest,
    responses(
        (status = 201, description = "Created", body = APIResponse<TopicSummary>),
        (status = 404, description = "No such Workspace, or the caller has no standing in it"),
        (status = 409, description = "`name_in_use`"),
        (status = 422, description = "`invalid_name`")
    )
)]
pub async fn create_topic(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(id): Path<String>,
    Json(body): Json<CreateTopicRequest>,
) -> Response {
    respond(service::create_topic(&svc(&state), &user, &id, body).await, StatusCode::CREATED)
}

#[utoipa::path(
    post,
    path = "/workspaces/{id}/members",
    params(("id" = String, Path, description = "Workspace ID")),
    request_body = AddMemberRequest,
    responses(
        (status = 201, description = "Added", body = APIResponse<MemberView>),
        (status = 403, description = "Not an owner, or `cannot_edit_agent`"),
        (status = 404, description = "No such Workspace, person or agent"),
        (status = 409, description = "`handle_in_use`")
    )
)]
pub async fn add_member(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(id): Path<String>,
    Json(body): Json<AddMemberRequest>,
) -> Response {
    respond(service::add_member(&svc(&state), &user, &id, body).await, StatusCode::CREATED)
}

#[utoipa::path(
    delete,
    path = "/workspaces/{id}/members/{kind}/{handle}",
    params(
        ("id" = String, Path, description = "Workspace ID"),
        ("kind" = String, Path, description = "`person` or `agent`"),
        ("handle" = String, Path, description = "Username or agent ID")
    ),
    responses(
        (status = 200, description = "Removed"),
        (status = 403, description = "Not an owner"),
        (status = 404, description = "No such Workspace or member"),
        (status = 409, description = "`last_owner`")
    )
)]
pub async fn remove_member(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((id, kind, handle)): Path<(String, String, String)>,
) -> Response {
    respond(
        service::remove_member(&svc(&state), &user, &id, &kind, &handle).await,
        StatusCode::OK,
    )
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct TimelineQuery {
    /// The `next_before` of the previous page: entries older than this `seq`.
    pub before: Option<i64>,
    /// 1 to 200; default 50.
    pub limit: Option<usize>,
}

#[utoipa::path(
    get,
    path = "/workspaces/{id}/topics/{tid}/entries",
    params(
        ("id" = String, Path, description = "Workspace ID"),
        ("tid" = String, Path, description = "Topic ID"),
        TimelineQuery
    ),
    responses(
        (status = 200, description = "A page of messages, oldest first, with what the agents did in the same span", body = APIResponse<Timeline>),
        (status = 404, description = "No such Workspace or topic, or the caller has no standing in it")
    )
)]
pub async fn timeline(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((id, tid)): Path<(String, String)>,
    Query(query): Query<TimelineQuery>,
) -> Response {
    respond(
        service::timeline(&svc(&state), &user, &id, &tid, query.before, query.limit).await,
        StatusCode::OK,
    )
}

#[utoipa::path(
    post,
    path = "/workspaces/{id}/topics/{tid}/entries",
    params(
        ("id" = String, Path, description = "Workspace ID"),
        ("tid" = String, Path, description = "Topic ID")
    ),
    request_body = PostRequest,
    responses(
        (status = 201, description = "Posted, or a command ran and left a note", body = APIResponse<PostResponse>),
        (status = 403, description = "`cannot_lobotomy`"),
        (status = 404, description = "No such Workspace or topic, or the caller has no standing in it"),
        (status = 422, description = "`not_in_workspace` for /btw and /clear-files, or `too_long`")
    )
)]
pub async fn post_entry(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((id, tid)): Path<(String, String)>,
    Json(body): Json<PostRequest>,
) -> Response {
    respond(service::post(&svc(&state), &user, &id, &tid, body).await, StatusCode::CREATED)
}

#[utoipa::path(
    get,
    path = "/workspaces/{id}/commands",
    params(("id" = String, Path, description = "Workspace ID")),
    responses(
        (status = 200, description = "For the `/` menu: commands, and each member agent's skills with the agents that have them. Names only", body = APIResponse<CommandsView>),
        (status = 404, description = "No such Workspace, or the caller has no standing in it")
    )
)]
pub async fn commands(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(id): Path<String>,
) -> Response {
    respond(service::commands_menu(&svc(&state), &user, &id).await, StatusCode::OK)
}

#[utoipa::path(
    post,
    path = "/workspaces/{id}/stop",
    params(("id" = String, Path, description = "Workspace ID")),
    responses(
        (status = 200, description = "Every agent's turn in every topic aborted, and waiting mentions dropped", body = APIResponse<StopResponse>),
        (status = 404, description = "No such Workspace, or the caller has no standing in it")
    )
)]
pub async fn stop_all(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path(id): Path<String>,
) -> Response {
    respond(service::stop_all(&svc(&state), &user, &id).await, StatusCode::OK)
}

#[utoipa::path(
    post,
    path = "/workspaces/{id}/topics/{tid}/turns/{agent}/stop",
    params(
        ("id" = String, Path, description = "Workspace ID"),
        ("tid" = String, Path, description = "Topic ID"),
        ("agent" = String, Path, description = "Agent ID")
    ),
    responses(
        (status = 200, description = "That agent's turn in that topic aborted", body = APIResponse<StopResponse>),
        (status = 404, description = "No such Workspace, topic or agent member, or the caller has no standing in it")
    )
)]
pub async fn stop_agent(
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Path((id, tid, agent)): Path<(String, String, String)>,
) -> Response {
    respond(service::stop_agent(&svc(&state), &user, &id, &tid, &agent).await, StatusCode::OK)
}

/// How often the server pings. There is no idle deadline: a Workspace is watched while quiet.
const PING_EVERY: std::time::Duration = std::time::Duration::from_secs(30);

#[utoipa::path(
    get,
    path = "/workspaces/{id}/topics/{tid}/stream",
    params(
        ("id" = String, Path, description = "Workspace ID"),
        ("tid" = String, Path, description = "Topic ID")
    ),
    responses(
        (status = 101, description = "WebSocket established. The server pushes `hello`, `workspace_entry`, `history_entry`, `workspace_mentions`, `agent_state`, `member_changed`, `closed` and `resync` frames; the client sends `{ \"post\": { \"text\": \"...\" } }`"),
        (status = 404, description = "No such Workspace or topic, or the caller has no standing in it")
    )
)]
pub async fn stream(
    ws: WebSocketUpgrade,
    State(state): State<HTTPState>,
    Extension(user): Extension<Principal>,
    Extension(expiry): Extension<TokenExpiry>,
    Path((id, tid)): Path<(String, String)>,
) -> Response {
    let (out_tx, out_rx) = flume::unbounded();
    let connection = match WorkspaceConnection::open(&state, &user, &id, &tid, out_tx).await {
        Ok(connection) => connection,
        Err(e) => {
            if matches!(e, ServiceError::Internal(_)) {
                tracing::error!("failed to open a Workspace stream: {e}");
            }
            return Response::builder()
                .status(e.status())
                .body(axum::body::Body::empty())
                .unwrap();
        }
    };
    ws.on_upgrade(move |socket| serve_socket(socket, connection, out_rx, expiry))
}

/// Writes the connection's frames to the socket, hands the client's frames to it, pings, and
/// ends at the token's expiry.
async fn serve_socket(
    socket: WebSocket,
    mut connection: WorkspaceConnection,
    out_rx: flume::Receiver<serde_json::Value>,
    expiry: TokenExpiry,
) {
    let (mut writer, mut reader) = socket.split();
    let Some(outbound) = connection.outbound() else { return };
    let pushing = tokio::spawn(outbound.run());

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let until_expiry = expiry
        .remaining(now)
        .unwrap_or(std::time::Duration::from_secs(60 * 60 * 24 * 365));
    let expired = tokio::time::sleep(until_expiry);
    tokio::pin!(expired);
    let mut ping = tokio::time::interval(PING_EVERY);
    ping.tick().await;

    loop {
        tokio::select! {
            _ = ping.tick() => {
                if writer.send(Message::Ping(vec![].into())).await.is_err() {
                    break;
                }
            }
            frame = out_rx.recv_async() => match frame {
                Ok(frame) => {
                    if writer.send(Message::Text(frame.to_string().into())).await.is_err() {
                        break;
                    }
                }
                // The pushing half ended: the person was removed or the Workspace deleted.
                Err(_) => {
                    let _ = writer.send(Message::Close(None)).await;
                    break;
                }
            },
            msg = reader.next() => match msg {
                Some(Ok(Message::Text(text))) => {
                    if let Ok(frame) = serde_json::from_str::<serde_json::Value>(&text) {
                        connection.handle_inbound(frame).await;
                    }
                }
                Some(Ok(Message::Ping(data))) => {
                    let _ = writer.send(Message::Pong(data)).await;
                }
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
            _ = &mut expired => {
                let _ = writer.send(Message::Close(None)).await;
                break;
            }
        }
    }
    pushing.abort();
    connection.close();
}
