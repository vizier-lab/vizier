//! Hyperbolic speech and images, with the endpoints and bodies of rig-core 0.39
//! `providers/hyperbolic.rs`. As in rig, the configured speech "model" is the language.

use base64::Engine;
use serde_json::{Value, json};

use super::openai_shaped::Endpoint;
use crate::image_generation::VizierImageGenModel;
use crate::tts::VizierTtsModel;
use crate::{Result, VizierError};

pub const HYPERBOLIC_BASE_URL: &str = "https://api.hyperbolic.xyz/v1";

fn decode_b64(what: &str, value: Option<&Value>) -> Result<Vec<u8>> {
    let b64 = value
        .and_then(Value::as_str)
        .ok_or_else(|| VizierError(format!("{what}: missing data in the response")))?;
    base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| VizierError(format!("{what}: {e}")))
}

pub struct HyperbolicTts(pub Endpoint);

#[async_trait::async_trait]
impl VizierTtsModel for HyperbolicTts {
    async fn generate_speech(&self, text: &str, voice: &str, speed: f32) -> Result<Vec<u8>> {
        let body = json!({
            "language": self.0.model,
            "speaker": voice,
            "text": text,
            "speed": speed,
        });
        let req = self
            .0
            .post("/audio/generation")
            .await
            .map_err(|e| VizierError(format!("hyperbolic speech: {e}")))?;
        let reply = crate::llm::http::send_json(req.json(&body))
            .await
            .map_err(|e| VizierError(format!("hyperbolic speech: {e}")))?;
        decode_b64("hyperbolic speech", reply.get("audio"))
    }
}

pub struct HyperbolicImages(pub Endpoint);

#[async_trait::async_trait]
impl VizierImageGenModel for HyperbolicImages {
    async fn generate(&self, prompt: &str, size: Option<&str>) -> Result<(Vec<u8>, String)> {
        let (width, height) = size
            .and_then(|s| s.split_once('x'))
            .and_then(|(w, h)| Some((w.trim().parse::<u32>().ok()?, h.trim().parse::<u32>().ok()?)))
            .unwrap_or((256, 256));
        let body = json!({
            "model_name": self.0.model,
            "prompt": prompt,
            "height": height,
            "width": width,
        });
        let req = self
            .0
            .post("/image/generation")
            .await
            .map_err(|e| VizierError(format!("hyperbolic image generation: {e}")))?;
        let reply = crate::llm::http::send_json(req.json(&body))
            .await
            .map_err(|e| VizierError(format!("hyperbolic image generation: {e}")))?;
        let image = decode_b64("hyperbolic image generation", reply.pointer("/images/0/image"))?;
        Ok((image, "image/png".into()))
    }
}
