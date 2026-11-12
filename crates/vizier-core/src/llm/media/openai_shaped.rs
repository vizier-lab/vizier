//! OpenAI-shaped media endpoints: transcription, speech, image generation and embeddings, as
//! OpenAI and the providers that copy its API serve them. Request fields and defaults are
//! ported from the matching rig-core 0.39 modules (openai, groq, mistral, openrouter, xai,
//! together, copilot, voyageai); where a provider's endpoint differs, the difference is a
//! field set by the factory, never a provider check here.

use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use serde_json::{Value, json};

use crate::embedding::VizierEmbeddingModel;
use crate::image_generation::{ReferenceImage, VizierImageGenModel};
use crate::llm::auth::{CredentialSource, RequestInfo};
use crate::llm::dialect::join_url;
use crate::llm::error::LlmError;
use crate::llm::http;
use crate::llm::media::data_url;
use crate::stt::VizierSttModel;
use crate::tts::{VizierTtsModel, mp3_to_wav};
use crate::{Result, VizierError};

/// Where and how to reach one provider's media API.
#[derive(Clone)]
pub struct Endpoint {
    pub http: reqwest::Client,
    pub base_url: String,
    pub credential: Arc<dyn CredentialSource>,
    pub model: String,
}

impl Endpoint {
    pub(crate) async fn post(&self, path: &str) -> std::result::Result<reqwest::RequestBuilder, LlmError> {
        let base = self
            .credential
            .base_url()
            .await?
            .unwrap_or_else(|| self.base_url.clone());
        let req = self
            .http
            .post(join_url(&base, path))
            .timeout(Duration::from_secs(300));
        self.credential.authorize(req, &RequestInfo::default()).await
    }

    /// POST a JSON body, serializing it once so a body-aware credential (AWS SigV4) signs the
    /// exact bytes sent. The unchanged clients keep [`Endpoint::post`].
    pub(crate) async fn post_json(
        &self,
        path: &str,
        body: &Value,
    ) -> std::result::Result<http::HttpResponse, LlmError> {
        let base = self
            .credential
            .base_url()
            .await?
            .unwrap_or_else(|| self.base_url.clone());
        let url = join_url(&base, path);
        let bytes = serde_json::to_vec(body)
            .map_err(|e| LlmError::Decode(format!("cannot serialize the request body: {e}")))?;
        let req = self
            .http
            .post(&url)
            .timeout(Duration::from_secs(300))
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(reqwest::header::ACCEPT, "application/json")
            .body(bytes.clone());
        let req = self
            .credential
            .authorize_with_body(req, &RequestInfo::default(), "POST", &url, &bytes)
            .await?;
        http::send(req).await
    }
}

fn err(what: &str, e: LlmError) -> VizierError {
    VizierError(format!("{what}: {e}"))
}

// Speech to text -------------------------------------------------------------------------------

/// `POST {base}/audio/transcriptions`, multipart `file`, `model` and optional `language`.
pub struct OpenAiStt(pub Endpoint);

#[async_trait::async_trait]
impl VizierSttModel for OpenAiStt {
    async fn transcribe(&self, audio: &[u8], filename: &str, language: Option<&str>) -> Result<String> {
        let mut form = reqwest::multipart::Form::new()
            .text("model", self.0.model.clone())
            .part(
                "file",
                reqwest::multipart::Part::bytes(audio.to_vec()).file_name(filename.to_string()),
            );
        if let Some(language) = language {
            form = form.text("language", language.to_string());
        }
        let req = self.0.post("/audio/transcriptions").await.map_err(|e| err("transcription", e))?;
        let response = http::send_multipart(req, form)
            .await
            .map_err(|e| err("transcription", e))?;
        let body = response.json().map_err(|e| err("transcription", e))?;
        if let Some(e) = crate::llm::dialect::error_in_body(&body) {
            return Err(VizierError(format!("transcription: {e}")));
        }
        body.get("text")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| VizierError(format!("transcription: no text in {body}")))
    }
}

// Text to speech -------------------------------------------------------------------------------

/// `POST {base}/audio/speech` with `{model, input, voice, speed}`; the MP3 reply becomes WAV.
pub struct OpenAiTts {
    pub endpoint: Endpoint,
    /// OpenRouter needs the format named; OpenAI defaults to MP3.
    pub response_format: Option<&'static str>,
}

#[async_trait::async_trait]
impl VizierTtsModel for OpenAiTts {
    async fn generate_speech(&self, text: &str, voice: &str, speed: f32) -> Result<Vec<u8>> {
        let body = speech_body(&self.endpoint.model, text, voice, speed, self.response_format);
        let req = self.endpoint.post("/audio/speech").await.map_err(|e| err("speech", e))?;
        let response = http::send(req.json(&body)).await.map_err(|e| err("speech", e))?;
        mp3_to_wav(&response.body)
    }
}

/// The speech request body. `speed` is sent only when it is not the neutral 1.0: a model
/// without speed control may reject the field, and an unset speed must never become one
/// (research R5).
fn speech_body(
    model: &str,
    text: &str,
    voice: &str,
    speed: f32,
    response_format: Option<&str>,
) -> Value {
    let mut body = json!({
        "model": model,
        "input": text,
        "voice": voice,
    });
    if speed != 1.0 {
        body["speed"] = json!(speed);
    }
    if let Some(format) = response_format {
        body["response_format"] = json!(format);
    }
    body
}

/// xAI's own speech API: `POST {base}/tts` with `{text, voice_id, language}`; no model field,
/// and the audio comes back as it is.
pub struct XaiTts(pub Endpoint);

#[async_trait::async_trait]
impl VizierTtsModel for XaiTts {
    async fn generate_speech(&self, text: &str, voice: &str, _speed: f32) -> Result<Vec<u8>> {
        let voice = if voice.is_empty() { "eve" } else { voice };
        let body = json!({"text": text, "voice_id": voice, "language": "en"});
        let req = self.0.post("/tts").await.map_err(|e| err("speech", e))?;
        let response = http::send(req.json(&body)).await.map_err(|e| err("speech", e))?;
        Ok(response.body)
    }
}

// Image generation -----------------------------------------------------------------------------

/// How a provider sizes images: OpenAI takes `size: "WxH"`, xAI only an aspect ratio.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ImageSizing {
    Size,
    AspectRatio(&'static str),
}

/// Which image API to call: OpenAI's `/images/generations`, or OpenRouter's `/images`, which
/// takes the router's own `size` convenience and reports the returned `media_type`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ImageApi {
    Generations(ImageSizing),
    OpenRouter,
}

/// `POST {base}/images/generations` (OpenAI, xAI) or `{base}/images` (OpenRouter), decoding
/// `b64_json` (or fetching `url`).
pub struct OpenAiImages {
    pub endpoint: Endpoint,
    pub api: ImageApi,
}

/// rig 0.39's default when no size was asked for.
const DEFAULT_SIZE: (u32, u32) = (256, 256);

fn parse_size(size: Option<&str>) -> (u32, u32) {
    size.and_then(|s| s.split_once('x'))
        .and_then(|(w, h)| Some((w.trim().parse().ok()?, h.trim().parse().ok()?)))
        .unwrap_or(DEFAULT_SIZE)
}

/// The path and body for one image request, from the factory-set API variant (research R4).
///
/// A request carrying reference images becomes an edit: `/images/edits` for OpenAI and xAI,
/// and the same `/images` call with `input_references` for OpenRouter. With no references the
/// text-only bodies are exactly what they were.
fn image_request(
    api: ImageApi,
    model: &str,
    prompt: &str,
    size: Option<&str>,
    references: &[ReferenceImage],
) -> Result<(&'static str, Value)> {
    if !references.is_empty() {
        return edit_request(api, model, prompt, size, references);
    }
    let mut body = json!({"model": model, "prompt": prompt});
    let path = match api {
        ImageApi::Generations(ImageSizing::Size) => {
            let (w, h) = parse_size(size);
            body["size"] = json!(format!("{w}x{h}"));
            // The GPT image models always answer in base64 and refuse the parameter.
            if !model.starts_with("gpt-image") {
                body["response_format"] = json!("b64_json");
            }
            "/images/generations"
        }
        ImageApi::Generations(ImageSizing::AspectRatio(ratio)) => {
            body["response_format"] = json!("b64_json");
            body["aspect_ratio"] = json!(ratio);
            "/images/generations"
        }
        ImageApi::OpenRouter => {
            // OpenRouter normalizes the router's `size` convenience per provider, so it is
            // passed through as given and omitted when nobody asked for one.
            if let Some(size) = size {
                body["size"] = json!(size);
            }
            "/images"
        }
    };
    Ok((path, body))
}

/// The edit request for one API variant, given at least one reference image.
fn edit_request(
    api: ImageApi,
    model: &str,
    prompt: &str,
    size: Option<&str>,
    references: &[ReferenceImage],
) -> Result<(&'static str, Value)> {
    let mut body = json!({"model": model, "prompt": prompt});
    let path = match api {
        ImageApi::Generations(ImageSizing::Size) => {
            // OpenAI's edit models are the GPT image models, which take input images as JSON
            // references and default `size` to `auto`. Sending our text default would be
            // rejected, so a size is sent only when the caller asked for one; the same applies
            // to `response_format`, which these models refuse.
            body["images"] = json!(references
                .iter()
                .map(|r| json!({"image_url": data_url(&r.mime, &r.data)}))
                .collect::<Vec<_>>());
            if let Some(size) = size {
                body["size"] = json!(size);
            }
            "/images/edits"
        }
        ImageApi::OpenRouter => {
            body["input_references"] = json!(references
                .iter()
                .map(|r| json!({"type": "image_url", "image_url": {"url": data_url(&r.mime, &r.data)}}))
                .collect::<Vec<_>>());
            if let Some(size) = size {
                body["size"] = json!(size);
            }
            "/images"
        }
        ImageApi::Generations(ImageSizing::AspectRatio(_)) => {
            // xAI's edit call takes one source image and, by default, keeps its aspect ratio;
            // forcing the text call's ratio would defeat an edit, and the endpoint documents
            // `aspect_ratio` rather than `size`, so neither is sent.
            let [only] = references else {
                return Err(VizierError(
                    "image generation: xAI accepts one reference image per request".into(),
                ));
            };
            body["image"] = json!({
                "url": data_url(&only.mime, &only.data),
                "type": "image_url",
            });
            "/images/edits"
        }
    };
    Ok((path, body))
}

/// The media type of a base64 image reply: what the provider says, else PNG as before.
fn media_type_of(first: &Value) -> String {
    first
        .get("media_type")
        .and_then(Value::as_str)
        .filter(|m| m.starts_with("image/"))
        .unwrap_or("image/png")
        .to_string()
}

#[async_trait::async_trait]
impl VizierImageGenModel for OpenAiImages {
    async fn generate(
        &self,
        prompt: &str,
        size: Option<&str>,
        references: &[ReferenceImage],
    ) -> Result<(Vec<u8>, String)> {
        let (path, body) = image_request(self.api, &self.endpoint.model, prompt, size, references)?;
        let req = self
            .endpoint
            .post(path)
            .await
            .map_err(|e| err("image generation", e))?;
        let reply = http::send_json(req.json(&body))
            .await
            .map_err(|e| err("image generation", e))?;
        let first = reply
            .pointer("/data/0")
            .ok_or_else(|| VizierError("image generation: missing image data".into()))?;
        if let Some(b64) = first.get("b64_json").and_then(Value::as_str) {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(b64)
                .map_err(|e| VizierError(format!("image generation: {e}")))?;
            return Ok((bytes, media_type_of(first)));
        }
        if let Some(url) = first.get("url").and_then(Value::as_str) {
            let image = http::send(self.endpoint.http.get(url))
                .await
                .map_err(|e| err("image download", e))?;
            let mime = image
                .headers
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .filter(|m| m.starts_with("image/"))
                .unwrap_or("image/png")
                .to_string();
            return Ok((image.body, mime));
        }
        Err(VizierError("image generation: missing image data".into()))
    }
}

// Embeddings -----------------------------------------------------------------------------------

/// `POST {base}/embeddings` with `{model, input}`, plus `dimensions` when the factory knows
/// them (rig 0.39 sent the model's native size for the OpenAI-family models).
pub struct OpenAiEmbeddings {
    pub endpoint: Endpoint,
    pub dimensions: Option<usize>,
}

/// The `dimensions` rig 0.39 sent for an OpenAI-family embedding model: known sizes only, and
/// never for `text-embedding-ada-002`, which refuses the parameter.
pub fn openai_dimensions(model: &str) -> Option<usize> {
    match model {
        "text-embedding-3-large" => Some(3072),
        "text-embedding-3-small" => Some(1536),
        _ => None,
    }
}

pub(crate) fn order_by_index(items: &[Value], count: usize) -> std::result::Result<Vec<Vec<f64>>, String> {
    let mut indexed: Vec<(usize, Vec<f64>)> = items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let index = item.get("index").and_then(Value::as_u64).map_or(i, |n| n as usize);
            let vector = item
                .get("embedding")
                .and_then(Value::as_array)
                .map(|v| v.iter().filter_map(Value::as_f64).collect())
                .unwrap_or_default();
            (index, vector)
        })
        .collect();
    if indexed.len() != count {
        return Err(format!(
            "the response holds {} embeddings for {count} inputs",
            indexed.len()
        ));
    }
    indexed.sort_by_key(|(i, _)| *i);
    Ok(indexed.into_iter().map(|(_, v)| v).collect())
}

#[async_trait::async_trait]
impl VizierEmbeddingModel for OpenAiEmbeddings {
    async fn embed_text(&self, text: &str) -> anyhow::Result<Vec<f64>> {
        let mut all = self.embed_texts(vec![text.to_string()]).await?;
        all.pop()
            .ok_or_else(|| anyhow::anyhow!("embeddings: no vector returned"))
    }

    async fn embed_texts(&self, documents: Vec<String>) -> anyhow::Result<Vec<Vec<f64>>> {
        if documents.is_empty() {
            return Ok(vec![]);
        }
        let mut body = json!({"model": self.endpoint.model, "input": documents});
        if let Some(dims) = self.dimensions {
            body["dimensions"] = json!(dims);
        }
        let req = self
            .endpoint
            .post("/embeddings")
            .await
            .map_err(|e| anyhow::anyhow!("embeddings: {e}"))?;
        let reply = http::send_json(req.json(&body))
            .await
            .map_err(|e| anyhow::anyhow!("embeddings: {e}"))?;
        let data = reply
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow::anyhow!("embeddings: no data in the response"))?;
        order_by_index(data, documents.len()).map_err(|e| anyhow::anyhow!("embeddings: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_default_like_rig() {
        assert_eq!(parse_size(None), (256, 256));
        assert_eq!(parse_size(Some("1024x768")), (1024, 768));
        assert_eq!(parse_size(Some("big")), (256, 256));
    }

    /// Research R4: OpenRouter's `/images` takes the router's `size` as given, never
    /// `response_format`, and omits the size when nobody asked for one.
    #[test]
    fn openrouter_image_requests_go_to_images_with_the_size_as_given() {
        let (path, body) =
            image_request(ImageApi::OpenRouter, "bytedance-seed/seedream-4.5", "a fox", None, &[])
                .unwrap();
        assert_eq!(path, "/images");
        assert_eq!(body, json!({"model": "bytedance-seed/seedream-4.5", "prompt": "a fox"}));

        let (_, body) =
            image_request(ImageApi::OpenRouter, "m", "a fox", Some("1792x1024"), &[]).unwrap();
        assert_eq!(body["size"], json!("1792x1024"));
        assert!(body.get("response_format").is_none(), "{body}");

        // A tier is passed through too: OpenRouter normalizes it per provider.
        let (_, body) = image_request(ImageApi::OpenRouter, "m", "a fox", Some("2K"), &[]).unwrap();
        assert_eq!(body["size"], json!("2K"));
    }

    /// The OpenAI and xAI bodies are unchanged by the `ImageApi` split.
    #[test]
    fn generation_requests_keep_their_size_and_response_format() {
        let (path, body) =
            image_request(ImageApi::Generations(ImageSizing::Size), "dall-e-3", "a fox", None, &[])
                .unwrap();
        assert_eq!(path, "/images/generations");
        assert_eq!(body["size"], json!("256x256"));
        assert_eq!(body["response_format"], json!("b64_json"));

        let (_, gpt) = image_request(
            ImageApi::Generations(ImageSizing::Size),
            "gpt-image-1",
            "a fox",
            Some("1024x1024"),
            &[],
        )
        .unwrap();
        assert_eq!(gpt["size"], json!("1024x1024"));
        assert!(gpt.get("response_format").is_none(), "{gpt}");

        let (_, xai) = image_request(
            ImageApi::Generations(ImageSizing::AspectRatio("16:9")),
            "grok",
            "a fox",
            None,
            &[],
        )
        .unwrap();
        assert_eq!(xai["aspect_ratio"], json!("16:9"));
        assert_eq!(xai["response_format"], json!("b64_json"));
        assert!(xai.get("size").is_none(), "{xai}");
    }

    fn one_reference() -> ReferenceImage {
        ReferenceImage {
            mime: "image/png".into(),
            data: vec![1, 2, 3],
        }
    }

    /// OpenAI's edit call takes JSON references, and never the text call's `256x256` default or
    /// `response_format`, which the GPT image models refuse.
    #[test]
    fn openai_edits_send_references_and_only_an_explicit_size() {
        let reference = one_reference();
        let (path, body) = image_request(
            ImageApi::Generations(ImageSizing::Size),
            "gpt-image-1",
            "make it blue",
            None,
            std::slice::from_ref(&reference),
        )
        .unwrap();
        assert_eq!(path, "/images/edits");
        assert_eq!(body["prompt"], json!("make it blue"));
        assert_eq!(body["images"], json!([{"image_url": "data:image/png;base64,AQID"}]));
        assert!(body.get("size").is_none(), "{body}");
        assert!(body.get("response_format").is_none(), "{body}");

        let (_, sized) = image_request(
            ImageApi::Generations(ImageSizing::Size),
            "gpt-image-1",
            "make it blue",
            Some("1536x1024"),
            &[reference],
        )
        .unwrap();
        assert_eq!(sized["size"], json!("1536x1024"));
    }

    #[test]
    fn openrouter_edits_keep_the_images_path_and_send_input_references() {
        let (path, body) = image_request(
            ImageApi::OpenRouter,
            "google/gemini-2.5-flash-image",
            "make it blue",
            None,
            &[one_reference()],
        )
        .unwrap();
        assert_eq!(path, "/images");
        assert_eq!(
            body["input_references"],
            json!([{"type": "image_url", "image_url": {"url": "data:image/png;base64,AQID"}}])
        );
        assert!(body.get("size").is_none(), "{body}");
    }

    /// xAI edits are JSON with one `image`, at `/images/edits`; a second reference is refused
    /// rather than sent to an endpoint that documents only one.
    #[test]
    fn xai_edits_take_one_reference_image() {
        let api = ImageApi::Generations(ImageSizing::AspectRatio("1:1"));
        let (path, body) = image_request(
            api,
            "grok-imagine-image-2.0",
            "pencil sketch",
            Some("1024x768"),
            &[one_reference()],
        )
        .unwrap();
        assert_eq!(path, "/images/edits");
        assert_eq!(
            body["image"],
            json!({"url": "data:image/png;base64,AQID", "type": "image_url"})
        );
        // The edit keeps the source image's shape; the text call's ratio and size would fight it.
        assert!(body.get("aspect_ratio").is_none(), "{body}");
        assert!(body.get("size").is_none(), "{body}");

        let error = image_request(
            api,
            "grok-imagine-image-2.0",
            "pencil sketch",
            None,
            &[one_reference(), one_reference()],
        )
        .unwrap_err();
        assert!(error.0.contains("one reference image"), "{error:?}");
    }

    #[test]
    fn a_base64_image_keeps_the_media_type_it_reports() {
        assert_eq!(media_type_of(&json!({"b64_json": "x", "media_type": "image/webp"})), "image/webp");
        assert_eq!(media_type_of(&json!({"b64_json": "x", "media_type": "image/svg+xml"})), "image/svg+xml");
        assert_eq!(media_type_of(&json!({"b64_json": "x"})), "image/png");
        assert_eq!(media_type_of(&json!({"b64_json": "x", "media_type": "text/plain"})), "image/png");
    }

    /// Research R5: a model without speed control must not reject an unset speed, so the field
    /// is omitted at 1.0 and sent for anything else.
    #[test]
    fn the_speech_body_omits_the_neutral_speed() {
        let body = speech_body("hexgrad/kokoro-82m", "hi", "af_alloy", 1.0, Some("mp3"));
        assert_eq!(body, json!({"model": "hexgrad/kokoro-82m", "input": "hi", "voice": "af_alloy", "response_format": "mp3"}));
        assert!(body.get("speed").is_none(), "{body}");

        let body = speech_body("m", "hi", "v", 1.25, None);
        assert_eq!(body["speed"], json!(1.25));
        assert!(body.get("response_format").is_none(), "{body}");
    }

    #[test]
    fn embeddings_are_returned_in_input_order() {
        let items = vec![
            json!({"index": 1, "embedding": [2.0]}),
            json!({"index": 0, "embedding": [1.0, 0.5]}),
        ];
        assert_eq!(order_by_index(&items, 2).unwrap(), vec![vec![1.0, 0.5], vec![2.0]]);
        assert!(order_by_index(&items, 3).is_err());
    }
}
