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

/// `POST {base}/audio/speech` with `{model, input, voice, speed}`; the reply becomes WAV.
pub struct OpenAiTts {
    pub endpoint: Endpoint,
    /// OpenRouter needs the format named; OpenAI defaults to MP3.
    pub response_format: Option<&'static str>,
    /// Some models answer only in raw PCM, and refuse `response_format: "mp3"` outright.
    ///
    /// Gemini TTS through OpenRouter is one: asking for mp3 is a `400 … only supports
    /// response_format="pcm"`, which is what a live call was doing — every segment failed, the
    /// answer stayed on screen and no sound arrived. When this is `Some(rate)` the request names
    /// `pcm` and the body is bare 16-bit mono samples at that rate, which are resampled to
    /// [`SPEECH_RATE`] here so everything downstream still sees one rate.
    pub pcm_rate: Option<u32>,
}

/// The rate every spoken segment is delivered at (`live_voice::frame::SAMPLE_RATE`).
///
/// Repeated rather than imported, so this media layer keeps no upward dependency on the live
/// voice module. The test in `pcm_tests` pins the two numbers together.
pub const SPEECH_RATE: u32 = 16_000;

#[async_trait::async_trait]
impl VizierTtsModel for OpenAiTts {
    async fn generate_speech(&self, text: &str, voice: &str, speed: f32) -> Result<Vec<u8>> {
        let body = self.body_for(text, voice, speed);
        let req = self.endpoint.post("/audio/speech").await.map_err(|e| err("speech", e))?;
        let response = http::send(req.json(&body)).await.map_err(|e| err("speech", e))?;
        match self.pcm_rate {
            Some(rate) => Ok(pcm_to_wav16(&response.body, rate)?),
            None => mp3_to_wav(&response.body),
        }
    }
}

impl OpenAiTts {
    /// The JSON body this request will carry, built without sending it.
    ///
    /// A seam, not a convenience: the format a live call names is what broke it (a model that only
    /// accepts `pcm` was asked for `mp3` and answered `400` before producing a byte), and a test
    /// that drove `speech_format` directly stayed green when this call site was reverted to
    /// `self.response_format`. Building the body here is what a test can hold onto.
    fn body_for(&self, text: &str, voice: &str, speed: f32) -> Value {
        let format = speech_format(self.pcm_rate, self.response_format);
        speech_body(&self.endpoint.model, text, voice, speed, format)
    }
}

/// Which `response_format` a speech request names.
///
/// A pure function, and not an inline `match`, because it is the decision that broke a live call:
/// a model that only accepts `pcm` was asked for `mp3` and answered `400` before producing a byte.
/// Extracting it is what lets a test fail if this line is ever changed back — the test that drove
/// `speech_body` with a literal `Some("pcm")` stayed green under exactly that mutation.
fn speech_format(pcm_rate: Option<u32>, response_format: Option<&'static str>) -> Option<&'static str> {
    match pcm_rate {
        Some(_) => Some("pcm"),
        None => response_format,
    }
}

/// A provider's bare PCM16 at `rate`, as the WAV at [`SPEECH_RATE`] every spoken segment carries.
///
/// This is the whole of the PCM path, and it is one function on purpose: a test that only drove
/// `resample` would stay green if the resampling were dropped from here, which is the failure the
/// tests exist to catch.
/// The PCM body as a WAV, or an error naming what came back instead.
///
/// The guard is the point. A provider that answers `{"audio": "…base64…"}` instead of bare
/// samples would otherwise be wrapped into a WAV of JSON bytes: the call would play noise or
/// nothing, with no line in the log saying why — the exact failure this whole change exists to
/// end. One look at the first byte turns that into a named error, and the next real call says
/// which of the two shapes OpenRouter sends.
fn pcm_to_wav16(body: &[u8], rate: u32) -> Result<Vec<u8>> {
    let head = body.iter().find(|b| !b.is_ascii_whitespace());
    if head == Some(&b'{') || head == Some(&b'[') {
        let preview: String = String::from_utf8_lossy(&body[..body.len().min(160)]).into_owned();
        return Err(VizierError(format!(
            "speech: expected raw PCM at {rate} Hz, got a JSON body: {preview}"
        )));
    }
    if body.len() < 2 {
        return Err(VizierError(format!(
            "speech: expected raw PCM at {rate} Hz, got {} byte(s)",
            body.len()
        )));
    }
    Ok(wav_from_pcm16(resample(body, rate, SPEECH_RATE), SPEECH_RATE))
}

/// Bare little-endian PCM16 mono, resampled linearly.
///
/// Linear interpolation is enough here: this is speech that will be heard once, at a 3:2 ratio for
/// the rate that needed it, and a windowed-sinc filter would cost more than it returns. An odd
/// trailing byte is dropped rather than guessed at.
fn resample(pcm: &[u8], from: u32, to: u32) -> Vec<u8> {
    if from == to || from == 0 {
        return pcm.to_vec();
    }
    let samples: Vec<i16> = pcm
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| i16::from_le_bytes(*pair))
        .collect();
    if samples.is_empty() {
        return Vec::new();
    }
    let ratio = from as f64 / to as f64;
    let count = ((samples.len() as f64) / ratio).floor() as usize;
    let mut out = Vec::with_capacity(count * 2);
    for index in 0..count {
        let at = index as f64 * ratio;
        let left = at.floor() as usize;
        let frac = at - left as f64;
        let a = samples[left.min(samples.len() - 1)] as f64;
        let b = samples[(left + 1).min(samples.len() - 1)] as f64;
        let value = (a + (b - a) * frac).round().clamp(-32768.0, 32767.0) as i16;
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

/// A canonical 44-byte WAV header around `pcm`, at `rate`. Mono, 16-bit, as everywhere else.
fn wav_from_pcm16(pcm: Vec<u8>, rate: u32) -> Vec<u8> {
    let data = pcm.len() as u32;
    let mut header = [0u8; 44];
    header[0..4].copy_from_slice(b"RIFF");
    header[4..8].copy_from_slice(&(36 + data).to_le_bytes());
    header[8..12].copy_from_slice(b"WAVE");
    header[12..16].copy_from_slice(b"fmt ");
    header[16..20].copy_from_slice(&16u32.to_le_bytes());
    header[20..22].copy_from_slice(&1u16.to_le_bytes());
    header[22..24].copy_from_slice(&1u16.to_le_bytes());
    header[24..28].copy_from_slice(&rate.to_le_bytes());
    header[28..32].copy_from_slice(&(rate * 2).to_le_bytes());
    header[32..34].copy_from_slice(&2u16.to_le_bytes());
    header[34..36].copy_from_slice(&16u16.to_le_bytes());
    header[36..40].copy_from_slice(b"data");
    header[40..44].copy_from_slice(&data.to_le_bytes());
    let mut out = header.to_vec();
    out.extend_from_slice(&pcm);
    out
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
    /// Venice's OpenAI-compatible `/images/generations`: `size` from a fixed list (omitted for
    /// its own default), always base64, and no reference images.
    Venice,
    /// Together's `/images/generations`: pixel `width`/`height` and `response_format: "base64"`
    /// (not `b64_json`); its reference fields take URLs only, so references are refused.
    Together,
}

/// `POST {base}/images/generations` (OpenAI, xAI, Venice, Together) or `{base}/images`
/// (OpenRouter), decoding `b64_json` (or fetching `url`).
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
        ImageApi::Venice => {
            // Venice's default is `auto` (1024x1024); rig's 256x256 text default would shrink
            // every image, so a size is sent only when one was asked for.
            body["response_format"] = json!("b64_json");
            if let Some(size) = size {
                body["size"] = json!(size);
            }
            "/images/generations"
        }
        ImageApi::Together => {
            body["response_format"] = json!("base64");
            // Together defaults to 1024x1024; pixels are sent only for a size that parses.
            if let Some((w, h)) = size.and_then(parse_dimensions) {
                body["width"] = json!(w);
                body["height"] = json!(h);
            }
            "/images/generations"
        }
    };
    Ok((path, body))
}

/// `"WxH"` as pixels, or `None` when it is not two whole numbers.
fn parse_dimensions(size: &str) -> Option<(u32, u32)> {
    let (w, h) = size.split_once('x')?;
    Some((w.trim().parse().ok()?, h.trim().parse().ok()?))
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
        ImageApi::Venice | ImageApi::Together => {
            let name = if api == ImageApi::Venice { "Venice" } else { "Together AI" };
            return Err(VizierError(format!(
                "image generation: {name} does not accept reference images here; \
                 describe the change in the prompt instead"
            )));
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

/// The media type of a base64 image reply: what the provider says, else what the bytes are
/// (Together answers JPEG by default), else PNG as before.
fn media_type_of(first: &Value, bytes: &[u8]) -> String {
    first
        .get("media_type")
        .and_then(Value::as_str)
        .filter(|m| m.starts_with("image/"))
        .or_else(|| crate::llm::media::sniff_image(bytes))
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
            let mime = media_type_of(first, &bytes);
            return Ok((bytes, mime));
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
        assert_eq!(media_type_of(&json!({"b64_json": "x", "media_type": "image/webp"}), b""), "image/webp");
        assert_eq!(media_type_of(&json!({"b64_json": "x", "media_type": "image/svg+xml"}), b""), "image/svg+xml");
        assert_eq!(media_type_of(&json!({"b64_json": "x"}), b""), "image/png");
        assert_eq!(media_type_of(&json!({"b64_json": "x", "media_type": "text/plain"}), b""), "image/png");
    }

    /// With no `media_type`, the bytes decide: Together's default output is JPEG.
    #[test]
    fn an_unlabelled_base64_image_is_typed_by_its_bytes() {
        assert_eq!(media_type_of(&json!({"b64_json": "x"}), &[0xff, 0xd8, 0xff, 0]), "image/jpeg");
        assert_eq!(media_type_of(&json!({"b64_json": "x"}), b"RIFF1234WEBP"), "image/webp");
        // A stated type still wins over the bytes.
        assert_eq!(
            media_type_of(&json!({"media_type": "image/png"}), &[0xff, 0xd8, 0xff, 0]),
            "image/png"
        );
    }

    #[test]
    fn venice_asks_for_base64_and_sends_a_size_only_when_asked() {
        let (path, body) = image_request(ImageApi::Venice, "venice-sd35", "a fox", None, &[]).unwrap();
        assert_eq!(path, "/images/generations");
        assert_eq!(
            body,
            json!({"model": "venice-sd35", "prompt": "a fox", "response_format": "b64_json"})
        );
        let (_, sized) =
            image_request(ImageApi::Venice, "venice-sd35", "a fox", Some("1024x1792"), &[]).unwrap();
        assert_eq!(sized["size"], json!("1024x1792"));
    }

    #[test]
    fn together_sends_pixels_and_names_base64_its_own_way() {
        let model = "black-forest-labs/FLUX.1-schnell";
        let (path, body) = image_request(ImageApi::Together, model, "a fox", None, &[]).unwrap();
        assert_eq!(path, "/images/generations");
        assert_eq!(body, json!({"model": model, "prompt": "a fox", "response_format": "base64"}));

        let (_, sized) =
            image_request(ImageApi::Together, model, "a fox", Some("1280x720"), &[]).unwrap();
        assert_eq!(sized["width"], json!(1280));
        assert_eq!(sized["height"], json!(720));
        assert!(sized.get("size").is_none(), "{sized}");

        // A size that is not WxH is left to Together's default rather than sent.
        let (_, odd) = image_request(ImageApi::Together, model, "a fox", Some("2K"), &[]).unwrap();
        assert!(odd.get("width").is_none(), "{odd}");
    }

    #[test]
    fn venice_and_together_refuse_reference_images() {
        for (api, name) in [(ImageApi::Venice, "Venice"), (ImageApi::Together, "Together AI")] {
            let error = image_request(api, "m", "make it blue", None, &[one_reference()]).unwrap_err();
            assert!(error.0.contains(name), "{error:?}");
            assert!(error.0.contains("reference images"), "{error:?}");
        }
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

#[cfg(test)]
mod pcm_tests {
    //! A speech model that answers only in PCM (`Gemini TTS only supports response_format="pcm"`).
    //!
    //! Found in a live call at 16:33: every segment came back `400`, so the answer sat on screen
    //! with no sound and the state never left `listening`. These tests pin the two halves of the
    //! fix: the request names `pcm`, and the bare samples come back at the rate everything else
    //! expects.

    use super::*;

    #[test]
    fn a_gemini_tts_model_is_asked_for_pcm_and_nothing_else_is() {
        assert_eq!(crate::tts::gemini_pcm_rate("google/gemini-3.1-flash-tts-preview"), Some(24_000));
        assert_eq!(crate::tts::gemini_pcm_rate("GEMINI-2.5-flash-tts"), Some(24_000));
        assert_eq!(crate::tts::gemini_pcm_rate("hexgrad/kokoro-82m"), None);
        assert_eq!(crate::tts::gemini_pcm_rate("tts-1"), None);
        assert_eq!(crate::tts::gemini_pcm_rate("gemini-3.5-transcribe"), None);
    }

    #[test]
    fn the_request_names_pcm_when_the_model_needs_it() {
        let body = speech_body("google/gemini-3.1-flash-tts-preview", "hi", "Kore", 1.0, Some("pcm"));
        assert_eq!(body["response_format"], "pcm");
        let mp3 = speech_body("hexgrad/kokoro-82m", "hi", "af_alloy", 1.0, Some("mp3"));
        assert_eq!(mp3["response_format"], "mp3");
    }

    #[test]
    fn the_pcm_path_actually_resamples_and_does_not_trust_the_rate_it_was_told() {
        // The failure this catches: dropping the resample from `pcm_to_wav16` leaves every
        // other test in this module green, because they drive `resample` directly. Gemini
        // answers 24 kHz and every segment is delivered at 16 kHz, so the byte count is the
        // proof: 3·N samples in, 2·N samples out, and the header says the rate it was given.
        let pcm: Vec<u8> = (0..300i16).flat_map(|s| (s * 10).to_le_bytes()).collect();
        let wav = pcm_to_wav16(&pcm, 24_000).expect("raw PCM is the happy path");
        let data = &wav[44..];
        assert_eq!(data.len(), 200 * 2, "24 kHz to 16 kHz is 3:2");
        assert_eq!(
            u32::from_le_bytes([wav[24], wav[25], wav[26], wav[27]]),
            SPEECH_RATE,
            "the header must name the rate the samples are actually at"
        );
    }

    #[test]
    fn twenty_four_kilocycles_become_sixteen_without_a_click() {
        // 240 samples at 24 kHz is 10 ms; at 16 kHz the same interval is 160 samples.
        let samples: Vec<i16> = (0..240).map(|i| (i * 100) as i16).collect();
        let pcm: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        let out = resample(&pcm, 24_000, 16_000);
        assert_eq!(out.len(), 160 * 2, "the ratio is 3:2 and an odd byte is never guessed at");
        // A linear ramp stays a ramp: no sample may be out of the input's range.
        let peak = out
            .as_chunks::<2>().0.iter()
            .map(|p| i16::from_le_bytes([p[0], p[1]]))
            .max()
            .unwrap();
        assert!(peak <= 239 * 100, "resampling must not invent peaks: {peak}");
    }

    #[test]
    fn the_wav_header_says_the_rate_everything_else_reads() {
        let wav = wav_from_pcm16(vec![0u8; 320], SPEECH_RATE);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes([wav[24], wav[25], wav[26], wav[27]]), SPEECH_RATE);
        assert_eq!(u32::from_le_bytes([wav[40], wav[41], wav[42], wav[43]]), 320);
        // ...and it is the rate a spoken segment is delivered at, which is the whole point.
        assert_eq!(SPEECH_RATE, crate::live_voice::frame::SAMPLE_RATE);
    }


    #[test]
    fn a_json_body_is_named_not_wrapped_into_noise() {
        // The shape this guards against: a provider that answers base64 inside JSON. Wrapping that
        // into a WAV would play noise with nothing in the log saying why, which is the failure this
        // whole change exists to end.
        let body = br#"{"audio": "AAAA"}"#;
        let err = pcm_to_wav16(body, 24_000).expect_err("a JSON body is not PCM");
        assert!(err.to_string().contains("JSON body"), "{err}");
        assert!(err.to_string().contains("24000"), "the rate it expected is named: {err}");
    }

    #[test]
    fn an_empty_body_is_an_error_rather_than_a_silent_segment() {
        let err = pcm_to_wav16(&[], 24_000).expect_err("nothing is not audio");
        assert!(err.to_string().contains("0 byte"), "{err}");
    }

    #[test]
    fn real_samples_still_become_a_wav() {
        let pcm: Vec<u8> = (0..480i16).flat_map(|s| (s * 10).to_le_bytes()).collect();
        let wav = pcm_to_wav16(&pcm, 24_000).expect("raw PCM is the happy path");
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes([wav[24], wav[25], wav[26], wav[27]]), SPEECH_RATE);
    }

    #[test]
    fn an_already_matching_rate_is_left_alone() {
        // A ramp, not a constant: 64 identical bytes are returned unchanged by almost any
        // resampler, including one that rounds wrongly. This asserts the bytes a real provider
        // sends come back bit-for-bit when the rate already matches.
        let pcm: Vec<u8> = (0..64i16).flat_map(|s| (s * 7).to_le_bytes()).collect();
        assert_eq!(resample(&pcm, 16_000, 16_000), pcm);
    }

    #[test]
    fn the_request_format_follows_the_rate_and_not_the_provider_name() {
        // Driven through the body the request would actually carry. Reverting the call site to
        // `self.response_format` — the exact regression — turns this red; a test that called
        // `speech_format` directly did not.
        let at = |model: &str| Endpoint {
            http: reqwest::Client::new(),
            base_url: "http://127.0.0.1:1".into(),
            credential: crate::llm::media::StaticKey::shared(
                crate::llm::media::KeyStyle::Bearer,
                Some("test-key".into()),
            ),
            model: model.to_string(),
        };
        let pcm_model = OpenAiTts {
            endpoint: at("google/gemini-3.1-flash-tts-preview"),
            response_format: Some("mp3"),
            pcm_rate: Some(24_000),
        };
        assert_eq!(pcm_model.body_for("hi", "Kore", 1.0)["response_format"], "pcm");

        let mp3_model = OpenAiTts {
            endpoint: at("hexgrad/kokoro-82m"),
            response_format: Some("mp3"),
            pcm_rate: None,
        };
        assert_eq!(mp3_model.body_for("hi", "af_alloy", 1.0)["response_format"], "mp3");
    }
}
