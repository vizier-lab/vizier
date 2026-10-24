//! Media golden fixtures recorded from rig 0.39: each of our media clients is pointed at a
//! local server replaying rig's canned response, and must send the request rig sent and return
//! what rig returned. Embedding vectors must be equal element for element (FR-020).
//!
//! Documented differences: Gemini's key goes in the `x-goog-api-key` header rather than
//! `?key=` (so it never lands in a URL or a log); and Hugging Face requests go to
//! `/hf-inference/models/{model}`, the inference route rig 0.39 dropped.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::IntoResponse;
use base64::Engine;
use serde_json::{Value, json};

use super::cohere::CohereEmbeddings;
use super::gemini::{GeminiEmbeddings, GeminiStt, gemini_dimensions};
use super::huggingface::{HuggingFaceImages, HuggingFaceStt};
use super::hyperbolic::{HyperbolicImages, HyperbolicTts};
use super::ollama::OllamaEmbeddings;
use super::openai_shaped::{
    Endpoint, ImageApi, ImageSizing, OpenAiEmbeddings, OpenAiImages, OpenAiStt, OpenAiTts, XaiTts,
    openai_dimensions,
};
use crate::embedding::VizierEmbeddingModel;
use crate::image_generation::VizierImageGenModel;
use crate::llm::auth::{CopilotExchange, KeyStyle, StaticKey};
use crate::stt::VizierSttModel;
use crate::tts::{VizierTtsModel, mp3_to_wav};

pub(crate) const MEDIA_AUDIO: &[u8] = b"OggS-fake-audio-bytes";
pub(crate) const MEDIA_FILENAME: &str = "voice.ogg";
pub(crate) const MEDIA_DOCS: [&str; 2] = ["the garden", "tomatoes grow in summer"];

/// The fields of a multipart body as `{name, filename, value}`, in order.
pub(crate) fn multipart_fields(body: &[u8], content_type: &str) -> Value {
    let boundary = content_type
        .split("boundary=")
        .nth(1)
        .unwrap_or_default()
        .trim_matches('"');
    let text = String::from_utf8_lossy(body);
    let mut fields = vec![];
    for part in text.split(&format!("--{boundary}")) {
        let Some((head, value)) = part.split_once("\r\n\r\n") else {
            continue;
        };
        let name = head
            .split("name=\"")
            .nth(1)
            .and_then(|s| s.split('"').next())
            .unwrap_or_default();
        let filename = head
            .split("filename=\"")
            .nth(1)
            .and_then(|s| s.split('"').next());
        fields.push(json!({"name": name, "filename": filename, "value": value.trim_end_matches("\r\n")}));
    }
    Value::Array(fields)
}

#[derive(Default)]
pub(crate) struct Replay {
    pub response: (String, Vec<u8>),
    pub seen: Vec<(String, Vec<u8>, String)>,
}

pub(crate) type ReplayState = Arc<Mutex<Replay>>;

async fn replay(State(state): State<ReplayState>, uri: Uri, headers: HeaderMap, body: Bytes) -> impl IntoResponse {
    let mut s = state.lock().unwrap();
    let ct = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    s.seen.push((uri.to_string(), body.to_vec(), ct));
    let (content_type, body) = s.response.clone();
    (StatusCode::OK, [("content-type", content_type)], body)
}

/// A server on `127.0.0.1:0` that records every request and answers with `state.response`.
pub(crate) async fn replay_server() -> (String, ReplayState) {
    let state: ReplayState = Arc::default();
    let app = Router::new().fallback(replay).with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, state)
}

fn strip_nulls(v: &mut Value) {
    match v {
        Value::Object(map) => {
            map.retain(|_, v| !v.is_null());
            map.values_mut().for_each(strip_nulls);
        }
        Value::Array(items) => items.iter_mut().for_each(strip_nulls),
        _ => {}
    }
}

fn b64(data: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(data)
}

fn unb64(s: &str) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD.decode(s).unwrap()
}

/// Our client for one fixture, run once; returns our output as JSON.
async fn run_ours(name: &str, model: &str, base: String) -> Value {
    let http = reqwest::Client::new();
    let bearer = StaticKey::shared(KeyStyle::Bearer, Some("test-key".into()));
    let ep = |credential| Endpoint {
        http: http.clone(),
        base_url: base.clone(),
        credential,
        model: model.to_string(),
    };
    let docs: Vec<String> = MEDIA_DOCS.iter().map(|s| s.to_string()).collect();
    let stt = |m: Box<dyn VizierSttModel>| async move {
        json!(m.transcribe(MEDIA_AUDIO, MEDIA_FILENAME, Some("en")).await.unwrap())
    };
    let tts = |m: Box<dyn VizierTtsModel>, voice: &'static str| async move {
        json!(b64(&m.generate_speech("hi there", voice, 1.25).await.unwrap()))
    };
    let image = |m: Box<dyn VizierImageGenModel>| async move {
        json!(b64(&m.generate("a tomato", Some("1024x768")).await.unwrap().0))
    };
    let embed = |m: Box<dyn VizierEmbeddingModel + Send + Sync>, docs: Vec<String>| async move {
        json!(m.embed_texts(docs).await.unwrap())
    };
    match name {
        "stt_openai" | "stt_groq" | "stt_mistral" => stt(Box::new(OpenAiStt(ep(bearer)))).await,
        "stt_gemini" => {
            let key = StaticKey::shared(KeyStyle::Header("x-goog-api-key"), Some("test-key".into()));
            stt(Box::new(GeminiStt(ep(key)))).await
        }
        "stt_huggingface" => stt(Box::new(HuggingFaceStt(ep(bearer)))).await,
        "tts_openai" => {
            tts(Box::new(OpenAiTts { endpoint: ep(bearer), response_format: None }), "alloy").await
        }
        "tts_openrouter" => {
            tts(Box::new(OpenAiTts { endpoint: ep(bearer), response_format: Some("mp3") }), "alloy").await
        }
        "tts_xai" => tts(Box::new(XaiTts(ep(bearer))), "alloy").await,
        "tts_hyperbolic" => tts(Box::new(HyperbolicTts(ep(bearer))), "EN-US").await,
        "image_openai_dalle" | "image_openai_gpt" => {
            image(Box::new(OpenAiImages { endpoint: ep(bearer), api: ImageApi::Generations(ImageSizing::Size) })).await
        }
        "image_xai" => {
            image(Box::new(OpenAiImages {
                endpoint: ep(bearer),
                api: ImageApi::Generations(ImageSizing::AspectRatio("1:1")),
            }))
            .await
        }
        "image_hyperbolic" => image(Box::new(HyperbolicImages(ep(bearer)))).await,
        "image_huggingface" => image(Box::new(HuggingFaceImages(ep(bearer)))).await,
        "embed_openai" | "embed_openai_ada" => {
            embed(Box::new(OpenAiEmbeddings { endpoint: ep(bearer), dimensions: openai_dimensions(model) }), docs).await
        }
        "embed_openrouter" | "embed_together" | "embed_mistral" | "embed_voyageai" => {
            embed(Box::new(OpenAiEmbeddings { endpoint: ep(bearer), dimensions: None }), docs).await
        }
        "embed_copilot" => {
            let copilot = CopilotExchange::new(http.clone(), "tid=1;exp=9999999999".into(), base.clone());
            embed(Box::new(OpenAiEmbeddings { endpoint: ep(copilot), dimensions: openai_dimensions(model) }), docs).await
        }
        "embed_cohere" => {
            embed(Box::new(CohereEmbeddings { endpoint: ep(bearer), input_type: "search_document" }), docs).await
        }
        "embed_gemini" => {
            let key = StaticKey::shared(KeyStyle::Header("x-goog-api-key"), Some("test-key".into()));
            embed(Box::new(GeminiEmbeddings { endpoint: ep(key), dimensions: gemini_dimensions(model) }), docs).await
        }
        "embed_ollama" => embed(Box::new(OllamaEmbeddings(ep(StaticKey::none()))), docs).await,
        other => panic!("no client for media fixture {other}"),
    }
}

#[tokio::test]
async fn every_media_golden_matches_rig() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/llm/media/fixtures/golden");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    assert!(files.len() >= 20, "only {} media fixtures", files.len());

    let (url, state) = replay_server().await;
    let mut failures = vec![];
    for file in files {
        let name = file.file_stem().unwrap().to_string_lossy().to_string();
        let f: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        let model = f["model"].as_str().unwrap();
        let suffix = f["our_suffix"].as_str().unwrap();
        {
            let mut s = state.lock().unwrap();
            s.response = (
                f["response"]["content_type"].as_str().unwrap().to_string(),
                unb64(f["response"]["body_b64"].as_str().unwrap()),
            );
            s.seen.clear();
        }

        let ours = run_ours(&name, model, format!("{url}{suffix}")).await;
        let (path, body, ct) = state.lock().unwrap().seen.last().cloned().unwrap();

        // The URL.
        let rig_path = f["request"]["path"].as_str().unwrap();
        let rig_path = rig_path.split('?').next().unwrap();
        let expected_path = if name.ends_with("huggingface") {
            format!("{suffix}{rig_path}")
        } else {
            rig_path.to_string()
        };
        if path != expected_path {
            failures.push(format!("{name}: path {path} != {expected_path}"));
        }

        // The request.
        if let Some(fields) = f["request"].get("multipart") {
            let got = multipart_fields(&body, &ct);
            if &got != fields {
                failures.push(format!("{name}: multipart {got} != {fields}"));
            }
        } else {
            let mut got: Value = serde_json::from_slice(&body).unwrap();
            let mut want = f["request"]["body"].clone();
            strip_nulls(&mut got);
            strip_nulls(&mut want);
            if got != want {
                failures.push(format!("{name}: body\n ours: {got}\n  rig: {want}"));
            }
        }

        // The result. vizier turned OpenAI-shaped MP3 into WAV after rig returned it.
        let rig = &f["rig_output"];
        let rig = if name == "tts_openai" || name == "tts_openrouter" {
            json!(b64(&mp3_to_wav(&unb64(rig.as_str().unwrap())).unwrap()))
        } else {
            rig.clone()
        };
        if ours != rig {
            failures.push(format!("{name}: output {ours} != {rig}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
