//! Operator-defined providers (contract `http-api.md` § Custom providers): an OpenAI-compatible
//! or Anthropic endpoint at any address, with the operator's own model list. Keys and header
//! values are write-only.

use std::collections::BTreeMap;

use axum::{
    Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use super::{
    DeleteQuery, InUse, in_use,
    usage::{self, TestResult, UsedBy},
};
use crate::{
    channels::http::{
        models::{
            self,
            response::{APIResponse, api_response, err_response},
        },
        state::HTTPState,
    },
    config::provider::ProviderId,
    llm::ListedModel,
    schema::{CustomModel, CustomProtocol, CustomProvider},
    storage::provider::CustomProviderStorage,
};

pub fn custom() -> Router<HTTPState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/test", post(test))
        .route("/{id}", axum::routing::put(update).delete(remove))
        .route("/{id}/fetch-models", post(fetch_models))
}

/// A custom provider without its secrets.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct CustomProviderResponse {
    pub id: ProviderId,
    pub name: String,
    pub protocol: CustomProtocol,
    pub base_url: String,
    pub has_api_key: bool,
    /// Header names only; values are never returned.
    pub headers: Vec<String>,
    pub models: Vec<CustomModel>,
    pub models_fetched_at: Option<DateTime<Utc>>,
    pub used_by: UsedBy,
    pub reachable: Option<bool>,
}

fn to_response(state: &HTTPState, p: &CustomProvider, used_by: UsedBy) -> CustomProviderResponse {
    CustomProviderResponse {
        id: p.id.clone(),
        name: p.name.clone(),
        protocol: p.protocol,
        base_url: p.base_url.clone(),
        has_api_key: p.api_key.as_deref().is_some_and(|k| !k.is_empty()),
        headers: p.headers.keys().cloned().collect(),
        models: p.models.clone(),
        models_fetched_at: p.models_fetched_at,
        used_by,
        reachable: usage::reachable(state, &p.id),
    }
}

async fn respond(state: &HTTPState, p: &CustomProvider) -> CustomProviderResponse {
    let used = usage::used_by_all(state)
        .await
        .remove(&p.id)
        .map(|(u, _)| u)
        .unwrap_or_default();
    to_response(state, p, used)
}

/// A custom provider as the form sends it.
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct CustomProviderBody {
    /// Defaults to a slug of `name`; fixed once saved.
    pub id: Option<String>,
    pub name: String,
    pub protocol: CustomProtocol,
    pub base_url: String,
    /// Omitted keeps the stored key; `""` clears it.
    pub api_key: Option<String>,
    /// An omitted header keeps its stored value; `null` removes it.
    #[serde(default)]
    pub headers: BTreeMap<String, Option<String>>,
    pub models: Option<Vec<CustomModel>>,
}

/// `name` as an id: lower case, runs of anything else as one `-`.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.trim().to_lowercase().chars() {
        if c.is_ascii_alphanumeric() || c == '.' || c == '_' {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out: String = out.trim_matches('-').chars().take(64).collect();
    out.trim_end_matches('-').to_string()
}

/// Ids that are routes under `/providers/custom`.
const RESERVED: &[&str] = &["test", "custom"];

fn valid_url(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|u| matches!(u.scheme(), "http" | "https") && u.has_host())
}

/// Models as the operator saved them: a row that differs from the stored one (or is new) is
/// marked edited, so a later fetch leaves it alone.
pub fn merge_edits(stored: &[CustomModel], sent: Vec<CustomModel>) -> Vec<CustomModel> {
    sent.into_iter()
        .map(|mut row| {
            let before = stored.iter().find(|m| m.id == row.id);
            let changed = before.is_none_or(|b| {
                (b.context, b.output, b.tool_call, b.image_input, b.reasoning)
                    != (row.context, row.output, row.tool_call, row.image_input, row.reasoning)
            });
            row.edited = before.is_some_and(|b| b.edited) || changed;
            row
        })
        .collect()
}

/// A server's model listing merged into the stored rows (data-model §5): new ids are added,
/// edited rows are kept as they are, the others take the server's values. Rows the server no
/// longer lists are kept.
pub fn merge_fetched(stored: &[CustomModel], listed: &[ListedModel]) -> Vec<CustomModel> {
    let mut out: Vec<CustomModel> = stored.to_vec();
    for l in listed {
        match out.iter_mut().find(|m| m.id == l.id) {
            Some(row) if row.edited => {}
            Some(row) => row.context = l.context.or(row.context),
            None => out.push(CustomModel {
                id: l.id.clone(),
                context: l.context,
                ..Default::default()
            }),
        }
    }
    out
}

/// The body over the stored provider (if any). The id must already be checked.
fn build(id: ProviderId, body: CustomProviderBody, stored: Option<&CustomProvider>) -> CustomProvider {
    let mut headers = stored.map(|s| s.headers.clone()).unwrap_or_default();
    for (name, value) in body.headers {
        match value {
            None => {
                headers.remove(&name);
            }
            Some(v) if v.is_empty() && headers.contains_key(&name) => {}
            Some(v) => {
                headers.insert(name, v);
            }
        }
    }
    let api_key = match body.api_key {
        None => stored.and_then(|s| s.api_key.clone()),
        Some(k) if k.is_empty() => None,
        Some(k) => Some(k),
    };
    let stored_models = stored.map(|s| s.models.as_slice()).unwrap_or_default();
    let models = match body.models {
        Some(sent) => merge_edits(stored_models, sent),
        None => stored_models.to_vec(),
    };
    CustomProvider {
        id,
        name: body.name.trim().to_string(),
        protocol: body.protocol,
        base_url: body.base_url.trim().trim_end_matches('/').to_string(),
        api_key,
        headers,
        models,
        models_fetched_at: stored.and_then(|s| s.models_fetched_at),
    }
}

fn check_body(body: &CustomProviderBody) -> Result<(), String> {
    if body.name.trim().is_empty() {
        return Err("name is required".into());
    }
    if !valid_url(body.base_url.trim()) {
        return Err(format!("\"{}\" is not an http(s) URL", body.base_url));
    }
    Ok(())
}

#[utoipa::path(
    get,
    path = "/providers/custom",
    responses((status = 200, description = "Custom providers, without secrets", body = APIResponse<Vec<CustomProviderResponse>>))
)]
pub(crate) async fn list(State(state): State<HTTPState>) -> models::response::Response<Vec<CustomProviderResponse>> {
    crate::catalogue::refresh::maybe_refresh(&state.deps);
    let providers = match state.storage.list_custom_providers().await {
        Ok(p) => p,
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let mut used = usage::used_by_all(&state).await;
    let out = providers
        .iter()
        .map(|p| {
            let u = used.remove(&p.id).map(|(u, _)| u).unwrap_or_default();
            to_response(&state, p, u)
        })
        .collect();
    api_response(StatusCode::OK, out)
}

#[utoipa::path(
    post,
    path = "/providers/custom",
    request_body = CustomProviderBody,
    responses(
        (status = 200, description = "Created", body = APIResponse<CustomProviderResponse>),
        (status = 400, description = "Invalid id or URL", body = APIResponse<String>),
        (status = 409, description = "The id is taken; the message names by what", body = APIResponse<String>)
    )
)]
pub(crate) async fn create(
    State(state): State<HTTPState>,
    axum::Json(body): axum::Json<CustomProviderBody>,
) -> models::response::Response<CustomProviderResponse> {
    if let Err(e) = check_body(&body) {
        return err_response(StatusCode::BAD_REQUEST, e);
    }
    let raw = body.id.clone().filter(|i| !i.is_empty()).unwrap_or_else(|| slug(&body.name));
    let Ok(id) = ProviderId::new(raw.clone()) else {
        return err_response(
            StatusCode::BAD_REQUEST,
            format!("\"{raw}\" is not a valid id: use lower-case letters, digits, '.', '_' and '-'"),
        );
    };
    if RESERVED.contains(&id.as_str()) {
        return err_response(StatusCode::BAD_REQUEST, format!("\"{id}\" is reserved"));
    }
    let clash = if state.catalogue.provider(&id).is_some() {
        Some("a catalogue provider")
    } else if crate::catalogue::builtin::builtin(&id).is_some() {
        Some("a built-in provider")
    } else if state.storage.get_custom_provider(&id).await.ok().flatten().is_some() {
        Some("another custom provider")
    } else {
        None
    };
    if let Some(what) = clash {
        return err_response(
            StatusCode::CONFLICT,
            format!("the id \"{id}\" is already used by {what}; choose another id"),
        );
    }
    let provider = build(id, body, None);
    if let Err(e) = state.storage.upsert_custom_provider(&provider).await {
        return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string());
    }
    api_response(StatusCode::OK, respond(&state, &provider).await)
}

async fn stored(state: &HTTPState, id: &str) -> Result<CustomProvider, (StatusCode, String)> {
    let Ok(id) = ProviderId::parse(id) else {
        return Err((StatusCode::BAD_REQUEST, "invalid provider id".into()));
    };
    match state.storage.get_custom_provider(&id).await {
        Ok(Some(p)) => Ok(p),
        Ok(None) => Err((StatusCode::NOT_FOUND, "custom provider not found".into())),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

#[utoipa::path(
    put,
    path = "/providers/custom/{id}",
    params(("id" = String, Path, description = "Custom provider id")),
    request_body = CustomProviderBody,
    responses(
        (status = 200, description = "Updated", body = APIResponse<CustomProviderResponse>),
        (status = 400, description = "A different id, or an invalid URL", body = APIResponse<String>)
    )
)]
pub(crate) async fn update(
    Path(id): Path<String>,
    State(state): State<HTTPState>,
    axum::Json(body): axum::Json<CustomProviderBody>,
) -> models::response::Response<CustomProviderResponse> {
    let current = match stored(&state, &id).await {
        Ok(p) => p,
        Err((status, e)) => return err_response(status, e),
    };
    if let Some(sent) = body.id.as_deref().filter(|s| !s.is_empty())
        && sent != current.id.as_str()
    {
        return err_response(
            StatusCode::BAD_REQUEST,
            "a custom provider's id can't be changed; create a new one instead".into(),
        );
    }
    if let Err(e) = check_body(&body) {
        return err_response(StatusCode::BAD_REQUEST, e);
    }
    let provider = build(current.id.clone(), body, Some(&current));
    if let Err(e) = state.storage.upsert_custom_provider(&provider).await {
        return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string());
    }
    if provider != current {
        super::restart_users(&state, &provider.id).await;
    }
    api_response(StatusCode::OK, respond(&state, &provider).await)
}

#[utoipa::path(
    delete,
    path = "/providers/custom/{id}",
    params(("id" = String, Path, description = "Custom provider id"), DeleteQuery),
    responses(
        (status = 200, description = "Deleted", body = APIResponse<String>),
        (status = 409, description = "Agents use this provider; `data` lists them", body = APIResponse<InUse>)
    )
)]
pub(crate) async fn remove(
    Path(id): Path<String>,
    Query(query): Query<DeleteQuery>,
    State(state): State<HTTPState>,
) -> models::response::Response<serde_json::Value> {
    let current = match stored(&state, &id).await {
        Ok(p) => p,
        Err((status, e)) => return err_response(status, e),
    };
    if let Some(conflict) = in_use(&state, &current.id, query.confirm).await {
        return conflict;
    }
    if let Err(e) = state.storage.delete_custom_provider(&current.id).await {
        return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string());
    }
    super::restart_users(&state, &current.id).await;
    api_response(StatusCode::OK, "deleted".into())
}

async fn list_server_models(
    state: &HTTPState,
    provider: &CustomProvider,
) -> Result<Vec<ListedModel>, String> {
    let resolved = usage::resolve_for_test(state, &provider.id, None, Some(provider))
        .await
        .map_err(|e| e.to_string())?;
    let model = crate::llm::build(resolved, state.deps.http.clone(), Default::default());
    match tokio::time::timeout(std::time::Duration::from_secs(15), model.list_models()).await {
        Ok(Ok(models)) => Ok(models),
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err("the server did not answer within 15 seconds".into()),
    }
}

#[utoipa::path(
    post,
    path = "/providers/custom/{id}/fetch-models",
    params(("id" = String, Path, description = "Custom provider id")),
    responses(
        (status = 200, description = "The provider with the server's models merged in", body = APIResponse<CustomProviderResponse>),
        (status = 502, description = "The server could not be reached; nothing changed", body = APIResponse<String>)
    )
)]
pub(crate) async fn fetch_models(
    Path(id): Path<String>,
    State(state): State<HTTPState>,
) -> models::response::Response<CustomProviderResponse> {
    let mut provider = match stored(&state, &id).await {
        Ok(p) => p,
        Err((status, e)) => return err_response(status, e),
    };
    let listed = match list_server_models(&state, &provider).await {
        Ok(l) => l,
        Err(e) => return err_response(StatusCode::BAD_GATEWAY, e),
    };
    provider.models = merge_fetched(&provider.models, &listed);
    provider.models_fetched_at = Some(Utc::now());
    if let Err(e) = state.storage.upsert_custom_provider(&provider).await {
        return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string());
    }
    api_response(StatusCode::OK, respond(&state, &provider).await)
}

#[utoipa::path(
    post,
    path = "/providers/custom/test",
    request_body = CustomProviderBody,
    responses((status = 200, description = "`ok` says whether the server answered", body = APIResponse<TestResult>))
)]
pub(crate) async fn test(
    State(state): State<HTTPState>,
    axum::Json(body): axum::Json<CustomProviderBody>,
) -> models::response::Response<TestResult> {
    if let Err(e) = check_body(&body) {
        return err_response(StatusCode::BAD_REQUEST, e);
    }
    // Testing an edit: a key or header left out is the stored one.
    let raw = body.id.clone().filter(|i| !i.is_empty()).unwrap_or_else(|| slug(&body.name));
    let id = ProviderId::new(raw).unwrap_or_else(|_| ProviderId::new_static("unsaved-custom"));
    let current = state.storage.get_custom_provider(&id).await.ok().flatten();
    let provider = build(id.clone(), body, current.as_ref());
    match usage::resolve_for_test(&state, &id, None, Some(&provider)).await {
        Ok(resolved) => api_response(StatusCode::OK, usage::check(&state, resolved).await),
        Err(e) => api_response(
            StatusCode::OK,
            TestResult {
                ok: false,
                model_count: None,
                status: None,
                error: Some(e.to_string()),
            },
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, context: Option<u64>, edited: bool) -> CustomModel {
        CustomModel {
            id: id.into(),
            context,
            edited,
            ..Default::default()
        }
    }

    fn listed(id: &str, context: Option<u64>) -> ListedModel {
        ListedModel {
            id: id.into(),
            context,
        }
    }

    #[test]
    fn a_fetch_adds_new_ids_keeps_edited_rows_and_refreshes_the_rest() {
        let stored = vec![row("a", Some(1000), true), row("b", Some(1000), false), row("gone", None, false)];
        let merged = merge_fetched(
            &stored,
            &[listed("a", Some(9)), listed("b", Some(8192)), listed("c", Some(4096))],
        );
        assert_eq!(merged[0], row("a", Some(1000), true));
        assert_eq!(merged[1], row("b", Some(8192), false));
        assert_eq!(merged[2], row("gone", None, false));
        assert_eq!(merged[3], row("c", Some(4096), false));
    }

    #[test]
    fn a_changed_or_new_row_is_marked_edited() {
        let stored = vec![row("a", Some(1000), false), row("b", Some(1000), false)];
        let merged = merge_edits(
            &stored,
            vec![row("a", Some(1000), false), row("b", Some(2000), false), row("c", None, false)],
        );
        assert!(!merged[0].edited);
        assert!(merged[1].edited);
        assert!(merged[2].edited);
    }

    #[test]
    fn names_slug_into_ids() {
        assert_eq!(slug("Home vLLM"), "home-vllm");
        assert_eq!(slug("  My  Proxy!! (EU) "), "my-proxy-eu");
        assert!(ProviderId::is_valid(&slug("LiteLLM gateway v2.1")));
    }

    #[test]
    fn omitted_secrets_are_kept_and_null_headers_removed() {
        let stored = CustomProvider {
            id: ProviderId::new_static("home"),
            name: "Home".into(),
            protocol: CustomProtocol::OpenaiCompatible,
            base_url: "http://h/v1".into(),
            api_key: Some("k".into()),
            headers: [("a".to_string(), "1".to_string()), ("b".to_string(), "2".to_string())].into(),
            models: vec![],
            models_fetched_at: None,
        };
        let body = CustomProviderBody {
            id: None,
            name: "Home".into(),
            protocol: CustomProtocol::OpenaiCompatible,
            base_url: "http://h/v1/".into(),
            api_key: None,
            headers: [("a".to_string(), None), ("c".to_string(), Some("3".to_string()))].into(),
            models: None,
        };
        let built = build(stored.id.clone(), body, Some(&stored));
        assert_eq!(built.api_key.as_deref(), Some("k"));
        assert_eq!(built.base_url, "http://h/v1");
        assert_eq!(built.headers.keys().collect::<Vec<_>>(), ["b", "c"]);
    }
}
