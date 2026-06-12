use axum::{
    Router,
    extract::{Path, State},
    routing::{delete, get, put},
};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::{
    channels::http::{
        models::{
            self,
            response::{APIResponse, api_response, err_response},
        },
        state::HTTPState,
    },
    config::provider::ProviderVariant,
    schema::{ProviderEntry, ProviderEntryConfig},
    storage::provider::ProviderStorage,
};

pub fn providers() -> Router<HTTPState> {
    Router::new().route("/", get(list_providers)).route(
        "/{variant}",
        get(get_provider)
            .put(upsert_provider)
            .delete(delete_provider),
    )
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct ProviderResponse {
    pub variant: ProviderVariant,
    pub has_api_key: bool,
    pub base_url: Option<String>,
    pub enabled: Option<bool>,
}

fn provider_to_response(entry: &ProviderEntry) -> ProviderResponse {
    let (has_api_key, base_url, enabled) = match &entry.config {
        ProviderEntryConfig::Ollama { base_url } => (false, Some(base_url.clone()), None),
        ProviderEntryConfig::Openai { api_key } => (!api_key.is_empty(), None, None),
        ProviderEntryConfig::Anthropic { api_key } => (!api_key.is_empty(), None, None),
        ProviderEntryConfig::Deepseek { api_key } => (!api_key.is_empty(), None, None),
        ProviderEntryConfig::Openrouter { api_key } => (!api_key.is_empty(), None, None),
        ProviderEntryConfig::Gemini { api_key } => (!api_key.is_empty(), None, None),
        ProviderEntryConfig::Mimo { api_key } => (!api_key.is_empty(), None, None),
        ProviderEntryConfig::LlamaCpp { base_url } => (false, Some(base_url.clone()), None),
        ProviderEntryConfig::Mistralrs { enabled } => (false, None, Some(*enabled)),
        ProviderEntryConfig::Elevenlabs { api_key } => (!api_key.is_empty(), None, None),
        ProviderEntryConfig::Groq { api_key } => (!api_key.is_empty(), None, None),
        ProviderEntryConfig::Mistral { api_key } => (!api_key.is_empty(), None, None),
        ProviderEntryConfig::Xai { api_key } => (!api_key.is_empty(), None, None),
        ProviderEntryConfig::Perplexity { api_key } => (!api_key.is_empty(), None, None),
        ProviderEntryConfig::Moonshot { api_key, base_url } => {
            (!api_key.is_empty(), base_url.clone(), None)
        }
        ProviderEntryConfig::Zai { api_key, base_url } => {
            (!api_key.is_empty(), base_url.clone(), None)
        }
        ProviderEntryConfig::Minimax { api_key, base_url } => {
            (!api_key.is_empty(), base_url.clone(), None)
        }
        ProviderEntryConfig::Together { api_key } => (!api_key.is_empty(), None, None),
        ProviderEntryConfig::Cohere { api_key } => (!api_key.is_empty(), None, None),
        ProviderEntryConfig::Huggingface { api_key } => (!api_key.is_empty(), None, None),
        ProviderEntryConfig::Hyperbolic { api_key } => (!api_key.is_empty(), None, None),
        ProviderEntryConfig::Voyageai { api_key } => (!api_key.is_empty(), None, None),
        ProviderEntryConfig::Galadriel { api_key } => (!api_key.is_empty(), None, None),
        ProviderEntryConfig::Mira { api_key } => (!api_key.is_empty(), None, None),
        ProviderEntryConfig::Copilot { api_key } => (!api_key.is_empty(), None, None),
        ProviderEntryConfig::Chatgpt {
            access_token,
            account_id: _,
            base_url,
        } => (!access_token.is_empty(), base_url.clone(), None),
        ProviderEntryConfig::Azure { endpoint, api_key } => {
            (!api_key.is_empty(), Some(endpoint.clone()), None)
        }
    };

    ProviderResponse {
        variant: entry.variant.clone(),
        has_api_key,
        base_url,
        enabled,
    }
}

#[utoipa::path(
    get,
    path = "/providers",
    responses(
        (status = 200, description = "List of providers", body = APIResponse<Vec<ProviderResponse>>)
    )
)]
async fn list_providers(
    State(state): State<HTTPState>,
) -> models::response::Response<Vec<ProviderResponse>> {
    match state.storage.list_providers().await {
        Ok(entries) => {
            let res: Vec<ProviderResponse> = entries.iter().map(provider_to_response).collect();
            api_response(StatusCode::OK, res)
        }
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string().into()),
    }
}

#[utoipa::path(
    get,
    path = "/providers/{variant}",
    params(
        ("variant" = String, Path, description = "Provider variant")
    ),
    responses(
        (status = 200, description = "Provider details", body = APIResponse<ProviderResponse>),
        (status = 404, description = "Provider not found", body = APIResponse<String>)
    )
)]
async fn get_provider(
    Path(variant): Path<ProviderVariant>,
    State(state): State<HTTPState>,
) -> models::response::Response<ProviderResponse> {
    match state.storage.get_provider(&variant).await {
        Ok(Some(entry)) => api_response(StatusCode::OK, provider_to_response(&entry)),
        Ok(None) => err_response(StatusCode::NOT_FOUND, "provider not found".into()),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string().into()),
    }
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UpsertProviderRequest {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub enabled: Option<bool>,
    pub access_token: Option<String>,
    pub account_id: Option<String>,
    pub endpoint: Option<String>,
}

#[utoipa::path(
    put,
    path = "/providers/{variant}",
    params(
        ("variant" = String, Path, description = "Provider variant")
    ),
    request_body = UpsertProviderRequest,
    responses(
        (status = 200, description = "Provider upserted", body = APIResponse<ProviderResponse>),
        (status = 400, description = "Bad request", body = APIResponse<String>)
    )
)]
async fn upsert_provider(
    Path(variant): Path<ProviderVariant>,
    State(state): State<HTTPState>,
    axum::Json(body): axum::Json<UpsertProviderRequest>,
) -> models::response::Response<ProviderResponse> {
    let config = match variant {
        ProviderVariant::ollama => ProviderEntryConfig::Ollama {
            base_url: body
                .base_url
                .unwrap_or_else(|| "http://localhost:11434".into()),
        },
        ProviderVariant::openai => ProviderEntryConfig::Openai {
            api_key: body.api_key.unwrap_or_default(),
        },
        ProviderVariant::anthropic => ProviderEntryConfig::Anthropic {
            api_key: body.api_key.unwrap_or_default(),
        },
        ProviderVariant::deepseek => ProviderEntryConfig::Deepseek {
            api_key: body.api_key.unwrap_or_default(),
        },
        ProviderVariant::openrouter => ProviderEntryConfig::Openrouter {
            api_key: body.api_key.unwrap_or_default(),
        },
        ProviderVariant::gemini => ProviderEntryConfig::Gemini {
            api_key: body.api_key.unwrap_or_default(),
        },
        ProviderVariant::mimo => ProviderEntryConfig::Mimo {
            api_key: body.api_key.unwrap_or_default(),
        },
        ProviderVariant::llama_cpp => ProviderEntryConfig::LlamaCpp {
            base_url: body
                .base_url
                .unwrap_or_else(|| "http://localhost:8080".into()),
        },
        ProviderVariant::mistralrs => ProviderEntryConfig::Mistralrs {
            enabled: body.enabled.unwrap_or(true),
        },
        ProviderVariant::elevenlabs => ProviderEntryConfig::Elevenlabs {
            api_key: body.api_key.unwrap_or_default(),
        },
        ProviderVariant::groq => ProviderEntryConfig::Groq {
            api_key: body.api_key.unwrap_or_default(),
        },
        ProviderVariant::mistral => ProviderEntryConfig::Mistral {
            api_key: body.api_key.unwrap_or_default(),
        },
        ProviderVariant::xai => ProviderEntryConfig::Xai {
            api_key: body.api_key.unwrap_or_default(),
        },
        ProviderVariant::perplexity => ProviderEntryConfig::Perplexity {
            api_key: body.api_key.unwrap_or_default(),
        },
        ProviderVariant::moonshot => ProviderEntryConfig::Moonshot {
            api_key: body.api_key.unwrap_or_default(),
            base_url: body.base_url.filter(|s| !s.is_empty()),
        },
        ProviderVariant::zai => ProviderEntryConfig::Zai {
            api_key: body.api_key.unwrap_or_default(),
            base_url: body.base_url.filter(|s| !s.is_empty()),
        },
        ProviderVariant::minimax => ProviderEntryConfig::Minimax {
            api_key: body.api_key.unwrap_or_default(),
            base_url: body.base_url.filter(|s| !s.is_empty()),
        },
        ProviderVariant::together => ProviderEntryConfig::Together {
            api_key: body.api_key.unwrap_or_default(),
        },
        ProviderVariant::cohere => ProviderEntryConfig::Cohere {
            api_key: body.api_key.unwrap_or_default(),
        },
        ProviderVariant::huggingface => ProviderEntryConfig::Huggingface {
            api_key: body.api_key.unwrap_or_default(),
        },
        ProviderVariant::hyperbolic => ProviderEntryConfig::Hyperbolic {
            api_key: body.api_key.unwrap_or_default(),
        },
        ProviderVariant::voyageai => ProviderEntryConfig::Voyageai {
            api_key: body.api_key.unwrap_or_default(),
        },
        ProviderVariant::galadriel => ProviderEntryConfig::Galadriel {
            api_key: body.api_key.unwrap_or_default(),
        },
        ProviderVariant::mira => ProviderEntryConfig::Mira {
            api_key: body.api_key.unwrap_or_default(),
        },
        ProviderVariant::copilot => ProviderEntryConfig::Copilot {
            api_key: body.api_key.unwrap_or_default(),
        },
        ProviderVariant::chatgpt => ProviderEntryConfig::Chatgpt {
            access_token: body.access_token.unwrap_or_default(),
            account_id: body.account_id.unwrap_or_default(),
            base_url: body.base_url.filter(|s| !s.is_empty()),
        },
        ProviderVariant::azure => ProviderEntryConfig::Azure {
            endpoint: body
                .endpoint
                .or(body.base_url.clone())
                .unwrap_or_default(),
            api_key: body.api_key.unwrap_or_default(),
        },
    };

    let entry = ProviderEntry { variant, config };

    match state.storage.upsert_provider(&entry).await {
        Ok(()) => api_response(StatusCode::OK, provider_to_response(&entry)),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string().into()),
    }
}

#[utoipa::path(
    delete,
    path = "/providers/{variant}",
    params(
        ("variant" = String, Path, description = "Provider variant")
    ),
    responses(
        (status = 200, description = "Provider deleted", body = APIResponse<String>),
        (status = 404, description = "Provider not found", body = APIResponse<String>)
    )
)]
async fn delete_provider(
    Path(variant): Path<ProviderVariant>,
    State(state): State<HTTPState>,
) -> models::response::Response<String> {
    match state.storage.delete_provider(&variant).await {
        Ok(()) => api_response(StatusCode::OK, "deleted".into()),
        Err(e) => err_response(StatusCode::BAD_REQUEST, e.to_string().into()),
    }
}
