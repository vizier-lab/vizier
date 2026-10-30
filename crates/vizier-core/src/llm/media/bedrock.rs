//! Bedrock's media clients: memory embeddings and image generation, both through `InvokeModel`
//! on the runtime endpoint (research R6/R7, contract `bedrock-wire.md` §5–6). One file because
//! both families share the endpoint, the signer and the JSON transport.

use base64::Engine;
use serde_json::{Value, json};

use crate::Result;
use crate::VizierError;
use crate::embedding::VizierEmbeddingModel;
use crate::image_generation::VizierImageGenModel;
use crate::llm::bedrock::model_path;
use crate::llm::media::Endpoint;

/// The documented default when an agent leaves the embedding model empty.
pub const DEFAULT_EMBEDDING_MODEL: &str = "amazon.titan-embed-text-v2:0";
/// The documented default when an agent leaves the image model empty.
pub const DEFAULT_IMAGE_MODEL: &str = "amazon.nova-canvas-v1:0";
/// The documented Cohere batch maximum on Bedrock.
const COHERE_BATCH: usize = 96;
/// The documented default image size (both families accept it).
const DEFAULT_IMAGE_SIZE: (u32, u32) = (1024, 1024);

/// Region-prefixed model ids (`us.`, `eu.`, `apac.`, `global.`) name the same family.
fn family_id(model: &str) -> &str {
    for prefix in ["us.", "eu.", "apac.", "global."] {
        if let Some(rest) = model.strip_prefix(prefix) {
            return rest;
        }
    }
    model
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddingFamily {
    TitanV2,
    TitanV1,
    CohereV3,
}

impl EmbeddingFamily {
    /// Family from a model id, cross-region prefix ignored. `None` names no supported family.
    pub fn detect(model: &str) -> Option<Self> {
        let id = family_id(model);
        if id == DEFAULT_EMBEDDING_MODEL {
            Some(Self::TitanV2)
        } else if id.starts_with("amazon.titan-embed-text-v1") {
            Some(Self::TitanV1)
        } else if id.starts_with("cohere.embed-") {
            Some(Self::CohereV3)
        } else {
            None
        }
    }

    pub fn supported() -> &'static str {
        "Amazon Titan Text Embeddings V2 (amazon.titan-embed-text-v2:0), \
         Titan Text Embeddings V1 (amazon.titan-embed-text-v1) and Cohere Embed v3 \
         (cohere.embed-english-v3, cohere.embed-multilingual-v3)"
    }
}

/// Bedrock embeddings: Titan embeds one text per call; Cohere batches.
pub struct BedrockEmbeddings {
    pub endpoint: Endpoint,
    pub family: EmbeddingFamily,
}

impl BedrockEmbeddings {
    /// Build the client, refusing a model outside the supported families before any request.
    pub fn new(endpoint: Endpoint) -> Result<Self> {
        let family = EmbeddingFamily::detect(&endpoint.model).ok_or_else(|| {
            VizierError(format!(
                "unsupported Bedrock embedding model {:?}: use {}",
                endpoint.model,
                EmbeddingFamily::supported()
            ))
        })?;
        Ok(Self { endpoint, family })
    }

    async fn invoke(&self, body: &Value) -> anyhow::Result<Value> {
        let response = self
            .endpoint
            .post_json(&model_path(&self.endpoint.model, "invoke"), body)
            .await
            .map_err(|e| anyhow::anyhow!("embeddings: {e}"))?;
        response
            .json()
            .map_err(|e| anyhow::anyhow!("embeddings: {e}"))
    }

    fn vector(value: Option<&Value>) -> anyhow::Result<Vec<f64>> {
        let values = value
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow::anyhow!("embeddings: the response has no vector"))?;
        values
            .iter()
            .map(|v| {
                v.as_f64()
                    .ok_or_else(|| anyhow::anyhow!("embeddings: the response has a non-number"))
            })
            .collect()
    }

    async fn embed_titan(&self, document: &str) -> anyhow::Result<Vec<f64>> {
        let body = match self.family {
            EmbeddingFamily::TitanV2 => json!({"inputText": document, "normalize": true}),
            _ => json!({"inputText": document}),
        };
        let reply = self.invoke(&body).await?;
        Self::vector(reply.get("embedding"))
    }

    async fn embed_cohere(&self, documents: &[String]) -> anyhow::Result<Vec<Vec<f64>>> {
        let body = json!({"texts": documents, "input_type": "search_document"});
        let reply = self.invoke(&body).await?;
        let items = reply
            .get("embeddings")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow::anyhow!("embeddings: the response has no embeddings"))?;
        if items.len() != documents.len() {
            return Err(anyhow::anyhow!(
                "embeddings: the response holds {} vectors for {} inputs",
                items.len(),
                documents.len()
            ));
        }
        items.iter().map(|v| Self::vector(Some(v))).collect()
    }
}

#[async_trait::async_trait]
impl VizierEmbeddingModel for BedrockEmbeddings {
    async fn embed_text(&self, text: &str) -> anyhow::Result<Vec<f64>> {
        match self.family {
            EmbeddingFamily::TitanV2 | EmbeddingFamily::TitanV1 => self.embed_titan(text).await,
            EmbeddingFamily::CohereV3 => {
                let mut all = self.embed_cohere(&[text.to_string()]).await?;
                all.pop()
                    .ok_or_else(|| anyhow::anyhow!("embeddings: no vector returned"))
            }
        }
    }

    async fn embed_texts(&self, documents: Vec<String>) -> anyhow::Result<Vec<Vec<f64>>> {
        if documents.is_empty() {
            return Ok(vec![]);
        }
        match self.family {
            // Titan's InvokeModel takes one text per call; memory saves chunk passages already,
            // so the loop stays sequential and predictable.
            EmbeddingFamily::TitanV2 | EmbeddingFamily::TitanV1 => {
                let mut out = Vec::with_capacity(documents.len());
                for document in &documents {
                    out.push(self.embed_titan(document).await?);
                }
                Ok(out)
            }
            EmbeddingFamily::CohereV3 => {
                let mut out = Vec::with_capacity(documents.len());
                for chunk in documents.chunks(COHERE_BATCH) {
                    out.extend(self.embed_cohere(chunk).await?);
                }
                Ok(out)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFamily {
    NovaCanvas,
    TitanImageV2,
}

impl ImageFamily {
    pub fn detect(model: &str) -> Option<Self> {
        let id = family_id(model);
        if id == DEFAULT_IMAGE_MODEL {
            Some(Self::NovaCanvas)
        } else if id.starts_with("amazon.titan-image-generator-v2") {
            Some(Self::TitanImageV2)
        } else {
            None
        }
    }

    pub fn supported() -> &'static str {
        "Amazon Nova Canvas (amazon.nova-canvas-v1:0) and Titan Image Generator V2 \
         (amazon.titan-image-generator-v2:0)"
    }
}

/// Bedrock image generation: Nova Canvas or Titan Image Generator V2, one image per call.
pub struct BedrockImages {
    pub endpoint: Endpoint,
    pub family: ImageFamily,
}

impl BedrockImages {
    pub fn new(endpoint: Endpoint) -> Result<Self> {
        let family = ImageFamily::detect(&endpoint.model).ok_or_else(|| {
            VizierError(format!(
                "unsupported Bedrock image model {:?}: use {}",
                endpoint.model,
                ImageFamily::supported()
            ))
        })?;
        Ok(Self { endpoint, family })
    }
}

/// `WxH`, both sides required and numeric; absent is the documented 1024×1024. Provider limits
/// (Nova's 320–4096/multiple-of-16, Titan's table) are the provider's to enforce.
fn parse_size(size: Option<&str>) -> Result<(u32, u32)> {
    let Some(size) = size.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(DEFAULT_IMAGE_SIZE);
    };
    let parsed = size
        .split_once(['x', 'X'])
        .and_then(|(w, h)| Some((w.trim().parse::<u32>().ok()?, h.trim().parse::<u32>().ok()?)));
    parsed.ok_or_else(|| {
        VizierError(format!(
            "image generation: invalid size {size:?}; use WxH, e.g. 1024x1024"
        ))
    })
}

/// The image's real type from its magic bytes; never a default guess (FR-030).
fn sniff_image(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/// The request body shared by both families.
fn image_body(prompt: &str, (width, height): (u32, u32)) -> Value {
    json!({
        "taskType": "TEXT_IMAGE",
        "textToImageParams": {"text": prompt},
        "imageGenerationConfig": {
            "numberOfImages": 1,
            "quality": "standard",
            "width": width,
            "height": height,
            "cfgScale": 8.0,
        }
    })
}

#[async_trait::async_trait]
impl VizierImageGenModel for BedrockImages {
    async fn generate(&self, prompt: &str, size: Option<&str>) -> Result<(Vec<u8>, String)> {
        let dimensions = parse_size(size)?;
        let body = image_body(prompt, dimensions);
        let response = self
            .endpoint
            .post_json(&model_path(&self.endpoint.model, "invoke"), &body)
            .await
            .map_err(|e| VizierError(format!("image generation: {e}")))?;
        let reply = response
            .json()
            .map_err(|e| VizierError(format!("image generation: {e}")))?;
        // Guardrail/RAI deflection arrives as a 2xx with an error message and no images.
        if let Some(error) = reply
            .get("error")
            .and_then(Value::as_str)
            .filter(|e| !e.trim().is_empty())
        {
            return Err(VizierError(format!("image generation: {error}")));
        }
        let encoded = reply
            .pointer("/images/0")
            .and_then(Value::as_str)
            .ok_or_else(|| VizierError("image generation: no image was returned".into()))?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|e| VizierError(format!("image generation: {e}")))?;
        let mime = sniff_image(&bytes).ok_or_else(|| {
            VizierError("image generation: the image is not PNG, JPEG or WEBP".into())
        })?;
        Ok((bytes, mime.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn families_strip_cross_region_prefixes_and_refuse_unknown_models() {
        for id in [
            "amazon.titan-embed-text-v2:0",
            "us.amazon.titan-embed-text-v2:0",
            "eu.amazon.titan-embed-text-v2:0",
            "global.amazon.titan-embed-text-v2:0",
        ] {
            assert_eq!(
                EmbeddingFamily::detect(id),
                Some(EmbeddingFamily::TitanV2),
                "{id}"
            );
        }
        assert_eq!(
            EmbeddingFamily::detect("amazon.titan-embed-text-v1"),
            Some(EmbeddingFamily::TitanV1)
        );
        assert_eq!(
            EmbeddingFamily::detect("cohere.embed-english-v3"),
            Some(EmbeddingFamily::CohereV3)
        );
        assert_eq!(
            EmbeddingFamily::detect("apac.cohere.embed-multilingual-v3"),
            Some(EmbeddingFamily::CohereV3)
        );
        assert_eq!(
            EmbeddingFamily::detect("anthropic.claude-haiku-4-5-20251001-v1:0"),
            None
        );
        assert_eq!(EmbeddingFamily::detect(""), None);

        assert_eq!(
            ImageFamily::detect("amazon.nova-canvas-v1:0"),
            Some(ImageFamily::NovaCanvas)
        );
        assert_eq!(
            ImageFamily::detect("global.amazon.nova-canvas-v1:0"),
            Some(ImageFamily::NovaCanvas)
        );
        assert_eq!(
            ImageFamily::detect("amazon.titan-image-generator-v2:0"),
            Some(ImageFamily::TitanImageV2)
        );
        assert_eq!(
            ImageFamily::detect("stability.stable-diffusion-xl-v1"),
            None
        );
    }

    #[test]
    fn sizes_are_strict_and_default_to_1024() {
        assert_eq!(parse_size(None).unwrap(), (1024, 1024));
        assert_eq!(parse_size(Some("")).unwrap(), (1024, 1024));
        assert_eq!(parse_size(Some("1024x1024")).unwrap(), (1024, 1024));
        assert_eq!(parse_size(Some("1792x1024")).unwrap(), (1792, 1024));
        assert_eq!(
            parse_size(Some("100x100")).unwrap(),
            (100, 100),
            "the provider validates"
        );
        for bad in ["1024", "1024x", "x1024", "1024x1024x2", "big", "10.5x10"] {
            let err = parse_size(Some(bad)).unwrap_err().0;
            assert!(err.contains("WxH"), "{bad}: {err}");
        }
    }

    #[test]
    fn the_request_body_is_the_documented_shape() {
        assert_eq!(
            image_body("a red fox", (1024, 1024)),
            json!({
                "taskType": "TEXT_IMAGE",
                "textToImageParams": {"text": "a red fox"},
                "imageGenerationConfig": {
                    "numberOfImages": 1,
                    "quality": "standard",
                    "width": 1024,
                    "height": 1024,
                    "cfgScale": 8.0
                }
            })
        );
    }

    #[test]
    fn image_magic_bytes_decide_the_media_type() {
        assert_eq!(
            sniff_image(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0]),
            Some("image/png")
        );
        assert_eq!(sniff_image(&[0xff, 0xd8, 0xff, 0]), Some("image/jpeg"));
        assert_eq!(sniff_image(b"RIFF1234WEBP"), Some("image/webp"));
        assert_eq!(
            sniff_image(b"GIF89a"),
            None,
            "GIF is not requested from these models"
        );
        assert_eq!(sniff_image(b"not an image"), None);
    }

    #[test]
    fn vectors_are_numbers_only() {
        assert_eq!(
            BedrockEmbeddings::vector(Some(&json!([0.5, 1.0]))).unwrap(),
            vec![0.5, 1.0]
        );
        assert!(BedrockEmbeddings::vector(Some(&json!(["x"]))).is_err());
        assert!(BedrockEmbeddings::vector(None).is_err());
    }
}
